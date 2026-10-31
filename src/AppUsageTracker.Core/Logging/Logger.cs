using System.IO;
using System.Text;

namespace AppUsageTracker.Core.Logging;

/// <summary>极简滚动文件日志：按天一个文件，写入失败时静默忽略，绝不影响主流程。</summary>
public static class Logger
{
    private static readonly object Gate = new();
    private static string? _logDir;

    /// <summary>初始化日志目录（通常为 数据目录\logs）。未初始化时日志不落盘。</summary>
    public static void Init(string logDirectory)
    {
        lock (Gate)
        {
            _logDir = logDirectory;
            try { Directory.CreateDirectory(logDirectory); } catch { /* 忽略 */ }
        }
    }

    public static void Info(string message) => Write("INFO", message, null);
    public static void Warn(string message) => Write("WARN", message, null);
    public static void Error(string message, Exception? ex = null) => Write("ERROR", message, ex);

    private static void Write(string level, string message, Exception? ex)
    {
        string? dir;
        lock (Gate) { dir = _logDir; }
        if (dir is null) return;

        try
        {
            var path = Path.Combine(dir, $"log-{DateTime.Now:yyyyMMdd}.txt");
            var sb = new StringBuilder();
            sb.Append(DateTime.Now.ToString("yyyy-MM-dd HH:mm:ss.fff")).Append(' ')
              .Append(level).Append(' ').AppendLine(message);
            if (ex is not null) sb.AppendLine(ex.ToString());
            lock (Gate) File.AppendAllText(path, sb.ToString(), Encoding.UTF8);
        }
        catch { /* 日志失败静默 */ }
    }
}
