//! 时间线数据：某一天的活跃区间与状态区间。
//!
//! 从 lib.rs 整体搬出（拆分第九步），行为不变。
use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::settings::display_alias;
use crate::AppState;

#[derive(Serialize, Clone)]
pub(crate) struct TimelineSegment {
    pub(crate) app_key: String,
    pub(crate) display_name: String,
    pub(crate) category: String,
    pub(crate) start_time: String,
    pub(crate) end_time: String,
    pub(crate) duration_seconds: f64,
}

#[derive(Serialize, Clone)]
pub(crate) struct TimelineState {
    pub(crate) state: String,
    pub(crate) start_time: String,
    pub(crate) end_time: String,
    pub(crate) duration_seconds: f64,
}

#[derive(Serialize, Clone)]
pub(crate) struct TimelineData {
    pub(crate) date: String,
    pub(crate) segments: Vec<TimelineSegment>,
    pub(crate) states: Vec<TimelineState>,
}

#[tauri::command]
pub(crate) fn get_timeline(app: AppHandle, date: String) -> Result<TimelineData, String> {
    timeline_data(&app, &date)
}

pub(crate) fn timeline_data(app: &AppHandle, date: &str) -> Result<TimelineData, String> {
    let app = app.clone();
    let date = date.to_string();
    let state = app.state::<AppState>();
    let resolver = app.state::<AppState>().resolver.lock().unwrap().clone();
    let (intervals, states) = {
        let st = state.store.lock().unwrap();
        (
            st.query_intervals(&date, &date)
                .map_err(|e| e.to_string())?,
            st.query_state_intervals(&date, &date)
                .map_err(|e| e.to_string())?,
        )
    };
    let segments = intervals
        .into_iter()
        .take(3000)
        .map(|i| TimelineSegment {
            category: resolver.resolve(&i.display_name, &i.process_name, &i.exe_path),
            display_name: {
                let s = state.settings.lock().unwrap();
                display_alias(&s, &i.app_key, &i.display_name)
            },
            app_key: i.app_key,
            start_time: i.start_time,
            end_time: i.end_time,
            duration_seconds: i.duration_seconds,
        })
        .collect();
    Ok(TimelineData {
        date,
        segments,
        states: states
            .into_iter()
            .map(|s| TimelineState {
                state: s.state,
                start_time: s.start_time,
                end_time: s.end_time,
                duration_seconds: s.duration_seconds,
            })
            .collect(),
    })
}
