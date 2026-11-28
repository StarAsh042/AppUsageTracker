<script lang="ts">
  import { api, type TrackerSettings } from './lib/api';
  import { lang, translate } from './lib/i18n';
  import { errText } from './lib/errors';
  import ExtensionModal from './lib/ExtensionModal.svelte';
  import GeneralPanel from './lib/settings/GeneralPanel.svelte';
  import ExtensionPanel from './lib/settings/ExtensionPanel.svelte';
  import CategoriesPanel from './lib/settings/CategoriesPanel.svelte';
  import DataPanel from './lib/settings/DataPanel.svelte';
  import './lib/settings/settings.css';

  let s = $state<TrackerSettings | null>(null);
  let excludedText = $state('');
  let titlePatternsText = $state('');
  let status = $state('');
  let saving = $state(false);
  let appVersion = $state('');
  let showExtInstall = $state(false);

  const t = $derived((key: string, params?: Record<string, string | number>) => translate($lang, key, params));

  $effect(() => {
    (async () => {
      const st = await api.getSettings();
      s = st;
      // 两个文本域各自跨越多行，绑定的是原始文本，保存时才按行规整。
      excludedText = st.excluded_app_keys.join('\n');
      titlePatternsText = st.title_exclude_patterns.join('\n');
      appVersion = await api.getVersion();
    })();
  });

  async function save() {
    if (!s) return;
    status = '';
    saving = true;
    try {
      const snapshot = $state.snapshot(s) as TrackerSettings;
      snapshot.excluded_app_keys = excludedText
        .split(/\r?\n/)
        .map((x) => x.trim().toLowerCase())
        .filter(Boolean);
      snapshot.title_exclude_patterns = titlePatternsText
        .split(/\r?\n/)
        .map((x) => x.trim())
        .filter(Boolean);
      await api.saveSettings(snapshot);
      status = t('已保存，设置即时生效');
    } catch (e) {
      status = `${t('保存失败')}: ${errText(e)}`;
    } finally {
      saving = false;
    }
  }
</script>

<div class="settings">
  {#if s}
    <div class="sections">
      <GeneralPanel bind:settings={s} bind:titlePatternsText onsave={save} onstatus={(m) => (status = m)} />
      <ExtensionPanel
        bind:settings={s}
        bind:excludedText
        onshowinstall={() => (showExtInstall = true)}
        onstatus={(m) => (status = m)}
      />
      <CategoriesPanel bind:settings={s} onstatus={(m) => (status = m)} />
      <DataPanel bind:settings={s} {appVersion} onsave={save} onstatus={(m) => (status = m)} />
    </div>

    <div class="save-bar">
      {#if status}<span class="status">{status}</span>{/if}
      <button class="primary" disabled={saving} onclick={save}>{t('保存设置')}</button>
    </div>
  {:else}
    <div class="loading">{t('加载中')}…</div>
  {/if}
</div>

{#if s && showExtInstall}
  <ExtensionModal onclose={() => (showExtInstall = false)} />
{/if}
