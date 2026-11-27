// 窗口：主窗显示/隐藏、悬浮窗创建与背板、贴边收起/展开、尺寸动画、悬浮窗右键菜单。
// 从 lib.rs 整体搬出（拆分 lib.rs 的第三步），逻辑与顺序不变。
//
// 注意：crate 里已有外部 crate `windows`（Win32 绑定），本模块与它同名，
// 所以模块内引用 Win32 时必须写 `::windows::...`，否则会被解析成 crate::windows。

use std::sync::atomic::Ordering;

use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, WebviewWindow, WindowEvent};
use tracker_core::settings::WidgetPrefs;

use crate::error::AppError;
use crate::settings::{save_settings_file, set_widget_visible};
use crate::sync::MutexExt;
use crate::AppState;
use rust_i18n::t;

/// 悬浮窗三种形态（逻辑像素）：贴边收起后只留一根细线（横边横线、竖边竖线）。
pub(crate) const WIDGET_NORMAL: (f64, f64) = (300.0, 216.0);
pub(crate) const WIDGET_COMPACT: (f64, f64) = (200.0, 76.0);
pub(crate) const WIDGET_LINE_H: (f64, f64) = (160.0, 4.0);
pub(crate) const WIDGET_LINE_V: (f64, f64) = (4.0, 160.0);

/// 悬浮窗尺寸动画代数（新动画使旧动画失效）。
static ANIM_GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// 悬浮窗双击切换主界面（前端调用）。
#[tauri::command]
pub(crate) fn toggle_main_window(app: AppHandle) {
    toggle_main(&app);
}

/// 主界面显示/隐藏切换（全局热键用）。
pub(crate) fn toggle_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        if w.is_visible().unwrap_or(false) && w.is_focused().unwrap_or(false) {
            let _ = w.hide();
        } else {
            show_main(app);
        }
    }
}

pub(crate) fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}
pub(crate) fn setup_main_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        // 主窗口玻璃背板：Mica 优先，退回亚克力
        if !apply_system_backdrop(&w, false) {
            let _ = window_vibrancy::apply_acrylic(&w, Some((24, 26, 33, 90)));
        }
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

pub(crate) fn create_widget_window(app: &AppHandle) -> tauri::Result<()> {
    let prefs = {
        app.state::<AppState>()
            .settings
            .lock_or_recover()
            .widget
            .clone()
    };
    let (w, h) = if prefs.compact {
        WIDGET_COMPACT
    } else {
        WIDGET_NORMAL
    };
    // 窗口的固定属性（标题、透明、无边框、不占任务栏、无阴影、初始位置 12,12）
    // 声明在 tauri.conf.json 的 app.windows 里，且 create=false —— 启动时不自动建窗，
    // 由这里按需 from_config 建。配置里表达不了的只有随偏好变化的尺寸与置顶，
    // 用 builder 的 inner_size / always_on_top 覆写（同名字段后者覆盖前者）。
    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|c| c.label == "widget")
        .ok_or(tauri::Error::WindowNotFound)?;
    let win = tauri::WebviewWindowBuilder::from_config(app, config)?
        .inner_size(w, h)
        .always_on_top(prefs.always_on_top)
        .build()?;
    // Win11 Mica 常显背板（失焦也保持磨砂，不再只在点击时透明）；旧系统退回 legacy 亚克力
    if !apply_system_backdrop(&win, false)
        && window_vibrancy::apply_acrylic(&win, Some(accent_tint(&prefs.accent))).is_err()
    {
        let _ = window_vibrancy::apply_mica(&win, None);
    }
    apply_rounded_corners(&win);
    // 系统背板应用时会把窗口踢出 topmost 层，必须在背板之后重新置顶
    if prefs.always_on_top {
        let _ = win.set_always_on_top(true);
    }
    // 悬浮窗右键原生菜单（事件处理器只在创建时注册一次）
    let h = app.clone();
    win.on_menu_event(move |_win, event| handle_widget_menu_event(&h, event.id().as_ref()));
    Ok(())
}

/// DWM 系统级背板（Win11 22H2+ 真系统亚克力/Mica，质感优于 legacy 合成）。
/// transient=true 悬浮窗亚克力；false 主窗口 Mica。成功返回 true。
pub(crate) fn apply_system_backdrop(win: &WebviewWindow, transient: bool) -> bool {
    #[cfg(windows)]
    {
        use ::windows::Win32::Foundation::HWND;
        use ::windows::Win32::Graphics::Dwm::{
            DwmSetWindowAttribute, DWMSBT_MAINWINDOW, DWMSBT_TRANSIENTWINDOW,
            DWMWA_SYSTEMBACKDROP_TYPE, DWM_SYSTEMBACKDROP_TYPE,
        };
        if let Ok(hwnd) = win.hwnd() {
            let backdrop = if transient {
                DWMSBT_TRANSIENTWINDOW
            } else {
                DWMSBT_MAINWINDOW
            };
            let ok = unsafe {
                DwmSetWindowAttribute(
                    HWND(hwnd.0),
                    DWMWA_SYSTEMBACKDROP_TYPE,
                    &backdrop as *const _ as *const std::ffi::c_void,
                    std::mem::size_of::<DWM_SYSTEMBACKDROP_TYPE>() as u32,
                )
            };
            return ok.is_ok();
        }
    }
    #[cfg(not(windows))]
    let _ = (win, transient);
    false
}

/// DWM 系统圆角（Win11），让亚克力背板一起被裁圆。
pub(crate) fn apply_rounded_corners(win: &WebviewWindow) {
    #[cfg(windows)]
    {
        use ::windows::Win32::Foundation::HWND;
        use ::windows::Win32::Graphics::Dwm::{
            DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
            DWM_WINDOW_CORNER_PREFERENCE,
        };
        if let Ok(hwnd) = win.hwnd() {
            let pref = DWM_WINDOW_CORNER_PREFERENCE(DWMWCP_ROUND.0);
            unsafe {
                let _ = DwmSetWindowAttribute(
                    HWND(hwnd.0),
                    DWMWA_WINDOW_CORNER_PREFERENCE,
                    &pref as *const _ as *const std::ffi::c_void,
                    std::mem::size_of::<DWM_WINDOW_CORNER_PREFERENCE>() as u32,
                );
            }
        }
    }
    #[cfg(not(windows))]
    let _ = win;
}

pub(crate) fn setup_widget_clamp(app: &AppHandle) {
    let Some(w) = app.get_webview_window("widget") else {
        return;
    };
    let h = app.clone();
    let last_clamp =
        std::cell::Cell::new(std::time::Instant::now() - std::time::Duration::from_secs(1));
    w.on_window_event(move |e| {
        if let WindowEvent::Moved(_) = e {
            if last_clamp.get().elapsed() < std::time::Duration::from_millis(120) {
                return;
            }
            last_clamp.set(std::time::Instant::now());
            if let Some(w) = h.get_webview_window("widget") {
                clamp_widget_pos(&w);
                // 展开态且非动画中才记录锚点（动画中途的位置会污染锚点）
                if let (Ok(p), Ok(s)) = (w.outer_position(), w.outer_size()) {
                    let st = h.state::<AppState>();
                    let (ew, eh) = {
                        let settings = st.settings.lock_or_recover();
                        let (lw, lh) = if settings.widget.compact {
                            WIDGET_COMPACT
                        } else {
                            WIDGET_NORMAL
                        };
                        (lw, lh)
                    };
                    let sf = w.scale_factor().unwrap_or(1.0);
                    let animating = st.widget_anim.load(Ordering::Relaxed);
                    if !animating
                        && (s.width as f64 - ew * sf).abs() < 8.0
                        && (s.height as f64 - eh * sf).abs() < 8.0
                    {
                        *st.widget_anchor.lock_or_recover() = (p.x, p.y);
                    }
                }
            }
        }
    });
}

pub(crate) fn clamp_widget_pos(w: &WebviewWindow) {
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

/// 贴边收起的几何计算：算出吸附哪条边、线放在哪、线多大。
///
/// 只做算术、不碰窗口，所以能直接单测 —— 贴边是纯坐标问题，
/// 而调窗口 API 出错时程序只会静默不动作，最难查。
///
/// 返回 None 表示窗口离四条边都超过 8 逻辑像素，不该收起。
fn collapse_target(
    mon_pos: (i32, i32),
    mon_size: (u32, u32),
    anchor: (i32, i32),
    sf: f64,
    compact: bool,
) -> Option<(&'static str, i32, i32, f64, f64)> {
    let (ew, eh) = if compact {
        WIDGET_COMPACT
    } else {
        WIDGET_NORMAL
    };
    let (mx, my) = mon_pos;
    let (mw, mh) = (mon_size.0 as i32, mon_size.1 as i32);
    // 只有悬浮窗本身贴近屏幕边缘才收起（间隙按窗口矩形计算，逻辑像素）
    let gap_l = anchor.0 as f64 / sf;
    let gap_t = anchor.1 as f64 / sf;
    let gap_r = (mx + mw - anchor.0 - (ew * sf) as i32) as f64 / sf;
    let gap_b = (my + mh - anchor.1 - (eh * sf) as i32) as f64 / sf;
    let min = gap_l.min(gap_r).min(gap_t).min(gap_b);
    if min > 8.0 {
        return None;
    }
    // 线沿边缘摆放的位置以锚点窗口中心为基准
    let cx = anchor.0 + ((ew * sf) as i32) / 2;
    let cy = anchor.1 + ((eh * sf) as i32) / 2;
    let (edge, lw, lh) = if min == gap_l {
        ("left", WIDGET_LINE_V.0, WIDGET_LINE_V.1)
    } else if min == gap_r {
        ("right", WIDGET_LINE_V.0, WIDGET_LINE_V.1)
    } else if min == gap_t {
        ("top", WIDGET_LINE_H.0, WIDGET_LINE_H.1)
    } else {
        ("bottom", WIDGET_LINE_H.0, WIDGET_LINE_H.1)
    };
    let (pw, ph) = ((lw * sf) as i32, (lh * sf) as i32);
    let (x, y) = if edge == "left" {
        (mx, (cy - ph / 2).clamp(my, my + mh - ph))
    } else if edge == "right" {
        (mx + mw - pw, (cy - ph / 2).clamp(my, my + mh - ph))
    } else if edge == "top" {
        ((cx - pw / 2).clamp(mx, mx + mw - pw), my)
    } else {
        ((cx - pw / 2).clamp(mx, mx + mw - pw), my + mh - ph)
    };
    Some((edge, x, y, lw, lh))
}

/// 贴边收起：把悬浮窗缩成小条并吸附到最近的屏幕边缘。
pub(crate) fn collapse_widget(app: &AppHandle, win: &WebviewWindow) {
    let (mon, sf, compact) = match (win.current_monitor(), win.scale_factor()) {
        (Ok(Some(m)), Ok(f)) => (
            m,
            f,
            app.state::<AppState>()
                .settings
                .lock_or_recover()
                .widget
                .compact,
        ),
        _ => return,
    };
    let anchor = *app.state::<AppState>().widget_anchor.lock_or_recover();
    let mp = mon.position();
    let ms = mon.size();
    let Some((edge, x, y, lw, lh)) =
        collapse_target((mp.x, mp.y), (ms.width, ms.height), anchor, sf, compact)
    else {
        // 不贴边：保持展开，并给前端一个明确的提示（旧实现直接 return 导致空白）
        let _ = app.emit("widget://edge", "none");
        let _ = app.emit("widget://hint", "edge_required");
        return;
    };
    let cur = (
        win.outer_position().map(|p| p.x).unwrap_or(x),
        win.outer_position().map(|p| p.y).unwrap_or(y),
        win.outer_size().map(|s| s.width as f64 / sf).unwrap_or(lw),
        win.outer_size().map(|s| s.height as f64 / sf).unwrap_or(lh),
    );
    // 先切样式再收窗：前端只画一根 4px 居中细线，线随窗口一起收到边缘
    let _ = app.emit("widget://edge", edge);
    animate_window(app, win, cur, (x as f64, y as f64, lw, lh), 160, None, None);
}

/// 展开悬浮窗（正常 / 缩略），回到锚点位置。
pub(crate) fn expand_widget(app: &AppHandle, win: &WebviewWindow, compact: bool) {
    let (sf, mon) = match (win.scale_factor(), win.current_monitor()) {
        (Ok(f), Ok(Some(m))) => (f, m),
        _ => return,
    };
    let (w, h) = if compact {
        WIDGET_COMPACT
    } else {
        WIDGET_NORMAL
    };
    let mp = mon.position();
    let ms = mon.size();
    let pos = win
        .outer_position()
        .unwrap_or(PhysicalPosition::new(mp.x, mp.y));
    let size = win.outer_size().unwrap_or(tauri::PhysicalSize::new(4, 160));
    // 线中心与线贴的是哪条边（间隙最小者）
    let lcx = pos.x + size.width as i32 / 2;
    let lcy = pos.y + size.height as i32 / 2;
    let dl = pos.x - mp.x;
    let dr = mp.x + ms.width as i32 - (pos.x + size.width as i32);
    let dt = pos.y - mp.y;
    let db = mp.y + ms.height as i32 - (pos.y + size.height as i32);
    let min = dl.min(dr).min(dt).min(db);
    let (ew, eh) = ((w * sf) as i32, (h * sf) as i32);
    let (x, y) = if min == dl && dl <= 8 {
        (
            mp.x,
            (lcy - eh / 2).clamp(mp.y, mp.y + ms.height as i32 - eh),
        )
    } else if min == dr && dr <= 8 {
        (
            mp.x + ms.width as i32 - ew,
            (lcy - eh / 2).clamp(mp.y, mp.y + ms.height as i32 - eh),
        )
    } else if min == dt && dt <= 8 {
        (
            (lcx - ew / 2).clamp(mp.x, mp.x + ms.width as i32 - ew),
            mp.y,
        )
    } else if min == db && db <= 8 {
        (
            (lcx - ew / 2).clamp(mp.x, mp.x + ms.width as i32 - ew),
            mp.y + ms.height as i32 - eh,
        )
    } else {
        // 线不在边缘（兜底）：回到锚点
        let a = *app.state::<AppState>().widget_anchor.lock_or_recover();
        (a.0, a.1)
    };
    let cur = (
        pos.x,
        pos.y,
        size.width as f64 / sf,
        size.height as f64 / sf,
    );
    // 内容等窗口长回原尺寸后再显示，避免动画期间被压扁
    animate_window(
        app,
        win,
        cur,
        (x as f64, y as f64, w, h),
        160,
        Some((x, y)),
        Some("none"),
    );
}

/// 窗口尺寸/位置动画（smoothstep，10 步；新动画会取消旧动画）。
#[allow(clippy::too_many_arguments)]
pub(crate) fn animate_window(
    app: &AppHandle,
    win: &WebviewWindow,
    from: (i32, i32, f64, f64),
    to: (f64, f64, f64, f64),
    ms: u64,
    end_anchor: Option<(i32, i32)>,
    end_edge: Option<&'static str>,
) {
    let gen = ANIM_GEN.fetch_add(1, Ordering::SeqCst) + 1;
    let win = win.clone();
    let app = app.clone();
    let steps = 10u64;
    app.state::<AppState>()
        .widget_anim
        .store(true, Ordering::Relaxed);
    std::thread::spawn(move || {
        for i in 1..=steps {
            if ANIM_GEN.load(Ordering::SeqCst) != gen {
                app.state::<AppState>()
                    .widget_anim
                    .store(false, Ordering::Relaxed);
                return;
            }
            let t = i as f64 / steps as f64;
            let t = t * t * (3.0 - 2.0 * t);
            let _ = win.set_size(tauri::LogicalSize::new(
                from.2 + (to.2 - from.2) * t,
                from.3 + (to.3 - from.3) * t,
            ));
            let _ = win.set_position(PhysicalPosition::new(
                (from.0 as f64 + (to.0 - from.0 as f64) * t) as i32,
                (from.1 as f64 + (to.1 - from.1 as f64) * t) as i32,
            ));
            std::thread::sleep(std::time::Duration::from_millis(ms / steps));
        }
        if ANIM_GEN.load(Ordering::SeqCst) == gen {
            let _ = win.set_size(tauri::LogicalSize::new(to.2, to.3));
            let _ = win.set_position(PhysicalPosition::new(to.0 as i32, to.1 as i32));
            if let Some((ax, ay)) = end_anchor {
                *app.state::<AppState>().widget_anchor.lock_or_recover() = (ax, ay);
            }
            // 收起/展开的样式在动画结束后才切换，避免窗口还是大尺寸时先套上蓝色细线导致闪烁
            if let Some(e) = end_edge {
                let _ = app.emit("widget://edge", e);
            }
        }
        app.state::<AppState>()
            .widget_anim
            .store(false, Ordering::Relaxed);
    });
}

/// 强调色 → 亚克力 tint（深色底与强调色 10% 混合）。
pub(crate) fn accent_tint(accent: &str) -> (u8, u8, u8, u8) {
    tracker_core::accent::accent_tint(accent)
}

/// 应用悬浮窗偏好：持久化 + 窗口行为 + 通知前端。
pub(crate) fn apply_widget_prefs(app: &AppHandle, prefs: &WidgetPrefs) {
    {
        let state = app.state::<AppState>();
        let mut s = state.settings.lock_or_recover();
        s.widget = prefs.clone();
    }
    save_settings_file(app);
    if let Some(win) = app.get_webview_window("widget") {
        let _ = win.set_always_on_top(prefs.always_on_top);
        if prefs.edge_snap {
            collapse_widget(app, &win);
        } else {
            expand_widget(app, &win, prefs.compact);
        }
    }
    let _ = app.emit("widget://prefs", prefs);
}

/// 悬浮窗右键原生菜单事件（id: w.top / w.lock / w.snap / w.compact / w.op.N / w.settings）。
pub(crate) fn handle_widget_menu_event(app: &AppHandle, id: &str) {
    let mut prefs = {
        app.state::<AppState>()
            .settings
            .lock_or_recover()
            .widget
            .clone()
    };
    match id {
        "w.settings" => {
            show_main(app);
            let _ = app.emit("ui://navigate", "settings");
            return;
        }
        "w.top" => prefs.always_on_top = !prefs.always_on_top,
        "w.lock" => prefs.locked = !prefs.locked,
        "w.snap" => prefs.edge_snap = !prefs.edge_snap,
        "w.compact" => prefs.compact = !prefs.compact,
        _ => {
            if let Some(v) = id.strip_prefix("w.op.").and_then(|v| v.parse::<u8>().ok()) {
                prefs.opacity = v;
            } else {
                return;
            }
        }
    }
    apply_widget_prefs(app, &prefs);
}

// ---------- 悬浮窗相关的 IPC 命令 ----------
// 这五个命令原先在 lib.rs 的「Tauri Commands」段里，第十一步（实际为第十二步）搬到这里，
// 让悬浮窗的窗口操作、右键菜单与状态同步集中在一个模块。行为不变。

/// 悬浮窗右键 → 弹出原生上下文菜单（带 ✓ 与子菜单，Win11 玻璃样式）。
#[tauri::command]
pub(crate) fn show_widget_menu(app: AppHandle) -> Result<(), AppError> {
    // 用稳定的 get_webview_window + popup_menu，避免为这一个调用点开启 tauri 的 unstable feature。
    let window = app
        .get_webview_window("widget")
        .ok_or_else(|| AppError::new("widget-missing", "悬浮窗未创建"))?;
    let prefs = {
        app.state::<AppState>()
            .settings
            .lock_or_recover()
            .widget
            .clone()
    };
    let check = |id: &str, label: &str, checked: bool| {
        CheckMenuItem::with_id(&app, id, label, true, checked, None::<&str>)
            .map_err(|e| AppError::new("menu", e.to_string()))
    };
    let top_i = check("w.top", &t!("置顶显示"), prefs.always_on_top)?;
    let lock_i = check("w.lock", &t!("锁定位置"), prefs.locked)?;
    let snap_i = check("w.snap", &t!("贴边收起"), prefs.edge_snap)?;
    let compact_i = check("w.compact", &t!("缩略模式"), prefs.compact)?;

    let op_menu = Submenu::with_id(&app, "w.opacity", &t!("透明度"), true)
        .map_err(|e| AppError::new("menu", e.to_string()))?;
    for (v, label) in [
        (85u8, &t!("高 85%")),
        (70, &t!("中 70%")),
        (55, &t!("低 55%")),
        (40, &t!("低 40%")),
    ] {
        let item = check(&format!("w.op.{v}"), label, prefs.opacity == v)?;
        op_menu
            .append(&item)
            .map_err(|e| AppError::new("menu", e.to_string()))?;
    }
    let settings_i = MenuItem::with_id(&app, "w.settings", &t!("设置…"), true, None::<&str>)
        .map_err(|e| AppError::new("menu", e.to_string()))?;
    let sep =
        PredefinedMenuItem::separator(&app).map_err(|e| AppError::new("menu", e.to_string()))?;
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
    .map_err(|e| AppError::new("menu", e.to_string()))?;
    window
        .popup_menu(&menu)
        .map_err(|e| AppError::new("menu", e.to_string()))
}

/// 主界面顶栏按钮：切换悬浮窗显示，返回切换后的可见状态。
#[tauri::command]
pub(crate) fn toggle_widget(app: AppHandle) -> bool {
    let visible = app
        .get_webview_window("widget")
        .map(|w| w.is_visible().unwrap_or(false))
        .unwrap_or(false);
    let next = !visible;
    set_widget_visible(&app, next);
    if let Some(h) = app.state::<AppState>().tray.lock_or_recover().as_ref() {
        let _ = h.widget_toggle.set_checked(next);
    }
    next
}

/// 悬浮窗 ✕ 按钮：关闭（隐藏）悬浮窗并写回设置、同步托盘勾选。
#[tauri::command]
pub(crate) fn close_widget(app: AppHandle) {
    set_widget_visible(&app, false);
    if let Some(h) = app.state::<AppState>().tray.lock_or_recover().as_ref() {
        let _ = h.widget_toggle.set_checked(false);
    }
}

/// 贴边收起模式下，鼠标悬停展开 / 离开收起。
#[tauri::command]
pub(crate) fn set_widget_collapsed(app: AppHandle, collapsed: bool) {
    let prefs = {
        app.state::<AppState>()
            .settings
            .lock_or_recover()
            .widget
            .clone()
    };
    if let Some(win) = app.get_webview_window("widget") {
        if collapsed && prefs.edge_snap {
            collapse_widget(&app, &win);
        } else if !prefs.edge_snap || !collapsed {
            expand_widget(&app, &win, prefs.compact);
        }
    }
}

/// 前端直接更新悬浮窗偏好（预留）。
#[tauri::command]
pub(crate) fn update_widget_settings(app: AppHandle, widget: WidgetPrefs) -> Result<(), AppError> {
    apply_widget_prefs(&app, &widget);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // 1080p 主屏、缩放 1.0、悬浮窗在左上角。
    const MON_POS: (i32, i32) = (0, 0);
    const MON_SIZE: (u32, u32) = (1920, 1080);

    fn target(anchor: (i32, i32)) -> Option<(&'static str, i32, i32, f64, f64)> {
        collapse_target(MON_POS, MON_SIZE, anchor, 1.0, false)
    }

    #[test]
    fn 离四边都远时不收起() {
        assert!(target((600, 400)).is_none(), "屏幕中间不该收起");
        // 离左边 9px 就超过阈值了，边界要卡在 8。
        assert!(target((9, 400)).is_none(), "9px 间隙不该收起");
    }

    #[test]
    fn 贴左边缘收成竖线() {
        let (edge, x, y, lw, lh) = target((0, 400)).expect("贴左边应收起");
        assert_eq!(edge, "left");
        assert_eq!(x, 0, "线应紧贴左边缘");
        assert_eq!((lw, lh), WIDGET_LINE_V, "左边缘用竖线");
        assert_eq!(y + (lh as i32) / 2, 400 + 108, "线中心应对齐悬浮窗中心");
    }

    #[test]
    fn 贴右边缘收成竖线并贴住屏幕右边() {
        let anchor = (1920 - WIDGET_NORMAL.0 as i32, 300);
        let (edge, x, _, lw, lh) = target(anchor).expect("贴右边应收起");
        assert_eq!(edge, "right");
        assert_eq!((lw, lh), WIDGET_LINE_V);
        assert_eq!(x + lw as i32, 1920, "线的右沿应贴住屏幕右沿");
    }

    #[test]
    fn 贴上边缘收成横线() {
        let (edge, x, y, lw, lh) = target((700, 0)).expect("贴上边应收起");
        assert_eq!(edge, "top");
        assert_eq!(y, 0, "线应紧贴上边缘");
        assert_eq!((lw, lh), WIDGET_LINE_H, "上下边缘用横线");
        assert_eq!(x + (lw as i32) / 2, 700 + 150, "线中心应对齐悬浮窗中心");
    }

    #[test]
    fn 贴下边缘收成横线并贴住屏幕下沿() {
        let anchor = (700, 1080 - WIDGET_NORMAL.1 as i32);
        let (edge, _, y, lw, lh) = target(anchor).expect("贴下边应收起");
        assert_eq!(edge, "bottom");
        assert_eq!((lw, lh), WIDGET_LINE_H);
        assert_eq!(y + lh as i32, 1080, "线的下沿应贴住屏幕下沿");
    }

    #[test]
    fn 角落同时贴两边时按左和上的先序取一条() {
        // 左上角：四条间隙都是 0，按 left→right→top→bottom 的顺序取 left。
        assert_eq!(target((0, 0)).map(|t| t.0), Some("left"));
    }

    #[test]
    fn 缩略形态用缩略高度算间隙() {
        // 同一个锚点，正常形态下沿到 1076（离屏幕下沿 4px），缩略形态只到 936。
        let anchor = (700, 860);
        let normal =
            collapse_target(MON_POS, MON_SIZE, anchor, 1.0, false).expect("正常形态贴下沿应收起");
        assert_eq!(normal.0, "bottom");
        assert!(
            collapse_target(MON_POS, MON_SIZE, anchor, 1.0, true).is_none(),
            "缩略形态没那么高，还不算贴边"
        );
    }

    #[test]
    fn 线不会因为锚点越界而被推到屏幕外() {
        // 锚点已经顶到屏幕上沿：线仍必须落在屏幕内。
        let (_, _, y, _, lh) = target((700, -400)).expect("应算出吸附位置");
        assert!(y >= 0, "线不该跑到屏幕上方: {y}");
        assert!(y + lh as i32 <= 1080, "线不该跑到屏幕下方");
    }
}
