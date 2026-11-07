# AppUsageTracker（软件使用时间统计）

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-stable-orange.svg)](https://www.rust-lang.org/)
[![Tauri](https://img.shields.io/badge/Tauri-2-24C8D8.svg)](https://v2.tauri.app/)
[![Svelte](https://img.shields.io/badge/Svelte-5-FF3E00.svg)](https://svelte.dev/)
[![Platform](https://img.shields.io/badge/Platform-Windows%2010%2F11-lightgrey.svg)](#)

常驻 Windows 系统托盘的软件使用时间统计工具，**只统计前台激活使用时间**（ManicTime / RescueTime 口径）。Rust 核心 + Tauri 2 壳 + Svelte 5 前端，独立 exe 约 11 MB。所有数据只保存在本机，无任何网络上传。

> 本分支（`rewrite/rust-tauri2`）为 Rust 全量重写版，旧 C#/WPF 实现保留在 `main`/`dev` 分支历史中。

## 功能特性

### 统计核心
- **前台活跃时间**：只统计软件获得前台焦点、且用户有键鼠操作的时长
  - 鼠标键盘无操作超过阈值（默认 180 秒，可配置）自动暂停计时，区间精确截止在最后一次输入时刻
  - 锁屏（安全桌面检测）、睡眠/休眠自动停止计时，睡眠时长不会被计入
  - 跨午夜区间按自然日自动切分；程序自身、资源管理器默认不统计
- **分类统计**：内置「开发 / 浏览器 / 办公 / 沟通 / 设计 / 娱乐 / 工具 / 系统」八类关键词规则，支持 `分类=关键词1|关键词2` 自定义；排行右键可手动指定分类（对历史数据同样生效）
- **分类限额提醒**：为任意分类设置每日分钟限额，首次越限弹系统通知
- **数据安全**：内存增量聚合，每 30 秒 + 状态切换 + 退出时落库（SQLite WAL，绝对覆盖式 upsert 幂等可重放）；崩溃最多丢 30 秒数据

### 界面
- **桌面悬浮窗**：真·亚克力磨砂置顶小窗（Win11），实时显示统计状态、今日活跃、当前应用（含本轮秒表）与 Top5 排行；整窗可拖动、默认左上角、拖出屏幕自动钳回；不抢焦点
- **仪表盘**：今天/昨天/本周/本月/自定义日期区间；汇总卡片、应用排行（占比条）、分类占比、每日活跃趋势；今日数据随秒级 tick 自动刷新
- **托盘**：左键打开主界面；右键菜单（悬浮窗 ✓ / 暂停监控 ✓ / 设置 / 退出）
- **CSV 导出**：UTF-8 BOM 兼容 Excel，导出后自动打开所在目录
- 开机自启（HKCU Run）、单实例、关闭主窗 = 隐藏到托盘

## 构建与运行

环境要求：Windows 10/11 + [Rust stable (MSVC)](https://rustup.rs/) + [VS Build Tools（C++ 工作负载）](https://visualstudio.microsoft.com/visual-cpp-build-tools/) + Node.js 20+。

```bash
npm install

# 开发联调（Vite 热更新）
npm run tauri dev

# 单元测试（聚合状态机：区间切分/空闲/锁屏/跨午夜/排除）
cargo test -p tracker-core

# 生产构建（NSIS 安装包 + 独立 exe）
npm run tauri build
# 产物：target/release/app-usage-tracker.exe（独立 exe 可直接运行）
#       target/release/bundle/nsis/*.exe（安装包）
```

启动后默认只显示悬浮窗；主界面从托盘左键打开，关闭主窗只是隐藏到托盘，退出请用托盘右键菜单。

## 项目结构

```
├── crates/tracker-core/        # Rust 核心库（与 UI 无关，可单测）
│   └── src/
│       ├── aggregator.rs       # 聚合状态机（Active/Idle/Locked/Suspended/Paused）
│       ├── category.rs         # 分类解析（关键词匹配 + 手动覆盖）
│       ├── store.rs            # SQLite（rusqlite，WAL）
│       ├── settings.rs         # 设置与默认分类规则
│       ├── limits.rs           # 分类限额纯函数
│       ├── win32.rs            # 前台窗口/空闲/锁屏 Win32 封装
│       └── models.rs           # 数据模型
├── src-tauri/                  # Tauri 2 壳
│   └── src/
│       ├── lib.rs              # 监控线程 / IPC 命令 / 托盘 / 悬浮窗
│       ├── version_info.rs     # exe FileDescription 提取（显示名）
│       └── autostart.rs        # 开机自启注册表
└── src/                        # Svelte 5 前端（多页构建）
    ├── App.svelte              # 主窗口（仪表盘 + 设置）
    ├── Dashboard.svelte        # 仪表盘
    ├── SettingsView.svelte     # 设置
    ├── Widget.svelte           # 悬浮窗（widget.html 入口）
    └── lib/api.ts              # IPC 封装与类型
```

## 浏览器扩展（可选）

统计浏览器标签页的网页浏览时长（按域名聚合）：

1. 设置 → 隐私 → 开启「网页浏览统计」（默认关闭）
2. Chrome/Edge 打开 `chrome://extensions` → 开发者模式 → 「加载已解压的扩展程序」→ 选择仓库的 `extension/` 目录
3. 扩展每 60s 把活动标签页 URL+标题发到本地 API，时间线页出现「Top 网页」

## 数据库维护

设置 → 数据库维护 → 「完整性检查 + VACUUM」：一键检查数据库完整性并压缩文件。

## 本地 HTTP API

设置中开启后（默认开，仅监听 `127.0.0.1:59598`），第三方工具可读取统计：

```
GET /                                          端点索引
GET /api/live                                  实时状态（当前应用/今日总时长/Top5）
GET /api/dashboard?from=&to=                   区间聚合（应用排行/分类占比/趋势）
GET /api/timeline?date=YYYY-MM-DD              单日时间线（明细片段 + 状态区间）
GET /api/settings                              当前设置
GET /api/version                               版本
POST /api/web-heartbeat                        浏览器心跳 {url, title}（需开启网页统计）
GET /api/web-top?from=&to=                     Top 网页（按域名+标题聚合）
```

## 数据存储

- 数据目录：`%LocalAppData%\AppUsageTracker\`（可在设置中更改，改后自动迁移历史数据）
  - `usage.db`：SQLite WAL（`ForegroundIntervals` 明细片段 + `DailyAppUsage` 按天聚合）
  - `settings.json`：全部设置（含分类规则）
- 隐私：不记录窗口标题、不记录网址、无任何网络请求

## License

[MIT](LICENSE)
