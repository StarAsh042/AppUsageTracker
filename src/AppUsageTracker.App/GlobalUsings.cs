// WPF 与 WinForms（托盘 NotifyIcon、隐式全局 using）混用时的类型二义性，统一在此消解为 WPF 侧。
// System.Drawing.Pen 仅图标绘制使用，由隐式 using 自然命中；
// 自绘控件等纯 WPF 文件中的例外（如 Media.Pen）用文件级 using alias 覆盖。
global using Application = System.Windows.Application;
global using MessageBox = System.Windows.MessageBox;
global using Brush = System.Windows.Media.Brush;
global using Point = System.Windows.Point;
global using MouseEventArgs = System.Windows.Input.MouseEventArgs;
