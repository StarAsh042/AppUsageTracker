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

use tracker_core::fmt::fmt_hms;

use crate::live::live_status;
use crate::sync::MutexExt;
use crate::{version_info, AppState};

/// 连续落库失败次数。归零时说明上一轮已经写过一次成功的。
static FLUSH_FAILURES: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
/// 连续失败到这个次数才弹一次通知：偶发的一次写失败不值得打扰。
const FLUSH_NOTIFY_AFTER: u32 = 3;

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
                let v = st.settings.lock_or_recover().foreground_poll_ms;
                v.clamp(250, 10_000)
            };
            std::thread::sleep(std::time::Duration::from_millis(poll_ms));

            let now = Local::now();
            let tick = tracker_core::win32::get_tick_count64();
            let wall_delta = (now - last_wall).num_milliseconds();
            let tick_delta = (tick - last_tick) as i64;

            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let state = app.state::<AppState>();
                {
                    let mut agg = state.aggregator.lock_or_recover();
                    if wall_delta - tick_delta > 3000 {
                        // 系统睡眠过：把睡眠起点之前记为活动，其余丢弃
                        let suspend_at =
                            last_wall + chrono::Duration::milliseconds(tick_delta.max(0));
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
                    if state.user_paused.load(Ordering::Relaxed)
                        && agg.state != TrackingState::Paused
                    {
                        agg.pause(now);
                    }
                }

                // 窗口标题记录（隐私开关；仅在活跃态记录，排除模式命中即丢弃）
                {
                    let (record, patterns) = {
                        let s = state.settings.lock_or_recover();
                        (s.record_window_titles, s.title_exclude_patterns.clone())
                    };
                    if record {
                        let (app_key, title) = {
                            let agg = state.aggregator.lock_or_recover();
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
                        let mut agg = state.aggregator.lock_or_recover();
                        agg.on_title_tick(feed, now);
                    }
                }

                // 休息提醒：连续活跃累计，空闲即视为已休息
                let rest_minutes = state.settings.lock_or_recover().rest_reminder_minutes;
                {
                    let agg = state.aggregator.lock_or_recover();
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
                        .body(t!(
                            "已连续活跃使用 %{mins} 分钟，站起来活动一下吧",
                            mins = rest_minutes
                        ))
                        .show();
                }

                // 跨天：flush + 清限额提醒标记 + 保留策略清理
                let today = now.format("%Y-%m-%d").to_string();
                if today != cur_day {
                    cur_day = today;
                    flush_state(&app);
                    state.notified_limits.lock_or_recover().clear();
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
                    if let Err(e) = state.store.lock_or_recover().checkpoint() {
                        log::error!("periodic checkpoint failed: {e}");
                    }
                }
                check_limits(&app);

                let status = live_status(&app);
                let _ = app.emit("usage://tick", &status);
                // 托盘 tooltip 实时显示今日活跃与当前应用
                let tray_icon = state.tray_icon.lock_or_recover();
                if let Some(tray) = tray_icon.as_ref() {
                    let tip = match &status.current_app {
                        Some(a) => format!(
                            "{}\n{}",
                            t!("软件使用时间统计"),
                            t!(
                                "今日 %{total} · %{app}",
                                total = fmt_hms(status.today_total_seconds),
                                app = a.display_name
                            )
                        ),
                        None => format!(
                            "{}\n{}",
                            t!("软件使用时间统计"),
                            t!("今日 %{total}", total = fmt_hms(status.today_total_seconds))
                        ),
                    };
                    let _ = tray.set_tooltip(Some(tip));
                }
            }));
            if outcome.is_err() {
                // 单轮采样 panic 不该终结统计：捕获、告知界面、睡一拍再继续。
                // 线程一旦退出就再没人采样，界面数字会停在原地而用户看不出来。
                log::error!("监控线程本轮采样 panic，已捕获并继续");
                let _ = app.emit("monitor://panicked", ());
                std::thread::sleep(std::time::Duration::from_secs(1));
            }

            last_wall = now;
            last_tick = tick;
        }
    });
}

pub(crate) fn flush_state(app: &AppHandle) {
    let state = app.state::<AppState>();
    let daily: Vec<DailyUsage> = {
        let agg = state.aggregator.lock_or_recover();
        agg.daily.values().cloned().collect()
    };
    // 只拍快照、不出队：队列本身就是重试缓冲区。写成功之后才把写过的前缀
    // 摘掉；写失败则原样留着，下一轮整批重试。之前的写法是先 take 再写，
    // 一旦磁盘满或库被锁，这批数据就只剩日志里的一行了。
    let (intervals, state_intervals, title_intervals, web_intervals): (
        Vec<ForegroundInterval>,
        Vec<StateInterval>,
        Vec<TitleInterval>,
        Vec<WebInterval>,
    ) = {
        let agg = state.aggregator.lock_or_recover();
        let intervals = agg.pending_intervals.clone();
        let state_intervals = agg.pending_state_intervals.clone();
        let title_intervals = agg.pending_title_intervals.clone();
        // 网页心跳：闭合打开段（同键续记）后追加到待写队列
        let now2 = Local::now();
        let mut web_open = state.web_open.lock_or_recover();
        let mut web_pending = state.web_pending.lock_or_recover();
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
        (
            intervals,
            state_intervals,
            title_intervals,
            web_pending.clone(),
        )
    };

    if daily.is_empty()
        && intervals.is_empty()
        && state_intervals.is_empty()
        && title_intervals.is_empty()
        && web_intervals.is_empty()
    {
        return;
    }
    let written = state.store.lock_or_recover().save_flush(
        &daily,
        &intervals,
        &state_intervals,
        &title_intervals,
        &web_intervals,
    );
    match written {
        Ok(()) => {
            {
                let mut agg = state.aggregator.lock_or_recover();
                drop_front(&mut agg.pending_intervals, intervals.len());
                drop_front(&mut agg.pending_state_intervals, state_intervals.len());
                drop_front(&mut agg.pending_title_intervals, title_intervals.len());
            }
            drop_front(
                &mut state.web_pending.lock_or_recover(),
                web_intervals.len(),
            );
            if FLUSH_FAILURES.swap(0, Ordering::Relaxed) > 0 {
                log::info!("落库恢复正常");
            }
        }
        Err(e) => {
            let fails = FLUSH_FAILURES.fetch_add(1, Ordering::Relaxed) + 1;
            log::error!("save_flush failed (第 {fails} 次连续失败): {e}");
            if fails == FLUSH_NOTIFY_AFTER {
                let _ = app
                    .notification()
                    .builder()
                    .title(t!("统计数据写入失败"))
                    .body(t!(
                        "数据暂存在内存并会自动重试，请检查磁盘空间与数据目录权限。"
                    ))
                    .show();
            }
        }
    }
}

/// 把队列前缀 n 条摘掉；只有确认写进库了才调用。
fn drop_front<T>(queue: &mut Vec<T>, n: usize) {
    if n > 0 {
        let take = n.min(queue.len());
        queue.drain(..take);
    }
}

/// 保留策略：清理 N 天前的明细与聚合（0 = 永久保留）。
pub(crate) fn purge_expired(app: &AppHandle) {
    let state = app.state::<AppState>();
    let retention = state.settings.lock_or_recover().retention_days;
    if retention == 0 {
        return;
    }
    let cutoff = (Local::now() - chrono::Duration::days(retention as i64))
        .format("%Y-%m-%d")
        .to_string();
    let result = state.store.lock_or_recover().purge_before(&cutoff);
    if let Err(e) = result {
        log::error!("purge_before failed: {e}");
    }
}

pub(crate) fn check_limits(app: &AppHandle) {
    let state = app.state::<AppState>();
    let limits = {
        let s = state.settings.lock_or_recover();
        s.category_daily_limit_minutes.clone()
    };
    if limits.is_empty() {
        return;
    }
    let resolver = app.state::<AppState>().resolver.lock_or_recover().clone();
    let today = Local::now().format("%Y-%m-%d").to_string();
    let mut per_cat: HashMap<String, f64> = HashMap::new();
    {
        let agg = state.aggregator.lock_or_recover();
        for d in agg.daily.values().filter(|d| d.usage_date == today) {
            let cat = resolver.resolve(&d.display_name, &d.process_name, &d.exe_path);
            *per_cat.entry(cat).or_insert(0.0) += d.active_seconds;
        }
    }
    let to_notify = {
        let notified = state.notified_limits.lock_or_recover();
        categories_to_notify(&per_cat, &limits, &notified)
    };
    for cat in to_notify {
        state.notified_limits.lock_or_recover().insert(cat.clone());
        let _ = app.emit("usage://limit-reached", &cat);
        let _ = app
            .notification()
            .builder()
            .title(t!("分类限额提醒"))
            .body(t!("「%{cat}」今日使用时长已超过设定限额", cat = cat))
            .show();
    }
}

#[cfg(test)]
mod tests {
    use super::drop_front;

    #[test]
    fn 只摘掉写成功的前缀() {
        let mut q = vec![1, 2, 3, 4];
        drop_front(&mut q, 2);
        assert_eq!(q, vec![3, 4], "应只摘掉已写库的前两条");
        drop_front(&mut q, 0);
        assert_eq!(q, vec![3, 4], "0 条不该动队列");
        drop_front(&mut q, 9);
        assert!(q.is_empty(), "超过长度时清空，不越界 panic");
    }
}
