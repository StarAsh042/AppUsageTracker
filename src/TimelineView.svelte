<script lang="ts">
  import { api, fmtDuration, localDateKey, type TimelineData } from './lib/api';

  let { initialDate = localDateKey() }: { initialDate?: string } = $props();

  let date = $state(initialDate);
  let data = $state<TimelineData | null>(null);
  let loading = $state(false);
  let hover = $state<{ name: string; detail: string; x: number } | null>(null);

  $effect(() => {
    void date;
    load();
  });

  async function load() {
    loading = true;
    try {
      data = await api.getTimeline(date);
    } finally {
      loading = false;
    }
  }

  function shiftDay(offset: number) {
    const d = new Date(date + 'T00:00:00');
    d.setDate(d.getDate() + offset);
    const p = (n: number) => String(n).padStart(2, '0');
    date = `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
  }

  // 时间带：按起始秒数定位（0-86400）
  function secOf(t: string): number {
    const m = /(\d{2}):(\d{2}):(\d{2})/.exec(t);
    return m ? Number(m[1]) * 3600 + Number(m[2]) * 60 + Number(m[3]) : 0;
  }

  function segStyle(start: string, end: string): string {
    const s = secOf(start);
    const e = Math.max(s + 0.15, secOf(end));
    const left = (s / 86400) * 100;
    const width = Math.max(0.12, ((e - s) / 86400) * 100);
    return `left:${left}%;width:${width}%`;
  }

  const PALETTE = ['#4f8cff', '#3ecf8e', '#f5a623', '#a78bfa', '#22d3ee', '#fb923c', '#f472b6', '#94a3b8'];

  function appColor(segments: { app_key: string }[], key: string): string {
    const idx = [...new Set(segments.map((s) => s.app_key))].indexOf(key);
    return PALETTE[idx % PALETTE.length];
  }

  const STATE_COLORS: Record<string, string> = {
    Active: '#3ecf8e',
    Idle: '#f5a623',
    Locked: '#ff5f5f',
    Suspended: '#8b93a5',
    Paused: '#565d6e',
  };

  const stateLabelCn: Record<string, string> = {
    Active: '统计中',
    Idle: '空闲',
    Locked: '锁定',
    Suspended: '挂起',
    Paused: '暂停',
  };

  function stateTotals(data: TimelineData): { state: string; seconds: number }[] {
    const m = new Map<string, number>();
    for (const s of data.states) {
      m.set(s.state, (m.get(s.state) ?? 0) + s.duration_seconds);
    }
    return [...m.entries()].map(([state, seconds]) => ({ state, seconds })).sort((a, b) => b.seconds - a.seconds);
  }

  function onMouse(e: MouseEvent, name: string, detail: string) {
    hover = { name, detail, x: e.clientX };
  }

  const hours = Array.from({ length: 24 }, (_, i) => i);
</script>

<div class="tl">
  <div class="toolbar">
    <button class="nav" onclick={() => shiftDay(-1)}>◀</button>
    <input type="date" bind:value={date} />
    <button class="nav" onclick={() => shiftDay(1)}>▶</button>
    <button class="ghost" onclick={() => (date = localDateKey())}>今天</button>
    <span class="hint">24 小时时间线 · 悬停查看详情</span>
  </div>

  {#if data}
    <div class="panel">
      <div class="band-title">应用</div>
      <div class="band" data-tauri-drag-region="false">
        {#each data.segments as seg (seg.start_time + seg.app_key)}
          <div
            class="seg"
            style="{segStyle(seg.start_time, seg.end_time)};background:{appColor(data.segments, seg.app_key)}"
            title="{seg.display_name} {seg.start_time.slice(11, 16)}–{seg.end_time.slice(11, 16)}（{fmtDuration(seg.duration_seconds)}）"
          ></div>
        {/each}
        {#each hours as h (h)}
          <span class="tick" style="left:{(h / 24) * 100}%">{h}</span>
        {/each}
      </div>

      <div class="band-title">状态</div>
      <div class="band thin">
        {#each data.states as st (st.start_time + st.state)}
          <div
            class="seg"
            style="{segStyle(st.start_time, st.end_time)};background:{STATE_COLORS[st.state] ?? '#565d6e'}"
            title="{stateLabelCn[st.state] ?? st.state} {st.start_time.slice(11, 16)}–{st.end_time.slice(11, 16)}（{fmtDuration(st.duration_seconds)}）"
          ></div>
        {/each}
        {#each hours as h (h)}
          <span class="tick" style="left:{(h / 24) * 100}%"></span>
        {/each}
      </div>

      <div class="states">
        {#each stateTotals(data) as s (s.state)}
          <span class="state-item">
            <span class="sw" style="background:{STATE_COLORS[s.state] ?? '#565d6e'}"></span>
            {stateLabelCn[s.state] ?? s.state} {fmtDuration(s.seconds)}
          </span>
        {:else}
          <span class="empty">该日暂无状态记录</span>
        {/each}
      </div>
    </div>

    <div class="panel legend">
      <div class="band-title">图例（按时长）</div>
      <div class="legend-list">
        {#each [...new Map(data.segments.map((s) => [s.app_key, s])).values()] as seg, i (seg.app_key)}
          <span class="legend-item">
            <span class="sw" style="background:{appColor(data.segments, seg.app_key)}"></span>
            {seg.display_name}
            <span class="cat">{seg.category}</span>
          </span>
        {/each}
      </div>
    </div>
  {:else}
    <div class="loading">{loading ? '加载中…' : ''}</div>
  {/if}

  {#if hover}
    <div class="tooltip" style="left:{hover.x}px">{hover.name} · {hover.detail}</div>
  {/if}
</div>

<style>
  .tl {
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
    gap: 10px;
  }

  .nav {
    background: var(--card);
    border: 1px solid var(--border);
    color: var(--text-dim);
    border-radius: 7px;
    padding: 6px 12px;
    cursor: pointer;
  }

  .nav:hover {
    color: var(--text);
    border-color: var(--accent);
  }

  input[type='date'] {
    background: var(--card);
    border: 1px solid var(--border);
    color: var(--text);
    border-radius: 7px;
    padding: 5px 10px;
    font-size: 13px;
  }

  .ghost {
    background: transparent;
    border: 1px solid var(--border);
    color: var(--text-dim);
    border-radius: 7px;
    padding: 6px 14px;
    cursor: pointer;
  }

  .hint {
    color: var(--text-dim);
    font-size: 12px;
    margin-left: auto;
  }

  .panel {
    background: var(--card);
    border: 1px solid var(--border);
    border-radius: 12px;
    padding: 14px 16px;
  }

  .band-title {
    font-size: 12px;
    color: var(--text-dim);
    margin-bottom: 8px;
  }

  .band {
    position: relative;
    height: 44px;
    background: var(--card2);
    border-radius: 8px;
    overflow: hidden;
    margin-bottom: 4px;
  }

  .band.thin {
    height: 20px;
  }

  .seg {
    position: absolute;
    top: 3px;
    bottom: 3px;
    border-radius: 3px;
    min-width: 2px;
  }

  .band.thin .seg {
    top: 2px;
    bottom: 2px;
  }

  .tick {
    position: absolute;
    top: 0;
    bottom: 0;
    border-left: 1px solid color-mix(in srgb, var(--border) 70%, transparent);
    font-size: 9px;
    color: var(--text-dim);
    padding-left: 3px;
    line-height: 12px;
    pointer-events: none;
  }

  .states {
    display: flex;
    flex-wrap: wrap;
    gap: 14px;
    margin-top: 10px;
    font-size: 12px;
    color: var(--text-dim);
  }

  .state-item,
  .legend-item {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }

  .sw {
    width: 10px;
    height: 10px;
    border-radius: 3px;
    display: inline-block;
  }

  .legend {
    margin-top: 14px;
  }

  .legend-list {
    display: flex;
    flex-wrap: wrap;
    gap: 12px 20px;
    font-size: 13px;
  }

  .legend-item .cat {
    color: var(--accent);
    font-size: 11px;
  }

  .empty {
    color: var(--text-dim);
    font-size: 12px;
  }

  .loading {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-dim);
  }

  .tooltip {
    position: fixed;
    top: 8px;
    transform: translateX(-50%);
    background: var(--card2);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 5px 10px;
    font-size: 12px;
    z-index: 50;
    pointer-events: none;
    white-space: nowrap;
  }
</style>
