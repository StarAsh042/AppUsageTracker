using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Threading;
using AppUsageTracker.App.Services;
using AppUsageTracker.App.Utils;
using AppUsageTracker.Core.Storage;
using System.Linq;
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

    /// <summary>今日已落库的总活跃秒数与按应用的活跃秒数缓存（每秒直读数据库刷新）。</summary>
    private double _todayBaseSeconds;
    private Dictionary<string, double> _appTodaySeconds = new(StringComparer.OrdinalIgnoreCase);
    private bool _baseLoaded;
    private bool _fetching;

    public FloatingWidgetWindow(App app)
    {
        InitializeComponent();
        _app = app;
        Icon = AppIconCache.GetAppIconImage();
        WindowChrome.ApplyFloating(this, ThemeManager.EffectiveTheme == "Dark");

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
        Dispatcher.BeginInvoke(() => WindowChrome.ApplyFloating(this, theme == "Dark"));

    private void RestorePosition()
    {
        var s = _app.Settings;
        ClampToScreen(s.WidgetX, s.WidgetY);
    }

    /// <summary>把悬浮窗位置钳制在工作区内，任何情况下不超出屏幕上下左右边界。</summary>
    private void ClampToScreen(double x, double y)
    {
        var area = SystemParameters.WorkArea;
        var maxLeft = Math.Max(0, area.Right - ActualWidth);
        var maxTop = Math.Max(0, area.Bottom - ActualHeight);
        Left = Math.Clamp(x, 0, maxLeft);
        Top = Math.Clamp(y, 0, maxTop);
    }

    /// <summary>有分类达到日限额时把状态点亮为警示色（用户选择的变色提醒）。</summary>
    private void RefreshLimitState(string todayKey)
    {
        try
        {
            var resolver = new Core.Monitoring.CategoryResolver(_app.Settings.CategoryRules);
            var over = _app.Store.QueryDaily(todayKey, todayKey)
                .Any(row => _app.Settings.CategoryDailyLimitMinutes.TryGetValue(
                               resolver.Resolve(row.DisplayName, row.ProcessName, row.ExePath), out var m)
                           && m > 0 && row.ActiveSeconds >= m * 60.0);
            LiveDot.SetResourceReference(System.Windows.Controls.TextBlock.ForegroundProperty,
                over ? "WarnBrush" : "OkBrush");
        }
        catch { }
    }

    private void Tick()
    {
        var now = DateTime.Now;

        if (!_fetching)
        {
            _fetching = true;
            var key = Format.DateKey(DateTime.Today);
            _ = Task.Run(() => (Totals: _app.Store.QueryDailyTotals(key, key), PerApp: _app.Store.QueryDaily(key, key)))
                .ContinueWith(t =>
                {
                    _fetching = false;
                    if (!t.IsCompletedSuccessfully) return;
                    _todayBaseSeconds = t.Result.Totals.FirstOrDefault(p => p.UsageDate == key)?.ActiveSeconds ?? 0;
                    _appTodaySeconds = t.Result.PerApp.ToDictionary(d => d.AppKey, d => d.ActiveSeconds, StringComparer.OrdinalIgnoreCase);
                    _baseLoaded = true;
                    Tick(); // 数据到位后立即刷新本秒显示
                }, TaskScheduler.FromCurrentSynchronizationContext());
        }

        if (!_baseLoaded) return;

        var segment = _app.Service.CurrentSegmentSeconds();
        TotalText.Text = Format.Duration(_todayBaseSeconds + segment);

        var current = _app.Service.CurrentApp;
        var selfKey = Environment.ProcessPath?.ToLowerInvariant();

        if (current is not null
            && !string.Equals(current.AppKey, selfKey, StringComparison.OrdinalIgnoreCase)
            && !_app.Settings.ExcludedAppKeys.Contains(current.AppKey, StringComparer.OrdinalIgnoreCase))
        {
            // 与仪表盘口径一致：该应用今日累计 = 已落库部分 + 当前未闭合区间
            var appSeconds = _appTodaySeconds.GetValueOrDefault(current.AppKey) + segment;
            CurrentAppText.Text = current.DisplayName;
            CurrentAppSeconds.Text = Format.Duration(appSeconds);
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
            ClampToScreen(Left, Top); // 拖动后钳制在屏幕边界内
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
