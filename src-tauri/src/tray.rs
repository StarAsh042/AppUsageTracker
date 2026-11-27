// 系统托盘：图标、右键菜单、左键唤起主界面，以及退出前的 flush + checkpoint。
// 从 lib.rs 整体搬出，逻辑与顺序不变（拆分 lib.rs 的第二步）。

use std::sync::atomic::Ordering;

use rust_i18n::t;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager};

use crate::monitor::flush_state;
use crate::settings::{set_paused_internal, set_widget_visible};
use crate::windows::show_main;
use crate::AppState;

/// 托盘菜单里需要回写的两个勾选项（/菜单事件里同步状态用）。
pub(crate) struct TrayHandles {
    pub(crate) widget_toggle: CheckMenuItem<tauri::Wry>,
    pub(crate) pause_toggle: CheckMenuItem<tauri::Wry>,
}

/// 建托盘图标与右键菜单，并把菜单项句柄存进 AppState 供后续勾选同步。
pub(crate) fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let open_i = MenuItem::with_id(app, "open", &t!("打开主界面"), true, None::<&str>)?;
    let widget_i = CheckMenuItem::with_id(app, "widget", &t!("悬浮窗"), true, true, None::<&str>)?;
    let pause_i = CheckMenuItem::with_id(app, "pause", &t!("暂停监控"), true, false, None::<&str>)?;
    let settings_i = MenuItem::with_id(app, "settings", &t!("设置"), true, None::<&str>)?;
    let quit_i = MenuItem::with_id(app, "quit", &t!("退出"), true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &open_i,
            &widget_i,
            &pause_i,
            &settings_i,
            &PredefinedMenuItem::separator(app)?,
            &quit_i,
        ],
    )?;

    let widget_handle = widget_i.clone();
    let pause_handle = pause_i.clone();

    let tray_icon = TrayIconBuilder::with_id("main-tray")
        .icon(app.default_window_icon().unwrap().clone())
        .tooltip(t!("软件使用时间统计"))
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => show_main(app),
            "widget" => {
                let st = app.state::<AppState>();
                let visible = app
                    .get_webview_window("widget")
                    .map(|w| w.is_visible().unwrap_or(false))
                    .unwrap_or(false);
                set_widget_visible(app, !visible);
                let _ = st
                    .tray
                    .lock()
                    .unwrap()
                    .as_ref()
                    .map(|h| h.widget_toggle.set_checked(!visible));
            }
            "pause" => {
                let st = app.state::<AppState>();
                let paused = !st.user_paused.load(Ordering::Relaxed);
                set_paused_internal(app, paused);
                let _ = st
                    .tray
                    .lock()
                    .unwrap()
                    .as_ref()
                    .map(|h| h.pause_toggle.set_checked(paused));
            }
            "settings" => {
                show_main(app);
                let _ = app.emit("ui://navigate", "settings");
            }
            "quit" => {
                flush_state(app);
                // 退出前 checkpoint 一次并截断 -wal：正常关闭后磁盘上只该剩主库。
                // 失败也不阻止退出，最多是 -wal 留到下次启动。
                {
                    let st = app.state::<AppState>();
                    let store = st.store.lock().unwrap();
                    if let Err(e) = store.checkpoint() {
                        log::error!("checkpoint on quit failed: {e}");
                    }
                }
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main(tray.app_handle());
            }
        })
        .build(app)?;
    let tray = tray_icon;

    let state = app.state::<AppState>();
    *state.tray_icon.lock().unwrap() = Some(tray);
    *state.tray.lock().unwrap() = Some(TrayHandles {
        widget_toggle: widget_handle,
        pause_toggle: pause_handle,
    });
    Ok(())
}
