//! 设置：settings.json 读写、语言/主题生效、分类规则缓存重建、暂停状态，
//! 以及这几个域对应的 IPC 命令。
//!
//! 这些函数原先散在 lib.rs 的「设置」与「Tauri Commands」两段里，行为未变；
//! 其他模块通过 crate::settings::xxx 调用。

use std::collections::{HashMap, HashSet};
use std::sync::atomic::Ordering;

use chrono::Local;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_global_shortcut::Shortcut;
use tracker_core::category::{CategoryResolver, UNCATEGORIZED};
use tracker_core::settings::{CategoryRuleDef, TrackerSettings};
use tracker_core::store::SqliteStore;

use crate::monitor::foreground_identity;
use crate::sync::MutexExt;
use crate::{api_server, apply_hotkeys, autostart, logging, windows, AppState};

/// 应用别名：仅影响展示，不改历史数据与分类匹配。
pub(crate) fn display_alias(settings: &TrackerSettings, app_key: &str, original: &str) -> String {
    settings
        .app_display_names
        .get(app_key)
        .cloned()
        .unwrap_or_else(|| original.to_string())
}

/// 解析界面语言（system = 中文）。
pub(crate) fn resolve_locale(language: &str) -> &'static str {
    if language.starts_with("en") {
        "en-US"
    } else {
        "zh-CN"
    }
}

pub(crate) fn apply_locale(app: &AppHandle) {
    let lang = {
        let st = app.state::<AppState>();
        let v = st.settings.lock_or_recover().language.clone();
        v
    };
    rust_i18n::set_locale(resolve_locale(&lang));
}

pub(crate) fn db_path(data_dir: &str) -> String {
    std::path::Path::new(data_dir)
        .join("usage.db")
        .to_string_lossy()
        .to_string()
}

pub(crate) fn load_settings() -> TrackerSettings {
    let base = TrackerSettings::default();
    let path = std::path::Path::new(&base.data_directory).join("settings.json");
    if let Ok(text) = std::fs::read_to_string(&path) {
        if let Ok(mut s) = serde_json::from_str::<TrackerSettings>(&text) {
            migrate_settings(&mut s);
            return s;
        }
    }
    base
}

/// 设置结构迁移链：按 schema_version 逐级升级（当前 v1）。
pub(crate) fn migrate_settings(s: &mut TrackerSettings) {
    if s.schema_version < 1 {
        s.schema_version = 1;
    }
}

pub(crate) fn save_settings_file(app: &AppHandle) {
    let state = app.state::<AppState>();
    let s = state.settings.lock_or_recover().clone();
    let _ = std::fs::create_dir_all(&s.data_directory);
    let path = std::path::Path::new(&s.data_directory).join("settings.json");
    if let Ok(json) = serde_json::to_string_pretty(&s) {
        let _ = std::fs::write(path, json);
    }
}

/// 悬浮窗建窗串行化：连续快速点击会各起一个线程，用这把锁保证只有一个真正建窗。
static WIDGET_CREATION_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub(crate) fn set_widget_visible(app: &AppHandle, visible: bool) {
    if visible {
        if app.get_webview_window("widget").is_none() {
            // tauri 文档（wry#583）：Windows 上在同步命令或事件处理器里调
            // WebviewWindowBuilder::build 会死锁 —— 建窗要等主线程的事件循环，
            // 而调用方此刻正占着主线程，界面就此卡死。这里把建窗挪到独立线程，
            // 主线程立刻返回、事件循环继续转，窗口建好后再挂拖拽约束。
            let handle = app.clone();
            std::thread::spawn(move || {
                // 拿不到锁（被毒化）也要继续建窗，锁只是用来串行化
                let _guard = WIDGET_CREATION_LOCK.lock_or_recover();
                // 排队期间前一个线程可能已经把窗口建好了
                if handle.get_webview_window("widget").is_some() {
                    return;
                }
                if let Err(e) = windows::create_widget_window(&handle) {
                    log::error!("create widget failed: {e}");
                    return;
                }
                windows::setup_widget_clamp(&handle);
            });
        } else if let Some(w) = app.get_webview_window("widget") {
            let _ = w.show();
        }
    } else if let Some(w) = app.get_webview_window("widget") {
        let _ = w.hide();
    }
    {
        let state = app.state::<AppState>();
        state.settings.lock_or_recover().show_floating_widget = visible;
    }
    save_settings_file(app);
    let _ = app.emit("widget://visibility", visible);
}

pub(crate) fn set_paused_internal(app: &AppHandle, paused: bool) {
    let state = app.state::<AppState>();
    state.user_paused.store(paused, Ordering::Relaxed);
    let mut agg = state.aggregator.lock_or_recover();
    let now = Local::now();
    if paused {
        agg.pause(now);
    } else {
        let mut cache = HashMap::new();
        agg.resume(foreground_identity(&mut cache), now);
    }
}

#[tauri::command]
pub(crate) fn get_settings(state: State<AppState>) -> TrackerSettings {
    state.settings.lock_or_recover().clone()
}

#[tauri::command]
pub(crate) fn save_settings(
    app: AppHandle,
    state: State<AppState>,
    settings: TrackerSettings,
) -> Result<(), String> {
    // 热键组合合法性校验（解析失败或重复即拒绝）
    if settings.hotkeys_enabled {
        let main_hk: Shortcut = settings
            .hotkey_main
            .parse()
            .map_err(|_| format!("主界面热键无效: {}", settings.hotkey_main))?;
        let widget_hk: Shortcut = settings
            .hotkey_widget
            .parse()
            .map_err(|_| format!("悬浮窗热键无效: {}", settings.hotkey_widget))?;
        if main_hk == widget_hk {
            return Err("主界面与悬浮窗热键不能相同".into());
        }
    }
    let old_dir = state.settings.lock_or_recover().data_directory.clone();
    let dir_changed = old_dir != settings.data_directory;

    autostart::set(
        settings.start_with_windows,
        &std::env::current_exe()
            .map_err(|e| e.to_string())?
            .to_string_lossy(),
    )?;

    if dir_changed {
        std::fs::create_dir_all(&settings.data_directory).map_err(|e| e.to_string())?;
        let (daily, intervals) = {
            let st = state.store.lock_or_recover();
            let daily = st
                .query_daily("0000-01-01", "9999-12-31")
                .map_err(|e| e.to_string())?;
            let intervals = st
                .query_intervals("0000-01-01", "9999-12-31")
                .map_err(|e| e.to_string())?;
            // 换库前把旧库的 WAL 落盘并截断，免得用户切回旧目录时看到一份
            // 落后若干个 30 秒周期的数据。
            let _ = st.checkpoint();
            (daily, intervals)
        };
        let new_store =
            SqliteStore::open(&db_path(&settings.data_directory)).map_err(|e| e.to_string())?;
        new_store
            .save_daily_replace(&daily)
            .map_err(|e| e.to_string())?;
        new_store
            .save_intervals(&intervals)
            .map_err(|e| e.to_string())?;
        *state.store.lock_or_recover() = new_store;
        logging::set_dir(&settings.data_directory);
    }

    {
        let mut excluded: HashSet<String> = settings.excluded_app_keys.iter().cloned().collect();
        if let Ok(self_exe) = std::env::current_exe() {
            excluded.insert(self_exe.to_string_lossy().to_lowercase());
        }
        let mut agg = state.aggregator.lock_or_recover();
        agg.apply_settings(settings.idle_threshold_seconds as f64, excluded);
    }

    *state.settings.lock_or_recover() = settings.clone();
    save_settings_file(&app);
    // 先重建 resolver 缓存，再通知前端，避免前端刷新时仍读到旧分类
    rebuild_resolver(&app);
    let _ = app.emit("app://settings", &settings);
    apply_locale(&app);
    apply_hotkeys(&app);
    api_server::spawn_api_server(&app);

    // 悬浮窗可见性同步
    let want = state.settings.lock_or_recover().show_floating_widget;
    let now_visible = app
        .get_webview_window("widget")
        .map(|w| w.is_visible().unwrap_or(false))
        .unwrap_or(false);
    if want != now_visible {
        set_widget_visible(&app, want);
    }
    if let Some(h) = state.tray.lock_or_recover().as_ref() {
        let _ = h.widget_toggle.set_checked(want);
    }
    Ok(())
}

#[tauri::command]
pub(crate) fn set_paused(app: AppHandle, state: State<AppState>, paused: bool) {
    set_paused_internal(&app, paused);
    if let Some(h) = state.tray.lock_or_recover().as_ref() {
        let _ = h.pause_toggle.set_checked(paused);
    }
}

/// 设置应用别名（空 = 恢复默认名）。
#[tauri::command]
pub(crate) fn set_app_display_name(
    app: AppHandle,
    state: State<AppState>,
    app_key: String,
    name: String,
) {
    {
        let mut s = state.settings.lock_or_recover();
        let trimmed = name.trim().to_string();
        if trimmed.is_empty() {
            s.app_display_names.remove(&app_key);
        } else {
            s.app_display_names.insert(app_key, trimmed);
        }
    }
    save_settings_file(&app);
    let settings = state.settings.lock_or_recover().clone();
    let _ = app.emit("app://settings", &settings);
}

/// 排除应用：停止统计 + 清除其全部已有记录。
#[tauri::command]
pub(crate) fn exclude_app(
    app: AppHandle,
    state: State<AppState>,
    app_key: String,
) -> Result<(), String> {
    {
        let mut s = state.settings.lock_or_recover();
        if !s.excluded_app_keys.contains(&app_key) {
            s.excluded_app_keys.push(app_key.clone());
        }
    }
    save_settings_file(&app);
    {
        let (idle, excluded_list) = {
            let s = state.settings.lock_or_recover();
            (s.idle_threshold_seconds as f64, s.excluded_app_keys.clone())
        };
        let mut excluded: HashSet<String> = excluded_list.into_iter().collect();
        if let Ok(self_exe) = std::env::current_exe() {
            excluded.insert(self_exe.to_string_lossy().to_lowercase());
        }
        let mut agg = state.aggregator.lock_or_recover();
        agg.apply_settings(idle, excluded);
        agg.daily.retain(|_, v| v.app_key != app_key);
        agg.pending_intervals.retain(|v| v.app_key != app_key);
        agg.pending_title_intervals.retain(|v| v.app_key != app_key);
    }
    state
        .store
        .lock_or_recover()
        .delete_app_rows(&app_key)
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// 分类规则/覆盖变化 → 重建 resolver 缓存（仪表盘、时间线、实时状态都读它）。
pub(crate) fn rebuild_resolver(app: &AppHandle) {
    let state = app.state::<AppState>();
    let (rules, overrides) = {
        let s = state.settings.lock_or_recover();
        (s.category_rules.clone(), s.app_category_overrides.clone())
    };
    *state.resolver.lock_or_recover() =
        std::sync::Arc::new(CategoryResolver::new(rules, overrides));
}

#[tauri::command]
pub(crate) fn set_app_category(
    app: AppHandle,
    state: State<AppState>,
    app_key: String,
    category: String,
) {
    {
        let mut s = state.settings.lock_or_recover();
        if category == UNCATEGORIZED {
            s.app_category_overrides.remove(&app_key);
        } else {
            s.app_category_overrides.insert(app_key, category);
        }
    }
    save_settings_file(&app);
    rebuild_resolver(&app);
    let settings = state.settings.lock_or_recover().clone();
    let _ = app.emit("app://settings", &settings);
}

#[tauri::command]
pub(crate) fn save_category_rules(
    app: AppHandle,
    state: State<AppState>,
    rules: Vec<CategoryRuleDef>,
) -> Result<(), String> {
    if rules.is_empty() {
        return Err("至少保留一条分类规则".into());
    }
    for r in &rules {
        if r.name.trim().is_empty() || r.keywords.is_empty() {
            return Err("分类名与关键词不能为空".into());
        }
    }
    {
        let mut s = state.settings.lock_or_recover();
        s.category_rules = rules;
    }
    save_settings_file(&app);
    rebuild_resolver(&app);
    let settings = state.settings.lock_or_recover().clone();
    let _ = app.emit("app://settings", &settings);
    Ok(())
}

#[tauri::command]
pub(crate) fn list_available_categories(state: State<AppState>) -> Vec<String> {
    let s = state.settings.lock_or_recover();
    let mut cats: Vec<String> = s.category_rules.iter().map(|r| r.name.clone()).collect();
    cats.push(UNCATEGORIZED.to_string());
    cats
}
