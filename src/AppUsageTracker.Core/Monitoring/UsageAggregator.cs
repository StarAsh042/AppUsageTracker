using AppUsageTracker.Core.Models;

namespace AppUsageTracker.Core.Monitoring;

/// <summary>
/// 统计状态机与内存聚合器（纯逻辑、无 Win32 依赖、线程安全）。
/// 接收监控器上报的原始观测，维护前台区间的开闭、空闲/锁屏/睡眠切换，
/// 并把时长按自然日累计到内存桶中；落库数据通过 Drain* 取走，
/// 失败可用 <see cref="ReAddPending"/> 放回。
/// </summary>
public sealed class UsageAggregator
{
    private readonly object _gate = new();
    private readonly HashSet<string> _excluded;
    private readonly double _idleThresholdSeconds;

    /// <summary>累计值（含已落库部分），供界面展示。</summary>
    private readonly Dictionary<string, DailyAppUsage> _cumulative = new();

    /// <summary>尚未写入数据库的增量。</summary>
    private readonly Dictionary<string, DailyAppUsage> _pending = new();

    private readonly List<ForegroundInterval> _pendingIntervals = new();

    private TrackingState _state = TrackingState.Active;
    private AppIdentity? _current;
    private DateTime _segmentStart;
    private DateTime _stateChangedAt;

    /// <summary>状态发生变化时触发（在内部锁内调用，订阅方须快速返回）。</summary>
    public event Action<TrackingState>? StateChanged;

    public UsageAggregator(double idleThresholdSeconds, IEnumerable<string>? excludedAppKeys = null)
    {
        if (idleThresholdSeconds <= 0) throw new ArgumentOutOfRangeException(nameof(idleThresholdSeconds));
        _idleThresholdSeconds = idleThresholdSeconds;
        _excluded = new HashSet<string>(excludedAppKeys ?? Enumerable.Empty<string>(), StringComparer.OrdinalIgnoreCase);
    }

    public TrackingState State { get { lock (_gate) return _state; } }

    /// <summary>待落库的前台区间数量（用于检测区间闭合时机）。</summary>
    public int PendingIntervalCount { get { lock (_gate) return _pendingIntervals.Count; } }

    /// <summary>当前前台应用（可能为 null；排除名单内的应用也会返回，调用方自行判断）。</summary>
    public AppIdentity? CurrentApp { get { lock (_gate) return _current; } }

    /// <summary>当前打开的前台区间已累计秒数；仅活跃态有效，空闲/锁屏/暂停或排除应用返回 0。</summary>
    public double CurrentSegmentSeconds(DateTime now)
    {
        lock (_gate)
        {
            if (_state != TrackingState.Active || _current is null) return 0;
            if (_excluded.Contains(_current.AppKey)) return 0;
            var seconds = (now - _segmentStart).TotalSeconds;
            return seconds > 0 ? seconds : 0;
        }
    }

    #region 前台状态机

    /// <summary>前台轮询 tick。</summary>
    public void OnTick(AppIdentity? foreground, double idleSeconds, DateTime now)
    {
        lock (_gate)
        {
            if (_state is TrackingState.Paused or TrackingState.Locked or TrackingState.Suspended)
            {
                // 睡眠恢复事件丢失的兜底：挂起超过 10 分钟视为事件丢失，自动回到活跃态
                if (_state == TrackingState.Suspended && now - _stateChangedAt > TimeSpan.FromMinutes(10))
                {
                    EnterActive(foreground, now);
                }
                else
                {
                    return;
                }
            }

            bool isIdle = idleSeconds >= _idleThresholdSeconds;

            if (_state == TrackingState.Idle)
            {
                if (isIdle) return;
                EnterActive(foreground, now); // 用户重新活动
                return;
            }

            if (isIdle)
            {
                // 用户实际是在 idleSeconds 秒前停止操作的，区间应止于那一刻
                var idleStart = now.AddSeconds(-idleSeconds);
                Logging.Logger.Info($"进入空闲态（已无输入 {idleSeconds:0.#}s），前台={foreground?.DisplayName ?? "无"}");
                CloseCurrentSegment(idleStart < _segmentStart ? _segmentStart : idleStart);
                _state = TrackingState.Idle;
                _stateChangedAt = now;
                _current = null;
                StateChanged?.Invoke(_state);
                return;
            }

            if (!SameApp(foreground, _current))
            {
                CloseCurrentSegment(now);
                _current = foreground;
                _segmentStart = now;
            }
        }
    }

    public void OnLocked(DateTime now) => Transition(_state => _state is TrackingState.Active or TrackingState.Idle,
        now, TrackingState.Locked);

    public void OnUnlocked(AppIdentity? foreground, DateTime now) =>
        EnterActiveIf(_state => _state == TrackingState.Locked, foreground, now);

    public void OnSuspend(DateTime now) => Transition(_ => true, now, TrackingState.Suspended);

    public void OnResume(AppIdentity? foreground, DateTime now) =>
        EnterActiveIf(_state => _state == TrackingState.Suspended, foreground, now);

    /// <summary>托盘「暂停监控」。</summary>
    public void Pause(DateTime now) => Transition(_ => _state != TrackingState.Paused, now, TrackingState.Paused);

    /// <summary>托盘「恢复监控」。</summary>
    public void Resume(AppIdentity? foreground, DateTime now) =>
        EnterActiveIf(_state => _state == TrackingState.Paused, foreground, now);

    /// <summary>程序退出前闭合未完区间（不改变状态）。</summary>
    public void CloseOpenSegment(DateTime now)
    {
        lock (_gate) CloseCurrentSegment(now);
    }

    private void Transition(Func<TrackingState, bool> allowed, DateTime now, TrackingState target)
    {
        lock (_gate)
        {
            if (!allowed(_state)) return;
            CloseCurrentSegment(now);
            _state = target;
            _stateChangedAt = now;
            _current = null;
            StateChanged?.Invoke(_state);
        }
    }

    private void EnterActiveIf(Func<TrackingState, bool> allowed, AppIdentity? foreground, DateTime now)
    {
        lock (_gate)
        {
            if (!allowed(_state)) return;
            EnterActive(foreground, now);
        }
    }

    private void EnterActive(AppIdentity? foreground, DateTime now)
    {
        // 调用方必须已持有 _gate
        _state = TrackingState.Active;
        _stateChangedAt = now;
        _current = foreground;
        _segmentStart = now;
        StateChanged?.Invoke(_state);
    }

    private void CloseCurrentSegment(DateTime end)
    {
        // 调用方必须已持有 _gate
        var app = _current;
        if (app is null) return;
        _current = null;

        if (_excluded.Contains(app.AppKey)) return;

        var start = _segmentStart;
        var duration = (end - start).TotalSeconds;
        if (duration < 0.5) return; // 过滤 Alt+Tab 毛刺

        foreach (var (s, e) in SplitByDay(start, end))
        {
            AddUsage(s.Date, app, activeSeconds: (e - s).TotalSeconds, aliveSeconds: 0, launches: 0);
            _pendingIntervals.Add(new ForegroundInterval
            {
                AppKey = app.AppKey,
                ProcessName = app.ProcessName,
                DisplayName = app.DisplayName,
                ExePath = app.ExePath,
                StartTime = s,
                EndTime = e,
            });
        }
    }

    private static bool SameApp(AppIdentity? a, AppIdentity? b) =>
        string.Equals(a?.AppKey, b?.AppKey, StringComparison.OrdinalIgnoreCase);

    /// <summary>把跨午夜区间切分为多段自然日区间。</summary>
    internal static IEnumerable<(DateTime Start, DateTime End)> SplitByDay(DateTime start, DateTime end)
    {
        if (end <= start) yield break;
        var cur = start;
        while (cur.Date < end.Date)
        {
            var midnight = cur.Date.AddDays(1);
            yield return (cur, midnight);
            cur = midnight;
        }
        yield return (cur, end);
    }

    #endregion

    #region 聚合桶与落库数据

    private void AddUsage(DateTime day, AppIdentity app, double activeSeconds, double aliveSeconds, int launches)
    {
        // 调用方必须已持有 _gate
        var key = Key(day, app.AppKey);
        Accumulate(_cumulative, key, day, app, activeSeconds, aliveSeconds, launches);
        Accumulate(_pending, key, day, app, activeSeconds, aliveSeconds, launches);
    }

    private static string Key(DateTime day, string appKey) =>
        day.ToString("yyyy-MM-dd") + "|" + appKey;

    private static void Accumulate(Dictionary<string, DailyAppUsage> dict, string key, DateTime day,
        AppIdentity app, double activeSeconds, double aliveSeconds, int launches)
    {
        if (!dict.TryGetValue(key, out var bucket))
        {
            bucket = new DailyAppUsage
            {
                UsageDate = day.ToString("yyyy-MM-dd"),
                AppKey = app.AppKey,
                ProcessName = app.ProcessName,
                DisplayName = app.DisplayName,
                ExePath = app.ExePath,
            };
            dict[key] = bucket;
        }

        bucket.ActiveSeconds += activeSeconds;
        bucket.AliveSeconds += aliveSeconds;
        bucket.LaunchCount += launches;
    }

    /// <summary>取走待落库的前台明细。</summary>
    public List<ForegroundInterval> DrainPendingIntervals()
    {
        lock (_gate)
        {
            var result = _pendingIntervals.ToList();
            _pendingIntervals.Clear();
            return result;
        }
    }

    /// <summary>取走待落库的每日增量。</summary>
    public List<DailyAppUsage> DrainPendingDaily()
    {
        lock (_gate)
        {
            var result = _pending.Values.Select(b => b.Clone()).ToList();
            _pending.Clear();
            return result;
        }
    }

    /// <summary>落库失败时把数据放回待写队列，避免丢失。</summary>
    public void ReAddPending(IReadOnlyList<ForegroundInterval> intervals, IReadOnlyList<DailyAppUsage> daily)
    {
        lock (_gate)
        {
            _pendingIntervals.AddRange(intervals);
            foreach (var bucket in daily)
            {
                var app = new AppIdentity(bucket.AppKey, bucket.ProcessName, bucket.DisplayName, bucket.ExePath);
                Accumulate(_pending, Key(DateTime.Parse(bucket.UsageDate), bucket.AppKey),
                    DateTime.Parse(bucket.UsageDate), app,
                    bucket.ActiveSeconds, bucket.AliveSeconds, bucket.LaunchCount);
            }
        }
    }

    /// <summary>累计快照（供界面实时展示当日情况）。</summary>
    public List<DailyAppUsage> SnapshotCumulative()
    {
        lock (_gate) return _cumulative.Values.Select(b => b.Clone()).ToList();
    }

    #endregion
}
