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
use crate::error::AppError;
use crate::sync::MutexExt;
use crate::AppState;

/// 当前备份格式版本；v2 起包含窗口标题与网页区间，旧备份仍可恢复（缺省为空）。
const BACKUP_VERSION: u32 = 2;

#[derive(Debug, Serialize, Deserialize)]
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
pub(crate) fn export_backup_json(app: AppHandle) -> Result<String, AppError> {
    let state = app.state::<AppState>();
    let data_dir = state.settings.lock_or_recover().data_directory.clone();
    let (daily, intervals, states, titles, web) = {
        let st = state.store.lock_or_recover();
        (
            st.query_daily("0000-01-01", "9999-12-31")
                .map_err(|e| AppError::new("db", e.to_string()))?,
            st.query_intervals("0000-01-01", "9999-12-31")
                .map_err(|e| AppError::new("db", e.to_string()))?,
            st.query_state_intervals("0000-01-01", "9999-12-31")
                .map_err(|e| AppError::new("db", e.to_string()))?,
            st.query_title_intervals("0000-01-01", "9999-12-31")
                .map_err(|e| AppError::new("db", e.to_string()))?,
            st.query_web_intervals("0000-01-01", "9999-12-31")
                .map_err(|e| AppError::new("db", e.to_string()))?,
        )
    };
    let dir = backups_dir(&data_dir);
    std::fs::create_dir_all(&dir)?;
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
    std::fs::write(&path, serde_json::to_string(&file)?)?;
    Ok(path.to_string_lossy().to_string())
}

/// 解析备份名到 backups 目录内的 .json 文件，拒绝路径逃逸与非法文件名。
fn backup_path(data_dir: &str, name: &str) -> Result<std::path::PathBuf, AppError> {
    if name.is_empty()
        || name.contains("..")
        || name.contains('/')
        || name.contains('\\')
        || name.contains(':')
        || !name.to_lowercase().ends_with(".json")
    {
        return Err(AppError::new("backup-name", "非法备份名"));
    }
    let dir = backups_dir(data_dir);
    let path = dir.join(name);
    if path.parent() != Some(dir.as_path()) {
        return Err(AppError::new("backup-name", "非法备份名"));
    }
    Ok(path)
}

/// 删除指定备份文件（防路径逃逸：仅允许 backups 目录内的 json）。
#[tauri::command]
pub(crate) fn delete_backup(app: AppHandle, name: String) -> Result<(), AppError> {
    let state = app.state::<AppState>();
    let data_dir = state.settings.lock_or_recover().data_directory.clone();
    let path = backup_path(&data_dir, &name)?;
    std::fs::remove_file(&path)?;
    Ok(())
}

/// 解析备份文本：格式对不对、版本能不能落。
///
/// 与写库分开是为了能单独测：恢复最怕的不是写失败，而是把不认识的备份当成
/// 空备份写进去，把用户现有的数据清掉。
fn parse_backup(text: &str) -> Result<BackupFile, AppError> {
    let backup: BackupFile = serde_json::from_str(text).map_err(|e| {
        AppError::new(
            "backup-format",
            format!("备份文件不是有效的应用备份（{e}）"),
        )
    })?;
    if backup.version > BACKUP_VERSION {
        return Err(AppError::new(
            "backup-version",
            format!(
                "备份文件版本 {} 高于当前支持的版本 {}，请升级应用后再恢复",
                backup.version, BACKUP_VERSION
            ),
        ));
    }
    Ok(backup)
}

/// 共用恢复逻辑：清空当前库与内存态 → 写入备份 → 通知所有视图重新拉数据。
fn apply_backup(app: &AppHandle, text: &str, label: &str) -> Result<String, AppError> {
    let backup = parse_backup(text)?;
    let state = app.state::<AppState>();
    // 先清内存态再写库：写库期间监控线程仍会 flush，若内存里还留着旧数据，
    // 会和刚恢复的行混在一起。清空后 flush 只会补上新的增量。
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
    {
        let st = state.store.lock_or_recover();
        // 五张表在同一个事务里清空并写回，避免上万条明细逐条隐式提交长时间占用 store 锁
        st.restore_replace(
            &backup.daily,
            &backup.intervals,
            &backup.states,
            &backup.titles,
            &backup.web,
        )
        .map_err(|e| AppError::new("db", e.to_string()))?;
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
pub(crate) fn open_backup_dir(app: AppHandle) -> Result<(), AppError> {
    let data_dir = app
        .state::<AppState>()
        .settings
        .lock_or_recover()
        .data_directory
        .clone();
    let dir = backups_dir(&data_dir);
    std::fs::create_dir_all(&dir)?;
    open_path(dir.to_string_lossy().to_string())
}

/// 恢复数据目录 backups/ 里指定的备份。
#[tauri::command]
pub(crate) fn restore_backup_named(app: AppHandle, name: String) -> Result<String, AppError> {
    let data_dir = app
        .state::<AppState>()
        .settings
        .lock_or_recover()
        .data_directory
        .clone();
    let path = backup_path(&data_dir, &name)?;
    let text = std::fs::read_to_string(&path)?;
    apply_backup(&app, &text, &name)
}

/// 恢复从任意位置选择的备份 JSON（前端读取文件内容后传入）。
#[tauri::command]
pub(crate) fn restore_backup_content(app: AppHandle, content: String) -> Result<String, AppError> {
    apply_backup(&app, &content, "所选文件")
}

#[tauri::command]
pub(crate) fn import_backup_json(app: AppHandle) -> Result<String, AppError> {
    let data_dir = app
        .state::<AppState>()
        .settings
        .lock_or_recover()
        .data_directory
        .clone();
    let dir = backups_dir(&data_dir);
    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(&dir)
        .map_err(|e| AppError::new("backup-missing", format!("没有可导入的备份: {e}")))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().map(|e| e == "json").unwrap_or(false))
        .collect();
    files.sort();
    let path = files
        .pop()
        .ok_or_else(|| AppError::new("backup-missing", "没有可导入的备份"))?;
    let text = std::fs::read_to_string(&path)?;
    let label = path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    apply_backup(&app, &text, &label)
}

#[tauri::command]
pub(crate) fn list_backups(app: AppHandle) -> Vec<String> {
    let state = app.state::<AppState>();
    let data_dir = state.settings.lock_or_recover().data_directory.clone();
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

#[cfg(test)]
mod tests {
    use super::*;

    fn good_backup(version: u32) -> String {
        format!(
            r#"{{"version":{version},"exported_at":"2026-01-01 00:00:00","daily":[{{"usage_date":"2026-01-01","app_key":"c:/a.exe","process_name":"a","display_name":"A","exe_path":"C:/a.exe","active_seconds":12.5}}],"intervals":[],"states":[],"titles":[],"web":[]}}"#
        )
    }

    #[test]
    fn 正常备份能读出内容与版本() {
        let b = parse_backup(&good_backup(BACKUP_VERSION)).expect("应能解析");
        assert_eq!(b.version, BACKUP_VERSION, "版本应原样读回");
        assert_eq!(b.daily.len(), 1, "日聚合应读出一条");
        assert_eq!(b.daily[0].app_key, "c:/a.exe");
        assert_eq!(b.daily[0].active_seconds, 12.5);
    }

    #[test]
    fn 版本高于当前支持的备份被拒绝而不是当空备份() {
        let err = parse_backup(&good_backup(BACKUP_VERSION + 1)).expect_err("高版本应被拒绝");
        assert_eq!(err.code, "backup-version", "应给出可判定的 code");
        assert!(
            err.message.contains("请升级应用"),
            "应告诉用户怎么办，实际: {}",
            err.message
        );
    }

    #[test]
    fn 不是备份的文件不会被当成空备份() {
        for bad in ["not json", "{}", r#"{"daily":"x"}"#] {
            let err = parse_backup(bad).expect_err("坏文件应被拒绝: {bad}");
            assert_eq!(err.code, "backup-format", "输入 {bad} 应报格式错误");
        }
    }

    #[test]
    fn 旧备份缺省字段时按空处理() {
        // v0 的备份没有 titles/web 这些字段，恢复时不能因此报错。
        let text = r#"{"version":0,"daily":[]}"#;
        let b = parse_backup(text).expect("旧备份应能解析");
        assert_eq!(b.version, 0);
        assert!(b.intervals.is_empty() && b.titles.is_empty() && b.web.is_empty());
    }

    #[test]
    fn 非法备份名一律拒绝() {
        let dir = "C:/data";
        for bad in [
            "",
            "../evil.json",
            "a/b.json",
            "a\\b.json",
            "c:evil.json",
            "backup.txt",
        ] {
            let err = backup_path(dir, bad).expect_err("应拒绝: {bad}");
            assert_eq!(err.code, "backup-name", "输入 {bad:?} 应报非法备份名");
        }
    }

    #[test]
    fn 正常备份名落在备份目录内() {
        let path = backup_path("C:/data", "backup_20260101_000000.json").expect("合法名字应通过");
        assert!(path.starts_with("C:/data"), "必须落在数据目录下: {path:?}");
        assert!(
            path.to_string_lossy().contains("backups"),
            "必须落在 backups 子目录: {path:?}"
        );
    }
}
