using System;

namespace AppUsageTracker.Core.Tracking;

/// <summary>
/// 分类每日限额判定（纯逻辑、可单测）：给定今日各分类活跃秒数、限额与当日已通知集合，
/// 返回需要通知的分类。去重按"分类 + 日期"在调用方维护。
/// </summary>
public static class CategoryLimitChecker
{
    /// <summary>返回当日首次跨过限额的分类（今日活跃 ≥ 限额且尚未通知）。</summary>
    public static System.Collections.Generic.List<string> CategoriesToNotify(
        System.Collections.Generic.IReadOnlyDictionary<string, double> todaySecondsByCategory,
        System.Collections.Generic.IReadOnlyDictionary<string, int> limitMinutesByCategory,
        System.Collections.Generic.IReadOnlyCollection<string> alreadyNotifiedToday)
    {
        var result = new System.Collections.Generic.List<string>();
        foreach (var (category, minutes) in limitMinutesByCategory)
        {
            if (minutes <= 0) continue;
            if (alreadyNotifiedToday.Contains(category)) continue;
            if (todaySecondsByCategory.TryGetValue(category, out var seconds)
                && seconds >= minutes * 60.0)
            {
                result.Add(category);
            }
        }
        return result;
    }
}
