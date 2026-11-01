<script lang="ts">
  import { listen } from '@tauri-apps/api/event';
  import { api, fmtDuration, stateLabel, type LiveStatus } from './lib/api';

  let live = $state<LiveStatus | null>(null);

  $effect(() => {
    api.getLiveStatus().then((l) => (live = l));
    const un = listen<LiveStatus>('usage://tick', (e) => (live = e.payload));
    return () => {
      un.then((f) => f());
    };
  });
</script>

<div class="widget" data-tauri-drag-region>
  <div class="row top" data-tauri-drag-region>
    <span class="dot {live?.state.toLowerCase() ?? ''}"></span>
    <span class="state" data-tauri-drag-region>{live ? stateLabel(live.state) : '…'}</span>
    <span class="today" data-tauri-drag-region>今日 {live ? fmtDuration(live.today_total_seconds) : '--'}</span>
  </div>

  <div class="current" data-tauri-drag-region>
    {#if live?.current_app}
      <div class="app-name" data-tauri-drag-region>{live.current_app.display_name}</div>
      <div class="app-meta" data-tauri-drag-region>
        {live.current_category} · 本轮 {fmtDuration(live.current_seconds)}
      </div>
    {:else}
      <div class="app-name muted" data-tauri-drag-region>当前无前台应用</div>
    {/if}
  </div>

  <div class="tops" data-tauri-drag-region>
    {#each live?.top_apps ?? [] as t, i (t.display_name + i)}
      <div class="top-row">
        <span class="rank">{i + 1}</span>
        <span class="name">{t.display_name}</span>
        <span class="cat">{t.category}</span>
        <span class="dur">{fmtDuration(t.active_seconds)}</span>
      </div>
    {/each}
  </div>
</div>

<style>
  .widget {
    height: 100vh;
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 12px 14px;
    border-radius: 14px;
    background: rgba(16, 18, 24, 0.35);
    color: #ecf0f8;
    font-size: 12px;
    overflow: hidden;
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
    color: #8b93a5;
    font-size: 11px;
  }

  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .cat {
    color: #7aa8ff;
    font-size: 11px;
  }

  .dur {
    color: #aeb6c8;
    font-variant-numeric: tabular-nums;
    font-size: 11px;
  }
</style>
