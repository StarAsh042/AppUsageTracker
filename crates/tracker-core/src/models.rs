use serde::{Deserialize, Serialize};

/// 前台监控状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrackingState {
    Active,
    Idle,
    Locked,
    Suspended,
    Paused,
}

/// 单个应用的身份标识（以 exe 路径小写为 AppKey）。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AppIdentity {
    /// 小写 exe 完整路径，唯一标识。
    pub app_key: String,
    /// 进程名（无扩展名）。
    pub process_name: String,
    /// 文件描述 / 显示名。
    pub display_name: String,
    /// exe 完整路径（原始大小写）。
    pub exe_path: String,
}

/// 每日聚合记录（按日期 × 应用）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyUsage {
    pub usage_date: String,
    pub app_key: String,
    pub process_name: String,
    pub display_name: String,
    pub exe_path: String,
    pub active_seconds: f64,
}

/// 前台使用明细片段。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForegroundInterval {
    pub app_key: String,
    pub process_name: String,
    pub display_name: String,
    pub exe_path: String,
    pub start_time: String,
    pub end_time: String,
    pub duration_seconds: f64,
}

/// 窗口标题区间（隐私开关打开才记录）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TitleInterval {
    pub app_key: String,
    pub title: String,
    pub start_time: String,
    pub end_time: String,
    pub duration_seconds: f64,
}

/// 浏览器网页区间（浏览器扩展心跳聚合，隐私开关打开才记录）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebInterval {
    pub domain: String,
    pub title: String,
    pub start_time: String,
    pub end_time: String,
    pub duration_seconds: f64,
}

/// 跟踪状态区间（统计中/空闲/锁定/挂起/暂停），供时间线与状态分布统计。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateInterval {
    pub state: String,
    pub start_time: String,
    pub end_time: String,
    pub duration_seconds: f64,
}

/// 分类规则。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryRule {
    pub name: String,
    pub keywords: Vec<String>,
}
