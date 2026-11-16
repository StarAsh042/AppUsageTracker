//! 实时状态：悬浮窗与主窗口顶栏共用的「当前应用 / 今日总量 / Top 5」快照。
//!
//! 从 lib.rs 整体搬出（拆分第十五步），行为不变。

use std::collections::HashSet;

use chrono::Local;
use serde::Serialize;
use tauri::{AppHandle, Manager};
use tracker_core::models::{AppIdentity, TrackingState};

use crate::AppState;

#[derive(Serialize, Clone)]
pub(crate) struct LiveTopApp {
    pub(crate) display_name: String,
    pub(crate) category: String,
    pub(crate) active_seconds: f64,
}

#[derive(Serialize, Clone)]
pub(crate) struct LiveStatus {
    pub(crate) state: TrackingState,
    pub(crate) current_app: Option<AppIdentity>,
    pub(crate) current_category: Option<String>,
    pub(crate) current_seconds: f64,
    pub(crate) today_total_seconds: f64,
    pub(crate) top_apps: Vec<LiveTopApp>,
}

/// 组一份实时快照；IPC 的 get_live_status、HTTP 的 /api/live 与监控线程的 tick 共用。
pub(crate) fn live_status(app: &AppHandle) -> LiveStatus {
    let state = app.state::<AppState>();
    let resolver = app.state::<AppState>().resolver.lock().unwrap().clone();
    let now = Local::now();
    let today = now.format("%Y-%m-%d").to_string();
    // 先把设置整份快照出来再进聚合器锁：既避免 settings→aggregator 反序造成的
    // ABBA 死锁，也让下面的别名校验不再每行加一次锁。
    let (aliases, excluded) = {
        let s = state.settings.lock().unwrap();
        let mut excluded: HashSet<String> = s.excluded_app_keys.iter().cloned().collect();
        if let Ok(self_exe) = std::env::current_exe() {
            excluded.insert(self_exe.to_string_lossy().to_lowercase());
        }
        (s.app_display_names.clone(), excluded)
    };
    let alias_of = |app_key: &str, original: &str| -> String {
        aliases
            .get(app_key)
            .cloned()
            .unwrap_or_else(|| original.to_string())
    };
    let agg = state.aggregator.lock().unwrap();
    // 被排除的应用不再作为「当前前台应用」外露（悬浮窗 / 顶栏状态）
    let current = agg
        .current_app()
        .filter(|c| !excluded.contains(&c.app_key))
        .cloned();
    let (current_category, current_seconds) = match &current {
        Some(c) => (
            Some(resolver.resolve(&c.display_name, &c.process_name, &c.exe_path)),
            agg.current_segment_seconds(now),
        ),
        None => (None, 0.0),
    };
    let mut top: Vec<(String, String, f64)> = Vec::new();
    let mut today_total = 0.0;
    let cur_key = current.as_ref().map(|c| c.app_key.clone());
    for d in agg.daily.values().filter(|d| d.usage_date == today) {
        // 当前前台应用叠加进行中的区间秒数，排行与总量实时走表
        let mut secs = d.active_seconds;
        if cur_key.as_deref() == Some(d.app_key.as_str()) {
            secs += current_seconds;
        }
        today_total += secs;
        let cat = resolver.resolve(&d.display_name, &d.process_name, &d.exe_path);
        top.push((alias_of(&d.app_key, &d.display_name), cat, secs));
    }
    top.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
    let top_apps = top
        .into_iter()
        .take(5)
        .map(|(display_name, category, active_seconds)| LiveTopApp {
            display_name,
            category,
            active_seconds,
        })
        .collect();
    // 当前应用显示名走别名
    let mut current = current;
    if let Some(c) = current.as_mut() {
        c.display_name = alias_of(&c.app_key, &c.display_name);
    }
    LiveStatus {
        state: agg.state,
        current_app: current,
        current_category,
        current_seconds,
        today_total_seconds: today_total,
        top_apps,
    }
}

// ---------- 悬浮窗 / 主窗口实时状态相关的 IPC 命令 ----------

#[tauri::command]
pub(crate) fn get_live_status(app: AppHandle) -> LiveStatus {
    live_status(&app)
}
