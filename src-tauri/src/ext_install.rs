//! 浏览器扩展安装：内置扩展落盘、CRX3 打包、注册表注册与 17 款 Chromium 浏览器的智能安装。
//!
//! 从 lib.rs 整体搬出（拆分第五步），行为不变。
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::settings::save_settings_file;
use crate::AppState;

/// 「加载已解压的扩展程序」用的目录（数据目录内）。
/// 绝不去探测 exe 的相对路径：安装版会命中用户家目录下无关的 extension/ 目录。
fn extension_dir(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    let data_dir = {
        let state = app.state::<AppState>();
        let x = state.settings.lock().unwrap().data_directory.clone();
        x
    };
    Ok(std::path::Path::new(&data_dir)
        .join("browser-ext")
        .join("extension"))
}

/// 把内置扩展写到数据目录，返回可供浏览器手动加载的目录。
#[tauri::command]
pub(crate) fn get_extension_path(app: AppHandle) -> Result<String, String> {
    let dir = extension_dir(&app)?;
    materialize_extension(&dir)?;
    Ok(dir.to_string_lossy().to_string())
}

/// 单个浏览器的智能安装结果（前端逐条展示）。
#[derive(Serialize, Clone)]
pub(crate) struct ExtInstallResult {
    browser: String,
    name: String,
    ok: bool,
    /// ok / pack_failed / bad_crx / registry_failed（not_found 保留兼容旧前端）
    code: String,
    extension_id: String,
    message: String,
}

// CRX3 头部解析与扩展 ID 编码的真实实现搬到了 tracker-core 的 ext 模块（带单测），
// 这里只留薄封装，调用点不用改。
use tracker_core::ext::{
    crx_id_bytes as core_crx_id_bytes, extension_id_from_hash as core_extension_id,
};

/// CRX3 头部 → signed_header_data 里的 crx_id（= SHA256(公钥) 前 16 字节）。
/// 结构：magic "Cr24" | uint32 LE 版本(=3) | uint32 LE header 长度 | CrxFileHeader。
fn crx_id_bytes(crx: &[u8]) -> Option<Vec<u8>> {
    core_crx_id_bytes(crx)
}

/// 16 字节公钥哈希 → Chromium 扩展 ID（a-p 编码，32 字符）。
fn extension_id_from_hash(hash: &[u8]) -> String {
    core_extension_id(hash)
}

#[derive(Clone, Copy)]
struct BrowserSpec {
    key: &'static str,
    name: &'static str,
    /// 相对各安装基址的可执行文件路径（依次尝试）。
    exe_paths: &'static [&'static str],
    /// HKCU\Software 下的注册表根，外部扩展键写在 <root>\Extensions\<id>。
    /// 多数 Chromium fork 未改 install_static，因此同时写 Google\Chrome 兜底。
    registry_roots: &'static [&'static str],
    /// 扩展管理页。
    ext_page: &'static str,
}

fn browser_specs() -> Vec<BrowserSpec> {
    vec![
        BrowserSpec {
            key: "chrome",
            name: "Chrome",
            exe_paths: &[r"Google\Chrome\Application\chrome.exe"],
            registry_roots: &[r"Google\Chrome"],
            ext_page: "chrome://extensions",
        },
        BrowserSpec {
            key: "edge",
            name: "Edge",
            exe_paths: &[r"Microsoft\Edge\Application\msedge.exe"],
            registry_roots: &[r"Microsoft\Edge"],
            ext_page: "edge://extensions",
        },
        BrowserSpec {
            key: "chrome_beta",
            name: "Chrome Beta",
            exe_paths: &[r"Google\Chrome Beta\Application\chrome.exe"],
            registry_roots: &[r"Google\Chrome Beta", r"Google\Chrome"],
            ext_page: "chrome://extensions",
        },
        BrowserSpec {
            key: "chrome_dev",
            name: "Chrome Dev",
            exe_paths: &[r"Google\Chrome Dev\Application\chrome.exe"],
            registry_roots: &[r"Google\Chrome Dev", r"Google\Chrome"],
            ext_page: "chrome://extensions",
        },
        BrowserSpec {
            key: "chrome_canary",
            name: "Chrome Canary",
            exe_paths: &[r"Google\Chrome SxS\Application\chrome.exe"],
            registry_roots: &[r"Google\Chrome SxS", r"Google\Chrome"],
            ext_page: "chrome://extensions",
        },
        BrowserSpec {
            key: "edge_beta",
            name: "Edge Beta",
            exe_paths: &[r"Microsoft\Edge Beta\Application\msedge.exe"],
            registry_roots: &[r"Microsoft\Edge Beta", r"Microsoft\Edge"],
            ext_page: "edge://extensions",
        },
        BrowserSpec {
            key: "edge_dev",
            name: "Edge Dev",
            exe_paths: &[r"Microsoft\Edge Dev\Application\msedge.exe"],
            registry_roots: &[r"Microsoft\Edge Dev", r"Microsoft\Edge"],
            ext_page: "edge://extensions",
        },
        BrowserSpec {
            key: "chromium",
            name: "Chromium",
            exe_paths: &[r"Chromium\Application\chrome.exe"],
            registry_roots: &[r"Chromium", r"Google\Chrome"],
            ext_page: "chrome://extensions",
        },
        BrowserSpec {
            key: "brave",
            name: "Brave",
            exe_paths: &[r"BraveSoftware\Brave-Browser\Application\brave.exe"],
            registry_roots: &[r"BraveSoftware\Brave-Browser", r"Google\Chrome"],
            ext_page: "chrome://extensions",
        },
        BrowserSpec {
            key: "vivaldi",
            name: "Vivaldi",
            exe_paths: &[r"Vivaldi\Application\vivaldi.exe"],
            registry_roots: &[r"Vivaldi", r"Google\Chrome"],
            ext_page: "chrome://extensions",
        },
        BrowserSpec {
            key: "opera",
            name: "Opera",
            exe_paths: &[r"Opera Software\Opera Stable\opera.exe"],
            registry_roots: &[r"Opera Software", r"Google\Chrome"],
            ext_page: "opera://extensions",
        },
        BrowserSpec {
            key: "opera_gx",
            name: "Opera GX",
            exe_paths: &[r"Opera Software\Opera GX Stable\opera.exe"],
            registry_roots: &[r"Opera Software", r"Google\Chrome"],
            ext_page: "opera://extensions",
        },
        BrowserSpec {
            key: "cent",
            name: "Cent Browser",
            exe_paths: &[
                r"cent\CentBrowser\Application\chrome.exe",
                r"CentBrowser\Application\chrome.exe",
            ],
            registry_roots: &[r"CentBrowser", r"Google\Chrome"],
            ext_page: "chrome://extensions",
        },
        BrowserSpec {
            key: "browser360",
            name: "360 极速浏览器",
            exe_paths: &[
                r"360Chrome\Chrome\Application\360chrome.exe",
                r"360\360Chrome\Chrome\Application\360chrome.exe",
            ],
            registry_roots: &[r"360\360se6\Chrome", r"360chrome\Chrome", r"Google\Chrome"],
            ext_page: "chrome://extensions",
        },
        BrowserSpec {
            key: "yandex",
            name: "Yandex Browser",
            exe_paths: &[r"Yandex\YandexBrowser\Application\browser.exe"],
            registry_roots: &[r"Yandex\YandexBrowser", r"Google\Chrome"],
            ext_page: "chrome://extensions",
        },
        BrowserSpec {
            key: "coccoc",
            name: "CocCoc",
            exe_paths: &[r"CocCoc\Browser\Application\browser.exe"],
            registry_roots: &[r"CocCoc\Browser", r"Google\Chrome"],
            ext_page: "chrome://extensions",
        },
        BrowserSpec {
            key: "whale",
            name: "Naver Whale",
            exe_paths: &[r"Naver\Naver Whale\Application\whale.exe"],
            registry_roots: &[r"Naver\Naver Whale", r"Google\Chrome"],
            ext_page: "chrome://extensions",
        },
    ]
}

fn find_browser_exe(spec: &BrowserSpec) -> Option<std::path::PathBuf> {
    for env_key in ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"] {
        if let Ok(base) = std::env::var(env_key) {
            let base = std::path::PathBuf::from(base);
            for rel in spec.exe_paths {
                let p = base.join(rel);
                if p.is_file() {
                    return Some(p);
                }
            }
        }
    }
    None
}

/// 调用浏览器自带的 --pack-extension 生成 CRX3；用共享 .pem 保证扩展 ID 跨浏览器/重装稳定。
fn pack_extension(
    exe: &std::path::Path,
    unpacked: &std::path::Path,
    shared_pem: &std::path::Path,
) -> Result<std::path::PathBuf, String> {
    let crx = unpacked.with_extension("crx");
    let _ = std::fs::remove_file(&crx);
    // 上次失败可能留下 <name>.pem，Edge 会报「指定扩展的私钥已存在」；有共享密钥时不需要它
    let _ = std::fs::remove_file(unpacked.with_extension("pem"));
    let mut cmd = std::process::Command::new(exe);
    cmd.arg(format!("--pack-extension={}", unpacked.display()));
    if shared_pem.is_file() {
        cmd.arg(format!("--pack-extension-key={}", shared_pem.display()));
    }
    cmd.arg("--no-message-box")
        .arg("--no-first-run")
        .arg("--no-default-browser-check");
    // 独立用户目录：避免命令被转发给正在运行的浏览器实例而不产出 CRX
    let profile = unpacked.parent().unwrap_or(unpacked).join("pack-profile");
    cmd.arg(format!("--user-data-dir={}", profile.display()));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    let out = cmd.output().map_err(|e| format!("启动浏览器失败: {e}"))?;
    let _ = std::fs::remove_dir_all(&profile);
    if !crx.is_file() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let brief: String = stderr.chars().take(300).collect();
        return Err(format!("打包失败: {}", brief.trim()));
    }
    // 首次打包时浏览器会在扩展目录旁生成 <name>.pem，保存为共享密钥
    if !shared_pem.is_file() {
        let generated = unpacked.with_extension("pem");
        if generated.is_file() {
            if let Some(parent) = shared_pem.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::copy(&generated, shared_pem);
        }
    }
    Ok(crx)
}

/// 打包优先级：优先用 Chrome / Edge，fork 通常基于同版 Chromium 但命令行支持不一。
fn pack_priority(key: &str) -> usize {
    match key {
        "chrome" => 0,
        "edge" => 1,
        "chrome_beta" | "chrome_dev" | "chrome_canary" => 2,
        "edge_beta" | "edge_dev" | "chromium" => 3,
        "brave" => 4,
        _ => 10,
    }
}

/// 旧版把每个浏览器的 CRX / pem 放在 browser-ext/<browser>/ 下，新版统一放 browser-ext/ 根；
/// 这些残留目录不再被读取，安装时清理掉。
fn cleanup_legacy_dirs(base: &std::path::Path) {
    for spec in browser_specs() {
        let legacy = base.join(spec.key);
        if legacy.is_dir() {
            let _ = std::fs::remove_dir_all(&legacy);
        }
    }
}

/// 清理旧版本遗留的外部扩展注册表键：只删 path 指向本应用 browser-ext 目录、
/// 且 ID 与当前正确扩展不同的键；其它软件注册的扩展一律不动。
/// keep_id 为 None（打包失败、无正确 ID 可用）时删除全部指向本应用旧 CRX 的键。
#[cfg(windows)]
fn cleanup_stale_extensions(base: &std::path::Path, keep_id: Option<&str>, roots: &[&str]) {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_ALL_ACCESS};
    use winreg::RegKey;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let marker = base.to_string_lossy().to_lowercase();
    for &root in roots {
        let ext_key = match hkcu
            .open_subkey_with_flags(format!("Software\\{}\\Extensions", root), KEY_ALL_ACCESS)
        {
            Ok(k) => k,
            Err(_) => continue,
        };
        let mut stale: Vec<String> = Vec::new();
        for sub in ext_key.enum_keys().flatten() {
            if let Some(k) = keep_id {
                if k == sub.as_str() {
                    continue;
                }
            }
            let owned = match ext_key.open_subkey(&sub) {
                Ok(k) => k,
                Err(_) => continue,
            };
            let p: String = match owned.get_value("path") {
                Ok(v) => v,
                Err(_) => continue,
            };
            if p.to_lowercase().contains(&marker) {
                stale.push(sub);
            }
        }
        // 先收集再删，避免枚举过程中删除导致跳项
        for id in stale {
            let _ = ext_key.delete_subkey_all(&id);
        }
    }
}

#[cfg(not(windows))]
fn cleanup_stale_extensions(_base: &std::path::Path, _keep_id: Option<&str>, _roots: &[&str]) {}

/// 写注册表外部扩展键：HKCU\Software\<root>\Extensions\<id>。
/// roots 里任一写入成功即算成功（不同 fork 读取的根不同，逐个都写以兼容）。
#[cfg(windows)]
fn register_external_extension(
    roots: &[&str],
    id: &str,
    crx: &std::path::Path,
    version: &str,
) -> Result<(), String> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let crx_path = crx.to_string_lossy().to_string();
    let ver = version.to_string();
    let mut last_err = String::new();
    let mut any = false;
    for root in roots {
        let key_path = format!("Software\\{}\\Extensions\\{}", root, id);
        match hkcu.create_subkey(&key_path) {
            Ok((key, _)) => {
                if let Err(e) = key.set_value("path", &crx_path) {
                    last_err = format!("写入扩展路径失败: {e}");
                    continue;
                }
                if let Err(e) = key.set_value("version", &ver) {
                    last_err = format!("写入扩展版本失败: {e}");
                    continue;
                }
                any = true;
            }
            Err(e) => last_err = format!("打开注册表失败: {e}"),
        }
    }
    if any {
        Ok(())
    } else {
        Err(last_err)
    }
}

#[cfg(not(windows))]
fn register_external_extension(
    _roots: &[&str],
    _id: &str,
    _crx: &std::path::Path,
    _version: &str,
) -> Result<(), String> {
    Err("当前平台不支持自动注册扩展".into())
}

/// 扩展源码内置副本：安装包不携带 extension/ 目录时也能自动安装。
const EMBEDDED_MANIFEST: &str = include_str!("../../extension/manifest.json");
const EMBEDDED_BACKGROUND: &str = include_str!("../../extension/background.js");

/// 准备一份可打包的扩展目录：始终使用编译期内置副本。
/// 不从磁盘上的 extension/ 目录复制，避免把用户机器里其它扩展误打包进来。
fn materialize_extension(dst: &std::path::Path) -> Result<(), String> {
    if dst.exists() {
        let _ = std::fs::remove_dir_all(dst);
    }
    std::fs::create_dir_all(dst).map_err(|e| format!("创建扩展目录失败: {e}"))?;
    std::fs::write(dst.join("manifest.json"), EMBEDDED_MANIFEST)
        .map_err(|e| format!("写出 manifest 失败: {e}"))?;
    std::fs::write(dst.join("background.js"), EMBEDDED_BACKGROUND)
        .map_err(|e| format!("写出 background 失败: {e}"))?;
    Ok(())
}

/// 打包前兜底校验：manifest 必须是本项目的扩展，否则宁可失败也不注册。
fn validate_extension_manifest(dir: &std::path::Path) -> Result<(), String> {
    let name = std::fs::read(dir.join("manifest.json"))
        .ok()
        .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
        .and_then(|v| v.get("name").and_then(|x| x.as_str()).map(str::to_string))
        .unwrap_or_default();
    if !name.contains("AppUsageTracker") {
        return Err(format!("扩展目录内容异常（name={name}），已中止安装"));
    }
    Ok(())
}

/// 读扩展 manifest 里的版本号（注册表 version 值必须与 CRX 内一致）。
fn manifest_version(dir: &std::path::Path) -> String {
    tracker_core::ext::manifest_version(dir)
}

fn install_extension_blocking(app: &AppHandle) -> Result<Vec<ExtInstallResult>, String> {
    let data_dir = {
        let state = app.state::<AppState>();
        let s = state.settings.lock().unwrap();
        s.data_directory.clone()
    };
    let base = std::path::Path::new(&data_dir).join("browser-ext");
    std::fs::create_dir_all(&base).map_err(|e| format!("创建目录失败: {e}"))?;
    // 旧版残留：按浏览器分目录的 CRX / pem 已不再使用
    cleanup_legacy_dirs(&base);

    // 只处理本机确实安装了的浏览器，避免结果列表被一堆「未检测到」刷屏
    let specs = browser_specs();
    let detected: Vec<(&BrowserSpec, std::path::PathBuf)> = specs
        .iter()
        .filter_map(|s| find_browser_exe(s).map(|exe| (s, exe)))
        .collect();
    if detected.is_empty() {
        return Ok(vec![]);
    }

    // 汇总所有可能的注册表根（含未检测到的浏览器），用于清理旧版本残留键
    let mut all_roots: Vec<&str> = Vec::new();
    for spec in &specs {
        for &root in spec.registry_roots {
            if !all_roots.contains(&root) {
                all_roots.push(root);
            }
        }
    }

    // 扩展内容只用内置副本；共享密钥 → 所有浏览器同一扩展 ID
    let unpacked = base.join("extension");
    materialize_extension(&unpacked)?;
    validate_extension_manifest(&unpacked)?;
    let version = manifest_version(&unpacked);
    let shared_pem = base.join("extension-key.pem");

    // 按 Chrome / Edge 优先顺序逐个尝试打包，任一成功即止；失败自动降级到下一款
    let mut order: Vec<usize> = (0..detected.len()).collect();
    order.sort_by_key(|&i| pack_priority(detected[i].0.key));
    let mut crx: Option<std::path::PathBuf> = None;
    let mut pack_errors: Vec<String> = Vec::new();
    for &i in &order {
        match pack_extension(&detected[i].1, &unpacked, &shared_pem) {
            Ok(p) => {
                crx = Some(p);
                break;
            }
            Err(e) => pack_errors.push(format!("{}: {}", detected[i].0.name, e)),
        }
    }
    let crx = match crx {
        Some(p) => p,
        None => {
            cleanup_stale_extensions(&base, None, &all_roots);
            let message = pack_errors.join("；");
            return Ok(detected
                .iter()
                .map(|(spec, _)| ExtInstallResult {
                    browser: spec.key.to_string(),
                    name: spec.name.to_string(),
                    ok: false,
                    code: "pack_failed".into(),
                    extension_id: String::new(),
                    message: message.clone(),
                })
                .collect());
        }
    };
    let hash = match std::fs::read(&crx).ok().and_then(|b| crx_id_bytes(&b)) {
        Some(h) => h,
        None => {
            cleanup_stale_extensions(&base, None, &all_roots);
            let message = "打包产物无效，请改用手动安装".to_string();
            return Ok(detected
                .iter()
                .map(|(spec, _)| ExtInstallResult {
                    browser: spec.key.to_string(),
                    name: spec.name.to_string(),
                    ok: false,
                    code: "bad_crx".into(),
                    extension_id: String::new(),
                    message: message.clone(),
                })
                .collect());
        }
    };
    let id = extension_id_from_hash(&hash);
    // 删除旧版本写入的、指向本应用旧 CRX 的错误注册表键（保留当前正确 ID）
    cleanup_stale_extensions(&base, Some(id.as_str()), &all_roots);

    let mut results: Vec<ExtInstallResult> = Vec::new();
    for (spec, _exe) in &detected {
        let mut row = ExtInstallResult {
            browser: spec.key.to_string(),
            name: spec.name.to_string(),
            ok: false,
            code: "registry_failed".into(),
            extension_id: id.clone(),
            message: String::new(),
        };
        match register_external_extension(spec.registry_roots, &id, &crx, &version) {
            Ok(()) => {
                row.ok = true;
                row.code = "ok".into();
                row.message = crx.to_string_lossy().to_string();
            }
            Err(e) => row.message = e,
        }
        results.push(row);
    }

    // 装好至少一个浏览器 → 自动开启网页统计并通知前端刷新设置
    if results.iter().any(|r| r.ok) {
        let need = {
            let state = app.state::<AppState>();
            let mut s = state.settings.lock().unwrap();
            let need = !s.record_web;
            if need {
                s.record_web = true;
            }
            need
        };
        if need {
            save_settings_file(app);
            let snapshot = app.state::<AppState>().settings.lock().unwrap().clone();
            let _ = app.emit("app://settings", &snapshot);
        }
    }
    Ok(results)
}

/// 智能安装浏览器扩展：探测已装浏览器 → 打包 CRX3 → 写注册表外部扩展键。
#[tauri::command]
pub(crate) async fn smart_install_extension(
    app: AppHandle,
) -> Result<Vec<ExtInstallResult>, String> {
    tauri::async_runtime::spawn_blocking(move || install_extension_blocking(&app))
        .await
        .map_err(|e| format!("安装任务失败: {e}"))?
}

/// 打开浏览器扩展管理页。
#[tauri::command]
pub(crate) fn open_browser_ext_page(browser: String) -> Result<(), String> {
    let url = browser_specs()
        .iter()
        .find(|s| s.key == browser)
        .map(|s| s.ext_page)
        .ok_or("未知浏览器")?;
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // start 命令打开 chrome:// 或 edge:// 内部页面
        std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .creation_flags(0x08000000) // CREATE_NO_WINDOW
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(not(windows))]
    {
        let _ = url;
    }
    Ok(())
}
