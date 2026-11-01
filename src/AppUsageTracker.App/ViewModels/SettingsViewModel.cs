using System.IO;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using AppUsageTracker.App.Services;
using AppUsageTracker.Core;
using AppUsageTracker.Core.Models;
using AppUsageTracker.Core.Monitoring;

namespace AppUsageTracker.App.ViewModels;

/// <summary>设置窗口视图模型。</summary>
public partial class SettingsViewModel : ObservableObject
{
    private readonly TrackerSettings _settings;
    private readonly Action<TrackerSettings> _applySettings;

    [ObservableProperty] private int _idleThresholdSeconds;
    [ObservableProperty] private int _foregroundPollMs;
    [ObservableProperty] private int _restReminderMinutes;
    [ObservableProperty] private int _retentionDays;
    [ObservableProperty] private bool _showFloatingWidget;
    [ObservableProperty] private string _limitsText = "";
    [ObservableProperty] private bool _startWithWindows;
    [ObservableProperty] private string _excludedAppsText = "";
    [ObservableProperty] private string _categoryRulesText = "";
    [ObservableProperty] private string _dataDirectory = "";

    /// <summary>主题选择：Auto / Dark / Light。</summary>
    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(IsAutoTheme))]
    [NotifyPropertyChangedFor(nameof(IsDarkThemePill))]
    [NotifyPropertyChangedFor(nameof(IsLightThemePill))]
    private string _themeMode = "Auto";

    public bool IsAutoTheme { get => ThemeMode == "Auto"; set { if (value) ThemeMode = "Auto"; } }
    public bool IsDarkThemePill { get => ThemeMode == "Dark"; set { if (value) ThemeMode = "Dark"; } }
    public bool IsLightThemePill { get => ThemeMode == "Light"; set { if (value) ThemeMode = "Light"; } }

    public SettingsViewModel(TrackerSettings settings, Action<TrackerSettings> applySettings)
    {
        _settings = settings;
        _applySettings = applySettings;

        IdleThresholdSeconds = settings.IdleThresholdSeconds;
        ForegroundPollMs = settings.ForegroundPollMs;
        RestReminderMinutes = settings.RestReminderMinutes;
        RetentionDays = settings.RetentionDays;
        ShowFloatingWidget = settings.ShowFloatingWidget;
        StartWithWindows = AutoStartManager.IsEnabled();
        ThemeMode = settings.Theme is "Dark" or "Light" ? settings.Theme : "Auto";
        DataDirectory = settings.DataDirectory;
        ExcludedAppsText = string.Join(Environment.NewLine, settings.ExcludedAppKeys);
        CategoryRulesText = string.Join(Environment.NewLine,
            settings.CategoryRules.Select(r => r.Name + "=" + string.Join("|", r.Keywords)));
    }

    /// <summary>把当前前台程序加入排除名单（AppKey 即 exe 完整路径的小写）。</summary>
    [RelayCommand]
    private void AddForegroundApp()
    {
        var app = ForegroundMonitor.ProbeForeground();
        if (app is null) return;

        var lines = SplitLines(ExcludedAppsText).ToList();
        if (!lines.Contains(app.AppKey, StringComparer.OrdinalIgnoreCase))
        {
            lines.Add(app.AppKey);
            ExcludedAppsText = string.Join(Environment.NewLine, lines);
        }
    }

    [RelayCommand]
    private void ResetCategoryRules()
    {
        CategoryRulesText = string.Join(Environment.NewLine,
            TrackerSettings.CreateDefaultCategoryRules().Select(r => r.Name + "=" + string.Join("|", r.Keywords)));
    }

    [RelayCommand]
    private void OpenDataFolder()
    {
        try
        {
            System.Diagnostics.Process.Start("explorer.exe", DataDirectory);
        }
        catch { /* 目录可能不存在 */ }
    }

    /// <summary>校验并保存。成功返回 null，失败返回错误信息。</summary>
    public string? Save()
    {
        if (IdleThresholdSeconds is < 10 or > 3600) return "空闲阈值须在 10 ~ 3600 秒之间";
        if (ForegroundPollMs is < 200 or > 10_000) return "前台轮询间隔须在 200 ~ 10000 毫秒之间";
        if (RestReminderMinutes is < 0 or > 480) return "久坐提醒须在 0 ~ 480 分钟（0 为关闭）";
        if (RetentionDays is < 0 or > 3650) return "数据保留天数须在 0 ~ 3650（0 为永久）";

        List<CategoryRule> rules;
        try
        {
            rules = ParseCategoryRules(CategoryRulesText);
        }
        catch (FormatException ex)
        {
            return "分类规则格式有误：" + ex.Message;
        }

        var excluded = SplitLines(ExcludedAppsText)
            .Distinct(StringComparer.OrdinalIgnoreCase)
            .ToList();

        // 在当前设置的完整副本上修改，避免漏拷字段丢配置
        var updated = Clone(_settings);
        updated.IdleThresholdSeconds = IdleThresholdSeconds;
        updated.ForegroundPollMs = ForegroundPollMs;
        updated.RestReminderMinutes = RestReminderMinutes;
        updated.RetentionDays = RetentionDays;
        updated.ShowFloatingWidget = ShowFloatingWidget;
        updated.CategoryDailyLimitMinutes = ParseCategoryLimits(LimitsText);
        updated.CategoryRules = rules;
        updated.ExcludedAppKeys = excluded;
        updated.Theme = ThemeMode;

        try
        {
            if (StartWithWindows != AutoStartManager.IsEnabled())
            {
                AutoStartManager.SetEnabled(StartWithWindows);
            }
        }
        catch (Exception ex)
        {
            return "设置开机自启失败：" + ex.Message;
        }

        try
        {
            updated.Save();
        }
        catch (Exception ex)
        {
            return "保存设置文件失败：" + ex.Message;
        }

        ThemeManager.Apply(updated.Theme);
        _applySettings(updated);
        return null;
    }

    private static TrackerSettings Clone(TrackerSettings source)
    {
        // 逐项拷贝（含引用类型深拷贝），避免漏字段丢配置
        return new TrackerSettings
        {
            IdleThresholdSeconds = source.IdleThresholdSeconds,
            ForegroundPollMs = source.ForegroundPollMs,
            AlivePollMs = source.AlivePollMs,
            StartWithWindows = source.StartWithWindows,
            ExcludedAppKeys = new List<string>(source.ExcludedAppKeys),
            DataDirectory = source.DataDirectory,
            Theme = source.Theme,
            RestReminderMinutes = source.RestReminderMinutes,
            RetentionDays = source.RetentionDays,
            ShowFloatingWidget = source.ShowFloatingWidget,
            CategoryDailyLimitMinutes = new Dictionary<string, int>(source.CategoryDailyLimitMinutes),
            WidgetX = source.WidgetX,
            WidgetY = source.WidgetY,
            CategoryRules = source.CategoryRules.Select(r => new CategoryRule
            {
                Name = r.Name,
                Keywords = new List<string>(r.Keywords),
            }).ToList(),
            SettingsPath = source.SettingsPath,
        };
    }

    internal static List<CategoryRule> ParseCategoryRules(string text)
    {
        var rules = new List<CategoryRule>();
        foreach (var raw in text.Split(['\r', '\n'], StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries))
        {
            if (raw.Length == 0) continue;
            var eq = raw.IndexOf('=');
            if (eq <= 0)
            {
                throw new FormatException($"「{raw}」缺少 = 号，应为 分类名=关键词1|关键词2");
            }

            var name = raw[..eq].Trim();
            if (name.Length == 0) throw new FormatException("分类名不能为空");

            var keywords = raw[(eq + 1)..]
                .Split('|', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries)
                .ToList();
            if (keywords.Count == 0) throw new FormatException($"分类「{name}」至少需要一个关键词");

            rules.Add(new CategoryRule { Name = name, Keywords = keywords });
        }

        if (rules.Count == 0) throw new FormatException("至少保留一条分类规则");
        return rules;
    }

    internal static Dictionary<string, int> ParseCategoryLimits(string text)
    {
        var result = new Dictionary<string, int>(StringComparer.OrdinalIgnoreCase);
        foreach (var raw in text.Split(['', '
'], StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries))
        {
            if (raw.Length == 0) continue;
            var eq = raw.IndexOf('=');
            if (eq <= 0) throw new FormatException($"「{raw}」缺少 = 号，应为 分类名=分钟");
            var name = raw[..eq].Trim();
            if (!int.TryParse(raw[(eq + 1)..].Trim(), out var minutes) || minutes < 0)
                throw new FormatException($"「{name}」的分钟数无效");
            result[name] = minutes;
        }
        return result;
    }

    private static IEnumerable<string> SplitLines(string text) =>
        text.Split(['\r', '\n'], StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries);
}
