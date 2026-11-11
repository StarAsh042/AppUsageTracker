<script lang="ts">
  import { listen } from '@tauri-apps/api/event';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { lang, en } from './lib/i18n';
  import {
    api,
    fmtDuration,
    stateLabel,
    CATEGORY_PALETTE,
    WIDGET_ACCENTS,
    type LiveStatus,
    type TrackerSettings,
    type WidgetPrefs,
  } from './lib/api';

  let live = $state<LiveStatus | null>(null);
  let prefs = $state<WidgetPrefs | null>(null);
  let goalMinutes = $state(0);
  let hovering = $state(false);
  let collapsedUi = $state(false);
  let edge = $state<'none' | 'left' | 'right' | 'top' | 'bottom'>('none');

  const t = $derived((s: string) => ($lang === 'en' ? (en[s] ?? s) : s));
  let catColors = $state<Map<string, string>>(new Map());
  let leaveTimer: ReturnType<typeof setTimeout> | undefined;
  // 未贴边就想收起时，Rust 发 widget://hint = "edge_required"
  let hint = $state('');
  let hintTimer: ReturnType<typeof setTimeout> | undefined;

  function catColor(name: string): string {
    return catColors.get(name.split('/')[0].trim()) ?? '#aeb6c8';
  }

  const collapsed = $derived(prefs?.edge_snap === true && collapsedUi);
  const accent = $derived(WIDGET_ACCENTS[prefs?.accent ?? 'blue'] ?? '#4f8cff');
  const bgAlpha = $derived(((prefs?.opacity ?? 85) / 100).toFixed(2));
  const goalPct = $derived.by(() => {
    if (!goalMinutes || !live) return null;
    return Math.min(100, Math.round((live.today_total_seconds / (goalMinutes * 60)) * 100));
  });

  $effect(() => {
    api.getSettings().then((s) => {
      prefs = s.widget;
      goalMinutes = s.daily_goal_minutes;
      collapsedUi = false;
      lang.set(s.language === 'en' ? 'en' : 'zh');
    });
    api.listCategories().then((cats) => {
      const m = new Map<string, string>();
      for (const c of cats) {
        const top = c.split('/')[0].trim();
        if (top && !m.has(top)) m.set(top, CATEGORY_PALETTE[m.size % CATEGORY_PALETTE.length]);
      }
      catColors = m;
    });
    api.getLiveStatus().then((l) => (live = l));
    const un0 = listen<TrackerSettings>('app://settings', (e) => {
      goalMinutes = e.payload?.daily_goal_minutes ?? 0;
      lang.set(e.payload?.language === 'en' ? 'en' : 'zh');
    });
    const un1 = listen<LiveStatus>('usage://tick', (e) => (live = e.payload));
    const un2 = listen<WidgetPrefs>('widget://prefs', (e) => (prefs = e.payload));
    const un3 = listen<string>('widget://edge', (e) => {
      edge = (e.payload as typeof edge) ?? 'none';
      collapsedUi = edge !== 'none';
    });
    const un4 = listen<string>('widget://hint', (e) => {
      hint = e.payload === 'edge_required' ? t('请先拖到屏幕边缘再离开') : (e.payload ?? '');
      clearTimeout(hintTimer);
      hintTimer = setTimeout(() => (hint = ''), 2600);
    });
    // 禁用 WebView 默认右键菜单（右键由原生菜单接管）
    const prevent = (e: Event) => e.preventDefault();
    window.addEventListener('contextmenu', prevent);
    return () => {
      un0.then((f) => f());
      un1.then((f) => f());
      un2.then((f) => f());
      un3.then((f) => f());
      un4.then((f) => f());
      clearTimeout(hintTimer);
      window.removeEventListener('contextmenu', prevent);
    };
  });

  function openMenu(e: MouseEvent) {
    e.preventDefault();
    api.showWidgetMenu();
  }

  function onEnter() {
    hovering = true;
    clearTimeout(leaveTimer);
    // 展开后的样式等 Rust 动画结束发 widget://edge = "none" 再切，避免内容被压扁
    if (prefs?.edge_snap) api.setWidgetCollapsed(false);
  }

  function onLeave() {
    hovering = false;
    clearTimeout(leaveTimer);
    leaveTimer = setTimeout(() => {
      // 收不收由 Rust 判断：贴边才收起并回 widget://edge，未贴边回 "none" + 提示
      if (!hovering && prefs?.edge_snap) api.setWidgetCollapsed(true);
    }, 260);
  }

  // 手动拖拽（4px 阈值后才启动系统拖动，双击才可被识别）
  let pressT = 0;
  let pressX = 0;
  let pressY = 0;
  let dragging = false;
  let lastTap = 0;

  function onDown(e: MouseEvent) {
    if (e.button !== 0 || prefs?.locked) return;
    pressX = e.clientX;
    pressY = e.clientY;
    pressT = Date.now();
    dragging = false;
  }

  function onMove(e: MouseEvent) {
    if (!pressT || dragging) return;
    if (Math.hypot(e.clientX - pressX, e.clientY - pressY) > 4) {
      dragging = true;
      getCurrentWindow().startDragging();
    }
  }

  function onUp() {
    if (dragging) {
      pressT = 0;
      return;
    }
    const now = Date.now();
    if (now - pressT < 400 && now - lastTap < 400) {
      lastTap = 0;
      api.toggleMainWindow();
    } else {
      lastTap = now;
    }
    pressT = 0;
  }
</script>

<div
  class="widget"
  class:compact={prefs?.compact === true}
  class:collapsed
  class:h-edge={edge === 'top' || edge === 'bottom'}
  class:v-edge={edge === 'left' || edge === 'right'}
  style="--ac:{accent}; --wa:{bgAlpha}"
  onmousedown={onDown}
  onmousemove={onMove}
  onmouseup={onUp}
  oncontextmenu={openMenu}
  onmouseenter={onEnter}
  onmouseleave={onLeave}
>
  {#if collapsed}
    <div class="edge-line" title={t('软件使用时间统计')}></div>
  {:else}
    <div class="row top">
      <span class="dot {live?.state.toLowerCase() ?? ''}"></span>
      <span class="state">{live ? stateLabel(live.state, $lang) : '…'}</span>
      {#if prefs?.locked === true}
        <span class="lock" title={t('位置已锁定')}>🔒</span>
      {/if}
      <span class="today">
        {t('今日')} {live ? fmtDuration(live.today_total_seconds) : '--'}{goalPct !== null ? ` · 🎯${goalPct}%` : ''}
      </span>
      {#if hovering}
        <button
          class="close-btn"
          title={t('关闭悬浮窗')}
          onmousedown={(e) => e.stopPropagation()}
          onclick={(e) => { e.stopPropagation(); api.closeWidget(); }}
        >✕</button>
      {/if}
    </div>

    <div class="current">
      {#if live?.current_app}
        <div class="app-name">{live.current_app.display_name}</div>
        <div class="app-meta">
          {live.current_category} · {t('本轮')} {fmtDuration(live.current_seconds)}
        </div>
      {:else}
        <div class="app-name muted">{t('当前无前台应用')}</div>
      {/if}
    </div>

    {#if prefs?.compact !== true}
      <div class="tops">
        {#each live?.top_apps ?? [] as t, i (t.display_name + i)}
          <div class="top-row">
            <span class="rank">{i + 1}</span>
            <span class="name">{t.display_name}</span>
            <span class="cat" style="color:{catColor(t.category)}">{t.category}</span>
            <span class="dur">{fmtDuration(t.active_seconds)}</span>
          </div>
        {/each}
      </div>
    {/if}
  {/if}

  {#if hint}
    <div class="hint-bar">{hint}</div>
  {/if}
</div>

<style>
  /* 圆角交给 DWM 系统裁切（CSS 圆角与系统半径不一致会露出背板白边） */
  .widget {
    height: 100vh;
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 12px 14px;
    background: rgba(16, 18, 24, calc(var(--wa, 0.85) * 0.72));
    border: 1px solid color-mix(in srgb, var(--ac, #4f8cff) 30%, transparent);
    color: #ecf0f8;
    font-size: 12px;
    overflow: hidden;
    position: relative;
  }

  .widget.compact {
    gap: 4px;
    padding: 8px 10px;
    font-size: 11px;
  }

  .widget.compact .current {
    padding: 4px 8px;
  }

  .widget.compact .app-name {
    font-size: 12px;
  }

  /* 贴边收起：整窗透明，只留一根 4px 渐变细线（横边横线 / 竖边竖线）。
     线始终按 4px 居中绘制：窗口还在收放动画时不会铺满整块蓝面（旧实现 inset:0 + 发光 = 蓝闪）。 */
  .widget.collapsed {
    padding: 0;
    border: none;
    background: transparent;
  }

  .edge-line {
    position: absolute;
    border-radius: 2px;
  }

  .widget.collapsed.h-edge .edge-line {
    left: 0;
    right: 0;
    top: calc(50% - 2px);
    height: 4px;
    background: linear-gradient(
      90deg,
      transparent,
      color-mix(in srgb, var(--ac, #4f8cff) 85%, transparent) 15%,
      color-mix(in srgb, var(--ac, #4f8cff) 85%, transparent) 85%,
      transparent
    );
  }

  .widget.collapsed.v-edge .edge-line {
    top: 0;
    bottom: 0;
    left: calc(50% - 2px);
    width: 4px;
    background: linear-gradient(
      180deg,
      transparent,
      color-mix(in srgb, var(--ac, #4f8cff) 85%, transparent) 15%,
      color-mix(in srgb, var(--ac, #4f8cff) 85%, transparent) 85%,
      transparent
    );
  }

  .row {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--ok);
    flex: none;
  }

  .dot.idle {
    background: var(--warn);
  }

  .dot.locked,
  .dot.suspended,
  .dot.paused {
    background: var(--danger);
  }

  .state {
    font-weight: 600;
  }

  .today {
    margin-left: auto;
    color: #aeb6c8;
  }

  .current {
    padding: 6px 10px;
    background: rgba(255, 255, 255, 0.06);
    border-left: 2px solid var(--ac, #4f8cff);
    border-radius: 10px;
  }

  .app-name {
    font-size: 14px;
    font-weight: 700;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .app-name.muted {
    font-weight: 400;
    color: #8b93a5;
  }

  .app-meta {
    margin-top: 2px;
    color: #aeb6c8;
    font-size: 11px;
  }

  .tops {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-height: 0;
  }

  .top-row {
    display: grid;
    grid-template-columns: 16px 1fr auto auto;
    gap: 6px;
    align-items: center;
    padding: 3px 6px;
    border-radius: 6px;
  }

  .top-row:hover {
    background: rgba(255, 255, 255, 0.05);
  }

  .rank {
    color: var(--ac, #4f8cff);
    font-size: 11px;
    font-weight: 700;
  }

  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .cat {
    font-size: 11px;
  }

  .dur {
    color: #aeb6c8;
    font-variant-numeric: tabular-nums;
    font-size: 11px;
  }


  /* 锁徽标跟在状态文字后面（原来绝对定位压住「今日」） */
  .lock {
    font-size: 10px;
    opacity: 0.75;
    line-height: 1;
  }

  .close-btn {
    flex: none;
    width: 18px;
    height: 18px;
    display: grid;
    place-items: center;
    padding: 0;
    border: none;
    border-radius: 5px;
    background: rgba(255, 255, 255, 0.1);
    color: #cfd6e4;
    font-size: 11px;
    line-height: 1;
    cursor: pointer;
  }

  .close-btn:hover {
    background: rgba(248, 113, 113, 0.35);
    color: #fff;
  }

  .hint-bar {
    position: absolute;
    left: 10px;
    right: 10px;
    bottom: 10px;
    padding: 5px 8px;
    border-radius: 8px;
    background: rgba(245, 166, 35, 0.18);
    border: 1px solid rgba(245, 166, 35, 0.45);
    color: var(--warn);
    font-size: 11px;
    text-align: center;
  }
</style>
