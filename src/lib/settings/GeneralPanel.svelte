<script lang="ts">
  import { api, type TrackerSettings } from '../api';
  import { lang, translate } from '../i18n';

  let {
    settings = $bindable(),
    titlePatternsText = $bindable(''),
    onsave,
    onstatus,
  }: {
    settings: TrackerSettings;
    titlePatternsText: string;
    onsave: () => void;
    onstatus: (message: string) => void;
  } = $props();

  const t = $derived((key: string, params?: Record<string, string | number>) => translate($lang, key, params));

  async function openDataDir() {
    await api.openPath(settings.data_directory);
  }

  async function openLogDir() {
    await api.openPath(`${settings.data_directory}\\logs`);
  }

  async function clearData() {
    if (confirm(t('确定清空全部统计数据？此操作不可恢复。'))) {
      await api.clearAllData();
      onstatus(t('统计数据已清空'));
    }
  }
</script>

<section class="panel">
  <h3>{t('界面语言')}</h3>
  <div class="lang-switch">
    <button
      class:on={settings.language !== 'en'}
      onclick={() => {
        settings.language = 'zh-CN';
        onsave();
      }}
    >
      中文
    </button>
    <button
      class:on={settings.language === 'en'}
      onclick={() => {
        settings.language = 'en';
        onsave();
      }}
    >
      English
    </button>
  </div>
  <p class="hint" style="margin-top:6px">{t('语言切换即时生效，托盘菜单重启后应用')}</p>
</section>

<section class="panel">
  <h3>{t('常规')}</h3>
  <div class="grid2">
    <label>
      <span>{t('空闲阈值')}</span>
      <input type="number" min="30" max="3600" bind:value={settings.idle_threshold_seconds} />
    </label>
    <label>
      <span>{t('轮询间隔')}</span>
      <input type="number" min="250" max="10000" step="250" bind:value={settings.foreground_poll_ms} />
    </label>
    <label>
      <span>{t('休息提醒（分钟，0 = 关闭）')}</span>
      <input type="number" min="0" max="240" bind:value={settings.rest_reminder_minutes} />
    </label>
    <label>
      <span>{t('数据保留')}</span>
      <input type="number" min="30" max="3650" bind:value={settings.retention_days} />
    </label>
    <label>
      <span>{t('每日活跃目标')}</span>
      <input type="number" min="0" max="1440" bind:value={settings.daily_goal_minutes} />
    </label>
  </div>
  <div class="checks">
    <label class="check-row">
      <input type="checkbox" bind:checked={settings.start_with_windows} />
      <span>{t('开机自动启动')}</span>
    </label>
    <label class="check-row">
      <input type="checkbox" bind:checked={settings.show_floating_widget} />
      <span>{t('显示悬浮窗')}</span>
    </label>
    <label class="check-row">
      <input type="checkbox" bind:checked={settings.hotkeys_enabled} />
      <span>{t('启用全局热键')}</span>
    </label>
    {#if settings.hotkeys_enabled}
      <div class="grid2" style="margin-top:8px">
        <label>
          <span>{t('主界面热键')}</span>
          <input type="text" bind:value={settings.hotkey_main} spellcheck="false" />
        </label>
        <label>
          <span>{t('悬浮窗热键')}</span>
          <input type="text" bind:value={settings.hotkey_widget} spellcheck="false" />
        </label>
      </div>
      <p class="hint" style="margin:8px 0 0">{t('组合写法如 Ctrl+Alt+D / Ctrl+Shift+P，保存时校验有效性。')}</p>
    {/if}
    <label class="check-row">
      <input type="checkbox" bind:checked={settings.api_enabled} />
      <span>{t('本地：{port}，供第三方工具读取统计', { port: settings.api_port })}</span>
    </label>
  </div>
  {#if settings.api_enabled}
    <div class="grid2">
      <label>
        <span>{t('API 端口')}</span>
        <input type="number" min="1024" max="65535" bind:value={settings.api_port} />
      </label>
    </div>
  {/if}
  <div class="grid2">
    <label>
      <span>{t('数据目录')}</span>
      <input type="text" bind:value={settings.data_directory} />
    </label>
  </div>
  <div class="row-actions">
    <button class="ghost" onclick={openDataDir}>{t('打开数据目录')}</button>
    <button class="ghost" onclick={openLogDir}>{t('打开日志目录')}</button>
    <button class="danger" onclick={clearData}>{t('清空全部数据')}</button>
  </div>
</section>

<section class="panel">
  <h3>{t('隐私：窗口标题记录')}</h3>
  <label class="check-row" style="margin-bottom:10px">
    <input type="checkbox" bind:checked={settings.record_window_titles} />
    <span>{t('记录前台窗口标题')}</span>
  </label>
  <label class="check-row">
    <input type="checkbox" bind:checked={settings.record_web} />
    <span>{t('网页浏览统计（需安装浏览器扩展）')}</span>
  </label>
  {#if settings.record_web}
    <div class="grid2" style="margin-top:8px">
      <label>
        <span>{t('网页心跳间隔（秒，MV3 下限 30）')}</span>
        <input type="number" min="30" max="3600" bind:value={settings.web_heartbeat_interval_secs} />
      </label>
    </div>
  {/if}
  {#if settings.record_window_titles}
    <p class="hint">{t('排除规则——每行一条子串。')}</p>
    <textarea rows="4" bind:value={titlePatternsText} spellcheck="false" placeholder={t('排除规则示例')}></textarea>
  {/if}
</section>
