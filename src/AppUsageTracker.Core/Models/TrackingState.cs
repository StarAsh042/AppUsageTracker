namespace AppUsageTracker.Core.Models;

/// <summary>监控状态机当前状态。</summary>
public enum TrackingState
{
    /// <summary>用户正在使用电脑，正常计时。</summary>
    Active,

    /// <summary>鼠标键盘无操作超过阈值，暂停前台计时（存活时间照常）。</summary>
    Idle,

    /// <summary>Windows 会话已锁定，暂停前台计时。</summary>
    Locked,

    /// <summary>系统睡眠/休眠中。</summary>
    Suspended,

    /// <summary>用户在托盘菜单中手动暂停了监控。</summary>
    Paused,
}
