using AppUsageTracker.Core.Models;

namespace AppUsageTracker.Core.Monitoring;

/// <summary>
/// 分类规则引擎：按规则顺序用「包含关键词」匹配软件（显示名/进程名/exe 路径），
/// 未命中归入 <see cref="Uncategorized"/>。纯逻辑、可单测、线程安全。
/// </summary>
public sealed class CategoryResolver
{
    /// <summary>「未分类」在分类体系中的固定名称。</summary>
    public const string Uncategorized = "未分类";

    private readonly IReadOnlyList<CategoryRule> _rules;

    public CategoryResolver(IReadOnlyList<CategoryRule> rules) => _rules = rules;

    /// <summary>返回应用所属分类名。</summary>
    public string Resolve(string displayName, string processName, string exePath)
    {
        var haystack = string.Concat(displayName, '\n', processName, '\n', exePath);
        if (haystack.Length > 1)
        {
            haystack = haystack.ToLowerInvariant();
            foreach (var rule in _rules)
            {
                foreach (var keyword in rule.Keywords)
                {
                    if (keyword.Length > 0 && haystack.Contains(keyword.ToLowerInvariant()))
                    {
                        return rule.Name;
                    }
                }
            }
        }

        return Uncategorized;
    }
}
