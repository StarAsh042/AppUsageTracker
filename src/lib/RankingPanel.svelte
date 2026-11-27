<script lang="ts">
  import { categoryLabel, fmtDuration, type DashboardApp } from './api';
  import { lang, translate } from './i18n';

  let {
    apps,
    nameOf,
    colorOf,
    onopen,
    onmenu,
  }: {
    apps: DashboardApp[];
    nameOf: (a: DashboardApp) => string;
    colorOf: (category: string) => string;
    onopen: (appKey: string) => void;
    onmenu: (e: MouseEvent, appKey: string, category: string) => void;
  } = $props();

  const t = $derived((s: string, params?: Record<string, string | number>) => translate($lang, s, params));

  // 名字比列宽长时横向滚动给完整名字：滚动量按实际宽度算，不能写死。
  function hoverMarquee(node: HTMLElement) {
    const move = () => {
      const wrap = node.querySelector<HTMLElement>('.name-wrap');
      const inner = node.querySelector<HTMLElement>('.name-inner');
      if (!wrap || !inner || inner.scrollWidth <= wrap.clientWidth + 1) return;
      wrap.style.setProperty('--cw', wrap.clientWidth + 'px');
      wrap.classList.add('scrolling');
    };
    const stop = () => {
      node.querySelector('.name-wrap')?.classList.remove('scrolling');
    };
    node.addEventListener('pointerenter', move);
    node.addEventListener('pointerleave', stop);
    return {
      destroy() {
        node.removeEventListener('pointerenter', move);
        node.removeEventListener('pointerleave', stop);
      },
    };
  }
</script>

<div class="panel">
  <div class="panel-title">{t('应用排行')}</div>
  <div class="table-head">
    <span class="col-rank">#</span>
    <span class="col-name">{t('应用')}</span>
    <span class="col-cat">{t('分类')}</span>
    <span class="col-dur">{t('活跃时长')}</span>
    <span class="col-bar"><span class="pct-head">{t('占比')}</span></span>
  </div>
  <div class="table-body">
    {#each apps as a, i (a.app_key)}
      <button
        class="row"
        type="button"
        onclick={() => onopen(a.app_key)}
        oncontextmenu={(e) => onmenu(e, a.app_key, a.category)}
        title={a.exe_path}
      >
        <span class="col-rank">{i + 1}</span>
        <span class="col-name" use:hoverMarquee>
          {#if a.icon_data_url}<img class="app-icon" src={a.icon_data_url} alt="" />{/if}
          <span class="name-wrap"><span class="name-inner">{nameOf(a)}</span></span>
        </span>
        <span class="col-cat">
          <span class="cat-chip" style="--cc:{colorOf(a.category)}">{categoryLabel(a.category, $lang)}</span>
        </span>
        <span class="col-dur">{fmtDuration(a.active_seconds)}</span>
        <span class="col-bar">
          <span class="bar"><span class="fill" style="width:{Math.min(100, a.percent)}%"></span></span>
          <span class="pct">{a.percent.toFixed(1)}%</span>
        </span>
      </button>
    {:else}
      <div class="empty">{t('该时间段暂无数据')}</div>
    {/each}
  </div>
</div>

<style>
  .panel {
    background: var(--glass-card, var(--card));
    border: 1px solid var(--border);
    border-radius: 12px;
    padding: 14px;
    display: flex;
    flex-direction: column;
    min-height: 0;
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

  /* 排行行是可点进详情的，所以用 button：键盘能 Tab 到、能回车触发。
     按钮默认样式要全部抹掉，否则继承浏览器底色和边框。 */
  button.row {
    background: transparent;
    border: 0;
    color: inherit;
    font: inherit;
    text-align: left;
    width: 100%;
  }

  button.row:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
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
    display: flex;
    align-items: center;
    gap: 6px;
    overflow: visible;
    min-width: 0;
  }

  .app-icon {
    width: 16px;
    height: 16px;
    flex: 0 0 16px;
    object-fit: contain;
    border-radius: 3px;
  }

  .name-wrap {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
    flex: 1;
  }

  .name-inner {
    white-space: nowrap;
  }

  .name-wrap:global(.scrolling) .name-inner {
    display: inline-block;
    padding-right: 14px;
    animation: app-name-marquee 7s linear infinite;
  }

  @keyframes app-name-marquee {
    0%,
    12% {
      transform: translateX(0);
    }
    88%,
    100% {
      transform: translateX(calc(-100% + var(--cw, 0px)));
    }
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

  .pct-head {
    margin-left: auto;
    width: 52px;
    text-align: right;
  }

  .empty {
    color: var(--text-dim);
    padding: 18px 0;
    text-align: center;
    font-size: 13px;
  }
</style>
