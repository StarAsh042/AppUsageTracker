using System.Globalization;
using System.IO;
using System.Text;
using AppUsageTracker.Core.Monitoring;
using AppUsageTracker.Core.Storage;

namespace AppUsageTracker.App.Utils;

/// <summary>把指定日期范围的聚合数据导出为 CSV（UTF-8 BOM，Excel 直接打开不乱码）。</summary>
public static class CsvExporter
{
    public static void ExportRange(IUsageStore store, CategoryResolver resolver, string fromDate, string toDate, string filePath)
    {
        var rows = store.QueryDaily(fromDate, toDate)
            .OrderBy(r => r.UsageDate, StringComparer.Ordinal)
            .ThenByDescending(r => r.ActiveSeconds)
            .ToList();

        var sb = new StringBuilder();
        sb.AppendLine("日期,分类,应用,进程名,活跃时长(秒)");
        foreach (var row in rows)
        {
            var category = resolver.Resolve(row.DisplayName, row.ProcessName, row.ExePath);
            sb.Append(row.UsageDate).Append(',')
              .Append(Format.CsvField(category)).Append(',')
              .Append(Format.CsvField(row.DisplayName)).Append(',')
              .Append(Format.CsvField(row.ProcessName)).Append(',')
              .Append(row.ActiveSeconds.ToString("0.#", CultureInfo.InvariantCulture))
              .AppendLine();
        }

        File.WriteAllText(filePath, sb.ToString(), new UTF8Encoding(encoderShouldEmitUTF8Identifier: true));
    }
}
