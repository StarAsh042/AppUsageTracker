# 后续开发计划

> 状态快照：2026-11-16（本地时区 Asia/Shanghai）。`v0.6.4` 已于同日发布（工程质量版本），P0 与 P1 的前端测试项均已完成。
> 本文件记录待办与优先级，**不是交付承诺**；每完成一项，就把用户可见的变化写进 [CHANGELOG.md](../CHANGELOG.md) 的 `## [Unreleased]`，从本文件删除或标记「已完成」。
> 依据：仓库当前代码与配置（`package.json`、各 `Cargo.toml`、`src-tauri/tauri.conf.json`、`.github/workflows/`）、README 与 CHANGELOG。凡未在本机实际运行验证的判断都标注【推断】。

## 当前基线

- 已发布版本 `v0.6.4`（2026-11-16），`main` 与 `dev` 的代码树一致。
- `v0.6.4` 为工程质量版本，详见 CHANGELOG `[0.6.4]`：前端接入 vitest（29 例纯逻辑单测、CI 增加 `npm test`）、GitHub Actions 全量升到 Node 24 运行时、修复仪表盘右栏分类占比盖住效率趋势图。
- `v0.6.3`（2026-11-16）的功能批次，详见 CHANGELOG `[0.6.3]`：JSON 备份升到 v2（新增窗口标题与网页浏览区间）、备份/恢复入口、智能安装扩展到 17 款 Chromium 浏览器、对比视图默认「今天 vs 昨天」、顶栏「悬浮窗」按钮左移；修复加载失败三态、开机自启静默与老路径规范化、日历日期钉死、排除应用过滤、扩展误打包与私钥残留、分类规则实时生效、恢复事务化与备份校验等。
- `v0.6.2`（2026-11-11）包含：主界面「悬浮窗」开关、悬浮窗 ✕ 关闭按钮、贴边收起提示、贴边动画去蓝闪、锁定徽标行内显示、仪表盘四卡单行、更新代理开关 `update_use_proxy`、移除界面配色、扩展智能安装首版。
- 代码规模（行）：

| 位置 | 说明 |
| --- | --- |
| `src-tauri/src/lib.rs` | 1114 行（`update.rs`、`tray.rs`、`windows.rs`、`monitor.rs`、`ext_install.rs`、`api_server.rs`、`backup.rs`、`dashboard.rs`、`timeline.rs`、`apps.rs` 已切出），监控线程 / IPC 命令 / 托盘 / 悬浮窗 / 更新 / 扩展安装全在一起；另有 `autostart.rs` 43 行 |
| `crates/tracker-core/src/` | store 595、aggregator 509、settings 481、win32 141、models 83、category 32、limits 22、lib 13 |
| `src/` | Dashboard 1280、SettingsView 1121、TimelineView 632、CalendarView 494、CompareView 479、Widget 428、App 266、lib/api.ts 288、lib/i18n.ts 242、lib/compare.ts 75（+ 测试 165）、lib/limits.ts 17（+ 测试 46） |

- 测试：`crates/tracker-core` 有 13 处 `#[test]`（aggregator 9 + store 4，`cargo test` 报告 11 passed）；`src-tauri/src/` **0 个测试**；前端已接入 `vitest`（`npm test`），`src/lib/compare.test.ts` 22 例 + `src/lib/limits.test.ts` 7 例，共 29 例通过，覆盖纯逻辑层；组件级测试与 e2e 仍未接入。
- CI（`.github/workflows/ci.yml`）：`frontend`、`rust`、`tauri-build`、`cross-check` 四个 job，外加聚合 job `ci`（必需检查名）。`frontend` job 步骤为 `npm ci` → `npm run check` → `npm test` → `npm run build`。所有 action 已升到 Node 24 运行时（`checkout`/`setup-node` v5、`upload-artifact` v6、`download-artifact` v7）。`cross-check` 目前只跑 `windows-2022`（`macos-14` 在该账号不可用已移除）。

## 优先级总览

| 优先级 | 主题 | 目标 |
| --- | --- | --- |
| P0 | 发布 0.6.4 ✅ | 已完成：`v0.6.4` 已发布，主干与标签一致 |
| P1 | 质量内建 | 让 `src-tauri` 与前端也有自动化测试，收敛既有告警 |
| P2 | 架构收敛 | 拆 `lib.rs`、去掉 `tauri` 的 `unstable`、收紧 CSP |
| P3 | 功能演进 | 国际化补全、跨平台、数据可迁移 |
| P4 | 工程化 | 统一换行与格式化、依赖与安全基线 |

---

## P0 发布 0.6.4

- [x] **发版（`v0.6.4`，2026-11-16）**：工程质量版本。四处版本号（`package.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`crates/tracker-core/Cargo.toml`）从 `0.6.3` 提到 `0.6.4` 并同步 `Cargo.lock`，把 `[Unreleased]` 收进 `## [0.6.4]`，走 `dev` → `main`（`git read-tree` 整体取 dev 树，避开两边无共同祖先导致的 add/add 冲突）→ 打 tag → 等 Release 工作流。
  - 验收：Release 页面 `v0.6.4` 为 Latest，五个资产齐全；`main` 与 `dev` 代码树一致。
- [x] **发版（`v0.6.3`，2026-11-16）**：四处版本号（`package.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`crates/tracker-core/Cargo.toml`）从 `0.6.2` 提到 `0.6.3` 并同步 `Cargo.lock`，把 `[Unreleased]` 收进 `## [0.6.3]`，按既有流程 `dev` → `main`（受 `required_linear_history` 保护，需 rebase 出线性分支）→ 打 tag → 等 Release 工作流产包并核对资产（`AppUsageTracker_0.6.3_x64-setup.exe`、`.exe.sig`、`latest.json`、`latest-mirror.json`、`SHA256SUMS`）。
  - 验收：Release 页面 `v0.6.3` 为 Latest，五个资产齐全；`main` 与 `dev` 代码树一致；应用内「检查更新」能从 0.6.2 升到 0.6.3（真机验证）。
- [x] **发版（`v0.6.2`，2026-11-11）**：四处版本号 `0.6.1` → `0.6.2`，`[Unreleased]` 收进 `## [0.6.2]`，`dev` → `main` → tag → Release 资产五项齐全。
- [x] **去掉重复注册的 IPC 命令**：`src-tauri/src/lib.rs:207-208` 连续两行都是 `get_top_titles`。目前能编译也能通过检查，但属无意义重复，删掉其中一行。
  - 验收：`cargo clippy --all-targets -- -D warnings` 仍为 0；前端标题面板功能不变。
- [x] **删除配色残留死代码**：`src-tauri/src/lib.rs:989` 注释仍写 `w.color.X`、`:1012-1013` 仍保留 `id.strip_prefix("w.color.")` 分支（右键菜单已不产生该类 id）。`accent_tint`（`:957-968`）与 `WidgetPrefs.accent` 可保留——它仍用于 Windows 亚克力着色，只是没有 UI 入口，需在注释里写明这一点。
  - 验收：右键菜单行为不变；`cargo test` 与 `npm run check` 全绿。

## P1 质量内建

- [x] **前端测试框架（纯逻辑层）**：已引入 `vitest` 5.1.0（`npm test` / `npm run test:watch`），把 `localDateKey`、区间切换（`presetRange` 的 today / yesterday / week / month 四种预设）、`orderedDelta` 变化率与 `limitOf` / `setDailyLimit` 抽到 `src/lib/compare.ts` 与 `src/lib/limits.ts` 并加 29 个用例；CI `frontend` job 已加 `npm test`。
- [x] **数据库体积治理**：建库时设 `wal_autocheckpoint=1000` 与 `journal_size_limit=1048576`，监控线程每 5 分钟 `wal_checkpoint(TRUNCATE)` 一次，退出 / 切换数据目录 / 冒烟模式下各补一次；设置页新增「数据库维护」面板，把原本有后端没入口的 `db_maintenance` 命令接上（完整性检查 + VACUUM + WAL 截断 + 体积统计）。
  - 验收：`cargo test --all --locked` 从 22 例增加到 25 例（checkpoint 不丢数据、打开后 WAL 上限已配置、页大小与页数自洽）；实测对线上库执行 truncate checkpoint 能把 7.3 MB 的 `usage.db-wal` 归零。
- [ ] **组件测试与 e2e**：仍是空白。组件级可用 `@testing-library/svelte`，端到端见下方「端到端冒烟」。
- [x] **`src-tauri` 命令层可测化**：纯逻辑已抽到 `crates/tracker-core` 的 `ext.rs`（`read_varint`、`crx_id_bytes`、`signed_data_crx_id`、`extension_id_from_hash`、`manifest_version`）与 `accent.rs`（`accent_tint`），`src-tauri/src/lib.rs` 只留薄封装，调用点不变。
  - 验收：`cargo test --all --locked` 从 12 例增加到 22 例，其中 10 例覆盖上述函数（varint 单/多字节与截断、CRX3 正常与四类坏输入、signed_data 只认 len 16 的 field 1、扩展 ID 编码、manifest_version 读取与回退、accent_tint 三色）。
- [x] **清理 svelte-check 既有告警**：弹窗外壳抽成 `src/lib/Modal.svelte`（遮罩 click、Esc 关闭、`role="dialog"`、打开即聚焦），`Dashboard`/`SettingsView` 两处弹窗改为复用它——顺带修掉设置页那个弹窗此前完全没有样式（类未定义、无 `:global`）的问题；排行行 `div` → `button`；`TimelineView` 的 `state_referenced_locally` 改为一次性 `$effect.pre` 赋值；`Widget` 根节点补 `role="application"`；删掉死 CSS `.card .value.small`。
  - 验收：`npm run check` 由 13 warnings / 4 files 降为 0 errors 0 warnings。
- [ ] **端到端冒烟**：一条脚本化路径「启动 → 打开主窗口 → 切换页面 → 开/关悬浮窗 → 检查更新」，用 Playwright 驱动 WebView 或用 CLI 参数化的自检模式【推断：Tauri 2 下推荐 `tauri-driver` + WebDriver】。

## P2 架构收敛

- [ ] **拆分 `src-tauri/src/lib.rs`（1114 行）**：按职责切成 `monitor.rs`（监控线程与聚合）、`windows.rs`（主窗/悬浮窗/动画/钳位）、`tray.rs`、`commands/`（按域分文件）、`update.rs`、`ext_install.rs`。先做纯移动 + `pub(crate)` 可见性调整，不改行为，每步都用 `cargo check`/`clippy`/`test` 验证。
  - 进度：`update.rs` 已切出（96 行，含 `updater_endpoints`、`UpdateInfo`、`check_update`、`install_update`）；`tray.rs` 已切出（113 行，含 `TrayHandles`、`build_tray` 与托盘菜单事件、退出前 flush + checkpoint）；`windows.rs` 已切出（488 行，含主窗/悬浮窗、DWM 背板与圆角、贴边收起展开、尺寸动画、悬浮窗右键菜单与三种窗口尺寸常量）；`monitor.rs` 已切出（321 行，含 `self_stem`、`foreground_identity`、`spawn_monitor` 主循环、`flush_state`、`purge_expired`、`check_limits`）；`ext_install.rs` 已切出（583 行，含 `extension_dir`、`materialize_extension`、`pack_extension`、`register_external_extension`、`install_extension_blocking` 与 `BrowserSpec` 表）；`api_server.rs` 已切出（308 行，含本地 HTTP API 与浏览器扩展心跳）；`backup.rs` 已切出（229 行，含 JSON 备份导出与恢复）；`dashboard.rs` 已切出（210 行，含仪表盘数据聚合与 `get_dashboard`、`build_dashboard`）；`timeline.rs` 已切出（81 行，含时间线数据与 `get_timeline`、`timeline_data`）；`apps.rs` 已切出（217 行，含应用详情与应用扫描）。`lib.rs` 从 3727 行降到 1114 行。每切一个模块合一次 PR。
- [ ] **移除 `tauri` 的 `unstable` feature**：`src-tauri/Cargo.toml` 里 `features = ["tray-icon", "unstable"]`。先定位用到的不稳定 API（悬浮窗动态创建 / 窗口位置相关）【推断】，再用稳定 API 替代或隔离。
- [ ] **收紧 CSP**：`src-tauri/tauri.conf.json` 的 `app.security.csp` 目前是 `null`。改成只允许 `'self'` 与必要的 `style-src 'unsafe-inline'`（Svelte 内联样式），逐页验证不白屏。
- [ ] **窗口定义收敛**：widget 窗口是代码里 `create_widget_window` 动态建的（`tauri.conf.json` 的 windows 数组只有 main）。评估改为配置声明 + 运行时只改位置，减少命令式建窗的分支。

## P3 功能演进

- [ ] **国际化补全**：`src/lib/i18n.ts` 目前是中文键 → 英文值的字典（46 条），只有包了 `t()` 的文案才会切换语言。实测各视图中文出现次数与 `t()` 调用数差距很大：TimelineView 21 处中文但仅 3 个 `t()`，Widget 41 / 7，SettingsView 150 / 100。要真正实现「中英双语」，需要把文案抽成 key 而非中文串，或至少逐个补齐 `t()`。
- [ ] **跨平台**：目前只有 Windows 有发布包与实机验收，`cross-check` 只剩 `windows-2022`。macOS 需要先解决 runner 不可用问题，再补签名/公证；Linux 需要补 WebKitGTK 依赖与打包目标。均为【未验证】。
- [ ] **数据可迁移**：`0.3.0` 是 C#/WPF → Rust 的破坏性重写，仓库没有旧版数据迁移说明；`settings.rs` 里有 `schema_version` 与迁移链的位置，但历史数据（若用户从 C# 版迁移）没有路径。评估做一个一次性导入工具或明确声明不支持。
- [ ] **更新回滚**：当前 updater 只前进不后退，回滚表现【未验证】。评估记录「上一个可用版本」并在启动连续失败时提示手动回退。
- [ ] **周报自动化**：`export_weekly_report`（`src-tauri/src/lib.rs:2623`）已能导出 Markdown 周报，可加「每周一自动生成并可选通知」的开关。

## P4 工程化

- [ ] **统一换行符**：仓库没有 `.gitattributes`，`src-tauri/src/lib.rs` 是 LF、README 与 `src/lib/api.ts` 等是 CRLF，`git status` 会周期性冒出 `src-tauri/gen/schemas/*.json` 的伪改动（LF/CRLF 缓存差异）。加一条 `* text=auto` 并一次性归一化。
- [ ] **格式化与静态检查**：前端没有 Prettier / ESLint，只靠 `svelte-check` 做类型检查；Rust 侧已有 `cargo fmt` + `clippy -D warnings` 在 CI 强制。补齐前端对应工具并加进 CI `frontend` job。
- [ ] **安全基线**：仓库无 `SECURITY.md`、无依赖更新机器人（Dependabot/Renovate）。本地 HTTP API 仅监听 `127.0.0.1`，但 token 与来源校验的边界值得单独写清。
- [x] **CI action 运行时升级**：GitHub Actions 的 Node 20 运行时已弃用，`checkout` / `setup-node` 升到 `v5`、`upload-artifact` 升到 `v6`、`download-artifact` 升到 `v7`（`upload-artifact` v5 与 `download-artifact` v5/v6 仍是 node20，不能只升一档）。
- [ ] **依赖升级策略**：`tauri` 2、`svelte` 5、`vite` 6 都在大版本演进期，建议按季度批量小步升级，每次升级用 `npm test` + `npm run check` + `cargo clippy` + 真机冒烟作为门槛。

---

## 明确不做

- 不做云端同步 / 账号体系：本项目定位是本地优先的单机工具，任何联网都只出现在更新检查。
- 不做浏览器扩展商店上架：扩展只服务本机 `127.0.0.1` 心跳，走外部扩展注册表已经够用。
- 不主动改写 `CHANGELOG.md` 的历史条目（例如 0.6.1 里提到的「主题设置并入外观区块」），历史记录保持原样。

## 维护约定

1. 所有用户可见变化先进 `[Unreleased]`，发布时整体移入新版本标题，**不直接改已发布版本的条目**。
2. 分支：`main` = 发布，`dev` = 集成，两者都受保护禁止直推。改动走 `feat/`、`fix/`、`docs/` 前缀分支 → 向 `dev` 开 PR → CI 的 `ci` 检查全绿 → squash 合并并删分支。
3. 版本号四处同步（见 P0 第一条），tag 在 `main` 的 CI 全绿后推送 `vMAJOR.MINOR.PATCH`。
4. 本文件每完成一项就同步更新；新增待办写清「现状证据 + 动作 + 验收」，不写无验收标准的口号。
