mod api_server;
mod app_meta;
mod apps;
mod autostart;
mod backup;
mod commands;
mod dashboard;
mod error;
mod ext_install;
mod live;
mod logging;
mod monitor;
mod reports;
mod settings;
mod sync;
mod timeline;
mod tray;
mod update;
mod version_info;
mod windows;

rust_i18n::i18n!("locales", fallback = "zh-CN");

use std::collections::HashSet;
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;

use chrono::Local;
use tauri::tray::TrayIcon;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use tracker_core::aggregator::UsageAggregator;
use tracker_core::category::CategoryResolver;
use tracker_core::models::{DailyUsage, WebInterval};
use tracker_core::settings::TrackerSettings;
use tracker_core::store::SqliteStore;

use crate::error::AppError;
use crate::sync::MutexExt;

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
                        let s = st.settings.lock_or_recover();
                        (s.hotkey_main.clone(), s.hotkey_widget.clone())
                    };
                    if is_hotkey(&main_hk_s, shortcut) {
                        windows::toggle_main(app);
                    } else if is_hotkey(&widget_hk_s, shortcut) {
                        let st = app.state::<AppState>();
                        let visible = app
                            .get_webview_window("widget")
                            .map(|w| w.is_visible().unwrap_or(false))
                            .unwrap_or(false);
                        settings::set_widget_visible(app, !visible);
                        let guard = st.tray.lock_or_recover();
                        if let Some(h) = guard.as_ref() {
                            let _ = h.widget_toggle.set_checked(!visible);
                        }
                    }
                })
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            dashboard::get_dashboard,
            live::get_live_status,
            settings::get_settings,
            settings::save_settings,
            settings::set_paused,
            settings::set_app_category,
            settings::save_category_rules,
            commands::export_csv,
            commands::clear_all_data,
            settings::list_available_categories,
            commands::open_path,
            commands::get_app_version,
            windows::show_widget_menu,
            windows::set_widget_collapsed,
            windows::update_widget_settings,
            windows::toggle_widget,
            windows::close_widget,
            timeline::get_timeline,
            windows::toggle_main_window,
            apps::get_app_detail,
            settings::exclude_app,
            settings::set_app_display_name,
            update::check_update,
            update::install_update,
            reports::get_top_titles,
            reports::get_top_web,
            ext_install::get_extension_path,
            ext_install::open_browser_ext_page,
            ext_install::smart_install_extension,
            reports::db_maintenance,
            backup::delete_backup,
            reports::export_weekly_report,
            apps::scan_installed_apps,
            backup::export_backup_json,
            backup::import_backup_json,
            backup::list_backups,
            backup::open_backup_dir,
            backup::restore_backup_named,
            backup::restore_backup_content,
        ])
        .setup(|app| {
            let (settings, load_event) = settings::load_settings();
            logging::init(&settings.data_directory);
            log::info!(
                "启动 v{}，数据目录 {}",
                env!("CARGO_PKG_VERSION"),
                settings.data_directory
            );
            std::fs::create_dir_all(&settings.data_directory)?;
            let store = SqliteStore::open(&settings::db_path(&settings.data_directory))
                .map_err(|e| AppError::new("db", format!("打开数据库失败: {e}")))?;

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
                .map_err(|e| AppError::new("db", e.to_string()))?;
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
            // 配置读取出过问题的话，到这里才有通知通道可用。
            settings::report_load_event(&handle, load_event);
            // 老版本写进 HKCU Run 的是裸 exe 路径（无引号、无标记）。这里按当前
            // 设置再写一次，把历史值就地规范化成 "exe" --autostart 的形式。
            if handle
                .state::<AppState>()
                .settings
                .lock_or_recover()
                .start_with_windows
            {
                if let Ok(exe) = std::env::current_exe() {
                    let _ = autostart::set(true, &exe.to_string_lossy());
                }
            }
            settings::apply_locale(&handle);
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
                let v = st.settings.lock_or_recover().show_floating_widget;
                v
            };
            if show_widget {
                windows::create_widget_window(&handle)?;
                windows::setup_widget_clamp(&handle);
                let _ = handle.emit("widget://visibility", true);
                if handle
                    .state::<AppState>()
                    .settings
                    .lock_or_recover()
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
                    let _ = st.store.lock_or_recover().checkpoint();
                    let (total, apps) = {
                        let agg = st.aggregator.lock_or_recover();
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
                        let s = st.settings.lock_or_recover();
                        s.data_directory.clone()
                    };
                    let path = std::path::Path::new(&dir).join("smoke-report.json");
                    match serde_json::to_string_pretty(&report) {
                        Ok(json) => {
                            if let Err(e) = std::fs::write(&path, json) {
                                log::error!("smoke report write failed: {e}");
                            }
                        }
                        Err(e) => log::error!("smoke report serialize failed: {e}"),
                    }
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

/// 这次按下的快捷键是否就是设置里这个组合。
///
/// 设置字符串可能被手工改坏，解析失败就当没配这个热键。以前这里用的是
/// expect，一次无效配置会让整个热键回调 panic。
pub(crate) fn is_hotkey(configured: &str, pressed: &Shortcut) -> bool {
    configured.parse::<Shortcut>().ok().as_ref() == Some(pressed)
}

/// 全局热键注册（设置开关）。
pub(crate) fn apply_hotkeys(app: &AppHandle) {
    let (enabled, main_hk_s, widget_hk_s) = {
        let st = app.state::<AppState>();
        let s = st.settings.lock_or_recover();
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
    // 解析与注册失败都在这里报一次：热键无效是用户能感知的配置问题，
    // 静默跳过会让人以为按了没反应是别的毛病。
    match main_hk_s.parse::<Shortcut>() {
        Ok(hk) => {
            if let Err(e) = gs.register(hk) {
                log::warn!("主热键注册失败({main_hk_s}): {e}");
            }
        }
        Err(e) => log::warn!("主热键无效({main_hk_s:?})，已跳过: {e}"),
    }
    match widget_hk_s.parse::<Shortcut>() {
        Ok(hk) => {
            if let Err(e) = gs.register(hk) {
                log::warn!("悬浮窗热键注册失败({widget_hk_s}): {e}");
            }
        }
        Err(e) => log::warn!("悬浮窗热键无效({widget_hk_s:?})，已跳过: {e}"),
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 无效热键设置不会命中() {
        let pressed: Shortcut = "Ctrl+Alt+D".parse().expect("测试用的组合应当可解析");
        assert!(is_hotkey("Ctrl+Alt+D", &pressed), "相同组合应当命中");
        assert!(
            !is_hotkey("", &pressed),
            "空串是无效配置，不能命中更不能 panic"
        );
        assert!(!is_hotkey("Ctrl+Alt+不是键", &pressed), "写坏的值不能命中");
        assert!(!is_hotkey("Ctrl+Alt+W", &pressed), "另一个热键不能误命中");
    }
}
