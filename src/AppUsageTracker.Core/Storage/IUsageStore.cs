using AppUsageTracker.Core.Models;

namespace AppUsageTracker.Core.Storage;

/// <summary>每日汇总查询的单日总计点。</summary>
public sealed record DailyTotalPoint(string UsageDate, double ActiveSeconds, double AliveSeconds);

/// <summary>使用数据的持久化接口。所有日期参数均为 yyyy-MM-dd 字符串，闭区间。</summary>
public interface IUsageStore : IDisposable
{
    /// <summary>初始化（建库、建表）。可在任意线程调用。</summary>
    void Initialize();

    void SaveIntervals(IReadOnlyList<ForegroundInterval> intervals);

    /// <summary>把每日增量累加进库（与已有记录相加，而非覆盖）。</summary>
    void AddDailyUsage(IReadOnlyList<DailyAppUsage> deltas);

    /// <summary>按日期范围查询每日每应用聚合，按活跃时长降序。</summary>
    List<DailyAppUsage> QueryDaily(string fromDate, string toDate);

    /// <summary>按日期范围查询每日总计（趋势图用）。</summary>
    List<DailyTotalPoint> QueryDailyTotals(string fromDate, string toDate);

    /// <summary>按日期范围（按 StartTime 归日）查询前台明细，按开始时间升序。</summary>
    List<ForegroundInterval> QueryIntervals(string fromDate, string toDate);

    /// <summary>删除截止日（含）之前的所有明细与聚合数据，返回删除行数。</summary>
    (long Intervals, long Daily) CleanupBefore(string toDateInclusive);
}
