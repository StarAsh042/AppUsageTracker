using System.IO;
using System.Text.Json;
using System.Text.Json.Serialization;
using AppUsageTracker.Core.Models;

namespace AppUsageTracker.Core;

/// <summary>程序设置，以 JSON 持久化在 %AppData%\AppUsageTracker\settings.json。</summary>
public sealed class TrackerSettings
{
    /// <summary>鼠标键盘无操作多少秒后视为空闲并暂停前台计时。</summary>
    public int IdleThresholdSeconds { get; set; } = 180;

    /// <summary>前台窗口轮询间隔（毫秒）。</summary>
    public int ForegroundPollMs { get; set; } = 1000;

    /// <summary>进程存活轮询间隔（毫秒）。</summary>
    public int AlivePollMs { get; set; } = 5000;

    /// <summary>是否开机自启（对应 HKCU Run 注册表键）。</summary>
    public bool StartWithWindows { get; set; }

    /// <summary>排除统计的应用（AppKey，即小写 exe 全路径），每项一个。</summary>
    public List<string> ExcludedAppKeys { get; set; } = new();

    /// <summary>前台为 Shell（桌面/explorer）时不计入活跃，因为那意味着没有真正使用任何软件。</summary>
    public bool ExcludeShell { get; set; } = true;

    /// <summary>数据目录（存放 usage.db、日志、图标缓存等）。</summary>
    public string DataDirectory { get; set; } = DefaultDataDirectory();

    /// <summary>界面主题：Auto（跟随 Windows）/ Dark / Light。</summary>
    public string Theme { get; set; } = "Auto";

    /// <summary>连续活跃使用多少分钟后提醒休息，0 = 关闭。</summary>
    public int RestReminderMinutes { get; set; } = 60;

    /// <summary>明细数据保留天数（按天清理，0 = 永久保留）。</summary>
    public int RetentionDays { get; set; } = 365;

    /// <summary>是否显示桌面悬浮窗。</summary>
    public bool ShowFloatingWidget { get; set; } = true;

    /// <summary>悬浮窗位置（屏幕坐标，-1 表示使用默认位置）。</summary>
    public int WidgetX { get; set; } = -1;
    public int WidgetY { get; set; } = -1;

    /// <summary>分类规则（按顺序匹配，未命中归入「未分类」）。</summary>
    public List<CategoryRule> CategoryRules { get; set; } = CreateDefaultCategoryRules();

    /// <summary>按 AppKey（小写 exe 路径）指定的分类覆盖，优先于规则匹配。</summary>
    public Dictionary<string, string> AppCategoryOverrides { get; set; } = new(StringComparer.OrdinalIgnoreCase);

    /// <summary>分类每日限额（分钟），0 或缺省 = 不限。</summary>
    public Dictionary<string, int> CategoryDailyLimitMinutes { get; set; } = new(StringComparer.OrdinalIgnoreCase);

    [JsonIgnore]
    public string SettingsPath { get; set; } = DefaultSettingsPath;

    public static string DefaultDataDirectory() =>
        Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "AppUsageTracker");

    public static string DefaultSettingsPath =>
        Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData), "AppUsageTracker", "settings.json");

    /// <summary>
    /// 内置默认分类（借鉴 ActivityWatch 的类别思路）。
    /// 覆盖 Windows 系统组件、本机实际安装的软件（含联网核实过的 DSH NEXT、DeskBox、
    /// 软媒魔方、Samsung Magician 等），以及网上常见的主流软件。
    /// 匹配按行序优先：「系统」靠后兜底 Windows 自带组件，未命中计入「未分类」。
    /// 注意：浏览器排在沟通前（否则 "qq" 会先命中 QQ 浏览器）。
    /// </summary>
    public static List<CategoryRule> CreateDefaultCategoryRules() =>
    [
        new() { Name = "开发", Keywords = ["code", "visual studio", "vscode", "devenv", "cursor", "windsurf", "git", "github", "gitlab", "sourcetree", "gitkraken", "tortoisegit", "beyond compare", "bash", "cmd", "powershell", "terminal", "windowsterminal", "conemu", "cmder", "tabby", "xshell", "finalshell", "mobaxterm", "putty", "winscp", "filezilla", "idea", "pycharm", "webstorm", "clion", "goland", "rider", "datagrip", "rubymine", "fleet", "android studio", "deveco", "eclipse", "netbeans", "hbuilder", "dotnet", "node", "npm", "yarn", "pnpm", "python", "anaconda", "conda", "jupyter", "java", "gradle", "maven", "bun", "deno", "sublime", "notepad++", "emeditor", "vscode", "vim", "neovim", "postman", "apifox", "navicat", "dbeaver", "sqlyog", "heidisql", "redis", "docker", "podman", "vmware", "virtualbox", "hype-v", "wsl", "ilspy", "dnspy", "x64dbg", "ghidra", "ida ", "ollydbg", "burp", "charles", "fiddler", "wireshark", "nmap", "zcode", "dsh", "deepseek", "harness", "chatgpt", "openai", "claude", "copilot", "ollama", "qoder", "opencode", "workbuddy", "tortoisegit", "mathematica"] },
        new() { Name = "浏览器", Keywords = ["chrome", "chromium", "edge", "msedge", "webview2", "firefox", "waterfox", "browser", "浏览器", "opera", "brave", "vivaldi", "safari", "arc", "360se", "360 极速", "qqbrowser", "搜狗浏览器", "uc浏览器", "夸克", "星愿", "猎豹", "2345浏览器", "thorium"] },
        new() { Name = "办公", Keywords = ["word", "excel", "powerpoint", "outlook", "onenote", "access", "visio", "project", "publisher", "wps", "office", "金山文档", "腾讯文档", "石墨文档", "语雀", "notion", "印象笔记", "evernote", "有道云", "为知笔记", "obsidian", "notepad", "acrobat", "福昕", "foxit", "sumatra", "calibre", "pdf", "xmind", "mindmaster", "幕布", "滴答清单", "ticktick", "to do", "typora", "marktext", "libreoffice", "wordpad", "写字板", "mathematica", "openoffice", "onlyoffice", "欧路词典", "eudic", "有道词典", "金山词霸"] },
        new() { Name = "沟通", Keywords = ["wechat", "weixin", "微信", "企业微信", "wecom", "qq", "tim", "ntqq", "dingtalk", "钉钉", "feishu", "lark", "飞书", "slack", "teams", "telegram", "discord", "whatsapp", "signal", "skype", "zoom", "腾讯会议", "网易会议", "瞩目", "mail", "邮箱", "thunderbird", "foxmail", "脉脉"] },
        new() { Name = "设计", Keywords = ["photoshop", "illustrator", "indesign", "lightroom", "audition", "premiere", "after effects", "adobe xd", "figma", "sketch", "axure", "墨刀", "蓝湖", "mastergo", "即时设计", "blender", "cinema 4d", "c4d", "maya", "3ds max", "zbrush", "substance", "unity", "unreal", "godot", "davinci", "达芬奇", "final cut", "capcut", "剪映", "必剪", "万兴喵影", "filmora", "coreldraw", "affinity", "gimp", "krita", "inkscape", "painttool", "clip studio", "medibang", "canva", "画图", "mspaint"] },
        new() { Name = "娱乐", Keywords = ["game", "steam", "epic games", "gog ", "uplay", "origin", "ea app", "battle.net", "战网", "wegame", "riot client", "minecraft", "我的世界", "原神", "genshin", "league of legends", "英雄联盟", "cs2", "csgo", "valorant", "无畏契约", "dota", "崩坏", "itch.io", "mumu", "雷电模拟", "ldplayer", "nox", "bluestacks", "cloudmusic", "网易云", "spotify", "qqmusic", "qq音乐", "kugou", "kuwo", "酷狗", "酷我", "汽水音乐", "foobar", "musicbee", "apple music", "bilibili", "哔哩哔哩", "iqiyi", "爱奇艺", "优酷", "youku", "芒果tv", "腾讯视频", "youtube", "netflix", "douyu", "斗鱼", "huya", "虎牙", "douyin", "抖音", "快手", "potplayer", "vlc", "mpc", "kmplayer", "mx player", "暴风影音", "player", "迅雷", "thunder", "qbittorrent", "utorrent", "比特彗星", "xbox"] },
        new() { Name = "工具", Keywords = ["everything", "listary", "utools", "quicker", "snipaste", "pixpin", "sharex", "faststone", "greenshot", "flameshot", "7-zip", "winrar", "bandizip", "360压缩", "peazip", "motrix", "idm", "fdm", "free download manager", "aria2", "qbittorrent", "clash", "v2ray", "shadowsocks", "wireguard", "openvpn", "tailscale", "zerotier", "向日葵", "sunlogin", "todesk", "teamviewer", "anydesk", "rustdesk", "localsend", "火绒", "huorong", "360安全", "电脑管家", "金山毒霸", "kaspersky", "malwarebytes", "ccleaner", "dism++", "geek uninstaller", "revo uninstaller", "cpu-z", "gpu-z", "hwinfo", "aida64", "crystaldiskinfo", "crystaldiskmark", "hdtune", "diskgenius", "diskinfo", "partition", "easeus", "recuva", "treesize", "spacesniffer", "wiztree", "trafficmonitor", "鲁大师", "驱动人生", "驱动精灵", "driver booster", "电脑管家", "huawei", "supportassist", "samsung", "magician", "migration", "软媒", "魔方", "系统雷达", "desktopok", "autohotkey", "powertoys", "ditto", "copyq", "deskbox", "fences", "calculator", "计算器"] },
        new() { Name = "系统", Keywords = ["explorer", "资源管理器", "windows", "task manager", "任务管理器", "regedit", "注册表", "device manager", "设备管理器", "控制面板", "control panel", "系统设置", "settings", "winget", "store", "应用商店", "shell", "host", "widget", "runtime broker", "csrss", "dwm", "conhost", "ime", "输入法", "ctfmon", "sihost", "security", "defender", "smartscreen", "svchost", "svc", "services", "服务", "打印", "print", "audio", "音频", "spooler", "登录应用", "启动应用", "会话管理器", "桌面窗口管理器", "后台处理", "winlogon", "lsass", "fontdrvhost", "qingjian", "driver", "驱动", "update", "更新", "lamparray", "livecaptions", "voiceaccess", "oobe"] },
    ];

    /// <summary>
    /// 历史内置规则快照：用户从未改过分类规则（仍是任意一版内置默认）时，
    /// 加载时自动升级到最新默认；自定义过则原样保留。
    /// </summary>
    private static readonly List<List<CategoryRule>> LegacyDefaultSnapshots =
    [
        // v1：最初七类（无系统类）
        [
            new() { Name = "开发", Keywords = ["code", "visual studio", "vscode", "devenv", "git", "bash", "cmd", "powershell", "terminal", "idea", "pycharm", "rider", "dotnet", "node", "python", "sublime", "notepad++", "postman", "docker", "sql", "datagrip", "ssms", "cursor", "wireshark", "nmap"] },
            new() { Name = "办公", Keywords = ["word", "excel", "powerpoint", "outlook", "onenote", "wps", "office", "notepad", "acrobat", "pdf", "xmind", "typora", "obsidian", "visio", "access", "ppt"] },
            new() { Name = "沟通", Keywords = ["wechat", "weixin", "微信", "qq", "dingtalk", "钉钉", "feishu", "飞书", "slack", "teams", "telegram", "discord", "mail", "thunderbird", "foxmail"] },
            new() { Name = "浏览器", Keywords = ["chrome", "edge", "firefox", "browser", "浏览器", "opera", "brave", "safari", "360se", "qqbrowser"] },
            new() { Name = "设计", Keywords = ["photoshop", "illustrator", "figma", "sketch", "blender", "premiere", "davinci", "剪映", "after effects", "lightroom", "affinity", "paint", "inkscape", "canva", "stablediffusion", "stable diffusion", "comfyui", "aigc", "midjourney", "daz ", "marmoset", "speedtree", "worldcreator", "gaea", "pmxeditor", "mikumikudance", "kingdraw", "billfish", "pureref", "booru", "naiprompt"] },
            new() { Name = "娱乐", Keywords = ["game", "steam", "epic games", "music", "网易云", "spotify", "qqmusic", "kugou", "bilibili", "video", "player", "potplayer", "vlc", "抖音", "douyin", "iqiyi", "爱奇艺", "腾讯视频", "youtube", "斗罗", "dldl", "hollow knight", "honeyselect", "oxygen not included", "pal7", "sword and fairy", "warframe", "yuzu", "modmanager", "米喵", "fl studio", "kontakt", "pianoteq", "ace studio", "everyonepiano", "bodian"] },
            new() { Name = "工具", Keywords = ["everything", "listary", "utools", "quicker", "snipaste", "pixpin", "sharex", "7-zip", "winrar", "bandizip", "motrix", "idm", "clash", "v2ray", "mihomo", "vpn", "proxy"] },
        ],
        // v2：加入系统类与部分本机软件
        [
            new() { Name = "开发", Keywords = ["code", "visual studio", "vscode", "devenv", "cursor", "git", "github", "bash", "cmd", "powershell", "terminal", "windowsterminal", "idea", "pycharm", "rider", "datagrip", "dotnet", "node", "npm", "python", "bun", "deno", "sublime", "notepad++", "postman", "docker", "sql", "ssms", "wireshark", "nmap", "zcode", "dsh", "deepseek", "harness", "obsidian"] },
            new() { Name = "办公", Keywords = ["word", "excel", "powerpoint", "outlook", "onenote", "wps", "office", "notepad", "acrobat", "pdf", "xmind", "typora", "visio", "access", "ppt", "marker", "goodnotes"] },
            new() { Name = "沟通", Keywords = ["wechat", "weixin", "微信", "qq", "dingtalk", "钉钉", "feishu", "飞书", "slack", "teams", "telegram", "discord", "mail", "thunderbird", "foxmail"] },
            new() { Name = "浏览器", Keywords = ["chrome", "edge", "firefox", "browser", "浏览器", "opera", "brave", "safari", "360se", "qqbrowser", "webview2"] },
            new() { Name = "设计", Keywords = ["photoshop", "illustrator", "figma", "sketch", "blender", "premiere", "davinci", "剪映", "after effects", "lightroom", "affinity", "paint", "inkscape", "canva", "stablediffusion", "stable diffusion", "comfyui", "aigc", "midjourney", "daz ", "marmoset", "speedtree", "worldcreator", "gaea", "pmxeditor", "mikumikudance", "kingdraw", "billfish", "pureref", "booru", "naiprompt"] },
            new() { Name = "娱乐", Keywords = ["game", "steam", "epic games", "music", "网易云", "spotify", "qqmusic", "kugou", "bilibili", "video", "player", "potplayer", "vlc", "抖音", "douyin", "iqiyi", "爱奇艺", "腾讯视频", "youtube", "斗罗", "dldl", "hollow knight", "honeyselect", "oxygen not included", "pal7", "sword and fairy", "warframe", "yuzu", "modmanager", "米喵", "fl studio", "kontakt", "pianoteq", "ace studio", "everyonepiano", "bodian"] },
            new() { Name = "工具", Keywords = ["everything", "listary", "utools", "quicker", "snipaste", "pixpin", "sharex", "7-zip", "winrar", "bandizip", "motrix", "idm", "clash", "v2ray", "mihomo", "vpn", "proxy", "deskbox", "netlimiter", "defraggler", "formatfactory", "screentogif", "小丸", "按键精灵", "程序窗口控制", "synctrayzor", "syncthing", "baidunetdisk", "百度网盘", "adrive", "阿里云盘", "cheat engine", "ghostcat", "dev-sidecar", "samsung", "magician", "软媒", "魔方", "系统雷达", "cpu", "gpu", "diskinfo", "crystaldisk", "trafficmonitor"] },
            new() { Name = "系统", Keywords = ["explorer", "资源管理器", "windows", "shell", "host", "widget", "runtime broker", "csrss", "dwm", "conhost", "ime", "输入法", "security", "defender", "smart Screen", "svchost", "svc", "服务", "设置", "search", "打印", "print", "audio", "音频", "登录应用", "启动应用", "会话管理器", "桌面窗口管理器", "后台处理", "notepad system", "system informer"] },
        ],
    ];

    /// <summary>判断当前规则是否与任意一版历史默认完全一致（从未被用户修改过）。</summary>
    private static bool MatchesLegacyDefaults(List<CategoryRule> rules)
    {
        foreach (var snapshot in LegacyDefaultSnapshots)
        {
            if (MatchesSnapshot(rules, snapshot)) return true;
        }
        return false;
    }

    private static bool MatchesSnapshot(List<CategoryRule> rules, List<CategoryRule> snapshot)
    {
        if (rules.Count != snapshot.Count) return false;
        for (var i = 0; i < rules.Count; i++)
        {
            if (!string.Equals(rules[i].Name, snapshot[i].Name, StringComparison.Ordinal)) return false;
            if (rules[i].Keywords.Count != snapshot[i].Keywords.Count) return false;
            if (!rules[i].Keywords.ToHashSet(StringComparer.Ordinal)
                .SetEquals(snapshot[i].Keywords)) return false;
        }
        return true;
    }

    private static readonly JsonSerializerOptions JsonOptions = new()
    {
        WriteIndented = true,
        DefaultIgnoreCondition = JsonIgnoreCondition.Never,
    };

    public static TrackerSettings Load(string? path = null)
    {
        path ??= DefaultSettingsPath;
        try
        {
            if (File.Exists(path))
            {
                var settings = JsonSerializer.Deserialize<TrackerSettings>(File.ReadAllText(path), JsonOptions);
                if (settings != null)
                {
                    settings.SettingsPath = path;
                    settings.Normalize();
                    return settings;
                }
            }
        }
        catch (Exception ex)
        {
            Logging.Logger.Error("加载设置失败，使用默认设置", ex);
        }

        var defaults = new TrackerSettings { SettingsPath = path };
        defaults.Normalize();
        return defaults;
    }

    public void Save()
    {
        var dir = Path.GetDirectoryName(SettingsPath);
        if (!string.IsNullOrEmpty(dir)) Directory.CreateDirectory(dir);
        File.WriteAllText(SettingsPath, JsonSerializer.Serialize(this, JsonOptions));
    }

    private void Normalize()
    {
        IdleThresholdSeconds = Math.Clamp(IdleThresholdSeconds, 10, 3600);
        ForegroundPollMs = Math.Clamp(ForegroundPollMs, 200, 10_000);
        AlivePollMs = Math.Clamp(AlivePollMs, 1000, 60_000);
        RestReminderMinutes = Math.Clamp(RestReminderMinutes, 0, 480);
        RetentionDays = Math.Clamp(RetentionDays, 0, 3650);
        if (string.IsNullOrWhiteSpace(DataDirectory)) DataDirectory = DefaultDataDirectory();
        ExcludedAppKeys ??= new List<string>();
        CategoryRules ??= CreateDefaultCategoryRules();
        AppCategoryOverrides ??= new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);
        CategoryDailyLimitMinutes ??= new Dictionary<string, int>(StringComparer.OrdinalIgnoreCase);
        // Shell（explorer）不计入前台活跃：在桌面无实际软件使用
        if (!ExcludedAppKeys.Contains(@"c:\windows\explorer.exe", StringComparer.OrdinalIgnoreCase))
            ExcludedAppKeys.Add(@"c:\windows\explorer.exe".ToLowerInvariant());
        // 旧版默认规则自动升级为新默认（用户自定义过则原样保留）
        if (MatchesLegacyDefaults(CategoryRules)) CategoryRules = CreateDefaultCategoryRules();
        if (Theme is not ("Auto" or "Dark" or "Light")) Theme = "Auto";
    }
}
