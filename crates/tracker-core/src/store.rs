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
             ) WITHOUT ROWID;
             CREATE TABLE IF NOT EXISTS StateIntervals (
                 Id INTEGER PRIMARY KEY AUTOINCREMENT,
                 State TEXT NOT NULL,
                 StartTime TEXT NOT NULL, EndTime TEXT NOT NULL, DurationSeconds REAL NOT NULL
             );
             CREATE INDEX IF NOT EXISTS IX_States_Start ON StateIntervals(StartTime);
             CREATE TABLE IF NOT EXISTS TitleIntervals (
                 Id INTEGER PRIMARY KEY AUTOINCREMENT,
                 AppKey TEXT NOT NULL, Title TEXT NOT NULL,
                 StartTime TEXT NOT NULL, EndTime TEXT NOT NULL, DurationSeconds REAL NOT NULL
             );
             CREATE INDEX IF NOT EXISTS IX_Titles_Start ON TitleIntervals(StartTime);
             CREATE INDEX IF NOT EXISTS IX_Titles_App ON TitleIntervals(AppKey, Title);
             CREATE TABLE IF NOT EXISTS WebIntervals (
                 Id INTEGER PRIMARY KEY AUTOINCREMENT,
                 Domain TEXT NOT NULL, Title TEXT NOT NULL,
                 StartTime TEXT NOT NULL, EndTime TEXT NOT NULL, DurationSeconds REAL NOT NULL
             );
             CREATE INDEX IF NOT EXISTS IX_Web_Start ON WebIntervals(StartTime);
             CREATE INDEX IF NOT EXISTS IX_Web_Domain ON WebIntervals(Domain, Title);",
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
            stmt.execute(rusqlite::params![
                r.usage_date,
                r.app_key,
                r.process_name,
                r.display_name,
                r.exe_path,
                r.active_seconds
            ])?;
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
            stmt.execute(rusqlite::params![
                r.usage_date,
                r.app_key,
                r.process_name,
                r.display_name,
                r.exe_path,
                r.active_seconds
            ])?;
        }
        Ok(())
    }

    pub fn save_intervals(&self, rows: &[ForegroundInterval]) -> Result<(), rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "INSERT INTO ForegroundIntervals (AppKey, ProcessName, DisplayName, ExePath, StartTime, EndTime, DurationSeconds)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )?;
        for r in rows {
            stmt.execute(rusqlite::params![
                r.app_key,
                r.process_name,
                r.display_name,
                r.exe_path,
                r.start_time,
                r.end_time,
                r.duration_seconds
            ])?;
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

    pub fn query_intervals(
        &self,
        from: &str,
        to: &str,
    ) -> Result<Vec<ForegroundInterval>, rusqlite::Error> {
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

    pub fn save_state_intervals(&self, rows: &[StateInterval]) -> Result<(), rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "INSERT INTO StateIntervals (State, StartTime, EndTime, DurationSeconds)
             VALUES (?1, ?2, ?3, ?4)",
        )?;
        for r in rows {
            stmt.execute(rusqlite::params![
                r.state,
                r.start_time,
                r.end_time,
                r.duration_seconds
            ])?;
        }
        Ok(())
    }

    pub fn query_state_intervals(
        &self,
        from: &str,
        to: &str,
    ) -> Result<Vec<StateInterval>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT State, StartTime, EndTime, DurationSeconds
             FROM StateIntervals
             WHERE substr(StartTime, 1, 10) >= ?1 AND substr(StartTime, 1, 10) <= ?2
             ORDER BY StartTime",
        )?;
        let rows = stmt.query_map(rusqlite::params![from, to], |row| {
            Ok(StateInterval {
                state: row.get(0)?,
                start_time: row.get(1)?,
                end_time: row.get(2)?,
                duration_seconds: row.get(3)?,
            })
        })?;
        rows.collect()
    }

    pub fn save_title_intervals(&self, rows: &[TitleInterval]) -> Result<(), rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "INSERT INTO TitleIntervals (AppKey, Title, StartTime, EndTime, DurationSeconds)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;
        for r in rows {
            stmt.execute(rusqlite::params![
                r.app_key,
                r.title,
                r.start_time,
                r.end_time,
                r.duration_seconds
            ])?;
        }
        Ok(())
    }

    /// Top 标题：按 (应用, 标题) 聚合时长，最近出现时间排序辅助。
    pub fn top_titles(
        &self,
        from: &str,
        to: &str,
        limit: i64,
    ) -> Result<Vec<(String, String, f64)>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT AppKey, Title, SUM(DurationSeconds)
             FROM TitleIntervals
             WHERE substr(StartTime, 1, 10) >= ?1 AND substr(StartTime, 1, 10) <= ?2
             GROUP BY AppKey, Title
             ORDER BY SUM(DurationSeconds) DESC
             LIMIT ?3",
        )?;
        let rows = stmt.query_map(rusqlite::params![from, to, limit], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, f64>(2)?,
            ))
        })?;
        rows.collect()
    }

    pub fn save_web_intervals(&self, rows: &[WebInterval]) -> Result<(), rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "INSERT INTO WebIntervals (Domain, Title, StartTime, EndTime, DurationSeconds)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;
        for r in rows {
            stmt.execute(rusqlite::params![
                r.domain,
                r.title,
                r.start_time,
                r.end_time,
                r.duration_seconds
            ])?;
        }
        Ok(())
    }

    /// Top 网页：按 (域名, 标题) 聚合时长。
    pub fn top_web(
        &self,
        from: &str,
        to: &str,
        limit: i64,
    ) -> Result<Vec<(String, String, f64)>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT Domain, Title, SUM(DurationSeconds)
             FROM WebIntervals
             WHERE substr(StartTime, 1, 10) >= ?1 AND substr(StartTime, 1, 10) <= ?2
             GROUP BY Domain, Title
             ORDER BY SUM(DurationSeconds) DESC
             LIMIT ?3",
        )?;
        let rows = stmt.query_map(rusqlite::params![from, to, limit], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, f64>(2)?,
            ))
        })?;
        rows.collect()
    }

    /// 执行单值 PRAGMA 查询（integrity_check / page_count 等）。
    pub fn pragma(&self, pragma: &str) -> Result<String, rusqlite::Error> {
        self.conn
            .query_row(pragma, [], |row| row.get::<_, String>(0))
    }

    /// 执行单值数值 PRAGMA。
    pub fn pragma_i64(&self, pragma: &str) -> Result<i64, rusqlite::Error> {
        self.conn.query_row(pragma, [], |row| row.get::<_, i64>(0))
    }

    /// VACUUM 整理数据库文件。
    pub fn vacuum(&self) -> Result<(), rusqlite::Error> {
        self.conn.execute_batch("VACUUM;")
    }

    /// 删除某应用的全部记录（用于剔除统计程序自身的历史行）。
    pub fn delete_app_rows(&self, app_key: &str) -> Result<(), rusqlite::Error> {
        self.conn.execute_batch(&format!(
            "DELETE FROM ForegroundIntervals WHERE AppKey = '{app_key}';
             DELETE FROM TitleIntervals WHERE AppKey = '{app_key}';
             DELETE FROM DailyAppUsage WHERE AppKey = '{app_key}';"
        ))
    }

    /// 保留策略：删除指定日期之前的全部明细（含前台区间与状态区间）与日聚合。
    pub fn purge_before(&self, date: &str) -> Result<(), rusqlite::Error> {
        self.conn.execute_batch(&format!(
            "DELETE FROM ForegroundIntervals WHERE substr(StartTime, 1, 10) < '{date}';
             DELETE FROM StateIntervals WHERE substr(StartTime, 1, 10) < '{date}';
             DELETE FROM TitleIntervals WHERE substr(StartTime, 1, 10) < '{date}';
             DELETE FROM WebIntervals WHERE substr(StartTime, 1, 10) < '{date}';
             DELETE FROM DailyAppUsage WHERE UsageDate < '{date}';"
        ))
    }

    pub fn clear_all(&self) -> Result<(), rusqlite::Error> {
        self.conn.execute_batch(
            "DELETE FROM ForegroundIntervals; DELETE FROM StateIntervals;
             DELETE FROM TitleIntervals; DELETE FROM WebIntervals; DELETE FROM DailyAppUsage;",
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_db() -> SqliteStore {
        let path = std::env::temp_dir().join(format!(
            "tracker_test_{}_{}.db",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_file(&path);
        SqliteStore::open(path.to_string_lossy().as_ref()).unwrap()
    }

    #[test]
    fn 存取与日期过滤() {
        let store = temp_db();
        let daily = vec![DailyUsage {
            usage_date: "2026-11-04".into(),
            app_key: "a".into(),
            process_name: "a".into(),
            display_name: "A".into(),
            exe_path: "a.exe".into(),
            active_seconds: 60.0,
        }];
        store.save_daily_replace(&daily).unwrap();
        let rows = store.query_daily("2026-11-04", "2026-11-04").unwrap();
        assert_eq!(rows.len(), 1);
        assert!(store
            .query_daily("2026-11-05", "2026-11-05")
            .unwrap()
            .is_empty());
    }

    #[test]
    fn 绝对覆盖幂等() {
        let store = temp_db();
        let row = DailyUsage {
            usage_date: "2026-11-04".into(),
            app_key: "a".into(),
            process_name: "a".into(),
            display_name: "A".into(),
            exe_path: "a.exe".into(),
            active_seconds: 30.0,
        };
        store
            .save_daily_replace(std::slice::from_ref(&row))
            .unwrap();
        let mut grown = row.clone();
        grown.active_seconds = 90.0;
        store.save_daily_replace(&[grown]).unwrap();
        let rows = store.query_daily("2026-11-04", "2026-11-04").unwrap();
        assert_eq!(rows[0].active_seconds, 90.0, "重复 flush 不应累加");
    }

    #[test]
    fn 状态区间存取与保留清理() {
        let store = temp_db();
        let states = vec![
            StateInterval {
                state: "Idle".into(),
                start_time: "2026-11-03 10:00:00.000".into(),
                end_time: "2026-11-03 10:05:00.000".into(),
                duration_seconds: 300.0,
            },
            StateInterval {
                state: "Locked".into(),
                start_time: "2026-11-04 09:00:00.000".into(),
                end_time: "2026-11-04 09:10:00.000".into(),
                duration_seconds: 600.0,
            },
        ];
        store.save_state_intervals(&states).unwrap();
        assert_eq!(
            store
                .query_state_intervals("2026-11-04", "2026-11-04")
                .unwrap()
                .len(),
            1
        );

        store.purge_before("2026-11-04").unwrap();
        assert!(store
            .query_state_intervals("2026-11-03", "2026-11-03")
            .unwrap()
            .is_empty());
        assert_eq!(
            store
                .query_state_intervals("2026-11-04", "2026-11-04")
                .unwrap()
                .len(),
            1
        );
    }
}
