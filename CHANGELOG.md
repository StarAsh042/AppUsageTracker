# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.6.0] - 2026-11-07

### Added
- 备份文件删除按钮（左移至文字前）
- 日历年热力图（GitHub contribution 风格小方块，月/年切换）
- 界面配色实时切换（6 色按钮，设置页选色即改全界面+悬浮窗）
- 更新下载通道（自动=官方→镜像回退 / 仅官方 / 仅镜像，gh-proxy 加速）
- 浏览器扩展安装引导按钮（打开扩展页 + 打开扩展目录）
- 网页心跳间隔可调（默认 10s，MV3 下限 30s）
- 语言切换提升到设置页首屏（中文/English 分段按钮）

### Fixed
- 更新镜像失败：下载 URL 走 gh-proxy 代理，绕过失效系统代理
- 日历年视图正确渲染
## [0.5.1] - 2026-11-07

### Changed
- 更新链路端到端验证版（无功能变化；用于验证 0.5.0 → 0.5.1 应用内自动更新全流程）

## [0.5.0] - 2026-11-07

### Added
- 浏览器扩展（Chrome/Edge MV3）：每 60s 心跳当前标签页 URL+标题到本地 API，按域名聚合网页浏览时长；隐私开关 record_web 默认关（关闭时心跳 403 拒绝）
- 时间线页新增「Top 网页」面板（按域名+标题聚合）；本地 API 新增 /api/web-heartbeat（POST）与 /api/web-top
- 数据库维护：设置页一键完整性检查 + VACUUM + 页数/空闲页统计（db_maintenance 命令）
- i18n 收尾：全部静态 title/placeholder 属性接入翻译（审计清零）；英文模式时长 1h23m/12m30s 格式

### Fixed
- 并行同名工具链项目共用数据目录导致的配置互踩问题定位与规避

## [0.4.0] - 2026-11-07

### Added
- 应用内自动更新：tauri-plugin-updater + ed25519 签名校验，设置页检查/一键更新（Release 流水线自动生成 latest.json）
- 全局热键自定义（保存时校验有效性/冲突）
- 窗口标题记录（隐私开关默认关 + 排除规则），时间线页新增「Top 标题」
- 效率趋势：效率评分按天成图（时长/效率切换，评分三档配色）
- 周报导出：一键生成 Markdown 周报（汇总/分类/Top 应用/每日）到下载目录
- i18n 中英双语：全界面（含托盘菜单/通知/悬浮窗）跟随设置切换，缺译回退中文
- `--smoke N` 无界面冒烟模式（写 smoke-report.json，退出码 0/3）
- 设置 schema_version 迁移链

### Changed
- 汇总卡合并（总活跃+日均+环比同卡），单/多日视图布局稳定
- 分类树：「开发/前端」层级名，顶层汇总环形图 + 树形占比
- 应用别名：详情弹窗改名，排行/悬浮窗/时间线读取层全局生效
- 本地 HTTP API 5 端点（live/dashboard/timeline/settings/version）

## [0.3.0] - 2026-11-06

### 重写
- 全量重写为 Rust 核心 + Tauri 2 壳 + Svelte 5 前端（独立 exe 约 11 MB，旧 C#/WPF 实现保留在 main/dev 历史）
- tracker-core 纯逻辑库：聚合状态机（空闲/锁屏/睡眠/暂停、跨午夜切分），11 个单元测试

### Added
- 仪表盘：今天/昨天/本周/本月/自定义区间，汇总卡（总活跃+日均+环比、每日目标进度）、应用排行、分类占比、每日趋势
- 时间线：24 小时色带（滚轮缩放/拖拽平移/明细表+搜索）、状态区间（统计中/空闲/锁定/挂起）
- 日历热力图（GitHub contribution 风格，点选跳转）
- 对比视图：双时段应用/分类并排对比（A/B 交换、快捷预设、变化 %）
- 分类树：分类名支持「开发/前端」层级，顶层汇总环形图 + 树形占比
- 应用详情下钻 + 应用改名别名（读取层全局生效）
- 桌面悬浮窗：Win11 Mica 常显磨砂 + DWM 圆角，缩略/贴边收起成细线（动画）、置顶/锁定/透明度/配色右键菜单、双击开关主界面
- 托盘：实时 tooltip、左键主界面、右键菜单（悬浮窗✓/暂停✓/设置/退出）
- 全局热键 Ctrl+Alt+D / Ctrl+Alt+W；休息提醒；分类限额系统通知
- 本地 HTTP API（127.0.0.1:59598：/api/live|dashboard|timeline|settings|version）
- JSON 全量备份导出/恢复；数据保留策略；CSV 导出；开机自启；单实例
- 深色/浅色主题；扫描桌面/开始菜单快捷方式补分类
- GitHub Actions：CI（frontend/rust/tauri-build/cross-check/聚合）+ Release（tag 触发、版本校验、SHA256SUMS）

### Fixed
- 统计程序自身不再被统计（路径 + 进程名双保险，历史行自动清理）
- 悬浮窗圆角白边（圆角交 DWM 裁切）、失焦玻璃消失（改 Mica 常显）、置顶失效（背板后重设 topmost）
- 贴边收起：不贴边不吸附、锚点不漂移、原地展开 + 去抖防震荡、四边细线动画

## [0.2.0] - 2026-11-01

### Added
- 交互式时间线：滚轮缩放（光标锚定）、左键拖拽平移、刻度自适应（Core.TimelineScale，8 单测）
- 分类下钻：点环形图扇区/图例进入该分类的应用构成，点击图例返回
- 分类每日限额 + 托盘气泡通知（当日去重）+ 悬浮窗状态点变色
- JSON 全量导出/导入（schemaVersion=1；合并=跳过已存在；替换=R2 确认）
- 悬浮窗逐小时迷你柱状图（实时增长）
- 全局快捷键：Ctrl+Alt+U 主界面、Ctrl+Alt+P 暂停/恢复（RegisterHotKey）
- 排行右键菜单：按行设置分类覆盖，优先于规则匹配
- 悬浮窗真 Acrylic 磨砂（SetWindowCompositionAttribute）+ 屏幕边界钳制
- 排行榜新增应用实时加入（免手动刷新）；秒表每秒直读数据库
- GitHub Actions CI（windows-latest build/test/vulnerable-scan + 聚合 ci）

### Fixed
- 设置窗口保存报错（非模态窗口 DialogResult）+ 底部按钮不可见 + 数字框文字裁切
- 悬浮窗计时口径与仪表盘统一（每秒直读数据库）；默认贴左上角 + 拖不越界
- 排行占比 4266% 缺陷（基数未加载）；日期步进单日死按钮
- 分类规则覆盖本机全部软件（桌面/开始菜单扫描 + 互联网常用软件）
- 主界面启动只开悬浮窗（widget-first）

## [0.1.0] - 2026-10-31

### Added
- 前台活跃时间统计（1s 轮询 + 空闲检测 + 锁屏/睡眠感知 + 跨午夜切分）
- 分类规则引擎（关键词匹配 + 覆盖表 + 未分类）
- SQLite WAL 持久化（前台明细 + 每日聚合 + 保留策略）
- WPF 仪表盘（排行/环形图/时间线/趋势/CSV 导出）+ 系统托盘 + 悬浮窗
- 设置窗口（阈值/轮询/排除/规则/限额/自启/主题）+ 开机自启 + 久坐提醒
- GitHub 开源发布（MIT）
