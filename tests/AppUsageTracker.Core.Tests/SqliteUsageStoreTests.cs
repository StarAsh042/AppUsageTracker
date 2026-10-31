using System.Globalization;
using AppUsageTracker.Core.Models;
using AppUsageTracker.Core.Storage;
using Xunit;

namespace AppUsageTracker.Core.Tests;

public class SqliteUsageStoreTests : IDisposable
{
    private readonly SqliteUsageStore _store;
    private readonly string _dbPath;
    private const string Day = "2026-10-31";
    private const string NextDay = "2026-11-01";

    public SqliteUsageStoreTests()
    {
        _dbPath = Path.Combine(Path.GetTempPath(), $"usage-test-{Guid.NewGuid():N}.db");
        _store = new SqliteUsageStore(_dbPath);
        _store.Initialize();
    }

    private static DailyAppUsage Bucket(string date, string key, double active = 0, double alive = 0, int launches = 0) =>
        new()
        {
            UsageDate = date,
            AppKey = key,
            ProcessName = "proc",
            DisplayName = key,
            ExePath = key,
            ActiveSeconds = active,
            AliveSeconds = alive,
            LaunchCount = launches,
        };

    [Fact]
    public void 前台明细_写入并按日期范围读回()
    {
        var start = DateTime.ParseExact($"{Day} 10:00:00.000", "yyyy-MM-dd HH:mm:ss.fff", CultureInfo.InvariantCulture);
        _store.SaveIntervals(new[]
        {
            new ForegroundInterval
            {
                AppKey = @"c:\apps\a.exe", ProcessName = "a", DisplayName = "应用A", ExePath = @"c:\apps\a.exe",
                StartTime = start, EndTime = start.AddMinutes(5),
            },
        });

        var inRange = _store.QueryIntervals(Day, Day);
        var interval = Assert.Single(inRange);
        Assert.Equal(@"c:\apps\a.exe", interval.AppKey);
        Assert.Equal(start, interval.StartTime);
        Assert.Equal(300, interval.DurationSeconds, precision: 1);

        Assert.Empty(_store.QueryIntervals(NextDay, NextDay)); // 范围外查不到
    }

    [Fact]
    public void 每日聚合_重复写入是累加而非覆盖()
    {
        _store.AddDailyUsage(new[] { Bucket(Day, @"c:\apps\a.exe", active: 60) });
        _store.AddDailyUsage(new[] { Bucket(Day, @"c:\apps\a.exe", active: 30, alive: 12, launches: 2) });

        var rows = _store.QueryDaily(Day, Day);
        var a = Assert.Single(rows);
        Assert.Equal(90, a.ActiveSeconds, precision: 1);
        Assert.Equal(12, a.AliveSeconds, precision: 1);
        Assert.Equal(2, a.LaunchCount);
    }

    [Fact]
    public void 查询_按日期范围过滤()
    {
        _store.AddDailyUsage(new[] { Bucket(Day, @"c:\apps\a.exe", active: 10) });
        _store.AddDailyUsage(new[] { Bucket(NextDay, @"c:\apps\b.exe", active: 20) });

        Assert.Single(_store.QueryDaily(Day, Day));
        Assert.Equal(2, _store.QueryDaily(Day, NextDay).Count);
    }

    [Fact]
    public void 每日总计_按日期分组求和()
    {
        _store.AddDailyUsage(new[]
        {
            Bucket(Day, @"c:\apps\a.exe", active: 10, alive: 100),
            Bucket(Day, @"c:\apps\b.exe", active: 20, alive: 50),
            Bucket(NextDay, @"c:\apps\a.exe", active: 5),
        });

        var totals = _store.QueryDailyTotals(Day, NextDay);
        Assert.Equal(2, totals.Count);

        var day1 = totals.Single(t => t.UsageDate == Day);
        Assert.Equal(30, day1.ActiveSeconds, precision: 1);
        Assert.Equal(150, day1.AliveSeconds, precision: 1);

        var day2 = totals.Single(t => t.UsageDate == NextDay);
        Assert.Equal(5, day2.ActiveSeconds, precision: 1);
    }

    [Fact]
    public void 保留清理_只删截止日及之前的数据()
    {
        _store.AddDailyUsage(new[]
        {
            Bucket("2026-10-01", @"c:\apps\old.exe", active: 10),
            Bucket("2026-10-15", @"c:\apps\mid.exe", active: 20),
            Bucket("2026-10-31", @"c:\apps\new.exe", active: 30),
        });
        var start = DateTime.ParseExact("2026-10-01 09:00:00.000", "yyyy-MM-dd HH:mm:ss.fff", CultureInfo.InvariantCulture);
        _store.SaveIntervals(new[]
        {
            new ForegroundInterval { AppKey = @"c:\apps\old.exe", ProcessName = "o", DisplayName = "旧", ExePath = @"c:\apps\old.exe", StartTime = start, EndTime = start.AddMinutes(1) },
        });

        var (intervals, daily) = _store.CleanupBefore("2026-10-15");

        Assert.Equal(1, intervals);
        Assert.Equal(2, daily);
        Assert.Single(_store.QueryDaily("2026-10-01", "2026-10-31")); // 只剩 10-31 一天
        Assert.Empty(_store.QueryIntervals("2026-10-01", "2026-10-31"));
    }

    public void Dispose()
    {
        _store.Dispose();
        try { File.Delete(_dbPath); } catch { /* 忽略 */ }
    }
}
