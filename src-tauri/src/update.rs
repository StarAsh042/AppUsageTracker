// 应用内更新：下载通道（官方 / 镜像 / 自动回退）与检查、安装两个命令。
//
// 端点与安装逻辑整体搬自 lib.rs，行为不变：auto 通道官方优先、镜像回退；
// 关闭「使用代理」时强制直连；安装时把下载 URL 重写到 gh-proxy。

use serde::Serialize;
use tauri::{AppHandle, Manager};
use tauri_plugin_updater::UpdaterExt;

use crate::error::AppError;
use crate::sync::MutexExt;
use crate::AppState;

const OFFICIAL_LATEST: &str =
    "https://github.com/StarAsh042/AppUsageTracker/releases/latest/download/latest.json";
const MIRROR_LATEST: &str = "https://gh-proxy.com/https://github.com/StarAsh042/AppUsageTracker/releases/latest/download/latest-mirror.json";

/// 按下载通道返回更新端点（auto = 官方优先、镜像回退）。
pub fn updater_endpoints(channel: &str) -> Vec<url::Url> {
    use std::str::FromStr;
    let official = url::Url::from_str(OFFICIAL_LATEST).expect("valid url");
    let mirror = url::Url::from_str(MIRROR_LATEST).expect("valid url");
    match channel {
        "official" => vec![official],
        "mirror" => vec![mirror],
        _ => vec![official, mirror],
    }
}

#[derive(Serialize, Clone)]
pub struct UpdateInfo {
    version: String,
    notes: String,
    date: String,
}

#[tauri::command]
pub async fn check_update(app: AppHandle) -> Result<Option<UpdateInfo>, AppError> {
    let (channel, use_proxy) = {
        let st = app.state::<AppState>();
        let s = st.settings.lock_or_recover();
        (s.update_channel.clone(), s.update_use_proxy)
    };
    let mut builder = app
        .updater_builder()
        .endpoints(updater_endpoints(&channel))
        .map_err(|e| AppError::new("update-endpoint", format!("端点配置失败: {e}")))?;
    // 关闭代理时强制跳过系统代理（直连）；开启时沿用系统代理设置。
    if !use_proxy {
        builder = builder.no_proxy();
    }
    let update = builder
        .build()
        .map_err(|e| AppError::new("update-init", format!("初始化更新器失败: {e}")))?
        .check()
        .await
        .map_err(|e| AppError::new("update-check", format!("检查更新失败: {e}")))?;
    Ok(update.map(|u| UpdateInfo {
        version: u.version.clone(),
        notes: u.body.clone().unwrap_or_default(),
        date: u.date.map(|d| d.to_string()).unwrap_or_default(),
    }))
}

#[tauri::command]
pub async fn install_update(app: AppHandle) -> Result<(), AppError> {
    let (channel, use_proxy) = {
        let st = app.state::<AppState>();
        let s = st.settings.lock_or_recover();
        (s.update_channel.clone(), s.update_use_proxy)
    };
    let mut builder = app
        .updater_builder()
        .endpoints(updater_endpoints(&channel))
        .map_err(|e| AppError::new("update-endpoint", format!("端点配置失败: {e}")))?;
    // 关闭代理时强制跳过系统代理（直连）；开启时沿用系统代理设置。
    if !use_proxy {
        builder = builder.no_proxy();
    }
    let mut update = builder
        .build()
        .map_err(|e| AppError::new("update-init", format!("初始化更新器失败: {e}")))?
        .check()
        .await
        .map_err(|e| AppError::new("update-check", format!("检查更新失败: {e}")))?
        .ok_or_else(|| AppError::new("update-latest", "当前已是最新版本"))?;
    // 下载 URL 走 gh-proxy（国内直连可达）
    let original = update.download_url.to_string();
    update.download_url = url::Url::parse(&format!("https://gh-proxy.com/{}", original))
        .map_err(|e| AppError::new("update-url", format!("URL 重写失败: {e}")))?;
    let version = update.version.clone();
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|e| AppError::new("update-install", format!("安装更新失败: {e}")))?;
    log::info!("已更新到 {version}，应用即将重启");
    Ok(())
}
