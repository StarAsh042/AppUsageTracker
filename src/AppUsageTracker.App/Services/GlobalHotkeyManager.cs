using System.Runtime.InteropServices;
using AppUsageTracker.Core.Logging;

namespace AppUsageTracker.App.Services;

/// <summary>
/// 全局快捷键（RegisterHotKey）：Ctrl+Alt+U 呼出/隐藏主界面，Ctrl+Alt+P 暂停/恢复监控。
/// 注册冲突时记日志并跳过该键，其余快捷键仍可用；退出时自动注销。
/// </summary>
public sealed class GlobalHotkeyManager : IDisposable
{
    [DllImport("user32.dll")] private static extern bool RegisterHotKey(IntPtr hWnd, int id, uint fsModifiers, uint vk);
    [DllImport("user32.dll")] private static extern bool UnregisterHotKey(IntPtr hWnd, int id);

    private const uint ModControl = 0x0002;
    private const uint ModAlt = 0x0001;
    private const uint VkU = 0x55;
    private const uint VkP = 0x50;

    private const int HotkeyDashboard = 1;
    private const int HotkeyPause = 2;

    private readonly App _app;
    private readonly List<int> _registered = new();
    private IntPtr _hwnd;

    public GlobalHotkeyManager(App app) => _app = app;

    /// <summary>在任何窗口句柄可用后调用（通常用悬浮窗句柄）。</summary>
    public void Register(IntPtr windowHandle)
    {
        _hwnd = windowHandle;
        TryRegister(HotkeyDashboard, ModControl | ModAlt, 0x55, "Ctrl+Alt+U 呼出主界面");
        TryRegister(HotkeyPause, ModControl | ModAlt, 0x50, "Ctrl+Alt+P 暂停/恢复");
    }

    private void TryRegister(int id, uint modifiers, uint vk, string label)
    {
        if (RegisterHotKey(_hwnd, id, modifiers, vk))
        {
            _registered.Add(id);
            Logger.Info($"全局快捷键已注册：{label}");
        }
        else
        {
            Logger.Warn($"全局快捷键注册失败（可能被占用）：{label}");
        }
    }

    public void Dispose()
    {
        foreach (var id in _registered)
        {
            try { UnregisterHotKey(_hwnd, id); }
            catch { }
        }
        _registered.Clear();
    }
}
