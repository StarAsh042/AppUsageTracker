<script lang="ts">
  import { api, type TrackerSettings, type CategoryRule } from './lib/api';

  let s = $state<TrackerSettings | null>(null);
  let rulesText = $state('');
  let excludedText = $state('');
  let status = $state('');
  let saving = $state(false);

  $effect(() => {
    (async () => {
      const st = await api.getSettings();
      s = st;
      rulesText = st.category_rules.map((r) => `${r.name}=${r.keywords.join('|')}`).join('\n');
      excludedText = st.excluded_app_keys.join('\n');
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
      await api.saveSettings(snapshot);
      status = '已保存，设置即时生效';
    } catch (e) {
      status = `保存失败: ${e}`;
    } finally {
      saving = false;
    }
  }

  async function saveRules() {
    status = '';
    try {
      const rules: CategoryRule[] = [];
      for (const line of rulesText.split(/\r?\n/)) {
        const raw = line.trim();
        if (!raw) continue;
        const eq = raw.indexOf('=');
        if (eq < 0) throw new Error(`行格式错误:「${raw}」，应为 名称=关键词1|关键词2`);
        const name = raw.slice(0, eq).trim();
        if (!name) throw new Error('分类名不能为空');
        const keywords = raw
          .slice(eq + 1)
          .split('|')
          .map((x) => x.trim())
          .filter(Boolean);
        if (!keywords.length) throw new Error(`分类「${name}」至少需要一个关键词`);
        rules.push({ name, keywords });
      }
      if (!rules.length) throw new Error('至少保留一条分类规则');
      await api.saveCategoryRules(rules);
      status = '分类规则已保存';
    } catch (e) {
      status = String(e);
    }
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
        <h3>常规</h3>
        <div class="grid2">
          <label>
            <span>空闲阈值（秒）</span>
            <input type="number" min="30" max="3600" bind:value={s.idle_threshold_seconds} />
          </label>
          <label>
            <span>轮询间隔（毫秒）</span>
            <input type="number" min="250" max="10000" step="250" bind:value={s.foreground_poll_ms} />
          </label>
          <label>
            <span>休息提醒（分钟，0 = 关闭）</span>
            <input type="number" min="0" max="240" bind:value={s.rest_reminder_minutes} />
          </label>
          <label>
            <span>数据保留（天）</span>
            <input type="number" min="30" max="3650" bind:value={s.retention_days} />
          </label>
        </div>
        <div class="checks">
          <label class="check-row">
            <input type="checkbox" bind:checked={s.start_with_windows} />
            <span>开机自动启动</span>
          </label>
          <label class="check-row">
            <input type="checkbox" bind:checked={s.show_floating_widget} />
            <span>显示悬浮窗</span>
          </label>
        </div>
        <div class="grid2">
          <label>
            <span>数据目录</span>
            <input type="text" bind:value={s.data_directory} />
          </label>
        </div>
        <div class="row-actions">
          <button class="ghost" onclick={openDataDir}>打开数据目录</button>
          <button class="danger" onclick={clearData}>清空全部数据</button>
        </div>
      </section>

      <section class="panel">
        <h3>排除应用</h3>
        <p class="hint">每行一个 exe 完整路径（小写），这些应用不计入统计。</p>
        <textarea rows="5" bind:value={excludedText} spellcheck="false"></textarea>
      </section>

      <section class="panel">
        <h3>分类每日限额（分钟，0 = 不限）</h3>
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
            </label>
          {/each}
        </div>
      </section>

      <section class="panel">
        <h3>分类规则</h3>
        <p class="hint">格式：分类名=关键词1|关键词2（每行一条，匹配应用名 / 进程名 / 路径，不区分大小写）。</p>
        <textarea rows="9" bind:value={rulesText} spellcheck="false"></textarea>
        <div class="row-actions">
          <button class="ghost" onclick={saveRules}>保存分类规则</button>
        </div>
      </section>
    </div>

    <div class="save-bar">
      {#if status}<span class="status">{status}</span>{/if}
      <button class="primary" disabled={saving} onclick={save}>保存设置</button>
    </div>
  {:else}
    <div class="loading">加载中…</div>
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
    background: var(--card);
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

  .limits {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 10px 16px;
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
