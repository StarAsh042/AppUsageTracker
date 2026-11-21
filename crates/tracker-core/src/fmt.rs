//! 展示用格式化：时长转中文时分秒、CSV 字段转义。
//!
//! 两者原先在 src-tauri/src/lib.rs 的「小工具」段里，只被报表与周报使用；
//! 搬到核心库后可以直接单测，不必起 Tauri。

/// 把秒数写成中文时长：>=1 小时用「时/分」，>=1 分钟用「分/秒」，否则「秒」。
pub fn fmt_hms(seconds: f64) -> String {
    let total = seconds as i64;
    let h = total / 3600;
    let m = (total % 3600) / 60;
    let s = total % 60;
    if h > 0 {
        format!("{h}时{m}分")
    } else if m > 0 {
        format!("{m}分{s}秒")
    } else {
        format!("{s}秒")
    }
}

/// CSV 字段转义：含逗号、双引号或换行时整体加引号，内部双引号翻倍。
pub fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fmt_hms_按量级切换单位() {
        assert_eq!(fmt_hms(0.0), "0秒");
        assert_eq!(fmt_hms(59.9), "59秒");
        assert_eq!(fmt_hms(60.0), "1分0秒");
        assert_eq!(fmt_hms(3_599.0), "59分59秒");
        assert_eq!(fmt_hms(3_600.0), "1时0分");
        assert_eq!(fmt_hms(7_384.0), "2时3分");
    }

    #[test]
    fn csv_escape_只在必要时加引号() {
        assert_eq!(csv_escape("微信"), "微信");
        assert_eq!(csv_escape("a,b"), "\"a,b\"");
        assert_eq!(csv_escape("say \"hi\""), "\"say \"\"hi\"\"\"");
        assert_eq!(csv_escape("line\nbreak"), "\"line\nbreak\"");
    }
}
