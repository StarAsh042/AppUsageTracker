using System.Globalization;
using Microsoft.Data.Sqlite;
using AppUsageTracker.Core.Models;

namespace AppUsageTracker.Core.Storage;

/// <summary>SQLite 持久化实现。WAL 模式，连接启用内建池化。</summary>
public sealed class SqliteUsageStore : IUsageStore
{
    public const string TimeFormat = "yyyy-MM-dd HH:mm:ss.fff";

    private readonly string _connectionString;
    private readonly object _writeGate = new();

    public SqliteUsageStore(string databasePath)
    {
        var dir = Path.GetDirectoryName(databasePath);
        if (!string.IsNullOrEmpty(dir)) Directory.CreateDirectory(dir);
        _connectionString = new SqliteConnectionStringBuilder { DataSource = databasePath, Mode = SqliteOpenMode.ReadWriteCreate }.ToString();
    }

    public void Initialize()
    {
        using var connection = Open();
        Execute(connection, """
            CREATE TABLE IF NOT EXISTS ForegroundIntervals (
                Id INTEGER PRIMARY KEY AUTOINCREMENT,
                AppKey TEXT NOT NULL,
                ProcessName TEXT NOT NULL,
                DisplayName TEXT NOT NULL,
                ExePath TEXT NOT NULL,
                StartTime TEXT NOT NULL,
                EndTime TEXT NOT NULL,
                DurationSeconds REAL NOT NULL
            );
            CREATE INDEX IF NOT EXISTS IX_Intervals_App_Start ON ForegroundIntervals(AppKey, StartTime);
            CREATE INDEX IF NOT EXISTS IX_Intervals_Start ON ForegroundIntervals(StartTime);

            CREATE TABLE IF NOT EXISTS DailyAppUsage (
                UsageDate TEXT NOT NULL,
                AppKey TEXT NOT NULL,
                ProcessName TEXT NOT NULL,
                DisplayName TEXT NOT NULL,
                ExePath TEXT NOT NULL,
                ActiveSeconds REAL NOT NULL DEFAULT 0,
                AliveSeconds REAL NOT NULL DEFAULT 0,
                LaunchCount INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (UsageDate, AppKey)
            ) WITHOUT ROWID;
            """);
    }

    private SqliteConnection Open()
    {
        var connection = new SqliteConnection(_connectionString);
        connection.Open();
        return connection;
    }

    private static void Execute(SqliteConnection connection, string sql)
    {
        using var command = connection.CreateCommand();
        command.CommandText = sql;
        command.ExecuteNonQuery();
    }

    public void SaveIntervals(IReadOnlyList<ForegroundInterval> intervals)
    {
        if (intervals.Count == 0) return;
        lock (_writeGate)
        {
            using var connection = Open();
            using var transaction = connection.BeginTransaction();
            using var command = connection.CreateCommand();
            command.Transaction = transaction;
            command.CommandText = """
                INSERT INTO ForegroundIntervals (AppKey, ProcessName, DisplayName, ExePath, StartTime, EndTime, DurationSeconds)
                VALUES ($appKey, $processName, $displayName, $exePath, $start, $end, $duration);
                """;
            var pAppKey = AddParameter(command, "$appKey");
            var pProcessName = AddParameter(command, "$processName");
            var pDisplayName = AddParameter(command, "$displayName");
            var pExePath = AddParameter(command, "$exePath");
            var pStart = AddParameter(command, "$start");
            var pEnd = AddParameter(command, "$end");
            var pDuration = AddParameter(command, "$duration");

            foreach (var interval in intervals)
            {
                pAppKey.Value = interval.AppKey;
                pProcessName.Value = interval.ProcessName;
                pDisplayName.Value = interval.DisplayName;
                pExePath.Value = interval.ExePath;
                pStart.Value = interval.StartTime.ToString(TimeFormat, CultureInfo.InvariantCulture);
                pEnd.Value = interval.EndTime.ToString(TimeFormat, CultureInfo.InvariantCulture);
                pDuration.Value = interval.DurationSeconds;
                command.ExecuteNonQuery();
            }

            transaction.Commit();
        }
    }

    public void AddDailyUsage(IReadOnlyList<DailyAppUsage> deltas)
    {
        if (deltas.Count == 0) return;
        lock (_writeGate)
        {
            using var connection = Open();
            using var transaction = connection.BeginTransaction();
            using var command = connection.CreateCommand();
            command.Transaction = transaction;
            command.CommandText = """
                INSERT INTO DailyAppUsage (UsageDate, AppKey, ProcessName, DisplayName, ExePath, ActiveSeconds, AliveSeconds, LaunchCount)
                VALUES ($date, $appKey, $processName, $displayName, $exePath, $active, $alive, $launches)
                ON CONFLICT (UsageDate, AppKey) DO UPDATE SET
                    ActiveSeconds = ActiveSeconds + excluded.ActiveSeconds,
                    AliveSeconds  = AliveSeconds + excluded.AliveSeconds,
                    LaunchCount   = LaunchCount + excluded.LaunchCount,
                    ProcessName   = excluded.ProcessName,
                    DisplayName   = excluded.DisplayName,
                    ExePath       = excluded.ExePath;
                """;
            var pDate = AddParameter(command, "$date");
            var pAppKey = AddParameter(command, "$appKey");
            var pProcessName = AddParameter(command, "$processName");
            var pDisplayName = AddParameter(command, "$displayName");
            var pExePath = AddParameter(command, "$exePath");
            var pActive = AddParameter(command, "$active");
            var pAlive = AddParameter(command, "$alive");
            var pLaunches = AddParameter(command, "$launches");

            foreach (var delta in deltas)
            {
                pDate.Value = delta.UsageDate;
                pAppKey.Value = delta.AppKey;
                pProcessName.Value = delta.ProcessName;
                pDisplayName.Value = delta.DisplayName;
                pExePath.Value = delta.ExePath;
                pActive.Value = delta.ActiveSeconds;
                pAlive.Value = delta.AliveSeconds;
                pLaunches.Value = delta.LaunchCount;
                command.ExecuteNonQuery();
            }

            transaction.Commit();
        }
    }

    public List<DailyAppUsage> QueryDaily(string fromDate, string toDate)
    {
        var results = new List<DailyAppUsage>();
        using var connection = Open();
        using var command = connection.CreateCommand();
        command.CommandText = """
            SELECT UsageDate, AppKey, ProcessName, DisplayName, ExePath, ActiveSeconds, AliveSeconds, LaunchCount
            FROM DailyAppUsage
            WHERE UsageDate >= $from AND UsageDate <= $to
            ORDER BY ActiveSeconds DESC;
            """;
        AddParameter(command, "$from").Value = fromDate;
        AddParameter(command, "$to").Value = toDate;

        using var reader = command.ExecuteReader();
        while (reader.Read())
        {
            results.Add(new DailyAppUsage
            {
                UsageDate = reader.GetString(0),
                AppKey = reader.GetString(1),
                ProcessName = reader.GetString(2),
                DisplayName = reader.GetString(3),
                ExePath = reader.GetString(4),
                ActiveSeconds = reader.GetDouble(5),
                AliveSeconds = reader.GetDouble(6),
                LaunchCount = reader.GetInt32(7),
            });
        }

        return results;
    }

    public List<DailyTotalPoint> QueryDailyTotals(string fromDate, string toDate)
    {
        var results = new List<DailyTotalPoint>();
        using var connection = Open();
        using var command = connection.CreateCommand();
        command.CommandText = """
            SELECT UsageDate, SUM(ActiveSeconds), SUM(AliveSeconds)
            FROM DailyAppUsage
            WHERE UsageDate >= $from AND UsageDate <= $to
            GROUP BY UsageDate
            ORDER BY UsageDate;
            """;
        AddParameter(command, "$from").Value = fromDate;
        AddParameter(command, "$to").Value = toDate;

        using var reader = command.ExecuteReader();
        while (reader.Read())
        {
            results.Add(new DailyTotalPoint(reader.GetString(0), reader.GetDouble(1), reader.GetDouble(2)));
        }

        return results;
    }

    public List<ForegroundInterval> QueryIntervals(string fromDate, string toDate)
    {
        var results = new List<ForegroundInterval>();
        using var connection = Open();
        using var command = connection.CreateCommand();
        command.CommandText = """
            SELECT Id, AppKey, ProcessName, DisplayName, ExePath, StartTime, EndTime, DurationSeconds
            FROM ForegroundIntervals
            WHERE substr(StartTime, 1, 10) >= $from AND substr(StartTime, 1, 10) <= $to
            ORDER BY StartTime;
            """;
        AddParameter(command, "$from").Value = fromDate;
        AddParameter(command, "$to").Value = toDate;

        using var reader = command.ExecuteReader();
        while (reader.Read())
        {
            results.Add(new ForegroundInterval
            {
                Id = reader.GetInt64(0),
                AppKey = reader.GetString(1),
                ProcessName = reader.GetString(2),
                DisplayName = reader.GetString(3),
                ExePath = reader.GetString(4),
                StartTime = DateTime.ParseExact(reader.GetString(5), TimeFormat, CultureInfo.InvariantCulture),
                EndTime = DateTime.ParseExact(reader.GetString(6), TimeFormat, CultureInfo.InvariantCulture),
            });
        }

        return results;
    }

    public (long Intervals, long Daily) CleanupBefore(string toDateInclusive)
    {
        lock (_writeGate)
        {
            using var connection = Open();
            using var transaction = connection.BeginTransaction();

            long intervals;
            using (var command = connection.CreateCommand())
            {
                command.Transaction = transaction;
                command.CommandText = "DELETE FROM ForegroundIntervals WHERE substr(StartTime, 1, 10) <= $date;";
                AddParameter(command, "$date").Value = toDateInclusive;
                intervals = command.ExecuteNonQuery();
            }

            long daily;
            using (var command = connection.CreateCommand())
            {
                command.Transaction = transaction;
                command.CommandText = "DELETE FROM DailyAppUsage WHERE UsageDate <= $date;";
                AddParameter(command, "$date").Value = toDateInclusive;
                daily = command.ExecuteNonQuery();
            }

            transaction.Commit();
            return (intervals, daily);
        }
    }

    private static SqliteParameter AddParameter(SqliteCommand command, string name)
    {
        var parameter = command.CreateParameter();
        parameter.ParameterName = name;
        command.Parameters.Add(parameter);
        return parameter;
    }

    public void Dispose() { /* 连接池自动回收 */ }
}
