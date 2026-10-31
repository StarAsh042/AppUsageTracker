using System.ComponentModel;
using System.Windows;
using System.Windows.Media;
using AppUsageTracker.App.Services;

namespace AppUsageTracker.App.Views;

/// <summary>仪表盘主窗口：无边框磨砂玻璃 + 自绘标题栏；点关闭只是隐藏到托盘。</summary>
public partial class DashboardWindow : Window
{
    private readonly App _app;

    public DashboardWindow(App app)
    {
        InitializeComponent();
        _app = app;
        Icon = AppIconCache.GetAppIconImage();
        DataContext = new ViewModels.DashboardViewModel(app.Store, app.Service, app.Settings, app.ShowSettings);

        ApplyChrome();
        // 主题切换（含跟随系统）时同步磨砂色调并重绘自绘控件
        ThemeManager.ThemeChanged += OnThemeChanged;
        Closed += (_, _) => ThemeManager.ThemeChanged -= OnThemeChanged;
    }

    private void OnThemeChanged(string theme)
    {
        Dispatcher.BeginInvoke(() =>
        {
            ApplyChrome();
            InvalidateAllVisuals(this);
        });
    }

    private void ApplyChrome() => WindowChrome.Apply(this, ThemeManager.EffectiveTheme == "Dark");

    private void OnMinimizeClick(object sender, RoutedEventArgs e) => WindowState = WindowState.Minimized;

    private void OnMaximizeClick(object sender, RoutedEventArgs e)
    {
        WindowState = WindowState == WindowState.Maximized ? WindowState.Normal : WindowState.Maximized;
        MaximizeButton.Content = WindowState == WindowState.Maximized ? "\uE923" : "\uE922";
    }

    private void OnHideClick(object sender, RoutedEventArgs e) => Hide();

    /// <summary>设置保存后由设置窗口调用，重载仪表盘数据。</summary>
    public void RefreshData()
    {
        if (DataContext is ViewModels.DashboardViewModel vm) vm.RefreshCommand.Execute(null);
    }

    private static void InvalidateAllVisuals(DependencyObject root)
    {
        for (var i = 0; i < VisualTreeHelper.GetChildrenCount(root); i++)
        {
            var child = VisualTreeHelper.GetChild(root, i);
            if (child is FrameworkElement element) element.InvalidateVisual();
            InvalidateAllVisuals(child);
        }
    }

    protected override void OnClosing(CancelEventArgs e)
    {
        if (!_app.IsShuttingDown)
        {
            e.Cancel = true;
            Hide();
        }
        base.OnClosing(e);
    }
}
