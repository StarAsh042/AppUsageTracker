using System.Diagnostics;
using System.IO;
using System.Text.Json;
using System.Windows;
using System.Windows.Threading;
using AppUsageTracker.App.Services;
using AppUsageTracker.App.Views;
using AppUsageTracker.Core;
using AppUsageTracker.Core.Logging;
using AppUsageTracker.Core.Storage;
using AppUsageTracker.Core.Tracking;

namespace AppUsageTracker.App;

/// <summary>
/// 应用入口：负责单实例控制、服务装配、托盘初始化与退出清理。
/// 另提供 <c>--smoke N</c> 无界面冒烟模式，供自动化验证统计链路。
/// </summary>
public partial class App : Application
{
    private const string InstanceMutexName = @"Local\AppUsageTracker.Instance";
    private const string ShowUiEventName = @"Local\AppUsageTracker.ShowUI";

    private static readonly JsonSerializerOptions ReportJsonOptions = new() { WriteIndented = true };

    private Mutex? _instanceMutex;
    private EventWaitHandle? _showUiSignal;
    private System.Threading.RegisteredWaitHandle? _showUiWait;
    private bool _smokeMode;
    private bool _ownsMutex;

    public TrackerSettings Settings { get; private set; } = null!;
    public IUsageStore Store { get; private set; } = null!;
    public TrackingService Service { get; private set; } = null!;
    public TrayController? Tray { get; private set; }
    public bool IsShuttingDown { get; private set; }



    protected override void OnStartup(StartupEventArgs e)
    {
        base.OnStartup(e);

        // 冒烟模式：--smoke N（秒）
        var smokeIndex = Array.IndexOf(e.Args, "--smoke");
        if (smokeIndex >= 0 && smokeIndex + 1 < e.Args.Length && int.TryParse(e.Args[smokeIndex + 1], out var smokeSeconds))
        {
            _smokeMode = true;
            _ = RunSmokeAsync(smokeSeconds);
            return;
        }

        DispatcherUnhandledException += OnDispatcherUnhandledException;

        // WPF 绑定错误默认静默，转发到文件日志便于排查
        PresentationTraceSources.DataBindingSource.Listeners.Add(new TraceListenerProxy());
        PresentationTraceSources.DataBindingSource.Switch.Level = SourceLevels.Error;

        _instanceMutex = new Mutex(true, InstanceMutexName, out var createdNew);
        _ownsMutex = createdNew;
        if (!createdNew)
        {
            // 已有实例在运行：通知其显示主窗口后退出
            try { EventWaitHandle.OpenExisting(ShowUiEventName).Set(); } catch { /* 首实例尚未创建信号 */ }
            Shutdown();
            return;
        }

        Bootstrap();
        // 外壳阶段：托盘常驻，主界面窗口在后续提交引入
    }

    private void Bootstrap()
    {
        Settings = TrackerSettings.Load();
        Logger.Init(Path.Combine(Settings.DataDirectory, "logs"));
        EnsureSelfExcluded();

        ThemeManager.Apply(Settings.Theme);
        Store = new SqliteUsageStore(Path.Combine(Settings.DataDirectory, "usage.db"));
        Store.Initialize();

        Service = new TrackingService(Settings, Store);
        Service.Start();
        Service.RestReminderDue += continuous =>
            Dispatcher.BeginInvoke(() => Tray?.NotifyRest(continuous));

        Tray = new TrayController(this);
        Tray.Initialize();

        // 默认只开悬浮窗，主界面由托盘双击/菜单唤起；关闭悬浮窗则纯托盘运行
        if (Settings.ShowFloatingWidget)
        {
            Dispatcher.BeginInvoke(ShowWidget);
        }
    }

    /// <summary>把自身加入排除名单（托盘程序自己不该被计入使用时间）。</summary>
    private void EnsureSelfExcluded()
    {
        var selfKey = Environment.ProcessPath?.ToLowerInvariant();
        if (string.IsNullOrEmpty(selfKey)) return;
        if (Settings.ExcludedAppKeys.Contains(selfKey, StringComparer.OrdinalIgnoreCase)) return;
        Settings.ExcludedAppKeys.Add(selfKey);
        Settings.Save();
    }


    public void ExitApplication()
    {
        IsShuttingDown = true;
        try
        {
            Tray?.Dispose();
            Service.Stop();
            Store.Dispose();
        }
        catch (Exception ex)
        {
            Logger.Error("退出清理异常", ex);
        }
        finally
        {
            Shutdown();
        }
    }

    private void OnDispatcherUnhandledException(object sender, DispatcherUnhandledExceptionEventArgs e)
    {
        Logger.Error("UI 线程未处理异常", e.Exception);
        MessageBox.Show("发生未处理的错误，程序将继续运行。详情见日志文件。\n\n" + e.Exception.Message,
            "软件使用时间统计", MessageBoxButton.OK, MessageBoxImage.Warning);
        e.Handled = true;
    }

    /// <summary>把 WPF 绑定错误转发到文件日志。</summary>
    private sealed class TraceListenerProxy : TraceListener
    {
        public override void Write(string? message) { }
        public override void WriteLine(string? message) => Logger.Warn("绑定: " + message);
    }

    protected override void OnExit(ExitEventArgs e)
    {
        _showUiWait?.Unregister(null);
        _showUiSignal?.Dispose();

        if (!_smokeMode)
        {
            try { Service.Stop(); } catch { }
            try { Store.Dispose(); } catch { }
        }

        // 只有持有者才能释放（createdNew=false 的第二个实例从未取得所有权）
        try { if (_ownsMutex) _instanceMutex?.ReleaseMutex(); } catch { }
        _instanceMutex?.Dispose();
        base.OnExit(e);
    }

    /// <summary>
    /// 无界面冒烟模式：监控 N 秒后落库，把统计摘要写入 数据目录\smoke-report.json，
    /// 退出码 0 表示数据库有当日数据，2 表示没有。
    /// </summary>
    private async Task RunSmokeAsync(int seconds)
    {
        seconds = Math.Clamp(seconds, 3, 3600);
        try
        {
            Settings = TrackerSettings.Load();
            Logger.Init(Path.Combine(Settings.DataDirectory, "logs"));
            EnsureSelfExcluded();

            Store = new SqliteUsageStore(Path.Combine(Settings.DataDirectory, "usage.db"));
            Store.Initialize();

            Service = new TrackingService(Settings, Store);
            Service.Start();

            Logger.Info($"冒烟模式：监控 {seconds} 秒…");
            await Task.Delay(TimeSpan.FromSeconds(seconds));

            Service.Stop();

            var today = DateTime.Today.ToString("yyyy-MM-dd");
            var daily = Store.QueryDaily(today, today);

            var report = new
            {
                StartedAt = DateTime.Now.ToString("yyyy-MM-dd HH:mm:ss"),
                DurationSeconds = seconds,
                DailyAppCount = daily.Count,
                ForegroundRows = Store.QueryIntervals(today, today).Count,
                TodayActiveSeconds = daily.Sum(d => d.ActiveSeconds),
                Apps = daily.Where(d => d.ActiveSeconds > 0).Select(d => new
                {
                    d.DisplayName,
                    ActiveSeconds = Math.Round(d.ActiveSeconds, 1),
                }).ToList(),
            };

            var reportPath = Path.Combine(Settings.DataDirectory, "smoke-report.json");
            File.WriteAllText(reportPath, JsonSerializer.Serialize(report, ReportJsonOptions));
            Console.WriteLine(reportPath);

            Environment.ExitCode = daily.Count > 0 ? 0 : 2;
        }
        catch (Exception ex)
        {
            Logger.Error("冒烟模式异常", ex);
            Environment.ExitCode = 1;
        }
        finally
        {
            Shutdown();
        }
    }
}
