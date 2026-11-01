using AppUsageTracker.Core.Logging;
using AppUsageTracker.Core.Models;
using AppUsageTracker.Core.Monitoring;
using AppUsageTracker.Core.Storage;
namespace AppUsageTracker.Core.Tracking;

/// <summary>
/// 监控编排器：装配前台监控、存活监控、会话事件与聚合器，并负责周期性落库。
/// 线程模型：监控器回调都在线程池线程触发，内部用一把锁串行化对聚合器的访问。
/// </summary>
public sealed class TrackingService : IDisposable
{
    private readonly TrackerSettings _settings;
    private readonly IUsageStore _store;
    private readonly object _gate = new();

    private UsageAggregator _aggregator;
    private ForegroundMonitor? _foregroundMonitor;
    private SessionEventSource? _sessionEvents;
    private System.Threading.Timer? _flushTimer;
    private readonly System.Collections.Generic.HashSet<string> _notifiedCategoriesToday = new(StringComparer.Ordinal);
    private System.Threading.Timer? _limitTimer;
    private int _flushing;
    private bool _started;

    /// <summary>状态变化通知（工作线程触发，UI 需自行调度）。</summary>
    public event Action<TrackingState>? StateChanged;

    /// <summary>久坐提醒：连续活跃使用达到设定分钟数时触发（参数为连续时长）。</summary>
    public event Action<TimeSpan>? RestReminderDue;

    /// <summary>分类日限额达到时触发（参数：分类名）。</summary>
    public event Action<string>? CategoryLimitReached;

    /// <summary>当前连续活跃使用的起点；null 表示当前不处于活跃状态。</summary>
    private DateTime? _activeSince;
    private DateTime _lastReminderAt = DateTime.MinValue;

    public TrackingService(TrackerSettings settings, IUsageStore store)
    {
        _settings = settings;
        _store = store;
        _aggregator = CreateAggregator(settings);
    }

    public TrackingState State { get { lock (_gate) return _aggregator.State; } }

    /// <summary>当前前台应用（实时，供悬浮窗显示）。</summary>
    public AppIdentity? CurrentApp { get { lock (_gate) return _aggregator.CurrentApp; } }

    /// <summary>当前前台区间的实时秒数（秒表跳动用）。</summary>
    public double CurrentSegmentSeconds() => _aggregator.CurrentSegmentSeconds(DateTime.Now);

    private static UsageAggregator CreateAggregator(TrackerSettings settings) =>
        new(settings.IdleThresholdSeconds, settings.ExcludedAppKeys);

    public void Start()
    {
        lock (_gate)
        {
            if (_started) return;
            _started = true;

            _foregroundMonitor = new ForegroundMonitor(_settings.ForegroundPollMs);
            _foregroundMonitor.Tick += OnForegroundTick;
            _foregroundMonitor.Start();

            _sessionEvents = new SessionEventSource();
            _sessionEvents.Locked += () => { lock (_gate) _aggregator.OnLocked(DateTime.Now); RaiseStateChanged(); };
            _sessionEvents.Unlocked += () => { lock (_gate) _aggregator.OnUnlocked(CurrentForegroundUnsafe(), DateTime.Now); RaiseStateChanged(); };
            _sessionEvents.Suspending += () => { lock (_gate) _aggregator.OnSuspend(DateTime.Now); RaiseStateChanged(); };
            _sessionEvents.Resumed += () => { lock (_gate) _aggregator.OnResume(CurrentForegroundUnsafe(), DateTime.Now); RaiseStateChanged(); };

            _flushTimer = new System.Threading.Timer(_ => FlushNow(), null, 30_000, 30_000);
            _limitTimer = new System.Threading.Timer(_ => CheckCategoryLimits(), null, 60_000, 60_000);
        }

        CleanupRetention();
        Logger.Info($"监控已启动（口径=前台活跃，空闲阈值 {_settings.IdleThresholdSeconds}s，轮询 {_settings.ForegroundPollMs}ms）");
    }

    /// <summary>停止监控并落盘。进程退出前必须调用。</summary>
    public void Stop()
    {
        lock (_gate)
        {
            if (!_started) return;
            _started = false;

            _flushTimer?.Dispose();
            _flushTimer = null;
            _limitTimer?.Dispose();
            _limitTimer = null;
            _sessionEvents?.Dispose();
            _sessionEvents = null;
            _foregroundMonitor?.Dispose();
            _foregroundMonitor = null;

            _aggregator.CloseOpenSegment(DateTime.Now);
        }

        FlushNow();
        Logger.Info("监控已停止");
    }

    /// <summary>托盘「暂停监控」。</summary>
    public void Pause()
    {
        lock (_gate)
        {
            _aggregator.Pause(DateTime.Now);
            RaiseStateChanged();
        }
        Logger.Info("监控已由用户暂停");
    }

    /// <summary>托盘「恢复监控」。</summary>
    public void Resume()
    {
        lock (_gate)
        {
            _aggregator.Resume(_foregroundMonitor?.CurrentForeground, DateTime.Now);
            RaiseStateChanged();
        }
        Logger.Info("监控已由用户恢复");
    }

    /// <summary>应用新设置（阈值、轮询、排除名单、提醒等）：先落盘再整体重启。</summary>
    public void ApplySettings(TrackerSettings newSettings)
    {
        Stop();
        lock (_gate)
        {
            _settings.IdleThresholdSeconds = newSettings.IdleThresholdSeconds;
            _settings.ForegroundPollMs = newSettings.ForegroundPollMs;
            _settings.RestReminderMinutes = newSettings.RestReminderMinutes;
            _settings.RetentionDays = newSettings.RetentionDays;
            _settings.ShowFloatingWidget = newSettings.ShowFloatingWidget;
            _notifiedCategoriesToday.Clear();
            _settings.CategoryRules = new List<CategoryRule>(newSettings.CategoryRules);
            _settings.ExcludedAppKeys = new List<string>(newSettings.ExcludedAppKeys);
            _activeSince = null;
            _aggregator = CreateAggregator(_settings);
        }
        Start();
    }

    /// <summary>立即把内存中的待写数据落库（可从任意线程调用）。</summary>
    public void FlushNow()
    {
        if (Interlocked.Exchange(ref _flushing, 1) == 1) return;
        try
        {
            List<ForegroundInterval> intervals;
            List<DailyAppUsage> daily;
            lock (_gate)
            {
                intervals = _aggregator.DrainPendingIntervals();
                daily = _aggregator.DrainPendingDaily();
            }

            if (intervals.Count == 0 && daily.Count == 0) return;

            try
            {
                _store.SaveIntervals(intervals);
                _store.AddDailyUsage(daily);
                Logger.Info($"落库完成：{intervals.Count} 条前台明细，{daily.Count} 个每日增量。");
            }
            catch (Exception ex)
            {
                Logger.Error("落库失败，数据已放回内存队列", ex);
                lock (_gate) _aggregator.ReAddPending(intervals, daily);
            }
        }
        finally
        {
            Interlocked.Exchange(ref _flushing, 0);
        }
    }

    /// <summary>累计快照（当次运行以来的聚合，供界面参考）。</summary>
    public List<DailyAppUsage> SnapshotCumulative()
    {
        lock (_gate) return _aggregator.SnapshotCumulative();
    }

    private void OnForegroundTick(AppIdentity? foreground, double idleSeconds, DateTime now)
    {
        bool changed = false;
        bool segmentClosed = false;
        TrackingState state;
        lock (_gate)
        {
            var before = _aggregator.State;
            var intervalsBefore = _aggregator.PendingIntervalCount;
            _aggregator.OnTick(foreground, idleSeconds, now);
            state = _aggregator.State;
            changed = state != before;
            segmentClosed = _aggregator.PendingIntervalCount > intervalsBefore;
            UpdateRestReminder(state, now);
        }
        if (changed) RaiseStateChanged();
        // 区间闭合（切换应用/进入空闲等）意味着数据形态变化：立即落库，
        // 让悬浮窗与仪表盘的每秒数据库读取始终拿到最新数据
        if (segmentClosed) FlushNow();
    }

    /// <summary>久坐提醒：只累计「活跃」状态；空闲/锁屏/暂停视为休息并重新计时。</summary>
    private void UpdateRestReminder(TrackingState state, DateTime now)
    {
        if (state != TrackingState.Active)
        {
            _activeSince = null;
            return;
        }

        _activeSince ??= now;
        if (_settings.RestReminderMinutes <= 0) return;

        var continuous = now - _activeSince.Value;
        if (continuous >= TimeSpan.FromMinutes(_settings.RestReminderMinutes)
            && now - _lastReminderAt >= TimeSpan.FromMinutes(_settings.RestReminderMinutes))
        {
            _lastReminderAt = now;
            _activeSince = now; // 从提醒时刻重新计数
            var handler = RestReminderDue;
            try { handler?.Invoke(continuous); }
            catch (Exception ex) { Logger.Error("久坐提醒订阅者异常", ex); }
        }
    }

    /// <summary>每分钟检查各分类日限额，达到时触发一次通知（当日去重）。</summary>
    private void CheckCategoryLimits()
    {
        try
        {
            if (_settings.CategoryDailyLimitMinutes.Count == 0) return;
            var resolver = new Monitoring.CategoryResolver(_settings.CategoryRules);
            var today = DateTime.Today.ToString("yyyy-MM-dd");
            var perApp = _store.QueryDaily(today, today);
            var perCategory = new System.Collections.Generic.Dictionary<string, double>(StringComparer.Ordinal);
            foreach (var row in perApp)
            {
                var cat = resolver.Resolve(row.DisplayName, row.ProcessName, row.ExePath);
                perCategory.TryGetValue(cat, out var sec);
                perCategory[cat] = sec + row.ActiveSeconds;
            }
            var toNotify = Tracking.CategoryLimitChecker.CategoriesToNotify(
                perCategory,
                _settings.CategoryDailyLimitMinutes,
                _notifiedCategoriesToday);
            foreach (var cat in toNotify)
            {
                _notifiedCategoriesToday.Add(cat);
                var handler = CategoryLimitReached;
                try { handler?.Invoke(cat); }
                catch (Exception ex) { Logger.Error("限额通知订阅者异常", ex); }
            }
        }
        catch (Exception ex)
        {
            Logger.Error("分类限额检查失败", ex);
        }
    }

    /// <summary>清空全部数据（JSON 替换导入前调用）。先落盘刷新内存，再由调用方清库。</summary>
    public void PrepareForReplaceImport()
    {
        lock (_gate) _aggregator = CreateAggregator(_settings);
    }

    /// <summary>按保留天数清理过期数据。</summary>
    private void CleanupRetention()
    {
        if (_settings.RetentionDays <= 0) return;
        try
        {
            var cutoff = DateTime.Today.AddDays(-_settings.RetentionDays).ToString("yyyy-MM-dd");
            var (intervals, daily) = _store.CleanupBefore(cutoff);
            if (intervals + daily > 0)
            {
                Logger.Info($"保留策略：已清理 {_settings.RetentionDays} 天前的 {intervals} 条明细、{daily} 条聚合。");
            }
        }
        catch (Exception ex)
        {
            Logger.Error("保留策略清理失败", ex);
        }
    }

    private AppIdentity? CurrentForegroundUnsafe() => _foregroundMonitor?.CurrentForeground;

    private void RaiseStateChanged()
    {
        var handler = StateChanged;
        if (handler is null) return;
        try
        {
            handler(State);
        }
        catch (Exception ex)
        {
            Logger.Error("StateChanged 订阅者异常", ex);
        }
    }

    public void Dispose()
    {
        lock (_gate)
        {
            if (!_started) return;
        }
        Stop();
    }
}
