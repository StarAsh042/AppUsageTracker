using System.Globalization;

namespace AppUsageTracker.App.Utils;

/// <summary>时长与日期范围的展示辅助。</summary>
public static class Format
{
    /// <summary>把秒数格式化为中文时长，如「3小时24分」「45分10秒」「12秒」。</summary>
    public static string Duration(double seconds)
    {
        var ts = TimeSpan.FromSeconds(Math.Max(0, Math.Round(seconds)));
        if (ts.TotalHours >= 1) return $"{(int)ts.TotalHours}小时{ts.Minutes:D2}分";
        if (ts.TotalMinutes >= 1) return $"{ts.Minutes}分{ts.Seconds:D2}秒";
        return $"{ts.Seconds}秒";
    }

    public static string DateKey(DateTime date) => date.ToString("yyyy-MM-dd", CultureInfo.InvariantCulture);

    /// <summary>本周一。</summary>
    public static DateTime StartOfThisWeek()
    {
        var today = DateTime.Today;
        int diff = ((int)today.DayOfWeek + 6) % 7; // 周一为一周开始
        return today.AddDays(-diff);
    }

    public static DateTime StartOfThisMonth() => new(DateTime.Today.Year, DateTime.Today.Month, 1);

    /// <summary>CSV 字段转义：包含逗号/引号/换行时加引号。</summary>
    public static string CsvField(string value)
    {
        if (value.Contains('"') || value.Contains(',') || value.Contains('\n') || value.Contains('\r'))
        {
            return "\"" + value.Replace("\"", "\"\"") + "\"";
        }
        return value;
    }
}
