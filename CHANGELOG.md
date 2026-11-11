# Changelog

本项目所有值得注意的用户可见变更都记录在此文件中。

格式依据 [Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/)，版本遵循 [Semantic Versioning 2.0.0](https://semver.org/spec/v2.0.0.html)。版本日期为发布当日日期（本地时区 Asia/Shanghai）；[GitHub Releases](https://github.com/StarAsh042/AppUsageTracker/releases) 的时间戳为 UTC，跨日显示可能相差一天。各版本标签与安装包见 [GitHub Releases](https://github.com/StarAsh042/AppUsageTracker/releases)。

本仓库为单人维护，发布提交多为直接提交或 squash 合并，历史条目没有可核实的 PR 编号，因此条目未附 PR / Issue 链接，也不编造链接。已提交但尚未发布的变更先追加到 `## [Unreleased]`；发布时由维护者移入新版本标题、同步 manifest 版本号并打标签。维护约定见 [README 的贡献章节](README.md#贡献)。

## [Unreleased]

### Added

- **settings:** 备份与恢复（JSON）新增「打开备份目录」、「选择备份文件恢复」，并可在备份列表中点选任意一份恢复。
- **extension:** 「智能安装扩展」扩展为自动识别 17 款常见 Chromium 内核浏览器（含 Cent Browser / Brave / Vivaldi / Opera 等），用共享密钥一次打包、按各浏览器注册表根逐个注册。

### Fixed

- **extension:** 修复智能安装误把家目录下无关的 `extension/` 目录（如「Codex认证助手」）打包注册的问题：扩展内容改为编译期内置副本，打包前校验 manifest 名称，并在安装时清理旧版本遗留的错误注册表键与按浏览器分目录的旧 CRX。
- **category:** 修复右键「设置分类」与「保存分类规则」未实时生效：抽出 `rebuild_resolver` 供三处共用，写后立即重建分类解析缓存并发 `app://settings`，主界面据此重挂载数据视图重新拉数据。
- **backup:** 恢复数据后同时清空内存聚合态、网页浏览态与限额通知态，并发 `app://data` 通知所有视图重新拉数据。
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

[Unreleased]: https://github.com/StarAsh042/AppUsageTracker/compare/v0.6.1...HEAD
[0.6.1]: https://github.com/StarAsh042/AppUsageTracker/compare/v0.6.0...v0.6.1
[0.6.0]: https://github.com/StarAsh042/AppUsageTracker/compare/v0.5.1...v0.6.0
[0.5.1]: https://github.com/StarAsh042/AppUsageTracker/compare/v0.5.0...v0.5.1
[0.5.0]: https://github.com/StarAsh042/AppUsageTracker/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/StarAsh042/AppUsageTracker/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/StarAsh042/AppUsageTracker/releases/tag/v0.3.0
