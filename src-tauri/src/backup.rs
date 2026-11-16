//! JSON 备份与恢复：整库导出、按名/按内容恢复、旧备份兼容。
//!
//! 从 lib.rs 整体搬出（拆分第七步），行为不变。
use chrono::Local;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use tracker_core::models::{
    DailyUsage, ForegroundInterval, StateInterval, TitleInterval, WebInterval,
};

use crate::commands::open_path;
use crate::AppState;

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
pub(crate) fn export_backup_json(app: AppHandle) -> Result<String, String> {
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
pub(crate) fn delete_backup(app: AppHandle, name: String) -> Result<(), String> {
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
pub(crate) fn open_backup_dir(app: AppHandle) -> Result<(), String> {
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
pub(crate) fn restore_backup_named(app: AppHandle, name: String) -> Result<String, String> {
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
pub(crate) fn restore_backup_content(app: AppHandle, content: String) -> Result<String, String> {
    apply_backup(&app, &content, "所选文件")
}

#[tauri::command]
pub(crate) fn import_backup_json(app: AppHandle) -> Result<String, String> {
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
pub(crate) fn list_backups(app: AppHandle) -> Vec<String> {
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
