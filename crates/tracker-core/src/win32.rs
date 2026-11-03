// Win32 封装：前台窗口 + 输入空闲检测 + 锁屏检测（非 Windows 提供桩实现，保证跨平台编译）
#![allow(non_snake_case)]

#[cfg(windows)]
mod imp {
    use windows::core::PWSTR;
    use windows::Win32::System::SystemInformation::GetTickCount64;
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

    pub fn get_foreground_pid() -> Option<u32> {
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.is_invalid() {
                return None;
            }
            let mut pid: u32 = 0;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            if pid == 0 {
                None
            } else {
                Some(pid)
            }
        }
    }

    pub fn get_idle_seconds() -> f64 {
        unsafe {
            let mut info = LASTINPUTINFO {
                cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
                dwTime: 0,
            };
            if !GetLastInputInfo(&mut info).as_bool() {
                return 0.0;
            }
            let now = GetTickCount64();
            let idle_ms = now - info.dwTime as u64;
            idle_ms as f64 / 1000.0
        }
    }

    pub fn get_process_image_name(pid: u32) -> Option<String> {
        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
            let mut buf = [0u16; 1024];
            let mut len: u32 = buf.len() as u32;
            let result = QueryFullProcessImageNameW(
                handle,
                PROCESS_NAME_WIN32,
                PWSTR(buf.as_mut_ptr()),
                &mut len,
            );
            let _ = windows::Win32::Foundation::CloseHandle(handle);
            if result.is_ok() && len > 0 {
                Some(String::from_utf16_lossy(&buf[..len as usize]))
            } else {
                None
            }
        }
    }

    /// 输入桌面是否可用（锁屏/安全桌面时返回 false）。
    pub fn is_input_desktop_available() -> bool {
        unsafe {
            use windows::Win32::System::StationsAndDesktops::{
                CloseDesktop, OpenInputDesktop, DESKTOP_CONTROL_FLAGS, DESKTOP_READOBJECTS,
            };
            let desktop = OpenInputDesktop(DESKTOP_CONTROL_FLAGS(0), false, DESKTOP_READOBJECTS);
            let ok = desktop.is_ok();
            if let Ok(d) = desktop {
                let _ = CloseDesktop(d);
            }
            ok
        }
    }

    /// 系统运行毫秒数（睡眠期间不增长，用于检测系统睡眠）。
    pub fn get_tick_count64() -> u64 {
        unsafe { GetTickCount64() }
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn get_foreground_pid() -> Option<u32> {
        None
    }

    pub fn get_idle_seconds() -> f64 {
        0.0
    }

    pub fn get_process_image_name(_pid: u32) -> Option<String> {
        None
    }

    pub fn is_input_desktop_available() -> bool {
        true
    }

    pub fn get_tick_count64() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }
}

pub use imp::*;
