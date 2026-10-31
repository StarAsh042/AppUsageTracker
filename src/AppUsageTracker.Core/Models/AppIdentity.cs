namespace AppUsageTracker.Core.Models;

/// <summary>
/// 被统计应用的唯一标识与显示信息。
/// <see cref="AppKey"/> 为 exe 完整路径的小写归一形式；无法取得路径时退化为进程名。
/// </summary>
public sealed record AppIdentity(string AppKey, string ProcessName, string DisplayName, string ExePath)
{
    public override string ToString() => DisplayName;
}
