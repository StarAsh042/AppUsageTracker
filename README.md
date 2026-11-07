# AppUsageTracker（软件使用时间统计）

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![.NET](https://img.shields.io/badge/.NET-8.0-purple.svg)](https://dotnet.microsoft.com/download/dotnet/8.0)
[![Platform](https://img.shields.io/badge/Platform-Windows%2010%2F11-lightgrey.svg)](#)

常驻 Windows 系统托盘的软件使用时间统计工具，**只统计前台激活使用时间**（ManicTime / RescueTime 口径），提供分类统计、单日时间线、趋势洞察与 CSV 导出。所有数据只保存在本机，无任何网络上传。

## 功能特性

### 统计核心
- **前台活跃时间**：只统计软件获得前台焦点、且用户有键鼠操作的时长
  - 鼠标键盘无操作超过阈值（默认 180 秒，可配置）自动暂停计时，区间精确截止在最后一次输入时刻
  - 锁屏、睡眠/休眠自动停止计时，睡眠时长不会被计入
  - 跨午夜区间按自然日自动切分
  - 后台进程、服务、系统组件一律不统计
- **分类统计**（借鉴 ActivityWatch 的 Categories）：内置「开发 / 办公 / 沟通 / 浏览器 / 设计 / 娱乐 / 工具」七类规则，按关键词匹配，可在设置中以 `分类=关键词1|关键词2` 格式自定义；仪表盘以环形图 + 图例展示分类占比
- **单日时间线**（借鉴 ActivityWatch 的 Timeline）：24 小时色块回溯当天每个时段在用什么软件，悬停显示「应用 起止（时长）」
- **洞察对比**：日均活跃时长、当前周期 vs 上一周期变化百分比
- **久坐提醒**：连续活跃使用达到设定分钟数（默认 60，可关）托盘气泡提醒休息
- **数据保留策略**：按天自动清理过期明细（默认 365 天，0 为永久）
- **数据安全**：内存增量聚合，每 30 秒 + 状态切换 + 退出时落库（SQLite WAL），落库失败自动放回内存重试；崩溃最多丢 30 秒数据

### 界面
- **磨砂玻璃质感**：中性灰阶配色（底色无彩色倾向），Windows 11 下应用系统 Acrylic 背板 + 圆角，无边框自绘标题栏
- **桌面悬浮窗**：置顶小窗实时显示今日活跃（秒表级跳动）与当前前台应用，左键拖动、位置记忆，托盘或设置中开关
- **实时秒表**：仪表盘总活跃/日均时长每秒刷新（已落库数据 + 当前未闭合区间）
- **主题**：跟随 Windows 深浅色自动切换（也可手动指定），窗口边框随主题联动
- 排行榜只列有实际使用的软件（0 秒不显示），带图标、分类色点、占比条；近 14 天趋势图
- 托盘：双击打开仪表盘，右键菜单（悬浮窗 / 暂停监控 / 开机自启 / 设置 / 退出），菜单配色跟随主题
- CSV 导出（日期、分类、应用、进程名、活跃秒数，UTF-8 BOM 兼容 Excel）

## 参与开发

> **给外部贡献者的重要说明**
>
> `main` 分支受保护，且仅维护者可直接推送。
> **请向 `dev` 分支提交 Pull Request；直接向 `main` 提交的 PR 大概率不会受理。**

推荐流程：

1. Fork 本仓库到你自己账号
2. 从 `dev` 分支切出特性分支（`git checkout -b feature/xxx`）
3. 提交改动并推送到你的 Fork
4. 向本仓库的 **`dev` 分支**开 Pull Request，等待 CI 通过与合并

分支流向：`feature/xxx` → `dev`（日常集成）→ `main`（发布版本）

## 构建与运行

环境要求：Windows 10/11 + [.NET 8 SDK](https://dotnet.microsoft.com/download/dotnet/8.0)。

```bash
dotnet build
dotnet test
dotnet run --project src/AppUsageTracker.App

# 发布单文件 exe（自包含，目标机器无需装 .NET，输出 publish/AppUsageTracker.exe）
dotnet publish src/AppUsageTracker.App -c Release -r win-x64 --self-contained true `
  -p:PublishSingleFile=true -p:IncludeNativeLibrariesForSelfExtract=true `
  -p:EnableCompressionInSingleFile=true -p:DebugType=none -o publish
```

> Git Bash 下把续行符换成 `\`，且 `-p:` 不要写成 `/p:`（MSYS 会误转换）。

启动后主窗口显示仪表盘；关闭窗口只是隐藏到托盘，退出请用托盘右键菜单。

## 数据存储

- 数据库：`%LocalAppData%\AppUsageTracker\usage.db`（SQLite，WAL 模式）
  - `ForegroundIntervals`：前台使用明细（起止片段，供时间线与回溯）
  - `DailyAppUsage`：按天 × 应用聚合
- 设置：`%AppData%\AppUsageTracker\settings.json`
- 日志：`%LocalAppData%\AppUsageTracker\logs\`（按天滚动，含 WPF 绑定错误转发）

## 自动化冒烟测试

```bash
AppUsageTracker.App.exe --smoke 10
```

无界面监控 N 秒后落库，摘要写入 `%LocalAppData%\AppUsageTracker\smoke-report.json`，退出码 0 表示当日有数据。

## 项目结构

```
AppUsageTracker/
├── src/AppUsageTracker.Core/          # 核心库（不依赖 UI，可单测）
│   ├── Interop/NativeMethods.cs       # Win32 P/Invoke（前台窗口、输入空闲检测）
│   ├── Monitoring/
│   │   ├── ForegroundMonitor.cs       # 前台焦点 1s 轮询
│   │   ├── SessionEventSource.cs      # 锁屏/解锁/睡眠/恢复事件
│   │   ├── UsageAggregator.cs         # 状态机 + 内存聚合（纯逻辑，核心可测点）
│   │   ├── CategoryResolver.cs        # 分类规则引擎
│   │   └── AppIdentityResolver.cs     # PID → 应用标识/显示名（带缓存）
│   ├── Storage/SqliteUsageStore.cs    # SQLite 持久化（明细 + 每日增量 upsert + 保留清理）
│   ├── Tracking/TrackingService.cs    # 编排：监控装配、周期落库、久坐提醒、保留策略
│   └── TrackerSettings.cs             # 设置持久化
├── src/AppUsageTracker.App/           # WPF 外壳（net8.0-windows）
│   ├── App.xaml(.cs)                  # 单实例、托盘装配、主题应用、冒烟模式、绑定错误日志
│   ├── Services/ThemeManager.cs       # 中性磨砂主题调色板，Auto 跟随 Windows（UserPreferenceChanged）
│   ├── Services/WindowChrome.cs       # DWM Acrylic 背板 / Win11 圆角
│   ├── Services/TrayController.cs     # 托盘图标与菜单（WinForms NotifyIcon）
│   ├── Services/AppIconCache.cs       # 应用图标提取缓存 + 图标运行时绘制（与 exe 图标同款）
│   ├── Controls/DonutChart.cs         # 自绘分类环形图
│   ├── Controls/TimelineView.cs       # 自绘 24 小时单日时间线
│   ├── Views/FloatingWidgetWindow.cs  # 桌面悬浮窗（实时今日活跃 + 当前应用）
│   ├── ViewModels/                    # CommunityToolkit.Mvvm
│   └── Views/                         # DashboardWindow、SettingsWindow
└── tests/AppUsageTracker.Core.Tests/  # xUnit，25 个用例（状态机全路径 + 分类 + 存储）
```

## 测试

```bash
dotnet test
```

覆盖：前台切换累计、亚秒毛刺过滤、空闲阈值进出与区间截断、跨午夜切分、锁屏/解锁、睡眠/恢复、手动暂停、排除名单、分类规则匹配、SQLite 读写 round-trip、增量累加语义与保留清理。

## 已知限制

- 无法取得 exe 路径的受保护系统进程不参与统计（不影响用户软件）
- 浏览器只按应用整体统计，不区分具体网站
- 数据目录暂不支持运行期迁移
- 时间线仅支持单日查看

## 后续可扩展方向

- 分类规则的界面化管理（拖拽归类）
- 每小时使用热力图（周视图）
- 数据看板导出 HTML/PDF 周报

## License

[MIT](LICENSE) © 2026 StarAsh042
