<script lang="ts">
  import { api } from './api';
  import { lang, translate } from './i18n';
  import Modal from './Modal.svelte';

  // 手动安装扩展的引导弹窗。每次打开都重新挂载，所以路径与「已复制」状态天生是干净的。
  let { onclose }: { onclose: () => void } = $props();

  let path = $state('');
  let copied = $state(false);

  const t = $derived((s: string, params?: Record<string, string | number>) => translate($lang, s, params));

  $effect(() => {
    api
      .getExtensionPath()
      .then((p) => (path = p))
      .catch(() => (path = ''));
  });

  async function install(browser: string) {
    await api.openBrowserExtPage(browser);
    copied = true;
    try {
      await navigator.clipboard.writeText(path);
    } catch {
      // 剪贴板不可用时静默：路径已经显示在界面上，用户仍可手动选中复制
    }
  }

  async function copyPath() {
    try {
      await navigator.clipboard.writeText(path);
      copied = true;
    } catch {
      // 剪贴板不可用时静默：路径已经显示在界面上，用户仍可手动选中复制
    }
  }
</script>

<Modal label={t('安装浏览器扩展')} {onclose}>
  {#snippet head()}
    <div class="m-name">{t('安装浏览器扩展')}</div>
  {/snippet}
  <p class="hint" style="margin:12px 0">{t('选择你的浏览器，将打开扩展管理页')}</p>
  <div class="ext-browser-row">
    <button class="ext-browser-btn" onclick={() => install('chrome')}>
      <span class="ext-icon">🌐</span> Chrome
    </button>
    <button class="ext-browser-btn" onclick={() => install('edge')}>
      <span class="ext-icon">⚡</span> Edge
    </button>
  </div>
  <div class="ext-steps">
    <p>{t('扩展管理页打开后：')}</p>
    <p>1. {t('开启「开发者模式」')}</p>
    <p>2. {t('点击「加载已解压的扩展程序」')}</p>
    <p>3. {t('选择以下目录（已复制到剪贴板）')}</p>
    <code class="ext-path">{copied ? t('已复制到剪贴板 ✓') : path}</code>
  </div>
  <button class="primary" style="margin-top:12px;width:100%" onclick={copyPath}>
    {copied ? t('已复制到剪贴板 ✓') : t('复制扩展目录路径')}
  </button>
</Modal>

<style>
  /* 弹窗标题（Modal 组件只管外壳，标题排版留给调用方） */
  .m-name {
    font-size: 17px;
    font-weight: 700;
  }

  .hint {
    color: var(--text-dim);
    font-size: 12px;
  }

  .ext-browser-row {
    display: flex;
    gap: 10px;
    margin: 12px 0;
  }

  .ext-browser-btn {
    flex: 1;
    background: var(--card2);
    border: 1px solid var(--border);
    color: var(--text);
    border-radius: 10px;
    padding: 14px;
    font-size: 14px;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    transition: border-color 0.12s;
  }

  .ext-browser-btn:hover {
    border-color: var(--accent);
  }

  .ext-icon {
    font-size: 18px;
  }

  .ext-steps {
    background: var(--card2);
    border-radius: 8px;
    padding: 10px 14px;
    font-size: 12px;
    color: var(--text-dim);
  }

  .ext-steps p {
    margin: 2px 0;
  }

  .ext-path {
    display: block;
    margin-top: 4px;
    padding: 4px 8px;
    background: var(--bg);
    border-radius: 4px;
    font-family: Consolas, monospace;
    font-size: 11px;
    color: var(--accent);
    word-break: break-all;
  }

  .primary {
    background: var(--accent);
    border: 0;
    color: #fff;
    border-radius: 8px;
    padding: 8px 22px;
    cursor: pointer;
    font-size: 13px;
    font-weight: 600;
  }
</style>
