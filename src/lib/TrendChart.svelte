<script module lang="ts">
  // 一根柱子。趋势面板与详情弹窗共用同一份渲染，只在外层决定数据和形态。
  export type TrendBar = {
    key: string;
    title: string;
    label: string;
    percent: number;
    score?: boolean;
    tone?: 'good' | 'bad';
  };
</script>

<script lang="ts">
  let { bars, compact = false, empty = '' }: { bars: TrendBar[]; compact?: boolean; empty?: string } = $props();
</script>

<div class="bars" class:compact>
  {#each bars as b (b.key)}
    <div class="col" title={b.title}>
      <div class="wrap">
        <div
          class="fill"
          class:score={b.score}
          class:good={b.tone === 'good'}
          class:bad={b.tone === 'bad'}
          style="height:{b.percent}%"
        ></div>
      </div>
      <div class="label">{b.label}</div>
    </div>
  {:else}
    {#if empty}<div class="empty">{empty}</div>{/if}
  {/each}
</div>

<style>
  .bars {
    flex: 1;
    min-height: 0;
    display: flex;
    align-items: stretch;
    justify-content: center;
    gap: 6px;
  }

  .col {
    flex: 0 1 72px;
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
  }

  .wrap {
    flex: 1;
    display: flex;
    align-items: flex-end;
    background: var(--card2);
    border-radius: 4px;
    overflow: hidden;
  }

  .fill {
    width: 100%;
    background: linear-gradient(180deg, #4f8cff, #3566cc);
    border-radius: 6px 6px 0 0;
    min-height: 2px;
  }

  .fill.score {
    background: linear-gradient(180deg, #f5a623, #c77f10);
  }

  .fill.score.good {
    background: linear-gradient(180deg, #3ecf8e, #23a56d);
  }

  .fill.score.bad {
    background: linear-gradient(180deg, #ff5f5f, #c73e3e);
  }

  .label {
    font-size: 10px;
    color: var(--text-dim);
    text-align: center;
    overflow: hidden;
    white-space: nowrap;
  }

  .empty {
    flex: 1;
    align-self: center;
    color: var(--text-dim);
    text-align: center;
    font-size: 13px;
  }

  /* 详情弹窗里的柱子更窄、字号更小，但仍沿用同一套结构。 */
  .bars.compact {
    gap: 5px;
  }

  .compact .col {
    gap: 4px;
  }

  .compact .fill {
    border-radius: 4px 4px 0 0;
  }

  .compact .label {
    font-size: 9px;
  }
</style>
