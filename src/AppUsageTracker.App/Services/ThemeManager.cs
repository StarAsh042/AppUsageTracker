using System.Windows;
using System.Windows.Media;
using Microsoft.Win32;
using Color = System.Windows.Media.Color;
using ColorConverter = System.Windows.Media.ColorConverter;

namespace AppUsageTracker.App.Services;

/// <summary>
/// 主题管理器：把两套调色板写入应用级资源字典。
/// 支持 Auto（跟随 Windows 个人设置的深浅色，含运行时切换监听）。
/// 所有 XAML 中的画刷引用使用 DynamicResource，切换主题即时生效。
/// </summary>
public static class ThemeManager
{
    private const string PersonalizeKey = @"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize";
    private const string AppsUseLightTheme = "AppsUseLightTheme";

    /// <summary>实际生效的主题（解析 Auto 之后的 Dark/Light）。</summary>
    public static string EffectiveTheme { get; private set; } = "Dark";

    /// <summary>设置里的主题选择（Auto/Dark/Light）。</summary>
    public static string PreferredTheme { get; private set; } = "Auto";

    /// <summary>实际主题变化时触发（含系统切换导致 Auto 解析结果变化）。</summary>
    public static event Action<string>? ThemeChanged;

    static ThemeManager()
    {
        // 跟随 Windows 深浅色切换（仅对 Auto 生效）
        SystemEvents.UserPreferenceChanged += (_, e) =>
        {
            if (e.Category is UserPreferenceCategory.General or UserPreferenceCategory.VisualStyle
                && PreferredTheme == "Auto")
            {
                Apply("Auto");
            }
        };
    }

    /// <summary>读取 Windows 当前应用深浅色设置。true = 浅色。</summary>
    public static bool IsWindowsLightMode()
    {
        try
        {
            using var key = Registry.CurrentUser.OpenSubKey(PersonalizeKey);
            return key?.GetValue(AppsUseLightTheme) is int v && v != 0;
        }
        catch
        {
            return false;
        }
    }

    public static void Apply(string preferred)
    {
        PreferredTheme = preferred is "Dark" or "Light" ? preferred : "Auto";
        var effective = PreferredTheme == "Auto"
            ? (IsWindowsLightMode() ? "Light" : "Dark")
            : PreferredTheme;

        var resources = Application.Current?.Resources;
        if (resources is null) return;

        var changed = effective != EffectiveTheme || resources[PrimaryBrushKey] is null;
        EffectiveTheme = effective;

        foreach (var (key, dark, light) in Palette)
        {
            var brush = new SolidColorBrush((Color)ColorConverter.ConvertFromString(effective == "Dark" ? dark : light));
            brush.Freeze();
            resources[key] = brush;
        }

        if (changed) ThemeChanged?.Invoke(effective);
    }

    /// <summary>调色板：键 / 深色值 / 浅色值。窗口底色半透明中性灰阶，让 Acrylic 背板透出磨砂质感。</summary>
    private static readonly (string Key, string Dark, string Light)[] Palette =
    {
        (PageBrushKey,         "#B41C1C1E", "#C2F5F5F5"),
        ("WidgetBrush",        "#8C141416", "#99F6F6F6"),
        ("CardBrush",          "#24FFFFFF", "#B3FFFFFF"),
        ("CardSoftBrush",      "#16FFFFFF", "#D9FFFFFF"),
        ("BorderBrush",        "#2EFFFFFF", "#4D000000"),
        ("TextPrimaryBrush",   "#F2FFFFFF", "#F0000000"),
        ("TextSecondaryBrush", "#B3FFFFFF", "#A6000000"),
        (PrimaryBrushKey,      "#5B8DEF", "#2563EB"),
        ("PrimaryHoverBrush",  "#4478E4", "#1D4ED8"),
        ("PrimarySoftBrush",   "#2E5B8DEF", "#EAF0FE"),
        ("OkBrush",            "#3DDC97", "#16A34A"),
        ("WarnBrush",          "#F5B759", "#D97706"),
        ("StoppedBrush",       "#99FFFFFF", "#9CA3AF"),
        ("InputBrush",         "#1AFFFFFF", "#CFFFFFFF"),
        ("TrackBrush",         "#22FFFFFF", "#26808080"),
        ("RowHoverBrush",      "#1AFFFFFF", "#73FFFFFF"),
        ("RowAltBrush",        "#0FFFFFFF", "#4DFFFFFF"),
        ("DangerBrush",        "#F87171", "#DC2626"),
        ("ScrollThumbBrush",   "#3DFFFFFF", "#40000000"),
        ("ScrollThumbHoverBrush", "#59FFFFFF", "#59000000"),
        ("ScrollThumbDragBrush", "#B3FFFFFF", "#8C000000"),
    };

    /// <summary>分类/时间线用调色板（深浅色下都有足够对比度）。</summary>
    public static readonly string[] SeriesColors =
    {
        "#5B8DEF", "#3DDC97", "#F5B759", "#F472B6",
        "#A78BFA", "#38BDF8", "#FB923C", "#94A3B8",
    };

    /// <summary>按序号取系列色（环形图/时间线/分类用）。</summary>
    public static SolidColorBrush SeriesBrush(int index)
    {
        var brush = new SolidColorBrush((Color)ColorConverter.ConvertFromString(SeriesColors[index % SeriesColors.Length]));
        brush.Freeze();
        return brush;
    }

    private static readonly object SeriesGate = new();
    private static readonly Dictionary<string, SolidColorBrush> SeriesCache = new(StringComparer.Ordinal);
    private static int _seriesCursor;

    /// <summary>
    /// 为名称（分类名或 AppKey）稳定分配一个系列色：首次出现按序取色，
    /// 保证同一名称颜色恒定，且前几个名称颜色互不相同（哈希取色会撞色）。
    /// </summary>
    public static SolidColorBrush StableBrush(string key)
    {
        lock (SeriesGate)
        {
            if (!SeriesCache.TryGetValue(key, out var brush))
            {
                brush = SeriesBrush(_seriesCursor++);
                SeriesCache[key] = brush;
            }
            return brush;
        }
    }

    public static SolidColorBrush FromHex(string hex)
    {
        var brush = new SolidColorBrush((Color)ColorConverter.ConvertFromString(hex));
        brush.Freeze();
        return brush;
    }

    private const string PageBrushKey = "PageBrush";
    private const string PrimaryBrushKey = "PrimaryBrush";
}
