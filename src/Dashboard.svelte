<script lang="ts">
  import { listen } from '@tauri-apps/api/event';
  import {
    api,
    fmtDuration,
    categoryLabel,
    localDateKey,
    CATEGORY_PALETTE,
    type AppDetail,
    type DashboardApp,
    type DashboardData,
    type LiveStatus,
    type TrackerSettings,
  } from './lib/api';
  import { lang, translate } from './lib/i18n';
  import { errText } from './lib/errors';
  import Modal from './lib/Modal.svelte';
  import RankingPanel from './lib/RankingPanel.svelte';
  import CategoryBreakdown from './lib/CategoryBreakdown.svelte';
  import TrendChart, { type TrendBar } from './lib/TrendChart.svelte';

  let {
    focusDate = null,
    onConsume,
  }: {
    focusDate?: string | null;
    onConsume?: () => void;
  } = $props();

  let from = $state(localDateKey());
  let to = $state(localDateKey());
  let data = $state<DashboardData | null>(null);
  let prevTotal = $state<number | null>(null);
  // 首帧就是"加载中"：挂载时 load() 还没跑完，否则会先闪一下"暂无数据"。
  let loading = $state(true);
  // 加载失败与空数据必须显示成可见文案，否则整块内容会看起来"消失了"。
  let loadError = $state<string | null>(null);
  let loadSeq = 0;
  let categories = $state<string[]>([]);
  let menu = $state<{ x: number; y: number; appKey: string; current: string } | null>(null);
  let lastLoad = 0;
  let settings = $state<TrackerSettings | null>(null);
  let detail = $state<AppDetail | null>(null);
  let catColors = $state<Map<string, string>>(new Map());
  let trendMode = $state<'duration' | 'score'>('duration');

  const t = $derived((s: string, params?: Record<string, string | number>) => translate($lang, s, params));
  let aliases = $state<Map<string, string>>(new Map());

  function aliasName(key: string, fallback: string): string {
    return aliases.get(key) || fallback;
  }

  // The Chinese annotation is only useful when the interface itself is Chinese.
  function displayName(a: DashboardApp): string {
    const name = aliasName(a.app_key, a.display_name);
    return $lang === 'zh' && a.zh_name ? `${name}(${a.zh_name})` : name;
  }

  $effect(() => {
    api.getSettings().then((s) => (settings = s));
    api.listCategories().then((cats) => {
      const m = new Map<string, string>();
      for (const c of cats) {
        const top = c.split('/')[0].trim();
        if (top && !m.has(top)) m.set(top, CATEGORY_PALETTE[m.size % CATEGORY_PALETTE.length]);
      }
      catColors = m;
    });
    api.getSettings().then((s) => {
      aliases = new Map(Object.entries(s.app_display_names ?? {}));
    });
    const un = listen<TrackerSettings>('app://settings', (e) => {
      settings = e.payload;
      aliases = new Map(Object.entries(e.payload?.app_display_names ?? {}));
    });
    return () => {
      un.then((f) => f());
    };
  });

  // 分类 → 颜色（按顶层分类稳定映射，排行/环形图/悬浮窗全局一致）
  function catTop(name: string): string {
    return name.split('/')[0].trim();
  }

  function catColor(name: string): string {
    return catColors.get(catTop(name)) ?? CATEGORY_PALETTE[0];
  }

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

  // 秒级走表：load 时记录基准，tick 时给当前应用加上进行中的区间
  let baseSeconds = new Map<string, number>();
  let baseTotal = 0;

  let aliasDraft = $state('');

  // 分类树：顶层（含子分类汇总）+ 子分类行
  const catTree = $derived.by(() => {
    if (!data) return [];
    const tops = new Map<string, number>();
    for (const c of data.categories) {
      const top = c.name.split('/')[0].trim();
      tops.set(top, (tops.get(top) ?? 0) + c.seconds);
    }
    const rows: { name: string; seconds: number; depth: number; percent: number }[] = [];
    const maxTop = Math.max(1, ...tops.values());
    const total = data.total_active_seconds || 1;
    for (const [top, secs] of [...tops.entries()].sort((a, b) => b[1] - a[1])) {
      rows.push({ name: top, seconds: secs, depth: 0, percent: (secs / maxTop) * 100 });
      const kids = data.categories.filter((c) => c.name.startsWith(top + '/')).sort((a, b) => b.seconds - a.seconds);
      for (const k of kids) {
        rows.push({
          name: k.name,
          seconds: k.seconds,
          depth: k.name.split('/').length - 1,
          percent: (k.seconds / total) * 100,
        });
      }
    }
    return rows;
  });

  async function openDetail(appKey: string) {
    detail = await api.getAppDetail(appKey, from, to);
    aliasDraft = aliasName(appKey, detail.display_name);
  }

  async function saveAlias() {
    if (!detail) return;
    await api.setAppDisplayName(detail.app_key, aliasDraft);
    await load();
  }

  // 日历页点选日期 → 聚焦该日，并且只消费一次。
  // 之前这里常驻监听 focusDate，父组件的值永不清空，导致每次重挂载
  // 都把 from/to 钉死在那一天，界面再也回不到今天。
  $effect(() => {
    if (!focusDate) return;
    from = focusDate;
    to = focusDate;
    onConsume?.();
  });

  $effect(() => {
    api.listCategories().then((c) => (categories = c));
  });

  async function load() {
    // 快速切换日期时，只让最后一次请求的结果落地，避免旧响应覆盖新范围。
    const seq = ++loadSeq;
    loading = true;
    try {
      const next = await api.getDashboard(from, to);
      if (seq !== loadSeq) return;
      data = next;
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
        if (seq === loadSeq) prevTotal = prev.total_active_seconds;
      }
      if (seq === loadSeq) loadError = null;
    } catch (e) {
      if (seq === loadSeq) loadError = errText(e);
    } finally {
      if (seq === loadSeq) loading = false;
    }
  }

  const isMultiDay = $derived(from < to);
  const dayCount = $derived(
    from < to
      ? Math.round((new Date(to + 'T00:00:00').getTime() - new Date(from + 'T00:00:00').getTime()) / 86400000) + 1
      : 1,
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

  // 趋势柱。柱子高度留 40% 余量，否则最高那根会顶到面板边缘。
  const trendBars = $derived.by<TrendBar[]>(() => {
    if (!data) return [];
    const max = Math.max(1, ...data.trend.map((p) => p.seconds)) * 1.4;
    const score = trendMode === 'score';
    return data.trend.map((pt) => {
      const val = score ? (pt.score === null ? 0 : pt.score) : pt.seconds;
      const title = score ? (pt.score === null ? t('无评分') : `${t('效率')} ${pt.score}%`) : fmtDuration(pt.seconds);
      return {
        key: pt.date,
        title: `${pt.date} ${title}`,
        label: score ? (pt.score === null ? '·' : `${pt.score}%`) : pt.date.slice(5),
        percent: score ? Math.min(100, val) : (val / max) * 100,
        score,
        tone: score ? (val >= 60 ? 'good' : val < 40 ? 'bad' : undefined) : undefined,
      };
    });
  });

  const detailBars = $derived.by<TrendBar[]>(() => {
    if (!detail) return [];
    const max = Math.max(1, ...detail.daily.map((p) => p.seconds)) * 1.4;
    return detail.daily.map((pt) => ({
      key: pt.date,
      title: `${pt.date} ${fmtDuration(pt.seconds)}`,
      label: pt.date.slice(5),
      percent: (pt.seconds / max) * 100,
    }));
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

  async function exportWeekly() {
    const path = await api.exportWeeklyReport();
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
    if (!confirm(t('「{name}」 — 将停止统计并清除其全部记录', { name }))) return;
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
</script>

<div class="dash">
  <div class="toolbar">
    <div class="ranges">
      <button onclick={() => setRange('today')}>{t('今天')}</button>
      <button onclick={() => setRange('yesterday')}>{t('昨天')}</button>
      <button onclick={() => setRange('week')}>{t('本周')}</button>
      <button onclick={() => setRange('month')}>{t('本月')}</button>
    </div>
    <div class="dates">
      <input type="date" bind:value={from} max={to} />
      <span>{t('至')}</span>
      <input type="date" bind:value={to} min={from} />
    </div>
    <div class="spacer"></div>
    <button class="ghost" onclick={exportCsv}>{t('导出')} CSV</button>
    <button class="ghost" onclick={exportWeekly}>{t('周报')}</button>
  </div>

  {#if data}
    <div class="cards">
      <div class="card">
        <div class="label">{t('总活跃时长')}</div>
        <div class="value">{fmtDuration(data.total_active_seconds)}</div>
        {#if isMultiDay}
          <div class="delta">
            {t('日均 {total}（{days} 天）', { total: fmtDuration(dailyAvg), days: dayCount })}
          </div>
        {/if}
        {#if deltaPct !== null}
          <div class="delta" class:up={deltaPct >= 0} class:down={deltaPct < 0}>
            {t('较上一周期')}
            {deltaPct >= 0 ? '+' : ''}{deltaPct.toFixed(1)}%
          </div>
        {/if}
        {#if goalPct !== null}
          <div class="goal-bar"><span style="width:{goalPct}%"></span></div>
          <div class="delta">{t('每日目标')} {settings?.daily_goal_minutes} {t('分钟')} · {goalPct}%</div>
        {/if}
      </div>
      <div class="card">
        <div class="label">{t('最常用应用')}</div>
        <div class="value">{data.top_app ?? '—'}</div>
      </div>
      {#if scoreInfo}
        <div class="card">
          <div class="label">{t('效率评分')}</div>
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
              <div class="delta">{t('按分类性质加权')}</div>
            </div>
          </div>
        </div>
      {/if}
      <div class="card">
        <div class="label">{t('应用数量')}</div>
        <div class="value">{data.apps.length}</div>
      </div>
    </div>

    <div class="content">
      <RankingPanel apps={data.apps} nameOf={displayName} colorOf={catColor} onopen={openDetail} onmenu={rowMenu} />

      <div class="side">
        <div class="panel">
          <div class="panel-title">{t('分类占比')}</div>
          <CategoryBreakdown tree={catTree} colorOf={catColor} />
        </div>
        {#if data.trend.length >= 2}
          <div class="panel trend">
            <div class="panel-title trend-head">
              <span>{trendMode === 'duration' ? t('每日活跃') : t('效率趋势')}</span>
              <span class="t-switch">
                <button class:on={trendMode === 'duration'} onclick={() => (trendMode = 'duration')}>
                  {t('时长')}
                </button>
                <button class:on={trendMode === 'score'} onclick={() => (trendMode = 'score')}>{t('效率')}</button>
              </span>
            </div>
            <TrendChart bars={trendBars} empty={t('暂无数据')} />
          </div>
        {/if}
      </div>
    </div>
  {:else}
    <div class="loading">
      {#if loading}
        {t('加载中')}…
      {:else if loadError}
        <div class="load-failed">
          <div>{t('加载失败')}</div>
          <div class="err-text">{loadError}</div>
          <button class="ghost" onclick={() => load()}>{t('重试')}</button>
        </div>
      {:else}
        {t('暂无数据')}
      {/if}
    </div>
  {/if}
</div>

{#if detail}
  <!-- 窄化用 @const 落到局部变量：snippet 内部访问外层 detail 时 TS 拿不到非空推断 -->
  {@const d = detail}
  <Modal label={aliasName(d.app_key, d.display_name)} onclose={() => (detail = null)}>
    {#snippet head()}
      <div class="m-name">{aliasName(d.app_key, d.display_name)}</div>
      <div class="m-meta">
        <span class="cat-chip">{categoryLabel(d.category, $lang)}</span> <span class="m-path">{d.exe_path}</span>
      </div>
    {/snippet}
    <div class="alias-row">
      <input class="alias-input" placeholder={t('自定义显示名')} bind:value={aliasDraft} spellcheck="false" />
      <button class="alias-save" onclick={saveAlias}>{t('改名')}</button>
    </div>
    <div class="m-total">{t('区间内活跃')} <b>{fmtDuration(detail.total_seconds)}</b></div>
    <div class="m-trend">
      <TrendChart bars={detailBars} compact empty={t('该区间无数据')} />
    </div>
    <div class="m-hint">{t('右键排行行可随时改分类')}</div>
  </Modal>
{/if}

{#if menu}
  <div class="ctx-menu" style="left:{menu.x}px; top:{menu.y}px">
    <div class="ctx-title">{t('设置分类')}</div>
    {#each categories as c (c)}
      <button class="ctx-item" class:current={c === menu.current} onclick={() => pickCategory(c)}>
        {#if c === menu.current}<span class="check">✓</span>{/if}
        {c}
      </button>
    {/each}
    <div class="ctx-sep"></div>
    <button class="ctx-item danger" onclick={() => excludeApp()}>🚫 {t('排除此应用')}</button>
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
    display: flex;
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
    flex: 1;
    min-width: 0;
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
    overflow-y: auto;
  }

  /* 分类占比吃掉剩余高度，但必须能收缩；内容溢出时自己滚，不能盖到下面的趋势图。 */
  .side > .panel:first-child {
    flex: 1 1 auto;
    min-height: 0;
    overflow: hidden;
  }

  /* 趋势图给定基准高度，否则上面板撑开后会把柱子压成一条线。 */
  .panel.trend {
    flex: 0 1 200px;
    min-height: 140px;
  }

  .panel-title {
    font-weight: 600;
    margin-bottom: 10px;
  }

  .trend-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .t-switch {
    display: inline-flex;
    gap: 4px;
  }

  .t-switch button {
    background: var(--card2);
    border: 1px solid var(--border);
    color: var(--text-dim);
    border-radius: 6px;
    padding: 3px 10px;
    font-size: 11px;
    cursor: pointer;
  }

  .t-switch button.on {
    background: var(--accent);
    border-color: var(--accent);
    color: #fff;
  }

  .loading {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-dim);
  }

  .load-failed {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    max-width: 520px;
    text-align: center;
  }

  .err-text {
    color: var(--text-dim);
    font-size: 12px;
    word-break: break-all;
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

  /* 弹窗外壳（遮罩 / 卡片 / 关闭按钮）在 lib/Modal.svelte 里，
     这里只留弹窗内部内容自己的样式。 */
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

  /* 详情弹窗里的分类标签；排行表里的同名标签已随组件搬走。 */
  .m-meta .cat-chip {
    background: color-mix(in srgb, var(--cc, #4f8cff) 16%, transparent);
    color: var(--cc, var(--accent));
    border: 1px solid color-mix(in srgb, var(--cc, #4f8cff) 38%, transparent);
    font-size: 12px;
    border-radius: 6px;
    padding: 2px 8px;
    white-space: nowrap;
  }

  .m-path {
    color: var(--text-dim);
    font-size: 11px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 340px;
  }

  .alias-row {
    display: flex;
    gap: 8px;
    margin-top: 12px;
  }

  .alias-input {
    flex: 1;
    background: var(--card2);
    border: 1px solid var(--border);
    color: var(--text);
    border-radius: 7px;
    padding: 7px 10px;
    font-size: 13px;
  }

  .alias-save {
    background: var(--accent);
    border: 0;
    color: #fff;
    border-radius: 7px;
    padding: 7px 14px;
    cursor: pointer;
    font-size: 13px;
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
    height: 110px;
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
