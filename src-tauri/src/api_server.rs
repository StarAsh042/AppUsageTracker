//! 本地 HTTP API：127.0.0.1 上的只读统计接口与浏览器扩展心跳回传。
//!
//! 从 lib.rs 整体搬出（拆分第六步），行为不变。
use std::sync::atomic::Ordering;
use std::sync::mpsc;

use chrono::Local;
use tauri::{AppHandle, Emitter, Manager};
use tracker_core::models::WebInterval;

use crate::dashboard::build_dashboard;
use crate::live::live_status;
use crate::sync::MutexExt;
use crate::timeline::timeline_data;
use crate::AppState;

static API_GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// 处理连接的固定线程数。本地只读接口，四个足够，线程数是常数不随连接数涨。
const API_WORKERS: usize = 4;
/// 排队等处理的连接上限；超了直接回 503，不再堆线程。
const API_QUEUE: usize = 8;

/// （重）启本地 API 服务线程；设置里关闭/改端口后自动退出旧线程。
pub(crate) fn spawn_api_server(app: &AppHandle) {
    let (enabled, port) = {
        let st = app.state::<AppState>();
        let s = st.settings.lock_or_recover();
        (s.api_enabled, s.api_port)
    };
    {
        let st = app.state::<AppState>();
        let mut last = st.api_last.lock_or_recover();
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
                log::error!("本地 API 启动失败({addr}): {e}");
                return;
            }
        };
        let _ = app.emit("api://started", &addr);

        // 固定大小的处理池：连接进有界队列，队列满了让新连接吃 503 后断开。
        // 这样外部连多少端口，本进程的线程数都是常数。
        let worker_app = app.clone();
        let tx = spawn_workers(API_WORKERS, API_QUEUE, move |stream| {
            handle_api(&worker_app, stream)
        });
        accept_loop(listener, tx, move || should_stop(gen));
    });
}

/// 起固定数量的处理线程，从有界队列里取连接。返回投递端。
///
/// 队列用 sync_channel 而不是给每个连接 spawn 线程：线程数只由 workers 决定，
/// 外部开多少连接都改变不了它。
fn spawn_workers<F>(
    workers: usize,
    queue: usize,
    handler: F,
) -> mpsc::SyncSender<std::net::TcpStream>
where
    F: Fn(std::net::TcpStream) + Send + Sync + 'static,
{
    let (tx, rx) = mpsc::sync_channel::<std::net::TcpStream>(queue);
    let rx = std::sync::Arc::new(std::sync::Mutex::new(rx));
    let handler = std::sync::Arc::new(handler);
    for _ in 0..workers {
        let rx = rx.clone();
        let handler = handler.clone();
        std::thread::spawn(move || loop {
            let next = rx.lock_or_recover().recv();
            let Ok(stream) = next else { break };
            let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(2)));
            handler(stream);
        });
    }
    tx
}

/// accept_loop 的停止条件：只认「自己的代际已经不是当前代际」。
///
/// 单独成函数是为了能测。「刚启动时代际就等于自己」这个前提一旦写反，
/// 线程会立刻退出，端口从头到尾没监听，而且一行日志都不留。
fn should_stop(gen: u64) -> bool {
    API_GEN.load(Ordering::SeqCst) != gen
}

/// 接受连接并投递给处理池；队列满时直接回 503 然后断开。
/// should_stop 返回 true 时退出（设置里关掉 API 或换端口）。
///
/// accept 保持阻塞模式：空闲时这个线程真的睡着，不像轮询那样每 300 毫秒醒一次。
/// 关停靠一个「自己连自己」的唤醒连接把 accept 打断，醒来先看标志再决定。
fn accept_loop(
    listener: std::net::TcpListener,
    tx: mpsc::SyncSender<std::net::TcpStream>,
    should_stop: impl Fn() -> bool + Send + Sync + 'static,
) {
    let should_stop = std::sync::Arc::new(should_stop);
    if let Ok(addr) = listener.local_addr() {
        let flag = should_stop.clone();
        std::thread::spawn(move || {
            while !flag() {
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
            let _ = std::net::TcpStream::connect(addr);
        });
    }
    while !should_stop() {
        match listener.accept() {
            Ok((stream, _)) => {
                // 唤醒用的那个连接只用来打断 accept，看到关停标志就直接走。
                if should_stop() {
                    break;
                }
                if let Err(e) = tx.try_send(stream) {
                    match e {
                        mpsc::TrySendError::Full(mut stream) => {
                            let _ =
                                stream.set_write_timeout(Some(std::time::Duration::from_secs(1)));
                            api_json(&mut stream, 503, error_body("server busy"), None);
                        }
                        mpsc::TrySendError::Disconnected(_) => break,
                    }
                }
            }
            Err(_) => {
                std::thread::sleep(std::time::Duration::from_millis(300));
            }
        }
    }
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
    let mut open = state.web_open.lock_or_recover();
    let extend = matches!(open.as_ref(), Some((d, t, _)) if *d == domain && *t == title);
    if !extend {
        if let Some((d, t, start)) = open.take() {
            let dur = (now - start).num_milliseconds() as f64 / 1000.0;
            if dur >= 5.0 {
                state.web_pending.lock_or_recover().push(WebInterval {
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

/// 浏览器扩展自己的来源。带 host_permissions 的扩展请求本来就不受 CORS 约束，
/// 这里列出它们，是为了让「其余来源一律拒绝」这条规则不靠推断成立。
fn is_extension_origin(origin: &str) -> bool {
    let o = origin.trim().to_ascii_lowercase();
    o.starts_with("chrome-extension://") || o.starts_with("moz-extension://")
}

/// 来源闸门的判定结果。
#[derive(Debug, PartialEq, Eq)]
enum OriginGate {
    /// 不带 Origin 的本地客户端（curl、第三方工具），或扩展访问自己的写入端点。
    Allowed,
    /// 拒绝，原因是给日志看的短句。
    Reject(&'static str),
}

/// 判断这次请求的来源与路径组合是否放行。
///
/// 浏览器发请求一定会带 Origin，所以任意网页都过不了第一道；扩展来源只写心跳，
/// 统计读取留给本地客户端 —— 扩展自己并不读统计，放开等于让任何装了扩展的
/// 浏览器读到窗口标题与域名。
fn check_origin(origin: Option<&str>, path: &str) -> OriginGate {
    match origin {
        None => OriginGate::Allowed,
        Some(o) if !is_extension_origin(o) => OriginGate::Reject("origin not allowed"),
        Some(_) if path != "/api/web-heartbeat" => {
            OriginGate::Reject("origin may only post heartbeats")
        }
        Some(_) => OriginGate::Allowed,
    }
}

/// 错误响应体。用 json! 拼，错误消息里带引号或换行也不会拼出非法 JSON。
fn error_body(message: &str) -> String {
    serde_json::json!({ "error": message }).to_string()
}

/// 序列化响应体。失败时给 500 而不是空 body：空 body 会被客户端当成
/// 「今天没有数据」，把一次失败读成一条正常结论。
fn serialize_body<T: serde::Serialize>(value: &T) -> (u16, String) {
    match serde_json::to_string(value) {
        Ok(body) => (200, body),
        Err(e) => (500, error_body(&format!("序列化失败: {e}"))),
    }
}

fn api_json(stream: &mut std::net::TcpStream, code: u16, body: String, allow_origin: Option<&str>) {
    let reason = match code {
        200 => "OK",
        204 => "No Content",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "OK",
    };
    let mut head = format!(
        "HTTP/1.1 {code} {reason}\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n",
        body.len()
    );
    // 只在来源被放行时回 CORS 头：回 "*" 等于允许任意网页读取本机统计。
    if let Some(origin) = allow_origin {
        head.push_str(&format!("Access-Control-Allow-Origin: {origin}\r\n"));
        head.push_str("Vary: Origin\r\n");
    }
    head.push_str("\r\n");
    use std::io::Write;
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(body.as_bytes());
    let _ = stream.flush();
}

/// 预检响应：只放行扩展来源，且只允许读写统计所需的少数方法。
fn api_preflight(stream: &mut std::net::TcpStream, allow_origin: Option<&str>) {
    let mut head = String::from("HTTP/1.1 204 No Content\r\n");
    if let Some(origin) = allow_origin {
        head.push_str(&format!("Access-Control-Allow-Origin: {origin}\r\n"));
        head.push_str("Vary: Origin\r\n");
        head.push_str("Access-Control-Allow-Methods: GET, POST, OPTIONS\r\n");
        head.push_str("Access-Control-Allow-Headers: content-type\r\n");
        head.push_str("Access-Control-Max-Age: 600\r\n");
    }
    head.push_str("Content-Length: 0\r\nConnection: close\r\n\r\n");
    use std::io::Write;
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.flush();
}

/// 响应一个可序列化的值；序列化失败自动降级成 500。
fn api_serialized<T: serde::Serialize>(
    stream: &mut std::net::TcpStream,
    allow_origin: Option<&str>,
    value: &T,
) {
    let (code, body) = serialize_body(value);
    api_json(stream, code, body, allow_origin);
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

    // 来源闸门：浏览器一定带 Origin，非扩展来源直接 403，跨源读取与
    // text/plain 伪装的写请求都在这一层被挡掉。不带 Origin 的本地客户端
    // （curl、第三方工具）照旧放行。
    let origin = head.lines().find_map(|line| {
        let (k, v) = line.split_once(':')?;
        k.trim()
            .eq_ignore_ascii_case("origin")
            .then(|| v.trim().to_string())
    });
    let allow = match check_origin(origin.as_deref(), &path) {
        OriginGate::Allowed => origin.as_deref(),
        OriginGate::Reject(reason) => {
            log::warn!("拒绝请求来源: {method} {path} ({reason})");
            return api_json(&mut stream, 403, error_body(reason), None);
        }
    };
    if method == "OPTIONS" {
        return api_preflight(&mut stream, allow);
    }

    match path.as_str() {
        "/api/web-heartbeat" if method == "POST" => {
            let (api_on, record) = {
                let st = app.state::<AppState>();
                let s = st.settings.lock_or_recover();
                (s.api_enabled, s.record_web)
            };
            if !api_on || !record {
                return api_json(&mut stream, 403, error_body("web tracking disabled"), allow);
            }
            #[derive(serde::Deserialize)]
            struct Hb {
                url: String,
                #[serde(default)]
                title: String,
            }
            let Ok(hb) = serde_json::from_str::<Hb>(body.trim()) else {
                return api_json(&mut stream, 400, error_body("bad json"), allow);
            };
            match extract_domain(&hb.url) {
                Some(domain) => {
                    heartbeat_web(app, domain, hb.title);
                    let interval = app
                        .state::<AppState>()
                        .settings
                        .lock_or_recover()
                        .web_heartbeat_interval_secs;
                    api_json(
                        &mut stream,
                        200,
                        serde_json::json!({ "ok": true, "interval": interval }).to_string(),
                        allow,
                    );
                }
                None => api_json(&mut stream, 400, error_body("bad url"), allow),
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
                let s = st.store.lock_or_recover();
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
                    api_serialized(&mut stream, allow, &serde_json::Value::Array(items));
                }
                Err(e) => api_json(&mut stream, 500, error_body(&e), allow),
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
            allow,
        ),
        "/api/live" => api_serialized(&mut stream, allow, &live_status(app)),
        "/api/dashboard" => {
            let today = Local::now().format("%Y-%m-%d").to_string();
            let from = qs("from").unwrap_or_else(|| today.clone());
            let to = qs("to").unwrap_or(today);
            match build_dashboard(app, &from, &to) {
                Ok(d) => api_serialized(&mut stream, allow, &d),
                Err(e) => api_json(&mut stream, 400, error_body(&e.message), allow),
            }
        }
        "/api/timeline" => {
            let Some(date) = qs("date") else {
                return api_json(&mut stream, 400, error_body("missing date"), allow);
            };
            match timeline_data(app, &date) {
                Ok(d) => api_serialized(&mut stream, allow, &d),
                Err(e) => api_json(&mut stream, 400, error_body(&e.message), allow),
            }
        }
        "/api/settings" => {
            let st = app.state::<AppState>();
            let s = st.settings.lock_or_recover().clone();
            api_serialized(&mut stream, allow, &s);
        }
        "/api/version" => api_json(
            &mut stream,
            200,
            serde_json::json!({ "version": app.package_info().version.to_string() }).to_string(),
            allow,
        ),
        _ => api_json(&mut stream, 404, error_body("not found"), allow),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 刻意序列化失败的载体，用来验证 D7 的降级路径。
    struct Broken;
    impl serde::Serialize for Broken {
        fn serialize<S: serde::Serializer>(&self, _s: S) -> Result<S::Ok, S::Error> {
            Err(<S::Error as serde::ser::Error>::custom("boom"))
        }
    }

    #[test]
    fn 网页来源一律不放行() {
        assert!(!is_extension_origin("https://evil.example"));
        assert!(!is_extension_origin("http://127.0.0.1:59598"));
        assert!(!is_extension_origin("null"));
        assert!(is_extension_origin("chrome-extension://abcdefghijklmnop"));
        assert!(is_extension_origin(" Moz-Extension://abc "));
    }

    #[test]
    fn 网页来源读统计被拒绝() {
        assert_eq!(
            check_origin(Some("https://evil.example"), "/api/settings"),
            OriginGate::Reject("origin not allowed")
        );
        assert_eq!(
            check_origin(Some("null"), "/api/dashboard"),
            OriginGate::Reject("origin not allowed")
        );
    }

    #[test]
    fn 扩展只能写心跳不能读统计() {
        let ext = "chrome-extension://abcdefghijklmnop";
        assert_eq!(
            check_origin(Some(ext), "/api/web-heartbeat"),
            OriginGate::Allowed
        );
        assert_eq!(
            check_origin(Some(ext), "/api/settings"),
            OriginGate::Reject("origin may only post heartbeats")
        );
        assert_eq!(
            check_origin(Some(ext), "/api/timeline"),
            OriginGate::Reject("origin may only post heartbeats")
        );
    }

    #[test]
    fn 本地客户端带不带来源都按同一套规则() {
        assert_eq!(check_origin(None, "/api/settings"), OriginGate::Allowed);
        assert_eq!(
            check_origin(None, "/api/web-heartbeat"),
            OriginGate::Allowed
        );
    }

    #[test]
    fn 序列化失败返回五百而不是空body() {
        let (code, body) = serialize_body(&Broken);
        assert_eq!(code, 500, "序列化失败必须是 500，不能当 200 处理");
        let v: serde_json::Value = serde_json::from_str(&body).expect("错误响应必须是合法 JSON");
        assert!(v.get("error").is_some(), "错误响应要有 error 字段");
    }

    #[test]
    fn 错误消息里的引号换行不破坏json() {
        let msg = r#"column "ActiveSeconds" 不存在
第二行"#;
        let body = error_body(msg);
        let v: serde_json::Value = serde_json::from_str(&body).expect("错误响应必须可解析");
        assert_eq!(v["error"], serde_json::json!(msg));
    }

    #[test]
    fn 放行才回_cors_头拒绝不回() {
        use std::io::{Read, Write};

        let read_response = |allow: Option<&str>| -> String {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
            let addr = listener.local_addr().expect("addr");
            let allow = allow.map(|s| s.to_string());
            let server = std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().expect("accept");
                api_json(&mut stream, 200, error_body("ok"), allow.as_deref());
            });
            let mut client = std::net::TcpStream::connect(addr).expect("connect");
            client
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .expect("timeout");
            let mut buf = Vec::new();
            let _ = client.read_to_end(&mut buf);
            let _ = client.write_all(b"");
            server.join().expect("server");
            String::from_utf8_lossy(&buf).to_string()
        };

        let rejected = read_response(None);
        assert!(
            !rejected
                .to_lowercase()
                .contains("access-control-allow-origin"),
            "不写 CORS 头，跨源方就读不到响应体"
        );
        let allowed = read_response(Some("chrome-extension://abc"));
        assert!(
            allowed.contains("Access-Control-Allow-Origin: chrome-extension://abc"),
            "放行的来源要拿到自己的来源回显，而不是通配符"
        );
        assert!(!allowed.contains("*"), "任何情况下都不该回通配符");
    }

    #[test]
    fn 正常值仍然返回两百() {
        let (code, body) = serialize_body(&serde_json::json!({ "ok": true }));
        assert_eq!(code, 200);
        assert_eq!(body, r#"{"ok":true}"#);
    }

    #[test]
    fn 队列满时直接回五百零三() {
        use std::io::Read;
        use std::sync::atomic::{AtomicBool, Ordering};

        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("local addr");
        // 容量 1 的队列且不放 worker：第一个连接占满队列，第二个必然挤不进去。
        let (tx, rx) = mpsc::sync_channel::<std::net::TcpStream>(1);
        let stop = std::sync::Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let handle = std::thread::spawn(move || {
            accept_loop(listener, tx, move || flag.load(Ordering::Relaxed))
        });

        let first = std::net::TcpStream::connect(addr).expect("connect 1");
        let mut second = std::net::TcpStream::connect(addr).expect("connect 2");
        second
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .expect("read timeout");
        let mut buf = [0u8; 256];
        let n = second.read(&mut buf).expect("第二个连接应当收到响应");
        let text = String::from_utf8_lossy(&buf[..n]).to_string();
        assert!(
            text.starts_with("HTTP/1.1 503"),
            "队列满时必须回 503，实际: {text}"
        );
        assert!(
            !text.contains("Access-Control-Allow-Origin"),
            "拒绝路径不该带 CORS 头"
        );

        stop.store(true, Ordering::Relaxed);
        drop(first);
        drop(rx);
        handle.join().expect("accept loop 应当退出");
    }

    #[test]
    fn 刚启动的代际继续监听被顶掉之后才退出() {
        let gen = API_GEN.fetch_add(1, Ordering::SeqCst) + 1;
        assert!(
            !should_stop(gen),
            "刚把自己记成当前代际，这时端口必须继续监听"
        );

        API_GEN.fetch_add(1, Ordering::SeqCst);
        assert!(should_stop(gen), "被下一次启动顶掉的旧代际必须退出");
    }
}
