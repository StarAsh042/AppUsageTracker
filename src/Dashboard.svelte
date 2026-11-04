<script lang="ts">
  import { listen } from '@tauri-apps/api/event';
  import { api, fmtDuration, localDateKey, type AppDetail, type DashboardData, type LiveStatus, type TrackerSettings } from './lib/api';

  const PALETTE = ['#4f8cff', '#3ecf8e', '#f5a623', '#a78bfa', '#22d3ee', '#fb923c', '#f472b6', '#94a3b8'];

  let { focusDate = null }: { focusDate?: string | null } = $props();

  let from = $state(localDateKey());
  let to = $state(localDateKey());
  let data = $state<DashboardData | null>(null);
  let prevTotal = $state<number | null>(null);
  let loading = $state(false);
  let categories = $state<string[]>([]);
  let menu = $state<{ x: number; y: number; appKey: string; current: string } | null>(null);
  let lastLoad = 0;
  let settings = $state<TrackerSettings | null>(null);
  let detail = $state<AppDetail | null>(null);

  $effect(() => {
    api.getSettings().then((s) => (settings = s));
    const un = listen<TrackerSettings>('app://settings', (e) => (settings = e.payload));
    return () => {
      un.then((f) => f());
    };
  });

  // 效率评分：产出 = 1，中性 = 0.5，分心 = 0（未标记分类不参与）
  const scoreInfo = $derived.by(() => {
    if (!data || !settings) return null;
    const nature = settings.category_nature ?? {};
    let marked = 0;
    let weighted = 0;
    for (const a of data.apps) {
      const n = nature[a.category];
      if (!n || n === 'neutral') continue;
      marked += a.active_seconds;
      if (n === 'productive') weighted += a.active_seconds;
    }
    if (marked < 1) return null;
    return { pct: Math.round((weighted / marked) * 100) };
  });

  const goalPct = $derived.by(() => {
    const goal = settings?.daily_goal_minutes ?? 0;
    if (!goal || !data || from !== to) return null;
    return Math.min(100, Math.round((data.total_active_seconds / (goal * 60)) * 100));
  });

  // 分类环形图
  const CAT_COLORS = PALETTE;

  // 分类 → 颜色（与环形图同序同色），排行 chip 复用
  function catColor(name: string): string {
    if (!data) return PALETTE[0];
    const idx = data.categories.findIndex((c) => c.name === name);
    return idx < 0 ? PALETTE[0] : PALETTE[idx % PALETTE.length];
  }

  // 秒级走表：load 时记录基准，tick 时给当前应用加上进行中的区间
  let baseSeconds = new Map<string, number>();
  let baseTotal = 0;
  const donut = $derived.by(() => {
    if (!data || !data.categories.length) return null;
    const total = data.categories.reduce((s, c) => s + c.seconds, 0) || 1;
    const R = 42;
    const C = 2 * Math.PI * R;
    let acc = 0;
    const segs = data.categories.map((c, i) => {
      const frac = c.seconds / total;
      const dash = Math.max(frac * C - 1.5, 0.6);
      const seg = {
        color: CAT_COLORS[i % CAT_COLORS.length],
        dash: `${dash} ${C - dash}`,
        offset: C * (1 - acc) + C / 4,
        name: c.name,
      };
      acc += frac;
      return seg;
    });
    return { segs, C, R };
  });

  async function openDetail(appKey: string) {
    detail = await api.getAppDetail(appKey, from, to);
  }

  function maxDetailPoints(points: { seconds: number }[]): number {
    return Math.max(1, ...points.map((p) => p.seconds)) * 1.4;
  }

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
      baseSeconds = new Map(data.apps.map((a) => [a.app_key, a.active_seconds]));
      baseTotal = data.total_active_seconds;
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

  // 今天视图下：每秒走表；每 5 秒全量刷新（排序/新应用）
  $effect(() => {
    const un = listen<LiveStatus>('usage://tick', (e) => {
      if (from !== localDateKey() || to !== localDateKey() || !data) return;
      const live = e.payload;
      const curKey = live.current_app?.app_key;
      const liveSec = live.current_seconds;
      const total = baseTotal + (curKey ? liveSec : 0);
      data = {
        ...data,
        total_active_seconds: total,
        apps: data.apps.map((a) =>
          a.app_key === curKey
            ? {
                ...a,
                active_seconds: (baseSeconds.get(a.app_key) ?? a.active_seconds) + liveSec,
                percent: total > 0 ? (((baseSeconds.get(a.app_key) ?? 0) + liveSec) / total) * 100 : 0,
              }
            : { ...a, percent: total > 0 ? ((baseSeconds.get(a.app_key) ?? a.active_seconds) / total) * 100 : 0 },
        ),
      };
      if (Date.now() - lastLoad > 5000) {
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
    // 防止菜单被窗口边缘裁切：估算尺寸后翻转/钳制
    const estH = (categories.length + 2) * 30 + 30;
    const estW = 150;
    let x = e.clientX;
    let y = e.clientY;
    if (y + estH > window.innerHeight - 8) {
      y = Math.max(8, window.innerHeight - estH - 8);
    }
    if (x + estW > window.innerWidth - 8) {
      x = window.innerWidth - estW - 8;
    }
    menu = { x, y, appKey, current };
  }

  async function excludeApp() {
    if (!menu) return;
    const key = menu.appKey;
    const name = data?.apps.find((a) => a.app_key === key)?.display_name ?? key;
    if (!confirm(`排除「${name}」？将停止统计并清除其全部记录。`)) return;
    menu = null;
    await api.excludeApp(key);
    await load();
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
    return Math.max(1, ...points.map((p) => p.seconds)) * 1.4;
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
        {#if goalPct !== null}
          <div class="goal-bar"><span style="width:{goalPct}%"></span></div>
          <div class="delta">每日目标 {settings?.daily_goal_minutes} 分钟 · {goalPct}%</div>
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
      {#if scoreInfo}
        <div class="card">
          <div class="label">效率评分</div>
          <div class="score-row">
            <svg viewBox="0 0 100 100" class="ring">
              <circle cx="50" cy="50" r="42" class="ring-bg"></circle>
              <circle
                cx="50"
                cy="50"
                r="42"
                class="ring-fg"
                style="stroke-dasharray:{(scoreInfo.pct / 100) * 263.9} 263.9"
              ></circle>
            </svg>
            <div>
              <div class="value">{scoreInfo.pct}%</div>
              <div class="delta">按分类性质加权</div>
            </div>
          </div>
        </div>
      {/if}
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
              onclick={() => openDetail(a.app_key)}
              oncontextmenu={(e) => rowMenu(e, a.app_key, a.category)}
              title={a.exe_path}
            >
              <span class="col-rank">{i + 1}</span>
              <span class="col-name">{a.display_name}</span>
              <span class="col-cat"><span class="cat-chip" style="--cc:{catColor(a.category)}">{a.category}</span></span>
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
          {#if donut}
            <div class="donut-wrap">
              <svg viewBox="0 0 100 100" class="donut">
                {#each donut.segs as s (s.name)}
                  <circle
                    cx="50"
                    cy="50"
                    r={donut.R}
                    fill="none"
                    stroke={s.color}
                    stroke-width="13"
                    stroke-dasharray={s.dash}
                    stroke-dashoffset={s.offset}
                  ></circle>
                {/each}
              </svg>
              <div class="donut-legend">
                {#each data.categories as c, i (c.name)}
                  <span class="legend-item">
                    <span class="sw" style="background:{CAT_COLORS[i % CAT_COLORS.length]}"></span>
                    {c.name}
                  </span>
                {/each}
              </div>
            </div>
          {/if}
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
        {#if data.trend.length >= 2}
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
        {/if}
      </div>
    </div>
  {:else}
    <div class="loading">{loading ? '加载中…' : ''}</div>
  {/if}
</div>

{#if detail}
  <div class="modal-mask" onclick={() => (detail = null)}>
    <div class="modal" onclick={(e) => e.stopPropagation()}>
      <div class="modal-head">
        <div>
          <div class="m-name">{detail.display_name}</div>
          <div class="m-meta"><span class="cat-chip">{detail.category}</span> <span class="m-path">{detail.exe_path}</span></div>
        </div>
        <button class="m-close" onclick={() => (detail = null)}>✕</button>
      </div>
      <div class="m-total">区间内活跃 <b>{fmtDuration(detail.total_seconds)}</b></div>
      <div class="m-trend">
        {#each detail.daily as pt (pt.date)}
          <div class="t-col" title="{pt.date} {fmtDuration(pt.seconds)}">
            <div class="t-bar-wrap">
              <div class="t-fill" style="height:{(pt.seconds / maxDetailPoints(detail.daily)) * 100}%"></div>
            </div>
            <div class="t-label">{pt.date.slice(5)}</div>
          </div>
        {:else}
          <div class="empty">该区间无数据</div>
        {/each}
      </div>
      <div class="m-hint">右键排行行可随时改分类</div>
    </div>
  </div>
{/if}

{#if menu}
  <div class="ctx-menu" style="left:{menu.x}px; top:{menu.y}px">
    <div class="ctx-title">设置分类</div>
    {#each categories as c (c)}
      <button class="ctx-item" class:current={c === menu.current} onclick={() => pickCategory(c)}>
        {#if c === menu.current}<span class="check">✓</span>{/if}
        {c}
      </button>
    {/each}
    <div class="ctx-sep"></div>
    <button class="ctx-item danger" onclick={() => excludeApp()}>🚫 排除此应用</button>
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
    background: var(--glass-card, var(--card));
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
    background: var(--glass-card, var(--card));
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

  .side > .panel:first-child {
    flex: 1;
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
    background: color-mix(in srgb, var(--cc, #4f8cff) 16%, transparent);
    color: var(--cc, var(--accent));
    border: 1px solid color-mix(in srgb, var(--cc, #4f8cff) 38%, transparent);
    font-size: 12px;
    border-radius: 6px;
    padding: 2px 8px;
    white-space: nowrap;
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
    width: 52px;
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
    justify-content: center;
    gap: 6px;
    min-height: 0;
  }

  .t-col {
    flex: 0 1 72px;
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

  .goal-bar {
    height: 5px;
    border-radius: 3px;
    background: var(--card2);
    overflow: hidden;
    margin-top: 8px;
  }

  .goal-bar span {
    display: block;
    height: 100%;
    border-radius: 3px;
    background: linear-gradient(90deg, #3ecf8e, #6ee7b7);
  }

  .score-row {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-top: 4px;
  }

  .ring {
    width: 52px;
    height: 52px;
    transform: rotate(-90deg);
  }

  .ring-bg {
    fill: none;
    stroke: var(--card2);
    stroke-width: 9;
  }

  .ring-fg {
    fill: none;
    stroke: var(--ok);
    stroke-width: 9;
    stroke-linecap: round;
  }

  .donut-wrap {
    display: flex;
    align-items: center;
    gap: 16px;
    margin-bottom: 12px;
  }

  .donut {
    width: 92px;
    height: 92px;
    flex: none;
  }

  .donut-legend {
    display: flex;
    flex-direction: column;
    gap: 5px;
    font-size: 12px;
  }

  .legend-item {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }

  .donut-legend .sw {
    width: 9px;
    height: 9px;
    border-radius: 3px;
  }

  .modal-mask {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 200;
  }

  .modal {
    width: min(560px, 88%);
    background: var(--card);
    border: 1px solid var(--border);
    border-radius: 14px;
    padding: 18px;
  }

  .modal-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 12px;
  }

  .m-name {
    font-size: 17px;
    font-weight: 700;
  }

  .m-meta {
    margin-top: 6px;
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .m-path {
    color: var(--text-dim);
    font-size: 11px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 340px;
  }

  .m-close {
    background: transparent;
    border: 1px solid var(--border);
    color: var(--text-dim);
    border-radius: 7px;
    padding: 4px 10px;
    cursor: pointer;
  }

  .m-close:hover {
    color: var(--text);
  }

  .m-total {
    margin: 14px 0 6px;
    color: var(--text-dim);
    font-size: 13px;
  }

  .m-total b {
    color: var(--text);
    font-size: 15px;
  }

  .m-trend {
    display: flex;
    align-items: stretch;
    justify-content: center;
    gap: 5px;
    height: 110px;
  }

  .m-trend .t-col {
    flex: 0 1 72px;
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }

  .m-trend .t-bar-wrap {
    flex: 1;
    display: flex;
    align-items: flex-end;
    background: var(--card2);
    border-radius: 4px;
    overflow: hidden;
  }

  .m-trend .t-fill {
    width: 100%;
    background: linear-gradient(180deg, #4f8cff, #3566cc);
    border-radius: 4px 4px 0 0;
    min-height: 2px;
  }

  .m-trend .t-label {
    font-size: 9px;
    color: var(--text-dim);
    text-align: center;
  }

  .m-hint {
    margin-top: 12px;
    color: var(--text-dim);
    font-size: 11px;
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

  .ctx-sep {
    height: 1px;
    background: var(--border);
    margin: 5px 4px;
  }

  .ctx-item.danger {
    color: var(--danger);
  }
</style>
