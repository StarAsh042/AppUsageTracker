using System.Collections.ObjectModel;
using System.IO;
using System.Windows;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using System.Windows.Threading;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using AppUsageTracker.App.Controls;
using AppUsageTracker.App.Services;
using AppUsageTracker.App.Utils;
using AppUsageTracker.Core;
using AppUsageTracker.Core.Logging;
using AppUsageTracker.Core.Models;
using AppUsageTracker.Core.Monitoring;
using AppUsageTracker.Core.Storage;
using AppUsageTracker.Core.Tracking;

namespace AppUsageTracker.App.ViewModels;

    /// <summary>排行榜单行（ActiveText/Percent/BaseActiveSeconds 可变，支持每秒按数据库实时刷新）。</summary>
    public partial class RankItem : ObservableObject
    {
        public int Index { get; init; }
        public string DisplayName { get; init; } = "";
        public string Category { get; init; } = "";
        public SolidColorBrush CategoryBrush { get; init; } = null!;
        public string AppKey { get; init; } = "";
        public string ExePath { get; init; } = "";
        public BitmapSource? Icon { get; init; }

        /// <summary>构建时已落库的活跃秒数（不含当前进行中的区间）。</summary>
        public double BaseActiveSeconds { get; init; }

        [ObservableProperty] private string _activeText = "";
        [ObservableProperty] private double _percent;
    }

/// <summary>分类占比图例行。</summary>
public sealed class CategoryItem
{
    public string Name { get; init; } = "";
    public string DurationText { get; init; } = "";
    public string PercentText { get; init; } = "";
    public SolidColorBrush Brush { get; init; } = null!;
}

/// <summary>近 14 天趋势单根柱。</summary>
public sealed class TrendItem
{
    public string Label { get; init; } = "";
    public string Tooltip { get; init; } = "";
    public double BarHeight { get; init; }
    public SolidColorBrush BarBrush { get; init; } = null!;
}

/// <summary>仪表盘视图模型：日期范围查询、排行、分类占比、单日时间线、趋势、洞察、导出。</summary>
public partial class DashboardViewModel : ObservableObject
{
    private const int ChartHeightPx = 120;

    private readonly IUsageStore _store;
    private readonly TrackingService _service;
    private readonly TrackerSettings _settings;
    private readonly Action _openSettings;
    private readonly DispatcherTimer _stateTimer;
    private readonly DispatcherTimer _liveTimer;

    /// <summary>今日已落库的活跃秒数缓存（每 30 秒刷新一次，叠加实时区间后构成秒表）。</summary>
    private double _todayBaseSeconds;
    private DateTime _lastBaseFetch = DateTime.MinValue;
    private bool _baseLoaded;

    [ObservableProperty] private DateTime _startDate = DateTime.Today;
    [ObservableProperty] private DateTime _endDate = DateTime.Today;
    [ObservableProperty] private string _rangeTitle = "今天";
    [ObservableProperty] private string _totalActiveText = "--";
    [ObservableProperty] private string _dailyAverageText = "--";
    [ObservableProperty] private string _topCategoryText = "--";
    [ObservableProperty] private string _appCountText = "--";
    [ObservableProperty] private string _topAppText = "--";
    [ObservableProperty] private string _compareText = "";
    [ObservableProperty] private SolidColorBrush _compareBrush = null!;
    [ObservableProperty] private string _stateText = "";
    [ObservableProperty] private SolidColorBrush _stateBrush = null!;
    [ObservableProperty] private ObservableCollection<RankItem> _ranking = new();
    [ObservableProperty] private ObservableCollection<CategoryItem> _categories = new();
    [ObservableProperty] private IReadOnlyList<DonutSegment> _donutSegments = Array.Empty<DonutSegment>();
    [ObservableProperty] private IReadOnlyList<TimelineSegment> _timelineSegments = Array.Empty<TimelineSegment>();
    [ObservableProperty] private string _timelineHint = "";
    [ObservableProperty] private ObservableCollection<TrendItem> _trend = new();

    public DashboardViewModel(IUsageStore store, TrackingService service, TrackerSettings settings, Action openSettings)
    {
        _store = store;
        _service = service;
        _settings = settings;
        _openSettings = openSettings;
        StateBrush = (SolidColorBrush)Application.Current.Resources["OkBrush"];
        CompareBrush = (SolidColorBrush)Application.Current.Resources["StoppedBrush"];

        _stateTimer = new DispatcherTimer { Interval = TimeSpan.FromSeconds(2) };
        _stateTimer.Tick += (_, _) => RefreshState();
        _stateTimer.Start();
        RefreshState();

        // 秒表：每秒刷新今日活跃（已落库缓存 + 当前未闭合区间）
        _liveTimer = new DispatcherTimer { Interval = TimeSpan.FromSeconds(1) };
        _liveTimer.Tick += (_, _) => TickLive();
        _liveTimer.Start();

        _ = SetRangeAsync("today");
    }

    /// <summary>实时刷新总活跃/日均；仅在查看包含今天的范围时生效。</summary>
    /// <summary>实时刷新总活跃/日均；仅在查看包含今天的范围时生效。</summary>
    private void TickLive()
    {
        if (EndDate != DateTime.Today) return;

        var now = DateTime.Now;
        if ((now - _lastBaseFetch).TotalSeconds >= 30)
        {
            _lastBaseFetch = now;
            var key = Format.DateKey(DateTime.Today);
            _ = Task.Run(() => _store.QueryDailyTotals(key, key))
                .ContinueWith(t =>
                {
                    if (t.IsCompletedSuccessfully) _todayBaseSeconds = t.Result.FirstOrDefault()?.ActiveSeconds ?? 0;
                }, TaskScheduler.FromCurrentSynchronizationContext());
        }

        var segment = _service.CurrentSegmentSeconds();
        var live = _todayBaseSeconds + segment;
        TotalActiveText = Format.Duration(live);
        DailyAverageText = Format.Duration(live / ((EndDate - StartDate).Days + 1));

        // 排行榜里当前应用的行同步跳动
        var current = _service.CurrentApp;
        if (current is null || segment <= 0 || live <= 0) return;
        var item = Ranking.FirstOrDefault(r =>
            string.Equals(r.AppKey, current.AppKey, StringComparison.OrdinalIgnoreCase));
        if (item is null) return;
        item.ActiveText = Format.Duration(item.BaseActiveSeconds + segment);
        item.Percent = Math.Round((item.BaseActiveSeconds + segment) / live * 100, 1);
    }

    private void RefreshState()
    {
        (StateText, StateBrush) = _service.State switch
        {
            TrackingState.Active => ("监控中", OkBrush()),
            TrackingState.Idle => ("空闲中", WarnBrush()),
            TrackingState.Locked => ("会话已锁定", StoppedBrush()),
            TrackingState.Suspended => ("系统挂起", StoppedBrush()),
            TrackingState.Paused => ("已暂停（托盘菜单可恢复）", StoppedBrush()),
            _ => ("未知", StoppedBrush()),
        };
    }

    private static SolidColorBrush OkBrush() => (SolidColorBrush)Application.Current.Resources["OkBrush"];
    private static SolidColorBrush WarnBrush() => (SolidColorBrush)Application.Current.Resources["WarnBrush"];
    private static SolidColorBrush StoppedBrush() => (SolidColorBrush)Application.Current.Resources["StoppedBrush"];

    [RelayCommand]
    private void OpenSettings() => _openSettings();

    [RelayCommand]
    private void ExportCsv()
    {
        var dialog = new Microsoft.Win32.SaveFileDialog
        {
            Title = "导出 CSV",
            Filter = "CSV 文件 (*.csv)|*.csv",
            FileName = $"软件使用统计_{Format.DateKey(StartDate)}_{Format.DateKey(EndDate)}.csv",
        };

        if (dialog.ShowDialog() != true) return;

        try
        {
            var from = Format.DateKey(StartDate);
            var to = Format.DateKey(EndDate);
            var resolver = new CategoryResolver(_settings.CategoryRules);
            Task.Run(() => CsvExporter.ExportRange(_store, resolver, from, to, dialog.FileName)).Wait();
            MessageBox.Show($"已导出到：\n{dialog.FileName}", "导出成功", MessageBoxButton.OK, MessageBoxImage.Information);
        }
        catch (Exception ex)
        {
            MessageBox.Show("导出失败：" + ex.Message, "导出失败", MessageBoxButton.OK, MessageBoxImage.Error);
        }
    }
}
