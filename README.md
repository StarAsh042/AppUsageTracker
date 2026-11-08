# AppUsageTracker

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Tauri 2](https://img.shields.io/badge/Tauri-2-24C8D8.svg)](https://v2.tauri.app/)
[![Svelte 5](https://img.shields.io/badge/Svelte-5-FF3E00.svg)](https://svelte.dev/)
[![Platform: Windows 10/11](https://img.shields.io/badge/Platform-Windows%2010%2F11-lightgrey.svg)](#兼容性)

AppUsageTracker 是一款 Windows 桌面端软件使用时间统计工具，用于按前台激活时长记录并分析应用与分类的使用情况，适合需要回顾自己电脑使用习惯的个人用户。

## 目录

- [简介](#简介)
- [功能](#功能)
- [截图与演示](#截图与演示)
- [不适用](#不适用)
- [前置条件](#前置条件)
- [安装](#安装)
- [快速开始](#快速开始)
- [配置](#配置)
- [浏览器扩展（可选）](#浏览器扩展可选)
- [本地 HTTP API](#本地-http-api)
- [数据存储与维护](#数据存储与维护)
- [兼容性](#兼容性)
- [权限与 capability](#权限与-capability)
- [隐私说明](#隐私说明)
- [自动更新](#自动更新)
- [项目结构](#项目结构)
- [文档导航](#文档导航)
- [贡献](#贡献)
- [许可证](#许可证)
- [维护状态与已知限制](#维护状态与已知限制)

## 简介

AppUsageTracker 在后台记录「哪个应用在前台被你主动使用了多久」，按应用与自定义分类聚合，提供仪表盘、时间线、日历热力图、对比视图与周报导出。核心统计在 `crates/tracker-core`（Rust），桌面壳为 Tauri 2，界面为 Svelte 5。

它解决的是「想客观回顾自己每天把时间花在哪些软件上」这一个问题；数据只落在本机 SQLite 文件里，不依赖账号或云端服务。目标用户是个人使用者；它不是员工监控工具，也不做跨设备同步。

当前发布版本为 `0.6.0`（2026-11-07）。本仓库工作区位于 `rewrite/rust-tauri2` 分支，即 Rust + Tauri 2 全量重写版；远端 `main` / `dev` 目前仍是旧 C#/WPF 版本，重写尚未合入，详见「维护状态与已知限制」。

## 功能

### 统计与聚合

- **前台活跃时间**：只统计应用获得前台焦点、且用户有键鼠操作的时长；键鼠无操作超过阈值（默认 180 秒，可配置）自动暂停，并把区间精确截止到最后一次输入时刻。
- **锁屏与睡眠感知**：检测到安全桌面（锁屏）或系统睡眠 / 休眠时停止计时，睡眠时长不计入。
- **跨午夜切分**：跨越自然日的区间按天自动切分；程序自身与资源管理器默认不统计，可在设置中调整排除列表。
- **分类统计**：内置「开发 / 浏览器 / 办公 / 沟通 / 设计 / 娱乐 / 工具 / 系统」八类关键词规则，支持 `分类=关键词1|关键词2` 自定义，也可在排行中手动指定分类（对历史数据同样生效）。
- **分类限额**：为任意分类设置每日分钟限额，首次越限时弹出系统通知。
- **落库与崩溃安全**：内存增量聚合，每 30 秒、状态切换与退出时写入 SQLite（WAL 模式）；异常退出最多丢失 30 秒数据。

### 界面与导出

- **桌面悬浮窗**：Win11 Mica 磨砂置顶小窗，实时显示统计状态、今日活跃、当前应用（含本轮秒表）与 Top5 排行；可整窗拖动，默认左上角，拖出屏幕自动钳回，不抢焦点。
- **仪表盘**：今天 / 昨天 / 本周 / 本月 / 自定义区间，含汇总卡、应用排行、分类占比与每日活跃趋势；今日数据随秒级 tick 自动刷新。
- **时间线**：24 小时色带，支持滚轮缩放、拖拽平移、明细表与搜索，并标注统计中 / 空闲 / 锁定 / 挂起状态区间。
- **日历热力图与对比视图**：GitHub contribution 风格的年度热力图（月 / 年切换），以及双时段应用 / 分类并排对比。
- **托盘**：左键打开主界面；右键菜单提供悬浮窗开关、暂停监控、设置与退出。
- **导出**：CSV（UTF-8 BOM，兼容 Excel，导出后自动打开所在目录）、JSON 全量备份 / 恢复、Markdown 周报。
- **其他**：开机自启（HKCU Run）、单实例、关闭主窗隐藏到托盘、全局热键、休息提醒、中英双语界面。

### 可选记录

- **窗口标题**：默认关闭；开启后时间线页出现「Top 标题」，可用排除规则按子串过滤。
- **网页浏览**：默认关闭；开启并安装浏览器扩展后，按域名与标题聚合网页时长，时间线页出现「Top 网页」。详见「浏览器扩展（可选）」与「隐私说明」。

### 本地集成

- **本地 HTTP API**：默认开启，仅监听 `127.0.0.1:59598`，供第三方工具读取统计。详见「本地 HTTP API」。

## 截图与演示

暂无截图。本仓库当前没有任何真实窗口截图文件，按书写规范不放置占位图；补齐 `docs/assets/` 下的真实桌面窗口截图是一项待办，见「维护状态与已知限制」。界面外观相关描述均为【未验证】：AI 无法启动 GUI，请你在安装后手动核对。

## 不适用

- 不是企业员工监控或上网行为审计工具，不提供远程管理、截图留存或键盘记录。
- 不提供账号体系、云同步或跨设备汇总；数据只保存在当前 Windows 机器的本地目录。
- 不提供 macOS / Linux 的正式发布包；这两个平台仅有 CI 交叉编译检查（见「兼容性」）。
- 不替代系统自带的屏幕使用时间功能做移动端统计。

## 前置条件

运行时（使用已发布安装包）：

- Windows 10 或 Windows 11，x86_64。
- WebView2 Runtime（Windows 11 与已安装 Edge 的 Windows 10 通常已具备）。

从源码构建：

- Rust stable，MSVC 工具链（`rustup` 的 `stable-x86_64-pc-windows-msvc`）。
- Visual Studio Build Tools，勾选「使用 C++ 的桌面开发」工作负载。
- Node.js >= 20（CI 使用 Node.js 22）与随附的 npm；本仓库使用 `package-lock.json`，包管理器为 npm，不使用 pnpm / yarn。
- Linux 上构建 Tauri 还需 `libwebkit2gtk-4.1-dev`、`libayatana-appindicator3-dev`、`librsvg2-dev`、`patchelf`（仅 CI 的 Ubuntu 任务使用）。

以下命令均以仓库根目录为当前目录。

## 安装

从已发布安装包安装（推荐给使用者）：

1. 打开 [GitHub Releases](https://github.com/StarAsh042/AppUsageTracker/releases)，在 `v0.6.0` 中下载 `AppUsageTracker_0.6.0_x64-setup.exe`（NSIS 安装包）。
2. 双击运行安装向导，按提示完成安装。
3. 从开始菜单启动。

卸载：Windows「设置 → 应用 → 已安装的应用」中找到 AppUsageTracker 并选择卸载；或再次运行安装包选择卸载。卸载不会自动删除 `%LocalAppData%\AppUsageTracker\` 下的统计数据，如需清理请手动删除该目录。

验证安装：启动后本地 API 应可访问（默认开启）：

```bash
curl -s http://127.0.0.1:59598/api/version
```

预期返回包含当前版本号 `0.6.0` 的 JSON（字段结构以实际响应为准）。系统托盘出现图标、主界面显示仪表盘属于界面现象，标记【未验证】，请你手动确认。

> 安装包未做 Windows Authenticode 代码签名；发布流水线只生成 Tauri 更新器使用的 `.sig` 签名。首次运行可能触发 SmartScreen 提示（【推测】，未在真机验证）。

从源码构建（面向开发者）见「快速开始」；构建产物为 `target/release/app-usage-tracker.exe`（独立 exe）与 `target/release/bundle/nsis/*_0.6.0_x64-setup.exe`（安装包）。

## 快速开始

只使用已发布软件：按「安装」三步完成，启动后从托盘左键打开主界面；停止请用托盘右键 → 退出（关闭主窗口只会隐藏到托盘）。

从源码运行（Vite 热更新 + Tauri 开发窗口）：

```bash
npm ci
npm run tauri dev
```

- 启动命令：`npm run tauri dev`。
- 入口：`src-tauri/tauri.conf.json` 的 `beforeDevCommand` 先执行 `npm run dev`（Vite，`http://localhost:5173`），随后 Tauri 打开窗口。
- 预期现象：终端输出 Vite 本地地址，随后弹出应用窗口与悬浮窗（【未验证】，请你手动确认）。
- 停止方式：在终端按 `Ctrl+C`。

首次构建需要下载并编译 Rust 依赖，耗时可能超过 10 分钟，请预留时间。

其他常用命令：

```bash
npm run check                               # svelte-check 类型检查
npm run build                               # 仅构建前端产物到 dist/
cargo test -p tracker-core                  # 聚合状态机单元测试
cargo fmt --all --check                     # 与 CI 一致的格式检查
cargo clippy --all-targets -- -D warnings   # 与 CI 一致的 lint
npm run tauri build                         # 生产构建（NSIS 安装包 + 独立 exe）
```

上述命令与 `.github/workflows/ci.yml` 一致；本地未逐一运行，标记【未验证】。

## 配置

本项目通过图形界面「设置」修改配置，持久化到数据目录下的 `settings.json`；没有命令行参数与环境变量覆盖。因此优先级简化为：**设置页修改 → `settings.json` → 内置默认值**。设置项即时生效；其中托盘菜单语言、开机自启等需要重启应用后完全生效。

键名是 `settings.json` 中的 snake_case 字段（本项目无环境变量，故无 UPPER_SNAKE_CASE 键）。默认值来自 `crates/tracker-core/src/settings.rs` 的 `TrackerSettings::default()`。

| 键 | 必需 | 默认值 | 说明 | 示例 | 敏感 |
| --- | --- | --- | --- | --- | --- |
| `idle_threshold_seconds` | 否 | `180` | 空闲判定阈值（秒）；无键鼠操作超过该值即暂停计时 | `300` | 否 |
| `foreground_poll_ms` | 否 | `1000` | 前台窗口轮询间隔（毫秒） | `1000` | 否 |
| `start_with_windows` | 否 | `false` | 开机自启（写入 HKCU Run） | `true` | 否 |
| `excluded_app_keys` | 否 | `["c:\\windows\\explorer.exe"]` | 不统计的应用列表 | — | 否 |
| `data_directory` | 否 | `%LocalAppData%\AppUsageTracker` | 数据目录；更改后会迁移历史数据 | — | 否 |
| `theme` | 否 | `auto` | 界面主题 | `dark` | 否 |
| `rest_reminder_minutes` | 否 | `60` | 休息提醒间隔（分钟，`0` 关闭） | `45` | 否 |
| `retention_days` | 否 | `365` | 统计数据保留天数 | `90` | 否 |
| `show_floating_widget` | 否 | `true` | 是否显示桌面悬浮窗 | `true` | 否 |
| `daily_goal_minutes` | 否 | `0` | 每日活跃目标（分钟，`0` 关闭） | `240` | 否 |
| `hotkeys_enabled` | 否 | `true` | 是否启用全局热键 | `true` | 否 |
| `hotkey_main` / `hotkey_widget` | 否 | `Ctrl+Alt+D` / `Ctrl+Alt+W` | 主界面 / 悬浮窗热键，保存时校验有效性与冲突 | `Ctrl+Shift+P` | 否 |
| `api_enabled` | 否 | `true` | 本地 HTTP API 开关 | `false` | 否 |
| `api_port` | 否 | `59598` | 本地 HTTP API 端口（仅回环地址） | `59598` | 否 |
| `record_window_titles` | 否 | `false` | 是否记录前台窗口标题（隐私敏感） | `false` | 是 |
| `title_exclude_patterns` | 否 | `[]` | 标题排除规则，子串匹配、不区分大小写 | `["银行"]` | 是 |
| `record_web` | 否 | `false` | 是否启用网页浏览统计（需安装扩展，隐私敏感） | `false` | 是 |
| `web_heartbeat_interval_secs` | 否 | `10` | 网页心跳间隔（秒）；MV3 定时器下限 30 秒 | `30` | 否 |
| `language` | 否 | `system` | 界面语言：`system`（中文）/ `en` | `en` | 否 |
| `update_channel` | 否 | `auto` | 更新下载通道：`auto` / `official` / `mirror` | `official` | 否 |
| `schema_version` | 否 | `1` | 设置结构版本，迁移链使用，不建议手改 | `1` | 否 |
| `widget` | 否 | 见 `WidgetPrefs` | 悬浮窗偏好：置顶、锁定、贴边、缩略、透明度、配色 | — | 否 |

> 表格中的示例值仅用于说明，不覆盖默认值。改变 `data_directory` 会触发历史数据迁移，操作前请先做 JSON 备份。

## 浏览器扩展（可选）

`extension/` 是一个 Manifest V3 扩展（`manifest.json` + `background.js`），把当前标签页的 URL 与标题通过本机回环地址上报给应用，按域名聚合浏览时长。

1. 在「设置 → 隐私」开启「网页浏览统计」（`record_web`，默认关闭；关闭时心跳返回 403）。
2. Chrome / Edge 打开 `chrome://extensions` → 开启「开发者模式」→「加载已解压的扩展程序」→ 选择仓库的 `extension/` 目录。
3. 扩展按 `web_heartbeat_interval_secs` 上报；默认 10 秒，但 MV3 定时器最小间隔为 30 秒，实际不会低于 30 秒（实际行为【未验证】）。
4. 时间线页出现「Top 网页」面板。

扩展的 `host_permissions` 仅为 `http://127.0.0.1:59598/*`，不请求其他站点权限（`extension/manifest.json`）。

## 本地 HTTP API

默认开启（`api_enabled: true`），只监听 `127.0.0.1:59598`，不对外网开放。端点：

```text
GET  /                              端点索引
GET  /api/live                      实时状态（当前应用 / 今日总时长 / Top5）
GET  /api/dashboard?from=&to=       区间聚合（应用排行 / 分类占比 / 趋势）
GET  /api/timeline?date=YYYY-MM-DD  单日时间线（明细片段 + 状态区间）
GET  /api/settings                  当前设置
GET  /api/version                   版本
GET  /api/web-top?from=&to=         Top 网页（按域名 + 标题聚合）
POST /api/web-heartbeat             浏览器心跳 {url, title}（需开启网页浏览统计）
```

使用示例：

```bash
curl -s "http://127.0.0.1:59598/api/dashboard?from=2026-11-01&to=2026-11-07"
```

预期返回该日期区间的应用排行、分类占比与趋势数据（字段结构以实际响应为准）。`POST /api/web-heartbeat` 在 `record_web` 关闭时返回 403。

## 数据存储与维护

- 默认数据目录：`%LocalAppData%\AppUsageTracker\`（可在设置中更改，更改时自动迁移历史数据）。
- `usage.db`：SQLite（WAL 模式）。`ForegroundIntervals` 保存前台明细片段，`DailyAppUsage` 保存按天聚合；开启隐私开关后还会记录窗口标题区间与网页浏览区间。
- `settings.json`：全部设置（含分类规则与排除列表）。

维护：设置 → 数据库维护 → 「完整性检查 + VACUUM」，可一键检查数据库完整性、压缩文件并查看页数 / 空闲页统计。设置 → 常规 → 「清空全部数据」会删除统计数据且不可撤销，请先做 JSON 备份。

## 兼容性

| 项目 | 状态 |
| --- | --- |
| 支持操作系统 | Windows 10 / Windows 11，x86_64（【已验证】：v0.6.0 发布包由 Windows 构建任务产出） |
| 安装包格式 | NSIS `.exe` 安装包；另提供独立 exe `app-usage-tracker.exe` |
| 系统要求 | WebView2 Runtime；Windows 11 与装有 Edge 的 Windows 10 通常已具备 |
| 签名 / 公证 | 更新包使用 minisign / ed25519 签名校验（`latest.json` 内含 `.sig`）；安装包未做 Windows Authenticode 代码签名，也无 macOS 发布包、未做公证 |
| 自动更新 | 内置 tauri-plugin-updater，行为见「自动更新」 |
| Linux / macOS | 仅 CI 交叉编译检查（`cross-check` 覆盖 windows-2022 与 macos-14；Ubuntu 任务执行 `npm run tauri build`），**未发布、未做平台验收**，标记【未验证】 |

CI 编译通过不等于真实平台验收；上表未标注「已验证」的平台均不作为可用性承诺。

## 权限与 capability

Tauri 权限清单在 `src-tauri/capabilities/default.json`：identifier 为 `default`，作用于 `main` 与 `widget` 两个窗口，权限为 `core:default` 加上 9 条 `core:window:allow-*`（开始拖拽、设置位置、读取窗口位置与尺寸、当前显示器、缩放因子、可见性、显示、隐藏），未使用 `allow-all`。

应用实际使用的系统资源：Win32 前台窗口 / 空闲 / 锁屏检测、HKCU Run 注册表项（仅开机自启开关打开时）、本地 SQLite 文件、`127.0.0.1:59598` 回环监听、浏览器扩展回环上报。

`src-tauri/tauri.conf.json` 中 `app.security.csp` 为 `null`，`tauri` 依赖启用了 `unstable` feature，两者属于已知技术债，不在此宣称任何额外安全能力（【静态推断】：基于配置文件与依赖清单，未做动态权限验证）。`identifier` 为 `com.starash042.app-usage-tracker`，按项目约束一经设置不得变更。

## 隐私说明

- **默认不记录窗口标题，也不记录网页 URL 与标题**：`record_window_titles` 与 `record_web` 在 `crates/tracker-core/src/settings.rs:143-144` 中默认均为 `false`。
- 打开 `record_window_titles` 后，前台窗口标题会写入标题区间，时间线出现「Top 标题」；可用 `title_exclude_patterns` 按子串排除。打开 `record_web` 并安装扩展后，网页 URL 与标题写入网页区间。两个开关都可在「设置 → 隐私」随时关闭。
- **统计数据只保存在本机** `%LocalAppData%\AppUsageTracker\`，不上传第三方服务器，不提供账号体系。
- **网络访问仅用于检查与下载更新**：更新清单 `latest.json` 来自 `https://github.com/StarAsh042/AppUsageTracker/releases/latest/download/latest.json`；`mirror` / `auto` 通道会经 `https://gh-proxy.com/` 下载。浏览器扩展只访问本机 `127.0.0.1:59598`。除此之外不发起外发请求（【静态推断】：依据 `tauri.conf.json` 的 updater endpoints、`extension/manifest.json` 的 host_permissions 与 `src-tauri/src/lib.rs` 中的 `.no_proxy()` / 下载逻辑，未做抓包验证）。
- 不记录键盘输入、屏幕截图或文件内容（【静态推断】：代码中未发现相关采集路径，未做动态验证）。
- 安全问题：仓库暂无 `SECURITY.md`，请通过 [GitHub Issues](https://github.com/StarAsh042/AppUsageTracker/issues) 反馈，避免公开可复现的敏感细节。

## 自动更新

内置 `tauri-plugin-updater`。在「设置」中点击检查更新时，应用读取 `tauri.conf.json` 配置的 `latest.json`，用其中的 minisign / ed25519 公钥校验更新包签名，再下载安装。`update_channel` 支持：

- `auto`（默认）：先尝试官方 GitHub Releases 地址，失败回退到镜像。
- `official`：只用官方地址。
- `mirror`：只用 `gh-proxy.com` 镜像地址。

回滚：自动更新不提供自动降级。需要回退时请手动下载旧版本安装包覆盖安装；数据目录与 `usage.db` 保持不变（【未验证】：仓库未提供回滚流程说明，未在真机验证）。

## 项目结构

```text
├── crates/tracker-core/        # Rust 核心库（与 UI 无关，可单测）
│   └── src/
│       ├── aggregator.rs       # 聚合状态机（Active/Idle/Locked/Suspended/Paused）
│       ├── category.rs         # 分类解析（关键词匹配 + 手动覆盖）
│       ├── store.rs            # SQLite（rusqlite，WAL）
│       ├── settings.rs         # 设置与默认分类规则
│       ├── limits.rs           # 分类限额纯函数
│       ├── win32.rs            # 前台窗口 / 空闲 / 锁屏 Win32 封装
│       └── models.rs           # 数据模型
├── src-tauri/                  # Tauri 2 壳
│   ├── capabilities/           # 权限清单
│   └── src/
│       ├── lib.rs              # 监控线程 / IPC 命令 / 托盘 / 悬浮窗
│       ├── version_info.rs     # exe FileDescription 提取（显示名）
│       └── autostart.rs        # 开机自启注册表
├── src/                        # Svelte 5 前端（多页构建）
│   ├── App.svelte              # 主窗口（仪表盘 + 设置）
│   ├── Dashboard.svelte        # 仪表盘
│   ├── SettingsView.svelte     # 设置
│   ├── Widget.svelte           # 悬浮窗（widget.html 入口）
│   └── lib/api.ts              # IPC 封装与类型
├── extension/                  # 可选 MV3 浏览器扩展
├── .github/workflows/          # CI 与 Release 流水线
├── src-tauri/tauri.conf.json   # 应用版本、窗口、更新器配置
└── package.json                # 前端脚本与依赖
```

## 文档导航

- [CHANGELOG.md](CHANGELOG.md)：各版本用户可见变更。
- [LICENSE](LICENSE)：MIT 许可证全文。
- [GitHub Releases](https://github.com/StarAsh042/AppUsageTracker/releases)：安装包与更新清单。
- [GitHub Issues](https://github.com/StarAsh042/AppUsageTracker/issues)：问题反馈。
- 贡献流程见本文「贡献」一节；远端 `main` 分支 README 的 [参与开发](https://github.com/StarAsh042/AppUsageTracker/blob/main/README.md#参与开发) 一节记录了当前仓库的 PR 约定。

## 贡献

仓库采用双分支模型：`main` 为发布分支、`dev` 为集成分支，两者均受分支保护，禁止直接推送。单人维护，以「CI + 自检清单 + PR 记录」代替多人评审，审批人数设为 0。

推荐流程：

1. Fork 仓库。
2. 从 `dev` 切出 `feature/xxx`、`fix/xxx`、`docs/xxx` 等分支（命名随变更类型）。
3. 在 Fork 上提交，提交信息使用 Conventional Commits（如 `docs(readme): clarify privacy defaults`）。
4. 向本仓库的 `dev` 分支开 PR，等待必需 CI 检查 `ci` 通过。
5. 按单人自检清单逐项核对后，用 squash 合并，并删除已合并的分支。

`main` 的更新来自 `dev` 的合并；版本标签 `vMAJOR.MINOR.PATCH` 在 `main` CI 全绿后由维护者手动推送。请勿直接向 `main` 推送或放宽分支保护。

> 本次工作区位于 `rewrite/rust-tauri2` 分支（Rust 重写），远端 `main` / `dev` 仍是旧 C#/WPF 版本；贡献前请先确认目标分支的实际内容。

## 许可证

[MIT](LICENSE) © 2026 StarAsh042。

## 维护状态与已知限制

- 单人维护；当前发布版本 `v0.6.0`，发布日期 2026-11-07。版本事实源为 `src-tauri/tauri.conf.json`、`package.json`、`src-tauri/Cargo.toml` 与 `crates/tracker-core/Cargo.toml`，四者均为 `0.6.0`，且与 tag `v0.6.0` 及 Latest Release 一致。
- 工作区在 `rewrite/rust-tauri2` 分支（Rust + Tauri 2 + Svelte 5 重写）；远端 `main` / `dev` 仍是旧 C#/WPF 实现（远端 `main` README 的构建段仍写「.NET 8 SDK」），重写尚未合入。
- `main` 最近一次 CI（`docs: add contributing guide (#1)`）中 `ci` 与 `security` 检查失败、`build` 与 `test` 通过；后续 PR 的必需检查可能持续失败，需先修复 CI。
- 仓库暂无真实窗口截图，按书写规范不放置占位图；补齐 `docs/assets/` 下的真实截图是发布前应完成的事项。
- `CHANGELOG.md` 的 `0.1.0` / `0.2.0` 属于旧 C# 版历史记录，没有对应的 Git tag 与 GitHub Release，因此没有 compare 链接；`0.3.0` 为破坏性全量重写，仓库未提供旧版数据到新版数据的迁移说明。
- `src-tauri/tauri.conf.json` 中 `app.security.csp` 为 `null`、`tauri` 依赖启用 `unstable` feature，属于待收敛的技术债。
- 界面外观、扩展上报间隔、更新回滚与未签名安装包的 SmartScreen 表现均为【未验证】或【推测】，需由你在真机手动验证；AI 无法启动 GUI。
- 本文件与 `CHANGELOG.md` 的命令、版本与配置默认值均取自仓库内 `package.json`、各 `Cargo.toml`、`tauri.conf.json`、`settings.rs` 与 `.github/workflows/`；未做外部联网核实，未在本机逐条运行，相关命令标【未验证】。
