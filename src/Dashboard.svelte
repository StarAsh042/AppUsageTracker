<script lang="ts">
  import { listen } from '@tauri-apps/api/event';
  import { api, fmtDuration, localDateKey, type DashboardData } from './lib/api';

  let { focusDate = null }: { focusDate?: string | null } = $props();

  let from = $state(localDateKey());
  let to = $state(localDateKey());
  let data = $state<DashboardData | null>(null);
  let prevTotal = $state<number | null>(null);
  let loading = $state(false);
  let categories = $state<string[]>([]);
  let menu = $state<{ x: number; y: number; appKey: string; current: string } | null>(null);
  let lastLoad = 0;

  // 日历页点选日期 → 聚焦该日
  $effect(() => {
    if (focusDate) {
      from = focusDate;
      to = focusDate;
    }
  });

  $effect(() => {
    api.listCategories().then((c) => (categories = c));
  });

  async function load() {
    loading = true;
    try {
      data = await api.getDashboard(from, to);
      // 多日范围：取上一周期做环比
      prevTotal = null;
      if (from < to) {
        const dayMs = 86400000;
        const f = new Date(from + 'T00:00:00').getTime();
        const t = new Date(to + 'T00:00:00').getTime();
        const days = Math.round((t - f) / dayMs) + 1;
        const pf = new Date(f - days * dayMs);
        const pt = new Date(f - dayMs);
        const p = (n: number) => String(n).padStart(2, '0');
        const prevFrom = `${pf.getFullYear()}-${p(pf.getMonth() + 1)}-${p(pf.getDate())}`;
        const prevTo = `${pt.getFullYear()}-${p(pt.getMonth() + 1)}-${p(pt.getDate())}`;
        const prev = await api.getDashboard(prevFrom, prevTo);
        prevTotal = prev.total_active_seconds;
      }
    } finally {
      loading = false;
    }
  }

  const isMultiDay = $derived(from < to);
  const dayCount = $derived(
    from < to ? Math.round((new Date(to + 'T00:00:00').getTime() - new Date(from + 'T00:00:00').getTime()) / 86400000) + 1 : 1
  );
  const dailyAvg = $derived(data ? data.total_active_seconds / dayCount : 0);
  const deltaPct = $derived.by(() => {
    if (!data || prevTotal === null || prevTotal <= 0) return null;
    return ((data.total_active_seconds - prevTotal) / prevTotal) * 100;
  });

  // 范围变化时重新加载
  $effect(() => {
    void from;
    void to;
    load();
  });

  // 今天视图下，每 5 秒随 tick 刷新
  $effect(() => {
    const un = listen('usage://tick', () => {
      if (from === localDateKey() && to === localDateKey() && Date.now() - lastLoad > 5000) {
        lastLoad = Date.now();
        load();
      }
    });
    return () => {
      un.then((f) => f());
    };
  });

  function setRange(kind: 'today' | 'yesterday' | 'week' | 'month') {
    const d = new Date();
    if (kind === 'today') {
      from = localDateKey();
      to = localDateKey();
    } else if (kind === 'yesterday') {
      from = localDateKey(-1);
      to = localDateKey(-1);
    } else if (kind === 'week') {
      const dow = (d.getDay() + 6) % 7;
      const s = new Date(d);
      s.setDate(d.getDate() - dow);
      const p = (n: number) => String(n).padStart(2, '0');
      from = `${s.getFullYear()}-${p(s.getMonth() + 1)}-${p(s.getDate())}`;
      to = localDateKey();
    } else {
      const p = (n: number) => String(n).padStart(2, '0');
      from = `${d.getFullYear()}-${p(d.getMonth() + 1)}-01`;
      to = localDateKey();
    }
  }

  async function exportCsv() {
    const path = await api.exportCsv(from, to);
    await api.openPath(path);
  }

  function rowMenu(e: MouseEvent, appKey: string, current: string) {
    e.preventDefault();
    menu = { x: e.clientX, y: e.clientY, appKey, current };
  }

  $effect(() => {
    if (!menu) return;
    const close = () => (menu = null);
    window.addEventListener('click', close);
    window.addEventListener('blur', close);
    return () => {
      window.removeEventListener('click', close);
      window.removeEventListener('blur', close);
    };
  });

  async function pickCategory(cat: string) {
    if (!menu) return;
    const m = menu;
    menu = null;
    await api.setAppCategory(m.appKey, cat);
    await load();
  }

  function trendMax(points: { seconds: number }[]): number {
    return Math.max(1, ...points.map((p) => p.seconds)) * 1.08;
  }
</script>

<div class="dash">
  <div class="toolbar">
    <div class="ranges">
      <button onclick={() => setRange('today')}>今天</button>
      <button onclick={() => setRange('yesterday')}>昨天</button>
      <button onclick={() => setRange('week')}>本周</button>
      <button onclick={() => setRange('month')}>本月</button>
    </div>
    <div class="dates">
      <input type="date" bind:value={from} max={to} />
      <span>至</span>
      <input type="date" bind:value={to} min={from} />
    </div>
    <div class="spacer"></div>
    <button class="ghost" onclick={exportCsv}>导出 CSV</button>
  </div>

  {#if data}
    <div class="cards">
      <div class="card">
        <div class="label">总活跃时长</div>
        <div class="value">{fmtDuration(data.total_active_seconds)}</div>
        {#if deltaPct !== null}
          <div class="delta" class:up={deltaPct >= 0} class:down={deltaPct < 0}>
            较上一周期 {deltaPct >= 0 ? '+' : ''}{deltaPct.toFixed(1)}%
          </div>
        {/if}
      </div>
      {#if isMultiDay}
        <div class="card">
          <div class="label">日均活跃（{dayCount} 天）</div>
          <div class="value">{fmtDuration(dailyAvg)}</div>
        </div>
      {/if}
      <div class="card">
        <div class="label">最常用应用</div>
        <div class="value">{data.top_app ?? '—'}</div>
      </div>
      <div class="card">
        <div class="label">应用数量</div>
        <div class="value">{data.apps.length}</div>
      </div>
      {#if !isMultiDay}
        <div class="card">
          <div class="label">日期范围</div>
          <div class="value small">{data.from} ~ {data.to}</div>
        </div>
      {/if}
    </div>

    <div class="content">
      <div class="panel ranking">
        <div class="panel-title">应用排行</div>
        <div class="table-head">
          <span class="col-rank">#</span>
          <span class="col-name">应用</span>
          <span class="col-cat">分类</span>
          <span class="col-dur">活跃时长</span>
          <span class="col-bar">占比</span>
        </div>
        <div class="table-body">
          {#each data.apps as a, i (a.app_key)}
            <div
              class="row"
              oncontextmenu={(e) => rowMenu(e, a.app_key, a.category)}
              title={a.exe_path}
            >
              <span class="col-rank">{i + 1}</span>
              <span class="col-name">{a.display_name}</span>
              <span class="col-cat"><span class="cat-chip">{a.category}</span></span>
              <span class="col-dur">{fmtDuration(a.active_seconds)}</span>
              <span class="col-bar">
                <span class="bar"><span class="fill" style="width:{Math.min(100, a.percent)}%"></span></span>
                <span class="pct">{a.percent.toFixed(1)}%</span>
              </span>
            </div>
          {:else}
            <div class="empty">该时间段暂无数据</div>
          {/each}
        </div>
      </div>

      <div class="side">
        <div class="panel">
          <div class="panel-title">分类占比</div>
          <div class="cats">
            {#each data.categories as c (c.name)}
              <div class="cat-row">
                <span class="cat-name">{c.name}</span>
                <span class="cat-bar"><span class="fill" style="width:{c.percent}%"></span></span>
                <span class="cat-dur">{fmtDuration(c.seconds)}</span>
              </div>
            {:else}
              <div class="empty">暂无数据</div>
            {/each}
          </div>
        </div>
        <div class="panel trend">
          <div class="panel-title">每日活跃</div>
          <div class="trend">
            {#each data.trend as t (t.date)}
              <div class="t-col" title="{t.date} {fmtDuration(t.seconds)}">
                <div class="t-bar-wrap">
                  <div class="t-fill" style="height:{(t.seconds / trendMax(data.trend)) * 100}%"></div>
                </div>
                <div class="t-label">{t.date.slice(5)}</div>
              </div>
            {:else}
              <div class="empty">暂无数据</div>
            {/each}
          </div>
        </div>
      </div>
    </div>
  {:else}
    <div class="loading">{loading ? '加载中…' : ''}</div>
  {/if}
</div>

{#if menu}
  <div class="ctx-menu" style="left:{menu.x}px; top:{menu.y}px">
    <div class="ctx-title">设置分类</div>
    {#each categories as c (c)}
      <button class="ctx-item" class:current={c === menu.current} onclick={() => pickCategory(c)}>
        {#if c === menu.current}<span class="check">✓</span>{/if}
        {c}
      </button>
    {/each}
  </div>
{/if}

<style>
  .dash {
    height: 100%;
    display: flex;
    flex-direction: column;
    padding: 14px 18px;
    gap: 14px;
    overflow: hidden;
  }

  .toolbar {
    display: flex;
    align-items: center;
    gap: 14px;
  }

  .ranges {
    display: flex;
    gap: 6px;
  }

  .ranges button {
    background: var(--card);
    border: 1px solid var(--border);
    color: var(--text-dim);
    border-radius: 7px;
    padding: 6px 14px;
    cursor: pointer;
    font-size: 13px;
  }

  .ranges button:hover {
    color: var(--text);
    border-color: var(--accent);
  }

  .dates {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--text-dim);
    font-size: 13px;
  }

  .dates input {
    background: var(--card);
    border: 1px solid var(--border);
    color: var(--text);
    border-radius: 7px;
    padding: 5px 10px;
    font-size: 13px;
  }

  .spacer {
    flex: 1;
  }

  .ghost {
    background: transparent;
    border: 1px solid var(--border);
    color: var(--text-dim);
    border-radius: 7px;
    padding: 6px 14px;
    cursor: pointer;
    font-size: 13px;
  }

  .ghost:hover {
    color: var(--text);
    border-color: var(--accent);
  }

  .cards {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 12px;
  }

  .delta {
    font-size: 11px;
    margin-top: 2px;
  }

  .delta.up {
    color: var(--warn);
  }

  .delta.down {
    color: var(--ok);
  }

  .card {
    background: var(--card);
    border: 1px solid var(--border);
    border-radius: 12px;
    padding: 14px 16px;
  }

  .card .label {
    color: var(--text-dim);
    font-size: 12px;
  }

  .card .value {
    font-size: 22px;
    font-weight: 700;
    margin-top: 6px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .card .value.small {
    font-size: 16px;
    line-height: 26px;
  }

  .content {
    flex: 1;
    display: grid;
    grid-template-columns: 1fr 320px;
    gap: 14px;
    min-height: 0;
  }

  .panel {
    background: var(--card);
    border: 1px solid var(--border);
    border-radius: 12px;
    padding: 14px;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }

  .side {
    display: flex;
    flex-direction: column;
    gap: 14px;
    min-height: 0;
  }

  .panel-title {
    font-weight: 600;
    margin-bottom: 10px;
  }

  .table-head,
  .row {
    display: grid;
    grid-template-columns: 36px 1fr 88px 96px 1fr;
    gap: 8px;
    align-items: center;
    padding: 7px 6px;
  }

  .table-head {
    color: var(--text-dim);
    font-size: 12px;
    border-bottom: 1px solid var(--border);
  }

  .table-body {
    flex: 1;
    overflow-y: auto;
  }

  .row {
    border-radius: 8px;
    cursor: context-menu;
  }

  .row:hover {
    background: var(--card2);
  }

  .col-rank {
    color: var(--text-dim);
    font-size: 12px;
  }

  .col-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .cat-chip {
    background: rgba(79, 140, 255, 0.14);
    color: var(--accent);
    font-size: 12px;
    border-radius: 6px;
    padding: 2px 8px;
  }

  .col-dur {
    text-align: right;
    font-variant-numeric: tabular-nums;
  }

  .col-bar {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .bar {
    flex: 1;
    height: 6px;
    border-radius: 3px;
    background: var(--card2);
    overflow: hidden;
  }

  .fill {
    display: block;
    height: 100%;
    border-radius: 3px;
    background: linear-gradient(90deg, #4f8cff, #7aa8ff);
  }

  .pct {
    color: var(--text-dim);
    font-size: 12px;
    width: 48px;
    text-align: right;
    font-variant-numeric: tabular-nums;
  }

  .cats {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .cat-row {
    display: grid;
    grid-template-columns: 64px 1fr 84px;
    gap: 8px;
    align-items: center;
    font-size: 13px;
  }

  .cat-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .cat-bar {
    height: 6px;
    border-radius: 3px;
    background: var(--card2);
    overflow: hidden;
  }

  .cat-bar .fill {
    background: linear-gradient(90deg, #3ecf8e, #6ee7b7);
  }

  .cat-dur {
    text-align: right;
    color: var(--text-dim);
    font-size: 12px;
    font-variant-numeric: tabular-nums;
  }

  .trend {
    flex: 1;
    display: flex;
    align-items: stretch;
    gap: 6px;
    min-height: 0;
  }

  .t-col {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
  }

  .t-bar-wrap {
    flex: 1;
    display: flex;
    align-items: flex-end;
    background: var(--card2);
    border-radius: 4px;
    overflow: hidden;
  }

  .t-fill {
    width: 100%;
    background: linear-gradient(180deg, #4f8cff, #3566cc);
    border-radius: 6px 6px 0 0;
    min-height: 2px;
    box-shadow: 0 0 12px rgba(79, 140, 255, 0.35);
  }

  .t-label {
    font-size: 10px;
    color: var(--text-dim);
    text-align: center;
    overflow: hidden;
    white-space: nowrap;
  }

  .empty {
    color: var(--text-dim);
    padding: 18px 0;
    text-align: center;
    font-size: 13px;
  }

  .loading {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-dim);
  }

  .ctx-menu {
    position: fixed;
    z-index: 100;
    background: var(--card2);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 6px;
    min-width: 140px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.45);
  }

  .ctx-title {
    font-size: 11px;
    color: var(--text-dim);
    padding: 4px 10px;
  }

  .ctx-item {
    display: block;
    width: 100%;
    text-align: left;
    background: transparent;
    border: 0;
    color: var(--text);
    padding: 6px 10px;
    border-radius: 6px;
    cursor: pointer;
    font-size: 13px;
  }

  .ctx-item:hover {
    background: rgba(79, 140, 255, 0.18);
  }

  .ctx-item.current {
    color: var(--accent);
  }

  .check {
    margin-right: 4px;
  }
</style>
