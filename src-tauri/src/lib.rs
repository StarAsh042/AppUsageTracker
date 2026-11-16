mod autostart;
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
use tauri_plugin_notification::NotificationExt;
use tracker_core::aggregator::UsageAggregator;
use tracker_core::category::{CategoryResolver, UNCATEGORIZED};
use tracker_core::limits::categories_to_notify;
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
            get_extension_path,
            open_browser_ext_page,
            smart_install_extension,
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
                if r.process_name.to_lowercase() == *self_stem() {
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

            purge_expired(&handle);
            spawn_api_server(&handle);

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
                    flush_state(&h);
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
            spawn_monitor(handle.clone());
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

// ---------- 监控线程 ----------

/// 自身进程名（小写，不含扩展名），用于排除统计程序自己（含旧数据清理）。
fn self_stem() -> &'static str {
    static SELF: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    SELF.get_or_init(|| {
        std::env::current_exe()
            .ok()
            .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().to_lowercase()))
            .unwrap_or_else(|| "app-usage-tracker".to_string())
    })
}

fn foreground_identity(display_cache: &mut HashMap<String, String>) -> Option<AppIdentity> {
    let pid = tracker_core::win32::get_foreground_pid()?;
    let exe = tracker_core::win32::get_process_image_name(pid)?;
    let key = exe.to_lowercase();
    let stem = std::path::Path::new(&exe)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| key.clone());
    // 永不统计统计程序自身（无论路径如何）
    if stem.to_lowercase() == *self_stem() {
        return None;
    }
    let display = if let Some(d) = display_cache.get(&key) {
        d.clone()
    } else {
        let d = version_info::file_description(&exe).unwrap_or_else(|| stem.clone());
        display_cache.insert(key.clone(), d.clone());
        d
    };
    Some(AppIdentity {
        app_key: key,
        process_name: stem,
        display_name: display,
        exe_path: exe,
    })
}

fn spawn_monitor(app: AppHandle) {
    std::thread::spawn(move || {
        let mut display_cache: HashMap<String, String> = HashMap::new();
        let mut last_wall = Local::now();
        let mut last_tick = tracker_core::win32::get_tick_count64();
        let mut last_flush = std::time::Instant::now();
        let mut last_checkpoint = std::time::Instant::now();
        let mut cur_day = last_wall.format("%Y-%m-%d").to_string();
        let mut rest_acc = 0.0f64;

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

            // 窗口标题记录（隐私开关；仅在活跃态记录，排除模式命中即丢弃）
            {
                let (record, patterns) = {
                    let s = state.settings.lock().unwrap();
                    (s.record_window_titles, s.title_exclude_patterns.clone())
                };
                if record {
                    let (app_key, title) = {
                        let agg = state.aggregator.lock().unwrap();
                        let key = agg.current_app().map(|c| c.app_key.clone());
                        (key, tracker_core::win32::get_foreground_title())
                    };
                    let excluded = title.as_deref().map(|t| {
                        let tl = t.to_lowercase();
                        patterns
                            .iter()
                            .any(|p| !p.is_empty() && tl.contains(&p.to_lowercase()))
                    });
                    let feed = match (&app_key, &title) {
                        (Some(k), Some(t)) if !excluded.unwrap_or(false) => {
                            Some((k.as_str(), t.as_str()))
                        }
                        _ => None,
                    };
                    let mut agg = state.aggregator.lock().unwrap();
                    agg.on_title_tick(feed, now);
                }
            }

            // 休息提醒：连续活跃累计，空闲即视为已休息
            let rest_minutes = state.settings.lock().unwrap().rest_reminder_minutes;
            {
                let agg = state.aggregator.lock().unwrap();
                match agg.state {
                    TrackingState::Active => rest_acc += wall_delta.max(0) as f64 / 1000.0,
                    TrackingState::Idle => rest_acc = 0.0,
                    _ => {}
                }
            }
            if rest_minutes > 0 && rest_acc >= rest_minutes as f64 * 60.0 {
                rest_acc = 0.0;
                let _ = app.emit("ui://rest", ());
                let _ = app
                    .notification()
                    .builder()
                    .title(t!("休息提醒"))
                    .body(format!(
                        "已连续活跃使用 {rest_minutes} 分钟，站起来活动一下吧"
                    ))
                    .show();
            }

            // 跨天：flush + 清限额提醒标记 + 保留策略清理
            let today = now.format("%Y-%m-%d").to_string();
            if today != cur_day {
                cur_day = today;
                flush_state(&app);
                state.notified_limits.lock().unwrap().clear();
                rest_acc = 0.0;
                purge_expired(&app);
            }

            if last_flush.elapsed() >= std::time::Duration::from_secs(30) {
                flush_state(&app);
                last_flush = std::time::Instant::now();
            }

            // 每 5 分钟补一次显式 checkpoint：自动 checkpoint 只是把 WAL 页
            // 复用置零，长事务或长时间只读期间可能一直不触发，-wal 于是越滚
            // 越大；这里配合 journal_size_limit 把文件真正截断。
            if last_checkpoint.elapsed() >= std::time::Duration::from_secs(300) {
                last_checkpoint = std::time::Instant::now();
                if let Err(e) = state.store.lock().unwrap().checkpoint() {
                    eprintln!("periodic checkpoint failed: {e}");
                }
            }
            check_limits(&app);

            let status = live_status(&app);
            let _ = app.emit("usage://tick", &status);
            // 托盘 tooltip 实时显示今日活跃与当前应用
            if let Some(tray) = state.tray_icon.lock().unwrap().as_ref() {
                let tip = match &status.current_app {
                    Some(a) => format!(
                        "软件使用时间统计
今日 {} · {}",
                        fmt_hms(status.today_total_seconds),
                        a.display_name
                    ),
                    None => format!(
                        "软件使用时间统计
今日 {}",
                        fmt_hms(status.today_total_seconds)
                    ),
                };
                let _ = tray.set_tooltip(Some(tip));
            }

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
    let (intervals, state_intervals, title_intervals, web_intervals): (
        Vec<ForegroundInterval>,
        Vec<StateInterval>,
        Vec<TitleInterval>,
        Vec<WebInterval>,
    ) = {
        let mut agg = state.aggregator.lock().unwrap();
        (
            std::mem::take(&mut agg.pending_intervals),
            std::mem::take(&mut agg.pending_state_intervals),
            std::mem::take(&mut agg.pending_title_intervals),
            {
                // 网页心跳：闭合打开段（同键续记）+ 取待写明细
                let now2 = Local::now();
                let mut web_open = state.web_open.lock().unwrap();
                let mut web_pending = state.web_pending.lock().unwrap();
                if let Some((d, t, start)) = web_open.take() {
                    let dur = (now2 - start).num_milliseconds() as f64 / 1000.0;
                    if dur >= 1.0 {
                        web_pending.push(WebInterval {
                            domain: d.clone(),
                            title: t.clone(),
                            start_time: start.format("%Y-%m-%d %H:%M:%S%.3f").to_string(),
                            end_time: now2.format("%Y-%m-%d %H:%M:%S%.3f").to_string(),
                            duration_seconds: dur,
                        });
                    }
                    *web_open = Some((d, t, now2));
                }
                std::mem::take(&mut web_pending)
            },
        )
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
    if !state_intervals.is_empty() {
        if let Err(e) = store.save_state_intervals(&state_intervals) {
            eprintln!("save_state_intervals failed: {e}");
        }
    }
    if !title_intervals.is_empty() {
        if let Err(e) = store.save_title_intervals(&title_intervals) {
            eprintln!("save_title_intervals failed: {e}");
        }
    }
    if !web_intervals.is_empty() {
        if let Err(e) = store.save_web_intervals(&web_intervals) {
            eprintln!("save_web_intervals failed: {e}");
        }
    }
}

// ---------- 本地 HTTP API（127.0.0.1，供第三方工具读取统计） ----------

static API_GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// （重）启本地 API 服务线程；设置里关闭/改端口后自动退出旧线程。
fn spawn_api_server(app: &AppHandle) {
    let (enabled, port) = {
        let st = app.state::<AppState>();
        let s = st.settings.lock().unwrap();
        (s.api_enabled, s.api_port)
    };
    {
        let st = app.state::<AppState>();
        let mut last = st.api_last.lock().unwrap();
        if *last == Some((enabled, port)) {
            return; // 配置未变，不打扰现有服务
        }
        *last = Some((enabled, port));
    }
    let gen = API_GEN.fetch_add(1, Ordering::SeqCst) + 1;
    if !enabled {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        let addr = format!("127.0.0.1:{port}");
        let listener = match std::net::TcpListener::bind(&addr) {
            Ok(l) => l,
            Err(e) => {
                eprintln!("本地 API 启动失败({addr}): {e}");
                return;
            }
        };
        let _ = listener.set_nonblocking(true);
        let _ = app.emit("api://started", &addr);
        loop {
            if API_GEN.load(Ordering::SeqCst) != gen {
                break;
            }
            match listener.accept() {
                Ok((stream, _)) => {
                    let app = app.clone();
                    std::thread::spawn(move || {
                        let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(2)));
                        handle_api(&app, stream);
                    });
                }
                Err(_) => {
                    std::thread::sleep(std::time::Duration::from_millis(300));
                }
            }
        }
    });
}

/// 从 URL 提取域名（去 scheme/www/端口/认证信息，小写）。
fn extract_domain(url: &str) -> Option<String> {
    let after = url.split_once("://").map(|x| x.1).unwrap_or(url);
    let host = after.split('/').next()?;
    let host = host.split('@').next()?;
    let host = host.split(':').next()?;
    let host = host.strip_prefix("www.").unwrap_or(host);
    let host = host.trim();
    if host.is_empty() {
        None
    } else {
        Some(host.to_lowercase())
    }
}

/// 浏览器心跳聚合：同域名+标题延长当前段，否则闭合旧段开新段。
fn heartbeat_web(app: &AppHandle, domain: String, title: String) {
    let state = app.state::<AppState>();
    let now = Local::now();
    let mut open = state.web_open.lock().unwrap();
    let extend = matches!(open.as_ref(), Some((d, t, _)) if *d == domain && *t == title);
    if !extend {
        if let Some((d, t, start)) = open.take() {
            let dur = (now - start).num_milliseconds() as f64 / 1000.0;
            if dur >= 5.0 {
                state.web_pending.lock().unwrap().push(WebInterval {
                    domain: d,
                    title: t,
                    start_time: start.format("%Y-%m-%d %H:%M:%S%.3f").to_string(),
                    end_time: now.format("%Y-%m-%d %H:%M:%S%.3f").to_string(),
                    duration_seconds: dur,
                });
            }
        }
        *open = Some((domain, title, now));
    }
}

fn api_json(stream: &mut std::net::TcpStream, code: u16, body: String) {
    let reason = match code {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        _ => "OK",
    };
    let head = format!(
        "HTTP/1.1 {code} {reason}\r\nContent-Type: application/json; charset=utf-8\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    use std::io::Write;
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(body.as_bytes());
    let _ = stream.flush();
}

fn handle_api(app: &AppHandle, mut stream: std::net::TcpStream) {
    use std::io::Read;
    // 读请求头 +（POST 时）完整 body
    let mut data: Vec<u8> = Vec::new();
    let mut tmp = [0u8; 2048];
    loop {
        let n = match stream.read(&mut tmp) {
            Ok(n) if n > 0 => n,
            _ => break,
        };
        data.extend_from_slice(&tmp[..n]);
        let Some(pos) = data.windows(4).position(|w| w == b"\r\n\r\n") else {
            if data.len() > 16384 {
                break;
            }
            continue;
        };
        let head_str = String::from_utf8_lossy(&data[..pos]);
        let cl = head_str
            .to_lowercase()
            .lines()
            .find(|l| l.starts_with("content-length:"))
            .and_then(|l| l.split(':').nth(1))
            .and_then(|v| v.trim().parse::<usize>().ok())
            .unwrap_or(0);
        if data.len() - (pos + 4) >= cl || data.len() > 65536 {
            break;
        }
    }
    let head = String::from_utf8_lossy(&data);
    let Some(req_line) = head.lines().next() else {
        return;
    };
    let mut parts = req_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("/");
    let body = String::from_utf8_lossy(
        &data[data
            .windows(4)
            .position(|w| w == b"\r\n\r\n")
            .map(|p| p + 4)
            .unwrap_or(data.len())..],
    )
    .to_string();
    let (path, query) = match target.split_once('?') {
        Some((p, q)) => (p.to_string(), q.to_string()),
        None => (target.to_string(), String::new()),
    };
    let qs = |key: &str| -> Option<String> {
        query.split('&').find_map(|kv| {
            let (k, v) = kv.split_once('=')?;
            (k == key).then(|| v.to_string())
        })
    };

    match path.as_str() {
        "/api/web-heartbeat" if method == "POST" => {
            let (api_on, record) = {
                let st = app.state::<AppState>();
                let s = st.settings.lock().unwrap();
                (s.api_enabled, s.record_web)
            };
            if !api_on || !record {
                return api_json(
                    &mut stream,
                    403,
                    r#"{"error":"web tracking disabled"}"#.into(),
                );
            }
            #[derive(serde::Deserialize)]
            struct Hb {
                url: String,
                #[serde(default)]
                title: String,
            }
            let Ok(hb) = serde_json::from_str::<Hb>(body.trim()) else {
                return api_json(&mut stream, 400, r#"{"error":"bad json"}"#.into());
            };
            match extract_domain(&hb.url) {
                Some(domain) => {
                    heartbeat_web(app, domain, hb.title);
                    let interval = app
                        .state::<AppState>()
                        .settings
                        .lock()
                        .unwrap()
                        .web_heartbeat_interval_secs;
                    api_json(
                        &mut stream,
                        200,
                        serde_json::json!({ "ok": true, "interval": interval }).to_string(),
                    );
                }
                None => api_json(&mut stream, 400, r#"{"error":"bad url"}"#.into()),
            }
        }
        "/api/web-top" if method == "GET" => {
            let today = Local::now().format("%Y-%m-%d").to_string();
            let from = qs("from").unwrap_or_else(|| today.clone());
            let to = qs("to").unwrap_or(today);
            let limit = qs("limit")
                .and_then(|v| v.parse::<i64>().ok())
                .unwrap_or(15);
            let st = app.state::<AppState>();
            let rows = {
                let s = st.store.lock().unwrap();
                s.top_web(&from, &to, limit.clamp(1, 200))
                    .map_err(|e| e.to_string())
            };
            match rows {
                Ok(list) => {
                    let items: Vec<serde_json::Value> = list
                        .into_iter()
                        .map(|(domain, title, seconds)| {
                            serde_json::json!({ "domain": domain, "title": title, "seconds": seconds })
                        })
                        .collect();
                    api_json(
                        &mut stream,
                        200,
                        serde_json::Value::Array(items).to_string(),
                    )
                }
                Err(e) => api_json(&mut stream, 500, format!("{{\"error\":\"{e}\"}}")),
            }
        }
        "/" | "/api" => api_json(
            &mut stream,
            200,
            serde_json::json!({
                "service": "AppUsageTracker",
                "endpoints": [
                    "/api/live",
                    "/api/dashboard?from=YYYY-MM-DD&to=YYYY-MM-DD",
                    "/api/timeline?date=YYYY-MM-DD",
                    "/api/settings",
                    "/api/version"
                ]
            })
            .to_string(),
        ),
        "/api/live" => api_json(
            &mut stream,
            200,
            serde_json::to_string(&live_status(app)).unwrap_or_default(),
        ),
        "/api/dashboard" => {
            let today = Local::now().format("%Y-%m-%d").to_string();
            let from = qs("from").unwrap_or_else(|| today.clone());
            let to = qs("to").unwrap_or(today);
            match build_dashboard(app, &from, &to) {
                Ok(d) => api_json(
                    &mut stream,
                    200,
                    serde_json::to_string(&d).unwrap_or_default(),
                ),
                Err(e) => api_json(&mut stream, 400, format!("{{\"error\":\"{e}\"}}")),
            }
        }
        "/api/timeline" => {
            let Some(date) = qs("date") else {
                return api_json(&mut stream, 400, r#"{"error":"missing date"}"#.into());
            };
            match timeline_data(app, &date) {
                Ok(d) => api_json(
                    &mut stream,
                    200,
                    serde_json::to_string(&d).unwrap_or_default(),
                ),
                Err(e) => api_json(&mut stream, 400, format!("{{\"error\":\"{e}\"}}")),
            }
        }
        "/api/settings" => {
            let st = app.state::<AppState>();
            let s = st.settings.lock().unwrap().clone();
            api_json(
                &mut stream,
                200,
                serde_json::to_string(&s).unwrap_or_default(),
            )
        }
        "/api/version" => api_json(
            &mut stream,
            200,
            serde_json::json!({ "version": app.package_info().version.to_string() }).to_string(),
        ),
        _ => api_json(&mut stream, 404, r#"{"error":"not found"}"#.into()),
    }
}

/// 保留策略：清理 N 天前的明细与聚合（0 = 永久保留）。/// 保留策略：清理 N 天前的明细与聚合（0 = 永久保留）。
fn purge_expired(app: &AppHandle) {
    let state = app.state::<AppState>();
    let retention = state.settings.lock().unwrap().retention_days;
    if retention == 0 {
        return;
    }
    let cutoff = (Local::now() - chrono::Duration::days(retention as i64))
        .format("%Y-%m-%d")
        .to_string();
    let result = state.store.lock().unwrap().purge_before(&cutoff);
    if let Err(e) = result {
        eprintln!("purge_before failed: {e}");
    }
}

fn check_limits(app: &AppHandle) {
    let state = app.state::<AppState>();
    let limits = {
        let s = state.settings.lock().unwrap();
        s.category_daily_limit_minutes.clone()
    };
    if limits.is_empty() {
        return;
    }
    let resolver = app.state::<AppState>().resolver.lock().unwrap().clone();
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
            .title(t!("分类限额提醒"))
            .body(t!("「%{cat}」今日使用时长已超过设定限额", cat = cat))
            .show();
    }
}

fn live_status(app: &AppHandle) -> LiveStatus {
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
        agg.resume(foreground_identity(&mut cache), now);
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
    spawn_api_server(&app);

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

// ---------- 浏览器扩展安装 ----------

/// 「加载已解压的扩展程序」用的目录（数据目录内）。
/// 绝不去探测 exe 的相对路径：安装版会命中用户家目录下无关的 extension/ 目录。
fn extension_dir(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    let data_dir = {
        let state = app.state::<AppState>();
        let x = state.settings.lock().unwrap().data_directory.clone();
        x
    };
    Ok(std::path::Path::new(&data_dir)
        .join("browser-ext")
        .join("extension"))
}

/// 把内置扩展写到数据目录，返回可供浏览器手动加载的目录。
#[tauri::command]
fn get_extension_path(app: AppHandle) -> Result<String, String> {
    let dir = extension_dir(&app)?;
    materialize_extension(&dir)?;
    Ok(dir.to_string_lossy().to_string())
}

/// 单个浏览器的智能安装结果（前端逐条展示）。
#[derive(Serialize, Clone)]
struct ExtInstallResult {
    browser: String,
    name: String,
    ok: bool,
    /// ok / pack_failed / bad_crx / registry_failed（not_found 保留兼容旧前端）
    code: String,
    extension_id: String,
    message: String,
}

// CRX3 头部解析与扩展 ID 编码的真实实现搬到了 tracker-core 的 ext 模块（带单测），
// 这里只留薄封装，调用点不用改。
use tracker_core::ext::{
    crx_id_bytes as core_crx_id_bytes, extension_id_from_hash as core_extension_id,
};

/// CRX3 头部 → signed_header_data 里的 crx_id（= SHA256(公钥) 前 16 字节）。
/// 结构：magic "Cr24" | uint32 LE 版本(=3) | uint32 LE header 长度 | CrxFileHeader。
fn crx_id_bytes(crx: &[u8]) -> Option<Vec<u8>> {
    core_crx_id_bytes(crx)
}

/// 16 字节公钥哈希 → Chromium 扩展 ID（a-p 编码，32 字符）。
fn extension_id_from_hash(hash: &[u8]) -> String {
    core_extension_id(hash)
}

#[derive(Clone, Copy)]
struct BrowserSpec {
    key: &'static str,
    name: &'static str,
    /// 相对各安装基址的可执行文件路径（依次尝试）。
    exe_paths: &'static [&'static str],
    /// HKCU\Software 下的注册表根，外部扩展键写在 <root>\Extensions\<id>。
    /// 多数 Chromium fork 未改 install_static，因此同时写 Google\Chrome 兜底。
    registry_roots: &'static [&'static str],
    /// 扩展管理页。
    ext_page: &'static str,
}

fn browser_specs() -> Vec<BrowserSpec> {
    vec![
        BrowserSpec {
            key: "chrome",
            name: "Chrome",
            exe_paths: &[r"Google\Chrome\Application\chrome.exe"],
            registry_roots: &[r"Google\Chrome"],
            ext_page: "chrome://extensions",
        },
        BrowserSpec {
            key: "edge",
            name: "Edge",
            exe_paths: &[r"Microsoft\Edge\Application\msedge.exe"],
            registry_roots: &[r"Microsoft\Edge"],
            ext_page: "edge://extensions",
        },
        BrowserSpec {
            key: "chrome_beta",
            name: "Chrome Beta",
            exe_paths: &[r"Google\Chrome Beta\Application\chrome.exe"],
            registry_roots: &[r"Google\Chrome Beta", r"Google\Chrome"],
            ext_page: "chrome://extensions",
        },
        BrowserSpec {
            key: "chrome_dev",
            name: "Chrome Dev",
            exe_paths: &[r"Google\Chrome Dev\Application\chrome.exe"],
            registry_roots: &[r"Google\Chrome Dev", r"Google\Chrome"],
            ext_page: "chrome://extensions",
        },
        BrowserSpec {
            key: "chrome_canary",
            name: "Chrome Canary",
            exe_paths: &[r"Google\Chrome SxS\Application\chrome.exe"],
            registry_roots: &[r"Google\Chrome SxS", r"Google\Chrome"],
            ext_page: "chrome://extensions",
        },
        BrowserSpec {
            key: "edge_beta",
            name: "Edge Beta",
            exe_paths: &[r"Microsoft\Edge Beta\Application\msedge.exe"],
            registry_roots: &[r"Microsoft\Edge Beta", r"Microsoft\Edge"],
            ext_page: "edge://extensions",
        },
        BrowserSpec {
            key: "edge_dev",
            name: "Edge Dev",
            exe_paths: &[r"Microsoft\Edge Dev\Application\msedge.exe"],
            registry_roots: &[r"Microsoft\Edge Dev", r"Microsoft\Edge"],
            ext_page: "edge://extensions",
        },
        BrowserSpec {
            key: "chromium",
            name: "Chromium",
            exe_paths: &[r"Chromium\Application\chrome.exe"],
            registry_roots: &[r"Chromium", r"Google\Chrome"],
            ext_page: "chrome://extensions",
        },
        BrowserSpec {
            key: "brave",
            name: "Brave",
            exe_paths: &[r"BraveSoftware\Brave-Browser\Application\brave.exe"],
            registry_roots: &[r"BraveSoftware\Brave-Browser", r"Google\Chrome"],
            ext_page: "chrome://extensions",
        },
        BrowserSpec {
            key: "vivaldi",
            name: "Vivaldi",
            exe_paths: &[r"Vivaldi\Application\vivaldi.exe"],
            registry_roots: &[r"Vivaldi", r"Google\Chrome"],
            ext_page: "chrome://extensions",
        },
        BrowserSpec {
            key: "opera",
            name: "Opera",
            exe_paths: &[r"Opera Software\Opera Stable\opera.exe"],
            registry_roots: &[r"Opera Software", r"Google\Chrome"],
            ext_page: "opera://extensions",
        },
        BrowserSpec {
            key: "opera_gx",
            name: "Opera GX",
            exe_paths: &[r"Opera Software\Opera GX Stable\opera.exe"],
            registry_roots: &[r"Opera Software", r"Google\Chrome"],
            ext_page: "opera://extensions",
        },
        BrowserSpec {
            key: "cent",
            name: "Cent Browser",
            exe_paths: &[
                r"cent\CentBrowser\Application\chrome.exe",
                r"CentBrowser\Application\chrome.exe",
            ],
            registry_roots: &[r"CentBrowser", r"Google\Chrome"],
            ext_page: "chrome://extensions",
        },
        BrowserSpec {
            key: "browser360",
            name: "360 极速浏览器",
            exe_paths: &[
                r"360Chrome\Chrome\Application\360chrome.exe",
                r"360\360Chrome\Chrome\Application\360chrome.exe",
            ],
            registry_roots: &[r"360\360se6\Chrome", r"360chrome\Chrome", r"Google\Chrome"],
            ext_page: "chrome://extensions",
        },
        BrowserSpec {
            key: "yandex",
            name: "Yandex Browser",
            exe_paths: &[r"Yandex\YandexBrowser\Application\browser.exe"],
            registry_roots: &[r"Yandex\YandexBrowser", r"Google\Chrome"],
            ext_page: "chrome://extensions",
        },
        BrowserSpec {
            key: "coccoc",
            name: "CocCoc",
            exe_paths: &[r"CocCoc\Browser\Application\browser.exe"],
            registry_roots: &[r"CocCoc\Browser", r"Google\Chrome"],
            ext_page: "chrome://extensions",
        },
        BrowserSpec {
            key: "whale",
            name: "Naver Whale",
            exe_paths: &[r"Naver\Naver Whale\Application\whale.exe"],
            registry_roots: &[r"Naver\Naver Whale", r"Google\Chrome"],
            ext_page: "chrome://extensions",
        },
    ]
}

fn find_browser_exe(spec: &BrowserSpec) -> Option<std::path::PathBuf> {
    for env_key in ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"] {
        if let Ok(base) = std::env::var(env_key) {
            let base = std::path::PathBuf::from(base);
            for rel in spec.exe_paths {
                let p = base.join(rel);
                if p.is_file() {
                    return Some(p);
                }
            }
        }
    }
    None
}

/// 调用浏览器自带的 --pack-extension 生成 CRX3；用共享 .pem 保证扩展 ID 跨浏览器/重装稳定。
fn pack_extension(
    exe: &std::path::Path,
    unpacked: &std::path::Path,
    shared_pem: &std::path::Path,
) -> Result<std::path::PathBuf, String> {
    let crx = unpacked.with_extension("crx");
    let _ = std::fs::remove_file(&crx);
    // 上次失败可能留下 <name>.pem，Edge 会报「指定扩展的私钥已存在」；有共享密钥时不需要它
    let _ = std::fs::remove_file(unpacked.with_extension("pem"));
    let mut cmd = std::process::Command::new(exe);
    cmd.arg(format!("--pack-extension={}", unpacked.display()));
    if shared_pem.is_file() {
        cmd.arg(format!("--pack-extension-key={}", shared_pem.display()));
    }
    cmd.arg("--no-message-box")
        .arg("--no-first-run")
        .arg("--no-default-browser-check");
    // 独立用户目录：避免命令被转发给正在运行的浏览器实例而不产出 CRX
    let profile = unpacked.parent().unwrap_or(unpacked).join("pack-profile");
    cmd.arg(format!("--user-data-dir={}", profile.display()));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    let out = cmd.output().map_err(|e| format!("启动浏览器失败: {e}"))?;
    let _ = std::fs::remove_dir_all(&profile);
    if !crx.is_file() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let brief: String = stderr.chars().take(300).collect();
        return Err(format!("打包失败: {}", brief.trim()));
    }
    // 首次打包时浏览器会在扩展目录旁生成 <name>.pem，保存为共享密钥
    if !shared_pem.is_file() {
        let generated = unpacked.with_extension("pem");
        if generated.is_file() {
            if let Some(parent) = shared_pem.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::copy(&generated, shared_pem);
        }
    }
    Ok(crx)
}

/// 打包优先级：优先用 Chrome / Edge，fork 通常基于同版 Chromium 但命令行支持不一。
fn pack_priority(key: &str) -> usize {
    match key {
        "chrome" => 0,
        "edge" => 1,
        "chrome_beta" | "chrome_dev" | "chrome_canary" => 2,
        "edge_beta" | "edge_dev" | "chromium" => 3,
        "brave" => 4,
        _ => 10,
    }
}

/// 旧版把每个浏览器的 CRX / pem 放在 browser-ext/<browser>/ 下，新版统一放 browser-ext/ 根；
/// 这些残留目录不再被读取，安装时清理掉。
fn cleanup_legacy_dirs(base: &std::path::Path) {
    for spec in browser_specs() {
        let legacy = base.join(spec.key);
        if legacy.is_dir() {
            let _ = std::fs::remove_dir_all(&legacy);
        }
    }
}

/// 清理旧版本遗留的外部扩展注册表键：只删 path 指向本应用 browser-ext 目录、
/// 且 ID 与当前正确扩展不同的键；其它软件注册的扩展一律不动。
/// keep_id 为 None（打包失败、无正确 ID 可用）时删除全部指向本应用旧 CRX 的键。
#[cfg(windows)]
fn cleanup_stale_extensions(base: &std::path::Path, keep_id: Option<&str>, roots: &[&str]) {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_ALL_ACCESS};
    use winreg::RegKey;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let marker = base.to_string_lossy().to_lowercase();
    for &root in roots {
        let ext_key = match hkcu
            .open_subkey_with_flags(format!("Software\\{}\\Extensions", root), KEY_ALL_ACCESS)
        {
            Ok(k) => k,
            Err(_) => continue,
        };
        let mut stale: Vec<String> = Vec::new();
        for sub in ext_key.enum_keys().flatten() {
            if let Some(k) = keep_id {
                if k == sub.as_str() {
                    continue;
                }
            }
            let owned = match ext_key.open_subkey(&sub) {
                Ok(k) => k,
                Err(_) => continue,
            };
            let p: String = match owned.get_value("path") {
                Ok(v) => v,
                Err(_) => continue,
            };
            if p.to_lowercase().contains(&marker) {
                stale.push(sub);
            }
        }
        // 先收集再删，避免枚举过程中删除导致跳项
        for id in stale {
            let _ = ext_key.delete_subkey_all(&id);
        }
    }
}

#[cfg(not(windows))]
fn cleanup_stale_extensions(_base: &std::path::Path, _keep_id: Option<&str>, _roots: &[&str]) {}

/// 写注册表外部扩展键：HKCU\Software\<root>\Extensions\<id>。
/// roots 里任一写入成功即算成功（不同 fork 读取的根不同，逐个都写以兼容）。
#[cfg(windows)]
fn register_external_extension(
    roots: &[&str],
    id: &str,
    crx: &std::path::Path,
    version: &str,
) -> Result<(), String> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let crx_path = crx.to_string_lossy().to_string();
    let ver = version.to_string();
    let mut last_err = String::new();
    let mut any = false;
    for root in roots {
        let key_path = format!("Software\\{}\\Extensions\\{}", root, id);
        match hkcu.create_subkey(&key_path) {
            Ok((key, _)) => {
                if let Err(e) = key.set_value("path", &crx_path) {
                    last_err = format!("写入扩展路径失败: {e}");
                    continue;
                }
                if let Err(e) = key.set_value("version", &ver) {
                    last_err = format!("写入扩展版本失败: {e}");
                    continue;
                }
                any = true;
            }
            Err(e) => last_err = format!("打开注册表失败: {e}"),
        }
    }
    if any {
        Ok(())
    } else {
        Err(last_err)
    }
}

#[cfg(not(windows))]
fn register_external_extension(
    _roots: &[&str],
    _id: &str,
    _crx: &std::path::Path,
    _version: &str,
) -> Result<(), String> {
    Err("当前平台不支持自动注册扩展".into())
}

/// 扩展源码内置副本：安装包不携带 extension/ 目录时也能自动安装。
const EMBEDDED_MANIFEST: &str = include_str!("../../extension/manifest.json");
const EMBEDDED_BACKGROUND: &str = include_str!("../../extension/background.js");

/// 准备一份可打包的扩展目录：始终使用编译期内置副本。
/// 不从磁盘上的 extension/ 目录复制，避免把用户机器里其它扩展误打包进来。
fn materialize_extension(dst: &std::path::Path) -> Result<(), String> {
    if dst.exists() {
        let _ = std::fs::remove_dir_all(dst);
    }
    std::fs::create_dir_all(dst).map_err(|e| format!("创建扩展目录失败: {e}"))?;
    std::fs::write(dst.join("manifest.json"), EMBEDDED_MANIFEST)
        .map_err(|e| format!("写出 manifest 失败: {e}"))?;
    std::fs::write(dst.join("background.js"), EMBEDDED_BACKGROUND)
        .map_err(|e| format!("写出 background 失败: {e}"))?;
    Ok(())
}

/// 打包前兜底校验：manifest 必须是本项目的扩展，否则宁可失败也不注册。
fn validate_extension_manifest(dir: &std::path::Path) -> Result<(), String> {
    let name = std::fs::read(dir.join("manifest.json"))
        .ok()
        .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
        .and_then(|v| v.get("name").and_then(|x| x.as_str()).map(str::to_string))
        .unwrap_or_default();
    if !name.contains("AppUsageTracker") {
        return Err(format!("扩展目录内容异常（name={name}），已中止安装"));
    }
    Ok(())
}

/// 读扩展 manifest 里的版本号（注册表 version 值必须与 CRX 内一致）。
fn manifest_version(dir: &std::path::Path) -> String {
    tracker_core::ext::manifest_version(dir)
}

fn install_extension_blocking(app: &AppHandle) -> Result<Vec<ExtInstallResult>, String> {
    let data_dir = {
        let state = app.state::<AppState>();
        let s = state.settings.lock().unwrap();
        s.data_directory.clone()
    };
    let base = std::path::Path::new(&data_dir).join("browser-ext");
    std::fs::create_dir_all(&base).map_err(|e| format!("创建目录失败: {e}"))?;
    // 旧版残留：按浏览器分目录的 CRX / pem 已不再使用
    cleanup_legacy_dirs(&base);

    // 只处理本机确实安装了的浏览器，避免结果列表被一堆「未检测到」刷屏
    let specs = browser_specs();
    let detected: Vec<(&BrowserSpec, std::path::PathBuf)> = specs
        .iter()
        .filter_map(|s| find_browser_exe(s).map(|exe| (s, exe)))
        .collect();
    if detected.is_empty() {
        return Ok(vec![]);
    }

    // 汇总所有可能的注册表根（含未检测到的浏览器），用于清理旧版本残留键
    let mut all_roots: Vec<&str> = Vec::new();
    for spec in &specs {
        for &root in spec.registry_roots {
            if !all_roots.contains(&root) {
                all_roots.push(root);
            }
        }
    }

    // 扩展内容只用内置副本；共享密钥 → 所有浏览器同一扩展 ID
    let unpacked = base.join("extension");
    materialize_extension(&unpacked)?;
    validate_extension_manifest(&unpacked)?;
    let version = manifest_version(&unpacked);
    let shared_pem = base.join("extension-key.pem");

    // 按 Chrome / Edge 优先顺序逐个尝试打包，任一成功即止；失败自动降级到下一款
    let mut order: Vec<usize> = (0..detected.len()).collect();
    order.sort_by_key(|&i| pack_priority(detected[i].0.key));
    let mut crx: Option<std::path::PathBuf> = None;
    let mut pack_errors: Vec<String> = Vec::new();
    for &i in &order {
        match pack_extension(&detected[i].1, &unpacked, &shared_pem) {
            Ok(p) => {
                crx = Some(p);
                break;
            }
            Err(e) => pack_errors.push(format!("{}: {}", detected[i].0.name, e)),
        }
    }
    let crx = match crx {
        Some(p) => p,
        None => {
            cleanup_stale_extensions(&base, None, &all_roots);
            let message = pack_errors.join("；");
            return Ok(detected
                .iter()
                .map(|(spec, _)| ExtInstallResult {
                    browser: spec.key.to_string(),
                    name: spec.name.to_string(),
                    ok: false,
                    code: "pack_failed".into(),
                    extension_id: String::new(),
                    message: message.clone(),
                })
                .collect());
        }
    };
    let hash = match std::fs::read(&crx).ok().and_then(|b| crx_id_bytes(&b)) {
        Some(h) => h,
        None => {
            cleanup_stale_extensions(&base, None, &all_roots);
            let message = "打包产物无效，请改用手动安装".to_string();
            return Ok(detected
                .iter()
                .map(|(spec, _)| ExtInstallResult {
                    browser: spec.key.to_string(),
                    name: spec.name.to_string(),
                    ok: false,
                    code: "bad_crx".into(),
                    extension_id: String::new(),
                    message: message.clone(),
                })
                .collect());
        }
    };
    let id = extension_id_from_hash(&hash);
    // 删除旧版本写入的、指向本应用旧 CRX 的错误注册表键（保留当前正确 ID）
    cleanup_stale_extensions(&base, Some(id.as_str()), &all_roots);

    let mut results: Vec<ExtInstallResult> = Vec::new();
    for (spec, _exe) in &detected {
        let mut row = ExtInstallResult {
            browser: spec.key.to_string(),
            name: spec.name.to_string(),
            ok: false,
            code: "registry_failed".into(),
            extension_id: id.clone(),
            message: String::new(),
        };
        match register_external_extension(spec.registry_roots, &id, &crx, &version) {
            Ok(()) => {
                row.ok = true;
                row.code = "ok".into();
                row.message = crx.to_string_lossy().to_string();
            }
            Err(e) => row.message = e,
        }
        results.push(row);
    }

    // 装好至少一个浏览器 → 自动开启网页统计并通知前端刷新设置
    if results.iter().any(|r| r.ok) {
        let need = {
            let state = app.state::<AppState>();
            let mut s = state.settings.lock().unwrap();
            let need = !s.record_web;
            if need {
                s.record_web = true;
            }
            need
        };
        if need {
            save_settings_file(app);
            let snapshot = app.state::<AppState>().settings.lock().unwrap().clone();
            let _ = app.emit("app://settings", &snapshot);
        }
    }
    Ok(results)
}

/// 智能安装浏览器扩展：探测已装浏览器 → 打包 CRX3 → 写注册表外部扩展键。
#[tauri::command]
async fn smart_install_extension(app: AppHandle) -> Result<Vec<ExtInstallResult>, String> {
    tauri::async_runtime::spawn_blocking(move || install_extension_blocking(&app))
        .await
        .map_err(|e| format!("安装任务失败: {e}"))?
}

/// 打开浏览器扩展管理页。
#[tauri::command]
fn open_browser_ext_page(browser: String) -> Result<(), String> {
    let url = browser_specs()
        .iter()
        .find(|s| s.key == browser)
        .map(|s| s.ext_page)
        .ok_or("未知浏览器")?;
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // start 命令打开 chrome:// 或 edge:// 内部页面
        std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .creation_flags(0x08000000) // CREATE_NO_WINDOW
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(not(windows))]
    {
        let _ = url;
    }
    Ok(())
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
