//! 通用 IPC 命令：清空全部数据、导出 CSV、用资源管理器打开路径、读应用版本。
//!
//! 从 lib.rs 整体搬出（拆分第十六步），行为不变。

use tauri::{AppHandle, Emitter, State};
use tracker_core::fmt::{csv_escape, fmt_hms};

use crate::sync::MutexExt;
use crate::AppState;

#[tauri::command]
pub(crate) fn clear_all_data(app: AppHandle, state: State<AppState>) -> Result<(), String> {
    {
        let st = state.store.lock_or_recover();
        st.clear_all().map_err(|e| e.to_string())?;
    }
    {
        let mut agg = state.aggregator.lock_or_recover();
        agg.daily.clear();
        agg.pending_intervals.clear();
        agg.pending_state_intervals.clear();
        agg.pending_title_intervals.clear();
    }
    {
        state.web_open.lock_or_recover().take();
        state.web_pending.lock_or_recover().clear();
    }
    state.notified_limits.lock_or_recover().clear();
    let _ = app.emit("app://data", ());
    Ok(())
}

#[tauri::command]
pub(crate) fn export_csv(app: AppHandle, from: String, to: String) -> Result<String, String> {
    let data = crate::dashboard::build_dashboard(&app, &from, &to)?;
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

/// 在资源管理器里打开目录，或定位到某个文件（Windows 下走 `explorer /select,`）。
#[tauri::command]
pub(crate) fn open_path(path: String) -> Result<(), String> {
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
pub(crate) fn get_app_version(app: AppHandle) -> String {
    app.package_info().version.to_string()
}
