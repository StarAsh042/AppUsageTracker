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

        /// <summary>该应用今日已落库的活跃秒数（每秒随数据库刷新）。</summary>
        public double BaseActiveSeconds { get; set; }

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

    /// <summary>今日已落库的活跃秒数与按应用缓存（每秒直读数据库刷新）。</summary>
    private double _todayBaseSeconds;
    private Dictionary<string, double> _appTodaySeconds = new(StringComparer.OrdinalIgnoreCase);
    private bool _baseLoaded;
    private bool _fetching;

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
    /// <summary>
    /// 实时刷新总活跃/日均与排行榜：每秒直读数据库（区间闭合时服务会即时落库），
    /// 排行榜整体按实时总数重算占比，当前应用行的时间同步跳动。
    /// 仅在查看包含今天的范围时生效。
    /// </summary>
    private void TickLive()
    {
        if (EndDate != DateTime.Today) return;

        if (!_fetching)
        {
            _fetching = true;
            var key = Format.DateKey(DateTime.Today);
            _ = Task.Run(() => (Totals: _store.QueryDailyTotals(key, key), PerApp: _store.QueryDaily(key, key)))
                .ContinueWith(t =>
                {
                    _fetching = false;
                    if (!t.IsCompletedSuccessfully) return;
                    _todayBaseSeconds = t.Result.Totals.FirstOrDefault(p => p.UsageDate == key)?.ActiveSeconds ?? 0;
                    _appTodaySeconds = t.Result.PerApp.ToDictionary(d => d.AppKey, d => d.ActiveSeconds, StringComparer.OrdinalIgnoreCase);
                    _baseLoaded = true;
                    TickLive(); // 数据到位后立即刷新本秒显示
                }, TaskScheduler.FromCurrentSynchronizationContext());
        }

        if (!_baseLoaded) return;

        var segment = _service.CurrentSegmentSeconds();
        var live = _todayBaseSeconds + segment;
        TotalActiveText = Format.Duration(live);
        DailyAverageText = Format.Duration(live / ((EndDate - StartDate).Days + 1));

        if (Ranking.Count == 0 || live <= 0) return;

        // 排行榜整体按实时总数重算占比，当前应用行的时间同步跳动
        var current = _service.CurrentApp;
        foreach (var row in Ranking)
        {
            var isCurrent = current is not null &&
                string.Equals(row.AppKey, current.AppKey, StringComparison.OrdinalIgnoreCase);
            if (_appTodaySeconds.TryGetValue(row.AppKey, out var dbActive)) row.BaseActiveSeconds = dbActive;
            var rowActive = row.BaseActiveSeconds + (isCurrent ? segment : 0);
            if (isCurrent) row.ActiveText = Format.Duration(rowActive);
            row.Percent = Math.Round(rowActive / live * 100, 1);
        }
    }

    private (DateTime Start, DateTime End) ResolveRange(string range) => range switch
    {
        "yesterday" => (DateTime.Today.AddDays(-1), DateTime.Today.AddDays(-1)),
        "week" => (Format.StartOfThisWeek(), DateTime.Today),
        "month" => (Format.StartOfThisMonth(), DateTime.Today),
        _ => (DateTime.Today, DateTime.Today),
    };

    [RelayCommand]
    private async Task SetRangeAsync(string? range)
    {
        if (string.IsNullOrEmpty(range)) return;
        var (start, end) = ResolveRange(range);
        StartDate = start;
        EndDate = end;
        await LoadAsync();
    }

    /// <summary>日期步进：param 形如 "start:-1" / "end:1"。</summary>
    [RelayCommand]
    private async Task ShiftDateAsync(string? param)
    {
        if (string.IsNullOrEmpty(param)) return;
        var parts = param.Split(':');
        if (parts.Length != 2 || !int.TryParse(parts[1], out var days)) return;

        if (parts[0] == "start") StartDate = StartDate.AddDays(days);
        else EndDate = EndDate.AddDays(days);

        if (EndDate < StartDate) EndDate = StartDate;
        if (StartDate > DateTime.Today) StartDate = DateTime.Today;
        if (EndDate > DateTime.Today) EndDate = DateTime.Today;
        await LoadAsync();
    }

    [RelayCommand]
    private Task RefreshAsync() => LoadAsync();

    [RelayCommand]
    private async Task ApplyDateRangeAsync()
    {
        if (EndDate < StartDate) EndDate = StartDate;
        await LoadAsync();
    }

    public async Task LoadAsync()
    {
        try
        {
            await LoadAsyncCore();
        }
        catch (Exception ex)
        {
            // LoadAsync 常以 fire-and-forget 调用，必须自捕获异常否则会被静默吞掉
            Logger.Error("仪表盘数据加载失败", ex);
        }
    }

    private async Task LoadAsyncCore()
    {
        var from = Format.DateKey(StartDate);
        var to = Format.DateKey(EndDate);
        RangeTitle = StartDate == EndDate ? $"{StartDate:M月d日}" : $"{StartDate:M月d日} – {EndDate:M月d日}";

        var rows = await Task.Run(() => _store.QueryDaily(from, to));
        var totals = await Task.Run(() => _store.QueryDailyTotals(from, to));

        // 播种实时秒表基数：范围包含今天时，以库中今日总数为基准（避免打开初期基数为 0 的错算）
        if (EndDate == DateTime.Today)
        {
            var todayKey = Format.DateKey(DateTime.Today);
            _todayBaseSeconds = totals.Where(t => t.UsageDate == todayKey).Sum(t => t.ActiveSeconds);
            _baseLoaded = true;
        }

        var prevTotals = await Task.Run(() =>
        {
            var len = (EndDate - StartDate).Days + 1;
            var prevEnd = StartDate.AddDays(-1);
            var prevStart = prevEnd.AddDays(-(len - 1));
            return _store.QueryDailyTotals(Format.DateKey(prevStart), Format.DateKey(prevEnd));
        });

        var resolver = new CategoryResolver(_settings.CategoryRules);
        var grouped = rows
            .GroupBy(r => r.AppKey, StringComparer.OrdinalIgnoreCase)
            .Select(g =>
            {
                var best = g.OrderByDescending(r => r.ActiveSeconds).First();
                return new
                {
                    App = best,
                    Category = resolver.Resolve(best.DisplayName, best.ProcessName, best.ExePath),
                    Active = g.Sum(r => r.ActiveSeconds),
                };
            })
            .Where(x => x.Active > 0) // 不显示 0 秒的应用（历史遗留的无活跃记录）
            .OrderByDescending(x => x.Active)
            .ToList();

        var totalActive = grouped.Sum(x => x.Active);
        var dayCount = (EndDate - StartDate).Days + 1;

        TotalActiveText = Format.Duration(totalActive);
        DailyAverageText = Format.Duration(totalActive / dayCount);
        AppCountText = grouped.Count.ToString();
        TopAppText = grouped.Count > 0 ? grouped[0].App.DisplayName : "暂无数据";

        UpdateCompare(totalActive, prevTotals);

        // 分类配色：按首次出现顺序稳定取色，同分类恒定、相邻分类不撞色
        SolidColorBrush CategoryBrush(string name) => ThemeManager.StableBrush(name);

        Ranking = new ObservableCollection<RankItem>(grouped.Select((x, i) => new RankItem
        {
            Index = i + 1,
            DisplayName = x.App.DisplayName,
            Category = x.Category,
            CategoryBrush = CategoryBrush(x.Category),
            AppKey = x.App.AppKey,
            ExePath = x.App.ExePath,
            Icon = AppIconCache.GetIcon(x.App.ExePath),
            BaseActiveSeconds = x.Active,
            ActiveText = Format.Duration(x.Active),
            Percent = totalActive > 0 ? Math.Round(x.Active / totalActive * 100, 1) : 0,
        }));

        // 分类占比（环形图 + 图例）
        var byCategory = grouped
            .Where(x => x.Active > 0)
            .GroupBy(x => x.Category, StringComparer.Ordinal)
            .Select(g => new { Name = g.Key, Active = g.Sum(x => x.Active) })
            .OrderByDescending(x => x.Active)
            .ToList();

        TopCategoryText = byCategory.Count > 0
            ? $"{byCategory[0].Name} · {Format.Duration(byCategory[0].Active)}"
            : "暂无数据";

        Categories = new ObservableCollection<CategoryItem>(byCategory.Select(x => new CategoryItem
        {
            Name = x.Name,
            DurationText = Format.Duration(x.Active),
            PercentText = totalActive > 0 ? $"{x.Active / totalActive * 100:0.0}%" : "0%",
            Brush = CategoryBrush(x.Name),
        }));

        DonutSegments = byCategory
            .Select(x => new DonutSegment { Value = x.Active, Brush = CategoryBrush(x.Name), Label = x.Name })
            .ToList();

        await LoadTimelineAsync(resolver);
        await LoadTrendAsync();
    }

    private void UpdateCompare(double currentActive, List<DailyTotalPoint> prevTotals)
    {
        var prevActive = prevTotals.Sum(t => t.ActiveSeconds);
        if (prevActive <= 0)
        {
            CompareText = "上一周期无数据";
            CompareBrush = (SolidColorBrush)Application.Current.Resources["StoppedBrush"];
            return;
        }

        var change = (currentActive - prevActive) / prevActive * 100;
        CompareText = change >= 0
            ? $"较上一周期 ↑ {change:0}%"
            : $"较上一周期 ↓ {Math.Abs(change):0}%";
        CompareBrush = change >= 0
            ? (SolidColorBrush)Application.Current.Resources["WarnBrush"]
            : (SolidColorBrush)Application.Current.Resources["OkBrush"];
    }

    private async Task LoadTimelineAsync(CategoryResolver resolver)
    {
        if (StartDate != EndDate)
        {
            TimelineSegments = Array.Empty<TimelineSegment>();
            TimelineHint = "时间线仅支持单日查看，请选择具体的一天。";
            return;
        }

        var day = StartDate;
        var key = Format.DateKey(day);
        var intervals = await Task.Run(() => _store.QueryIntervals(key, key));

        SolidColorBrush BrushFor(string appKey) => ThemeManager.StableBrush(appKey);

        TimelineSegments = intervals
            .Select(i =>
            {
                var start = Math.Clamp((i.StartTime - day.Date).TotalDays, 0.0, 1.0);
                var end = Math.Clamp((i.EndTime - day.Date).TotalDays, 0.0, 1.0);
                return new TimelineSegment
                {
                    StartFraction = start,
                    WidthFraction = Math.Max(end - start, 0.0005),
                    Brush = BrushFor(i.AppKey),
                    Tooltip = $"{resolver.Resolve(i.DisplayName, i.ProcessName, i.ExePath)} · {i.DisplayName}\n{i.StartTime:HH:mm:ss} – {i.EndTime:HH:mm:ss}（{Format.Duration(i.DurationSeconds)}）",
                };
            })
            .ToList();
        TimelineHint = intervals.Count == 0 ? "这一天没有前台使用记录。" : "";
    }

    private async Task LoadTrendAsync()
    {
        var from = Format.DateKey(DateTime.Today.AddDays(-13));
        var to = Format.DateKey(DateTime.Today);
        var totals = await Task.Run(() => _store.QueryDailyTotals(from, to));
        var map = totals.ToDictionary(t => t.UsageDate, t => t.ActiveSeconds, StringComparer.Ordinal);

        var max = Math.Max(60, totals.Count == 0 ? 0 : totals.Max(t => t.ActiveSeconds));
        var normalBrush = Fade(ThemeManager.FromHex(ThemeManager.SeriesColors[0]), 0.55);
        var todayBrush = (SolidColorBrush)Application.Current.Resources["PrimaryBrush"];

        var items = new List<TrendItem>();
        for (var day = DateTime.Today.AddDays(-13); day <= DateTime.Today; day = day.AddDays(1))
        {
            var key = Format.DateKey(day);
            var value = map.GetValueOrDefault(key);
            items.Add(new TrendItem
            {
                Label = day.ToString("M/d"),
                Tooltip = $"{day:M月d日}：{Format.Duration(value)}",
                BarHeight = Math.Max(3, ChartHeightPx * (value / max)),
                BarBrush = day == DateTime.Today ? todayBrush : normalBrush,
            });
        }

        Trend = new ObservableCollection<TrendItem>(items);
    }

    private static SolidColorBrush Fade(SolidColorBrush brush, double opacity)
    {
        var faded = brush.Clone();
        faded.Opacity = opacity;
        faded.Freeze();
        return faded;
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
