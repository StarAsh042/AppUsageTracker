//! 本地 HTTP API：127.0.0.1 上的只读统计接口与浏览器扩展心跳回传。
//!
//! 从 lib.rs 整体搬出（拆分第六步），行为不变。
use std::sync::atomic::Ordering;

use chrono::Local;
use tauri::{AppHandle, Emitter, Manager};
use tracker_core::models::WebInterval;

use crate::{build_dashboard, live_status, timeline_data, AppState};

static API_GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// （重）启本地 API 服务线程；设置里关闭/改端口后自动退出旧线程。
pub(crate) fn spawn_api_server(app: &AppHandle) {
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
