<script lang="ts">
  import { api, type TrackerSettings, type UpdateInfo, type DbMaintenance } from '../api';
  import { lang, translate } from '../i18n';
  import { errCode, errText } from '../errors';

  let {
    settings = $bindable(),
    appVersion,
    onsave,
    onstatus,
  }: {
    settings: TrackerSettings;
    appVersion: string;
    onsave: () => void;
    onstatus: (message: string) => void;
  } = $props();

  let backups = $state<string[]>([]);
  let updateInfo = $state<UpdateInfo | null>(null);
  let updateStatus = $state('');
  let updateChecking = $state(false);
  let updateInstalling = $state(false);
  let fileInput: HTMLInputElement | undefined = $state();
  let maint = $state<DbMaintenance | null>(null);
  let maintBusy = $state(false);
  let maintError = $state('');

  const t = $derived((key: string, params?: Record<string, string | number>) => translate($lang, key, params));

  $effect(() => {
    (async () => {
      backups = await api.listBackups();
    })();
  });

  async function exportBackup() {
    onstatus('');
    try {
      const path = await api.exportBackupJson();
      backups = await api.listBackups();
      onstatus(`${t('已导出')}: ${path}`);
    } catch (e) {
      onstatus(errText(e));
    }
  }

  async function deleteBackup(name: string) {
    if (!confirm(t('删除备份 {name}？', { name }))) return;
    try {
      await api.deleteBackup(name);
      backups = await api.listBackups();
      onstatus(t('备份已删除'));
    } catch (e) {
      onstatus(errText(e));
    }
  }

  async function importBackup() {
    if (!confirm(t('导入将清空当前全部数据并以最新备份替换，确定继续？'))) return;
    onstatus('');
    try {
      onstatus(await api.importBackupJson());
    } catch (e) {
      onstatus(errText(e));
    }
  }

  async function restoreBackup(name: string) {
    if (!confirm(t('恢复备份 {name}？将清空当前全部数据，确定继续？', { name }))) return;
    onstatus('');
    try {
      onstatus(await api.restoreBackupNamed(name));
    } catch (e) {
      onstatus(errText(e));
    }
  }

  async function openBackupDir() {
    try {
      await api.openBackupDir();
    } catch (e) {
      onstatus(errText(e));
    }
  }

  function pickBackupFile() {
    fileInput?.click();
  }

  async function onBackupFile(e: Event) {
    const input = e.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;
    if (!confirm(t('从所选文件恢复 {name}？将清空当前全部数据，确定继续？', { name: file.name }))) {
      input.value = '';
      return;
    }
    onstatus('');
    try {
      const content = await file.text();
      onstatus(await api.restoreBackupContent(content));
    } catch (err) {
      onstatus(errText(err));
    } finally {
      input.value = '';
    }
  }

  function formatBytes(n: number): string {
    if (!Number.isFinite(n) || n <= 0) return '0 B';
    const units = ['B', 'KB', 'MB', 'GB'];
    let v = n;
    let i = 0;
    while (v >= 1024 && i < units.length - 1) {
      v /= 1024;
      i += 1;
    }
    return `${v.toFixed(i === 0 ? 0 : 1)} ${units[i]}`;
  }

  async function runMaintenance() {
    maintBusy = true;
    maintError = '';
    try {
      maint = await api.dbMaintenance();
    } catch (e) {
      maintError = errText(e);
      maint = null;
    } finally {
      maintBusy = false;
    }
  }

  async function checkUpdateNow() {
    updateChecking = true;
    updateInfo = null;
    updateStatus = '';
    try {
      const info = await api.checkUpdate();
      if (info) {
        updateInfo = info;
      } else {
        updateStatus = t('当前已是最新版本');
      }
    } catch (e) {
      updateStatus = `${t('检查失败')}: ${errText(e)}`;
    } finally {
      updateChecking = false;
    }
  }

  async function installNow() {
    updateInstalling = true;
    try {
      await api.installUpdate();
    } catch (e) {
      // 已经是最新版本不是故障，别把它显示成「更新失败」。
      updateStatus = errCode(e) === 'update-latest' ? t('当前已是最新版本') : `${t('更新失败')}: ${errText(e)}`;
      updateInstalling = false;
    }
  }
</script>

<section class="panel">
  <h3>{t('更新')}</h3>
  <div class="grid2" style="margin-bottom:12px">
    <label>
      <span>{t('下载通道')}</span>
      <select bind:value={settings.update_channel} onchange={onsave}>
        <option value="auto">{t('自动（官方→镜像回退）')}</option>
        <option value="official">{t('仅官方')}</option>
        <option value="mirror">{t('仅镜像')}</option>
      </select>
    </label>
  </div>
  <label class="check-row" style="margin-bottom:12px">
    <input type="checkbox" bind:checked={settings.update_use_proxy} onchange={onsave} />
    <span>{t('更新时使用系统代理（默认关闭：强制直连、跳过代理）')}</span>
  </label>
  <div class="row-actions">
    <button class="ghost" disabled={updateChecking} onclick={checkUpdateNow}>
      {updateChecking ? t('检查中') + '…' : t('检查更新')}
    </button>
    {#if updateInfo}
      <span class="hint">
        {t('发现新版本')} v{updateInfo.version}
        {#if updateInfo.notes}· {updateInfo.notes}{/if}
      </span>
      <button class="primary" disabled={updateInstalling} onclick={installNow}>
        {updateInstalling ? t('下载安装中') + '…' : t('立即更新')}
      </button>
    {:else if updateStatus}
      <span class="hint">{updateStatus}</span>
    {/if}
  </div>
  <p class="hint">{t('当前版本')} v{appVersion}. {t('更新包经数字签名校验')}</p>
</section>

<section class="panel">
  <h3>{t('备份与恢复（JSON）')}</h3>
  <p class="hint">{t('备份保存在数据目录')}</p>
  <div class="row-actions">
    <button class="ghost" onclick={exportBackup}>{t('导出全部数据')}</button>
    <button class="ghost" onclick={openBackupDir}>{t('打开备份目录')}</button>
  </div>
  <div class="row-actions">
    <button class="ghost" onclick={importBackup}>{t('从最新备份恢复')}</button>
    <button class="ghost" onclick={pickBackupFile}>{t('选择备份文件恢复')}</button>
  </div>
  <input
    class="backup-file-input"
    type="file"
    accept=".json,application/json"
    bind:this={fileInput}
    onchange={onBackupFile}
  />
  {#if backups.length}
    <div class="backup-list">
      {#each backups as b (b)}
        <div class="backup-item">
          <button class="backup-restore" onclick={() => restoreBackup(b)}>📄 {b}</button>
          <button class="backup-del" title={t('删除备份')} onclick={() => deleteBackup(b)}>✕</button>
        </div>
      {/each}
    </div>
  {:else}
    <p class="hint">—</p>
  {/if}
</section>

<section class="panel">
  <h3>{t('数据库维护')}</h3>
  <p class="hint">{t('压缩数据库并截断 WAL 日志文件')}</p>
  <div class="row-actions">
    <button class="ghost" disabled={maintBusy} onclick={runMaintenance}>
      {maintBusy ? t('处理中') : t('完整性检查 + 压缩 (VACUUM)')}
    </button>
  </div>
  {#if maint}
    <p class="hint">
      {t('完整性检查：{integrity} · 体积 {size} · 页数 {pages} · 空闲页 {free}', {
        integrity: maint.integrity,
        size: formatBytes(maint.db_bytes),
        pages: maint.page_count,
        free: maint.freelist,
      })}
    </p>
    <p class="hint">
      {maint.vacuum_ok ? t('压缩完成') : t('压缩失败')} ·
      {maint.checkpoint_ok ? t('WAL 已截断') : t('WAL 截断失败')}
    </p>
  {/if}
  {#if maintError}
    <p class="hint error-text">{maintError}</p>
  {/if}
</section>
