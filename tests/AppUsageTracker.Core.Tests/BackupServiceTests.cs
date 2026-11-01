using AppUsageTracker.Core.Models;
using AppUsageTracker.Core.Storage;
using Xunit;

namespace AppUsageTracker.Core.Tests;

public class BackupServiceTests : IDisposable
{
    private readonly SqliteUsageStore _store;
    private readonly string _dbPath;
    private readonly string _jsonPath;

    public BackupServiceTests()
    {
        _dbPath = Path.Combine(Path.GetTempPath(), $"bk-test-{Guid.NewGuid():N}.db");
        _jsonPath = Path.Combine(Path.GetTempPath(), $"bk-test-{Guid.NewGuid():N}.json");
        _store = new SqliteUsageStore(_dbPath);
        _store.Initialize();
    }

    private static DailyAppUsage Bucket(string date, string key, double active) => new()
    {
        UsageDate = date, AppKey = key, ProcessName = "p", DisplayName = key, ExePath = key, ActiveSeconds = active,
    };

    [Fact]
    public void 导出再合并导入_不产生重复()
    {
        _store.AddDailyUsage(new[] { Bucket("2026-11-01", @"c:.exe", 60) });
        BackupService.Export(_store, _jsonPath, "2026-11-01", "2026-11-01");
        var json = File.ReadAllText(_jsonPath);
        Assert.Contains("\"schemaVersion\": 1", json);

        // 模拟"新库"：清空后先放一条不同的记录，再合并导入
        _store.ClearAll();
        _store.AddDailyUsage(new[] { Bucket("2026-11-01", @"c:.exe", 10) });
        var imported = BackupService.Import(_store, _jsonPath, replace: false);

        Assert.Equal(0, imported.Daily); // (date,appKey) 已存在 → 跳过
        var rows = _store.QueryDaily("2026-11-01", "2026-11-01");
        var a = Assert.Single(rows);
        Assert.Contains("a.exe", a.AppKey);
    }

    [Fact]
    public void 替换导入_清空后写入()
    {
        _store.AddDailyUsage(new[] { Bucket("2026-11-01", @"c:.exe", 60) });
        BackupService.Export(_store, _jsonPath, "2026-11-01", "2026-11-01");
        _store.ClearAll();
        _store.AddDailyUsage(new[] { Bucket("2026-11-01", @"c:\junk.exe", 999) });

        var imported = BackupService.Import(_store, _jsonPath, replace: true);
        Assert.Equal(1, imported.Daily);

        var rows = _store.QueryDaily("2026-11-01", "2026-11-01");
        Assert.DoesNotContain(rows, r => r.AppKey.Contains("junk"));
    }

    [Fact]
    public void 损坏文件_抛出不写库()
    {
        File.WriteAllText(_jsonPath, "{ not json !!");
        Assert.ThrowsAny<Exception>(() =>
            BackupService.Import(_store, _jsonPath, replace: true));
        Assert.Empty(_store.QueryDaily("2026-11-01", "2026-11-01"));
    }

    public void Dispose()
    {
        _store.Dispose();
        try { File.Delete(_dbPath); File.Delete(_jsonPath); } catch { }
    }
}
