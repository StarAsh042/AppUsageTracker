// 报表与维护：数据库维护（完整性检查 + VACUUM + WAL 截断）、周报导出、
// 以及「Top 标题 / Top 网页」两个排行榜。从 lib.rs 整体搬出，行为不变。

use chrono::Local;
use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::dashboard::build_dashboard;
use crate::{fmt_hms, AppState};

#[derive(Serialize, Clone)]
pub(crate) struct DbMaintenance {
    pub(crate) integrity: String,
    pub(crate) vacuum_ok: bool,
    pub(crate) page_count: i64,
    pub(crate) freelist: i64,
    pub(crate) page_size: i64,
    /// VACUUM 之后又把 WAL 截断了一次：-wal 在没有它的情况下可能比主库还大。
    pub(crate) checkpoint_ok: bool,
    /// VACUUM + checkpoint 之后的主库字节数，用于在设置页给出可对比的数字。
    pub(crate) db_bytes: i64,
}

#[tauri::command]
pub(crate) fn db_maintenance(state: State<AppState>) -> Result<DbMaintenance, String> {
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

#[tauri::command]
pub(crate) fn export_weekly_report(app: AppHandle) -> Result<String, String> {
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

#[derive(Serialize, Clone)]
pub(crate) struct TopTitle {
    pub(crate) app_key: String,
    pub(crate) display_name: String,
    pub(crate) category: String,
    pub(crate) title: String,
    pub(crate) seconds: f64,
}

#[tauri::command]
pub(crate) fn get_top_titles(
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

#[derive(Serialize, Clone)]
pub(crate) struct TopWeb {
    pub(crate) domain: String,
    pub(crate) title: String,
    pub(crate) seconds: f64,
}

#[tauri::command]
pub(crate) fn get_top_web(
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
