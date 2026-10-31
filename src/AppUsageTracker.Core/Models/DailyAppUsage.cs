namespace AppUsageTracker.Core.Models;

/// <summary>某个自然日内、某个应用的聚合使用数据。</summary>
public sealed class DailyAppUsage
{
    /// <summary>日期键，格式 yyyy-MM-dd（本地时间）。</summary>
    public required string UsageDate { get; set; }
    public required string AppKey { get; set; }
    public required string ProcessName { get; set; }
    public required string DisplayName { get; set; }
    public required string ExePath { get; set; }

    /// <summary>前台活跃累计秒数。</summary>
    public double ActiveSeconds { get; set; }

    /// <summary>进程存活（至少一个进程在运行）累计墙钟秒数。</summary>
    public double AliveSeconds { get; set; }

    /// <summary>启动次数（从无到有出现的次数）。</summary>
    public int LaunchCount { get; set; }

    public DailyAppUsage Clone() => new()
    {
        UsageDate = UsageDate,
        AppKey = AppKey,
        ProcessName = ProcessName,
        DisplayName = DisplayName,
        ExePath = ExePath,
        ActiveSeconds = ActiveSeconds,
        AliveSeconds = AliveSeconds,
        LaunchCount = LaunchCount,
    };
}
