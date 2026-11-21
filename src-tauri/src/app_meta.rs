// Derive small display metadata for the dashboard: an icon data URL and an
// optional Chinese annotation when the app's own name is not already Chinese.
#![allow(non_snake_case)]

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use crate::version_info::file_description;

pub(crate) fn icon_data_url(exe_path: &str) -> Option<String> {
    #[cfg(not(windows))]
    {
        let _ = exe_path;
        return None;
    }

    #[cfg(windows)]
    {
        static CACHE: OnceLock<Mutex<HashMap<String, Option<String>>>> = OnceLock::new();
        let key = exe_path.to_lowercase();
        let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
        if let Some(cached) = cache.lock().ok()?.get(&key) {
            return cached.clone();
        }
        let value = extract_icon_data_url(exe_path);
        if let Ok(mut cache) = cache.lock() {
            cache.insert(key, value.clone());
        }
        value
    }
}

pub(crate) fn chinese_name(exe_path: &str, process_name: &str, display_name: &str) -> Option<String> {
    if has_cjk(display_name) {
        return None;
    }

    let key = process_key(process_name).to_lowercase();
    let stem = std::path::Path::new(exe_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    let shown = display_name.trim().to_lowercase();
    for name in [key.as_str(), stem.as_str(), shown.as_str()] {
        // chrome.exe is shared by Google Chrome and many Chromium forks; only
        // annotate when the executable actually belongs to Google Chrome.
        if name == "chrome" && !is_google_chrome(exe_path) {
            continue;
        }
        if let Some(zh) = curated_name(name) {
            return Some(zh.to_string());
        }
    }

    file_description(exe_path).and_then(|desc| {
        let desc = desc.trim();
        if desc != display_name && has_cjk(desc) {
            Some(desc.to_string())
        } else {
            None
        }
    })
}

fn process_key(process_name: &str) -> &str {
    let name = process_name
        .trim()
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or(process_name);
    if name.len() >= 4 && name[name.len() - 4..].eq_ignore_ascii_case(".exe") {
        &name[..name.len() - 4]
    } else {
        name
    }
}

fn curated_name(key: &str) -> Option<&'static str> {
    Some(match key {
        "taskmgr" => "任务管理器",
        "explorer" => "文件资源管理器",
        "notepad" => "记事本",
        "cmd" => "命令提示符",
        "mspaint" => "画图",
        "msedge" => "微软 Edge 浏览器",
        "chrome" => "Chrome 浏览器",
        "qq" => "腾讯 QQ",
        "wechat" | "weixin" => "微信",
        "doubao" => "豆包",
        "cleanmgr" => "磁盘清理",
        "searchhost" => "搜索",
        "applicationframehost" => "应用框架宿主",
        _ => return None,
    })
}

fn is_google_chrome(exe_path: &str) -> bool {
    let path = exe_path.to_lowercase();
    path.contains("\\google\\chrome\\") || path.contains("/google/chrome/")
}

fn has_cjk(s: &str) -> bool {
    s.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c))
}

#[cfg(windows)]
fn extract_icon_data_url(exe_path: &str) -> Option<String> {
    use base64::Engine;
    use windows::core::PCWSTR;
    use windows::Win32::Graphics::Gdi::{
        CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, GetObjectW, BITMAP, BITMAPINFO,
        BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HBITMAP, HGDIOBJ,
    };
    use windows::Win32::Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES;
    use windows::Win32::UI::Shell::{SHGetFileInfoW, SHGFI_ICON, SHGFI_SMALLICON, SHFILEINFOW};
    use windows::Win32::UI::WindowsAndMessaging::{
        DestroyIcon, GetIconInfo, HICON, ICONINFO,
    };

    struct IconResources {
        dc: Option<windows::Win32::Graphics::Gdi::HDC>,
        icon: Option<HICON>,
        mask: Option<HBITMAP>,
        color: Option<HBITMAP>,
    }

    impl Drop for IconResources {
        fn drop(&mut self) {
            unsafe {
                if let Some(dc) = self.dc {
                    let _ = DeleteDC(dc);
                }
                if let Some(icon) = self.icon {
                    let _ = DestroyIcon(icon);
                }
                if let Some(mask) = self.mask {
                    let _ = DeleteObject(HGDIOBJ(mask.0));
                }
                if let Some(color) = self.color {
                    let _ = DeleteObject(HGDIOBJ(color.0));
                }
            }
        }
    }

    let wide: Vec<u16> = exe_path.encode_utf16().chain(std::iter::once(0)).collect();
    let mut info = SHFILEINFOW::default();
    let size = std::mem::size_of::<SHFILEINFOW>() as u32;
    let result = unsafe {
        SHGetFileInfoW(
            PCWSTR(wide.as_ptr()),
            FILE_FLAGS_AND_ATTRIBUTES(0),
            Some(&mut info),
            size,
            SHGFI_ICON | SHGFI_SMALLICON,
        )
    };
    if result == 0 || info.hIcon.0.is_null() {
        return None;
    }

    let mut resources = IconResources {
        dc: None,
        icon: Some(info.hIcon),
        mask: None,
        color: None,
    };
    let mut icon_info = ICONINFO::default();
    if unsafe { GetIconInfo(info.hIcon, &mut icon_info) }.is_err() {
        return None;
    }
    resources.color = (!icon_info.hbmColor.0.is_null()).then_some(icon_info.hbmColor);
    resources.mask = (!icon_info.hbmMask.0.is_null()).then_some(icon_info.hbmMask);
    let hbm = icon_info.hbmColor;
    if hbm.0.is_null() {
        return None;
    }

    let mut bitmap = BITMAP::default();
    let bitmap_len = std::mem::size_of::<BITMAP>() as i32;
    if unsafe { GetObjectW(HGDIOBJ(hbm.0), bitmap_len, Some(&mut bitmap as *mut _ as *mut _)) } != bitmap_len
        || bitmap.bmWidth <= 0
        || bitmap.bmHeight <= 0
    {
        return None;
    }
    let width = bitmap.bmWidth as usize;
    let height = bitmap.bmHeight as usize;
    let mut pixels = vec![0u8; width * height * 4];
    let mut bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width as i32,
            biHeight: height as i32,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let dc = unsafe { CreateCompatibleDC(None) };
    if dc.0.is_null() {
        return None;
    }
    resources.dc = Some(dc);
    if unsafe {
        GetDIBits(
            dc,
            hbm,
            0,
            height as u32,
            Some(pixels.as_mut_ptr() as *mut _),
            &mut bmi,
            DIB_RGB_COLORS,
        )
    } == 0
    {
        return None;
    }

    let mut rgba = vec![0u8; pixels.len()];
    for y in 0..height {
        let src = (height - 1 - y) * width * 4;
        let dst = y * width * 4;
        for x in 0..width {
            let s = src + x * 4;
            let d = dst + x * 4;
            rgba[d] = pixels[s + 2];
            rgba[d + 1] = pixels[s + 1];
            rgba[d + 2] = pixels[s];
            rgba[d + 3] = pixels[s + 3];
        }
    }

    let mut png = Vec::new();
    let mut encoder = png::Encoder::new(&mut png, width as u32, height as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().ok()?;
    writer.write_image_data(&rgba).ok()?;
    drop(writer);
    Some(format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(png)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chinese_name_adds_annotation_only_for_known_non_chinese_apps() {
        assert_eq!(
            chinese_name("C:\\Windows\\System32\\taskmgr.exe", "Taskmgr", "Taskmgr").as_deref(),
            Some("任务管理器")
        );
        assert_eq!(
            chinese_name("C:\\Program Files\\Microsoft\\Edge\\msedge.exe", "msedge", "Microsoft Edge").as_deref(),
            Some("微软 Edge 浏览器")
        );
        assert_eq!(
            chinese_name("C:\\Windows\\System32\\notepad.exe", "notepad", "记事本").as_deref(),
            None
        );
        assert_eq!(
            chinese_name("C:\\Apps\\Unknown.exe", "Unknown", "Unknown Tool").as_deref(),
            None
        );
    }

    #[test]
    fn chrome_annotation_is_scoped_to_google_chrome() {
        assert_eq!(
            chinese_name(
                "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
                "chrome",
                "Google Chrome"
            )
            .as_deref(),
            Some("Chrome 浏览器")
        );
        assert_eq!(
            chinese_name(
                "C:\\Program Files (x86)\\cent\\CentBrowser\\Application\\chrome.exe",
                "chrome",
                "Cent Browser"
            )
            .as_deref(),
            None
        );
    }

    #[test]
    fn process_key_strips_path_and_exe_suffix() {
        assert_eq!(process_key("C:\\Windows\\System32\\Taskmgr.exe"), "Taskmgr");
        assert_eq!(process_key("/usr/bin/chrome"), "chrome");
    }

    #[test]
    fn has_cjk_detects_chinese_characters() {
        assert!(has_cjk("任务管理器"));
        assert!(!has_cjk("Task Manager"));
    }

    #[cfg(windows)]
    #[test]
    fn icon_data_url_extracts_a_real_windows_icon() {
        let url = icon_data_url("C:\\Windows\\System32\\taskmgr.exe")
            .expect("expected a task manager icon on Windows");
        assert!(url.starts_with("data:image/png;base64,"));
        assert!(url.len() > 100);
    }
}
