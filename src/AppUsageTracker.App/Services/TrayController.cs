using System.Drawing;
using System.Windows.Forms;
using AppUsageTracker.App.Utils;
using AppUsageTracker.Core;
using AppUsageTracker.Core.Models;
using AppUsageTracker.Core.Tracking;

namespace AppUsageTracker.App.Services;

/// <summary>系统托盘图标与右键菜单（菜单配色跟随界面主题）。</summary>
public sealed class TrayController : IDisposable
{
    private readonly App _app;
    private NotifyIcon _icon = null!;
    private ContextMenuStrip _menu = null!;
    private ToolStripMenuItem _pauseItem = null!;
    private ToolStripMenuItem _autostartItem = null!;
    private ToolStripMenuItem _widgetItem = null!;
    private System.Timers.Timer? _tooltipTimer;

    public TrayController(App app) => _app = app;

    public void Initialize()
    {
        var menu = new ContextMenuStrip();

        var openItem = new ToolStripMenuItem("打开仪表盘");
        openItem.Font = new Font(openItem.Font, FontStyle.Bold);
        openItem.Click += (_, _) => _app.Dispatcher.BeginInvoke(() => _app.ShowDashboard());

        _widgetItem = new ToolStripMenuItem("悬浮窗");
        _widgetItem.CheckOnClick = true;
        _widgetItem.Checked = _app.Settings.ShowFloatingWidget;
        _widgetItem.Click += (_, _) => _app.Dispatcher.BeginInvoke(() =>
        {
            _app.SetWidgetVisible(_widgetItem.Checked);
        });

        _pauseItem = new ToolStripMenuItem("暂停监控");
        _pauseItem.CheckOnClick = true;
        _pauseItem.Click += (_, _) =>
        {
            if (_pauseItem.Checked) _app.Service.Pause();
            else _app.Service.Resume();
        };

        _autostartItem = new ToolStripMenuItem("开机自启");
        _autostartItem.CheckOnClick = true;
        _autostartItem.Checked = AutoStartManager.IsEnabled();
        _autostartItem.Click += (_, _) =>
        {
            try
            {
                AutoStartManager.SetEnabled(_autostartItem.Checked);
            }
            catch (Exception ex)
            {
                _autostartItem.Checked = !_autostartItem.Checked;
                MessageBox.Show("设置开机自启失败：" + ex.Message, "软件使用时间统计",
                    System.Windows.MessageBoxButton.OK, System.Windows.MessageBoxImage.Warning);
            }
        };

        var settingsItem = new ToolStripMenuItem("设置…");
        settingsItem.Click += (_, _) => _app.Dispatcher.BeginInvoke(_app.ShowSettings);

        var exitItem = new ToolStripMenuItem("退出");
        exitItem.Click += (_, _) => _app.Dispatcher.BeginInvoke(_app.ExitApplication);

        menu.Items.AddRange(new ToolStripItem[]
        {
            openItem,
            new ToolStripSeparator(),
            _widgetItem,
            _pauseItem,
            _autostartItem,
            settingsItem,
            new ToolStripSeparator(),
            exitItem,
        });

        _menu = menu;

        _icon = new NotifyIcon
        {
            Icon = AppIconCache.GetAppIcon(),
            Text = "软件使用时间统计",
            ContextMenuStrip = menu,
            Visible = true,
        };
        _icon.DoubleClick += (_, _) => _app.Dispatcher.BeginInvoke(() => _app.ShowDashboard());

        _app.Service.StateChanged += _ => _app.Dispatcher.BeginInvoke(RefreshMenuState);

        // 定期刷新托盘提示中的「今日活跃」
        _tooltipTimer = new System.Timers.Timer(60_000);
        _tooltipTimer.Elapsed += (_, _) => _app.Dispatcher.BeginInvoke(UpdateTooltip);
        _tooltipTimer.Start();

        ApplyMenuTheme(ThemeManager.EffectiveTheme == "Dark");
        ThemeManager.ThemeChanged += OnThemeChanged;
        RefreshMenuState();
        UpdateTooltip();
    }

    private void OnThemeChanged(string theme) =>
        _app.Dispatcher.BeginInvoke(() => ApplyMenuTheme(theme == "Dark"));

    /// <summary>让托盘菜单配色与界面主题一致（深色磨砂 / 浅色），圆角 + 半透明呼应磨砂玻璃。</summary>
    private void ApplyMenuTheme(bool dark)
    {
        var bg = dark ? Color.FromArgb(28, 28, 32) : Color.FromArgb(246, 246, 248);
        var hover = dark ? Color.FromArgb(56, 56, 62) : Color.FromArgb(229, 229, 234);
        var border = dark ? Color.FromArgb(60, 60, 66) : Color.FromArgb(212, 212, 218);
        var separator = dark ? Color.FromArgb(46, 46, 52) : Color.FromArgb(224, 224, 230);
        var text = dark ? Color.FromArgb(242, 242, 245) : Color.FromArgb(24, 24, 27);

        _menu.Renderer = new ToolStripProfessionalRenderer(new TrayMenuColorTable(bg, hover, border, separator));
        _menu.BackColor = bg;
        _menu.ForeColor = text;
        _menu.Opacity = dark ? 0.88 : 0.92; // 半透明呼应磨砂玻璃
        _menu.ShowImageMargin = false;
        _menu.ShowCheckMargin = true; // 勾选标记在专用列显示（悬浮窗/暂停/自启）
        _menu.Font = new Font("Microsoft YaHei UI", 9f);

        // 圆角菜单（裁切区域）
        var radius = 10;
        var path = new System.Drawing.Drawing2D.GraphicsPath();
        var w = _menu.Width > 0 ? _menu.Width : 220;
        var h = _menu.Height > 0 ? _menu.Height : 200;
        path.AddArc(0, 0, radius * 2, radius * 2, 180, 90);
        path.AddArc(w - radius * 2, 0, radius * 2, radius * 2, 270, 90);
        path.AddArc(w - radius * 2, h - radius * 2, radius * 2, radius * 2, 0, 90);
        path.AddArc(0, h - radius * 2, radius * 2, radius * 2, 90, 90);
        path.CloseFigure();
        _menu.Region = new Region(path);
    }

    /// <summary>托盘菜单的专业配色表（覆盖下拉背景/悬停/边框/分隔线）。</summary>
    private sealed class TrayMenuColorTable(Color bg, Color hover, Color border, Color separator)
        : ProfessionalColorTable
    {
        private readonly Color _bg = bg;
        private readonly Color _hover = hover;
        private readonly Color _border = border;
        private readonly Color _separator = separator;

        public override Color ToolStripDropDownBackground => _bg;
        public override Color ImageMarginGradientBegin => _bg;
        public override Color ImageMarginGradientMiddle => _bg;
        public override Color ImageMarginGradientEnd => _bg;
        public override Color MenuBorder => _border;
        public override Color MenuItemBorder => Color.Transparent;
        public override Color MenuItemSelected => _hover;
        public override Color MenuItemSelectedGradientBegin => _hover;
        public override Color MenuItemSelectedGradientEnd => _hover;
        public override Color MenuItemPressedGradientBegin => _hover;
        public override Color MenuItemPressedGradientEnd => _hover;
        public override Color SeparatorDark => _separator;
        public override Color SeparatorLight => _separator;
        public override Color CheckBackground => _hover;
        public override Color CheckSelectedBackground => _hover;
        public override Color CheckPressedBackground => _hover;
    }

    /// <summary>悬浮窗显隐变化时同步托盘菜单勾选状态。</summary>
    public void SyncWidgetItem(bool visible)
    {
        if (_widgetItem is not null) _widgetItem.Checked = visible;
    }

    /// <summary>首次启动的气泡提示。</summary>
    public void NotifyFirstRun()
    {
        _icon.BalloonTipTitle = "软件使用时间统计已启动";
        _icon.BalloonTipText = "正在后台记录各软件的使用时间。双击托盘图标可查看统计报表。";
        _icon.ShowBalloonTip(3000);
    }

    /// <summary>久坐提醒气泡。</summary>
    public void NotifyRest(TimeSpan continuous)
    {
        if (_icon is null) return;
        var hours = (int)continuous.TotalHours;
        var minutes = continuous.Minutes;
        _icon.BalloonTipTitle = "该休息一下了 ☕";
        _icon.BalloonTipText = $"您已连续使用电脑 {hours} 小时 {minutes} 分钟，起来活动一下吧。";
        _icon.ShowBalloonTip(6000);
    }

    private void RefreshMenuState()
    {
        var state = _app.Service.State;
        _pauseItem.Checked = state == TrackingState.Paused;
        UpdateTooltip();
    }

    private void UpdateTooltip()
    {
        try
        {
            var today = Format.DateKey(DateTime.Today);
            var totals = _app.Store.QueryDailyTotals(today, today);
            var active = totals.FirstOrDefault()?.ActiveSeconds ?? 0;
            var text = $"软件使用时间统计\n今日活跃：{Format.Duration(active)}";
            // NotifyIcon.Text 最长 63 字符
            _icon.Text = text.Length <= 63 ? text : text[..63];
        }
        catch
        {
            _icon.Text = "软件使用时间统计";
        }
    }

    public void Dispose()
    {
        _tooltipTimer?.Dispose();
        _tooltipTimer = null;
        ThemeManager.ThemeChanged -= OnThemeChanged;
        if (_icon is not null)
        {
            _icon.Visible = false;
            _icon.Dispose();
        }
    }
}
