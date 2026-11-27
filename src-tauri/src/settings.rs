//! 设置：settings.json 读写、语言/主题生效、分类规则缓存重建、暂停状态，
//! 以及这几个域对应的 IPC 命令。
//!
//! 这些函数原先散在 lib.rs 的「设置」与「Tauri Commands」两段里，行为未变；
//! 其他模块通过 crate::settings::xxx 调用。

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};

use chrono::Local;
use rust_i18n::t;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_global_shortcut::Shortcut;
use tauri_plugin_notification::NotificationExt;
use tracker_core::category::{CategoryResolver, UNCATEGORIZED};
use tracker_core::settings::{CategoryRuleDef, TrackerSettings};
use tracker_core::store::SqliteStore;

use crate::error::AppError;
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

const SETTINGS_FILE: &str = "settings.json";
const SETTINGS_TMP: &str = "settings.json.tmp";
const SETTINGS_BACKUP: &str = "settings.json.bak";
const SETTINGS_CORRUPT: &str = "settings.json.corrupt";

/// 读设置的结果。后两种说明配置出过问题，启动时要告诉用户。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LoadEvent {
    /// 读到了配置，或者这是第一次运行（还没有文件）。
    Ok,
    /// 主文件读不出来，用 .bak 顶上了。
    RecoveredFromBackup,
    /// 主文件与备份都读不出来，回到默认值。
    ResetToDefault,
}

pub(crate) fn load_settings() -> (TrackerSettings, LoadEvent) {
    let dir = std::path::PathBuf::from(TrackerSettings::default().data_directory);
    load_settings_from(&dir)
}

/// 从指定目录读设置。单元测试走这里，不碰真实数据目录。
pub(crate) fn load_settings_from(dir: &std::path::Path) -> (TrackerSettings, LoadEvent) {
    let path = dir.join(SETTINGS_FILE);
    if let Some(s) = read_settings_file(&path) {
        return (s, LoadEvent::Ok);
    }
    if !path.exists() {
        // 首次运行：没有文件是正常状态，不是损坏。
        return (TrackerSettings::default(), LoadEvent::Ok);
    }
    // 主文件损坏：先挪开留证，再试备份。
    let _ = std::fs::rename(&path, dir.join(SETTINGS_CORRUPT));
    if let Some(s) = read_settings_file(&dir.join(SETTINGS_BACKUP)) {
        if let Ok(json) = serde_json::to_string_pretty(&s) {
            let _ = write_settings_file(dir, &json);
        }
        return (s, LoadEvent::RecoveredFromBackup);
    }
    (TrackerSettings::default(), LoadEvent::ResetToDefault)
}

fn read_settings_file(path: &std::path::Path) -> Option<TrackerSettings> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut s: TrackerSettings = serde_json::from_str(&text).ok()?;
    migrate_settings(&mut s);
    Some(s)
}

/// 设置结构迁移链：按 schema_version 逐级升级（当前 v1）。
///
/// 加 v2 时在这里补一个 `if s.schema_version < 2` 分支再改成 2；旧文件缺的新
/// 字段由 serde 默认值补齐，不要复用已有版本号表达新语义。
pub(crate) fn migrate_settings(s: &mut TrackerSettings) {
    if s.schema_version < 1 {
        s.schema_version = 1;
    }
}

/// 落盘：先写临时文件并刷盘，再 rename 覆盖，另存一份可解析的 .bak。
///
/// 直接写 settings.json 的写法在写到一半断电时会留下截断的 JSON，下次启动读
/// 不出来，全部设置静默回默认值 —— 用户看到的是「应用忘掉了所有配置」，而且
/// 没有任何报错。同分区内 rename 是原子的，读者只会看到旧内容或新内容。
pub(crate) fn write_settings_file(dir: &std::path::Path, json: &str) -> std::io::Result<()> {
    use std::io::Write;
    std::fs::create_dir_all(dir)?;
    let path = dir.join(SETTINGS_FILE);
    let tmp = dir.join(SETTINGS_TMP);
    {
        let mut file = std::fs::File::create(&tmp)?;
        file.write_all(json.as_bytes())?;
        // rename 之前必须刷盘：否则断电后 tmp 是 0 字节，rename 完等于把配置删了。
        file.sync_all()?;
    }
    // 备份只在旧文件仍可解析时更新，保证 .bak 里永远躺着一份能用的设置。
    if let Ok(old) = std::fs::read_to_string(&path) {
        if serde_json::from_str::<TrackerSettings>(&old).is_ok() {
            let _ = std::fs::copy(&path, dir.join(SETTINGS_BACKUP));
        }
    }
    std::fs::rename(&tmp, &path)
}

/// 本次运行是否已经提示过「设置写不进磁盘」。
static SETTINGS_SAVE_FAILED: AtomicBool = AtomicBool::new(false);

fn notify(app: &AppHandle, title: &str, body: &str) {
    let _ = app.notification().builder().title(title).body(body).show();
}

/// 启动时把读取结果里需要人知道的两种讲出来。
pub(crate) fn report_load_event(app: &AppHandle, event: LoadEvent) {
    match event {
        LoadEvent::Ok => {}
        LoadEvent::RecoveredFromBackup => {
            log::warn!("settings.json 读不出来，已从备份恢复");
            notify(
                app,
                &t!("设置文件损坏"),
                &t!("已从备份恢复，最近的改动可能丢失，损坏的文件保留为 settings.json.corrupt。"),
            );
        }
        LoadEvent::ResetToDefault => {
            log::error!("settings.json 与备份都读不出来，已回到默认设置");
            notify(
                app,
                &t!("设置文件损坏"),
                &t!("已重置为默认设置，损坏的文件保留为 settings.json.corrupt。"),
            );
        }
    }
}

pub(crate) fn save_settings_file(app: &AppHandle) {
    let state = app.state::<AppState>();
    let s = state.settings.lock_or_recover().clone();
    let dir = std::path::PathBuf::from(&s.data_directory);
    let Ok(json) = serde_json::to_string_pretty(&s) else {
        log::error!("序列化设置失败");
        return;
    };
    if let Err(e) = write_settings_file(&dir, &json) {
        log::error!("保存设置失败: {e}");
        // 只在第一次失败时提醒：拖滑块会连续写盘，每次弹通知就是噪音。
        if !SETTINGS_SAVE_FAILED.swap(true, Ordering::Relaxed) {
            notify(
                app,
                &t!("设置保存失败"),
                &t!("改动未能写入磁盘，请检查数据目录权限与磁盘空间。"),
            );
        }
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
) -> Result<(), AppError> {
    // 热键组合合法性校验（解析失败或重复即拒绝）
    if settings.hotkeys_enabled {
        let main_hk: Shortcut = settings.hotkey_main.parse().map_err(|_| {
            AppError::new(
                "hotkey-invalid",
                format!("主界面热键无效: {}", settings.hotkey_main),
            )
        })?;
        let widget_hk: Shortcut = settings.hotkey_widget.parse().map_err(|_| {
            AppError::new(
                "hotkey-invalid",
                format!("悬浮窗热键无效: {}", settings.hotkey_widget),
            )
        })?;
        if main_hk == widget_hk {
            return Err(AppError::new(
                "hotkey-duplicate",
                "主界面与悬浮窗热键不能相同",
            ));
        }
    }
    let old_dir = state.settings.lock_or_recover().data_directory.clone();
    let dir_changed = old_dir != settings.data_directory;

    autostart::set(
        settings.start_with_windows,
        &std::env::current_exe()?.to_string_lossy(),
    )?;

    if dir_changed {
        std::fs::create_dir_all(&settings.data_directory)?;
        let (daily, intervals) = {
            let st = state.store.lock_or_recover();
            let daily = st
                .query_daily("0000-01-01", "9999-12-31")
                .map_err(|e| AppError::new("db", e.to_string()))?;
            let intervals = st
                .query_intervals("0000-01-01", "9999-12-31")
                .map_err(|e| AppError::new("db", e.to_string()))?;
            // 换库前把旧库的 WAL 落盘并截断，免得用户切回旧目录时看到一份
            // 落后若干个 30 秒周期的数据。
            let _ = st.checkpoint();
            (daily, intervals)
        };
        let new_store = SqliteStore::open(&db_path(&settings.data_directory))
            .map_err(|e| AppError::new("db", e.to_string()))?;
        new_store
            .save_daily_replace(&daily)
            .map_err(|e| AppError::new("db", e.to_string()))?;
        new_store
            .save_intervals(&intervals)
            .map_err(|e| AppError::new("db", e.to_string()))?;
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
) -> Result<(), AppError> {
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
        .map_err(|e| AppError::new("db", e.to_string()))?;
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
) -> Result<(), AppError> {
    if rules.is_empty() {
        return Err(AppError::new("rules-empty", "至少保留一条分类规则"));
    }
    for r in &rules {
        if r.name.trim().is_empty() || r.keywords.is_empty() {
            return Err(AppError::new("rules-empty", "分类名与关键词不能为空"));
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

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!(
            "tracker_settings_{tag}_{}_{}",
            std::process::id(),
            nanos
        ));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn save(dir: &std::path::Path, port: u16) {
        let s = TrackerSettings {
            api_port: port,
            ..Default::default()
        };
        let json = serde_json::to_string_pretty(&s).expect("设置应能序列化");
        write_settings_file(dir, &json).expect("写入应成功");
    }

    #[test]
    fn 设置往返读写() {
        let dir = temp_dir("roundtrip");
        let s = TrackerSettings {
            api_port: 60123,
            hotkey_main: "Ctrl+Shift+F9".to_string(),
            retention_days: 30,
            ..Default::default()
        };
        let json = serde_json::to_string_pretty(&s).expect("设置应能序列化");
        write_settings_file(&dir, &json).expect("写入应成功");

        let (loaded, event) = load_settings_from(&dir);
        assert_eq!(event, LoadEvent::Ok, "正常文件不该报损坏");
        assert_eq!(loaded.api_port, 60123, "端口应原样读回");
        assert_eq!(loaded.hotkey_main, "Ctrl+Shift+F9", "热键应原样读回");
        assert_eq!(loaded.retention_days, 30, "保留天数应原样读回");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn 主文件截断时从备份恢复() {
        let dir = temp_dir("recover");
        save(&dir, 60111);
        save(&dir, 60222);
        assert_eq!(
            load_settings_from(&dir).0.api_port,
            60222,
            "第二次写入应覆盖主文件"
        );
        assert!(
            !dir.join(SETTINGS_TMP).exists(),
            "rename 之后不该留下临时文件"
        );
        let backup = std::fs::read_to_string(dir.join(SETTINGS_BACKUP)).expect("应有备份");
        assert!(
            backup.contains("60111"),
            "备份里应是上一份完整设置，实际: {backup}"
        );

        // 模拟写到一半断电：主文件只剩半截 JSON。
        std::fs::write(dir.join(SETTINGS_FILE), "{\"api_port\": 60").expect("制造截断");

        let (loaded, event) = load_settings_from(&dir);
        assert_eq!(event, LoadEvent::RecoveredFromBackup, "应走备份恢复");
        assert_eq!(loaded.api_port, 60111, "应读回上一份完整设置");
        assert!(
            dir.join(SETTINGS_CORRUPT).exists(),
            "损坏的文件应被改名保留"
        );
        assert_eq!(
            load_settings_from(&dir).1,
            LoadEvent::Ok,
            "恢复后主文件应已重写，下次启动不再报损坏"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn 主文件与备份都损坏时回到默认() {
        let dir = temp_dir("reset");
        std::fs::create_dir_all(&dir).expect("建目录");
        std::fs::write(dir.join(SETTINGS_FILE), "not json").expect("写坏主文件");
        std::fs::write(dir.join(SETTINGS_BACKUP), "still not json").expect("写坏备份");

        let (loaded, event) = load_settings_from(&dir);
        assert_eq!(event, LoadEvent::ResetToDefault, "两份都坏时应回到默认");
        assert_eq!(
            loaded.api_port,
            TrackerSettings::default().api_port,
            "应取默认端口"
        );
        assert!(dir.join(SETTINGS_CORRUPT).exists(), "损坏的文件应留下证据");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
