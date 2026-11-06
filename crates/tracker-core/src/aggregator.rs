use crate::models::*;
use std::collections::{HashMap, HashSet};

/// 统计状态机与内存聚合器（纯逻辑、线程安全由调用方保证）。
pub struct UsageAggregator {
    pub state: TrackingState,
    pub idle_threshold_seconds: f64,
    excluded: HashSet<String>,
    current: Option<AppIdentity>,
    segment_start: chrono::DateTime<chrono::Local>,
    state_changed_at: chrono::DateTime<chrono::Local>,
    /// 当前状态区间的起点（用于落 StateIntervals）。
    state_started_at: chrono::DateTime<chrono::Local>,
    /// 今日逐应用活跃秒数（含当前未闭合区间之外的全部已闭合区间）。
    pub daily: HashMap<String, DailyUsage>,
    /// 待写入的前台明细。
    pub pending_intervals: Vec<ForegroundInterval>,
    /// 待写入的状态明细。
    pub pending_state_intervals: Vec<StateInterval>,
    /// 待写入的标题明细。
    pub pending_title_intervals: Vec<TitleInterval>,
    /// 当前标题段（app_key, title）。
    title_current: Option<(String, String)>,
    title_start: chrono::DateTime<chrono::Local>,
}

fn day_key(dt: chrono::DateTime<chrono::Local>) -> String {
    dt.format("%Y-%m-%d").to_string()
}

fn day_start(dt: chrono::DateTime<chrono::Local>) -> chrono::DateTime<chrono::Local> {
    dt.date_naive()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_local_timezone(chrono::Local)
        .unwrap()
}

impl UsageAggregator {
    pub fn new(idle_threshold_seconds: f64, excluded: HashSet<String>) -> Self {
        let now = chrono::Local::now();
        Self {
            state: TrackingState::Active,
            idle_threshold_seconds,
            excluded,
            current: None,
            segment_start: now,
            state_changed_at: now,
            state_started_at: now,
            daily: HashMap::new(),
            pending_intervals: Vec::new(),
            pending_state_intervals: Vec::new(),
            pending_title_intervals: Vec::new(),
            title_current: None,
            title_start: now,
        }
    }

    /// 标题轮询：同一应用内标题变化时切换标题段。
    pub fn on_title_tick(
        &mut self,
        foreground: Option<(&str, &str)>,
        now: chrono::DateTime<chrono::Local>,
    ) {
        let key = match foreground {
            Some((k, t)) => (k.to_string(), t.to_string()),
            None => {
                self.close_title_interval(now);
                return;
            }
        };
        let same = match &self.title_current {
            Some((k, t)) => k == &key.0 && t == &key.1,
            None => false,
        };
        if !same {
            self.close_title_interval(now);
            self.title_current = Some(key);
            self.title_start = now;
        }
    }

    fn close_title_interval(&mut self, now: chrono::DateTime<chrono::Local>) {
        if let Some((app_key, title)) = self.title_current.take() {
            let dur = (now - self.title_start).num_milliseconds() as f64 / 1000.0;
            if dur >= 1.0 {
                self.pending_title_intervals.push(TitleInterval {
                    app_key,
                    title,
                    start_time: self.title_start.format("%Y-%m-%d %H:%M:%S%.3f").to_string(),
                    end_time: now.format("%Y-%m-%d %H:%M:%S%.3f").to_string(),
                    duration_seconds: dur,
                });
            }
        }
    }

    /// 状态切换：闭合上一状态区间（≥1 秒才记录）。
    fn set_state(&mut self, new_state: TrackingState, now: chrono::DateTime<chrono::Local>) {
        if self.state == new_state {
            return;
        }
        let dur = (now - self.state_started_at).num_milliseconds() as f64 / 1000.0;
        if dur >= 1.0 {
            self.pending_state_intervals.push(StateInterval {
                state: format!("{:?}", self.state),
                start_time: self
                    .state_started_at
                    .format("%Y-%m-%d %H:%M:%S%.3f")
                    .to_string(),
                end_time: now.format("%Y-%m-%d %H:%M:%S%.3f").to_string(),
                duration_seconds: dur,
            });
        }
        self.state = new_state;
        self.state_changed_at = now;
        self.state_started_at = now;
    }

    /// 前台轮询 tick：处理状态切换与区间闭合。
    pub fn on_tick(
        &mut self,
        foreground: Option<AppIdentity>,
        idle_seconds: f64,
        now: chrono::DateTime<chrono::Local>,
    ) {
        match self.state {
            TrackingState::Paused | TrackingState::Locked | TrackingState::Suspended => {
                if self.state == TrackingState::Suspended
                    && now - self.state_changed_at > chrono::Duration::minutes(10)
                {
                    self.enter_active(foreground, now);
                }
                return;
            }
            _ => {}
        }

        let is_idle = idle_seconds >= self.idle_threshold_seconds;

        if self.state == TrackingState::Idle {
            if is_idle {
                return;
            }
            self.enter_active(foreground, now);
            return;
        }

        if is_idle {
            let idle_start = now - chrono::Duration::milliseconds((idle_seconds * 1000.0) as i64);
            let end = if idle_start < self.segment_start {
                self.segment_start
            } else {
                idle_start
            };
            self.close_segment(end);
            self.set_state(TrackingState::Idle, now);
            self.current = None;
            return;
        }

        if !same_app(&foreground, &self.current) {
            self.close_segment(now);
            self.current = foreground;
            self.segment_start = now;
        }
    }

    pub fn on_locked(&mut self, now: chrono::DateTime<chrono::Local>) {
        if self.state == TrackingState::Active || self.state == TrackingState::Idle {
            self.close_segment(now);
            self.set_state(TrackingState::Locked, now);
            self.current = None;
        }
    }

    pub fn on_unlocked(
        &mut self,
        foreground: Option<AppIdentity>,
        now: chrono::DateTime<chrono::Local>,
    ) {
        if self.state == TrackingState::Locked {
            self.enter_active(foreground, now);
        }
    }

    pub fn on_suspend(&mut self, now: chrono::DateTime<chrono::Local>) {
        self.close_segment(now);
        self.set_state(TrackingState::Suspended, now);
        self.current = None;
    }

    pub fn on_resume(
        &mut self,
        foreground: Option<AppIdentity>,
        now: chrono::DateTime<chrono::Local>,
    ) {
        if self.state == TrackingState::Suspended {
            self.enter_active(foreground, now);
        }
    }

    pub fn pause(&mut self, now: chrono::DateTime<chrono::Local>) {
        if self.state != TrackingState::Paused {
            self.close_segment(now);
            self.set_state(TrackingState::Paused, now);
            self.current = None;
        }
    }

    pub fn resume(
        &mut self,
        foreground: Option<AppIdentity>,
        now: chrono::DateTime<chrono::Local>,
    ) {
        if self.state == TrackingState::Paused {
            self.enter_active(foreground, now);
        }
    }

    pub fn current_segment_seconds(&self, now: chrono::DateTime<chrono::Local>) -> f64 {
        if self.state != TrackingState::Active || self.current.is_none() {
            return 0.0;
        }
        if self
            .current
            .as_ref()
            .is_some_and(|a| self.excluded.contains(&a.app_key))
        {
            return 0.0;
        }
        let d = (now - self.segment_start).num_milliseconds() as f64 / 1000.0;
        if d > 0.0 {
            d
        } else {
            0.0
        }
    }

    pub fn current_app(&self) -> Option<&AppIdentity> {
        self.current.as_ref()
    }

    /// 热更新设置（空闲阈值 / 排除名单），保留当前区间与今日数据。
    pub fn apply_settings(&mut self, idle_threshold_seconds: f64, excluded: HashSet<String>) {
        self.idle_threshold_seconds = idle_threshold_seconds;
        self.excluded = excluded;
    }

    /// 闭合未完区间（程序退出前调用）。
    pub fn close_open_segment(&mut self, now: chrono::DateTime<chrono::Local>) {
        self.close_segment(now);
    }

    fn enter_active(
        &mut self,
        foreground: Option<AppIdentity>,
        now: chrono::DateTime<chrono::Local>,
    ) {
        self.set_state(TrackingState::Active, now);
        self.current = foreground;
        self.segment_start = now;
    }

    fn close_segment(&mut self, end: chrono::DateTime<chrono::Local>) {
        let app = match self.current.take() {
            Some(a) => a,
            None => return,
        };
        if self.excluded.contains(&app.app_key) {
            return;
        }
        let start = self.segment_start;
        let duration = (end - start).num_milliseconds() as f64 / 1000.0;
        if duration < 0.5 {
            return;
        }

        // 按天切分
        let mut s = start;
        while s < end {
            let day_end = day_start(s) + chrono::Duration::days(1);
            let e = if day_end < end { day_end } else { end };
            if e <= s {
                s = day_end;
                continue;
            }
            let key = day_key(s) + "|" + &app.app_key;
            let entry = self.daily.entry(key).or_insert_with(|| DailyUsage {
                usage_date: day_key(s),
                app_key: app.app_key.clone(),
                process_name: app.process_name.clone(),
                display_name: app.display_name.clone(),
                exe_path: app.exe_path.clone(),
                active_seconds: 0.0,
            });
            entry.active_seconds += (e - s).num_milliseconds() as f64 / 1000.0;
            self.pending_intervals.push(ForegroundInterval {
                app_key: app.app_key.clone(),
                process_name: app.process_name.clone(),
                display_name: app.display_name.clone(),
                exe_path: app.exe_path.clone(),
                start_time: s.format("%Y-%m-%d %H:%M:%S%.3f").to_string(),
                end_time: e.format("%Y-%m-%d %H:%M:%S%.3f").to_string(),
                duration_seconds: (e - s).num_milliseconds() as f64 / 1000.0,
            });
            s = day_end;
        }
    }
}

fn same_app(a: &Option<AppIdentity>, b: &Option<AppIdentity>) -> bool {
    match (a, b) {
        (Some(x), Some(y)) => x.app_key == y.app_key,
        (None, None) => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    // 聚合器状态机单元测试：区间闭合 / 空闲 / 锁定 / 跨午夜 / 排除应用
    use crate::aggregator::UsageAggregator;
    use crate::models::{AppIdentity, TrackingState};
    use chrono::{Local, TimeZone};
    use std::collections::HashSet;

    fn identity(name: &str) -> AppIdentity {
        AppIdentity {
            app_key: format!("c:\\apps\\{}.exe", name.to_lowercase()),
            process_name: name.to_string(),
            display_name: name.to_string(),
            exe_path: format!("C:\\apps\\{}.exe", name),
        }
    }

    fn at(y: i32, m: u32, d: u32, h: u32, min: u32, s: u32) -> chrono::DateTime<Local> {
        Local.with_ymd_and_hms(y, m, d, h, min, s).unwrap()
    }

    fn agg() -> UsageAggregator {
        UsageAggregator::new(180.0, HashSet::new())
    }

    #[test]
    fn 前台切换闭合区间并累计() {
        let mut a = agg();
        a.on_tick(Some(identity("Chrome")), 0.0, at(2026, 11, 1, 10, 0, 0));
        a.on_tick(Some(identity("Chrome")), 0.0, at(2026, 11, 1, 10, 0, 30));
        a.on_tick(Some(identity("Code")), 0.0, at(2026, 11, 1, 10, 2, 0));
        assert_eq!(a.state, TrackingState::Active);
        let chrome = a
            .daily
            .values()
            .find(|d| d.app_key.contains("chrome"))
            .expect("chrome row");
        assert!(
            (chrome.active_seconds - 120.0).abs() < 0.001,
            "got {}",
            chrome.active_seconds
        );
        assert_eq!(a.pending_intervals.len(), 1);
        assert_eq!(a.pending_intervals[0].duration_seconds, 120.0);
    }

    #[test]
    fn 空闲超阈值暂停且扣除空闲前未计入部分() {
        let mut a = agg();
        let start = at(2026, 11, 1, 10, 0, 0);
        a.on_tick(Some(identity("Chrome")), 0.0, start);
        // 10:10 检测到空闲 200 秒 → 活动止于 10:06:40
        a.on_tick(Some(identity("Chrome")), 200.0, at(2026, 11, 1, 10, 10, 0));
        assert_eq!(a.state, TrackingState::Idle);
        let chrome = a
            .daily
            .values()
            .find(|d| d.app_key.contains("chrome"))
            .unwrap();
        assert!(
            (chrome.active_seconds - 400.0).abs() < 0.001,
            "got {}",
            chrome.active_seconds
        );

        // 空闲期间不累计
        a.on_tick(Some(identity("Chrome")), 300.0, at(2026, 11, 1, 10, 12, 0));
        let chrome = a
            .daily
            .values()
            .find(|d| d.app_key.contains("chrome"))
            .unwrap();
        assert!((chrome.active_seconds - 400.0).abs() < 0.001);

        // 输入恢复 → 重新活跃
        a.on_tick(Some(identity("Chrome")), 0.0, at(2026, 11, 1, 10, 13, 0));
        assert_eq!(a.state, TrackingState::Active);
    }

    #[test]
    fn 锁屏闭合区间解锁恢复() {
        let mut a = agg();
        a.on_tick(Some(identity("Code")), 0.0, at(2026, 11, 1, 22, 0, 0));
        a.on_locked(at(2026, 11, 1, 23, 0, 0));
        assert_eq!(a.state, TrackingState::Locked);
        let code = a
            .daily
            .values()
            .find(|d| d.app_key.contains("code"))
            .unwrap();
        assert!((code.active_seconds - 3600.0).abs() < 0.001);
        a.on_tick(Some(identity("Code")), 0.0, at(2026, 11, 1, 23, 5, 0));
        assert_eq!(a.state, TrackingState::Locked, "锁定期间 tick 不应计时");
        a.on_unlocked(Some(identity("Code")), at(2026, 11, 1, 23, 5, 0));
        assert_eq!(a.state, TrackingState::Active);
    }

    #[test]
    fn 跨午夜按天切分() {
        let mut a = agg();
        a.on_tick(Some(identity("Steam")), 0.0, at(2026, 10, 31, 23, 50, 0));
        a.on_tick(Some(identity("Steam")), 0.0, at(2026, 11, 1, 0, 10, 0));
        a.on_tick(Some(identity("Code")), 0.0, at(2026, 11, 1, 0, 15, 0));
        let keys: Vec<&str> = a.daily.keys().map(|k| k.as_str()).collect();
        assert!(
            keys.contains(&"2026-10-31|c:\\apps\\steam.exe")
                && keys.contains(&"2026-11-01|c:\\apps\\steam.exe"),
            "days: {keys:?}"
        );
        let steam_1031 = a
            .daily
            .values()
            .find(|d| d.usage_date == "2026-10-31")
            .unwrap();
        assert!((steam_1031.active_seconds - 600.0).abs() < 0.001);
    }

    #[test]
    fn 排除应用不计入() {
        let mut excluded = HashSet::new();
        excluded.insert("c:\\windows\\explorer.exe".to_string());
        let mut a = UsageAggregator::new(180.0, excluded);
        let explorer = AppIdentity {
            app_key: "c:\\windows\\explorer.exe".to_string(),
            process_name: "explorer".to_string(),
            display_name: "explorer".to_string(),
            exe_path: "C:\\windows\\explorer.exe".to_string(),
        };
        a.on_tick(Some(explorer), 0.0, at(2026, 11, 1, 10, 0, 0));
        a.on_tick(Some(identity("Chrome")), 0.0, at(2026, 11, 1, 10, 1, 0));
        assert!(a.daily.is_empty(), "explorer 不应产生记录");
        assert_eq!(a.current_segment_seconds(at(2026, 11, 1, 10, 2, 0)), 60.0);
    }

    #[test]
    fn 暂停与恢复() {
        let mut a = agg();
        a.on_tick(Some(identity("Code")), 0.0, at(2026, 11, 1, 10, 0, 0));
        a.pause(at(2026, 11, 1, 10, 10, 0));
        assert_eq!(a.state, TrackingState::Paused);
        a.on_tick(Some(identity("Code")), 0.0, at(2026, 11, 1, 10, 20, 0));
        assert_eq!(a.state, TrackingState::Paused, "暂停期间 tick 不应恢复");
        let code = a
            .daily
            .values()
            .find(|d| d.app_key.contains("code"))
            .unwrap();
        assert!((code.active_seconds - 600.0).abs() < 0.001);
        a.resume(Some(identity("Code")), at(2026, 11, 1, 10, 21, 0));
        assert_eq!(a.state, TrackingState::Active);
    }

    #[test]
    fn 睡眠恢复重新活跃() {
        let mut a = agg();
        a.on_tick(Some(identity("Code")), 0.0, at(2026, 11, 1, 12, 0, 0));
        a.on_suspend(at(2026, 11, 1, 12, 30, 0));
        assert_eq!(a.state, TrackingState::Suspended);
        a.on_resume(Some(identity("Code")), at(2026, 11, 1, 13, 0, 0));
        assert_eq!(a.state, TrackingState::Active);
        let code = a
            .daily
            .values()
            .find(|d| d.app_key.contains("code"))
            .unwrap();
        assert!((code.active_seconds - 1800.0).abs() < 0.001);
    }

    #[test]
    fn 短于半秒的区间被丢弃() {
        let mut a = agg();
        a.on_tick(Some(identity("Code")), 0.0, at(2026, 11, 1, 10, 0, 0));
        a.on_tick(
            Some(identity("Chrome")),
            0.0,
            at(2026, 11, 1, 10, 0, 0).chrono_plus(0.2),
        );
        assert!(a.daily.is_empty());
    }

    trait ChronoPlus {
        fn chrono_plus(&self, secs: f64) -> chrono::DateTime<Local>;
    }

    impl ChronoPlus for chrono::DateTime<Local> {
        fn chrono_plus(&self, secs: f64) -> chrono::DateTime<Local> {
            *self + chrono::Duration::milliseconds((secs * 1000.0) as i64)
        }
    }
}
