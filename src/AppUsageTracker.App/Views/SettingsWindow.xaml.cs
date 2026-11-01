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
            vm.LimitsText = LimitsText.Text;
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

    private void OnExportJsonClick(object sender, RoutedEventArgs e)
    {
        var dialog = new Microsoft.Win32.SaveFileDialog
        {
            Title = "导出 JSON 备份",
            Filter = "JSON (*.json)|*.json",
            FileName = $"usage-backup_{DateTime.Now:yyyyMMdd_HHmmss}.json",
        };
        if (dialog.ShowDialog() != true) return;
        try
        {
            var from = DateTime.Today.AddDays(-365);
            var to = DateTime.Today;
            Core.Storage.BackupService.Export(_app.Store, dialog.FileName, from, to);
            MessageBox.Show("导出完成。", "备份", MessageBoxButton.OK, MessageBoxImage.Information);
        }
        catch (Exception ex)
        {
            MessageBox.Show("导出失败：" + ex.Message, "备份", MessageBoxButton.OK, MessageBoxImage.Error);
        }
    }

    private void OnImportJsonClick(object sender, RoutedEventArgs e)
    {
        var dialog = new Microsoft.Win32.OpenFileDialog { Title = "导入 JSON 备份", Filter = "JSON (*.json)|*.json" };
        if (dialog.ShowDialog() != true) return;

        var choice = MessageBox.Show(
            "「是」= 合并导入（跳过已有记录）\n「否」= 替换导入（清空后导入，不可撤销）",
            "导入模式", MessageBoxButton.YesNoCancel, MessageBoxImage.Question);
        if (choice == MessageBoxResult.Cancel) return;
        var replace = choice == MessageBoxResult.No;
        if (replace)
        {
            var confirm = MessageBox.Show("替换将清空现有全部数据，确定？",
                "确认替换", MessageBoxButton.YesNo, MessageBoxImage.Warning);
            if (confirm != MessageBoxResult.Yes) return;
        }
        try
        {
            if (replace) _app.Service.PrepareForReplaceImport();
            var (daily, intervals) = Core.Storage.BackupService.Import(_app.Store, dialog.FileName, replace);
            MessageBox.Show($"导入完成：{daily} 条聚合，{intervals} 条明细。", "备份",
                MessageBoxButton.OK, MessageBoxImage.Information);
            _app.Windows.OfType<DashboardWindow>().FirstOrDefault()?.RefreshData();
        }
        catch (Exception ex)
        {
            MessageBox.Show("导入失败：" + ex.Message, "备份", MessageBoxButton.OK, MessageBoxImage.Error);
        }
    }
}
