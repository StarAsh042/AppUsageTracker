// 从 exe 的版本资源中提取 FileDescription 作为显示名（失败时回退文件名）。
#![allow(non_snake_case)]

use windows::core::{w, PCWSTR};
use windows::Win32::Storage::FileSystem::{
    GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW,
};

/// 读取 FileDescription；不存在或出错时返回 None。
pub fn file_description(exe_path: &str) -> Option<String> {
    let wide: Vec<u16> = exe_path.encode_utf16().chain(std::iter::once(0)).collect();
    let file = PCWSTR(wide.as_ptr());

    unsafe {
        let size = GetFileVersionInfoSizeW(file, None);
        if size == 0 {
            return None;
        }
        let mut data = vec![0u8; size as usize];
        if !GetFileVersionInfoW(file, 0, size, data.as_mut_ptr() as *mut _).as_bool() {
            return None;
        }

        // 1) 语言/代码页翻译表
        let mut ptr: *mut std::ffi::c_void = std::ptr::null_mut();
        let mut ptr_len: u32 = 0;
        if !VerQueryValueW(
            data.as_ptr() as *const _,
            w!("\\VarFileInfo\\Translation"),
            &mut ptr,
            &mut ptr_len,
        )
        .as_bool()
            || ptr.is_null()
            || ptr_len < 4
        {
            return None;
        }
        let translations = ptr as *const u16;
        let primary = u32::from(*translations) | (u32::from(*translations.add(1)) << 16);
        let lang_id = primary >> 16;
        let codepage = primary & 0xFFFF;

        // 2) 依次尝试常见 StringFileInfo 块
        let blocks = [
            format!("\\StringFileInfo\\{lang_id:04x}{codepage:04x}\\FileDescription"),
            "\\StringFileInfo\\040904b0\\FileDescription".to_string(),
            "\\StringFileInfo\\040904e4\\FileDescription".to_string(),
        ];
        for b in blocks {
            let path: Vec<u16> = b.encode_utf16().chain(std::iter::once(0)).collect();
            let mut out: *mut std::ffi::c_void = std::ptr::null_mut();
            let mut out_len: u32 = 0;
            if VerQueryValueW(
                data.as_ptr() as *const _,
                PCWSTR(path.as_ptr()),
                &mut out,
                &mut out_len,
            )
            .as_bool()
                && !out.is_null()
                && out_len > 0
            {
                let slice = std::slice::from_raw_parts(out as *const u16, out_len as usize);
                let s = String::from_utf16_lossy(slice);
                let s = s.trim_end_matches('\0').trim();
                if !s.is_empty() {
                    return Some(s.to_string());
                }
            }
        }
        None
    }
}
