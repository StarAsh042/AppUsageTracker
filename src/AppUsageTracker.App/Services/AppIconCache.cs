using System.Drawing;
using System.Drawing.Drawing2D;
using System.IO;
using System.Windows;
using System.Windows.Interop;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using Pen = System.Drawing.Pen;

namespace AppUsageTracker.App.Services;

/// <summary>应用图标提取与缓存，以及程序自身图标（托盘/窗口用）的运行时绘制。</summary>
public static class AppIconCache
{
    private static readonly object Gate = new();
    private static readonly Dictionary<string, BitmapSource?> Cache = new(StringComparer.OrdinalIgnoreCase);

    private static Icon? _appIcon;
    private static BitmapSource? _appIconImage;

    /// <summary>取 exe 关联图标（WPF 可绑定的 BitmapSource），失败返回 null。</summary>
    public static BitmapSource? GetIcon(string exePath)
    {
        if (string.IsNullOrEmpty(exePath) || !File.Exists(exePath)) return null;
        lock (Gate)
        {
            if (Cache.TryGetValue(exePath, out var cached)) return cached;
        }

        BitmapSource? source = null;
        try
        {
            using var icon = Icon.ExtractAssociatedIcon(exePath);
            if (icon is not null)
            {
                source = Imaging.CreateBitmapSourceFromHIcon(
                    icon.Handle, Int32Rect.Empty, BitmapSizeOptions.FromEmptyOptions());
                source.Freeze();
            }
        }
        catch { /* 图标提取失败用占位 */ }

        lock (Gate) Cache[exePath] = source;
        return source;
    }

    /// <summary>
    /// 程序自身图标：深色圆角底 + 极简纯白时钟，运行时绘制。
    /// 与 exe 文件图标（App.ico）同款设计，任务栏/资源管理器视觉统一。
    /// </summary>
    public static Icon GetAppIcon()
    {
        if (_appIcon is not null) return _appIcon;
        using var bitmap = new Bitmap(64, 64);
        using (var g = Graphics.FromImage(bitmap))
        {
            g.SmoothingMode = SmoothingMode.AntiAlias;
            using var face = new SolidBrush(System.Drawing.Color.FromArgb(0x1F, 0x1F, 0x24));
            using var facePath = RoundedRect(2, 2, 60, 60, 14);
            g.FillPath(face, facePath);

            using var white = new Pen(System.Drawing.Color.White, 4.5f)
            {
                StartCap = LineCap.Round,
                EndCap = LineCap.Round,
            };
            g.DrawEllipse(white, 14, 14, 36, 36); // 外圈
            g.DrawLine(white, 32, 32, 32, 20);    // 分针指 12
            g.DrawLine(white, 32, 32, 40, 36);    // 时针指 4 点方向
        }

        _appIcon = Icon.FromHandle(bitmap.GetHicon());
        return _appIcon;
    }

    private static System.Drawing.Drawing2D.GraphicsPath RoundedRect(float x, float y, float w, float h, float r)
    {
        var path = new System.Drawing.Drawing2D.GraphicsPath();
        path.AddArc(x, y, r * 2, r * 2, 180, 90);
        path.AddArc(x + w - r * 2, y, r * 2, r * 2, 270, 90);
        path.AddArc(x + w - r * 2, y + h - r * 2, r * 2, r * 2, 0, 90);
        path.AddArc(x, y + h - r * 2, r * 2, r * 2, 90, 90);
        path.CloseFigure();
        return path;
    }

    /// <summary>窗口标题栏/任务栏用的程序图标。</summary>
    public static ImageSource GetAppIconImage()
    {
        if (_appIconImage is not null) return _appIconImage;
        using var icon = (Icon)GetAppIcon().Clone();
        var source = Imaging.CreateBitmapSourceFromHIcon(
            icon.Handle, Int32Rect.Empty, BitmapSizeOptions.FromEmptyOptions());
        source.Freeze();
        _appIconImage = source;
        return _appIconImage;
    }
}
