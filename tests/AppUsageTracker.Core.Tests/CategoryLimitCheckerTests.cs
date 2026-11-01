using AppUsageTracker.Core.Tracking;
using Xunit;

namespace AppUsageTracker.Core.Tests;

public class CategoryLimitCheckerTests
{
    [Fact]
    public void 达到限额_通知一次()
    {
        var seconds = new Dictionary<string, double> { ["娱乐"] = 3600 };  // 1h
        var limits = new Dictionary<string, int> { ["娱乐"] = 60 };        // 60min
        var notified = new HashSet<string>();

        var first = CategoryLimitChecker.CategoriesToNotify(seconds, limits, notified);
        Assert.Single(first);
        notified.UnionWith(first);

        // 当日去重
        Assert.Empty(CategoryLimitChecker.CategoriesToNotify(seconds, limits, notified));
    }

    [Fact]
    public void 未达限额_不通知()
    {
        var seconds = new Dictionary<string, double> { ["娱乐"] = 3599 };
        var limits = new Dictionary<string, int> { ["娱乐"] = 60 };
        Assert.Empty(CategoryLimitChecker.CategoriesToNotify(seconds, limits, new HashSet<string>()));
    }

    [Fact]
    public void 零限额与缺省_不通知()
    {
        var seconds = new Dictionary<string, double> { ["娱乐"] = 99999 };
        var limits = new Dictionary<string, int> { ["娱乐"] = 0, ["办公"] = 60 };
        // 娱乐 0 限额；办公 无数据
        var result = CategoryLimitChecker.CategoriesToNotify(seconds, limits, new HashSet<string>());
        Assert.DoesNotContain("娱乐", result);
        Assert.DoesNotContain("办公", result);
    }
}
