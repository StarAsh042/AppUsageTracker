<script lang="ts">
  import { listen } from '@tauri-apps/api/event';
  import Dashboard from './Dashboard.svelte';
  import SettingsView from './SettingsView.svelte';
  import { api, fmtDuration, stateLabel, type LiveStatus } from './lib/api';

  let view = $state<'dashboard' | 'settings'>('dashboard');
  let live = $state<LiveStatus | null>(null);
  let version = $state('');
  let paused = $state(false);

  $effect(() => {
    api.getVersion().then((v) => (version = v));
    const un1 = listen<LiveStatus>('usage://tick', (e) => {
      live = e.payload;
      paused = e.payload.state === 'Paused';
    });
    const un2 = listen<string>('ui://navigate', (e) => {
      view = e.payload === 'settings' ? 'settings' : 'dashboard';
    });
    return () => {
      un1.then((f) => f());
      un2.then((f) => f());
    };
  });

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
      <button class:active={view === 'settings'} onclick={() => (view = 'settings')}>设置</button>
      <button onclick={togglePause}>{paused ? '恢复监控' : '暂停监控'}</button>
    </nav>
  </header>

  <main>
    {#if view === 'dashboard'}
      <Dashboard />
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
    background: var(--bg);
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

  footer {
    padding: 8px 18px;
    border-top: 1px solid var(--border);
    color: var(--text-dim);
    font-size: 12px;
  }
</style>
