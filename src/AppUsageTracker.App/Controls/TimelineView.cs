using System.Globalization;
using System.Windows;
using System.Windows.Input;
using System.Windows.Media;
using AppUsageTracker.Core.Timeline;
using Brushes = System.Windows.Media.Brushes;
using FlowDirection = System.Windows.FlowDirection;
using FontFamily = System.Windows.Media.FontFamily;
using Pen = System.Windows.Media.Pen;

namespace AppUsageTracker.App.Controls;

/// <summary>时间线上的一个使用片段（按一天 24 小时归一化）。</summary>
public sealed class TimelineSegment
{
    /// <summary>起点占 24 小时的比例（0~1）。</summary>
    public double StartFraction { get; init; }
    /// <summary>时长占 24 小时的比例。</summary>
    public double WidthFraction { get; init; }
    public Brush Brush { get; init; } = null!;
    public string Tooltip { get; init; } = "";
}

/// <summary>
/// 自绘交互式时间线（借鉴 ActivityWatch 的 timeline 视图）：
/// 底部轨道 + 刻度 + 使用片段色块，滚轮以光标为锚缩放、左键拖拽平移、
/// 悬停显示「应用 起止（时长）」；缩放/平移数学在 Core.Timeline.TimelineScale（可单测）。
/// </summary>
public sealed class TimelineView : FrameworkElement
{
    private TimelineScale _scale = new();

    public static readonly DependencyProperty SegmentsProperty = DependencyProperty.Register(
        nameof(Segments), typeof(IReadOnlyList<TimelineSegment>), typeof(TimelineView),
        new FrameworkPropertyMetadata(null, FrameworkPropertyMetadataOptions.AffectsRender,
            (d, _) => ((TimelineView)d).ResetScale()));

    public IReadOnlyList<TimelineSegment> Segments
    {
        get => (IReadOnlyList<TimelineSegment>)GetValue(SegmentsProperty);
        set => SetValue(SegmentsProperty, value);
    }

    public TimelineView()
    {
        ClipToBounds = true;
        MouseWheel += OnMouseWheel;
        MouseLeftButtonDown += (_, e) =>
        {
            if (!IsLoaded) return;
            CaptureMouse();
            _panLastFraction = FractionFromPixel(e.GetPosition(this).X);
            e.Handled = true;
        };
        MouseLeftButtonUp += (_, _) => ReleaseMouseCapture();
        MouseLeave += (_, _) => { ReleaseMouseCapture(); ToolTip = null; };
        MouseMove += OnMouseMove;
    }

    private double _panLastFraction;

    private void OnMouseWheel(object sender, MouseWheelEventArgs e)
    {
        var factor = e.Delta > 0 ? 1.4 : 1 / 1.4;
        _scale.ZoomAt(FractionFromPixel(e.GetPosition(this).X), factor);
        InvalidateVisual();
        UpdateHoverTooltip(e.GetPosition(this).X);
        e.Handled = true; // 阻止外层 ScrollViewer 抢滚轮
    }

    private void OnMouseMove(object sender, MouseEventArgs e)
    {
        var pos = e.GetPosition(this);
        if (e.LeftButton == MouseButtonState.Pressed && IsMouseCaptured)
        {
            var f = FractionFromPixel(pos.X);
            _scale.Pan(_panLastFraction - f);
            _panLastFraction = f;
            InvalidateVisual();
            return;
        }
        UpdateHoverTooltip(pos.X);
    }

    private void UpdateHoverTooltip(double pixelX)
    {
        var f = ActualWidth <= 0 ? -1 : pixelX / ActualWidth;
        foreach (var segment in Segments ?? Array.Empty<TimelineSegment>())
        {
            if (f >= segment.StartFraction && f <= segment.StartFraction + segment.WidthFraction)
            {
                ToolTip = segment.Tooltip;
                return;
            }
        }
        ToolTip = null;
    }

    private double FractionFromPixel(double pixelX) =>
        ActualWidth <= 0 ? 0 : Math.Clamp(pixelX / ActualWidth, 0.0, 1.0);

    private void ResetScale() => _scale = new TimelineScale();

    protected override void OnRender(DrawingContext dc)
    {
        var width = ActualWidth;
        var height = ActualHeight;
        if (width <= 0 || height <= 0) return;

        var track = (Brush?)TryFindResource(TrackBrushKey) ?? Brushes.LightGray;
        var grid = (Brush?)TryFindResource(BorderBrushKey) ?? Brushes.Gray;
        var text = (Brush?)TryFindResource(TextSecondaryBrushKey) ?? Brushes.Gray;
        var dpi = VisualTreeHelper.GetDpi(this).PixelsPerDip;
        var labelTypeface = new Typeface(new FontFamily("Segoe UI"), FontStyles.Normal, FontWeights.Normal, FontStretches.Normal);

        const double trackHeight = 34;
        var trackY = (height - trackHeight) / 2 - 6;
        dc.DrawRoundedRectangle(track, null, new Rect(0, trackY, width, trackHeight), 6, 6);

        var span = _scale.Span;
        var start = _scale.VisibleStart;

        foreach (var segment in Segments ?? Array.Empty<TimelineSegment>())
        {
            if (segment.WidthFraction <= 0) continue;
            var segStart = segment.StartFraction;
            var segEnd = segStart + segment.WidthFraction;
            if (segEnd <= start || segStart >= start + span) continue; // 可见窗口外

            var sx = (Math.Max(segStart, start) - start) / span * width;
            var ex = (Math.Min(segEnd, start + span) - start) / span * width;
            var rect = new Rect(sx, trackY + 6, Math.Max(1.5, ex - sx), trackHeight - 12);
            dc.DrawRoundedRectangle(segment.Brush, null, rect, 3, 3);
        }

        // 整小时刻度与标签（仅可见窗口内，步长随跨度自适应）
        var firstHour = (int)Math.Ceiling(start * 24);
        var lastHour = (int)Math.Floor((start + span) * 24);
        var step = Math.Max(1, Core.Timeline.TimelineScale.TickStepHours(span));
        for (var hour = firstHour; hour <= lastHour; hour++)
        {
            if (hour % step != 0) continue;
            var fx = (hour / 24.0 - start) / span * width;
            if (hour > 0 && hour < 24)
                dc.DrawLine(new Pen(grid, 1) { DashStyle = DashStyles.Dash }, new Point(fx, 2), new Point(fx, trackY + trackHeight + 2));

            var label = new FormattedText($"{hour:00}:00", CultureInfo.CurrentCulture, FlowDirection.LeftToRight,
                labelTypeface, 10, text, dpi);
            dc.DrawText(label, new Point(Math.Min(Math.Max(fx - label.Width / 2, 0), Math.Max(width - label.Width, 0)), trackY + trackHeight + 4));
        }
    }

    private static readonly string TrackBrushKey = "TrackBrush";
    private static readonly string BorderBrushKey = "BorderBrush";
    private static readonly string TextSecondaryBrushKey = "TextSecondaryBrush";
}
