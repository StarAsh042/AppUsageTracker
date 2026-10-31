using AppUsageTracker.Core.Models;
using AppUsageTracker.Core.Monitoring;
using Xunit;

namespace AppUsageTracker.Core.Tests;

public class CategoryResolverTests
{
    private static readonly CategoryResolver Resolver =
        new(TrackerSettings.CreateDefaultCategoryRules());

    [Theory]
    [InlineData("Cent Browser", "chrome", @"c:\program files\cent browser\chrome.exe", "浏览器")]
    [InlineData("Microsoft Visual Studio 2022", "devenv", @"c:\program files\...\devenv.exe", "开发")]
    [InlineData("WeChat", "weixin", @"c:\program files\tencent\wechat\wechat.exe", "沟通")]
    [InlineData("PotPlayer 64 位", "potplayer", @"d:\potplayer\potplayer.exe", "娱乐")]
    [InlineData("Photoshop", "photoshop", @"c:\program files\adobe\photoshop.exe", "设计")]
    [InlineData("Everything", "everything", @"c:\tools\everything.exe", "工具")]
    public void 内置规则_按关键词命中分类(string display, string process, string path, string expected)
    {
        Assert.Equal(expected, Resolver.Resolve(display, process, path));
    }

    [Fact]
    public void 未命中_归入未分类()
    {
        Assert.Equal(CategoryResolver.Uncategorized, Resolver.Resolve("某神秘软件", "mystery", @"e:\download\mystery.exe"));
    }

    [Fact]
    public void 匹配不区分大小写()
    {
        Assert.Equal("开发", Resolver.Resolve("GIT BASH", "bash", @"C:\PROGRAM FILES\GIT\GIT-BASH.EXE"));
    }

    [Fact]
    public void 应用级覆盖_优先于规则匹配()
    {
        var overrides = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase)
        {
            [@"e:\game\unknown\mystery.exe"] = "娱乐",
        };
        var resolver = new CategoryResolver(TrackerSettings.CreateDefaultCategoryRules(), overrides);

        // AppKey 命中覆盖：即使规则也能匹配，也以覆盖为准
        Assert.Equal("娱乐", resolver.Resolve("某未知程序", "mystery", @"E:\Game\Unknown\Mystery.exe"));
        // 未命中覆盖：走规则
        Assert.Equal("开发", resolver.Resolve("ZCode", "zcode", @"c:\program files\zcode\zcode.exe"));
    }

    [Fact]
    public void 自定义规则_优先于内置顺序()
    {
        var resolver = new CategoryResolver(new List<CategoryRule>
        {
            new() { Name = "写作", Keywords = ["obsidian"] },
            new() { Name = "办公", Keywords = ["office", "word"] },
        });

        Assert.Equal("写作", resolver.Resolve("Obsidian", "obsidian", @"c:\tools\obsidian.exe"));
        Assert.Equal("办公", resolver.Resolve("Word", "winword", @"c:\program files\office\winword.exe"));
    }
}
