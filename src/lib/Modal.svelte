<script lang="ts">
  import type { Snippet } from 'svelte';

  // 通用模态框。之前 Dashboard 与 SettingsView 各写一份 markup，
  // 而 CSS 只在 Dashboard 里定义过，SettingsView 的弹窗其实是散在页面里的裸元素。
  // 放到组件里之后：遮罩点击、Esc、关闭按钮、焦点归位都只有一处实现。
  let {
    label = '',
    closeLabel = '关闭',
    onclose,
    head,
    children,
  }: {
    label?: string;
    closeLabel?: string;
    onclose: () => void;
    head?: Snippet;
    children: Snippet;
  } = $props();

  let box = $state<HTMLDivElement | null>(null);

  function onKey(e: KeyboardEvent) {
    if (e.key !== 'Escape') return;
    e.preventDefault();
    onclose();
  }

  // 打开即把焦点移进弹窗，否则键盘用户要先 Tab 一大圈才够得着关闭按钮
  $effect(() => {
    box?.focus();
  });
</script>

<svelte:window onkeydown={onKey} />

<!-- 遮罩只承担「点空白关闭」，键盘关闭由上面的 window Escape 统一处理，
     所以这里不需要再挂 keydown；静态元素告警在此忽略。 -->
<!-- svelte-ignore a11y_click_events_have_key_events -->
<div class="modal-mask" role="presentation" onclick={onclose}>
  <div
    class="modal"
    role="dialog"
    aria-modal="true"
    aria-label={label || undefined}
    tabindex="-1"
    bind:this={box}
    onclick={(e) => e.stopPropagation()}
  >
    <div class="modal-head">
      <div class="modal-title">
        {#if head}{@render head()}{/if}
      </div>
      <button class="m-close" aria-label={closeLabel} onclick={onclose}>✕</button>
    </div>
    {@render children()}
  </div>
</div>

<style>
  .modal-mask {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 200;
  }

  .modal {
    width: min(560px, 88%);
    /* 内容多时自己滚，不把窗口撑破 */
    max-height: 82vh;
    overflow-y: auto;
    background: var(--card);
    border: 1px solid var(--border);
    border-radius: 14px;
    padding: 18px;
    outline: none;
  }

  .modal-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 12px;
  }

  .modal-title {
    flex: 1;
    min-width: 0;
  }

  .m-close {
    background: transparent;
    border: 1px solid var(--border);
    color: var(--text-dim);
    border-radius: 7px;
    padding: 4px 10px;
    cursor: pointer;
  }

  .m-close:hover {
    color: var(--text);
  }
</style>
