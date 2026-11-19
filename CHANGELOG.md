# Changelog

本项目所有值得注意的用户可见变更都记录在此文件中。

格式依据 [Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/)，版本遵循 [Semantic Versioning 2.0.0](https://semver.org/spec/v2.0.0.html)。版本日期为发布当日日期（本地时区 Asia/Shanghai）；[GitHub Releases](https://github.com/StarAsh042/AppUsageTracker/releases) 的时间戳为 UTC，跨日显示可能相差一天。各版本标签与安装包见 [GitHub Releases](https://github.com/StarAsh042/AppUsageTracker/releases)。

本仓库为单人维护，发布提交多为直接提交或 squash 合并，历史条目没有可核实的 PR 编号，因此条目未附 PR / Issue 链接，也不编造链接。已提交但尚未发布的变更先追加到 `## [Unreleased]`；发布时由维护者移入新版本标题、同步 manifest 版本号并打标签。维护约定见 [README 的贡献章节](README.md#贡献)。

## [Unreleased]

### Added

- **settings:** 设置页新增「数据库维护」面板：一键做完整性检查 + VACUUM，并把 WAL 日志截断，结果里显示完整性检查结论、压缩是否成功、主库体积、页数与空闲页数。

### Changed

- **frontend:** 弹窗外壳抽成共用组件 `src/lib/Modal.svelte`（遮罩点击关闭、Esc 关闭、打开即把焦点移进弹窗、内容超高时自己滚动），仪表盘的应用详情与设置页的扩展安装两处共用一套实现。设置页的扩展安装弹窗此前没有配套样式，是裸元素直接铺在页面上，现在与仪表盘弹窗外观一致。
- **frontend:** 仪表盘应用排行行由 `div` 改为 `button`，键盘可以 Tab 聚焦、回车打开详情，聚焦时有可见描边；行内文字排版与右键菜单行为不变。

### Fixed

- **a11y:** 把 `npm run check` 的 svelte-check 告警从 13 条清到 0 条：两处弹窗支持 Esc 关闭（此前按 Esc 无反应，只能点遮罩或关闭按钮）；`TimelineView` 不再在初始化时直接引用 prop（`state_referenced_locally`），改为挂载后同步一次；悬浮窗根节点补 `role="application"` 与 `aria-label` 说明它是整块拖拽表面；删除未使用的 `.card .value.small` 选择器。
- **app:** 修复启动瞬间先闪一下白底/灰底、随后才变暗的问题：主窗口在透明模式下启用 `noRedirectionBitmap`（`WS_EX_NOREDIRECTIONBITMAP`）并把 `backgroundColor` 设为全透明，WebView2 首帧渲染前不再露出白色默认底。
- **app:** 修复主界面点「悬浮窗」按钮后整个界面卡死、图表不再更新的问题。悬浮窗是首次开启时按需创建的，创建代码跑在同步 IPC 命令里，而 Windows 上建 WebView 窗口需要等主线程事件循环（tauri 文档明确写了在同步命令 / 事件处理器里调用会死锁，见 wry#583），于是主线程自己等自己、所有 IPC 永不返回。现在把建窗挪到独立线程（并加锁串行化，避免连点起两个线程重复建窗），主线程立刻返回继续转事件循环，窗口建好后再挂贴边约束。

### Internal

- **core:** 窗口定义收敛：悬浮窗（label `widget`）改为在 `src-tauri/tauri.conf.json` 的 `app.windows` 里声明（标题、300×216 初始尺寸、位置 12,12、无边框、透明、不可缩放、无阴影、不占任务栏、置顶），并置 `"create": false` 让它在启动时不自动建窗；`src-tauri/src/windows.rs` 的 `create_widget_window` 改为按 label 从 `app.config().app.windows` 取这份配置、用 `WebviewWindowBuilder::from_config` 建窗，只保留配置表达不了的两项——随偏好变化的初始尺寸（`inner_size`，紧凑模式 200×76）与 `always_on_top`——用 builder 覆写。建窗仍在独立线程里（建窗死锁的修法不变），DWM 背板、圆角、重新置顶与 `on_menu_event` 顺序不变。
- **core:** 收紧 CSP：`src-tauri/tauri.conf.json` 的 `app.security.csp` 从 `null` 改为 `default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src ipc: http://ipc.localhost; font-src 'self'; object-src 'none'; base-uri 'self'`。前端不引用任何外链资源（无图片/字体/CDN，构建产物只有 3 个 JS + 3 个 CSS 与各自一个 `<script type="module">`），所以脚本侧不需要 `'unsafe-inline'`；`style-src` 保留 `'unsafe-inline'` 是因为 Svelte 会输出内联 `style` 属性；`connect-src` 放行 `ipc:` 与 `http://ipc.localhost` 供 Tauri IPC 使用。
- **core:** 去掉 `tauri` 的 `unstable` feature：全仓库唯一的不稳定 API 是 `show_widget_menu` 里的 `AppHandle::get_window`，改用稳定的 `get_webview_window` + `WebviewWindow::popup_menu`，`src-tauri/Cargo.toml` 的 `features` 只剩 `["tray-icon"]`，行为不变。
- **core:** 拆分 `src-tauri/src/lib.rs` 的第十六步：四条通用 IPC 命令（`clear_all_data`、`export_csv`、`open_path`、`get_app_version`）搬到 `src-tauri/src/commands.rs`（80 行），`backup.rs` 改为从 `crate::commands` 导入 `open_path`，行为不变，`lib.rs` 从 397 行降到 324 行。
- **core:** 拆分 `src-tauri/src/lib.rs` 的第十五步：实时状态搬到 `src-tauri/src/live.rs`（109 行，含 `LiveTopApp`/`LiveStatus` 两个结构与 `live_status`、`get_live_status` 命令），`api_server.rs` 与 `monitor.rs` 改为从 `crate::live` 导入，行为不变，`lib.rs` 从 494 行降到 397 行。
- **core:** 拆分 `src-tauri/src/lib.rs` 的第十四步：本地工具函数 `fmt_hms`/`csv_escape` 从 `lib.rs` 的「小工具」段搬进 `crates/tracker-core/src/fmt.rs`（51 行），补两个单测（时长按量级切换单位、CSV 只在必要时加引号），`cargo test` 从 25 例增加到 27 例；`lib.rs`、`monitor.rs`、`reports.rs` 改为从 `tracker_core::fmt` 导入，行为不变，`lib.rs` 从 517 行降到 494 行。
- **core:** 拆分 `src-tauri/src/lib.rs` 的第十三步：报表与维护相关的代码搬到 `src-tauri/src/reports.rs`（184 行，含 `DbMaintenance` 结构、`db_maintenance` 命令、`export_weekly_report` 周报导出、`TopTitle`/`TopWeb` 两个排行榜结构与 `get_top_titles`/`get_top_web`），行为不变，`lib.rs` 从 699 行降到 517 行。
- **core:** 拆分 `src-tauri/src/lib.rs` 的第十二步：悬浮窗相关的五个 IPC 命令（`show_widget_menu`、`toggle_widget`、`close_widget`、`set_widget_collapsed`、`update_widget_settings`）搬进 `src-tauri/src/windows.rs`，与它们操作的窗口代码放在一起，行为不变，`lib.rs` 从 804 行降到 699 行。
- **core:** 拆分 `src-tauri/src/lib.rs` 的第十一步：设置链路整体搬到 `src-tauri/src/settings.rs`（341 行，含 `display_alias`、`resolve_locale`/`apply_locale`、`db_path`、`load_settings`/`migrate_settings`、`save_settings_file`、`set_widget_visible`、`set_paused_internal`，以及 `get_settings`、`save_settings`、`set_paused`、`set_app_display_name`、`exclude_app`、`set_app_category`、`save_category_rules`、`list_available_categories` 八个 IPC 命令），`apply_hotkeys` 提为 `pub(crate)` 供其调用，其余模块改从 `crate::settings` 导入，行为不变，`lib.rs` 从 1114 行降到 804 行。
- **core:** 拆分 `src-tauri/src/lib.rs` 的第十步：应用详情与应用扫描整体搬到 `src-tauri/src/apps.rs`（217 行，含 `AppDetail`/`ScannedApp`、`get_app_detail`、`scan_installed_apps` 与 `collect_lnks` 递归遍历桌面 / 开始菜单 `.lnk`），行为不变，`lib.rs` 从 1320 行降到 1114 行。
- **core:** 拆分 `src-tauri/src/lib.rs` 的第九步：时间线数据整体搬到 `src-tauri/src/timeline.rs`（81 行，含 `TimelineSegment`/`TimelineState`/`TimelineData`、`get_timeline` 命令与 `timeline_data`），`api_server.rs` 改为从新模块导入，行为不变，`lib.rs` 从 1395 行降到 1320 行。
- **core:** 拆分 `src-tauri/src/lib.rs` 的第八步：仪表盘 / 周报的数据聚合搬到 `src-tauri/src/dashboard.rs`（210 行，含 `DashboardApp`/`DashboardCategory`/`DashboardTrendPoint`/`DashboardData` 四个结构、`get_dashboard` 命令与 `build_dashboard`），`api_server.rs` 改为从新模块导入，行为不变，`lib.rs` 从 1588 行降到 1395 行。
- **core:** 拆分 `src-tauri/src/lib.rs` 的第七步：JSON 备份与恢复整体搬到 `src-tauri/src/backup.rs`（229 行，含 `BackupFile` v2 结构、导出、按名/按内容恢复、删除与列表），行为不变，`lib.rs` 从 1810 行降到 1588 行。
- **core:** 拆分 `src-tauri/src/lib.rs` 的第六步：本地 HTTP API 整体搬到 `src-tauri/src/api_server.rs`（308 行，含 `spawn_api_server`、`handle_api` 路由、`extract_domain`、浏览器扩展心跳 `heartbeat_web`），实时状态 `live_status` 留在 `lib.rs`（API 与 IPC 共用），行为不变，`lib.rs` 从 2107 行降到 1810 行。
- **core:** 拆分 `src-tauri/src/lib.rs` 的第五步：浏览器扩展安装整体搬到 `src-tauri/src/ext_install.rs`（583 行，含扩展落盘、CRX3 打包、注册表注册与 17 款 Chromium 浏览器的智能安装），行为不变，`lib.rs` 从 2682 行降到 2107 行。
- **core:** 拆分 `src-tauri/src/lib.rs` 的第四步：监控线程相关逻辑（自身进程名识别、前台窗口识别与进程名缓存、轮询主循环、落库 `flush_state`、保留策略清理、分类限额提醒）整体搬到 `src-tauri/src/monitor.rs`，行为不变，`lib.rs` 从 2987 行降到 2682 行。
- **core:** 拆分 `src-tauri/src/lib.rs` 的第三步：窗口相关逻辑（主窗显示/隐藏与热键切换、悬浮窗创建与 DWM 背板/圆角、贴边收起与展开、smoothstep 尺寸动画、悬浮窗右键菜单）连同三种窗口尺寸常量整体搬到 `src-tauri/src/windows.rs`，行为不变，`lib.rs` 从 3465 行降到 2987 行。模块与 Win32 绑定 crate 同名，模块内引用 Win32 时写全路径 `::windows::...`。
- **core:** 拆分 `src-tauri/src/lib.rs` 的第二步：系统托盘（图标、右键菜单五项、左键唤起主界面、退出前 flush + checkpoint）（113 行）整体搬到 `src-tauri/src/tray.rs`，菜单项句柄 `TrayHandles` 一并迁出，`AppState.tray` 改为持有 `tray::TrayHandles`，行为不变，`lib.rs` 从 3565 行降到 3465 行。
- **core:** 拆分 `src-tauri/src/lib.rs` 的第一步：应用内更新（下载通道解析、`check_update` / `install_update` 两个命令）整体搬到 `src-tauri/src/update.rs`，端点选择与代理/镜像回退行为不变，`lib.rs` 从 3624 行降到 3565 行。
- **core:** 治理 WAL 日志膨胀：建库时显式设 `wal_autocheckpoint=1000` 与 `journal_size_limit=1048576`，监控线程每 5 分钟做一次 `wal_checkpoint(TRUNCATE)`，退出、切换数据目录与冒烟模式下也会截断一次。此前全仓库没有一处 checkpoint 调用，`journal_size_limit` 是 SQLite 默认的 -1，自动 checkpoint 只把 WAL 置零复用、不缩文件，实测出现过 3.7 MB 的主库旁挂着 7.3 MB 的 `usage.db-wal`。
- **core:** 把 `src-tauri/src/lib.rs` 里的纯逻辑抽到 `crates/tracker-core`：新增 `ext.rs`（protobuf varint 读取、CRX3 头部解析取扩展 ID、公钥哈希到扩展 ID 的 a-p 编码、读 `manifest.json` 版本号）与 `accent.rs`（悬浮窗亚克力着色的混色计算），补 10 个单元测试，`cargo test --all --locked` 从 12 例增加到 22 例。`lib.rs` 只保留同名薄封装，调用点与运行时行为不变。

## [0.6.4] - 2026-11-16

### Added

- **frontend:** 引入 vitest 单元测试框架（`npm test` / `npm run test:watch`），覆盖 `localDateKey`、对比视图四种区间预设、变化率计算与每日限额读写，共 29 个用例；新增 `src/lib/compare.ts`、`src/lib/limits.ts` 两个纯逻辑模块承接被测逻辑，组件行为不变。
- **ci:** `frontend` job 在类型检查与构建之间新增 `npm test`。

### Changed

- **ci:** GitHub Actions 升级到 Node 24 运行时：`actions/checkout` 与 `actions/setup-node` 由 `v4` 升到 `v5`，`actions/upload-artifact` 由 `v4` 升到 `v6`，`actions/download-artifact` 由 `v4` 升到 `v7`，消除 Node 20 弃用告警。

### Fixed

- **frontend:** 修复仪表盘右栏在「本月」等分类较多的区间下，「分类占比」的类目列表盖到下方「效率趋势」图上的问题：右栏面板改为可收缩，类目列表与图例各自滚动，趋势图保留基准高度不再被压扁。

## [0.6.3] - 2026-11-16

### Added

- **backup:** JSON 备份升到 v2，新增窗口标题与网页浏览区间；恢复时一并写回，时间线的「Top 标题」与「Top 网页」不再整块为空。旧版 v1 备份仍可直接恢复。
- **settings:** 备份与恢复（JSON）新增「打开备份目录」、「选择备份文件恢复」，并可在备份列表中点选任意一份恢复。
- **extension:** 「智能安装扩展」扩展为自动识别 17 款常见 Chromium 内核浏览器（含 Cent Browser / Brave / Vivaldi / Opera 等），用共享密钥一次打包、按各浏览器注册表根逐个注册。

### Changed

- **frontend:** 对比视图默认区间改为「今天 vs 昨天」（A = 昨天、B = 今天，变化率仍按时间先后计算），工具栏新增「今天 vs 昨天」快捷按钮，原「昨天 vs 前日」保留。
- **frontend:** 顶栏「悬浮窗」按钮移到导航最左侧，紧邻品牌区。

### Fixed

- **frontend:** 修复主界面与时间线在加载失败时中间内容整块空白的问题：加载中 / 加载失败（含错误信息与「重试」）/ 暂无数据三态分别可见，快速切换日期时丢弃过期响应。
- **app:** 修复开启悬浮窗后重启只见悬浮窗、主窗口不出现：启动时显式显示主窗口（`--smoke` 无界面模式除外），同时覆盖开机自启与从托盘隐藏后重启的场景。
- **app:** 开机自启不再弹出主窗口：自启项改写成带引号的 `"exe路径" --autostart`（兼容含空格安装路径），带该标记启动时只常驻托盘与悬浮窗；老版本写入的裸路径会在启动时按当前设置就地规范化。
- **frontend:** 修复日历点选某一天后仪表盘与时间线被永久钉在那一天、回不到今天：选中日期只消费一次。
- **widget:** 悬浮窗与主界面顶栏的「当前前台应用」不再显示已排除的应用。
- **extension:** 修复智能安装误把家目录下无关的 `extension/` 目录（如「Codex认证助手」）打包注册的问题：扩展内容改为编译期内置副本，打包前校验 manifest 名称，并在安装时清理旧版本遗留的错误注册表键与按浏览器分目录的旧 CRX。
- **category:** 修复右键「设置分类」与「保存分类规则」未实时生效：抽出 `rebuild_resolver` 供三处共用，写后立即重建分类解析缓存并发 `app://settings`，主界面据此重挂载数据视图重新拉数据。
- **backup:** 恢复数据后同时清空内存聚合态、网页浏览态与限额通知态，并发 `app://data` 通知所有视图重新拉数据。
- **backup:** 恢复备份改为单事务清空并写回五张表：此前上万条明细逐条隐式提交、长时间占用数据库锁，恢复时界面可能整块卡死；现在耗时降到毫秒级，失败整体回滚不留半套数据。
- **extension:** 智能安装按 Chrome / Edge 优先顺序依次尝试打包，任一浏览器失败自动降级到下一款；全部失败或 CRX 无效时按浏览器逐条返回原因，不再整批中止。
- **extension:** 打包前清理残留私钥，避免重试时报「指定扩展的私钥已存在」。
- **backup:** JSON 备份恢复兼容缺失字段并校验支持的版本号；打开备份目录时转义含空格路径；备份文件名增加 .json 与路径逃逸校验，失败时给出可读提示。

## [0.6.2] - 2026-11-11

### Added

- **frontend:** 主界面顶栏新增「悬浮窗」按钮，直接开关桌面悬浮窗，按钮高亮状态与悬浮窗实际可见性同步。
- **frontend:** 悬浮窗悬停时右上角显示 ✕ 关闭按钮，不必再经托盘或热键关闭。
- **settings:** 更新区块新增「更新时使用系统代理」开关（`update_use_proxy`），默认关闭即强制直连、跳过系统代理。
- **extension:** 新增「智能安装扩展」：自动探测本机 Chrome / Edge，用浏览器自带的 `--pack-extension` 打包 CRX3 并写入 HKCU 外部扩展注册表项，成功后自动开启网页浏览统计；未检测到浏览器或打包失败时逐条给出原因并回退到手动安装。

### Changed

- **frontend:** 移除设置页「界面配色」区块，同时移除悬浮窗右键菜单的配色子菜单。
- **frontend:** 仪表盘四个汇总卡改为单行等分排布，窗口收窄后仍保持一排。

### Fixed

- **frontend:** 对比视图默认区间对调为「A = 前 7 天、B = 最近 7 天」，变化率按时间先后自动判断，不再出现负数百分比。
- **frontend:** 悬浮窗开启「贴边收起」但未贴边时不再静默收起，改为提示「请先拖到屏幕边缘再离开」。
- **frontend:** 修复悬浮窗贴边收起 / 展开动画中的蓝色闪烁：边缘指示条改为细线并去掉发光阴影，边缘事件改在动画结束后发出。
- **frontend:** 修复悬浮窗锁定徽标遮挡今日时长的问题，改为行内显示。

### Internal

- **chore:** 删除 `src-tauri/src/lib.rs` 中重复注册 `get_top_titles` 的一行，并移除已无入口的悬浮窗右键配色分支（`w.color.*`）与相关注释；`accent` 字段保留，仅用于 Windows 亚克力着色。
- **docs:** 新增 `docs/ROADMAP.md` 记录后续开发计划与优先级。

## [0.6.1] - 2026-11-08

### Added

- **frontend:** 新增浏览器扩展一键安装引导弹窗，可选择 Chrome 或 Edge 并自动复制扩展目录路径，替代原来的简单按钮。
- **docs:** README 补充真实界面截图（主窗口仪表盘 / 桌面悬浮窗），并修正过时的分支状态说明。

### Changed

- **settings:** 变更设置页布局，将主题设置并入首屏「外观」区块，改用独立强调色选择器，选色后实时应用到主界面与悬浮窗。

### Fixed

- **frontend:** 修复日历年热力图的 GitHub 风格布局，方块改为响应式尺寸并补齐月份与星期标签。
- **frontend:** 修复日历年热力图在无数据日期不显示方块的问题。

## [0.6.0] - 2026-11-07

### Added

- **frontend:** 新增日历年热力图，以 GitHub contribution 风格的小方块展示全年活跃分布，支持月 / 年切换。
- **frontend:** 新增界面配色实时切换，设置页提供 6 种配色按钮，选色后立即应用到主界面与悬浮窗。
- **frontend:** 将语言切换（中文 / English 分段按钮）提升到设置页首屏。
- **frontend:** 新增浏览器扩展安装引导按钮，可一键打开扩展页与扩展目录。
- **frontend:** 将备份文件的删除按钮移至文字前，减少误触。
- **settings:** 新增网页心跳间隔设置，默认 10 秒；受浏览器扩展 Manifest V3 定时器限制，实际间隔不低于 30 秒。
- **updater:** 新增更新下载通道设置：`auto`（官方地址失败后回退镜像）、`official` 与 `mirror`（经 `gh-proxy.com` 加速）。

### Fixed

- **updater:** 修复镜像通道下载失败的问题，下载 URL 改走 `gh-proxy.com` 代理以避开失效的系统代理。
- **frontend:** 修复日历年视图无法正确渲染的问题。

## [0.5.1] - 2026-11-07

### Changed

- **updater:** 发布验证版本，用于端到端验证从 `0.5.0` 升级到 `0.5.1` 的应用内自动更新流程；本版本无功能变化，使用者无需执行额外操作。

## [0.5.0] - 2026-11-07

### Added

- **extension:** 新增可选的 Chrome / Edge Manifest V3 浏览器扩展，定时把当前标签页的 URL 与标题上报到本机 API，按域名聚合网页浏览时长。
- **privacy:** 新增「网页浏览统计」隐私开关，默认关闭；关闭时浏览器心跳返回 403。
- **frontend:** 时间线页新增「Top 网页」面板，按域名与标题聚合浏览时长。
- **api:** 本地 HTTP API 新增 `POST /api/web-heartbeat` 与 `GET /api/web-top` 两个端点。
- **settings:** 设置页新增数据库维护，可一键执行完整性检查与 VACUUM，并查看页数与空闲页统计。
- **i18n:** 补全静态 `title` / `placeholder` 文案的翻译；英文模式时长改为 `1h23m` / `12m30s` 格式。

### Fixed

- **data:** 修复多个同名工具链项目共用数据目录导致的配置互相覆盖问题。

## [0.4.0] - 2026-11-07

### Added

- **updater:** 新增应用内自动更新，集成 `tauri-plugin-updater` 并用 ed25519 / minisign 签名校验更新包，设置页提供检查更新与一键更新；Release 流水线自动生成 `latest.json`。
- **settings:** 新增全局热键自定义，保存时校验组合键有效性与冲突。
- **privacy:** 新增「窗口标题记录」隐私开关（默认关闭）与标题排除规则；开启后时间线页出现「Top 标题」。
- **frontend:** 新增效率趋势视图，按天绘制效率评分，可切换时长 / 效率，评分分三档配色。
- **frontend:** 新增 Markdown 周报导出，包含汇总、分类、Top 应用与每日明细，输出到下载目录。
- **i18n:** 新增中英双语界面，主界面、托盘菜单、通知与悬浮窗跟随语言设置切换，缺译回退中文。
- **cli:** 新增 `--smoke N` 无界面冒烟模式，输出 `smoke-report.json`，以退出码 0 / 3 表示结果。

### Changed

- **frontend:** 合并汇总卡为单卡展示总活跃、日均与环比，稳定单日 / 多日视图布局。
- **frontend:** 分类树支持「开发/前端」层级命名，顶层改为汇总环形图加树形占比。
- **frontend:** 应用改名（别名）在详情弹窗设置后，于排行、悬浮窗与时间线的读取层全局生效。
- **api:** 本地 HTTP API 提供 `live`、`dashboard`、`timeline`、`settings`、`version` 五个端点。

## [0.3.0] - 2026-11-06

### Breaking Changes

- **BREAKING:** 从 C#/WPF 实现全量重写为 Rust 核心 + Tauri 2 壳 + Svelte 5 前端，独立 exe 约 11 MB，旧版数据文件与设置格式不再兼容。迁移：仓库未提供旧版数据到本版本的自动迁移说明，升级前请在旧版导出 JSON 备份，升级后在「设置」中重新核对分类规则、排除列表与数据目录。

### Added

- **backend:** 新增 `tracker-core` 聚合状态机，识别空闲、锁屏、睡眠与暂停状态，并支持跨午夜区间切分。
- **frontend:** 新增仪表盘，支持今天 / 昨天 / 本周 / 本月 / 自定义区间，含汇总卡、每日目标进度、应用排行、分类占比与每日趋势。
- **frontend:** 新增 24 小时时间线色带，支持滚轮缩放、拖拽平移、明细表与搜索，并标注统计中 / 空闲 / 锁定 / 挂起状态区间。
- **frontend:** 新增 GitHub contribution 风格日历热力图，点击日期可跳转。
- **frontend:** 新增对比视图，支持双时段应用 / 分类并排对比、A/B 交换、快捷预设与变化百分比。
- **frontend:** 新增分类树，支持「开发/前端」层级名、顶层汇总环形图与树形占比。
- **frontend:** 新增应用详情下钻与应用改名别名，在读取层全局生效。
- **frontend:** 新增桌面悬浮窗，Win11 Mica 常显磨砂与 DWM 圆角，支持缩略 / 贴边收起、置顶 / 锁定 / 透明度 / 配色右键菜单，双击开关主界面。
- **frontend:** 新增系统托盘，含实时 tooltip、左键打开主界面与右键菜单（悬浮窗 / 暂停 / 设置 / 退出）。
- **settings:** 新增全局热键 `Ctrl+Alt+D` / `Ctrl+Alt+W`、休息提醒与分类限额系统通知。
- **api:** 新增本地 HTTP API，监听 `127.0.0.1:59598`，提供 `live`、`dashboard`、`timeline`、`settings`、`version` 端点。
- **data:** 新增 JSON 全量备份导出 / 恢复、数据保留策略、CSV 导出、开机自启与单实例。
- **frontend:** 新增深色 / 浅色主题，以及扫描桌面与开始菜单快捷方式补齐分类。

### Fixed

- **backend:** 修复程序自身被计入统计的问题，按路径与进程名双重排除，并自动清理历史行。
- **frontend:** 修复悬浮窗圆角白边（圆角交由 DWM 裁切）、失焦后玻璃效果消失（改为 Mica 常显）与置顶失效（背板重建后重设 topmost）。
- **frontend:** 修复悬浮窗贴边收起的吸附判定、锚点漂移与展开抖动，改为原地展开、去抖防震荡并补齐四边细线动画。

## [0.2.0] - 2026-11-01

> 旧 C#/WPF 版本的历史记录，未发布对应的 Git tag 与 GitHub Release，故无 compare / Release 链接。

### Added

- **frontend:** 新增交互式时间线，支持滚轮缩放（光标锚定）、左键拖拽平移与刻度自适应。
- **frontend:** 新增分类下钻，点击环形图扇区或图例查看该分类的应用构成，点击图例返回。
- **settings:** 新增分类每日限额与托盘气泡通知（当日去重），悬浮窗状态点随限额变色。
- **data:** 新增 JSON 全量导出 / 导入（`schemaVersion=1`），导入支持「合并（跳过已存在）」与「替换」（需确认）两种模式。
- **frontend:** 新增悬浮窗逐小时迷你柱状图，实时增长。
- **settings:** 新增全局快捷键 `Ctrl+Alt+U` 打开主界面、`Ctrl+Alt+P` 暂停 / 恢复。
- **frontend:** 新增排行右键菜单，可按行设置分类覆盖，优先于规则匹配。
- **frontend:** 悬浮窗改用真 Acrylic 磨砂效果并加入屏幕边界钳制。
- **frontend:** 排行榜自动加入实时新增的应用，秒表每秒直读数据库。

### Fixed

- **settings:** 修复设置窗口保存报错、底部按钮不可见与数字框文字裁切的问题。
- **frontend:** 统一悬浮窗与仪表盘的计时口径（每秒直读数据库），并修正默认贴左上角与拖动越界的问题。
- **frontend:** 修复排行占比显示为 4266% 的缺陷（基数未加载）与日期步进的单日死按钮。
- **settings:** 扩充分类规则以覆盖本机全部软件（扫描桌面 / 开始菜单并补充互联网常用软件）。
- **frontend:** 启动后只打开悬浮窗（widget-first），主界面按需打开。

## [0.1.0] - 2026-10-31

> 旧 C#/WPF 版本的首版历史记录，未发布对应的 Git tag 与 GitHub Release，故无 compare / Release 链接。

### Added

- **backend:** 首次实现前台活跃时间统计，含 1 秒轮询、空闲检测、锁屏 / 睡眠感知与跨午夜切分。
- **backend:** 首次实现分类规则引擎，支持关键词匹配、覆盖表与未分类。
- **data:** 首次实现 SQLite WAL 持久化，含前台明细、每日聚合与保留策略。
- **frontend:** 首次实现 WPF 仪表盘（排行、环形图、时间线、趋势、CSV 导出）、系统托盘与悬浮窗。
- **settings:** 首次实现设置窗口（阈值、轮询、排除、规则、限额、自启、主题）、开机自启与久坐提醒。
- **packaging:** 以 MIT 许可证开源发布首个版本。

[Unreleased]: https://github.com/StarAsh042/AppUsageTracker/compare/v0.6.4...HEAD
[0.6.4]: https://github.com/StarAsh042/AppUsageTracker/compare/v0.6.3...v0.6.4
[0.6.3]: https://github.com/StarAsh042/AppUsageTracker/compare/v0.6.2...v0.6.3
[0.6.2]: https://github.com/StarAsh042/AppUsageTracker/compare/v0.6.1...v0.6.2
[0.6.1]: https://github.com/StarAsh042/AppUsageTracker/compare/v0.6.0...v0.6.1
[0.6.0]: https://github.com/StarAsh042/AppUsageTracker/compare/v0.5.1...v0.6.0
[0.5.1]: https://github.com/StarAsh042/AppUsageTracker/compare/v0.5.0...v0.5.1
[0.5.0]: https://github.com/StarAsh042/AppUsageTracker/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/StarAsh042/AppUsageTracker/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/StarAsh042/AppUsageTracker/releases/tag/v0.3.0
