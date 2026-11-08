<script lang="ts">
  import { api, fmtDuration, localDateKey, type DashboardData } from './lib/api';
  import { lang, en } from './lib/i18n';

  let { onPick = () => {} }: { onPick?: (date: string) => void } = $props();

  let cursor = $state(new Date());
  let mode = $state<'month' | 'year'>('month');
  let yearCursor = $state(new Date().getFullYear());
  let trend = $state<Map<string, number>>(new Map());
  let total = $state(0);
  let loading = $state(false);

  const t = $derived((s: string) => ($lang === 'en' ? (en[s] ?? s) : s));

  const WEEK_CN = $derived(
    $lang === 'en'
      ? ['M', 'T', 'W', 'T', 'F', 'S', 'S']
      : ['一', '二', '三', '四', '五', '六', '日'],
  );

  $effect(() => {
    load();
  });

  async function load() {
    loading = true;
    try {
      const p = (n: number) => String(n).padStart(2, '0');
      let from: string;
      let to: string;
      if (mode === 'year') {
        from = `${yearCursor}-01-01`;
        to = `${yearCursor}-12-31`;
      } else {
        const y = cursor.getFullYear();
        const m = cursor.getMonth();
        from = `${y}-${p(m + 1)}-01`;
        to = `${y}-${p(m + 1)}-${p(new Date(y, m + 1, 0).getDate())}`;
      }
      const data: DashboardData = await api.getDashboard(from, to);
      trend = new Map(data.trend.map((x) => [x.date, x.seconds]));
      total = data.total_active_seconds;
    } finally {
      loading = false;
    }
  }

  function shiftMonth(offset: number) {
    const d = new Date(cursor);
    d.setMonth(d.getMonth() + offset);
    cursor = d;
  }

  function shiftYear(offset: number) {
    yearCursor += offset;
  }

  function pick(d: number) {
    onPick(keyOf(grid.y, grid.m, d));
  }

  function pickDate(date: string) {
    onPick(date);
  }

  function keyOf(y: number, m: number, d: number): string {
    const p = (n: number) => String(n).padStart(2, '0');
    return `${y}-${p(m + 1)}-${p(d)}`;
  }

  // 月视图网格：周一开头
  const grid = $derived.by(() => {
    const y = cursor.getFullYear();
    const m = cursor.getMonth();
    const first = new Date(y, m, 1);
    const lead = (first.getDay() + 6) % 7;
    const days = new Date(y, m + 1, 0).getDate();
    const cells: (number | null)[] = Array(lead).fill(null);
    for (let d = 1; d <= days; d++) cells.push(d);
    while (cells.length % 7 !== 0) cells.push(null);
    return { y, m, cells };
  });

  const maxSeconds = $derived(Math.max(1, ...[...trend.values()]));

  // 年热力图：按周分列的小方块（GitHub contribution 风格）
  const yearGrid = $derived.by(() => {
    const y = yearCursor;
    const jan1 = new Date(y, 0, 1);
    const lead = (jan1.getDay() + 6) % 7;
    const dec31 = new Date(y, 11, 31);
    const totalDays = Math.round((dec31.getTime() - jan1.getTime()) / 86400000) + 1;
    const p = (n: number) => String(n).padStart(2, '0');
    const weeks: { date: string; d: number; seconds?: number }[][] = [];
    let wk: ({ date: string; d: number; seconds?: number } | null)[] = Array(lead).fill(null);
    for (let i = 0; i < totalDays; i++) {
      const d = new Date(y, 0, 1 + i);
      const key = `${y}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
      wk.push({ date: key, d: d.getDate(), seconds: trend.get(key) });
      if (wk.length === 7) {
        weeks.push(wk.filter((c): c is { date: string; d: number; seconds?: number } => c !== null));
        wk = [];
      }
    }
    if (wk.length) {
      weeks.push(wk.filter((c): c is { date: string; d: number; seconds?: number } => c !== null));
    }
    return weeks;
  });

  const yearMax = $derived(Math.max(1, ...[...trend.values()]));

  // 月份标签（在年热力图上方，按周位置分布）
  const yearMonths = $derived.by(() => {
    const y = yearCursor;
    const months: { name: string; weeks: number }[] = [];
    const MONTH_NAMES = $lang === 'en'
      ? ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec']
      : ['1月', '2月', '3月', '4月', '5月', '6月', '7月', '8月', '9月', '10月', '11月', '12月'];
    // 每月占的周数（近似：该月天数 / 7 向上取整）
    for (let m = 0; m < 12; m++) {
      const days = new Date(y, m + 1, 0).getDate();
      months.push({ name: MONTH_NAMES[m], weeks: Math.ceil(days / 7) });
    }
    return months;
  });

  const todayKey = $derived(localDateKey());

  function heat(seconds: number | undefined, max: number): string {
    if (seconds === undefined) return 'transparent';
    const r = seconds / max;
    return `color-mix(in srgb, var(--accent) ${Math.round(18 + r * 82)}%, transparent)`;
  }
</script>

<div class="cal">
  <div class="toolbar">
    <div class="mode-switch">
      <button class:on={mode === 'month'} onclick={() => { mode = 'month'; load(); }}>{t('月')}</button>
      <button class:on={mode === 'year'} onclick={() => { mode = 'year'; load(); }}>{t('年')}</button>
    </div>
    {#if mode === 'month'}
      <button class="nav" onclick={() => shiftMonth(-1)}>◀</button>
      <span class="month">
        {$lang === 'en'
          ? cursor.toLocaleDateString('en-US', { month: 'long', year: 'numeric' })
          : `${cursor.getFullYear()} 年 ${cursor.getMonth() + 1} 月`}
      </span>
      <button class="nav" onclick={() => shiftMonth(1)}>▶</button>
    {:else}
      <button class="nav" onclick={() => shiftYear(-1)}>◀</button>
      <span class="month">{yearCursor} 年</span>
      <button class="nav" onclick={() => shiftYear(1)}>▶</button>
    {/if}
    <span class="total">{fmtDuration(total)}</span>
  </div>

  {#if mode === 'year'}
    <div class="panel">
      <div class="year-heatmap">
        <div class="yh-months">
          <span class="yh-spacer"></span>
          {#each yearMonths as m (m.name)}
            <span class="yh-month" style="flex-grow:{m.weeks}">{m.name}</span>
          {/each}
        </div>
        <div class="yh-body">
          <div class="yh-days">
            {#each ['一','','三','','五','','日'] as d, i (i)}
              <span class="yh-day" class:visible={i % 2 === 0}>{d}</span>
            {/each}
          </div>
          <div class="yh-grid">
            {#each yearGrid as week, wi (wi)}
              <div class="yh-col">
                {#each week as cell (cell.date)}
                  {#if cell}
                    <button
                      class="yh-cell"
                      class:today={cell.date === todayKey}
                      style="background:{cell.seconds !== undefined ? heat(cell.seconds, yearMax) : 'color-mix(in srgb, var(--border) 30%, transparent)'}"
                      onclick={() => pickDate(cell.date)}
                      title="{cell.date}{cell.seconds !== undefined && cell.seconds > 0 ? ' · ' + fmtDuration(cell.seconds) : ''}"
                    ></button>
                  {:else}
                    <span class="yh-cell empty"></span>
                  {/if}
                {/each}
              </div>
            {/each}
          </div>
        </div>
        <div class="yh-scale">
          <span>{t('少')}</span>
          <span class="yh-sw" style="background:color-mix(in srgb, var(--accent) 25%, transparent)"></span>
          <span class="yh-sw" style="background:color-mix(in srgb, var(--accent) 50%, transparent)"></span>
          <span class="sw" style="background:color-mix(in srgb, var(--accent) 75%, transparent)"></span>
          <span class="yh-sw" style="background:var(--accent)"></span>
          <span>{t('多')}</span>
          {#if loading}<span class="loading-inline">{t('加载中')}…</span>{/if}
        </div>
      </div>
    </div>
  {:else}
    <div class="panel">
      <div class="week-head">
        {#each WEEK_CN as w (w)}<span>{w}</span>{/each}
      </div>
      <div class="grid">
        {#each grid.cells as d, i (i)}
          {#if d === null}
            <span class="cell empty-cell"></span>
          {:else}
            {@const key = keyOf(grid.y, grid.m, d)}
            {@const seconds = trend.get(key)}
            <button
              class="cell"
              class:today={key === todayKey}
              style="background:{heat(seconds, maxSeconds)}"
              onclick={() => pick(d)}
              title="{key}{seconds !== undefined ? ' · ' + fmtDuration(seconds) : ''}"
            >
              <span class="day">{d}</span>
              {#if seconds !== undefined && seconds > 0}
                <span class="min">{Math.round(seconds / 60)}分</span>
              {/if}
            </button>
          {/if}
        {/each}
      </div>
      <div class="scale">
        <span>{t('少')}</span>
        <span class="sw" style="background:color-mix(in srgb, var(--accent) 25%, transparent)"></span>
        <span class="sw" style="background:color-mix(in srgb, var(--accent) 50%, transparent)"></span>
        <span class="sw" style="background:color-mix(in srgb, var(--accent) 75%, transparent)"></span>
        <span class="sw" style="background:var(--accent)"></span>
        <span>{t('多')}</span>
        {#if loading}<span class="loading-inline">{t('加载中')}…</span>{/if}
      </div>
    </div>
  {/if}
</div>

<style>
  .cal {
    height: 100%;
    display: flex;
    flex-direction: column;
    padding: 14px 18px;
    gap: 14px;
    overflow-y: auto;
  }

  .toolbar {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .mode-switch {
    display: inline-flex;
    gap: 4px;
    margin-right: 6px;
  }

  .mode-switch button {
    background: var(--card);
    border: 1px solid var(--border);
    color: var(--text-dim);
    border-radius: 7px;
    padding: 6px 14px;
    font-size: 12px;
    cursor: pointer;
  }

  .mode-switch button.on {
    background: var(--accent);
    border-color: var(--accent);
    color: #fff;
  }

  .nav {
    background: var(--card);
    border: 1px solid var(--border);
    color: var(--text-dim);
    border-radius: 7px;
    padding: 6px 12px;
    cursor: pointer;
  }

  .nav:hover {
    color: var(--text);
    border-color: var(--accent);
  }

  .month {
    font-size: 15px;
    font-weight: 600;
  }

  .total {
    margin-left: auto;
    color: var(--text-dim);
    font-size: 13px;
  }

  .panel {
    background: var(--glass-card, var(--card));
    border: 1px solid var(--border);
    border-radius: 12px;
    padding: 16px;
  }

  .week-head {
    display: grid;
    grid-template-columns: repeat(7, 1fr);
    gap: 6px;
    margin-bottom: 6px;
  }

  .week-head span {
    text-align: center;
    color: var(--text-dim);
    font-size: 12px;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(7, 1fr);
    gap: 6px;
  }

  .cell {
    position: relative;
    aspect-ratio: 1.35;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: transparent;
    cursor: pointer;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    justify-content: flex-end;
    padding: 6px 7px;
    color: var(--text);
    transition: border-color 0.12s;
  }

  .cell:hover {
    border-color: var(--accent);
  }

  .cell.today {
    border-color: var(--accent);
    box-shadow: 0 0 0 1px var(--accent);
  }

  .empty-cell {
    border: 0;
    cursor: default;
  }

  .day {
    font-size: 13px;
    font-weight: 600;
  }

  .min {
    font-size: 10px;
    color: var(--text-dim);
  }

  /* 年热力图：GitHub contribution 风格，铺满面板 */
  .year-heatmap {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .yh-months {
    display: flex;
    gap: 3px;
    font-size: 10px;
    color: var(--text-dim);
    padding-left: 28px;
  }

  .yh-month {
    text-align: left;
    overflow: hidden;
    white-space: nowrap;
  }

  .yh-spacer {
    width: 24px;
    flex: none;
  }

  .yh-body {
    display: flex;
    gap: 4px;
  }

  .yh-days {
    display: flex;
    flex-direction: column;
    gap: 3px;
    width: 24px;
    flex: none;
  }

  .yh-day {
    height: 12px;
    font-size: 9px;
    color: var(--text-dim);
    line-height: 12px;
    text-align: right;
  }

  .yh-grid {
    flex: 1;
    display: flex;
    gap: 3px;
  }

  .yh-col {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }

  .yh-cell {
    width: 100%;
    aspect-ratio: 1;
    border-radius: 2px;
    border: none;
    background: color-mix(in srgb, var(--border) 35%, transparent);
    cursor: pointer;
    padding: 0;
    min-width: 8px;
  }

  .yh-cell.empty {
    background: transparent;
    cursor: default;
  }

  .yh-cell:hover {
    outline: 1px solid var(--accent);
  }

  .yh-cell.today {
    outline: 1.5px solid var(--accent);
    outline-offset: -1px;
  }

  .yh-scale {
    display: flex;
    align-items: center;
    gap: 5px;
    margin-top: 8px;
    color: var(--text-dim);
    font-size: 11px;
  }

  .yh-sw {
    width: 11px;
    height: 11px;
    border-radius: 2px;
  }

  .scale {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 12px;
    color: var(--text-dim);
    font-size: 11px;
  }

  .scale .sw {
    width: 12px;
    height: 12px;
    border-radius: 3px;
  }

  .loading-inline {
    margin-left: 12px;
  }
</style>
