using AppUsageTracker.Core.Models;
using AppUsageTracker.Core.Monitoring;
using Xunit;

namespace AppUsageTracker.Core.Tests;

public class UsageAggregatorTests
{
    private static readonly DateTime T0 = new(2026, 10, 31, 10, 0, 0);

    private static AppIdentity App(string key, string name) => new(key, name, name, key);

    private static readonly AppIdentity AppA = App(@"c:\apps\a.exe", "应用A");
    private static readonly AppIdentity AppB = App(@"c:\apps\b.exe", "应用B");
    private static readonly AppIdentity Self = App(@"c:\tools\appusagetracker.exe", "统计器自身");

    [Fact]
    public void 前台切换_按应用分别累计()
    {
        var agg = new UsageAggregator(idleThresholdSeconds: 180);

        agg.OnTick(AppA, idleSeconds: 0, T0);
        agg.OnTick(AppA, 30, T0.AddSeconds(30));
        agg.OnTick(AppB, 0, T0.AddSeconds(90));   // A 结束：90 秒
        agg.OnTick(null, 0, T0.AddSeconds(120));  // B 结束：30 秒

        var daily = agg.DrainPendingDaily();
        Assert.Equal(2, daily.Count);
        Assert.Equal(90, daily.Single(d => d.AppKey == AppA.AppKey).ActiveSeconds);
        Assert.Equal(30, daily.Single(d => d.AppKey == AppB.AppKey).ActiveSeconds);

        var intervals = agg.DrainPendingIntervals();
        Assert.Equal(2, intervals.Count);
        Assert.All(intervals, i => Assert.True(i.DurationSeconds is > 89 and < 91 or > 29 and < 31));
    }

    [Fact]
    public void 亚秒级切换_被毛刺过滤()
    {
        var agg = new UsageAggregator(180);
        agg.OnTick(AppA, 0, T0);
        agg.OnTick(AppB, 0, T0.AddSeconds(5));                        // B 开段
        agg.OnTick(null, 0, T0.AddSeconds(5).AddMilliseconds(200));   // B 只存在 0.2 秒 → 丢弃

        var daily = agg.DrainPendingDaily();
        var a = Assert.Single(daily); // 只有 A
        Assert.Equal(AppA.AppKey, a.AppKey);
        Assert.Equal(5, a.ActiveSeconds, precision: 1);
        Assert.Equal(1, agg.DrainPendingIntervals().Count);
    }

    [Fact]
    public void 空闲超阈值_停在输入停止时刻_恢复后重新开段()
    {
        var agg = new UsageAggregator(180);

        agg.OnTick(AppA, 0, T0);                 // 用户有输入
        agg.OnTick(AppA, 30, T0.AddSeconds(30)); // 仍未超阈值
        agg.OnTick(AppA, 181, T0.AddSeconds(181)); // 超阈值，实际空闲始于 T0 → 本段 0 秒（全被丢弃）
        agg.OnTick(AppA, 200, T0.AddSeconds(240)); // 仍空闲（175 会小于阈值！），不计时
        agg.OnTick(AppA, 0, T0.AddSeconds(300));   // 重新活动，新段开始
        agg.OnTick(AppB, 0, T0.AddSeconds(360));   // A 第二段 60 秒

        var daily = agg.DrainPendingDaily();
        Assert.Equal(60, daily.Single(d => d.AppKey == AppA.AppKey).ActiveSeconds);

        var intervals = agg.DrainPendingIntervals();
        var interval = Assert.Single(intervals);
        Assert.Equal(T0.AddSeconds(300), interval.StartTime);
        Assert.Equal(T0.AddSeconds(360), interval.EndTime);
    }

    [Fact]
    public void 空闲判定_区间止于实际空闲开始时刻()
    {
        var agg = new UsageAggregator(180);
        agg.OnTick(AppA, 0, T0);                  // 输入于 T0
        agg.OnTick(AppA, 0, T0.AddSeconds(20));   // 又一次输入于 T0+20
        agg.OnTick(AppA, 170, T0.AddSeconds(190)); // 未超阈值，继续
        // T0+201 时发现已空闲 181 秒，即用户实际在 T0+20 停止操作 → A 只计 20 秒
        agg.OnTick(AppA, 181, T0.AddSeconds(201));
        agg.OnTick(AppA, 200, T0.AddSeconds(400)); // 空闲中不计时
        agg.CloseOpenSegment(T0.AddSeconds(400));  // 空闲段无区间可闭合

        var daily = agg.DrainPendingDaily();
        Assert.Equal(20, daily.Single(d => d.AppKey == AppA.AppKey).ActiveSeconds, precision: 1);
    }

    [Fact]
    public void 跨午夜_按自然日切分()
    {
        var agg = new UsageAggregator(180);
        var start = new DateTime(2026, 10, 31, 23, 59, 30);

        agg.OnTick(AppA, 0, start);
        agg.OnTick(null, 0, start.AddSeconds(60)); // 30 秒在 10-31，30 秒在 11-01

        var daily = agg.DrainPendingDaily();
        Assert.Equal(2, daily.Count);
        Assert.Equal(30, daily.Single(d => d.UsageDate == "2026-10-31").ActiveSeconds);
        Assert.Equal(30, daily.Single(d => d.UsageDate == "2026-11-01").ActiveSeconds);

        Assert.Equal(2, agg.DrainPendingIntervals().Count);
    }

    [Fact]
    public void 锁屏_停止计时_解锁后恢复()
    {
        var agg = new UsageAggregator(180);
        agg.OnTick(AppA, 0, T0);
        agg.OnLocked(T0.AddSeconds(50));
        Assert.Equal(TrackingState.Locked, agg.State);

        agg.OnTick(AppA, 0, T0.AddSeconds(100)); // 锁定期间忽略
        agg.OnUnlocked(AppA, T0.AddSeconds(100));
        Assert.Equal(TrackingState.Active, agg.State);

        agg.OnTick(AppA, 0, T0.AddSeconds(150));
        agg.CloseOpenSegment(T0.AddSeconds(200));

        var daily = agg.DrainPendingDaily();
        Assert.Equal(150, daily.Single(d => d.AppKey == AppA.AppKey).ActiveSeconds); // 50（锁前）+ 100（解锁后）
    }

    [Fact]
    public void 睡眠恢复_不把睡眠时长计入()
    {
        var agg = new UsageAggregator(180);
        agg.OnTick(AppA, 0, T0);
        agg.OnSuspend(T0.AddSeconds(60));    // A 计 60 秒
        agg.OnResume(AppA, T0.AddHours(8));  // 睡了 8 小时
        agg.OnTick(AppA, 0, T0.AddHours(8).AddSeconds(30)); // 恢复后又用 30 秒
        agg.CloseOpenSegment(T0.AddHours(8).AddSeconds(30));

        var daily = agg.DrainPendingDaily();
        // 60 + 30 = 90；8 小时睡眠不被计入（否则会是 8 小时 + 90 秒）
        Assert.Equal(90, daily.Single(d => d.AppKey == AppA.AppKey).ActiveSeconds, precision: 1);
    }

    [Fact]
    public void 手动暂停恢复_暂停期间不计时()
    {
        var agg = new UsageAggregator(180);
        agg.OnTick(AppA, 0, T0);
        agg.Pause(T0.AddSeconds(40));
        Assert.Equal(TrackingState.Paused, agg.State);
        agg.OnTick(AppB, 0, T0.AddSeconds(100)); // 暂停期间忽略
        agg.Resume(AppA, T0.AddSeconds(150));
        Assert.Equal(TrackingState.Active, agg.State);
        agg.OnTick(AppA, 0, T0.AddSeconds(180));
        agg.CloseOpenSegment(T0.AddSeconds(180));

        var daily = agg.DrainPendingDaily();
        var a = daily.Single(d => d.AppKey == AppA.AppKey);
        Assert.Equal(70, a.ActiveSeconds); // 40 + 30
        Assert.DoesNotContain(daily, d => d.AppKey == AppB.AppKey);
    }

    [Fact]
    public void 排除名单内的应用不统计()
    {
        var agg = new UsageAggregator(180, new[] { Self.AppKey });
        agg.OnTick(Self, 0, T0);
        agg.OnTick(AppA, 0, T0.AddSeconds(60));
        agg.OnTick(null, 0, T0.AddSeconds(120)); // 闭合 A：60 秒

        var daily = agg.DrainPendingDaily();
        Assert.Single(daily);
        Assert.Equal(AppA.AppKey, daily[0].AppKey);
        Assert.Equal(60, daily[0].ActiveSeconds, precision: 1);
    }

    [Fact]
    public void 当前区间实时秒数_状态与排除名单生效()
    {
        var agg = new UsageAggregator(180, new[] { Self.AppKey });
        Assert.Equal(0, agg.CurrentSegmentSeconds(T0.AddSeconds(10))); // 无前台

        agg.OnTick(AppA, 0, T0);
        Assert.Equal(10, agg.CurrentSegmentSeconds(T0.AddSeconds(10)), precision: 1); // 活跃中实时累计

        agg.Pause(T0.AddSeconds(5));
        Assert.Equal(0, agg.CurrentSegmentSeconds(T0.AddSeconds(10))); // 暂停不计时

        agg.Resume(AppA, T0.AddSeconds(100));
        Assert.Equal(5, agg.CurrentSegmentSeconds(T0.AddSeconds(105)), precision: 1);

        agg.OnTick(Self, 0, T0.AddSeconds(110)); // 排除应用获得前台
        Assert.Equal(0, agg.CurrentSegmentSeconds(T0.AddSeconds(120)));
    }

    [Fact]
    public void 落库失败重放_ReAddPending()
    {
        var agg = new UsageAggregator(180);
        agg.OnTick(AppA, 0, T0);
        agg.OnTick(null, 0, T0.AddSeconds(60));

        var intervals = agg.DrainPendingIntervals();
        var daily = agg.DrainPendingDaily();
        Assert.Single(intervals);
        Assert.Single(daily);
        Assert.Empty(agg.DrainPendingIntervals()); // 已取空

        agg.ReAddPending(intervals, daily);
        Assert.Single(agg.DrainPendingIntervals());
        Assert.Single(agg.DrainPendingDaily());
    }

    [Fact]
    public void SplitByDay_边界情况()
    {
        // 同一天内不切
        var day = new DateTime(2026, 10, 31, 8, 0, 0);
        Assert.Single(UsageAggregator.SplitByDay(day, day.AddMinutes(5)));

        // 结束早于开始（时钟回拨）→ 空
        Assert.Empty(UsageAggregator.SplitByDay(day.AddMinutes(5), day));

        // 整点午夜边界
        var start = new DateTime(2026, 10, 31, 0, 0, 0);
        var end = new DateTime(2026, 11, 1, 0, 0, 0);
        Assert.Equal(2, UsageAggregator.SplitByDay(start, end).Count());
    }
}
