using System;

namespace AppUsageTracker.Core.Timeline;

/// <summary>
/// 交互式时间线的缩放/平移数学（纯逻辑、可单测）。
/// 可见窗口用「占全天比例」表示：VisibleStart ∈ [0, 1-Span]，Span ∈ [MinSpan, 1]。
/// </summary>
public sealed class TimelineScale
{
    /// <summary>最小可见跨度：15 分钟（占一天的比例）。</summary>
    public static readonly TimeSpan MinSpan = TimeSpan.FromMinutes(15);

    public double VisibleStart { get; private set; }
    public double Span { get; private set; } = 1.0;

    public TimelineScale() { }

    public TimelineScale(double visibleStart, double span)
    {
        VisibleStart = Math.Clamp(visibleStart, 0.0, 1.0);
        Span = Math.Clamp(span, MinSpan.TotalDays, 1.0);
        VisibleStart = Math.Clamp(VisibleStart, 0.0, 1.0 - Span);
    }

    /// <summary>以 anchorFraction（光标位置占比）为锚缩放；factor &gt; 1 放大。</summary>
    public void ZoomAt(double anchorFraction, double factor)
    {
        if (factor <= 0) return;
        var newSpan = Math.Clamp(Span / factor, MinSpan.TotalDays, 1.0);
        var anchor = Math.Clamp(anchorFraction, 0.0, 1.0);
        var anchorTime = VisibleStart + anchor * Span;
        Span = newSpan;
        VisibleStart = Math.Clamp(anchorTime - anchor * Span, 0.0, 1.0 - Span);
    }

    /// <summary>平移；deltaFraction 为可见跨度比例的偏移，自动钳制在边界内。</summary>
    public void Pan(double deltaFraction)
    {
        VisibleStart = Math.Clamp(VisibleStart + deltaFraction, 0.0, 1.0 - Span);
    }

    /// <summary>根据可见跨度选择刻度步长（小时）。</summary>
    public static int TickStepHours(double visibleSpanFraction)
    {
        var hours = visibleSpanFraction * 24;
        return hours switch
        {
            >= 20 => 4,
            >= 8 => 2,
            _ => 1,
        };
    }

    /// <summary>把整天时间转为占比（用于片段定位）。</summary>
    public static double FractionOfDay(DateTime t, DateTime day) =>
        Math.Clamp((t - day).TotalDays, 0.0, 1.0);
}
