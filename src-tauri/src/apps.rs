//! 应用详情与应用扫描（桌面 / 开始菜单 .lnk → 补分类）。
//!
//! 从 lib.rs 整体搬出（拆分第十步），行为不变。
use std::collections::HashMap;
#[cfg(windows)]
use std::collections::HashSet;

use serde::Serialize;
use tauri::{AppHandle, Manager};

#[cfg(windows)]
use tracker_core::category::UNCATEGORIZED;

use crate::dashboard::DashboardTrendPoint;
use crate::error::AppError;
use crate::settings::display_alias;
use crate::sync::MutexExt;
use crate::AppState;

#[derive(Serialize, Clone)]
pub(crate) struct AppDetail {
    pub(crate) app_key: String,
    pub(crate) display_name: String,
    pub(crate) exe_path: String,
    pub(crate) category: String,
    pub(crate) total_seconds: f64,
    pub(crate) daily: Vec<DashboardTrendPoint>,
}

#[tauri::command]
pub(crate) fn get_app_detail(
    app: AppHandle,
    app_key: String,
    from: String,
    to: String,
) -> Result<AppDetail, AppError> {
    let state = app.state::<AppState>();
    let resolver = app.state::<AppState>().resolver.lock_or_recover().clone();

    let store_rows = {
        let st = state.store.lock_or_recover();
        st.query_daily(&from, &to)
            .map_err(|e| AppError::new("db", e.to_string()))?
    };
    let mut daily: HashMap<String, f64> = HashMap::new();
    let mut display_name = String::new();
    let mut exe_path = String::new();
    for r in store_rows.into_iter().filter(|r| r.app_key == app_key) {
        if display_name.is_empty() {
            display_name = r.display_name.clone();
            exe_path = r.exe_path.clone();
        }
        *daily.entry(r.usage_date).or_insert(0.0) += r.active_seconds;
    }
    {
        // 今日内存实时数据
        let agg = state.aggregator.lock_or_recover();
        for v in agg.daily.values().filter(|v| {
            v.app_key == app_key
                && v.usage_date.as_str() >= from.as_str()
                && v.usage_date.as_str() <= to.as_str()
        }) {
            if display_name.is_empty() {
                display_name = v.display_name.clone();
                exe_path = v.exe_path.clone();
            }
            *daily.entry(v.usage_date.clone()).or_insert(0.0) += v.active_seconds;
        }
    }
    if display_name.is_empty() {
        display_name = app_key
            .rsplit(['\\', '/'])
            .next()
            .unwrap_or(&app_key)
            .trim_end_matches(".exe")
            .to_string();
    }
    let category = resolver.resolve(&display_name, &display_name, &exe_path);
    display_name = {
        let s = state.settings.lock_or_recover();
        display_alias(&s, &app_key, &display_name)
    };
    let mut points: Vec<DashboardTrendPoint> = daily
        .into_iter()
        .map(|(date, seconds)| DashboardTrendPoint {
            date,
            seconds,
            score: None,
        })
        .collect();
    points.sort_by(|a, b| a.date.cmp(&b.date));
    let total_seconds = points.iter().map(|p| p.seconds).sum();
    Ok(AppDetail {
        app_key,
        display_name,
        exe_path,
        category,
        total_seconds,
        daily: points,
    })
}

// ---------- 应用扫描（桌面 / 开始菜单 lnk → 补分类） ----------

#[derive(Serialize, Clone)]
pub(crate) struct ScannedApp {
    pub(crate) app_key: String,
    pub(crate) display_name: String,
    pub(crate) category: String,
    pub(crate) source: String,
}

#[tauri::command]
pub(crate) fn scan_installed_apps(app: AppHandle) -> Result<Vec<ScannedApp>, AppError> {
    #[cfg(windows)]
    {
        let resolver = app.state::<AppState>().resolver.lock_or_recover().clone();

        let mut dirs: Vec<(std::path::PathBuf, &str)> = Vec::new();
        if let Ok(profile) = std::env::var("USERPROFILE") {
            let desktop = std::path::Path::new(&profile).join("Desktop");
            if desktop.is_dir() {
                dirs.push((desktop, "桌面"));
            }
            let onedrive_desktop = std::path::Path::new(&profile)
                .join("OneDrive")
                .join("Desktop");
            if onedrive_desktop.is_dir() {
                dirs.push((onedrive_desktop, "桌面"));
            }
        }
        if let Ok(appdata) = std::env::var("APPDATA") {
            let start_menu =
                std::path::Path::new(&appdata).join(r"Microsoft\Windows\Start Menu\Programs");
            if start_menu.is_dir() {
                dirs.push((start_menu, "开始菜单"));
            }
        }
        if let Ok(pd) = std::env::var("PROGRAMDATA") {
            let start_menu =
                std::path::Path::new(&pd).join(r"Microsoft\Windows\Start Menu\Programs");
            if start_menu.is_dir() {
                dirs.push((start_menu, "开始菜单"));
            }
        }

        let mut seen: HashSet<String> = HashSet::new();
        let mut result: Vec<ScannedApp> = Vec::new();
        for (dir, source) in dirs {
            collect_lnks(&dir, &mut |lnk_path, name| {
                let Ok(link) = lnk::ShellLink::open(lnk_path, encoding_rs::GBK) else {
                    return;
                };
                let Some(target) = link
                    .link_info()
                    .as_ref()
                    .and_then(|li| li.local_base_path().map(|s| s.to_string()))
                    .map(std::path::PathBuf::from)
                else {
                    return;
                };
                if target
                    .extension()
                    .map(|e| e.to_string_lossy().to_lowercase() != "exe")
                    .unwrap_or(true)
                {
                    return;
                }
                let exe = target.to_string_lossy().to_string();
                let key = exe.to_lowercase();
                if key.is_empty() || !seen.insert(key.clone()) {
                    return;
                }
                let display = if name.is_empty() {
                    target
                        .file_stem()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_else(|| key.clone())
                } else {
                    name
                };
                let category = resolver.resolve(&display, &display, &exe);
                result.push(ScannedApp {
                    app_key: key,
                    display_name: display,
                    category,
                    source: source.to_string(),
                });
            });
        }
        result.sort_by(|a, b| {
            (a.category == UNCATEGORIZED)
                .cmp(&(b.category == UNCATEGORIZED))
                .then_with(|| b.category.cmp(&a.category))
                .then_with(|| a.display_name.cmp(&b.display_name))
        });
        Ok(result)
    }
    #[cfg(not(windows))]
    {
        let _ = app;
        Ok(vec![])
    }
}

/// 递归收集 .lnk 文件并回调（路径，lnk 文件名去扩展名）。
#[cfg(windows)]
pub(crate) fn collect_lnks(dir: &std::path::Path, cb: &mut dyn FnMut(&std::path::PathBuf, String)) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_dir() {
            collect_lnks(&path, cb);
        } else if path.extension().map(|e| e == "lnk").unwrap_or(false) {
            let name = path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            cb(&path, name);
        }
    }
}
