// 开机自启：写入 HKCU Run 注册表键（与旧 C# 版本行为一致；非 Windows 平台为空实现）。

#[cfg(windows)]
mod imp {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE};
    use winreg::RegKey;

    const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const VALUE_NAME: &str = "AppUsageTracker";

    pub fn set(enabled: bool, exe_path: &str) -> Result<(), String> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let key = hkcu
            .open_subkey_with_flags(RUN_KEY, KEY_SET_VALUE | KEY_QUERY_VALUE)
            .or_else(|_| hkcu.create_subkey(RUN_KEY).map(|(k, _)| k))
            .map_err(|e| e.to_string())?;
        if enabled {
            key.set_value(VALUE_NAME, &exe_path)
                .map_err(|e| e.to_string())
        } else {
            let _ = key.delete_value(VALUE_NAME);
            Ok(())
        }
    }
}

#[cfg(windows)]
pub use imp::set;

#[cfg(not(windows))]
pub fn set(_enabled: bool, _exe_path: &str) -> Result<(), String> {
    Ok(())
}
