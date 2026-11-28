<script lang="ts">
  import { categoryLabel, fmtDuration } from './api';
  import { lang, translate } from './i18n';

  type Row = { name: string; seconds: number; depth: number; percent: number };

  let { tree, colorOf }: { tree: Row[]; colorOf: (name: string) => string } = $props();

  const t = $derived((s: string, params?: Record<string, string | number>) => translate($lang, s, params));

  const R = 42;
  const CIRC = 2 * Math.PI * R;

  // 环形图：只画顶层分类，段间留一点缝；子分类在下面的列表里展开。
  const segs = $derived.by(() => {
    const total = tree.reduce((s, c) => s + c.seconds, 0) || 1;
    let acc = 0;
    return tree
      .filter((c) => c.depth === 0)
      .map((c) => {
        const frac = c.seconds / total;
        const dash = Math.max(frac * CIRC - 1.5, 0.6);
        const seg = {
          name: c.name,
          color: colorOf(c.name),
          dash: dash + ' ' + (CIRC - dash),
          offset: CIRC * (1 - acc) + CIRC / 4,
        };
        acc += frac;
        return seg;
      });
  });
</script>

{#if segs.length}
  <div class="donut-wrap">
    <svg viewBox="0 0 100 100" class="donut">
      {#each segs as s (s.name)}
        <circle
          cx="50"
          cy="50"
          r={R}
          fill="none"
          stroke={s.color}
          stroke-width="13"
          stroke-dasharray={s.dash}
          stroke-dashoffset={s.offset}
        ></circle>
      {/each}
    </svg>
    <div class="donut-legend">
      {#each segs as s (s.name)}
        <span class="legend-item">
          <span class="sw" style="background:{s.color}"></span>
          {categoryLabel(s.name, $lang)}
        </span>
      {/each}
    </div>
  </div>
{/if}
<div class="cats">
  {#each tree as c (c.name)}
    <div class="cat-row" class:child={c.depth > 0}>
      <span class="cat-name" class:dim={c.depth > 0}>
        {c.depth > 0 ? '└ ' : ''}{categoryLabel(c.name, $lang)}
      </span>
      <span class="cat-bar"><span class="fill" style="width:{c.percent}%"></span></span>
      <span class="cat-dur">{fmtDuration(c.seconds)}</span>
    </div>
  {:else}
    <div class="empty">{t('暂无数据')}</div>
  {/each}
</div>

<style>
  .donut-wrap {
    display: flex;
    align-items: center;
    gap: 16px;
    margin-bottom: 12px;
    flex: none;
  }

  .donut {
    width: 92px;
    height: 92px;
    flex: none;
  }

  /* 图例改成两列并限高：分类多时（本月最容易多）单列图例会顶穿面板。 */
  .donut-legend {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 4px 12px;
    font-size: 12px;
    max-height: 92px;
    overflow-y: auto;
    align-content: start;
  }

  .legend-item {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }

  .donut-legend .sw {
    width: 9px;
    height: 9px;
    border-radius: 3px;
  }

  .cats {
    display: flex;
    flex-direction: column;
    gap: 10px;
    flex: 1 1 auto;
    min-height: 72px;
    overflow-y: auto;
    padding-right: 4px;
  }

  .cat-row {
    display: grid;
    grid-template-columns: 64px 1fr 84px;
    gap: 8px;
    align-items: center;
    font-size: 13px;
  }

  .cat-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .cat-row.child {
    padding-left: 10px;
  }

  .cat-name.dim {
    color: var(--text-dim);
    font-size: 12px;
  }

  .cat-bar {
    height: 6px;
    border-radius: 3px;
    background: var(--card2);
    overflow: hidden;
  }

  .fill {
    display: block;
    height: 100%;
    border-radius: 3px;
    background: linear-gradient(90deg, #3ecf8e, #6ee7b7);
  }

  .cat-dur {
    text-align: right;
    color: var(--text-dim);
    font-size: 12px;
    font-variant-numeric: tabular-nums;
  }

  .empty {
    color: var(--text-dim);
    padding: 18px 0;
    text-align: center;
    font-size: 13px;
  }
</style>
