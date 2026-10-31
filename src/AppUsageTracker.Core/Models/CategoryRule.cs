namespace AppUsageTracker.Core.Models;

/// <summary>一条分类规则：软件的显示名/进程名/exe 路径包含任一关键词即归入该分类（按规则顺序匹配）。</summary>
public sealed class CategoryRule
{
    public required string Name { get; set; }
    public List<string> Keywords { get; set; } = new();
}
