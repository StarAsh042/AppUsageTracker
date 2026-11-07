<script lang="ts">
  import {
    api,
    type TrackerSettings,
    type CategoryRule,
    type ScannedApp,
    type UpdateInfo,
  } from './lib/api';
  import { lang, en } from './lib/i18n';

  let s = $state<TrackerSettings | null>(null);
  let rules = $state<{ name: string; keywords: string }[]>([]);
  let excludedText = $state('');
  let status = $state('');
  let saving = $state(false);
  let backups = $state<string[]>([]);
  let titlePatternsText = $state('');
  let appVersion = $state('');
  let updateInfo = $state<UpdateInfo | null>(null);
  let updateStatus = $state('');
  let updateChecking = $state(false);
  let updateInstalling = $state(false);

  const t = $derived((s: string) => ($lang === 'en' ? (en[s] ?? s) : s));
  let scanned = $state<ScannedApp[] | null>(null);
  let scanning = $state(false);

  function natureOf(name: string): string {
    return s?.category_nature[name] ?? 'neutral';
  }

  function setNature(name: string, val: string) {
    if (!s) return;
    if (!val || val === 'neutral') {
      delete s.category_nature[name];
    } else {
      s.category_nature[name] = val;
    }
  }

  const NATURE_LABELS: Record<string, string> = {
    productive: '产出',
    neutral: '中性',
    distracting: '分心',
  };

  async function runScan() {
    scanning = true;
    try {
      scanned = await api.scanInstalledApps();
    } catch (e) {
      status = String(e);
    } finally {
      scanning = false;
    }
  }

  async function assignCategory(appKey: string, cat: string) {
    await api.setAppCategory(appKey, cat);
    if (scanned) {
      scanned = scanned.map((a) => (a.app_key === appKey ? { ...a, category: cat } : a));
    }
    status = '分类已更新';
  }

  $effect(() => {
    (async () => {
      const st = await api.getSettings();
      s = st;
      rules = st.category_rules.map((r) => ({ name: r.name, keywords: r.keywords.join('|') }));
      excludedText = st.excluded_app_keys.join('\n');
      titlePatternsText = st.title_exclude_patterns.join('\n');
      backups = await api.listBackups();
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
      status = $lang === 'en' ? 'Saved. Changes apply immediately' : '已保存，设置即时生效';
    } catch (e) {
      status = `保存失败: ${e}`;
    } finally {
      saving = false;
    }
  }

  function addRule() {
    rules = [...rules, { name: '', keywords: '' }];
  }

  function delRule(i: number) {
    rules = rules.filter((_, idx) => idx !== i);
  }

  async function saveRules() {
    status = '';
    try {
      const out: CategoryRule[] = [];
      const seen = new Set<string>();
      for (const r of rules) {
        const name = r.name.trim();
        const keywords = r.keywords
          .split('|')
          .map((x) => x.trim())
          .filter(Boolean);
        if (!name && !keywords.length) continue; // 空行跳过
        if (!name) throw new Error('分类名不能为空');
        if (seen.has(name)) throw new Error(`分类「${name}」重复`);
        seen.add(name);
        if (!keywords.length) throw new Error(`分类「${name}」至少需要一个关键词`);
        out.push({ name, keywords });
      }
      if (!out.length) throw new Error('至少保留一条分类规则');
      await api.saveCategoryRules(out);
      if (s) s.category_rules = out;
      status = '分类规则已保存';
    } catch (e) {
      status = String(e);
    }
  }

  async function exportBackup() {
    status = '';
    try {
      const path = await api.exportBackupJson();
      backups = await api.listBackups();
      status = `已导出: ${path}`;
    } catch (e) {
      status = String(e);
    }
  }

  async function deleteBackup(name: string) {
    if (!confirm(`${t('删除备份')} ${name}？`)) return;
    try {
      await api.deleteBackup(name);
      backups = await api.listBackups();
      status = t('备份已删除');
    } catch (e) {
      status = String(e);
    }
  }

  async function importBackup() {
    if (!confirm('导入将清空当前全部数据并以最新备份替换，确定继续？')) return;
    status = '';
    try {
      status = await api.importBackupJson();
    } catch (e) {
      status = String(e);
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
        updateStatus = '当前已是最新版本';
      }
    } catch (e) {
      updateStatus = `检查失败: ${e}`;
    } finally {
      updateChecking = false;
    }
  }

  async function installNow() {
    updateInstalling = true;
    try {
      await api.installUpdate();
    } catch (e) {
      updateStatus = `更新失败: ${e}`;
      updateInstalling = false;
    }
  }

  async function openExtensionsPage() {
    try {
      await api.openPath('chrome://extensions');
    } catch {
      // chrome:// 协议无法从外部打开时退化
      window.open('chrome://extensions', '_blank');
    }
  }

  async function openExtensionFolder() {
    await api.openPath('extension');
  }

  async function clearData() {
    if (confirm('确定清空全部统计数据？此操作不可恢复。')) {
      await api.clearAllData();
      status = '统计数据已清空';
    }
  }

  async function openDataDir() {
    if (s) await api.openPath(s.data_directory);
  }

  function limitOf(name: string): number {
    return s?.category_daily_limit_minutes[name] ?? 0;
  }

  function setLimit(name: string, val: number) {
    if (!s) return;
    if (val > 0) {
      s.category_daily_limit_minutes[name] = val;
    } else {
      delete s.category_daily_limit_minutes[name];
    }
  }

  function ruleNames(): string[] {
    return s?.category_rules.map((r) => r.name) ?? [];
  }
</script>

<div class="settings">
  {#if s}
    <div class="sections">
      <section class="panel">
        <h3>{t('界面语言')}</h3>
        <div class="lang-switch">
          <button class:on={s?.language !== 'en'} onclick={() => { if (s) { s.language = 'zh-CN'; save(); } }}>中文</button>
          <button class:on={s?.language === 'en'} onclick={() => { if (s) { s.language = 'en'; save(); } }}>English</button>
        </div>
        <p class="hint" style="margin-top:6px">{t('语言切换即时生效，托盘菜单重启后应用')}</p>
      </section>

      <section class="panel">
        <h3>{t('常规')}</h3>
        <div class="grid2">
          <label>
            <span>{t('空闲阈值')}</span>
            <input type="number" min="30" max="3600" bind:value={s.idle_threshold_seconds} />
          </label>
          <label>
            <span>{t('轮询间隔')}</span>
            <input type="number" min="250" max="10000" step="250" bind:value={s.foreground_poll_ms} />
          </label>
          <label>
            <span>{t('休息提醒')}（{t('分钟')}，0 = {t('关闭')}）</span>
            <input type="number" min="0" max="240" bind:value={s.rest_reminder_minutes} />
          </label>
          <label>
            <span>{t('数据保留')}</span>
            <input type="number" min="30" max="3650" bind:value={s.retention_days} />
          </label>
          <label>
            <span>{t('每日活跃目标')}</span>
            <input type="number" min="0" max="1440" bind:value={s.daily_goal_minutes} />
          </label>
        </div>
        <div class="checks">
          <label class="check-row">
            <input type="checkbox" bind:checked={s.start_with_windows} />
            <span>{t('开机自动启动')}</span>
          </label>
          <label class="check-row">
            <input type="checkbox" bind:checked={s.show_floating_widget} />
            <span>{t('显示悬浮窗')}</span>
          </label>
          <label class="check-row">
            <input type="checkbox" bind:checked={s.hotkeys_enabled} />
            <span>{t('启用全局热键')}</span>
          </label>
          {#if s.hotkeys_enabled}
            <div class="grid2" style="margin-top:8px">
              <label>
                <span>{t('主界面热键')}</span>
                <input type="text" bind:value={s.hotkey_main} spellcheck="false" />
              </label>
              <label>
                <span>{t('悬浮窗热键')}</span>
                <input type="text" bind:value={s.hotkey_widget} spellcheck="false" />
              </label>
            </div>
            <p class="hint" style="margin:8px 0 0">组合写法如 Ctrl+Alt+D / Ctrl+Shift+P，保存时校验有效性。</p>
          {/if}
          <label class="check-row">
            <input type="checkbox" bind:checked={s.api_enabled} />
            <span>{t('本地')}:{s.api_port}，{t('供第三方工具读取统计')}</span>
          </label>
        </div>
        {#if s.api_enabled}
          <div class="grid2">
            <label>
              <span>{t('API 端口')}</span>
              <input type="number" min="1024" max="65535" bind:value={s.api_port} />
            </label>
          </div>
        {/if}
        <div class="grid2">
          <label>
            <span>{t('数据目录')}</span>
            <input type="text" bind:value={s.data_directory} />
          </label>
        </div>
        <div class="row-actions">
          <button class="ghost" onclick={openDataDir}>{t('打开数据目录')}</button>
          <button class="danger" onclick={clearData}>{t('清空全部数据')}</button>
        </div>
      </section>

      <section class="panel">
        <h3>{t('隐私')}：{t('窗口标题记录')}</h3>
        <label class="check-row" style="margin-bottom:10px">
          <input type="checkbox" bind:checked={s.record_window_titles} />
          <span>{t('记录前台窗口标题')}</span>
        </label>
        <label class="check-row">
          <input type="checkbox" bind:checked={s.record_web} />
          <span>{t('网页浏览统计')}（{t('需安装浏览器扩展')}）</span>
        </label>
        {#if s.record_web}
          <div class="grid2" style="margin-top:8px">
            <label>
              <span>{t('网页心跳间隔（秒，MV3 下限 30）')}</span>
              <input type="number" min="30" max="3600" bind:value={s.web_heartbeat_interval_secs} />
            </label>
          </div>
        {/if}
        {#if s.record_window_titles}
          <p class="hint">{t('排除规则')}——{t('每行一条子串')}。</p>
          <textarea
            rows="4"
            bind:value={titlePatternsText}
            spellcheck="false"
            placeholder={t('排除规则示例')}
          ></textarea>
        {/if}
      </section>

      <section class="panel">
        <h3>{t('浏览器扩展')}</h3>
        <p class="hint">{t('安装说明')}</p>
        <div class="row-actions">
          <button class="ghost" onclick={openExtensionsPage}>{t('打开浏览器扩展页')}</button>
          <button class="ghost" onclick={openExtensionFolder}>{t('打开扩展目录')}</button>
        </div>
      </section>

      <section class="panel">
        <h3>{t('排除应用')}</h3>
        <p class="hint">{t('每行一个')}。</p>
        <textarea rows="5" bind:value={excludedText} spellcheck="false"></textarea>
      </section>

      <section class="panel">
        <h3>{t('分类每日限额')}（{t('分钟')}，0 = {t('不限')}）</h3>
        <div class="limits">
          {#each ruleNames() as name (name)}
            <label class="limit-row">
              <span>{name}</span>
              <input
                type="number"
                min="0"
                max="1440"
                value={limitOf(name)}
                oninput={(e) => setLimit(name, Number((e.currentTarget as HTMLInputElement).value))}
              />
              <select
                value={natureOf(name)}
                onchange={(e) => setNature(name, (e.currentTarget as HTMLSelectElement).value)}
              >
                {#each Object.entries(NATURE_LABELS) as [val, label] (val)}
                  <option value={val}>{t(label)}</option>
                {/each}
              </select>
            </label>
          {/each}
          <p class="hint">{t('产出')} / {t('中性')} / {t('分心')} → {t('效率趋势')}。</p>
        </div>
      </section>

      <section class="panel">
        <h3>{t('分类规则')}</h3>
        <p class="hint">{t('关键词以')} — {t('层级')}。</p>
        <div class="rules-editor">
          {#each rules as r, i (`${r.name}-${i}`)}
            <div class="rule-row">
              <input class="rule-name" placeholder={t('分类名')} bind:value={r.name} spellcheck="false" />
              <input class="rule-kw" placeholder={t('关键词1')} bind:value={r.keywords} spellcheck="false" />
              <button class="rule-del" title="删除分类" onclick={() => delRule(i)}>✕</button>
            </div>
          {/each}
        </div>
        <div class="row-actions">
          <button class="ghost" onclick={addRule}>+ {t('新增分类')}</button>
          <button class="ghost" onclick={saveRules}>{t('保存分类规则')}</button>
        </div>
      </section>

      <section class="panel">
        <h3>{t('扫描应用补分类')}</h3>
        <p class="hint">{t('扫描桌面与开始菜单快捷方式')}</p>
        <div class="row-actions">
          <button class="ghost" disabled={scanning} onclick={runScan}>{scanning ? t('扫描中') + '…' : t('扫描应用')}</button>
          {#if scanned}
            <span class="hint">{t('共')} {scanned.length} {t('个应用')}，{t('其中未分类')} {scanned.filter((a) => a.category === '未分类').length}</span>
          {/if}
        </div>
        {#if scanned && scanned.length}
          <div class="scan-list">
            {#each scanned as a (a.app_key)}
              <div class="scan-row" class:uncat={a.category === '未分类'}>
                <span class="scan-name" title={a.app_key}>{a.display_name} <span class="scan-src">{a.source}</span></span>
                <span class="cat-chip">{a.category}</span>
                <select
                  value={a.category === '未分类' ? '' : a.category}
                  onchange={(e) => assignCategory(a.app_key, (e.currentTarget as HTMLSelectElement).value || '未分类')}
                >
                  <option value="">{t('设为分类')}</option>
                  {#each ruleNames() as name (name)}
                    <option value={name}>{name}</option>
                  {/each}
                </select>
              </div>
            {/each}
          </div>
        {/if}
      </section>

      <section class="panel">
        <h3>{t('更新')}</h3>
        <div class="grid2" style="margin-bottom:12px">
          <label>
            <span>{t('下载通道')}</span>
            <select bind:value={s.update_channel} onchange={save}>
              <option value="auto">{t('自动（官方→镜像回退）')}</option>
              <option value="official">{t('仅官方')}</option>
              <option value="mirror">{t('仅镜像')}</option>
            </select>
          </label>
        </div>
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
        <h3>{t('备份与恢复')}（JSON）</h3>
        <p class="hint">{t('备份保存在数据目录')}</p>
        <div class="row-actions">
          <button class="ghost" onclick={exportBackup}>{t('导出全部数据')}</button>
          <button class="ghost" onclick={importBackup}>{t('从最新备份恢复')}</button>
        </div>
        {#if backups.length}
          <div class="backup-list">
            {#each backups.slice(0, 5) as b (b)}
              <div class="backup-item">
                📄 {b}
                <button
                  class="backup-del"
                  title={t('删除备份')}
                  onclick={() => deleteBackup(b)}
                >✕</button>
              </div>
            {/each}
          </div>
        {:else}
          <p class="hint">—</p>
        {/if}
      </section>

      <section class="panel">
        <h3>{t('外观')}</h3>
        <div class="theme-row">
          <label class="check-row">
            <input
              type="radio"
              name="theme"
              checked={s.theme !== 'light'}
              onchange={() => {
                if (s) s.theme = 'dark';
              }}
            />
            <span>深色</span>
          </label>
          <label class="check-row">
            <input
              type="radio"
              name="theme"
              checked={s.theme === 'light'}
              onchange={() => {
                if (s) s.theme = 'light';
              }}
            />
            <span>浅色</span>
          </label>
        </div>
      </section>
    </div>

    <div class="save-bar">
      {#if status}<span class="status">{status}</span>{/if}
      <button class="primary" disabled={saving} onclick={save}>{t('保存设置')}</button>
    </div>
  {:else}
    <div class="loading">{t('加载中')}…</div>
  {/if}
</div>

<style>
  .settings {
    height: 100%;
    display: flex;
    flex-direction: column;
    padding: 14px 18px;
    gap: 12px;
    overflow: hidden;
  }

  .sections {
    flex: 1;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding-right: 4px;
  }

  .panel {
    background: var(--glass-card, var(--card));
    border: 1px solid var(--border);
    border-radius: 12px;
    padding: 14px 16px;
  }

  h3 {
    margin: 0 0 12px;
    font-size: 14px;
  }

  .hint {
    margin: 0 0 10px;
    color: var(--text-dim);
    font-size: 12px;
  }

  .grid2 {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px 16px;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 13px;
    color: var(--text-dim);
  }

  input[type='number'],
  input[type='text'] {
    background: var(--card2);
    border: 1px solid var(--border);
    color: var(--text);
    border-radius: 7px;
    padding: 7px 10px;
    font-size: 13px;
  }

  input:focus,
  textarea:focus {
    outline: none;
    border-color: var(--accent);
  }

  .checks {
    display: flex;
    gap: 24px;
    margin: 12px 0;
  }

  .check-row {
    flex-direction: row;
    align-items: center;
    gap: 8px;
    color: var(--text);
    cursor: pointer;
  }

  .check-row input {
    accent-color: var(--accent);
  }

  textarea {
    width: 100%;
    background: var(--card2);
    border: 1px solid var(--border);
    color: var(--text);
    border-radius: 8px;
    padding: 10px;
    font-size: 12px;
    font-family: Consolas, 'Microsoft YaHei', monospace;
    line-height: 1.6;
    resize: vertical;
  }

  .rules-editor {
    display: flex;
    flex-direction: column;
    gap: 6px;
    max-height: 320px;
    overflow-y: auto;
    padding-right: 4px;
  }

  .rule-row {
    display: grid;
    grid-template-columns: 110px 1fr 34px;
    gap: 8px;
    align-items: center;
  }

  .rule-name,
  .rule-kw {
    background: var(--card2);
    border: 1px solid var(--border);
    color: var(--text);
    border-radius: 7px;
    padding: 6px 10px;
    font-size: 12px;
  }

  .rule-del {
    background: transparent;
    border: 1px solid var(--border);
    color: var(--danger);
    border-radius: 7px;
    padding: 5px 0;
    cursor: pointer;
    font-size: 12px;
  }

  .rule-del:hover {
    background: rgba(255, 95, 95, 0.12);
  }

  .backup-list {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin-top: 10px;
  }

  .backup-item {
    font-size: 12px;
    color: var(--text-dim);
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
  }

  .backup-del {
    background: transparent;
    border: 1px solid var(--border);
    color: var(--danger);
    border-radius: 6px;
    padding: 2px 8px;
    cursor: pointer;
    font-size: 11px;
  }

  .backup-del:hover {
    background: rgba(255, 95, 95, 0.12);
  }

  .theme-row {
    display: flex;
    gap: 24px;
  }

  .lang-switch {
    display: inline-flex;
    gap: 4px;
  }

  .lang-switch button {
    background: var(--card2);
    border: 1px solid var(--border);
    color: var(--text-dim);
    border-radius: 7px;
    padding: 8px 22px;
    font-size: 13px;
    cursor: pointer;
  }

  .lang-switch button.on {
    background: var(--accent);
    border-color: var(--accent);
    color: #fff;
  }

  select {
    background: var(--card2);
    border: 1px solid var(--border);
    color: var(--text);
    border-radius: 7px;
    padding: 7px 8px;
    font-size: 12px;
  }

  .limits {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 10px 16px;
  }

  .limit-row select {
    background: var(--card2);
    border: 1px solid var(--border);
    color: var(--text);
    border-radius: 7px;
    padding: 7px 8px;
    font-size: 12px;
  }

  .scan-list {
    display: flex;
    flex-direction: column;
    gap: 5px;
    max-height: 260px;
    overflow-y: auto;
    margin-top: 10px;
  }

  .scan-row {
    display: grid;
    grid-template-columns: 1fr 110px 130px;
    gap: 10px;
    align-items: center;
    padding: 6px 8px;
    border-radius: 8px;
    background: var(--card2);
    font-size: 13px;
  }

  .scan-row.uncat {
    outline: 1px solid rgba(245, 166, 35, 0.45);
  }

  .scan-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .scan-src {
    color: var(--text-dim);
    font-size: 11px;
  }

  .scan-row select {
    background: var(--glass-card, var(--card));
    border: 1px solid var(--border);
    color: var(--text);
    border-radius: 7px;
    padding: 5px 8px;
    font-size: 12px;
  }

  .limit-row span {
    font-size: 13px;
  }

  .row-actions {
    display: flex;
    gap: 10px;
    margin-top: 12px;
  }

  .ghost {
    background: transparent;
    border: 1px solid var(--border);
    color: var(--text-dim);
    border-radius: 7px;
    padding: 7px 14px;
    cursor: pointer;
    font-size: 13px;
  }

  .ghost:hover {
    color: var(--text);
    border-color: var(--accent);
  }

  .danger {
    background: rgba(255, 95, 95, 0.12);
    border: 1px solid rgba(255, 95, 95, 0.4);
    color: var(--danger);
    border-radius: 7px;
    padding: 7px 14px;
    cursor: pointer;
    font-size: 13px;
  }

  .danger:hover {
    background: rgba(255, 95, 95, 0.2);
  }

  .save-bar {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 12px;
    border-top: 1px solid var(--border);
    padding-top: 10px;
  }

  .status {
    color: var(--ok);
    font-size: 13px;
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

  .primary:disabled {
    opacity: 0.6;
  }

  .loading {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-dim);
  }
</style>
