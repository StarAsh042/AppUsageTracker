//! 命令层统一错误：给每个失败原因一个稳定的 code。
//!
//! 命令以前一律返回 `Result<T, String>`，前端只能把整句话原样显示出去 ——
//! 既没法按原因分支（磁盘满了要提示清理，备份格式坏了要提示保原文件），
//! 也没法本地化。这里把「原因」和「描述」分开：`code` 给程序看，`message` 给人看。
//!
//! 只做承载不做聚合：底层错误用 `From` 带粗粒度 code（io / db / json），
//! 需要区分场景的调用点显式给更细的 code。
use serde::Serialize;

/// 传给前端的错误，序列化成 `{ "code": "…", "message": "…" }`。
#[derive(Debug, Clone, Serialize)]
pub(crate) struct AppError {
    /// 机器可读的原因，前端据此分支与本地化。
    pub code: &'static str,
    /// 给人看的描述，直接展示。
    pub message: String,
}

impl AppError {
    pub(crate) fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for AppError {}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        Self::new("io", e.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        Self::new("json", e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 序列化成_code_与_message_两个字段() {
        let e = AppError::new("backup-invalid", "备份文件不是有效的应用备份");
        let v: serde_json::Value = serde_json::to_value(&e).expect("serialize");
        assert_eq!(v["code"], "backup-invalid");
        assert_eq!(v["message"], "备份文件不是有效的应用备份");
    }

    #[test]
    fn 展示文本就是_message() {
        let e = AppError::new("io", "磁盘已满");
        assert_eq!(e.to_string(), "磁盘已满");
    }

    #[test]
    fn 底层错误默认归到粗粒度_code() {
        let io = AppError::from(std::io::Error::other("boom"));
        assert_eq!(io.code, "io");
        assert!(io.message.contains("boom"));
    }
}
