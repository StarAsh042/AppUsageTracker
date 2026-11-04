<script lang="ts">
  import { listen } from '@tauri-apps/api/event';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import {
    api,
    fmtDuration,
    stateLabel,
    WIDGET_ACCENTS,
    type LiveStatus,
    type WidgetPrefs,
  } from './lib/api';

  let live = $state<LiveStatus | null>(null);
  let prefs = $state<WidgetPrefs | null>(null);
  let hovering = $state(false);

  const collapsed = $derived(prefs?.edge_snap === true && !hovering);
  const accent = $derived(WIDGET_ACCENTS[prefs?.accent ?? 'blue'] ?? '#4f8cff');
  const bgAlpha = $derived(((prefs?.opacity ?? 85) / 100).toFixed(2));

  $effect(() => {
    api.getSettings().then((s) => (prefs = s.widget));
    api.getLiveStatus().then((l) => (live = l));
    const un1 = listen<LiveStatus>('usage://tick', (e) => (live = e.payload));
    const un2 = listen<WidgetPrefs>('widget://prefs', (e) => (prefs = e.payload));
    // 禁用 WebView 默认右键菜单（右键由原生菜单接管）
    const prevent = (e: Event) => e.preventDefault();
    window.addEventListener('contextmenu', prevent);
    return () => {
      un1.then((f) => f());
      un2.then((f) => f());
      window.removeEventListener('contextmenu', prevent);
    };
  });

  function openMenu(e: MouseEvent) {
    e.preventDefault();
    api.showWidgetMenu();
  }

  function onEnter() {
    hovering = true;
    if (prefs?.edge_snap) api.setWidgetCollapsed(false);
  }

  function onLeave() {
    hovering = false;
    if (prefs?.edge_snap) api.setWidgetCollapsed(true);
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
  style="--ac:{accent}; --wa:{bgAlpha}"
  onmousedown={onDown}
  onmousemove={onMove}
  onmouseup={onUp}
  oncontextmenu={openMenu}
  onmouseenter={onEnter}
  onmouseleave={onLeave}
>
  {#if collapsed}
    <div class="pill" title="使用时长悬浮窗">
      <span class="pill-icon">⏱</span>
      <span class="pill-text">
        {live ? fmtDuration(live.today_total_seconds) : '--'}
      </span>
    </div>
  {:else}
    <div class="row top">
      <span class="dot {live?.state.toLowerCase() ?? ''}"></span>
      <span class="state">{live ? stateLabel(live.state) : '…'}</span>
      <span class="today">
        今日 {live ? fmtDuration(live.today_total_seconds) : '--'}
      </span>
    </div>

    <div class="current">
      {#if live?.current_app}
        <div class="app-name">{live.current_app.display_name}</div>
        <div class="app-meta">
          {live.current_category} · 本轮 {fmtDuration(live.current_seconds)}
        </div>
      {:else}
        <div class="app-name muted">当前无前台应用</div>
      {/if}
    </div>

    {#if prefs?.compact !== true}
      <div class="tops">
        {#each live?.top_apps ?? [] as t, i (t.display_name + i)}
          <div class="top-row">
            <span class="rank">{i + 1}</span>
            <span class="name">{t.display_name}</span>
            <span class="cat">{t.category}</span>
            <span class="dur">{fmtDuration(t.active_seconds)}</span>
          </div>
        {/each}
      </div>
    {/if}
  {/if}

  {#if prefs?.locked === true}
    <span class="lock-badge">🔒</span>
  {/if}
</div>

<style>
  .widget {
    height: 100vh;
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 12px 14px;
    border-radius: 14px;
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

  .widget.collapsed {
    padding: 5px;
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
    color: var(--ac, #4f8cff);
    font-size: 11px;
  }

  .dur {
    color: #aeb6c8;
    font-variant-numeric: tabular-nums;
    font-size: 11px;
  }

  .pill {
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 6px;
  }

  .pill-icon {
    font-size: 15px;
  }

  .pill-text {
    writing-mode: vertical-rl;
    text-orientation: upright;
    font-size: 11px;
    color: #aeb6c8;
    font-variant-numeric: tabular-nums;
    letter-spacing: 1px;
    max-height: 100px;
    overflow: hidden;
  }

  .lock-badge {
    position: absolute;
    top: 6px;
    right: 8px;
    font-size: 10px;
    opacity: 0.75;
    pointer-events: none;
  }
</style>
