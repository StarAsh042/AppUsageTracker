using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Interop;

namespace AppUsageTracker.App.Services;

/// <summary>
/// 窗口外观统一处理：
/// - DWM 圆角（Windows 11，旧系统忽略）
/// - 主窗口：SystemBackdrop Acrylic 背板
/// - 悬浮窗：SetWindowCompositionAttribute Acrylic 加速磨砂 + 移除描边
/// </summary>
internal static class WindowChrome
{
    [DllImport("dwmapi.dll")]
    private static extern int DwmSetWindowAttribute(IntPtr hwnd, int attribute, ref int value, int size);

    [DllImport("user32.dll")]
    private static extern int SetWindowCompositionAttribute(IntPtr hwnd, ref WindowCompositionAttributeData data);

    private const int DwmwaUseImmersiveDarkMode = 20;
    private const int DwmwaWindowCornerPreference = 33;
    private const int DwmwaSystemBackdropType = 38;
    private const int DwmwaBorderColor = 34;
    private const int DwmwcpRound = 2;
    private const int DwmsbtTransientwindow = 3; // Acrylic 质感背板（Windows 11 22H2+）
    private const int DwmwaColorNone = unchecked((int)0xFFFFFFFE); // 移除 DWM 自动描边

    private const int WcaAccentPolicy = 19;
    private const int AccentEnableAcrylicBlurbehind = 4;

    [StructLayout(LayoutKind.Sequential)]
    private struct AccentPolicy
    {
        public int AccentState;
        public int AccentFlags;
        public uint GradientColor; // ABGR
        public int AnimationId;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct WindowCompositionAttributeData
    {
        public int Attribute;
        public IntPtr Data;
        public int SizeOfData;
    }

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

    /// <summary>
    /// 悬浮窗专用：Acrylic 加速磨砂（SetWindowCompositionAttribute）+ 圆角 + 无描边。
    /// 窗口背景需为透明/半透明才能透出磨砂；策略不受支持的系统由窗口内容兜底。
    /// </summary>
    public static void ApplyFloating(Window window, bool darkTheme)
    {
        try
        {
            var hwnd = new WindowInteropHelper(window).EnsureHandle();

            var round = DwmwcpRound;
            DwmSetWindowAttribute(hwnd, DwmwaWindowCornerPreference, ref round, sizeof(int));

            var dark = darkTheme ? 1 : 0;
            DwmSetWindowAttribute(hwnd, DwmwaUseImmersiveDarkMode, ref dark, sizeof(int));

            var none = DwmwaColorNone;
            DwmSetWindowAttribute(hwnd, DwmwaBorderColor, ref none, sizeof(int));

            // 中性灰阶色调（GradientColor 为 ABGR 序；R==G==B 时与 RGBA 序等价）
            var tint = darkTheme ? 0x8C18181Au : 0x8CF5F5F5u;
            var accent = new AccentPolicy
            {
                AccentState = AccentEnableAcrylicBlurbehind,
                AccentFlags = 2,
                GradientColor = tint,
            };
            var accentPtr = Marshal.AllocHGlobal(Marshal.SizeOf<AccentPolicy>());
            try
            {
                Marshal.StructureToPtr(accent, accentPtr, false);
                var data = new WindowCompositionAttributeData
                {
                    Attribute = WcaAccentPolicy,
                    Data = accentPtr,
                    SizeOfData = Marshal.SizeOf<AccentPolicy>(),
                };
                SetWindowCompositionAttribute(hwnd, ref data);
            }
            finally
            {
                Marshal.FreeHGlobal(accentPtr);
            }
        }
        catch
        {
            // 策略不受支持：由窗口内容兜底显示
        }
    }
}
