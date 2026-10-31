using System.ComponentModel;
using System.Runtime.InteropServices;
using System.Text;

namespace AppUsageTracker.Core.Interop;

/// <summary>前台窗口 / 输入空闲检测所需的 Win32 API 封装。</summary>
internal static class NativeMethods
{
    private const uint ProcessQueryLimitedInformation = 0x1000;

    [StructLayout(LayoutKind.Sequential)]
    private struct LASTINPUTINFO
    {
        public uint cbSize;
        public uint dwTime;
    }

    [DllImport("user32.dll")]
    private static extern IntPtr GetForegroundWindow();

    [DllImport("user32.dll")]
    private static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint lpdwProcessId);

    [DllImport("user32.dll")]
    private static extern bool GetLastInputInfo(ref LASTINPUTINFO plii);

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern IntPtr OpenProcess(uint dwDesiredAccess, bool bInheritHandle, uint dwProcessId);

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool CloseHandle(IntPtr hObject);

    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern bool QueryFullProcessImageName(IntPtr hProcess, uint dwFlags, StringBuilder lpExeName, ref uint lpdwSize);

    private delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);

    [DllImport("user32.dll")]
    private static extern bool EnumWindows(EnumWindowsProc lpEnumFunc, IntPtr lParam);

    [DllImport("user32.dll")]
    private static extern bool IsWindowVisible(IntPtr hWnd);

    /// <summary>返回当前桌面上拥有至少一个可见顶层窗口的进程 PID 集合（最小化的窗口也算可见）。</summary>
    public static HashSet<uint> GetVisibleWindowOwnerPids()
    {
        var pids = new HashSet<uint>();
        EnumWindows((hWnd, _) =>
        {
            if (IsWindowVisible(hWnd) && GetWindowThreadProcessId(hWnd, out var pid) != 0 && pid != 0)
            {
                pids.Add(pid);
            }
            return true; // 继续枚举
        }, IntPtr.Zero);
        return pids;
    }

    /// <summary>取当前无鼠标键盘输入的秒数（基于系统最后输入时间，自动处理 tick 回绕）。</summary>
    public static double GetIdleSeconds()
    {
        var info = new LASTINPUTINFO { cbSize = (uint)Marshal.SizeOf<LASTINPUTINFO>() };
        if (!GetLastInputInfo(ref info)) return 0;
        // uint 无符号差值在 GetTickCount 回绕（约 49.7 天）时依然正确
        uint idleMs = unchecked((uint)Environment.TickCount - info.dwTime);
        return idleMs / 1000.0;
    }

    /// <summary>返回当前前台窗口所属进程的 PID；无前台窗口时返回 null。</summary>
    public static uint? GetForegroundProcessId()
    {
        var hwnd = GetForegroundWindow();
        if (hwnd == IntPtr.Zero) return null;
        uint pid = GetWindowThreadProcessId(hwnd, out var processId);
        return pid != 0 && processId != 0 ? processId : null;
    }

    /// <summary>按 PID 读取进程 exe 完整路径。系统受保护进程等读取失败时返回 null。</summary>
    public static string? TryGetProcessImageName(uint pid)
    {
        if (pid <= 4) return null;
        var handle = OpenProcess(ProcessQueryLimitedInformation, false, pid);
        if (handle == IntPtr.Zero) return null;
        try
        {
            const uint capacity = 1024;
            var sb = new StringBuilder((int)capacity);
            uint size = capacity;
            return QueryFullProcessImageName(handle, 0, sb, ref size) ? sb.ToString(0, (int)size) : null;
        }
        catch (Exception ex) when (ex is Win32Exception or InvalidOperationException)
        {
            return null;
        }
        finally
        {
            CloseHandle(handle);
        }
    }
}
