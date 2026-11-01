using System.Globalization;
using System.Windows;
using System.Windows.Input;
using System.Windows.Media;
using Brushes = System.Windows.Media.Brushes;
using FlowDirection = System.Windows.FlowDirection;
using FontFamily = System.Windows.Media.FontFamily;
using Pen = System.Windows.Media.Pen;
using Size = System.Windows.Size;

namespace AppUsageTracker.App.Controls;

/// <summary>环形图的一个扇区。</summary>
public sealed class DonutSegment
{
    public double Value { get; init; }
    public Brush Brush { get; init; } = null!;
    public string Label { get; init; } = "";
}

/// <summary>自绘环形图（分类占比），中心显示汇总文字。</summary>
public sealed class DonutChart : FrameworkElement
{
    public static readonly DependencyProperty SegmentsProperty = DependencyProperty.Register(
        nameof(Segments), typeof(IReadOnlyList<DonutSegment>), typeof(DonutChart),
        new FrameworkPropertyMetadata(null, FrameworkPropertyMetadataOptions.AffectsRender));

    public static readonly DependencyProperty CenterTextProperty = DependencyProperty.Register(
        nameof(CenterText), typeof(string), typeof(DonutChart),
        new FrameworkPropertyMetadata(string.Empty, FrameworkPropertyMetadataOptions.AffectsRender));

    public IReadOnlyList<DonutSegment> Segments
    {
        get => (IReadOnlyList<DonutSegment>)GetValue(SegmentsProperty);
        set => SetValue(SegmentsProperty, value);
    }

    public string CenterText
    {
        get => (string)GetValue(CenterTextProperty);
        set => SetValue(CenterTextProperty, value);
    }

    /// <summary>点击扇区时触发（Label = 分类名）。</summary>
    public event Action<string>? SegmentClicked;

    private void OnClick(object sender, MouseButtonEventArgs e)
    {
        var total = Segments?.Sum(s => s.Value) ?? 0;
        if (total <= 0 || Segments is null) return;
        var center = new Point(ActualWidth / 2, ActualHeight / 2);
        var radius = Math.Min(ActualWidth, ActualHeight) / 2 - 4;
        var pos = e.GetPosition(this);
        var dx = pos.X - center.X; var dy = pos.Y - center.Y;
        var dist = Math.Sqrt(dx * dx + dy * dy);
        var inner = radius * 0.5; // 环内缘约半径一半
        if (dist < inner || dist > radius) return;

        var angle = (Math.Atan2(dy, dx) * 180 / Math.PI + 90 + 360) % 360;
        var acc = 0.0;
        foreach (var segment in Segments)
        {
            if (segment.Value <= 0) continue;
            var sweep = segment.Value / total * 360.0;
            if (angle >= acc && angle < acc + sweep) { SegmentClicked?.Invoke(segment.Label); return; }
            acc += sweep;
        }
    }

    protected override void OnRender(DrawingContext dc)
    {
        var width = ActualWidth;
        var height = ActualHeight;
        if (width <= 0 || height <= 0) return;

        var radius = Math.Min(width, height) / 2 - 4;
        var center = new Point(width / 2, height / 2);
        var thickness = Math.Max(14, radius * 0.32);
        var text = (Brush?)TryFindResource(TextPrimaryBrushKey) ?? Brushes.Gray;

        var total = Segments?.Sum(s => s.Value) ?? 0;
        if (total <= 0)
        {
            var emptyPen = new Pen((Brush?)TryFindResource(TrackBrushKey) ?? Brushes.Gray, thickness);
            dc.DrawEllipse(null, emptyPen, center, radius - thickness / 2, radius - thickness / 2);
            DrawCenterText(dc, center, "暂无数据", text);
            return;
        }

        // 扇区间留 2° 缝隙更精致
        var gap = Segments!.Count > 1 ? 2.0 : 0.0;
        double sweepUsed = 0;
        foreach (var segment in Segments)
        {
            if (segment.Value <= 0) continue;
            var sweep = segment.Value / total * 360.0;
            var startAngle = -90 + sweepUsed + gap / 2;
            var angle = Math.Max(sweep - gap, 0.5);

            var geometry = new StreamGeometry();
            using (var ctx = geometry.Open())
            {
                AddRingArc(ctx, center, radius - thickness / 2, startAngle, angle);
            }
            geometry.Freeze();
            dc.DrawGeometry(null, new Pen(segment.Brush, thickness)
            {
                StartLineCap = PenLineCap.Round,
                EndLineCap = PenLineCap.Round,
            }, geometry);
            sweepUsed += sweep;
        }

        DrawCenterText(dc, center, CenterText, text);
    }

    /// <summary>用圆弧路径表示一段圆环（以 Pen 描边宽度为环厚）。</summary>
    private static void AddRingArc(StreamGeometryContext ctx, Point center, double radius, double startAngle, double sweep)
    {
        var start = Polar(center, radius, startAngle);
        var end = Polar(center, radius, startAngle + sweep);
        var large = sweep > 180;

        ctx.BeginFigure(start, false, false);
        ctx.ArcTo(end, new Size(radius, radius), 0, large, SweepDirection.Clockwise, true, false);
    }

    private static Point Polar(Point center, double radius, double angleDeg)
    {
        var rad = angleDeg * Math.PI / 180;
        return new Point(center.X + radius * Math.Cos(rad), center.Y + radius * Math.Sin(rad));
    }

    private void DrawCenterText(DrawingContext dc, Point center, string content, Brush brush)
    {
        if (string.IsNullOrEmpty(content)) return;
        var dpi = VisualTreeHelper.GetDpi(this).PixelsPerDip;
        var formatted = new FormattedText(content, CultureInfo.CurrentCulture, FlowDirection.LeftToRight,
            new Typeface(new FontFamily("Segoe UI"), FontStyles.Normal, FontWeights.SemiBold, FontStretches.Normal),
            14, brush, dpi);
        dc.DrawText(formatted, new Point(center.X - formatted.Width / 2, center.Y - formatted.Height / 2));
    }

    private static readonly string TextPrimaryBrushKey = "TextPrimaryBrush";
    private static readonly string TrackBrushKey = "TrackBrush";
}
