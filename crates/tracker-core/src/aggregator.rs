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
    /// 今日逐应用活跃秒数（含当前未闭合区间之外的全部已闭合区间）。
    pub daily: HashMap<String, DailyUsage>,
    /// 待写入的前台明细。
    pub pending_intervals: Vec<ForegroundInterval>,
}

fn day_key(dt: chrono::DateTime<chrono::Local>) -> String {
    dt.format("%Y-%m-%d").to_string()
}

fn day_start(dt: chrono::DateTime<chrono::Local>) -> chrono::DateTime<chrono::Local> {
    dt.date_naive().and_hms_opt(0, 0, 0).unwrap().and_local_timezone(chrono::Local).unwrap()
}

impl UsageAggregator {
    pub fn new(idle_threshold_seconds: f64, excluded: HashSet<String>) -> Self {
        Self {
            state: TrackingState::Active,
            idle_threshold_seconds,
            excluded,
            current: None,
            segment_start: chrono::Local::now(),
            state_changed_at: chrono::Local::now(),
            daily: HashMap::new(),
            pending_intervals: Vec::new(),
        }
    }

    /// 前台轮询 tick：处理状态切换与区间闭合。
    pub fn on_tick(&mut self, foreground: Option<AppIdentity>, idle_seconds: f64, now: chrono::DateTime<chrono::Local>) {
        match self.state {
            TrackingState::Paused | TrackingState::Locked | TrackingState::Suspended => {
                if self.state == TrackingState::Suspended && now - self.state_changed_at > chrono::Duration::minutes(10) {
                    self.enter_active(foreground, now);
                }
                return;
            }
            _ => {}
        }

        let is_idle = idle_seconds >= self.idle_threshold_seconds;

        if self.state == TrackingState::Idle {
            if is_idle { return; }
            self.enter_active(foreground, now);
            return;
        }

        if is_idle {
            let idle_start = now - chrono::Duration::milliseconds((idle_seconds * 1000.0) as i64);
            let end = if idle_start < self.segment_start { self.segment_start } else { idle_start };
            self.close_segment(end);
            self.state = TrackingState::Idle;
            self.state_changed_at = now;
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
            self.state = TrackingState::Locked;
            self.state_changed_at = now;
            self.current = None;
        }
    }

    pub fn on_unlocked(&mut self, foreground: Option<AppIdentity>, now: chrono::DateTime<chrono::Local>) {
        if self.state == TrackingState::Locked { self.enter_active(foreground, now); }
    }

    pub fn on_suspend(&mut self, now: chrono::DateTime<chrono::Local>) {
        self.close_segment(now);
        self.state = TrackingState::Suspended;
        self.state_changed_at = now;
        self.current = None;
    }

    pub fn on_resume(&mut self, foreground: Option<AppIdentity>, now: chrono::DateTime<chrono::Local>) {
        if self.state == TrackingState::Suspended { self.enter_active(foreground, now); }
    }

    pub fn pause(&mut self, now: chrono::DateTime<chrono::Local>) {
        if self.state != TrackingState::Paused {
            self.close_segment(now);
            self.state = TrackingState::Paused;
            self.state_changed_at = now;
            self.current = None;
        }
    }

    pub fn resume(&mut self, foreground: Option<AppIdentity>, now: chrono::DateTime<chrono::Local>) {
        if self.state == TrackingState::Paused { self.enter_active(foreground, now); }
    }

    pub fn current_segment_seconds(&self, now: chrono::DateTime<chrono::Local>) -> f64 {
        if self.state != TrackingState::Active || self.current.is_none() { return 0.0; }
        if self.current.as_ref().map_or(false, |a| self.excluded.contains(&a.app_key)) { return 0.0; }
        let d = (now - self.segment_start).num_milliseconds() as f64 / 1000.0;
        if d > 0.0 { d } else { 0.0 }
    }

    pub fn current_app(&self) -> Option<&AppIdentity> { self.current.as_ref() }

    /// 热更新设置（空闲阈值 / 排除名单），保留当前区间与今日数据。
    pub fn apply_settings(&mut self, idle_threshold_seconds: f64, excluded: HashSet<String>) {
        self.idle_threshold_seconds = idle_threshold_seconds;
        self.excluded = excluded;
    }

    /// 闭合未完区间（程序退出前调用）。
    pub fn close_open_segment(&mut self, now: chrono::DateTime<chrono::Local>) {
        self.close_segment(now);
    }

    fn enter_active(&mut self, foreground: Option<AppIdentity>, now: chrono::DateTime<chrono::Local>) {
        self.state = TrackingState::Active;
        self.state_changed_at = now;
        self.current = foreground;
        self.segment_start = now;
    }

    fn close_segment(&mut self, end: chrono::DateTime<chrono::Local>) {
        let app = match self.current.take() { Some(a) => a, None => return };
        if self.excluded.contains(&app.app_key) { return; }
        let start = self.segment_start;
        let duration = (end - start).num_milliseconds() as f64 / 1000.0;
        if duration < 0.5 { return; }

        // 按天切分
        let mut s = start;
        while s < end {
            let day_end = day_start(s) + chrono::Duration::days(1);
            let e = if day_end < end { day_end } else { end };
            if e <= s { s = day_end; continue; }
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
