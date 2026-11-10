# 后续开发计划

> 状态快照：2026-11-11（本地时区 Asia/Shanghai）。
> 本文件记录待办与优先级，**不是交付承诺**；每完成一项，就把用户可见的变化写进 [CHANGELOG.md](../CHANGELOG.md) 的 `## [Unreleased]`，从本文件删除或标记「已完成」。
> 依据：仓库当前代码与配置（`package.json`、各 `Cargo.toml`、`src-tauri/tauri.conf.json`、`.github/workflows/`）、README 与 CHANGELOG。凡未在本机实际运行验证的判断都标注【推断】。

## 当前基线

- 已发布版本 `v0.6.1`（2026-11-08），`main` 与 `dev` 的代码树一致。
- `dev` 上有一批**已实现、已验证、尚未发布**的改动，详见 CHANGELOG `[Unreleased]`：主界面「悬浮窗」开关、悬浮窗 ✕ 关闭按钮、贴边收起未贴边提示、贴边动画去蓝闪、锁定徽标改行内、对比视图默认区间与变化率修正、仪表盘四卡单行、更新代理开关 `update_use_proxy`、移除界面配色、扩展智能安装。
- 代码规模（行）：

| 位置 | 说明 |
| --- | --- |
| `src-tauri/src/lib.rs` | 3304 行，监控线程 / 36 个 IPC 命令 / 托盘 / 悬浮窗 / 更新 / 扩展安装全在一起 |
| `crates/tracker-core/src/` | aggregator 509、settings 480、store 420、win32 141、models 83、category 32、limits 22、lib 12 |
| `src/` | Dashboard 1215、SettingsView 1048、TimelineView 592、CompareView 496、CalendarView 494、Widget 428、App 248、lib/api.ts 289、lib/i18n.ts 233 |

- 测试：`crates/tracker-core` 有 13 处 `#[test]`（aggregator 9 + store 4，`cargo test` 报告 11 passed）；`src-tauri/src/` **0 个测试**；前端**没有测试框架**（`package.json` scripts 只有 dev / build / preview / check / tauri）。
- CI（`.github/workflows/ci.yml`）：`frontend`、`rust`、`tauri-build`、`cross-check` 四个 job，外加聚合 job `ci`（必需检查名）。`cross-check` 目前只跑 `windows-2022`（`macos-14` 在该账号不可用已移除）。

## 优先级总览

| 优先级 | 主题 | 目标 |
| --- | --- | --- |
| P0 | 发布 0.6.2 | 把 `dev` 上这批改动发出去，主干与标签回到一致 |
| P1 | 质量内建 | 让 `src-tauri` 与前端也有自动化测试，收敛既有告警 |
| P2 | 架构收敛 | 拆 `lib.rs`、去掉 `tauri` 的 `unstable`、收紧 CSP |
| P3 | 功能演进 | 国际化补全、跨平台、数据可迁移 |
| P4 | 工程化 | 统一换行与格式化、依赖与安全基线 |

---

## P0 发布 0.6.2

- [ ] **发版**：四处版本号（`package.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`crates/tracker-core/Cargo.toml`）从 `0.6.1` 提到 `0.6.2`，把 `[Unreleased]` 收进 `## [0.6.2]`，按既有流程 `dev` → `main`（受 `required_linear_history` 保护，需 rebase 出线性分支）→ 打 tag → 等 Release 工作流产包并核对资产（`AppUsageTracker_0.6.2_x64-setup.exe`、`.exe.sig`、`latest.json`、`latest-mirror.json`、`SHA256SUMS`）。
  - 验收：Release 页面 `v0.6.2` 为 Latest，五个资产齐全；`main` 与 `dev` 代码树一致；应用内「检查更新」能从 0.6.1 升到 0.6.2（真机验证）。
- [ ] **去掉重复注册的 IPC 命令**：`src-tauri/src/lib.rs:207-208` 连续两行都是 `get_top_titles`。目前能编译也能通过检查，但属无意义重复，删掉其中一行。
  - 验收：`cargo clippy --all-targets -- -D warnings` 仍为 0；前端标题面板功能不变。
- [ ] **删除配色残留死代码**：`src-tauri/src/lib.rs:989` 注释仍写 `w.color.X`、`:1012-1013` 仍保留 `id.strip_prefix("w.color.")` 分支（右键菜单已不产生该类 id）。`accent_tint`（`:957-968`）与 `WidgetPrefs.accent` 可保留——它仍用于 Windows 亚克力着色，只是没有 UI 入口，需在注释里写明这一点。
  - 验收：右键菜单行为不变；`cargo test` 与 `npm run check` 全绿。

## P1 质量内建

- [ ] **前端测试框架**：引入 `vitest` 与组件测试（`@testing-library/svelte` 或 Playwright），先覆盖纯逻辑：`localDateKey`、区间切换、`orderedDelta`（对比视图变化率，本次刚修过）、`limitOf` 等。CI `frontend` job 增加 `npm test`。
- [ ] **`src-tauri` 命令层可测化**：把纯逻辑（`extension_id_from_hash`、CRX3 header 解析 `crx_id_bytes`/`signed_data_crx_id`、`manifest_version`、`accent_tint`）抽到 `crates/tracker-core` 或独立模块并加单测；它们目前嵌在 3304 行的 `lib.rs` 里、0 覆盖。
  - 验收：`cargo test --all --locked` 中 `src-tauri` 相关测试数从 0 增加到覆盖上述函数的规模。
- [ ] **清理 svelte-check 既有告警**：`src/SettingsView.svelte` 的 `modal-mask`/`modal` div click（`a11y_click_events_have_key_events`、`a11y_no_static_element_interactions`）、`src/Dashboard.svelte` 的 `.card .value.small` 未使用选择器、`src/TimelineView.svelte:18` 的 `state_referenced_locally`。目标是把 `npm run check` 的输出降为 0 条。
- [ ] **端到端冒烟**：一条脚本化路径「启动 → 打开主窗口 → 切换页面 → 开/关悬浮窗 → 检查更新」，用 Playwright 驱动 WebView 或用 CLI 参数化的自检模式【推断：Tauri 2 下推荐 `tauri-driver` + WebDriver】。

## P2 架构收敛

- [ ] **拆分 `src-tauri/src/lib.rs`（3304 行）**：按职责切成 `monitor.rs`（监控线程与聚合）、`windows.rs`（主窗/悬浮窗/动画/钳位）、`tray.rs`、`commands/`（按域分文件）、`update.rs`、`ext_install.rs`。先做纯移动 + `pub(crate)` 可见性调整，不改行为，每步都用 `cargo check`/`clippy`/`test` 验证。
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
- [ ] **依赖升级策略**：`tauri` 2、`svelte` 5、`vite` 6 都在大版本演进期，建议按季度批量小步升级，每次升级用 `npm run check` + `cargo clippy` + 真机冒烟作为门槛。

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
