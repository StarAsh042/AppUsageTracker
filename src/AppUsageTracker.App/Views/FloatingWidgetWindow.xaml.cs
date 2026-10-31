using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Threading;
using AppUsageTracker.App.Services;
using AppUsageTracker.App.Utils;
using AppUsageTracker.Core.Storage;
using ContextMenu = System.Windows.Controls.ContextMenu;
using MenuItem = System.Windows.Controls.MenuItem;

namespace AppUsageTracker.App.Views;

/// <summary>
/// 桌面悬浮窗：置顶显示今日活跃总时长（秒表实时跳动）与当前前台应用的今日累计时长。
/// 口径与仪表盘一致：每秒直读数据库（区间闭合时服务即时落库）+ 当前未闭合区间。
/// 左键拖动移动位置（拖完记忆），右键弹出菜单；DWM 圆角 + Acrylic 磨砂。
/// </summary>
public partial class FloatingWidgetWindow : Window
{
    private readonly App _app;
    private readonly DispatcherTimer _timer;

    /// <summary>今日已落库的总活跃秒数缓存（每 30 秒刷新一次）。</summary>
    private double _todayBaseSeconds;
    private DateTime _lastBaseFetch = DateTime.MinValue;

    public FloatingWidgetWindow(App app)
    {
        InitializeComponent();
        _app = app;
        Icon = AppIconCache.GetAppIconImage();
        WindowChrome.Apply(this, ThemeManager.EffectiveTheme == "Dark");

        _timer = new DispatcherTimer { Interval = TimeSpan.FromSeconds(1) };
        _timer.Tick += (_, _) => Tick();
        _timer.Start();

        ThemeManager.ThemeChanged += OnThemeChanged;
        Closed += (_, _) =>
        {
            ThemeManager.ThemeChanged -= OnThemeChanged;
            _timer.Stop();
        };

        Loaded += (_, _) => RestorePosition();
    }

    private void OnThemeChanged(string theme) =>
        Dispatcher.BeginInvoke(() => WindowChrome.Apply(this, theme == "Dark"));

    private void RestorePosition()
    {
        var s = _app.Settings;
        var area = SystemParameters.WorkArea;
        if (s.WidgetX >= 0 && s.WidgetY >= 0
            && s.WidgetX < area.Width - 40 && s.WidgetY < area.Height - 40)
        {
            Left = s.WidgetX;
            Top = s.WidgetY;
        }
        else
        {
            // 默认贴屏幕左上角
            Left = 14;
            Top = 12;
        }
    }

    private void Tick()
    {
        var now = DateTime.Now;

        if ((now - _lastBaseFetch).TotalSeconds >= 30)
        {
            _lastBaseFetch = now;
            var key = Format.DateKey(DateTime.Today);
            _ = Task.Run(() => _app.Store.QueryDailyTotals(key, key))
                .ContinueWith(t =>
                {
                    if (t.IsCompletedSuccessfully) _todayBaseSeconds = t.Result.FirstOrDefault()?.ActiveSeconds ?? 0;
                }, TaskScheduler.FromCurrentSynchronizationContext());
        }

        var segment = _app.Service.CurrentSegmentSeconds();
        TotalText.Text = Format.Duration(_todayBaseSeconds + segment);

        var current = _app.Service.CurrentApp;
        var selfKey = Environment.ProcessPath?.ToLowerInvariant();

        if (current is not null
            && !string.Equals(current.AppKey, selfKey, StringComparison.OrdinalIgnoreCase))
        {
            CurrentAppText.Text = current.DisplayName;
            CurrentAppSeconds.Text = Format.Duration(segment);
        }
        else
        {
            CurrentAppText.Text = _app.Service.State == Core.Models.TrackingState.Idle ? "空闲中" : "当前无可统计应用";
            CurrentAppSeconds.Text = "";
        }
    }

    private void OnDragMove(object sender, MouseButtonEventArgs e)
    {
        if (e.ButtonState != MouseButtonState.Pressed) return;
        try
        {
            DragMove();
            // 拖动结束记忆位置
            _app.Settings.WidgetX = (int)Left;
            _app.Settings.WidgetY = (int)Top;
            _app.Settings.Save();
        }
        catch { /* 窗口正在非激活态拖动等边缘情况 */ }
    }

    private void OnContextMenu(object sender, MouseButtonEventArgs e)
    {
        var menu = new ContextMenu();
        var openItem = new MenuItem { Header = "打开仪表盘" };
        openItem.Click += (_, _) => _app.ShowDashboard();
        var hideItem = new MenuItem { Header = "隐藏悬浮窗" };
        hideItem.Click += (_, _) => _app.SetWidgetVisible(false);
        menu.Items.Add(openItem);
        menu.Items.Add(hideItem);
        menu.IsOpen = true;
    }
}
