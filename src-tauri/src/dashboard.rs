//! 仪表盘 / 周报共用的数据聚合：把 daily 表与内存实时数据合成前端要的结构。
//!
//! 从 lib.rs 整体搬出（拆分第八步），行为不变。
use std::collections::HashMap;

use serde::Serialize;
use tauri::{AppHandle, Manager};
use tracker_core::models::DailyUsage;

use crate::settings::display_alias;
use crate::AppState;

#[derive(Serialize, Clone)]
pub(crate) struct DashboardApp {
    pub(crate) app_key: String,
    pub(crate) display_name: String,
    pub(crate) process_name: String,
    pub(crate) exe_path: String,
    pub(crate) category: String,
    pub(crate) active_seconds: f64,
    pub(crate) percent: f64,
}

#[derive(Serialize, Clone)]
pub(crate) struct DashboardCategory {
    pub(crate) name: String,
    pub(crate) seconds: f64,
    pub(crate) percent: f64,
}

#[derive(Serialize, Clone)]
pub(crate) struct DashboardTrendPoint {
    pub(crate) date: String,
    pub(crate) seconds: f64,
    /// 当日效率评分（0-100，仅当存在已标记性质的分类时才有值）
    pub(crate) score: Option<f64>,
}

#[derive(Serialize, Clone)]
pub(crate) struct DashboardData {
    pub(crate) from: String,
    pub(crate) to: String,
    pub(crate) total_active_seconds: f64,
    pub(crate) top_app: Option<String>,
    pub(crate) apps: Vec<DashboardApp>,
    pub(crate) categories: Vec<DashboardCategory>,
    pub(crate) trend: Vec<DashboardTrendPoint>,
}

#[tauri::command]
pub(crate) fn get_dashboard(
    app: AppHandle,
    from: String,
    to: String,
) -> Result<DashboardData, String> {
    build_dashboard(&app, &from, &to)
}

pub(crate) fn build_dashboard(
    app: &AppHandle,
    from: &str,
    to: &str,
) -> Result<DashboardData, String> {
    let state = app.state::<AppState>();
    let resolver = app.state::<AppState>().resolver.lock().unwrap().clone();

    let store_rows = {
        let st = state.store.lock().unwrap();
        st.query_daily(from, to).map_err(|e| e.to_string())?
    };
    let mut by_key: HashMap<String, DailyUsage> = store_rows
        .into_iter()
        .map(|r| (format!("{}|{}", r.usage_date, r.app_key), r))
        .collect();
    {
        // 今日内存实时数据覆盖库中同键数据
        let agg = state.aggregator.lock().unwrap();
        for (k, v) in &agg.daily {
            if v.usage_date.as_str() >= from && v.usage_date.as_str() <= to {
                by_key.insert(k.clone(), v.clone());
            }
        }
    }

    struct AggApp {
        display_name: String,
        process_name: String,
        exe_path: String,
        seconds: f64,
    }
    let nature = {
        let s = state.settings.lock().unwrap();
        s.category_nature.clone()
    };
    let mut per_app: HashMap<String, AggApp> = HashMap::new();
    let mut trend: HashMap<String, f64> = HashMap::new();
    // 每日每分类秒数（算当日效率评分）
    let mut day_cat: HashMap<(String, String), f64> = HashMap::new();
    let mut total = 0.0;
    for (_, r) in by_key {
        let e = per_app.entry(r.app_key.clone()).or_insert(AggApp {
            display_name: r.display_name.clone(),
            process_name: r.process_name.clone(),
            exe_path: r.exe_path.clone(),
            seconds: 0.0,
        });
        e.seconds += r.active_seconds;
        if e.display_name.is_empty() {
            e.display_name = r.display_name.clone();
            e.exe_path = r.exe_path.clone();
        }
        *trend.entry(r.usage_date.clone()).or_insert(0.0) += r.active_seconds;
        let cat = resolver.resolve(&r.display_name, &r.process_name, &r.exe_path);
        *day_cat.entry((r.usage_date.clone(), cat)).or_insert(0.0) += r.active_seconds;
        total += r.active_seconds;
    }
    let day_score = |date: &str| -> Option<f64> {
        let mut marked = 0.0;
        let mut weighted = 0.0;
        for ((d, cat), secs) in &day_cat {
            if d != date {
                continue;
            }
            match nature.get(cat).map(|x| x.as_str()) {
                Some("productive") => {
                    marked += secs;
                    weighted += secs;
                }
                Some("distracting") => marked += secs,
                _ => {}
            }
        }
        if marked < 1.0 {
            None
        } else {
            Some((weighted / marked * 100.0 * 10.0).round() / 10.0)
        }
    };

    let mut apps: Vec<DashboardApp> = per_app
        .into_iter()
        .map(|(app_key, e)| {
            let category = resolver.resolve(&e.display_name, &e.process_name, &e.exe_path);
            let shown = {
                let s = state.settings.lock().unwrap();
                display_alias(&s, &app_key, &e.display_name)
            };
            DashboardApp {
                app_key,
                display_name: shown,
                process_name: e.process_name,
                exe_path: e.exe_path,
                category,
                active_seconds: e.seconds,
                percent: if total > 0.0 {
                    e.seconds / total * 100.0
                } else {
                    0.0
                },
            }
        })
        .collect();
    apps.sort_by(|a, b| {
        b.active_seconds
            .partial_cmp(&a.active_seconds)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut cats: HashMap<String, f64> = HashMap::new();
    for a in &apps {
        *cats.entry(a.category.clone()).or_insert(0.0) += a.active_seconds;
    }
    let mut categories: Vec<DashboardCategory> = cats
        .into_iter()
        .map(|(name, seconds)| DashboardCategory {
            name,
            seconds,
            percent: if total > 0.0 {
                seconds / total * 100.0
            } else {
                0.0
            },
        })
        .collect();
    categories.sort_by(|a, b| {
        b.seconds
            .partial_cmp(&a.seconds)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut trend: Vec<DashboardTrendPoint> = trend
        .into_iter()
        .map(|(date, seconds)| DashboardTrendPoint {
            score: day_score(&date),
            date,
            seconds,
        })
        .collect();
    trend.sort_by(|a, b| a.date.cmp(&b.date));

    let top_app = apps.first().map(|a| a.display_name.clone());
    Ok(DashboardData {
        from: from.to_string(),
        to: to.to_string(),
        total_active_seconds: total,
        top_app,
        apps,
        categories,
        trend,
    })
}
