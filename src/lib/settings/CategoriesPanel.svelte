<script lang="ts">
  import { api, UNCATEGORIZED, categoryLabel, type TrackerSettings, type CategoryRule, type ScannedApp } from '../api';
  import { lang, translate } from '../i18n';
  import { errText } from '../errors';
  import { limitOf as limitOfSetting, setDailyLimit } from '../limits';

  let { settings = $bindable(), onstatus }: { settings: TrackerSettings; onstatus: (message: string) => void } =
    $props();

  let rules = $state<{ name: string; keywords: string }[]>(
    settings.category_rules.map((r) => ({ name: r.name, keywords: r.keywords.join('|') })),
  );
  let scanned = $state<ScannedApp[] | null>(null);
  let scanning = $state(false);

  const t = $derived((key: string, params?: Record<string, string | number>) => translate($lang, key, params));

  const NATURE_LABELS: Record<string, string> = {
    productive: '产出',
    neutral: '中性',
    distracting: '分心',
  };

  function natureOf(name: string): string {
    return settings.category_nature[name] ?? 'neutral';
  }

  function setNature(name: string, val: string) {
    if (!val || val === 'neutral') {
      delete settings.category_nature[name];
    } else {
      settings.category_nature[name] = val;
    }
  }

  function ruleNames(): string[] {
    return settings.category_rules.map((r) => r.name);
  }

  function limitOf(name: string): number {
    return limitOfSetting(settings.category_daily_limit_minutes, name);
  }

  function setLimit(name: string, val: number) {
    setDailyLimit(settings.category_daily_limit_minutes, name, val);
  }

  function addRule() {
    rules = [...rules, { name: '', keywords: '' }];
  }

  function delRule(i: number) {
    rules = rules.filter((_, idx) => idx !== i);
  }

  async function saveRules() {
    onstatus('');
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
        if (!name) throw new Error(t('分类名不能为空'));
        if (seen.has(name)) throw new Error(t('分类「{name}」重复', { name }));
        seen.add(name);
        if (!keywords.length) throw new Error(t('分类「{name}」至少需要一个关键词', { name }));
        out.push({ name, keywords });
      }
      if (!out.length) throw new Error(t('至少保留一条分类规则'));
      await api.saveCategoryRules(out);
      settings.category_rules = out;
      onstatus(t('分类规则已保存'));
    } catch (e) {
      onstatus(errText(e));
    }
  }

  async function runScan() {
    scanning = true;
    try {
      scanned = await api.scanInstalledApps();
    } catch (e) {
      onstatus(errText(e));
    } finally {
      scanning = false;
    }
  }

  async function assignCategory(appKey: string, cat: string) {
    await api.setAppCategory(appKey, cat);
    if (scanned) {
      scanned = scanned.map((a) => (a.app_key === appKey ? { ...a, category: cat } : a));
    }
    onstatus(t('分类已更新'));
  }
</script>

<section class="panel">
  <h3>{t('分类每日限额（分钟，0 = 不限）')}</h3>
  <div class="limits">
    {#each ruleNames() as name (name)}
      <label class="limit-row">
        <span>{categoryLabel(name, $lang)}</span>
        <input
          type="number"
          min="0"
          max="1440"
          value={limitOf(name)}
          oninput={(e) => setLimit(name, Number((e.currentTarget as HTMLInputElement).value))}
        />
        <select value={natureOf(name)} onchange={(e) => setNature(name, (e.currentTarget as HTMLSelectElement).value)}>
          {#each Object.entries(NATURE_LABELS) as [val, label] (val)}
            <option value={val}>{t(label)}</option>
          {/each}
        </select>
      </label>
    {/each}
    <p class="hint">{t('产出 / 中性 / 分心 → 效率趋势。')}</p>
  </div>
</section>

<section class="panel">
  <h3>{t('分类规则')}</h3>
  <p class="hint">{t('关键词以 | 分隔，可写「父—子」层级。')}</p>
  <div class="rules-editor">
    {#each rules as r, i (`${r.name}-${i}`)}
      <div class="rule-row">
        <input class="rule-name" placeholder={t('分类名')} bind:value={r.name} spellcheck="false" />
        <input class="rule-kw" placeholder={t('关键词1')} bind:value={r.keywords} spellcheck="false" />
        <button class="rule-del" title={t('删除分类')} onclick={() => delRule(i)}>✕</button>
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
    <button class="ghost" disabled={scanning} onclick={runScan}>
      {scanning ? t('扫描中') + '…' : t('扫描应用')}
    </button>
    {#if scanned}
      <span class="hint">
        {t('共 {n} 个应用，其中未分类 {m}', {
          n: scanned.length,
          m: scanned.filter((a) => a.category === UNCATEGORIZED).length,
        })}
      </span>
    {/if}
  </div>
  {#if scanned && scanned.length}
    <div class="scan-list">
      {#each scanned as a (a.app_key)}
        <div class="scan-row" class:uncat={a.category === UNCATEGORIZED}>
          <span class="scan-name" title={a.app_key}>
            {a.display_name} <span class="scan-src">{a.source}</span>
          </span>
          <span class="cat-chip">{categoryLabel(a.category, $lang)}</span>
          <select
            value={a.category === UNCATEGORIZED ? '' : a.category}
            onchange={(e) => assignCategory(a.app_key, (e.currentTarget as HTMLSelectElement).value || UNCATEGORIZED)}
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
