using AppUsageTracker.Core.Models;

namespace AppUsageTracker.Core.Monitoring;

/// <summary>
/// 前台窗口轮询器：固定间隔上报「当前前台应用 + 输入空闲秒数」。
/// 自身不做任何状态决策，全部交给 <see cref="UsageAggregator"/>，便于单元测试。
/// </summary>
public sealed class ForegroundMonitor : IDisposable
{
    private readonly int _pollMs;
    private System.Threading.Timer? _timer;
    private int _busy;

    /// <summary>(前台应用（可为 null 表示无前台窗口）, 输入空闲秒数, 时间戳)。</summary>
    public event Action<AppIdentity?, double, DateTime>? Tick;

    /// <summary>最近一次轮询到的前台应用，供暂停/恢复时续接区间使用。</summary>
    public AppIdentity? CurrentForeground { get; private set; }

    public ForegroundMonitor(int pollMs)
    {
        if (pollMs < 50) throw new ArgumentOutOfRangeException(nameof(pollMs));
        _pollMs = pollMs;
    }

    public void Start() => _timer ??= new System.Threading.Timer(_ => Poll(), null, 0, _pollMs);

    /// <summary>立即探测一次前台应用（供「排除当前前台程序」等交互使用）。</summary>
    public static AppIdentity? ProbeForeground()
    {
        try
        {
            var pid = Interop.NativeMethods.GetForegroundProcessId();
            return pid is null ? null : AppIdentityResolver.Resolve(pid.Value);
        }
        catch (Exception ex)
        {
            Logging.Logger.Error("探测前台窗口失败", ex);
            return null;
        }
    }

    private void Poll()
    {
        // 上一次回调尚未结束时跳过本次，防止堆积
        if (Interlocked.Exchange(ref _busy, 1) == 1) return;
        try
        {
            var now = DateTime.Now;
            var pid = Interop.NativeMethods.GetForegroundProcessId();
            var fg = pid is null ? null : AppIdentityResolver.Resolve(pid.Value);
            var idleSeconds = Interop.NativeMethods.GetIdleSeconds();
            CurrentForeground = fg;
            Tick?.Invoke(fg, idleSeconds, now);
        }
        catch (Exception ex)
        {
            Logging.Logger.Error("前台轮询异常", ex);
        }
        finally
        {
            Interlocked.Exchange(ref _busy, 0);
        }
    }

    public void Dispose()
    {
        _timer?.Dispose();
        _timer = null;
    }
}
