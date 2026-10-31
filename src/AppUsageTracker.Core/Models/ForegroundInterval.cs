namespace AppUsageTracker.Core.Models;

/// <summary>一段连续的前台使用明细（已按自然日切分）。</summary>
public sealed class ForegroundInterval
{
    public long Id { get; set; }

    public required string AppKey { get; init; }
    public required string ProcessName { get; init; }
    public required string DisplayName { get; init; }
    public required string ExePath { get; init; }

    public required DateTime StartTime { get; init; }
    public required DateTime EndTime { get; init; }

    public double DurationSeconds => (EndTime - StartTime).TotalSeconds;
}
