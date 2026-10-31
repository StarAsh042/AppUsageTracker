using System.Diagnostics;
using AppUsageTracker.Core.Interop;
using AppUsageTracker.Core.Models;

namespace AppUsageTracker.Core.Monitoring;

/// <summary>
/// 把进程 PID 解析为 <see cref="AppIdentity"/>，并按 exe 路径缓存显示信息，
/// 避免每次轮询都读取文件版本信息。
/// </summary>
internal static class AppIdentityResolver
{
    private static readonly object Gate = new();
    private static readonly Dictionary<string, AppIdentity> CacheByPath = new(StringComparer.OrdinalIgnoreCase);

    /// <summary>解析 PID 对应的应用。无法取得 exe 路径时以进程名兜底；彻底失败返回 null。</summary>
    public static AppIdentity? Resolve(uint pid)
    {
        var path = NativeMethods.TryGetProcessImageName(pid);
        if (!string.IsNullOrEmpty(path))
        {
            lock (Gate)
            {
                if (CacheByPath.TryGetValue(path, out var cached)) return cached;
            }

            var identity = new AppIdentity(
                AppKey: path.ToLowerInvariant(),
                ProcessName: Path.GetFileNameWithoutExtension(path),
                DisplayName: GetDisplayName(path),
                ExePath: path);

            lock (Gate) CacheByPath[path] = identity;
            return identity;
        }

        // 路径不可得（权限/系统进程）：退化为进程名，仍可统计但精度较低
        try
        {
            using var proc = Process.GetProcessById((int)pid);
            var name = proc.ProcessName;
            return string.IsNullOrEmpty(name)
                ? null
                : new AppIdentity($"$proc:{name.ToLowerInvariant()}", name, name, string.Empty);
        }
        catch
        {
            return null;
        }
    }

    private static string GetDisplayName(string exePath)
    {
        try
        {
            var info = FileVersionInfo.GetVersionInfo(exePath);
            if (!string.IsNullOrWhiteSpace(info.FileDescription)) return info.FileDescription.Trim();
        }
        catch { /* 文件可能已被替换/删除 */ }

        return Path.GetFileNameWithoutExtension(exePath);
    }
}
