using Microsoft.Win32;

namespace AppUsageTracker.Core.Monitoring;

/// <summary>把 Windows 会话锁定/解锁、睡眠/恢复转换为简单事件。事件在工作线程触发。</summary>
public sealed class SessionEventSource : IDisposable
{
    /// <summary>会话锁定（Win+L 等）。</summary>
    public event Action? Locked;

    /// <summary>会话解锁。</summary>
    public event Action? Unlocked;

    /// <summary>系统即将睡眠/休眠。</summary>
    public event Action? Suspending;

    /// <summary>系统从睡眠/休眠恢复。</summary>
    public event Action? Resumed;

    public SessionEventSource()
    {
        SystemEvents.SessionSwitch += OnSessionSwitch;
        SystemEvents.PowerModeChanged += OnPowerModeChanged;
    }

    private void OnSessionSwitch(object? sender, SessionSwitchEventArgs e)
    {
        switch (e.Reason)
        {
            case SessionSwitchReason.SessionLock:
                Locked?.Invoke();
                break;
            case SessionSwitchReason.SessionUnlock:
                Unlocked?.Invoke();
                break;
        }
    }

    private void OnPowerModeChanged(object? sender, PowerModeChangedEventArgs e)
    {
        switch (e.Mode)
        {
            case PowerModes.Suspend:
                Suspending?.Invoke();
                break;
            case PowerModes.Resume:
                Resumed?.Invoke();
                break;
        }
    }

    public void Dispose()
    {
        SystemEvents.SessionSwitch -= OnSessionSwitch;
        SystemEvents.PowerModeChanged -= OnPowerModeChanged;
    }
}
