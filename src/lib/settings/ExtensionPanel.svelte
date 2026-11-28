<script lang="ts">
  import { api, type TrackerSettings, type ExtInstallResult } from '../api';
  import { lang, translate } from '../i18n';
  import { errText } from '../errors';

  let {
    settings = $bindable(),
    excludedText = $bindable(''),
    onshowinstall,
    onstatus,
  }: {
    settings: TrackerSettings;
    excludedText: string;
    onshowinstall: () => void;
    onstatus: (message: string) => void;
  } = $props();

  let extInstalling = $state(false);
  let extResults = $state<ExtInstallResult[]>([]);

  const t = $derived((key: string, params?: Record<string, string | number>) => translate($lang, key, params));

  async function smartInstall() {
    extInstalling = true;
    extResults = [];
    try {
      extResults = await api.smartInstallExtension();
      const okCount = extResults.filter((r) => r.ok).length;
      onstatus(okCount ? t('扩展已注册，重启浏览器后生效') : t('未能自动安装，请改用手动安装'));
      if (okCount) settings.record_web = true;
    } catch (e) {
      onstatus(`${t('安装失败')}: ${errText(e)}`);
    } finally {
      extInstalling = false;
    }
  }

  function extResultText(r: ExtInstallResult): string {
    switch (r.code) {
      case 'ok':
        return `${t('已注册，重启浏览器后生效')} · ${t('扩展 ID')} ${r.extension_id}`;
      case 'not_found':
        return t('未检测到该浏览器');
      case 'pack_failed':
        return t('打包失败：{message}', { message: r.message });
      case 'bad_crx':
        return t('打包产物无效，请改用手动安装');
      case 'registry_failed':
        return t('写入注册表失败：{message}', { message: r.message });
      default:
        return r.message;
    }
  }
</script>

<section class="panel">
  <h3>{t('浏览器扩展')}</h3>
  <p class="hint" style="margin-bottom:8px">
    {t(
      '一键安装：自动识别已安装的 Chromium 内核浏览器（Chrome / Edge / Cent / Brave / Vivaldi 等），打包并注册扩展，重启浏览器后生效',
    )}
  </p>
  <div class="row-actions">
    <button class="primary" disabled={extInstalling} onclick={smartInstall}>
      {extInstalling ? t('安装中') + '…' : t('智能安装扩展（推荐）')}
    </button>
    <button class="ghost" onclick={() => onshowinstall()}>{t('手动安装')}</button>
  </div>
  {#if extResults.length}
    <div class="ext-results">
      {#each extResults as r (r.browser)}
        <div class="ext-result" class:ok={r.ok}>
          <span class="ext-badge">{r.ok ? '✓' : '·'}</span>
          <span class="ext-name">{r.name}</span>
          <span class="ext-msg">{extResultText(r)}</span>
        </div>
      {/each}
    </div>
  {/if}
</section>

<section class="panel">
  <h3>{t('排除应用')}</h3>
  <p class="hint">{t('每行一个。')}</p>
  <textarea rows="5" bind:value={excludedText} spellcheck="false" aria-label={t('排除应用')}></textarea>
</section>
