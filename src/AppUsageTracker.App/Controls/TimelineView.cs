using System.Globalization;
using System.Windows;
using System.Windows.Input;
using System.Windows.Media;
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
/// 自绘 24 小时单日时间线（借鉴 ActivityWatch 的 timeline 视图）：
/// 底部轨道 + 整点刻度 + 使用片段色块，鼠标悬停显示「应用 起止（时长）」。
/// </summary>
public sealed class TimelineView : FrameworkElement
{
    public static readonly DependencyProperty SegmentsProperty = DependencyProperty.Register(
        nameof(Segments), typeof(IReadOnlyList<TimelineSegment>), typeof(TimelineView),
        new FrameworkPropertyMetadata(null, FrameworkPropertyMetadataOptions.AffectsRender));

    public IReadOnlyList<TimelineSegment> Segments
    {
        get => (IReadOnlyList<TimelineSegment>)GetValue(SegmentsProperty);
        set => SetValue(SegmentsProperty, value);
    }

    public TimelineView()
    {
        MouseMove += OnMouseMove;
        MouseLeave += (_, _) => ToolTip = null;
    }

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
        var trackRect = new Rect(0, trackY, width, trackHeight);
        dc.DrawRoundedRectangle(track, null, trackRect, 6, 6);

        var segments = Segments ?? Array.Empty<TimelineSegment>();
        foreach (var segment in segments)
        {
            if (segment.WidthFraction <= 0) continue;
            var rect = new Rect(
                segment.StartFraction * width,
                trackY + 6,
                Math.Max(1.5, segment.WidthFraction * width),
                trackHeight - 12);
            dc.DrawRoundedRectangle(segment.Brush, null, rect, 3, 3);
        }

        // 整点刻度与标签（每 3 小时）
        for (var hour = 0; hour <= 24; hour += 3)
        {
            var x = hour / 24.0 * width;
            if (hour is > 0 and < 24)
            {
                dc.DrawLine(new Pen(grid, 1) { DashStyle = DashStyles.Dash }, new Point(x, 2), new Point(x, trackY + trackHeight + 2));
            }

            var label = new FormattedText($"{hour:00}:00", CultureInfo.CurrentCulture, FlowDirection.LeftToRight,
                labelTypeface, 10, text, dpi);
            dc.DrawText(label, new Point(Math.Min(x, width - label.Width), trackY + trackHeight + 4));
        }
    }

    private void OnMouseMove(object sender, MouseEventArgs e)
    {
        var pos = e.GetPosition(this);
        if (ActualWidth <= 0) return;
        var fraction = pos.X / ActualWidth;

        foreach (var segment in Segments ?? Array.Empty<TimelineSegment>())
        {
            if (fraction >= segment.StartFraction && fraction <= segment.StartFraction + segment.WidthFraction)
            {
                ToolTip = segment.Tooltip;
                return;
            }
        }
        ToolTip = null;
    }

    private static readonly string TrackBrushKey = "TrackBrush";
    private static readonly string BorderBrushKey = "BorderBrush";
    private static readonly string TextSecondaryBrushKey = "TextSecondaryBrush";
}
