mod api_server;
mod autostart;
mod ext_install;
mod monitor;
mod tray;
mod update;
mod version_info;
mod windows;

rust_i18n::i18n!("locales", fallback = "zh-CN");
use rust_i18n::t;

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use chrono::Local;
use serde::{Deserialize, Serialize};
use tauri::menu::{CheckMenuItem, ContextMenu, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::TrayIcon;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use tracker_core::aggregator::UsageAggregator;
use tracker_core::category::{CategoryResolver, UNCATEGORIZED};
use tracker_core::models::{
    AppIdentity, DailyUsage, ForegroundInterval, StateInterval, TitleInterval, TrackingState,
    WebInterval,
};
use tracker_core::settings::{CategoryRuleDef, TrackerSettings, WidgetPrefs};
use tracker_core::store::SqliteStore;

/// 全局应用状态（Tauri managed state）。
struct AppState {
    settings: Mutex<TrackerSettings>,
    aggregator: Mutex<UsageAggregator>,
    store: Mutex<SqliteStore>,
    user_paused: AtomicBool,
    notified_limits: Mutex<HashSet<String>>,
    tray: Mutex<Option<tray::TrayHandles>>,
    tray_icon: Mutex<Option<TrayIcon<tauri::Wry>>>,
    /// 悬浮窗展开态锚点（物理坐标），贴边收起/展开围绕它，避免循环漂移。
    widget_anchor: Mutex<(i32, i32)>,
    /// 悬浮窗尺寸动画进行中（动画期间的 Moved 不回写锚点）。
    widget_anim: AtomicBool,
    /// 本地 API 上次生效配置（enabled, port）。
    api_last: Mutex<Option<(bool, u16)>>,
    /// 浏览器心跳当前打开段（domain, title, start）。
    web_open: Mutex<Option<(String, String, chrono::DateTime<Local>)>>,
    /// 浏览器心跳待写入明细。
    web_pending: Mutex<Vec<WebInterval>>,
    /// 分类解析器缓存（save_settings 时重建，避免每秒克隆规则）。
    resolver: Mutex<std::sync::Arc<CategoryResolver>>,
}

// ---------- 悬浮窗 / 主窗口实时状态 ----------

#[derive(Serialize, Clone)]
struct LiveTopApp {
    display_name: String,
    category: String,
    active_seconds: f64,
}

#[derive(Serialize, Clone)]
struct LiveStatus {
    state: TrackingState,
    current_app: Option<AppIdentity>,
    current_category: Option<String>,
    current_seconds: f64,
    today_total_seconds: f64,
    top_apps: Vec<LiveTopApp>,
}

// ---------- 仪表盘数据 ----------

#[derive(Serialize, Clone)]
struct DashboardApp {
    app_key: String,
    display_name: String,
    process_name: String,
    exe_path: String,
    category: String,
    active_seconds: f64,
    percent: f64,
}

#[derive(Serialize, Clone)]
struct DashboardCategory {
    name: String,
    seconds: f64,
    percent: f64,
}

#[derive(Serialize, Clone)]
struct DashboardTrendPoint {
    date: String,
    seconds: f64,
    /// 当日效率评分（0-100，仅当存在已标记性质的分类时才有值）
    score: Option<f64>,
}

#[derive(Serialize, Clone)]
struct DashboardData {
    from: String,
    to: String,
    total_active_seconds: f64,
    top_app: Option<String>,
    apps: Vec<DashboardApp>,
    categories: Vec<DashboardCategory>,
    trend: Vec<DashboardTrendPoint>,
}

// ---------- 入口 ----------

#[cfg_attr(mobile, tauri_mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            windows::show_main(app);
        }))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state() != ShortcutState::Pressed {
                        return;
                    }
                    // 每次触发时读当前设置（支持自定义组合）
                    let (main_hk_s, widget_hk_s) = {
                        let st = app.state::<AppState>();
                        let s = st.settings.lock().unwrap();
                        (s.hotkey_main.clone(), s.hotkey_widget.clone())
                    };
                    let main_hk: Shortcut = main_hk_s.parse().expect("valid shortcut");
                    let widget_hk: Shortcut = widget_hk_s.parse().expect("valid shortcut");
                    if shortcut == &main_hk {
                        windows::toggle_main(app);
                    } else if shortcut == &widget_hk {
                        let st = app.state::<AppState>();
                        let visible = app
                            .get_webview_window("widget")
                            .map(|w| w.is_visible().unwrap_or(false))
                            .unwrap_or(false);
                        set_widget_visible(app, !visible);
                        let guard = st.tray.lock().unwrap();
                        if let Some(h) = guard.as_ref() {
                            let _ = h.widget_toggle.set_checked(!visible);
                        }
                    }
                })
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            get_dashboard,
            get_live_status,
            get_settings,
            save_settings,
            set_paused,
            set_app_category,
            save_category_rules,
            export_csv,
            clear_all_data,
            list_available_categories,
            open_path,
            get_app_version,
            show_widget_menu,
            set_widget_collapsed,
            update_widget_settings,
            toggle_widget,
            close_widget,
            get_timeline,
            windows::toggle_main_window,
            get_app_detail,
            exclude_app,
            set_app_display_name,
            update::check_update,
            update::install_update,
            get_top_titles,
            get_top_web,
            ext_install::get_extension_path,
            ext_install::open_browser_ext_page,
            ext_install::smart_install_extension,
            db_maintenance,
            delete_backup,
            export_weekly_report,
            scan_installed_apps,
            export_backup_json,
            import_backup_json,
            list_backups,
            open_backup_dir,
            restore_backup_named,
            restore_backup_content,
        ])
        .setup(|app| {
            let settings = load_settings();
            std::fs::create_dir_all(&settings.data_directory).map_err(|e| e.to_string())?;
            let store = SqliteStore::open(&db_path(&settings.data_directory))
                .map_err(|e| format!("打开数据库失败: {e}"))?;

            // 排除名单 = 用户配置 + 自身（始终排除）
            let mut excluded: HashSet<String> =
                settings.excluded_app_keys.iter().cloned().collect();
            if let Ok(self_exe) = std::env::current_exe() {
                excluded.insert(self_exe.to_string_lossy().to_lowercase());
            }

            // 启动时把今日已有记录载入内存，保证后续绝对覆盖式 flush 不丢数据
            let today = Local::now().format("%Y-%m-%d").to_string();
            let mut agg = UsageAggregator::new(settings.idle_threshold_seconds as f64, excluded);
            let rows = store
                .query_daily(&today, &today)
                .map_err(|e| e.to_string())?;
            for r in rows {
                // 剔除修复前误记的自身历史行（内存 + 库）
                if r.process_name.to_lowercase() == *monitor::self_stem() {
                    let _ = store.delete_app_rows(&r.app_key);
                    continue;
                }
                agg.daily
                    .insert(format!("{}|{}", r.usage_date, r.app_key), r);
            }

            let resolver = std::sync::Arc::new(CategoryResolver::new(
                settings.category_rules.clone(),
                settings.app_category_overrides.clone(),
            ));
            app.manage(AppState {
                settings: Mutex::new(settings),
                aggregator: Mutex::new(agg),
                store: Mutex::new(store),
                user_paused: AtomicBool::new(false),
                notified_limits: Mutex::new(HashSet::new()),
                tray: Mutex::new(None),
                tray_icon: Mutex::new(None),
                widget_anchor: Mutex::new((12, 12)),
                widget_anim: AtomicBool::new(false),
                api_last: Mutex::new(None),
                web_open: Mutex::new(None),
                web_pending: Mutex::new(Vec::new()),
                resolver: Mutex::new(resolver),
            });

            let handle = app.handle().clone();
            // 老版本写进 HKCU Run 的是裸 exe 路径（无引号、无标记）。这里按当前
            // 设置再写一次，把历史值就地规范化成 "exe" --autostart 的形式。
            if handle
                .state::<AppState>()
                .settings
                .lock()
                .unwrap()
                .start_with_windows
            {
                if let Ok(exe) = std::env::current_exe() {
                    let _ = autostart::set(true, &exe.to_string_lossy());
                }
            }
            apply_locale(&handle);
            tray::build_tray(&handle)?;
            apply_hotkeys(&handle);
            windows::setup_main_window(&handle);
            // 主窗口在 tauri.conf.json 里是 visible:false，此前只有二次启动
            // （single_instance 回调）或托盘点击才会点亮它：开过悬浮窗再重启、
            // 或从托盘隐藏后重启时，主界面根本不出现。这里补一次显式显示。
            // 例外：--smoke 是无界面监控模式，--autostart 是开机自启 —— 后者
            // 应安安静静常驻托盘与悬浮窗，不该每次开机都弹一个窗口出来。
            let args: Vec<String> = std::env::args().collect();
            let headless = args
                .iter()
                .any(|a| a == "--smoke" || a == autostart::AUTOSTART_FLAG);
            if !headless {
                windows::show_main(&handle);
            }

            let show_widget = {
                let st = handle.state::<AppState>();
                let v = st.settings.lock().unwrap().show_floating_widget;
                v
            };
            if show_widget {
                windows::create_widget_window(&handle)?;
                windows::setup_widget_clamp(&handle);
                let _ = handle.emit("widget://visibility", true);
                if handle
                    .state::<AppState>()
                    .settings
                    .lock()
                    .unwrap()
                    .widget
                    .edge_snap
                {
                    if let Some(w) = handle.get_webview_window("widget") {
                        windows::collapse_widget(&handle, &w);
                    }
                }
            }

            monitor::purge_expired(&handle);
            api_server::spawn_api_server(&handle);

            // --smoke N：无界面监控 N 秒后落库、写摘要并退出（供自动化验证）
            if let Some(secs) = std::env::args()
                .nth(
                    std::env::args()
                        .position(|a| a == "--smoke")
                        .map(|i| i + 1)
                        .unwrap_or(usize::MAX),
                )
                .and_then(|v| v.parse::<u64>().ok())
            {
                let h = handle.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_secs(secs.max(1)));
                    monitor::flush_state(&h);
                    let st = h.state::<AppState>();
                    let _ = st.store.lock().unwrap().checkpoint();
                    let (total, apps) = {
                        let agg = st.aggregator.lock().unwrap();
                        let today = Local::now().format("%Y-%m-%d").to_string();
                        let rows: Vec<&DailyUsage> = agg
                            .daily
                            .values()
                            .filter(|d| d.usage_date == today)
                            .collect();
                        (
                            rows.iter().map(|d| d.active_seconds).sum::<f64>(),
                            rows.len(),
                        )
                    };
                    let report = serde_json::json!({
                        "mode": "smoke",
                        "seconds": secs,
                        "today_total_seconds": total,
                        "apps": apps,
                        "healthy": total > 0.0 && apps > 0,
                    });
                    let dir = {
                        let s = st.settings.lock().unwrap();
                        s.data_directory.clone()
                    };
                    let path = std::path::Path::new(&dir).join("smoke-report.json");
                    let _ = std::fs::write(&path, serde_json::to_string_pretty(&report).unwrap());
                    println!("smoke-report: {}", path.display());
                    if report["healthy"].as_bool().unwrap_or(false) {
                        h.exit(0);
                    } else {
                        h.exit(3);
                    }
                });
            }
            monitor::spawn_monitor(handle.clone());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// 全局热键注册（设置开关）。
fn apply_hotkeys(app: &AppHandle) {
    let (enabled, main_hk_s, widget_hk_s) = {
        let st = app.state::<AppState>();
        let s = st.settings.lock().unwrap();
        (
            s.hotkeys_enabled,
            s.hotkey_main.clone(),
            s.hotkey_widget.clone(),
        )
    };
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();
    if !enabled {
        return;
    }
    if let Ok(hk) = main_hk_s.parse::<Shortcut>() {
        let _ = gs.register(hk);
    }
    if let Ok(hk) = widget_hk_s.parse::<Shortcut>() {
        let _ = gs.register(hk);
    }
}

// ---------- 实时状态（悬浮窗 / 主窗口） ----------

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

// ---------- 设置 ----------

/// 应用别名：仅影响展示，不改历史数据与分类匹配。
fn display_alias(settings: &TrackerSettings, app_key: &str, original: &str) -> String {
    settings
        .app_display_names
        .get(app_key)
        .cloned()
        .unwrap_or_else(|| original.to_string())
}

/// 解析界面语言（system = 中文）。
fn resolve_locale(language: &str) -> &'static str {
    if language.starts_with("en") {
        "en-US"
    } else {
        "zh-CN"
    }
}

fn apply_locale(app: &AppHandle) {
    let lang = {
        let st = app.state::<AppState>();
        let v = st.settings.lock().unwrap().language.clone();
        v
    };
    rust_i18n::set_locale(resolve_locale(&lang));
}

fn db_path(data_dir: &str) -> String {
    std::path::Path::new(data_dir)
        .join("usage.db")
        .to_string_lossy()
        .to_string()
}

fn load_settings() -> TrackerSettings {
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
fn migrate_settings(s: &mut TrackerSettings) {
    if s.schema_version < 1 {
        s.schema_version = 1;
    }
}

fn save_settings_file(app: &AppHandle) {
    let state = app.state::<AppState>();
    let s = state.settings.lock().unwrap().clone();
    let _ = std::fs::create_dir_all(&s.data_directory);
    let path = std::path::Path::new(&s.data_directory).join("settings.json");
    if let Ok(json) = serde_json::to_string_pretty(&s) {
        let _ = std::fs::write(path, json);
    }
}

fn set_widget_visible(app: &AppHandle, visible: bool) {
    if visible {
        if app.get_webview_window("widget").is_none() {
            if let Err(e) = windows::create_widget_window(app) {
                eprintln!("create widget failed: {e}");
            }
            windows::setup_widget_clamp(app);
        } else if let Some(w) = app.get_webview_window("widget") {
            let _ = w.show();
        }
    } else if let Some(w) = app.get_webview_window("widget") {
        let _ = w.hide();
    }
    {
        let state = app.state::<AppState>();
        state.settings.lock().unwrap().show_floating_widget = visible;
    }
    save_settings_file(app);
    let _ = app.emit("widget://visibility", visible);
}

fn set_paused_internal(app: &AppHandle, paused: bool) {
    let state = app.state::<AppState>();
    state.user_paused.store(paused, Ordering::Relaxed);
    let mut agg = state.aggregator.lock().unwrap();
    let now = Local::now();
    if paused {
        agg.pause(now);
    } else {
        let mut cache = HashMap::new();
        agg.resume(monitor::foreground_identity(&mut cache), now);
    }
}

// ---------- Tauri Commands ----------

#[tauri::command]
fn get_live_status(app: AppHandle) -> LiveStatus {
    live_status(&app)
}

/// 悬浮窗右键 → 弹出原生上下文菜单（带 ✓ 与子菜单，Win11 玻璃样式）。
#[tauri::command]
fn show_widget_menu(app: AppHandle) -> Result<(), String> {
    let window = app.get_window("widget").ok_or("悬浮窗未创建")?;
    let prefs = {
        app.state::<AppState>()
            .settings
            .lock()
            .unwrap()
            .widget
            .clone()
    };
    let check = |id: &str, label: &str, checked: bool| {
        CheckMenuItem::with_id(&app, id, label, true, checked, None::<&str>)
            .map_err(|e| e.to_string())
    };
    let top_i = check("w.top", &t!("置顶显示"), prefs.always_on_top)?;
    let lock_i = check("w.lock", &t!("锁定位置"), prefs.locked)?;
    let snap_i = check("w.snap", &t!("贴边收起"), prefs.edge_snap)?;
    let compact_i = check("w.compact", &t!("缩略模式"), prefs.compact)?;

    let op_menu =
        Submenu::with_id(&app, "w.opacity", &t!("透明度"), true).map_err(|e| e.to_string())?;
    for (v, label) in [
        (85u8, &t!("高 85%")),
        (70, &t!("中 70%")),
        (55, &t!("低 55%")),
        (40, &t!("低 40%")),
    ] {
        let item = check(&format!("w.op.{v}"), label, prefs.opacity == v)?;
        op_menu.append(&item).map_err(|e| e.to_string())?;
    }
    let settings_i = MenuItem::with_id(&app, "w.settings", &t!("设置…"), true, None::<&str>)
        .map_err(|e| e.to_string())?;
    let sep = PredefinedMenuItem::separator(&app).map_err(|e| e.to_string())?;
    let menu = Menu::with_items(
        &app,
        &[
            &top_i,
            &lock_i,
            &snap_i,
            &compact_i,
            &op_menu,
            &sep,
            &settings_i,
        ],
    )
    .map_err(|e| e.to_string())?;
    menu.popup(window).map_err(|e| e.to_string())
}

/// 主界面顶栏按钮：切换悬浮窗显示，返回切换后的可见状态。
#[tauri::command]
fn toggle_widget(app: AppHandle) -> bool {
    let visible = app
        .get_webview_window("widget")
        .map(|w| w.is_visible().unwrap_or(false))
        .unwrap_or(false);
    let next = !visible;
    set_widget_visible(&app, next);
    if let Some(h) = app.state::<AppState>().tray.lock().unwrap().as_ref() {
        let _ = h.widget_toggle.set_checked(next);
    }
    next
}

/// 悬浮窗 ✕ 按钮：关闭（隐藏）悬浮窗并写回设置、同步托盘勾选。
#[tauri::command]
fn close_widget(app: AppHandle) {
    set_widget_visible(&app, false);
    if let Some(h) = app.state::<AppState>().tray.lock().unwrap().as_ref() {
        let _ = h.widget_toggle.set_checked(false);
    }
}

/// 贴边收起模式下，鼠标悬停展开 / 离开收起。
#[tauri::command]
fn set_widget_collapsed(app: AppHandle, collapsed: bool) {
    let prefs = {
        app.state::<AppState>()
            .settings
            .lock()
            .unwrap()
            .widget
            .clone()
    };
    if let Some(win) = app.get_webview_window("widget") {
        if collapsed && prefs.edge_snap {
            windows::collapse_widget(&app, &win);
        } else if !prefs.edge_snap || !collapsed {
            windows::expand_widget(&app, &win, prefs.compact);
        }
    }
}

/// 前端直接更新悬浮窗偏好（预留）。
#[tauri::command]
fn update_widget_settings(app: AppHandle, widget: WidgetPrefs) -> Result<(), String> {
    windows::apply_widget_prefs(&app, &widget);
    Ok(())
}

#[tauri::command]
fn get_dashboard(app: AppHandle, from: String, to: String) -> Result<DashboardData, String> {
    build_dashboard(&app, &from, &to)
}

fn build_dashboard(app: &AppHandle, from: &str, to: &str) -> Result<DashboardData, String> {
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

#[tauri::command]
fn get_settings(state: State<AppState>) -> TrackerSettings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
fn save_settings(
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
    let old_dir = state.settings.lock().unwrap().data_directory.clone();
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
            let st = state.store.lock().unwrap();
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
        *state.store.lock().unwrap() = new_store;
    }

    {
        let mut excluded: HashSet<String> = settings.excluded_app_keys.iter().cloned().collect();
        if let Ok(self_exe) = std::env::current_exe() {
            excluded.insert(self_exe.to_string_lossy().to_lowercase());
        }
        let mut agg = state.aggregator.lock().unwrap();
        agg.apply_settings(settings.idle_threshold_seconds as f64, excluded);
    }

    *state.settings.lock().unwrap() = settings.clone();
    save_settings_file(&app);
    // 先重建 resolver 缓存，再通知前端，避免前端刷新时仍读到旧分类
    rebuild_resolver(&app);
    let _ = app.emit("app://settings", &settings);
    apply_locale(&app);
    apply_hotkeys(&app);
    api_server::spawn_api_server(&app);

    // 悬浮窗可见性同步
    let want = state.settings.lock().unwrap().show_floating_widget;
    let now_visible = app
        .get_webview_window("widget")
        .map(|w| w.is_visible().unwrap_or(false))
        .unwrap_or(false);
    if want != now_visible {
        set_widget_visible(&app, want);
    }
    if let Some(h) = state.tray.lock().unwrap().as_ref() {
        let _ = h.widget_toggle.set_checked(want);
    }
    Ok(())
}

#[tauri::command]
fn set_paused(app: AppHandle, state: State<AppState>, paused: bool) {
    set_paused_internal(&app, paused);
    if let Some(h) = state.tray.lock().unwrap().as_ref() {
        let _ = h.pause_toggle.set_checked(paused);
    }
}

/// 设置应用别名（空 = 恢复默认名）。
#[tauri::command]
fn set_app_display_name(app: AppHandle, state: State<AppState>, app_key: String, name: String) {
    {
        let mut s = state.settings.lock().unwrap();
        let trimmed = name.trim().to_string();
        if trimmed.is_empty() {
            s.app_display_names.remove(&app_key);
        } else {
            s.app_display_names.insert(app_key, trimmed);
        }
    }
    save_settings_file(&app);
    let settings = state.settings.lock().unwrap().clone();
    let _ = app.emit("app://settings", &settings);
}

/// 排除应用：停止统计 + 清除其全部已有记录。
#[tauri::command]
fn exclude_app(app: AppHandle, state: State<AppState>, app_key: String) -> Result<(), String> {
    {
        let mut s = state.settings.lock().unwrap();
        if !s.excluded_app_keys.contains(&app_key) {
            s.excluded_app_keys.push(app_key.clone());
        }
    }
    save_settings_file(&app);
    {
        let (idle, excluded_list) = {
            let s = state.settings.lock().unwrap();
            (s.idle_threshold_seconds as f64, s.excluded_app_keys.clone())
        };
        let mut excluded: HashSet<String> = excluded_list.into_iter().collect();
        if let Ok(self_exe) = std::env::current_exe() {
            excluded.insert(self_exe.to_string_lossy().to_lowercase());
        }
        let mut agg = state.aggregator.lock().unwrap();
        agg.apply_settings(idle, excluded);
        agg.daily.retain(|_, v| v.app_key != app_key);
        agg.pending_intervals.retain(|v| v.app_key != app_key);
        agg.pending_title_intervals.retain(|v| v.app_key != app_key);
    }
    state
        .store
        .lock()
        .unwrap()
        .delete_app_rows(&app_key)
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// 分类规则/覆盖变化 → 重建 resolver 缓存（仪表盘、时间线、实时状态都读它）。
fn rebuild_resolver(app: &AppHandle) {
    let state = app.state::<AppState>();
    let (rules, overrides) = {
        let s = state.settings.lock().unwrap();
        (s.category_rules.clone(), s.app_category_overrides.clone())
    };
    *state.resolver.lock().unwrap() = std::sync::Arc::new(CategoryResolver::new(rules, overrides));
}

#[tauri::command]
fn set_app_category(app: AppHandle, state: State<AppState>, app_key: String, category: String) {
    {
        let mut s = state.settings.lock().unwrap();
        if category == UNCATEGORIZED {
            s.app_category_overrides.remove(&app_key);
        } else {
            s.app_category_overrides.insert(app_key, category);
        }
    }
    save_settings_file(&app);
    rebuild_resolver(&app);
    let settings = state.settings.lock().unwrap().clone();
    let _ = app.emit("app://settings", &settings);
}

#[tauri::command]
fn save_category_rules(
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
        let mut s = state.settings.lock().unwrap();
        s.category_rules = rules;
    }
    save_settings_file(&app);
    rebuild_resolver(&app);
    let settings = state.settings.lock().unwrap().clone();
    let _ = app.emit("app://settings", &settings);
    Ok(())
}

#[tauri::command]
fn list_available_categories(state: State<AppState>) -> Vec<String> {
    let s = state.settings.lock().unwrap();
    let mut cats: Vec<String> = s.category_rules.iter().map(|r| r.name.clone()).collect();
    cats.push(UNCATEGORIZED.to_string());
    cats
}

#[tauri::command]
fn clear_all_data(app: AppHandle, state: State<AppState>) -> Result<(), String> {
    {
        let st = state.store.lock().unwrap();
        st.clear_all().map_err(|e| e.to_string())?;
    }
    {
        let mut agg = state.aggregator.lock().unwrap();
        agg.daily.clear();
        agg.pending_intervals.clear();
        agg.pending_state_intervals.clear();
        agg.pending_title_intervals.clear();
    }
    {
        state.web_open.lock().unwrap().take();
        state.web_pending.lock().unwrap().clear();
    }
    state.notified_limits.lock().unwrap().clear();
    let _ = app.emit("app://data", ());
    Ok(())
}

#[tauri::command]
fn export_csv(app: AppHandle, from: String, to: String) -> Result<String, String> {
    let data = build_dashboard(&app, &from, &to)?;
    let dir = dirs::download_dir().unwrap_or_else(std::env::temp_dir);
    let file = dir.join(format!("使用时长_{from}_至_{to}.csv"));
    let mut out = String::with_capacity(4096);
    out.push('\u{FEFF}');
    out.push_str("应用,分类,活跃时长,活跃秒数\r\n");
    for a in &data.apps {
        out.push_str(&format!(
            "{},{},{},{:.0}\r\n",
            csv_escape(&a.display_name),
            csv_escape(&a.category),
            fmt_hms(a.active_seconds),
            a.active_seconds
        ));
    }
    std::fs::write(&file, out).map_err(|e| e.to_string())?;
    Ok(file.to_string_lossy().to_string())
}

#[tauri::command]
fn open_path(path: String) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let trimmed = path.trim_end_matches(['\\', '/']);
        let quoted = format!("\"{trimmed}\"");
        let is_dir = std::path::Path::new(&path).is_dir();
        let arg = if is_dir {
            quoted
        } else {
            format!("/select,{quoted}")
        };
        std::process::Command::new("explorer")
            .raw_arg(arg)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(not(windows))]
    let _ = path;
    Ok(())
}

#[tauri::command]
fn get_app_version(app: AppHandle) -> String {
    app.package_info().version.to_string()
}

// ---------- 应用详情 ----------

#[derive(Serialize, Clone)]
struct AppDetail {
    app_key: String,
    display_name: String,
    exe_path: String,
    category: String,
    total_seconds: f64,
    daily: Vec<DashboardTrendPoint>,
}

#[tauri::command]
fn get_app_detail(
    app: AppHandle,
    app_key: String,
    from: String,
    to: String,
) -> Result<AppDetail, String> {
    let state = app.state::<AppState>();
    let resolver = app.state::<AppState>().resolver.lock().unwrap().clone();

    let store_rows = {
        let st = state.store.lock().unwrap();
        st.query_daily(&from, &to).map_err(|e| e.to_string())?
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
        let agg = state.aggregator.lock().unwrap();
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
        let s = state.settings.lock().unwrap();
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
struct ScannedApp {
    app_key: String,
    display_name: String,
    category: String,
    source: String,
}

#[tauri::command]
fn scan_installed_apps(app: AppHandle) -> Result<Vec<ScannedApp>, String> {
    #[cfg(windows)]
    {
        let resolver = app.state::<AppState>().resolver.lock().unwrap().clone();

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
fn collect_lnks(dir: &std::path::Path, cb: &mut dyn FnMut(&std::path::PathBuf, String)) {
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

// ---------- 时间线 ----------

#[derive(Serialize, Clone)]
struct TimelineSegment {
    app_key: String,
    display_name: String,
    category: String,
    start_time: String,
    end_time: String,
    duration_seconds: f64,
}

#[derive(Serialize, Clone)]
struct TimelineState {
    state: String,
    start_time: String,
    end_time: String,
    duration_seconds: f64,
}

#[derive(Serialize, Clone)]
struct TimelineData {
    date: String,
    segments: Vec<TimelineSegment>,
    states: Vec<TimelineState>,
}

#[tauri::command]
fn get_timeline(app: AppHandle, date: String) -> Result<TimelineData, String> {
    timeline_data(&app, &date)
}

fn timeline_data(app: &AppHandle, date: &str) -> Result<TimelineData, String> {
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

// ---------- 数据库维护 ----------

#[derive(Serialize, Clone)]
struct DbMaintenance {
    integrity: String,
    vacuum_ok: bool,
    page_count: i64,
    freelist: i64,
    page_size: i64,
    /// VACUUM 之后又把 WAL 截断了一次：-wal 在没有它的情况下可能比主库还大。
    checkpoint_ok: bool,
    /// VACUUM + checkpoint 之后的主库字节数，用于在设置页给出可对比的数字。
    db_bytes: i64,
}

#[tauri::command]
fn db_maintenance(state: State<AppState>) -> Result<DbMaintenance, String> {
    let st = state.store.lock().unwrap();
    let integrity: String = st
        .pragma("PRAGMA integrity_check")
        .map_err(|e| e.to_string())?;
    let vacuum_ok = st.vacuum().is_ok();
    // VACUUM 会把 WAL 里残留的页一并回写，紧接着做一次 truncate 模式的
    // checkpoint 才能真正把 -wal 收回 1 MiB 以内（journal_size_limit 见 store::open）。
    let checkpoint_ok = st.checkpoint().is_ok();
    let (page_size, page_count, freelist) = st.db_stats().map_err(|e| e.to_string())?;
    Ok(DbMaintenance {
        integrity,
        vacuum_ok,
        page_count,
        freelist,
        page_size,
        checkpoint_ok,
        db_bytes: page_size * page_count,
    })
}

// ---------- 周报 ----------

#[tauri::command]
fn export_weekly_report(app: AppHandle) -> Result<String, String> {
    let to = Local::now().format("%Y-%m-%d").to_string();
    let from = (Local::now() - chrono::Duration::days(6))
        .format("%Y-%m-%d")
        .to_string();
    let data = build_dashboard(&app, &from, &to)?;

    let mut md = String::new();
    md.push_str(&format!("# 使用周报（{from} ~ {to}）\n\n"));
    md.push_str(&format!(
        "- 总活跃：**{}**\n- 日均：**{}**\n- 应用数：{}\n\n",
        fmt_hms(data.total_active_seconds),
        fmt_hms(data.total_active_seconds / 7.0),
        data.apps.len()
    ));
    md.push_str("## 分类占比\n\n| 分类 | 时长 | 占比 |\n|---|---:|---:|\n");
    for c in &data.categories {
        md.push_str(&format!(
            "| {} | {} | {:.1}% |\n",
            c.name,
            fmt_hms(c.seconds),
            c.percent
        ));
    }
    md.push_str("\n## 应用 Top 15\n\n| # | 应用 | 分类 | 时长 | 占比 |\n|---|---|---|---:|---:|\n");
    for (i, a) in data.apps.iter().take(15).enumerate() {
        md.push_str(&format!(
            "| {} | {} | {} | {} | {:.1}% |\n",
            i + 1,
            a.display_name,
            a.category,
            fmt_hms(a.active_seconds),
            a.percent
        ));
    }
    md.push_str("\n## 每日活跃\n\n| 日期 | 时长 |\n|---|---:|\n");
    for t in &data.trend {
        md.push_str(&format!("| {} | {} |\n", t.date, fmt_hms(t.seconds)));
    }

    let dir = dirs::download_dir().unwrap_or_else(std::env::temp_dir);
    let file = dir.join(format!("使用周报_{from}_{to}.md"));
    std::fs::write(&file, md).map_err(|e| e.to_string())?;
    Ok(file.to_string_lossy().to_string())
}

// ---------- Top 标题 ----------

#[derive(Serialize, Clone)]
struct TopTitle {
    app_key: String,
    display_name: String,
    category: String,
    title: String,
    seconds: f64,
}

#[tauri::command]
fn get_top_titles(
    app: AppHandle,
    from: String,
    to: String,
    limit: i64,
) -> Result<Vec<TopTitle>, String> {
    let state = app.state::<AppState>();
    let resolver = app.state::<AppState>().resolver.lock().unwrap().clone();
    let aliases_map = state.settings.lock().unwrap().app_display_names.clone();
    let rows = {
        let st = state.store.lock().unwrap();
        st.top_titles(&from, &to, limit.clamp(1, 200))
            .map_err(|e| e.to_string())?
    };
    let mut out: Vec<TopTitle> = Vec::new();
    for (app_key, title, seconds) in rows {
        // 行所属应用可能已被排除/删除——标题表同步清理过，直接呈现即可
        let (display, _cat) = {
            let st = state.store.lock().unwrap();
            match st.query_daily(&from, &to).ok() {
                Some(rows) => {
                    let hit = rows.iter().find(|r| r.app_key == app_key);
                    match hit {
                        Some(r) => (r.display_name.clone(), ()),
                        None => (String::new(), ()),
                    }
                }
                None => (String::new(), ()),
            }
        };
        let display = if display.is_empty() {
            app_key
                .rsplit(['\\', '/'])
                .next()
                .unwrap_or(&app_key)
                .trim_end_matches(".exe")
                .to_string()
        } else {
            display
        };
        let display = aliases_map.get(&app_key).cloned().unwrap_or(display);
        let category = resolver.resolve(&display, &display, &app_key);
        out.push(TopTitle {
            app_key,
            display_name: display,
            category,
            title,
            seconds,
        });
    }
    Ok(out)
}

// ---------- Top 网页 ----------

#[derive(Serialize, Clone)]
struct TopWeb {
    domain: String,
    title: String,
    seconds: f64,
}

#[tauri::command]
fn get_top_web(
    app: AppHandle,
    from: String,
    to: String,
    limit: i64,
) -> Result<Vec<TopWeb>, String> {
    let state = app.state::<AppState>();
    let rows = {
        let st = state.store.lock().unwrap();
        st.top_web(&from, &to, limit.clamp(1, 200))
            .map_err(|e| e.to_string())?
    };
    Ok(rows
        .into_iter()
        .map(|(domain, title, seconds)| TopWeb {
            domain,
            title,
            seconds,
        })
        .collect())
}

// ---------- JSON 备份 ----------

/// 当前备份格式版本；v2 起包含窗口标题与网页区间，旧备份仍可恢复（缺省为空）。
const BACKUP_VERSION: u32 = 2;

#[derive(Serialize, Deserialize)]
struct BackupFile {
    #[serde(default)]
    version: u32,
    #[serde(default)]
    exported_at: String,
    daily: Vec<DailyUsage>,
    #[serde(default)]
    intervals: Vec<ForegroundInterval>,
    #[serde(default)]
    states: Vec<StateInterval>,
    #[serde(default)]
    titles: Vec<TitleInterval>,
    #[serde(default)]
    web: Vec<WebInterval>,
}

fn backups_dir(data_dir: &str) -> std::path::PathBuf {
    std::path::Path::new(data_dir).join("backups")
}

#[tauri::command]
fn export_backup_json(app: AppHandle) -> Result<String, String> {
    let state = app.state::<AppState>();
    let data_dir = state.settings.lock().unwrap().data_directory.clone();
    let (daily, intervals, states, titles, web) = {
        let st = state.store.lock().unwrap();
        (
            st.query_daily("0000-01-01", "9999-12-31")
                .map_err(|e| e.to_string())?,
            st.query_intervals("0000-01-01", "9999-12-31")
                .map_err(|e| e.to_string())?,
            st.query_state_intervals("0000-01-01", "9999-12-31")
                .map_err(|e| e.to_string())?,
            st.query_title_intervals("0000-01-01", "9999-12-31")
                .map_err(|e| e.to_string())?,
            st.query_web_intervals("0000-01-01", "9999-12-31")
                .map_err(|e| e.to_string())?,
        )
    };
    let dir = backups_dir(&data_dir);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!(
        "backup_{}.json",
        Local::now().format("%Y%m%d_%H%M%S")
    ));
    let file = BackupFile {
        version: BACKUP_VERSION,
        exported_at: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        daily,
        intervals,
        states,
        titles,
        web,
    };
    std::fs::write(
        &path,
        serde_json::to_string(&file).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().to_string())
}

/// 解析备份名到 backups 目录内的 .json 文件，拒绝路径逃逸与非法文件名。
fn backup_path(data_dir: &str, name: &str) -> Result<std::path::PathBuf, String> {
    if name.is_empty()
        || name.contains("..")
        || name.contains('/')
        || name.contains('\\')
        || name.contains(':')
        || !name.to_lowercase().ends_with(".json")
    {
        return Err("非法备份名".into());
    }
    let dir = backups_dir(data_dir);
    let path = dir.join(name);
    if path.parent() != Some(dir.as_path()) {
        return Err("非法备份名".into());
    }
    Ok(path)
}

/// 删除指定备份文件（防路径逃逸：仅允许 backups 目录内的 json）。
#[tauri::command]
fn delete_backup(app: AppHandle, name: String) -> Result<(), String> {
    let state = app.state::<AppState>();
    let data_dir = state.settings.lock().unwrap().data_directory.clone();
    let path = backup_path(&data_dir, &name)?;
    std::fs::remove_file(&path).map_err(|e| e.to_string())
}

/// 共用恢复逻辑：清空当前库与内存态 → 写入备份 → 通知所有视图重新拉数据。
fn apply_backup(app: &AppHandle, text: &str, label: &str) -> Result<String, String> {
    let backup: BackupFile =
        serde_json::from_str(text).map_err(|e| format!("备份文件不是有效的应用备份（{e}）"))?;
    if backup.version > BACKUP_VERSION {
        return Err(format!(
            "备份文件版本 {} 高于当前支持的版本 {}，请升级应用后再恢复",
            backup.version, BACKUP_VERSION
        ));
    }
    let state = app.state::<AppState>();
    // 先清内存态再写库：写库期间监控线程仍会 flush，若内存里还留着旧数据，
    // 会和刚恢复的行混在一起。清空后 flush 只会补上新的增量。
    {
        let mut agg = state.aggregator.lock().unwrap();
        agg.daily.clear();
        agg.pending_intervals.clear();
        agg.pending_state_intervals.clear();
        agg.pending_title_intervals.clear();
    }
    {
        state.web_open.lock().unwrap().take();
        state.web_pending.lock().unwrap().clear();
    }
    state.notified_limits.lock().unwrap().clear();
    {
        let st = state.store.lock().unwrap();
        // 五张表在同一个事务里清空并写回，避免上万条明细逐条隐式提交长时间占用 store 锁
        st.restore_replace(
            &backup.daily,
            &backup.intervals,
            &backup.states,
            &backup.titles,
            &backup.web,
        )
        .map_err(|e| e.to_string())?;
    }
    let _ = app.emit("app://data", ());
    Ok(format!(
        "已从 {} 恢复 {} 条日聚合 / {} 条明细",
        label,
        backup.daily.len(),
        backup.intervals.len()
    ))
}

/// 打开备份目录（不存在则先创建）。
#[tauri::command]
fn open_backup_dir(app: AppHandle) -> Result<(), String> {
    let data_dir = app
        .state::<AppState>()
        .settings
        .lock()
        .unwrap()
        .data_directory
        .clone();
    let dir = backups_dir(&data_dir);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    open_path(dir.to_string_lossy().to_string())
}

/// 恢复数据目录 backups/ 里指定的备份。
#[tauri::command]
fn restore_backup_named(app: AppHandle, name: String) -> Result<String, String> {
    let data_dir = app
        .state::<AppState>()
        .settings
        .lock()
        .unwrap()
        .data_directory
        .clone();
    let path = backup_path(&data_dir, &name)?;
    let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    apply_backup(&app, &text, &name)
}

/// 恢复从任意位置选择的备份 JSON（前端读取文件内容后传入）。
#[tauri::command]
fn restore_backup_content(app: AppHandle, content: String) -> Result<String, String> {
    apply_backup(&app, &content, "所选文件")
}

#[tauri::command]
fn import_backup_json(app: AppHandle) -> Result<String, String> {
    let data_dir = app
        .state::<AppState>()
        .settings
        .lock()
        .unwrap()
        .data_directory
        .clone();
    let dir = backups_dir(&data_dir);
    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(&dir)
        .map_err(|e| format!("没有可导入的备份: {e}"))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().map(|e| e == "json").unwrap_or(false))
        .collect();
    files.sort();
    let path = files.pop().ok_or("没有可导入的备份")?;
    let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let label = path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    apply_backup(&app, &text, &label)
}

#[tauri::command]
fn list_backups(app: AppHandle) -> Vec<String> {
    let state = app.state::<AppState>();
    let data_dir = state.settings.lock().unwrap().data_directory.clone();
    let Ok(entries) = std::fs::read_dir(backups_dir(&data_dir)) else {
        return vec![];
    };
    let mut names: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "json").unwrap_or(false))
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    names.sort();
    names.reverse();
    names
}

// ---------- 小工具 ----------

fn fmt_hms(seconds: f64) -> String {
    let total = seconds as i64;
    let h = total / 3600;
    let m = (total % 3600) / 60;
    let s = total % 60;
    if h > 0 {
        format!("{h}时{m}分")
    } else if m > 0 {
        format!("{m}分{s}秒")
    } else {
        format!("{s}秒")
    }
}

fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}
