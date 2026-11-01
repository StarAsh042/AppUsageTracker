using System;
using AppUsageTracker.Core.Timeline;
using Xunit;

namespace AppUsageTracker.Core.Tests;

public class TimelineScaleTests
{
    [Fact]
    public void 初始状态_可见全天()
    {
        var scale = new TimelineScale();
        Assert.Equal(0.0, scale.VisibleStart, precision: 6);
        Assert.Equal(1.0, scale.Span, precision: 6);
    }

    [Fact]
    public void 缩放_锚点时刻保持不动()
    {
        var scale = new TimelineScale();
        // 光标锚在 50% 处，放大 4 倍 → 可见跨度 0.25，锚点前后各占 0.125
        scale.ZoomAt(0.5, 4);
        Assert.Equal(0.25, scale.Span, precision: 6);
        Assert.Equal(0.375, scale.VisibleStart, precision: 6);
        // 锚点时刻 (0.5) 仍在可见窗口内
        Assert.InRange(0.5, scale.VisibleStart, scale.VisibleStart + scale.Span);
    }

    [Fact]
    public void 缩放_不小于最小跨度()
    {
        var scale = new TimelineScale();
        scale.ZoomAt(0.0, 1e9);
        Assert.Equal(TimelineScale.MinSpan.TotalDays, scale.Span, precision: 9);
    }

    [Fact]
    public void 缩放_不超出右边界()
    {
        var scale = new TimelineScale();
        // 锚在最右端放大：可见窗口不得越过 1.0
        scale.ZoomAt(1.0, 4);
        Assert.Equal(0.75, scale.VisibleStart, precision: 6);
        Assert.Equal(0.25, scale.Span, precision: 6);
    }

    [Fact]
    public void 平移_在边界内钳制()
    {
        var scale = new TimelineScale();
        scale.Pan(0.5);
        Assert.Equal(0.0, scale.VisibleStart, precision: 6); // 全跨度时无处可移
        scale.ZoomAt(0.25, 2); // span=0.5, start=0.125
        scale.Pan(10.0);
        Assert.Equal(0.5, scale.VisibleStart, precision: 6);
        scale.Pan(-10.0);
        Assert.Equal(0.0, scale.VisibleStart, precision: 6);
    }

    [Fact]
    public void 无效缩放系数_被忽略()
    {
        var scale = new TimelineScale();
        scale.ZoomAt(0.5, 0);
        scale.ZoomAt(0.5, -1);
        Assert.Equal(1.0, scale.Span, precision: 6);
    }

    [Fact]
    public void 刻度步长_随跨度变化()
    {
        Assert.Equal(4, TimelineScale.TickStepHours(1.0));      // 24h
        Assert.Equal(2, TimelineScale.TickStepHours(0.5));      // 12h
        Assert.Equal(1, TimelineScale.TickStepHours(0.1));      // 2.4h
        Assert.Equal(1, TimelineScale.TickStepHours(0.01));     // 14.4min
    }

    [Fact]
    public void 片段占比_钳制在全天内()
    {
        var day = new DateTime(2026, 11, 1);
        Assert.Equal(0.0, TimelineScale.FractionOfDay(day.AddHours(-2), day), precision: 6);
        Assert.Equal(1.0, TimelineScale.FractionOfDay(day.AddHours(30), day), precision: 6);
        Assert.Equal(0.5, TimelineScale.FractionOfDay(day.AddHours(12), day), precision: 6);
    }
}
