<script lang="ts">
  import { api, fmtDuration, localDateKey, type DashboardData } from './lib/api';

  let { onPick = () => {} }: { onPick?: (date: string) => void } = $props();

  let cursor = $state(new Date());
  let trend = $state<Map<string, number>>(new Map());
  let total = $state(0);
  let loading = $state(false);

  const WEEK_CN = ['一', '二', '三', '四', '五', '六', '日'];

  $effect(() => {
    load();
  });

  async function load() {
    loading = true;
    try {
      const y = cursor.getFullYear();
      const m = cursor.getMonth();
      const p = (n: number) => String(n).padStart(2, '0');
      const from = `${y}-${p(m + 1)}-01`;
      const to = `${y}-${p(m + 1)}-${p(new Date(y, m + 1, 0).getDate())}`;
      const data: DashboardData = await api.getDashboard(from, to);
      trend = new Map(data.trend.map((t) => [t.date, t.seconds]));
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

  function keyOf(y: number, m: number, d: number): string {
    const p = (n: number) => String(n).padStart(2, '0');
    return `${y}-${p(m + 1)}-${p(d)}`;
  }

  // 月历网格：周一开头
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
  const todayKey = $derived(localDateKey());

  function heat(seconds: number | undefined): string {
    if (seconds === undefined) return 'transparent';
    const r = seconds / maxSeconds;
    if (r <= 0) return 'transparent';
    return `color-mix(in srgb, var(--accent) ${Math.round(18 + r * 82)}%, transparent)`;
  }

  function pick(d: number) {
    onPick(keyOf(grid.y, grid.m, d));
  }
</script>

<div class="cal">
  <div class="toolbar">
    <button class="nav" onclick={() => shiftMonth(-1)}>◀</button>
    <span class="month">{cursor.getFullYear()} 年 {cursor.getMonth() + 1} 月</span>
    <button class="nav" onclick={() => shiftMonth(1)}>▶</button>
    <span class="total">本月活跃 {fmtDuration(total)}</span>
  </div>

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
            style="background:{heat(seconds)}"
            onclick={() => pick(d)}
            title="{key}{seconds !== undefined ? ' · ' + fmtDuration(seconds) : ' · 无记录'}"
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
      <span>少</span>
      <span class="sw" style="background:color-mix(in srgb, var(--accent) 25%, transparent)"></span>
      <span class="sw" style="background:color-mix(in srgb, var(--accent) 50%, transparent)"></span>
      <span class="sw" style="background:color-mix(in srgb, var(--accent) 75%, transparent)"></span>
      <span class="sw" style="background:var(--accent)"></span>
      <span>多</span>
      {#if loading}<span class="loading-inline">加载中…</span>{/if}
    </div>
  </div>
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
    background: var(--card);
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
