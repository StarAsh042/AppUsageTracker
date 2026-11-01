# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
