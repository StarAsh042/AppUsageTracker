mod autostart;
mod version_info;

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use chrono::Local;
use serde::Serialize;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, State, WebviewUrl, WebviewWindow, WindowEvent};
use tauri_plugin_notification::NotificationExt;
use tracker_core::aggregator::UsageAggregator;
use tracker_core::category::{CategoryResolver, UNCATEGORIZED};
use tracker_core::limits::categories_to_notify;
use tracker_core::models::{AppIdentity, DailyUsage, ForegroundInterval, TrackingState};
use tracker_core::settings::{CategoryRuleDef, TrackerSettings};
use tracker_core::store::SqliteStore;

/// 全局应用状态（Tauri managed state）。
struct AppState {
    settings: Mutex<TrackerSettings>,
    aggregator: Mutex<UsageAggregator>,
    store: Mutex<SqliteStore>,
    user_paused: AtomicBool,
    notified_limits: Mutex<HashSet<String>>,
    tray: Mutex<Option<TrayHandles>>,
}

struct TrayHandles {
    widget_toggle: CheckMenuItem<tauri::Wry>,
    pause_toggle: CheckMenuItem<tauri::Wry>,
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
            show_main(app);
        }))
        .plugin(tauri_plugin_notification::init())
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
        ])
        .setup(|app| {
            let settings = load_settings();
            std::fs::create_dir_all(&settings.data_directory).map_err(|e| e.to_string())?;
            let store = SqliteStore::open(&db_path(&settings.data_directory))
                .map_err(|e| format!("打开数据库失败: {e}"))?;

            // 启动时把今日已有记录载入内存，保证后续绝对覆盖式 flush 不丢数据
            let today = Local::now().format("%Y-%m-%d").to_string();
            let mut agg = UsageAggregator::new(
                settings.idle_threshold_seconds as f64,
                settings.excluded_app_keys.iter().cloned().collect(),
            );
            let rows = store.query_daily(&today, &today).map_err(|e| e.to_string())?;
            for r in rows {
                agg.daily.insert(format!("{}|{}", r.usage_date, r.app_key), r);
            }

            app.manage(AppState {
                settings: Mutex::new(settings),
                aggregator: Mutex::new(agg),
                store: Mutex::new(store),
                user_paused: AtomicBool::new(false),
                notified_limits: Mutex::new(HashSet::new()),
                tray: Mutex::new(None),
            });

            let handle = app.handle().clone();
            build_tray(&handle)?;
            setup_main_window(&handle);

            let show_widget = {
                let st = handle.state::<AppState>();
                let v = st.settings.lock().unwrap().show_floating_widget;
                v
            };
            if show_widget {
                create_widget_window(&handle)?;
                setup_widget_clamp(&handle);
            }

            spawn_monitor(handle.clone());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// ---------- 托盘 ----------

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let open_i = MenuItem::with_id(app, "open", "打开主界面", true, None::<&str>)?;
    let widget_i = CheckMenuItem::with_id(app, "widget", "悬浮窗", true, true, None::<&str>)?;
    let pause_i = CheckMenuItem::with_id(app, "pause", "暂停监控", true, false, None::<&str>)?;
    let settings_i = MenuItem::with_id(app, "settings", "设置", true, None::<&str>)?;
    let quit_i = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[&open_i, &widget_i, &pause_i, &settings_i, &PredefinedMenuItem::separator(app)?, &quit_i],
    )?;

    let widget_handle = widget_i.clone();
    let pause_handle = pause_i.clone();

    TrayIconBuilder::with_id("main-tray")
        .icon(app.default_window_icon().unwrap().clone())
        .tooltip("软件使用时间统计")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => show_main(app),
            "widget" => {
                let st = app.state::<AppState>();
                let visible = app.get_webview_window("widget").map(|w| w.is_visible().unwrap_or(false)).unwrap_or(false);
                set_widget_visible(app, !visible);
                let _ = st.tray.lock().unwrap().as_ref().map(|h| h.widget_toggle.set_checked(!visible));
            }
            "pause" => {
                let st = app.state::<AppState>();
                let paused = !st.user_paused.load(Ordering::Relaxed);
                set_paused_internal(app, paused);
                let _ = st.tray.lock().unwrap().as_ref().map(|h| h.pause_toggle.set_checked(paused));
            }
            "settings" => {
                show_main(app);
                let _ = app.emit("ui://navigate", "settings");
            }
            "quit" => {
                flush_state(app);
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show_main(tray.app_handle());
            }
        })
        .build(app)?;

    let state = app.state::<AppState>();
    *state.tray.lock().unwrap() = Some(TrayHandles { widget_toggle: widget_handle, pause_toggle: pause_handle });
    Ok(())
}

// ---------- 窗口 ----------

fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

fn setup_main_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let h = app.clone();
        w.on_window_event(move |e| {
            // 关闭 = 隐藏到托盘
            if let WindowEvent::CloseRequested { api, .. } = e {
                api.prevent_close();
                if let Some(m) = h.get_webview_window("main") {
                    let _ = m.hide();
                }
            }
        });
    }
}

fn create_widget_window(app: &AppHandle) -> tauri::Result<()> {
    let win = tauri::WebviewWindowBuilder::new(app, "widget", WebviewUrl::App("widget.html".into()))
        .title("使用时长悬浮窗")
        .inner_size(300.0, 216.0)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .shadow(false)
        .position(12.0, 12.0)
        .build()?;
    // 真·亚克力磨砂；失败则退回 Mica
    if window_vibrancy::apply_acrylic(&win, Some((18, 18, 24, 110))).is_err() {
        let _ = window_vibrancy::apply_mica(&win, None);
    }
    Ok(())
}

fn setup_widget_clamp(app: &AppHandle) {
    let Some(w) = app.get_webview_window("widget") else { return };
    let h = app.clone();
    let last_clamp = std::cell::Cell::new(std::time::Instant::now() - std::time::Duration::from_secs(1));
    w.on_window_event(move |e| {
        if let WindowEvent::Moved(_) = e {
            if last_clamp.get().elapsed() < std::time::Duration::from_millis(120) {
                return;
            }
            last_clamp.set(std::time::Instant::now());
            if let Some(w) = h.get_webview_window("widget") {
                clamp_widget_pos(&w);
            }
        }
    });
}

fn clamp_widget_pos(w: &WebviewWindow) {
    let pos = match w.outer_position() {
        Ok(p) => p,
        Err(_) => return,
    };
    let mon = match w.current_monitor() {
        Ok(Some(m)) => m,
        _ => return,
    };
    let size = match w.outer_size() {
        Ok(s) => s,
        Err(_) => return,
    };
    let mp = mon.position();
    let ms = mon.size();
    let max_x = mp.x + ms.width as i32 - size.width as i32;
    let max_y = mp.y + ms.height as i32 - size.height as i32;
    let x = pos.x.clamp(mp.x, max_x.max(mp.x));
    let y = pos.y.clamp(mp.y, max_y.max(mp.y));
    if x != pos.x || y != pos.y {
        let _ = w.set_position(PhysicalPosition::new(x, y));
    }
}

// ---------- 监控线程 ----------

fn foreground_identity(display_cache: &mut HashMap<String, String>) -> Option<AppIdentity> {
    let pid = tracker_core::win32::get_foreground_pid()?;
    let exe = tracker_core::win32::get_process_image_name(pid)?;
    let key = exe.to_lowercase();
    let stem = std::path::Path::new(&exe)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| key.clone());
    let display = if let Some(d) = display_cache.get(&key) {
        d.clone()
    } else {
        let d = version_info::file_description(&exe).unwrap_or_else(|| stem.clone());
        display_cache.insert(key.clone(), d.clone());
        d
    };
    Some(AppIdentity { app_key: key, process_name: stem, display_name: display, exe_path: exe })
}

fn spawn_monitor(app: AppHandle) {
    std::thread::spawn(move || {
        let mut display_cache: HashMap<String, String> = HashMap::new();
        let mut last_wall = Local::now();
        let mut last_tick = tracker_core::win32::get_tick_count64();
        let mut last_flush = std::time::Instant::now();
        let mut cur_day = last_wall.format("%Y-%m-%d").to_string();

        loop {
            let poll_ms = {
                let st = app.state::<AppState>();
                let v = st.settings.lock().unwrap().foreground_poll_ms;
                v.clamp(250, 10_000)
            };
            std::thread::sleep(std::time::Duration::from_millis(poll_ms));

            let now = Local::now();
            let tick = tracker_core::win32::get_tick_count64();
            let wall_delta = (now - last_wall).num_milliseconds();
            let tick_delta = (tick - last_tick) as i64;

            let state = app.state::<AppState>();
            {
                let mut agg = state.aggregator.lock().unwrap();
                if wall_delta - tick_delta > 3000 {
                    // 系统睡眠过：把睡眠起点之前记为活动，其余丢弃
                    let suspend_at = last_wall + chrono::Duration::milliseconds(tick_delta.max(0));
                    agg.on_suspend(suspend_at);
                    agg.on_resume(foreground_identity(&mut display_cache), now);
                } else if !tracker_core::win32::is_input_desktop_available() {
                    agg.on_locked(now);
                } else if agg.state == TrackingState::Locked {
                    agg.on_unlocked(foreground_identity(&mut display_cache), now);
                } else {
                    let idle = tracker_core::win32::get_idle_seconds();
                    agg.on_tick(foreground_identity(&mut display_cache), idle, now);
                }
                if state.user_paused.load(Ordering::Relaxed) && agg.state != TrackingState::Paused {
                    agg.pause(now);
                }
            }

            // 跨天：flush + 清限额提醒标记
            let today = now.format("%Y-%m-%d").to_string();
            if today != cur_day {
                cur_day = today;
                flush_state(&app);
                state.notified_limits.lock().unwrap().clear();
            }

            if last_flush.elapsed() >= std::time::Duration::from_secs(30) {
                flush_state(&app);
                last_flush = std::time::Instant::now();
            }
            check_limits(&app);

            let status = live_status(&app);
            let _ = app.emit("usage://tick", &status);

            last_wall = now;
            last_tick = tick;
        }
    });
}

fn flush_state(app: &AppHandle) {
    let state = app.state::<AppState>();
    let daily: Vec<DailyUsage> = {
        let agg = state.aggregator.lock().unwrap();
        agg.daily.values().cloned().collect()
    };
    let intervals: Vec<ForegroundInterval> = {
        let mut agg = state.aggregator.lock().unwrap();
        std::mem::take(&mut agg.pending_intervals)
    };
    let store = state.store.lock().unwrap();
    if let Err(e) = store.save_daily_replace(&daily) {
        eprintln!("save_daily failed: {e}");
    }
    if !intervals.is_empty() {
        if let Err(e) = store.save_intervals(&intervals) {
            eprintln!("save_intervals failed: {e}");
        }
    }
}

fn check_limits(app: &AppHandle) {
    let state = app.state::<AppState>();
    let (limits, rules, overrides) = {
        let s = state.settings.lock().unwrap();
        (s.category_daily_limit_minutes.clone(), s.category_rules.clone(), s.app_category_overrides.clone())
    };
    if limits.is_empty() {
        return;
    }
    let resolver = CategoryResolver::new(rules, overrides);
    let today = Local::now().format("%Y-%m-%d").to_string();
    let mut per_cat: HashMap<String, f64> = HashMap::new();
    {
        let agg = state.aggregator.lock().unwrap();
        for d in agg.daily.values().filter(|d| d.usage_date == today) {
            let cat = resolver.resolve(&d.display_name, &d.process_name, &d.exe_path);
            *per_cat.entry(cat).or_insert(0.0) += d.active_seconds;
        }
    }
    let to_notify = {
        let notified = state.notified_limits.lock().unwrap();
        categories_to_notify(&per_cat, &limits, &notified)
    };
    for cat in to_notify {
        state.notified_limits.lock().unwrap().insert(cat.clone());
        let _ = app.emit("usage://limit-reached", &cat);
        let _ = app
            .notification()
            .builder()
            .title("分类限额提醒")
            .body(format!("「{cat}」今日使用时长已超过设定限额"))
            .show();
    }
}

fn live_status(app: &AppHandle) -> LiveStatus {
    let state = app.state::<AppState>();
    let (rules, overrides) = {
        let s = state.settings.lock().unwrap();
        (s.category_rules.clone(), s.app_category_overrides.clone())
    };
    let resolver = CategoryResolver::new(rules, overrides);
    let now = Local::now();
    let today = now.format("%Y-%m-%d").to_string();
    let agg = state.aggregator.lock().unwrap();
    let current = agg.current_app().cloned();
    let (current_category, current_seconds) = match &current {
        Some(c) => (
            Some(resolver.resolve(&c.display_name, &c.process_name, &c.exe_path)),
            agg.current_segment_seconds(now),
        ),
        None => (None, 0.0),
    };
    let mut top: Vec<(String, String, f64)> = Vec::new();
    let mut today_total = 0.0;
    for d in agg.daily.values().filter(|d| d.usage_date == today) {
        today_total += d.active_seconds;
        let cat = resolver.resolve(&d.display_name, &d.process_name, &d.exe_path);
        top.push((d.display_name.clone(), cat, d.active_seconds));
    }
    top.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
    let top_apps = top
        .into_iter()
        .take(5)
        .map(|(display_name, category, active_seconds)| LiveTopApp { display_name, category, active_seconds })
        .collect();
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

fn db_path(data_dir: &str) -> String {
    std::path::Path::new(data_dir).join("usage.db").to_string_lossy().to_string()
}

fn load_settings() -> TrackerSettings {
    let base = TrackerSettings::default();
    let path = std::path::Path::new(&base.data_directory).join("settings.json");
    if let Ok(text) = std::fs::read_to_string(&path) {
        if let Ok(s) = serde_json::from_str::<TrackerSettings>(&text) {
            return s;
        }
    }
    base
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
            if let Err(e) = create_widget_window(app) {
                eprintln!("create widget failed: {e}");
            }
            setup_widget_clamp(app);
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
        agg.resume(foreground_identity(&mut cache), now);
    }
}

// ---------- Tauri Commands ----------

#[tauri::command]
fn get_live_status(app: AppHandle) -> LiveStatus {
    live_status(&app)
}

#[tauri::command]
fn get_dashboard(app: AppHandle, from: String, to: String) -> Result<DashboardData, String> {
    build_dashboard(&app, &from, &to)
}

fn build_dashboard(app: &AppHandle, from: &str, to: &str) -> Result<DashboardData, String> {
    let state = app.state::<AppState>();
    let (rules, overrides) = {
        let s = state.settings.lock().unwrap();
        (s.category_rules.clone(), s.app_category_overrides.clone())
    };
    let resolver = CategoryResolver::new(rules, overrides);

    let store_rows = {
        let st = state.store.lock().unwrap();
        st.query_daily(from, to).map_err(|e| e.to_string())?
    };
    let mut by_key: HashMap<String, DailyUsage> =
        store_rows.into_iter().map(|r| (format!("{}|{}", r.usage_date, r.app_key), r)).collect();
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
    let mut per_app: HashMap<String, AggApp> = HashMap::new();
    let mut trend: HashMap<String, f64> = HashMap::new();
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
        total += r.active_seconds;
    }

    let mut apps: Vec<DashboardApp> = per_app
        .into_iter()
        .map(|(app_key, e)| {
            let category = resolver.resolve(&e.display_name, &e.process_name, &e.exe_path);
            DashboardApp {
                app_key,
                display_name: e.display_name,
                process_name: e.process_name,
                exe_path: e.exe_path,
                category,
                active_seconds: e.seconds,
                percent: if total > 0.0 { e.seconds / total * 100.0 } else { 0.0 },
            }
        })
        .collect();
    apps.sort_by(|a, b| b.active_seconds.partial_cmp(&a.active_seconds).unwrap_or(std::cmp::Ordering::Equal));

    let mut cats: HashMap<String, f64> = HashMap::new();
    for a in &apps {
        *cats.entry(a.category.clone()).or_insert(0.0) += a.active_seconds;
    }
    let mut categories: Vec<DashboardCategory> = cats
        .into_iter()
        .map(|(name, seconds)| DashboardCategory {
            name,
            seconds,
            percent: if total > 0.0 { seconds / total * 100.0 } else { 0.0 },
        })
        .collect();
    categories.sort_by(|a, b| b.seconds.partial_cmp(&a.seconds).unwrap_or(std::cmp::Ordering::Equal));

    let mut trend: Vec<DashboardTrendPoint> =
        trend.into_iter().map(|(date, seconds)| DashboardTrendPoint { date, seconds }).collect();
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
fn save_settings(app: AppHandle, state: State<AppState>, settings: TrackerSettings) -> Result<(), String> {
    let old_dir = state.settings.lock().unwrap().data_directory.clone();
    let dir_changed = old_dir != settings.data_directory;

    autostart::set(settings.start_with_windows, &std::env::current_exe().map_err(|e| e.to_string())?.to_string_lossy())?;

    if dir_changed {
        std::fs::create_dir_all(&settings.data_directory).map_err(|e| e.to_string())?;
        let (daily, intervals) = {
            let st = state.store.lock().unwrap();
            let daily = st.query_daily("0000-01-01", "9999-12-31").map_err(|e| e.to_string())?;
            let intervals = st.query_intervals("0000-01-01", "9999-12-31").map_err(|e| e.to_string())?;
            (daily, intervals)
        };
        let new_store = SqliteStore::open(&db_path(&settings.data_directory)).map_err(|e| e.to_string())?;
        new_store.save_daily_replace(&daily).map_err(|e| e.to_string())?;
        new_store.save_intervals(&intervals).map_err(|e| e.to_string())?;
        *state.store.lock().unwrap() = new_store;
    }

    {
        let mut agg = state.aggregator.lock().unwrap();
        agg.apply_settings(
            settings.idle_threshold_seconds as f64,
            settings.excluded_app_keys.iter().cloned().collect(),
        );
    }

    *state.settings.lock().unwrap() = settings;
    save_settings_file(&app);

    // 悬浮窗可见性同步
    let want = state.settings.lock().unwrap().show_floating_widget;
    let now_visible = app.get_webview_window("widget").map(|w| w.is_visible().unwrap_or(false)).unwrap_or(false);
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
}

#[tauri::command]
fn save_category_rules(app: AppHandle, state: State<AppState>, rules: Vec<CategoryRuleDef>) -> Result<(), String> {
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
    }
    state.notified_limits.lock().unwrap().clear();
    let _ = app;
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
        let is_dir = std::path::Path::new(&path).is_dir();
        let arg = if is_dir {
            path.clone()
        } else {
            format!("/select,{path}")
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
