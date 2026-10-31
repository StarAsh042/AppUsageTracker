using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Interop;

namespace AppUsageTracker.App.Services;

/// <summary>
/// 窗口外观统一处理：
/// - DWM 圆角（Windows 11，旧系统忽略）
/// - Acrylic 磨砂玻璃背景（SetWindowCompositionAttribute），配合半透明窗口底色使用
/// </summary>
internal static class WindowChrome
{
    [DllImport("dwmapi.dll")]
    private static extern int DwmSetWindowAttribute(IntPtr hwnd, int attribute, ref int value, int size);

    private const int DwmwaUseImmersiveDarkMode = 20;
    private const int DwmwaWindowCornerPreference = 33;
    private const int DwmwaSystemBackdropType = 38;
    private const int DwmwaBorderColor = 34;
    private const int DwmwcpRound = 2;
    private const int DwmsbtTransientwindow = 3; // Acrylic 质感背板（Windows 11 22H2+）
    private const int DwmwaColorNone = unchecked((int)0xFFFFFFFE); // 移除 DWM 自动描边

    /// <summary>
    /// 对窗口应用磨砂质感：Win11 用系统 Acrylic 背板（随主题明暗），Win10 自动退化为纯色底。
    /// 窗口底色画刷为高不透明度中性灰阶，保证不支持背板时也不出现白底。
    /// <paramref name="removeBorder"/> 为 true 时移除 DWM 自动描边（悬浮窗等无边框面板用）。
    /// </summary>
    public static void Apply(Window window, bool darkTheme, bool removeBorder = false)
    {
        try
        {
            var hwnd = new WindowInteropHelper(window).EnsureHandle();

            var round = DwmwcpRound;
            DwmSetWindowAttribute(hwnd, DwmwaWindowCornerPreference, ref round, sizeof(int));

            var dark = darkTheme ? 1 : 0;
            DwmSetWindowAttribute(hwnd, DwmwaUseImmersiveDarkMode, ref dark, sizeof(int));

            var backdrop = DwmsbtTransientwindow;
            DwmSetWindowAttribute(hwnd, DwmwaSystemBackdropType, ref backdrop, sizeof(int));

            if (removeBorder)
            {
                var none = DwmwaColorNone;
                DwmSetWindowAttribute(hwnd, DwmwaBorderColor, ref none, sizeof(int));
            }
        }
        catch
        {
            // 无 DWM 或系统不支持：保持窗口底色兜底
        }
    }
}
