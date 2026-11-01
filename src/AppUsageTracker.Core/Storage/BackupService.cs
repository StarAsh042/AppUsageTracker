using System.Globalization;
using System.Text.Json;
using AppUsageTracker.Core.Models;

namespace AppUsageTracker.Core.Storage;

/// <summary>
/// JSON 全量导出/导入（ActivityWatch 式完整备份）。
/// schemaVersion=1：daily（每日聚合）+ intervals（前台明细）。
/// </summary>
public static class BackupService
{
    public record BackupData(int SchemaVersion, string ExportedAt,
        List<DailyAppUsage> Daily, List<ForegroundInterval> Intervals);

    private static readonly JsonSerializerOptions JsonOptions = new() { WriteIndented = true };

    private static string Key(DateTime d) => d.ToString("yyyy-MM-dd", CultureInfo.InvariantCulture);

    public static void Export(IUsageStore store, string path, DateTime from, DateTime to)
    {
        var data = new BackupData(1,
            DateTime.Now.ToString("yyyy-MM-dd HH:mm:ss", CultureInfo.InvariantCulture),
            store.QueryDaily(Key(from), Key(to)),
            store.QueryIntervals(Key(from), Key(to)));
        System.IO.File.WriteAllText(path, JsonSerializer.Serialize(data, JsonOptions));
    }

    /// <summary>mode: merge=跳过已存在的（日期+AppKey）记录；replace=清空后导入。</summary>
    public static (int Daily, int Intervals) Import(IUsageStore store, string path, bool replace)
    {
        var json = System.IO.File.ReadAllText(path);
        BackupData data;
        try
        {
            data = JsonSerializer.Deserialize<BackupData>(json, JsonOptions)
                   ?? throw new InvalidDataException("文件不是有效的备份数据");
        }
        catch (JsonException ex)
        {
            throw new InvalidDataException("文件不是有效的备份数据", ex);
        }
        if (data.SchemaVersion != 1)
            throw new InvalidDataException($"不支持的备份版本：{data.SchemaVersion}");

        if (replace) store.ClearAll();

        if (replace)
        {
            store.SaveIntervals(data.Intervals);
            store.AddDailyUsage(data.Daily);
            return (data.Daily.Count, data.Intervals.Count);
        }

        // 合并：跳过已存在的（日期+AppKey）
        var from = data.Daily.Count > 0 ? data.Daily.Min(d => d.UsageDate) : "9999-99-99";
        var to = data.Daily.Count > 0 ? data.Daily.Max(d => d.UsageDate) : "0000-00-00";
        var existing = store.QueryDaily(from, to)
            .Select(d => d.UsageDate + "|" + d.AppKey).ToHashSet();
        var newDaily = data.Daily.Where(d => !existing.Contains(d.UsageDate + "|" + d.AppKey)).ToList();
        store.SaveIntervals(data.Intervals);
        store.AddDailyUsage(newDaily);
        return (newDaily.Count, data.Intervals.Count);
    }
}
