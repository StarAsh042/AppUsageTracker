// 开机自启：写入 HKCU Run 注册表键（与旧 C# 版本行为一致；非 Windows 平台为空实现）。

#[cfg(not(windows))]
use crate::error::AppError;

#[cfg(windows)]
mod imp {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE};
    use winreg::RegKey;

    use crate::error::AppError;

    const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const VALUE_NAME: &str = "AppUsageTracker";

    /// 开机自启时附加的标记：主窗口在 tauri.conf.json 里是 visible:false，
    /// 启动逻辑据此判断"这是自启，不要弹主界面"，只在托盘常驻。
    pub const AUTOSTART_FLAG: &str = "--autostart";

    pub fn set(enabled: bool, exe_path: &str) -> Result<(), AppError> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let key = hkcu
            .open_subkey_with_flags(RUN_KEY, KEY_SET_VALUE | KEY_QUERY_VALUE)
            .or_else(|_| hkcu.create_subkey(RUN_KEY).map(|(k, _)| k))
            .map_err(|e| AppError::new("autostart", e.to_string()))?;
        if enabled {
            // 带引号以兼容含空格的安装路径，并附自启标记
            key.set_value(VALUE_NAME, &format!("\"{exe_path}\" {AUTOSTART_FLAG}"))
                .map_err(|e| AppError::new("autostart", e.to_string()))
        } else {
            let _ = key.delete_value(VALUE_NAME);
            Ok(())
        }
    }
}

#[cfg(windows)]
pub use imp::set;
#[cfg(windows)]
pub use imp::AUTOSTART_FLAG;

#[cfg(not(windows))]
pub const AUTOSTART_FLAG: &str = "--autostart";

#[cfg(not(windows))]
pub fn set(_enabled: bool, _exe_path: &str) -> Result<(), AppError> {
    Ok(())
}
