<script lang="ts">
  import { api, fmtDuration, localDateKey, CATEGORY_PALETTE, type DashboardData } from './lib/api';
  import { lang, en } from './lib/i18n';

  let aFrom = $state(localDateKey(-6));
  let aTo = $state(localDateKey());
  let bFrom = $state(localDateKey(-13));
  let bTo = $state(localDateKey(-7));
  let aData = $state<DashboardData | null>(null);
  let bData = $state<DashboardData | null>(null);
  let loading = $state(false);
  let aliases = $state<Map<string, string>>(new Map());

  const t = $derived((s: string) => ($lang === 'en' ? (en[s] ?? s) : s));

  $effect(() => {
    api.getSettings().then((s) => {
      aliases = new Map(Object.entries(s.app_display_names ?? {}));
    });
    load();
  });

  async function load() {
    loading = true;
    try {
      [aData, bData] = await Promise.all([
        api.getDashboard(aFrom, aTo),
        api.getDashboard(bFrom, bTo),
      ]);
    } finally {
      loading = false;
    }
  }

  function swap() {
    [aFrom, bFrom] = [bFrom, aFrom];
    [aTo, bTo] = [bTo, aTo];
    load();
  }

  function preset(kind: 'week' | 'month' | 'yesterday') {
    const p = (n: number) => String(n).padStart(2, '0');
    const fmt = (d: Date) => `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
    if (kind === 'yesterday') {
      aFrom = localDateKey(-1);
      aTo = localDateKey(-1);
      bFrom = localDateKey(-2);
      bTo = localDateKey(-2);
    } else if (kind === 'week') {
      const d = new Date();
      const dow = (d.getDay() + 6) % 7;
      const mon = new Date(d);
      mon.setDate(d.getDate() - dow);
      const lastMon = new Date(mon);
      lastMon.setDate(mon.getDate() - 7);
      const lastSun = new Date(mon);
      lastSun.setDate(mon.getDate() - 1);
      aFrom = fmt(mon);
      aTo = localDateKey();
      bFrom = fmt(lastMon);
      bTo = fmt(lastSun);
    } else {
      const d = new Date();
      aFrom = fmt(new Date(d.getFullYear(), d.getMonth(), 1));
      aTo = localDateKey();
      bFrom = fmt(new Date(d.getFullYear(), d.getMonth() - 1, 1));
      bTo = fmt(new Date(d.getFullYear(), d.getMonth(), 0));
    }
    load();
  }

  interface CmpRow {
    key: string;
    name: string;
    cat: string;
    a: number;
    b: number;
  }

  const appRows = $derived.by(() => {
    if (!aData || !bData) return [];
    const m = new Map<string, CmpRow>();
    for (const a of aData.apps) {
      m.set(a.app_key, {
        key: a.app_key,
        name: aliases.get(a.app_key) || a.display_name,
        cat: a.category,
        a: a.active_seconds,
        b: 0,
      });
    }
    for (const b of bData.apps) {
      const e = m.get(b.app_key);
      if (e) {
        e.b = b.active_seconds;
      } else {
        m.set(b.app_key, {
          key: b.app_key,
          name: aliases.get(b.app_key) || b.display_name,
          cat: b.category,
          a: 0,
          b: b.active_seconds,
        });
      }
    }
    return [...m.values()]
      .sort((x, y) => Math.max(y.a, y.b) - Math.max(x.a, x.b))
      .slice(0, 40);
  });

  const catRows = $derived.by(() => {
    if (!aData || !bData) return [];
    const m = new Map<string, CmpRow>();
    for (const c of aData.categories) {
      m.set(c.name, { key: c.name, name: c.name, cat: c.name, a: c.seconds, b: 0 });
    }
    for (const c of bData.categories) {
      const e = m.get(c.name);
      if (e) e.b = c.seconds;
      else m.set(c.name, { key: c.name, name: c.name, cat: c.name, a: 0, b: c.seconds });
    }
    return [...m.values()].sort((x, y) => Math.max(y.a, y.b) - Math.max(x.a, x.b));
  });

  function catColor(name: string): string {
    const top = name.split('/')[0].trim();
    const idx = catRows.findIndex((c) => c.name.split('/')[0].trim() === top);
    return CATEGORY_PALETTE[(idx < 0 ? 0 : idx) % CATEGORY_PALETTE.length];
  }

  function deltaPct(a: number, b: number): number | null {
    if (a <= 0) return null;
    return ((b - a) / a) * 100;
  }

  function deltaText(a: number, b: number): string {
    const d = deltaPct(a, b);
    return d === null ? '新增' : `${d >= 0 ? '+' : ''}${d.toFixed(0)}%`;
  }

  function deltaCls(a: number, b: number): string {
    const d = deltaPct(a, b);
    return d === null ? '' : d >= 0 ? 'up' : 'down';
  }

  function maxOf(rows: CmpRow[]): number {
    return Math.max(1, ...rows.map((r) => Math.max(r.a, r.b)));
  }

  const appMax = $derived(maxOf(appRows));
  const catMax = $derived(maxOf(catRows));
</script>

<div class="cmp">
  <div class="toolbar">
    <span class="tag a">A</span>
    <input type="date" bind:value={aFrom} max={aTo} />
    <span class="tilde">{t('至')}</span>
    <input type="date" bind:value={aTo} min={aFrom} />
    <span class="tag b">B</span>
    <input type="date" bind:value={bFrom} max={bTo} />
    <span class="tilde">至</span>
    <input type="date" bind:value={bTo} min={bFrom} />
    <button class="ghost" onclick={swap}>⇄ {t('交换')}</button>
    <div class="spacer"></div>
    <button class="ghost" onclick={() => preset('yesterday')}>{t('昨天')} vs {t('前日')}</button>
    <button class="ghost" onclick={() => preset('week')}>{t('本周')} vs {t('上周')}</button>
    <button class="ghost" onclick={() => preset('month')}>{t('本月')} vs {t('上月')}</button>
  </div>

  {#if aData && bData}
    <div class="cards">
      <div class="card">
        <div class="label"><span class="tag a">A</span> {aData.from} ~ {aData.to}</div>
        <div class="value">{fmtDuration(aData.total_active_seconds)}</div>
      </div>
      <div class="card">
        <div class="label"><span class="tag b">B</span> {bData.from} ~ {bData.to}</div>
        <div class="value">{fmtDuration(bData.total_active_seconds)}</div>
      </div>
      <div class="card">
        <div class="label">{t('变化')}</div>
        <div
          class="value {deltaCls(aData.total_active_seconds, bData.total_active_seconds)}"
        >
          {deltaPct(aData.total_active_seconds, bData.total_active_seconds) === null
            ? '—'
            : deltaText(aData.total_active_seconds, bData.total_active_seconds)}
        </div>
        <div class="hint2">B vs A</div>
      </div>
    </div>

    <div class="panel">
      <div class="panel-title">{t('应用对比')}</div>
      <div class="head">
        <span class="c-name">应用</span>
        <span class="c-bars">A vs B（{t('时长')}）</span>
      </div>
      <div class="rows">
        {#each appRows as r (r.key)}
          {@const d = deltaPct(r.a, r.b)}
          <div class="row" title={r.key}>
            <span class="c-name">{r.name}<span class="c-cat">{r.cat}</span></span>
            <span class="c-bars">
              <span class="duo">
                <span class="bar a" style="width:{(r.a / appMax) * 100}%"><i>{fmtDuration(r.a)}</i></span>
                <span class="bar b" style="width:{(r.b / appMax) * 100}%"><i>{fmtDuration(r.b)}</i></span>
              </span>
              <span class="c-delta {deltaCls(r.a, r.b)}">{deltaText(r.a, r.b)}</span>
            </span>
          </div>
        {:else}
          <div class="empty">{t('暂无数据')}</div>
        {/each}
      </div>
    </div>

    <div class="panel">
      <div class="panel-title">{t('分类对比')}</div>
      <div class="rows">
        {#each catRows as r (r.key)}
          {@const d = deltaPct(r.a, r.b)}
          <div class="row">
            <span class="c-name">
              <span class="sw" style="background:{catColor(r.name)}"></span>{r.name}
            </span>
            <span class="c-bars">
              <span class="duo">
                <span class="bar a" style="width:{(r.a / catMax) * 100}%"><i>{fmtDuration(r.a)}</i></span>
                <span class="bar b" style="width:{(r.b / catMax) * 100}%"><i>{fmtDuration(r.b)}</i></span>
              </span>
              <span class="c-delta {deltaCls(r.a, r.b)}">{deltaText(r.a, r.b)}</span>
            </span>
          </div>
        {:else}
          <div class="empty">暂无数据</div>
        {/each}
      </div>
    </div>
  {:else}
    <div class="loading">{loading ? t('加载中') + '…' : ''}</div>
  {/if}
</div>

<style>
  .cmp {
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
    gap: 8px;
    flex-wrap: wrap;
  }

  .tag {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    border-radius: 6px;
    font-size: 12px;
    font-weight: 700;
    color: #fff;
    flex: none;
  }

  .tag.a {
    background: #565d6e;
  }

  .tag.b {
    background: var(--accent);
  }

  .tilde {
    color: var(--text-dim);
    font-size: 12px;
  }

  input[type='date'] {
    background: var(--card);
    border: 1px solid var(--border);
    color: var(--text);
    border-radius: 7px;
    padding: 5px 8px;
    font-size: 12px;
  }

  .ghost {
    background: transparent;
    border: 1px solid var(--border);
    color: var(--text-dim);
    border-radius: 7px;
    padding: 6px 12px;
    cursor: pointer;
    font-size: 12px;
  }

  .ghost:hover {
    color: var(--text);
    border-color: var(--accent);
  }

  .spacer {
    flex: 1;
  }

  .cards {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 12px;
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
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .card .value {
    font-size: 22px;
    font-weight: 700;
    margin-top: 6px;
  }

  .card .value.up {
    color: var(--warn);
  }

  .card .value.down {
    color: var(--ok);
  }

  .hint2 {
    color: var(--text-dim);
    font-size: 11px;
    margin-top: 2px;
  }

  .panel {
    background: var(--glass-card, var(--card));
    border: 1px solid var(--border);
    border-radius: 12px;
    padding: 14px 16px;
  }

  .panel-title {
    font-weight: 600;
    margin-bottom: 10px;
  }

  .head {
    color: var(--text-dim);
    font-size: 11px;
    display: grid;
    grid-template-columns: 220px 1fr;
    gap: 12px;
    padding-bottom: 6px;
    border-bottom: 1px solid var(--border);
  }

  .rows {
    display: flex;
    flex-direction: column;
  }

  .row {
    display: grid;
    grid-template-columns: 220px 1fr;
    gap: 12px;
    align-items: center;
    padding: 6px 4px;
    border-radius: 8px;
  }

  .row:hover {
    background: var(--card2);
  }

  .c-name {
    font-size: 13px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .c-cat {
    color: var(--text-dim);
    font-size: 11px;
  }

  .sw {
    width: 9px;
    height: 9px;
    border-radius: 3px;
    display: inline-block;
    flex: none;
  }

  .c-bars {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
  }

  .duo {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }

  .bar {
    height: 12px;
    border-radius: 3px;
    min-width: 2px;
    display: flex;
    align-items: center;
    overflow: hidden;
  }

  .bar i {
    font-style: normal;
    font-size: 10px;
    color: rgba(255, 255, 255, 0.85);
    padding-left: 6px;
    white-space: nowrap;
  }

  .bar.a {
    background: #565d6e;
  }

  .bar.b {
    background: linear-gradient(90deg, #4f8cff, #7aa8ff);
  }

  .c-delta {
    width: 52px;
    text-align: right;
    font-size: 12px;
    font-variant-numeric: tabular-nums;
    flex: none;
  }

  .c-delta.up {
    color: var(--warn);
  }

  .c-delta.down {
    color: var(--ok);
  }

  .empty {
    color: var(--text-dim);
    padding: 16px 0;
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
</style>
