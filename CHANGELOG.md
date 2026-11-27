# Changelog

本项目所有值得注意的用户可见变更都记录在此文件中。

格式依据 [Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/)，版本遵循 [Semantic Versioning 2.0.0](https://semver.org/spec/v2.0.0.html)。版本日期为发布当日日期（本地时区 Asia/Shanghai）；[GitHub Releases](https://github.com/StarAsh042/AppUsageTracker/releases) 的时间戳为 UTC，跨日显示可能相差一天。各版本标签与安装包见 [GitHub Releases](https://github.com/StarAsh042/AppUsageTracker/releases)。

本仓库为单人维护，发布提交多为直接提交或 squash 合并，历史条目没有可核实的 PR 编号，因此条目未附 PR / Issue 链接，也不编造链接。已提交但尚未发布的变更先追加到 `## [Unreleased]`；发布时由维护者移入新版本标题、同步 manifest 版本号并打标签。维护约定见 [README 的贡献章节](README.md#贡献与许可)。

## [Unreleased]

（暂无未发布变更）

## [0.6.6] - 2026-11-22

### Added

- **dashboard:** 应用排行新增应用图标，按可执行文件提取并缓存为 data URL；行内图标与名称同排显示。
- **dashboard:** 为常用应用与 Windows 系统工具补充中文备注（如 `Taskmgr(任务管理器)`、`百分浏览器`），中文备注仅在中文界面显示；名称过长时省略号截断，鼠标悬停滚动展示完整名称。

### Fixed

- **dashboard:** 应用排行「占比」表头与下方百分比右对齐，不再错位。

## [0.6.5] - 2026-11-21

### Added

- **settings:** 设置页新增「数据库维护」面板：一键做完整性检查 + VACUUM 并截断 WAL 日志，结果里显示检查结论、压缩结果、主库体积、页数与空闲页数。

### Changed

- **frontend:** 弹窗外壳抽成共用组件 `src/lib/Modal.svelte`（遮罩点击与 Esc 关闭、打开即把焦点移进弹窗、内容超高时自己滚动），仪表盘的应用详情与设置页的扩展安装两处共用。
- **frontend:** 仪表盘应用排行行由 `div` 改为 `button`，键盘可 Tab 聚焦、回车打开详情，聚焦时有可见描边。
- **frontend:** 时间线、仪表盘、对比、设置、悬浮窗与弹窗等处剩余的硬编码文案全部收进 `t()`（字典从 222 条增到 270 条），内置分类名在英文下显示为 Development / Browser 等。
- **app:** 休息提醒的通知正文与托盘悬停提示改走 `rust_i18n` 词条，英文下不再露出中文。

### Fixed

- **a11y:** `npm run check` 的 svelte-check 告警从 13 条清到 0 条；两处弹窗此前按 Esc 无反应，现已支持 Esc 关闭。
- **app:** 修复启动瞬间先闪一下白底、随后才变暗：主窗口启用 `noRedirectionBitmap` 并把 `backgroundColor` 设为全透明。
- **app:** 修复点「悬浮窗」按钮后主界面卡死、图表不再更新：建窗在同步 IPC 命令里等待主线程事件循环而死锁，改为在独立线程建窗。

### Internal

- **core:** 悬浮窗改为在 `src-tauri/tauri.conf.json` 的 `app.windows` 里声明（含 `"create": false`），`windows.rs` 按 label 读取该配置建窗，只保留随偏好变化的尺寸与置顶两项运行时覆写。
- **core:** 收紧 `app.security.csp`：从 `null` 改为白名单策略；去掉 `tauri` 的 `unstable` feature，`features` 只剩 `["tray-icon"]`。
- **core:** 按职责拆分 `src-tauri/src/lib.rs`：依次搬出 `update`、`tray`、`windows`、`monitor`、`ext_install`、`api_server`、`backup`、`dashboard`、`timeline`、`apps`、`settings`、`reports`、`live`、`commands` 十四个模块，`lib.rs` 从 3624 行降到 324 行；`fmt`、`ext`、`accent` 三块纯逻辑下沉到 `crates/tracker-core` 并补单元测试。
- **core:** 治理 WAL 日志膨胀：设 `wal_autocheckpoint=1000` 与 `journal_size_limit=1048576`，监控线程每 5 分钟 checkpoint 一次，退出与切换数据目录时也截断一次；此前实测出现过 3.7 MB 主库旁边挂着 7.3 MB 的 `usage.db-wal`。

## [0.6.4] - 2026-11-16

### Added

- **frontend:** 引入 vitest 单元测试（`npm test` / `npm run test:watch`），覆盖 `localDateKey`、对比视图四种区间预设、变化率计算与每日限额读写，共 29 个用例；新增 `src/lib/compare.ts`、`src/lib/limits.ts` 承接被测逻辑。
- **ci:** `frontend` job 在类型检查与构建之间新增 `npm test`。

### Changed

- **ci:** GitHub Actions 升级到 Node 24 运行时：`actions/checkout` 与 `actions/setup-node` 升到 `v5`、`actions/upload-artifact` 升到 `v6`、`actions/download-artifact` 升到 `v7`。

### Fixed

- **frontend:** 修复仪表盘右栏在分类较多时，「分类占比」的类目列表盖到下方「效率趋势」图上：右栏面板改为可收缩，列表与图例各自滚动。

## [0.6.3] - 2026-11-16

### Added

- **backup:** JSON 备份升到 v2，新增窗口标题与网页浏览区间；恢复后时间线的「Top 标题」与「Top 网页」不再整块为空。旧版 v1 备份仍可直接恢复。
- **settings:** 备份与恢复新增「打开备份目录」与「选择备份文件恢复」，并可在备份列表中点选任意一份恢复。
- **extension:** 「智能安装扩展」扩展到自动识别 17 款常见 Chromium 内核浏览器（含 Cent Browser / Brave / Vivaldi / Opera 等）。

### Changed

- **frontend:** 对比视图默认区间改为「今天 vs 昨天」（A = 昨天、B = 今天），工具栏新增同名快捷按钮，原「昨天 vs 前日」保留。
- **frontend:** 顶栏「悬浮窗」按钮移到导航最左侧，紧邻品牌区。

### Fixed

- **frontend:** 主界面与时间线加载失败时中间内容不再整块空白：加载中 / 加载失败（含错误信息与「重试」）/ 暂无数据三态分别可见，快速切换日期时丢弃过期响应。
- **app:** 修复开启悬浮窗后重启只见悬浮窗、主窗口不出现。
- **app:** 开机自启不再弹出主窗口：自启项改写为带引号的 `"exe路径" --autostart`，兼容含空格的安装路径。
- **frontend:** 修复日历点选某一天后仪表盘与时间线被永久钉在那一天：选中日期只消费一次。
- **widget:** 悬浮窗与主界面顶栏的「当前前台应用」不再显示已排除的应用。
- **extension:** 修复智能安装误把家目录下无关的 `extension/` 目录打包注册：改用编译期内置副本，打包前校验 manifest 名称，并清理旧版本遗留的错误注册表键。
- **category:** 修复右键「设置分类」与「保存分类规则」未实时生效：写后立即重建分类解析缓存并通知主界面重新拉数据。
- **backup:** 恢复备份改为单事务清空并写回五张表，耗时从数万条逐条隐式提交降到毫秒级，失败整体回滚。
- **extension:** 智能安装按 Chrome / Edge 优先顺序依次尝试，失败自动降级到下一款，全部失败时逐条返回原因；打包前清理残留私钥。
- **backup:** JSON 备份新增版本号校验与文件名路径逃逸校验，打开备份目录时转义含空格路径。

## [0.6.2] - 2026-11-11

### Added

- **frontend:** 主界面顶栏新增「悬浮窗」按钮，按钮高亮状态与悬浮窗实际可见性同步。
- **frontend:** 悬浮窗悬停时右上角显示 ✕ 关闭按钮，不必再经托盘或热键关闭。
- **settings:** 更新区块新增「更新时使用系统代理」开关（`update_use_proxy`），默认关闭即强制直连。
- **extension:** 新增「智能安装扩展」：自动探测本机 Chrome / Edge，用浏览器自带的 `--pack-extension` 打包 CRX3 并写入外部扩展注册表，成功后自动开启网页浏览统计。

### Changed

- **frontend:** 移除设置页「界面配色」区块与悬浮窗右键菜单的配色子菜单。
- **frontend:** 仪表盘四个汇总卡改为单行等分排布，窗口收窄后仍保持一排。

### Fixed

- **frontend:** 对比视图默认区间对调为「A = 前 7 天、B = 最近 7 天」，变化率按时间先后自动判断，不再出现负数百分比。
- **frontend:** 悬浮窗开启「贴边收起」但未贴边时不再静默收起，改为提示「请先拖到屏幕边缘再离开」。
- **frontend:** 修复悬浮窗贴边收起 / 展开动画中的蓝色闪烁与锁定徽标遮挡今日时长。

### Internal

- **chore:** 删除 `lib.rs` 中重复注册 `get_top_titles` 的一行，并移除已无入口的悬浮窗右键配色分支。

## [0.6.1] - 2026-11-08

### Added

- **frontend:** 新增浏览器扩展一键安装引导弹窗，可选择 Chrome 或 Edge 并自动复制扩展目录路径。

### Changed

- **settings:** 设置页布局改版，主题并入首屏「外观」区块，改用独立强调色选择器，选色后实时应用到主界面与悬浮窗。

### Fixed

- **frontend:** 修复日历年热力图的 GitHub 风格布局（方块改为响应式尺寸并补齐月份与星期标签），以及无数据日期不显示方块。

## [0.6.0] - 2026-11-07

### Added

- **frontend:** 新增日历年热力图，以 GitHub contribution 风格的小方块展示全年活跃分布，支持月 / 年切换。
- **frontend:** 设置页提供 6 种配色按钮，选色后立即应用到主界面与悬浮窗。
- **frontend:** 语言切换（中文 / English 分段按钮）提升到设置页首屏；新增浏览器扩展安装引导按钮。
- **settings:** 新增网页心跳间隔设置，默认 10 秒；受 MV3 定时器限制实际间隔不低于 30 秒。
- **updater:** 新增更新下载通道设置：`auto`（官方失败后回退镜像）、`official` 与 `mirror`（经 `gh-proxy.com` 加速）。

### Fixed

- **updater:** 修复镜像通道下载失败：下载 URL 改走 `gh-proxy.com` 代理，避开失效的系统代理。
- **frontend:** 修复日历年视图无法正确渲染的问题。

## [0.5.1] - 2026-11-07

### Changed

- **updater:** 发布验证版本，用于端到端验证从 `0.5.0` 升级到 `0.5.1` 的应用内自动更新流程；本版本无功能变化。

## [0.5.0] - 2026-11-07

### Added

- **extension:** 新增可选的 Manifest V3 浏览器扩展，定时把当前标签页的 URL 与标题上报到本机 API，按域名聚合网页浏览时长。
- **privacy:** 新增「网页浏览统计」隐私开关，默认关闭；关闭时浏览器心跳返回 403。
- **frontend:** 时间线页新增「Top 网页」面板。
- **api:** 本地 HTTP API 新增 `POST /api/web-heartbeat` 与 `GET /api/web-top` 两个端点。
- **settings:** 设置页新增数据库维护，可一键执行完整性检查与 VACUUM，并查看页数与空闲页统计。
- **i18n:** 补全静态 `title` / `placeholder` 文案翻译；英文模式时长改为 `1h23m` / `12m30s` 格式。

### Fixed

- **data:** 修复多个同名工具链项目共用数据目录导致的配置互相覆盖问题。

## [0.4.0] - 2026-11-07

### Added

- **updater:** 新增应用内自动更新，集成 `tauri-plugin-updater` 并用 minisign / ed25519 校验更新包签名；Release 流水线自动生成 `latest.json`。
- **settings:** 新增全局热键自定义，保存时校验组合键有效性与冲突。
- **privacy:** 新增「窗口标题记录」隐私开关（默认关闭）与标题排除规则，开启后时间线出现「Top 标题」。
- **frontend:** 新增效率趋势视图，按天绘制效率评分并支持时长 / 效率切换。
- **frontend:** 新增 Markdown 周报导出，输出到下载目录。
- **i18n:** 新增中英双语界面，主界面、托盘菜单、通知与悬浮窗跟随语言设置切换，缺译回退中文。
- **cli:** 新增 `--smoke N` 无界面冒烟模式，输出 `smoke-report.json`。

### Changed

- **frontend:** 合并汇总卡为单卡，展示总活跃、日均与环比。
- **frontend:** 分类树支持「开发/前端」层级命名，顶层改为汇总环形图加树形占比。
- **frontend:** 应用别名在排行、悬浮窗与时间线的读取层全局生效。
- **api:** 本地 HTTP API 提供 `live`、`dashboard`、`timeline`、`settings`、`version` 五个端点。

## [0.3.0] - 2026-11-06

### Breaking Changes

- **BREAKING:** 从 C#/WPF 实现全量重写为 Rust 核心 + Tauri 2 壳 + Svelte 5 前端，旧版数据文件与设置格式不再兼容。仓库未提供旧版数据的自动迁移，升级前请在旧版导出 JSON 备份，升级后重新核对分类规则、排除列表与数据目录。

### Added

- **backend:** 新增 `tracker-core` 聚合状态机，识别空闲、锁屏、睡眠与暂停状态，支持跨午夜区间切分。
- **frontend:** 新增仪表盘，支持今天 / 昨天 / 本周 / 本月 / 自定义区间，含汇总卡、每日目标、应用排行、分类占比与每日趋势。
- **frontend:** 新增 24 小时时间线色带，支持滚轮缩放、拖拽平移、明细表与搜索。
- **frontend:** 新增日历热力图、对比视图、分类树与应用详情下钻。
- **frontend:** 新增桌面悬浮窗（Mica 常显、DWM 圆角、缩略 / 贴边收起、置顶 / 锁定 / 透明度）与系统托盘。
- **settings:** 新增全局热键 `Ctrl+Alt+D` / `Ctrl+Alt+W`、休息提醒与分类限额系统通知。
- **api:** 新增本地 HTTP API，监听 `127.0.0.1:59598`。
- **data:** 新增 JSON 全量备份导出 / 恢复、数据保留策略、CSV 导出、开机自启与单实例。
- **frontend:** 新增深色 / 浅色主题，以及扫描桌面与开始菜单快捷方式补齐分类。

### Fixed

- **backend:** 修复程序自身被计入统计的问题：按路径与进程名双重排除，并自动清理历史行。
- **frontend:** 修复悬浮窗圆角白边（圆角交由 DWM 裁切）、失焦后玻璃效果消失（改为 Mica 常显）与置顶失效。
- **frontend:** 修复悬浮窗贴边收起的吸附判定、锚点漂移与展开抖动。

## [0.2.0] - 2026-11-01

> 旧 C#/WPF 版本的历史记录，未发布对应的 Git tag 与 GitHub Release，故无 compare / Release 链接。

### Added

- **frontend:** 新增交互式时间线（滚轮缩放、左键拖拽平移、刻度自适应）与分类下钻。
- **settings:** 新增分类每日限额与托盘气泡通知（当日去重），悬浮窗状态点随限额变色。
- **data:** 新增 JSON 全量导出 / 导入（`schemaVersion=1`），导入支持「合并」与「替换」两种模式。
- **frontend:** 新增悬浮窗逐小时迷你柱状图，排行榜实时加入新增应用。
- **settings:** 新增全局快捷键 `Ctrl+Alt+U` 打开主界面、`Ctrl+Alt+P` 暂停 / 恢复。
- **frontend:** 新增排行右键菜单，可按行设置分类覆盖，优先于规则匹配。

### Fixed

- **settings:** 修复设置窗口保存报错、底部按钮不可见与数字框文字裁切。
- **frontend:** 统一悬浮窗与仪表盘的计时口径，并修正默认贴左上角与拖动越界。
- **frontend:** 修复排行占比显示为 4266% 的缺陷（基数未加载）与日期步进的单日死按钮。
- **frontend:** 启动后只打开悬浮窗（widget-first），主界面按需打开。

## [0.1.0] - 2026-10-31

> 旧 C#/WPF 版本的首版历史记录，未发布对应的 Git tag 与 GitHub Release，故无 compare / Release 链接。

### Added

- **backend:** 首次实现前台活跃时间统计，含 1 秒轮询、空闲检测、锁屏 / 睡眠感知与跨午夜切分。
- **backend:** 首次实现分类规则引擎，支持关键词匹配、覆盖表与未分类。
- **data:** 首次实现 SQLite WAL 持久化，含前台明细、每日聚合与保留策略。
- **frontend:** 首次实现 WPF 仪表盘（排行、环形图、时间线、趋势、CSV 导出）、系统托盘与悬浮窗。
- **settings:** 首次实现设置窗口、开机自启与久坐提醒。
- **packaging:** 以 MIT 许可证开源发布首个版本。

[Unreleased]: https://github.com/StarAsh042/AppUsageTracker/compare/v0.6.6...HEAD
[0.6.6]: https://github.com/StarAsh042/AppUsageTracker/compare/v0.6.5...v0.6.6
[0.6.5]: https://github.com/StarAsh042/AppUsageTracker/compare/v0.6.4...v0.6.5
[0.6.4]: https://github.com/StarAsh042/AppUsageTracker/compare/v0.6.3...v0.6.4
[0.6.3]: https://github.com/StarAsh042/AppUsageTracker/compare/v0.6.2...v0.6.3
[0.6.2]: https://github.com/StarAsh042/AppUsageTracker/compare/v0.6.1...v0.6.2
[0.6.1]: https://github.com/StarAsh042/AppUsageTracker/compare/v0.6.0...v0.6.1
[0.6.0]: https://github.com/StarAsh042/AppUsageTracker/compare/v0.5.1...v0.6.0
[0.5.1]: https://github.com/StarAsh042/AppUsageTracker/compare/v0.5.0...v0.5.1
[0.5.0]: https://github.com/StarAsh042/AppUsageTracker/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/StarAsh042/AppUsageTracker/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/StarAsh042/AppUsageTracker/releases/tag/v0.3.0
