using System.Windows;
using AppUsageTracker.App.Services;
using AppUsageTracker.App.ViewModels;
using AppUsageTracker.Core;

namespace AppUsageTracker.App.Views;

/// <summary>设置窗口：保存后热应用监控参数（先落盘再重启监控服务）。</summary>
public partial class SettingsWindow : Window
{
    private readonly App _app;

    public SettingsWindow(App app)
    {
        InitializeComponent();
        _app = app;
        Owner = app.Windows.OfType<DashboardWindow>().FirstOrDefault();
        DataContext = new SettingsViewModel(app.Settings, ApplySettings);
        ApplyChrome();
        ThemeManager.ThemeChanged += OnThemeChanged;
        Closed += (_, _) => ThemeManager.ThemeChanged -= OnThemeChanged;
    }

    private void OnThemeChanged(string theme) =>
        Dispatcher.BeginInvoke(() => WindowChrome.Apply(this, ThemeManager.EffectiveTheme == "Dark"));

    private void ApplyChrome() => WindowChrome.Apply(this, ThemeManager.EffectiveTheme == "Dark");

    private void OnCloseClick(object sender, RoutedEventArgs e) => Close();

    private void ApplySettings(TrackerSettings updated)
    {
        try
        {
            _app.Service.ApplySettings(updated);
            // 悬浮窗开关实时生效；分类规则等变化让仪表盘立即重载数据
            _app.SetWidgetVisible(updated.ShowFloatingWidget);
            _app.Windows.OfType<DashboardWindow>().FirstOrDefault()?.RefreshData();
        }
        catch (Exception ex)
        {
            MessageBox.Show("应用新设置失败：" + ex.Message, "设置",
                MessageBoxButton.OK, MessageBoxImage.Warning);
        }
    }

    private void OnSaveClick(object sender, RoutedEventArgs e)
    {
        if (DataContext is SettingsViewModel vm)
        {
            var error = vm.Save();
            if (error is not null)
            {
                MessageBox.Show(error, "设置有误", MessageBoxButton.OK, MessageBoxImage.Warning);
                return;
            }
        }

        // 本窗口以 Show() 非模态打开，不能设置 DialogResult，直接关闭即可
        Close();
    }
}
