use crate::models::*;

/// SQLite 持久化（rusqlite，WAL 模式）。
pub struct SqliteStore {
    conn: rusqlite::Connection,
}

impl SqliteStore {
    pub fn open(path: &str) -> Result<Self, rusqlite::Error> {
        let conn = rusqlite::Connection::open(path)?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             CREATE TABLE IF NOT EXISTS ForegroundIntervals (
                 Id INTEGER PRIMARY KEY AUTOINCREMENT,
                 AppKey TEXT NOT NULL, ProcessName TEXT NOT NULL,
                 DisplayName TEXT NOT NULL, ExePath TEXT NOT NULL,
                 StartTime TEXT NOT NULL, EndTime TEXT NOT NULL, DurationSeconds REAL NOT NULL
             );
             CREATE INDEX IF NOT EXISTS IX_Intervals_App_Start ON ForegroundIntervals(AppKey, StartTime);
             CREATE INDEX IF NOT EXISTS IX_Intervals_Start ON ForegroundIntervals(StartTime);
             CREATE TABLE IF NOT EXISTS DailyAppUsage (
                 UsageDate TEXT NOT NULL, AppKey TEXT NOT NULL,
                 ProcessName TEXT NOT NULL, DisplayName TEXT NOT NULL, ExePath TEXT NOT NULL,
                 ActiveSeconds REAL NOT NULL DEFAULT 0,
                 PRIMARY KEY (UsageDate, AppKey)
             ) WITHOUT ROWID;",
        )?;
        Ok(Self { conn })
    }

    pub fn save_daily(&self, rows: &[DailyUsage]) -> Result<(), rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "INSERT INTO DailyAppUsage (UsageDate, AppKey, ProcessName, DisplayName, ExePath, ActiveSeconds)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT (UsageDate, AppKey) DO UPDATE SET ActiveSeconds = ActiveSeconds + excluded.ActiveSeconds",
        )?;
        for r in rows {
            stmt.execute(rusqlite::params![r.usage_date, r.app_key, r.process_name, r.display_name, r.exe_path, r.active_seconds])?;
        }
        Ok(())
    }

    /// 绝对覆盖式 upsert：以内存聚合值为准（幂等，可重复 flush）。
    pub fn save_daily_replace(&self, rows: &[DailyUsage]) -> Result<(), rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "INSERT INTO DailyAppUsage (UsageDate, AppKey, ProcessName, DisplayName, ExePath, ActiveSeconds)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT (UsageDate, AppKey) DO UPDATE SET
               ProcessName = excluded.ProcessName,
               DisplayName = excluded.DisplayName,
               ExePath = excluded.ExePath,
               ActiveSeconds = excluded.ActiveSeconds",
        )?;
        for r in rows {
            stmt.execute(rusqlite::params![r.usage_date, r.app_key, r.process_name, r.display_name, r.exe_path, r.active_seconds])?;
        }
        Ok(())
    }

    pub fn save_intervals(&self, rows: &[ForegroundInterval]) -> Result<(), rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "INSERT INTO ForegroundIntervals (AppKey, ProcessName, DisplayName, ExePath, StartTime, EndTime, DurationSeconds)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )?;
        for r in rows {
            stmt.execute(rusqlite::params![r.app_key, r.process_name, r.display_name, r.exe_path, r.start_time, r.end_time, r.duration_seconds])?;
        }
        Ok(())
    }

    pub fn query_daily(&self, from: &str, to: &str) -> Result<Vec<DailyUsage>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT UsageDate, AppKey, ProcessName, DisplayName, ExePath, ActiveSeconds
             FROM DailyAppUsage WHERE UsageDate >= ?1 AND UsageDate <= ?2
             ORDER BY ActiveSeconds DESC",
        )?;
        let rows = stmt.query_map(rusqlite::params![from, to], |row| {
            Ok(DailyUsage {
                usage_date: row.get(0)?,
                app_key: row.get(1)?,
                process_name: row.get(2)?,
                display_name: row.get(3)?,
                exe_path: row.get(4)?,
                active_seconds: row.get(5)?,
            })
        })?;
        rows.collect()
    }

    pub fn query_intervals(&self, from: &str, to: &str) -> Result<Vec<ForegroundInterval>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT AppKey, ProcessName, DisplayName, ExePath, StartTime, EndTime, DurationSeconds
             FROM ForegroundIntervals
             WHERE substr(StartTime, 1, 10) >= ?1 AND substr(StartTime, 1, 10) <= ?2
             ORDER BY StartTime",
        )?;
        let rows = stmt.query_map(rusqlite::params![from, to], |row| {
            Ok(ForegroundInterval {
                app_key: row.get(0)?,
                process_name: row.get(1)?,
                display_name: row.get(2)?,
                exe_path: row.get(3)?,
                start_time: row.get(4)?,
                end_time: row.get(5)?,
                duration_seconds: row.get(6)?,
            })
        })?;
        rows.collect()
    }

    pub fn clear_all(&self) -> Result<(), rusqlite::Error> {
        self.conn.execute_batch("DELETE FROM ForegroundIntervals; DELETE FROM DailyAppUsage;")
    }
}
