//! 监控线程：前台轮询、聚合落库、限额与保留策略。
//!
//! 从 lib.rs 整体搬出（拆分第四步），行为不变。
use std::collections::HashMap;
use std::sync::atomic::Ordering;

use chrono::Local;
use rust_i18n::t;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;
use tracker_core::limits::categories_to_notify;
use tracker_core::models::{
    AppIdentity, DailyUsage, ForegroundInterval, StateInterval, TitleInterval, TrackingState,
    WebInterval,
};

use crate::{fmt_hms, live_status, version_info, AppState};

/// 自身进程名（小写，不含扩展名），用于排除统计程序自己（含旧数据清理）。
pub(crate) fn self_stem() -> &'static str {
    static SELF: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    SELF.get_or_init(|| {
        std::env::current_exe()
            .ok()
            .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().to_lowercase()))
            .unwrap_or_else(|| "app-usage-tracker".to_string())
    })
}

pub(crate) fn foreground_identity(
    display_cache: &mut HashMap<String, String>,
) -> Option<AppIdentity> {
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

pub(crate) fn spawn_monitor(app: AppHandle) {
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

pub(crate) fn flush_state(app: &AppHandle) {
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

/// 保留策略：清理 N 天前的明细与聚合（0 = 永久保留）。
pub(crate) fn purge_expired(app: &AppHandle) {
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

pub(crate) fn check_limits(app: &AppHandle) {
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
