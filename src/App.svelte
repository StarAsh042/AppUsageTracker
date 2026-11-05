<script lang="ts">
  import { listen } from '@tauri-apps/api/event';
  import Dashboard from './Dashboard.svelte';
  import TimelineView from './TimelineView.svelte';
  import CalendarView from './CalendarView.svelte';
  import CompareView from './CompareView.svelte';
  import SettingsView from './SettingsView.svelte';
  import { api, fmtDuration, stateLabel, type LiveStatus } from './lib/api';

  let view = $state<'dashboard' | 'timeline' | 'calendar' | 'compare' | 'settings'>('dashboard');
  let live = $state<LiveStatus | null>(null);
  let version = $state('');
  let paused = $state(false);
  let focusDate = $state<string | null>(null);
  let restTip = $state(false);
  let restTimer: ReturnType<typeof setTimeout> | undefined;

  $effect(() => {
    api.getVersion().then((v) => (version = v));
    api.getSettings().then((s) => applyTheme(s.theme));
    const un1 = listen<LiveStatus>('usage://tick', (e) => {
      live = e.payload;
      paused = e.payload.state === 'Paused';
    });
    const un2 = listen<string>('ui://navigate', (e) => {
      view = (e.payload as typeof view) ?? 'dashboard';
    });
    const un3 = listen<{ theme?: string }>('app://settings', (e) => applyTheme(e.payload?.theme));
    const un4 = listen('ui://rest', () => {
      restTip = true;
      clearTimeout(restTimer);
      restTimer = setTimeout(() => (restTip = false), 30_000);
    });
    // 禁用 WebView 默认右键菜单（排行右键分类等自定义菜单不受影响）
    const prevent = (e: Event) => e.preventDefault();
    window.addEventListener('contextmenu', prevent);
    return () => {
      un1.then((f) => f());
      un2.then((f) => f());
      un3.then((f) => f());
      un4.then((f) => f());
      window.removeEventListener('contextmenu', prevent);
    };
  });

  function applyTheme(theme?: string) {
    document.documentElement.classList.toggle('theme-light', theme === 'light');
  }

  function onPickDate(date: string) {
    focusDate = date;
    view = 'dashboard';
  }

  async function togglePause() {
    paused = !paused;
    await api.setPaused(paused);
  }
</script>

<div class="app">
  <header>
    <div class="brand">
      <span class="logo">⏱</span>
      <span>软件使用时间统计</span>
    </div>
    <div class="live">
      {#if live}
        <span class="dot {live.state.toLowerCase()}"></span>
        <span>{stateLabel(live.state)}</span>
        <span class="sep">·</span>
        <span>今日 {fmtDuration(live.today_total_seconds)}</span>
      {/if}
    </div>
    <nav>
      <button class:active={view === 'dashboard'} onclick={() => (view = 'dashboard')}>仪表盘</button>
      <button class:active={view === 'timeline'} onclick={() => (view = 'timeline')}>时间线</button>
      <button class:active={view === 'calendar'} onclick={() => (view = 'calendar')}>日历</button>
      <button class:active={view === 'compare'} onclick={() => (view = 'compare')}>对比</button>
      <button class:active={view === 'settings'} onclick={() => (view = 'settings')}>设置</button>
      <button onclick={togglePause}>{paused ? '恢复监控' : '暂停监控'}</button>
    </nav>
  </header>

  {#if restTip}
    <div class="rest-banner">
      ⏸ 已连续使用较久，建议起身休息一下
      <button onclick={() => (restTip = false)}>知道了</button>
    </div>
  {/if}

  <main>
    {#if view === 'dashboard'}
      <Dashboard {focusDate} />
    {:else if view === 'timeline'}
      <TimelineView initialDate={focusDate ?? undefined} />
    {:else if view === 'calendar'}
      <CalendarView onPick={(d) => onPickDate(d)} />
    {:else if view === 'compare'}
      <CompareView />
    {:else}
      <SettingsView />
    {/if}
  </main>

  <footer>v{version} · 仅统计前台活跃时间 · 数据保存在本地</footer>
</div>

<style>
  .app {
    height: 100vh;
    display: flex;
    flex-direction: column;
    /* 半透明底让 Mica/亚克力背板透出 */
    background: color-mix(in srgb, var(--bg) 84%, transparent);
  }

  header {
    display: flex;
    align-items: center;
    gap: 18px;
    padding: 12px 18px;
    border-bottom: 1px solid var(--border);
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 15px;
    font-weight: 600;
  }

  .logo {
    font-size: 18px;
  }

  .live {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--text-dim);
    font-size: 13px;
  }

  .sep {
    opacity: 0.5;
  }

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--ok);
  }

  .dot.idle {
    background: var(--warn);
  }

  .dot.locked,
  .dot.suspended,
  .dot.paused {
    background: var(--danger);
  }

  nav {
    margin-left: auto;
    display: flex;
    gap: 8px;
  }

  nav button {
    background: var(--card);
    color: var(--text-dim);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 7px 16px;
    cursor: pointer;
    font-size: 13px;
    transition: all 0.15s;
  }

  nav button:hover {
    color: var(--text);
    border-color: var(--accent);
  }

  nav button.active {
    background: var(--accent);
    border-color: var(--accent);
    color: #fff;
  }

  main {
    flex: 1;
    overflow: hidden;
  }

  .rest-banner {
    display: flex;
    align-items: center;
    gap: 14px;
    background: rgba(245, 166, 35, 0.14);
    border-bottom: 1px solid rgba(245, 166, 35, 0.4);
    color: var(--warn);
    padding: 8px 18px;
    font-size: 13px;
  }

  .rest-banner button {
    margin-left: auto;
    background: transparent;
    border: 1px solid rgba(245, 166, 35, 0.4);
    color: var(--warn);
    border-radius: 6px;
    padding: 3px 12px;
    cursor: pointer;
  }

  footer {
    padding: 8px 18px;
    border-top: 1px solid var(--border);
    color: var(--text-dim);
    font-size: 12px;
  }
</style>
