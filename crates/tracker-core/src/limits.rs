use std::collections::{HashMap, HashSet};

/// 返回当日首次跨过限额的分类列表。
pub fn categories_to_notify(
    today_seconds: &HashMap<String, f64>,
    limit_minutes: &HashMap<String, u32>,
    already_notified: &HashSet<String>,
) -> Vec<String> {
    let mut result = Vec::new();
    for (cat, mins) in limit_minutes {
        if *mins == 0 || already_notified.contains(cat) { continue; }
        if today_seconds.get(cat).map_or(false, |&s| s >= *mins as f64 * 60.0) {
            result.push(cat.clone());
        }
    }
    result
}
