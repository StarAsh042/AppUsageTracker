<script lang="ts">
  import { listen } from '@tauri-apps/api/event';
  import Dashboard from './Dashboard.svelte';
  import TimelineView from './TimelineView.svelte';
  import CalendarView from './CalendarView.svelte';
  import CompareView from './CompareView.svelte';
  import SettingsView from './SettingsView.svelte';
  import { api, fmtDuration, stateLabel, type LiveStatus } from './lib/api';
  import { lang, en } from './lib/i18n';

  let view = $state<'dashboard' | 'timeline' | 'calendar' | 'compare' | 'settings'>('dashboard');
  let live = $state<LiveStatus | null>(null);
  let version = $state('');
  let paused = $state(false);
  let widgetVisible = $state(false);
  let focusDate = $state<string | null>(null);
  let restTip = $state(false);
  let restTimer: ReturnType<typeof setTimeout> | undefined;
  // 备份恢复 / 清空数据后自增，重挂载数据视图以重新拉数据
  let dataEpoch = $state(0);

  const t = $derived((s: string) => ($lang === 'en' ? (en[s] ?? s) : s));

  $effect(() => {
    api.getVersion().then((v) => (version = v));
    api.getSettings().then((s) => {
      applyTheme(s.theme);
      widgetVisible = s.show_floating_widget;
      lang.set(s.language === 'en' ? 'en' : 'zh');
    });
    const un1 = listen<LiveStatus>('usage://tick', (e) => {
      live = e.payload;
      paused = e.payload.state === 'Paused';
    });
    const un2 = listen<string>('ui://navigate', (e) => {
      view = (e.payload as typeof view) ?? 'dashboard';
    });
    const un3 = listen<{ theme?: string; language?: string }>('app://settings', (e) => {
      applyTheme(e.payload?.theme);
      lang.set(e.payload?.language === 'en' ? 'en' : 'zh');
    });
    const un4 = listen('ui://rest', () => {
      restTip = true;
      clearTimeout(restTimer);
      restTimer = setTimeout(() => (restTip = false), 30_000);
    });
    // 悬浮窗显示状态（顶栏按钮 / 托盘 / 悬浮窗自身关闭都会同步）
    const un5 = listen<boolean>('widget://visibility', (e) => {
      widgetVisible = e.payload === true;
    });
    const un6 = listen('app://data', () => {
      dataEpoch += 1;
    });
    // 禁用 WebView 默认右键菜单（排行右键分类等自定义菜单不受影响）
    const prevent = (e: Event) => e.preventDefault();
    window.addEventListener('contextmenu', prevent);
    return () => {
      un1.then((f) => f());
      un2.then((f) => f());
      un3.then((f) => f());
      un4.then((f) => f());
      un5.then((f) => f());
      un6.then((f) => f());
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

  async function toggleWidget() {
    widgetVisible = await api.toggleWidget();
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
        <span>{stateLabel(live.state, $lang)}</span>
        <span class="sep">·</span>
        <span>{t('今日')} {fmtDuration(live.today_total_seconds)}</span>
      {/if}
    </div>
    <nav>
      <button class:active={view === 'dashboard'} onclick={() => (view = 'dashboard')}>{t('仪表盘')}</button>
      <button class:active={view === 'timeline'} onclick={() => (view = 'timeline')}>{t('时间线')}</button>
      <button class:active={view === 'calendar'} onclick={() => (view = 'calendar')}>{t('日历')}</button>
      <button class:active={view === 'compare'} onclick={() => (view = 'compare')}>{t('对比')}</button>
      <button class:active={view === 'settings'} onclick={() => (view = 'settings')}>{t('设置')}</button>
      <button class:active={widgetVisible} onclick={toggleWidget}>{t('悬浮窗')}</button>
      <button onclick={togglePause}>{paused ? t('恢复监控') : t('暂停监控')}</button>
    </nav>
  </header>

  {#if restTip}
    <div class="rest-banner">
      ⏸ {t('已连续使用较久')}，{t('建议起身休息一下')}
      <button onclick={() => (restTip = false)}>{t('知道了')}</button>
    </div>
  {/if}

  <main>
    {#if view === 'settings'}
      <SettingsView />
    {:else}
      {#key dataEpoch}
        {#if view === 'dashboard'}
          <Dashboard {focusDate} />
        {:else if view === 'timeline'}
          <TimelineView initialDate={focusDate ?? undefined} />
        {:else if view === 'calendar'}
          <CalendarView onPick={(d) => onPickDate(d)} />
        {:else if view === 'compare'}
          <CompareView />
        {/if}
      {/key}
    {/if}
  </main>

  <footer>v{version} · {t('仅统计前台活跃时间')} · {t('数据保存在本地')}</footer>
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
