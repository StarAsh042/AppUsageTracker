import { cleanup, render, screen, waitFor } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { TrackerSettings } from './lib/api';
import SettingsView from './SettingsView.svelte';

// 结构契约：拆文件之前把每个面板的标题、控件、文案按出现顺序钉住。
// 拆分只许搬位置，任何一个面板的顺序、标签、属性变了都算改行为。
const m = vi.hoisted(() => ({
  getSettings: vi.fn(),
  listBackups: vi.fn(),
  getVersion: vi.fn(),
  saveSettings: vi.fn(),
  saveCategoryRules: vi.fn(),
}));

vi.mock('./lib/api', async (importOriginal) => {
  const actual = await importOriginal<typeof import('./lib/api')>();
  return {
    ...actual,
    api: {
      ...actual.api,
      getSettings: m.getSettings,
      listBackups: m.listBackups,
      getVersion: m.getVersion,
      saveSettings: m.saveSettings,
      saveCategoryRules: m.saveCategoryRules,
    },
  };
});

const settings: TrackerSettings = {
  idle_threshold_seconds: 300,
  foreground_poll_ms: 1000,
  start_with_windows: false,
  excluded_app_keys: ['c:\\windows\\explorer.exe'],
  data_directory: 'C:\\data',
  theme: 'dark',
  rest_reminder_minutes: 0,
  retention_days: 365,
  show_floating_widget: false,
  category_daily_limit_minutes: {},
  app_category_overrides: {},
  category_rules: [
    { name: '开发', keywords: ['code', 'cursor'] },
    { name: '浏览器', keywords: ['chrome'] },
  ],
  daily_goal_minutes: 0,
  hotkeys_enabled: true,
  hotkey_main: 'Ctrl+Alt+D',
  hotkey_widget: 'Ctrl+Alt+W',
  api_enabled: true,
  api_port: 59598,
  category_nature: {},
  app_display_names: {},
  record_window_titles: true,
  record_web: true,
  web_heartbeat_interval_secs: 30,
  title_exclude_patterns: ['secret'],
  language: 'zh-CN',
  update_channel: 'auto',
  update_use_proxy: false,
  schema_version: 1,
  widget: { always_on_top: false, locked: false, edge_snap: true, compact: false, opacity: 0.95, accent: 'blue' },
};

function attr(el: Element, name: string): string | undefined {
  const v = el.getAttribute(name);
  return v === null ? undefined : v;
}

/** 把一个元素压成稳定可读的一行，去掉随机与空值字段。 */
function describeControl(el: Element): Record<string, unknown> {
  const out: Record<string, unknown> = { tag: el.tagName.toLowerCase() };
  for (const name of ['type', 'min', 'max', 'step', 'rows', 'aria-label', 'placeholder', 'spellcheck']) {
    const v = attr(el, name);
    if (v !== undefined) out[name] = v;
  }
  if (el.tagName === 'INPUT' && (el as HTMLInputElement).type === 'checkbox') {
    out.checked = (el as HTMLInputElement).checked;
  }
  if (el.tagName === 'BUTTON') {
    out.text = el.textContent?.trim();
    // 去掉 Svelte 的作用域哈希（svelte-xxxxxx）：它随所在文件变化，不属于行为契约。
    out.class = el.className
      .split(/\s+/)
      .filter((c) => c && !c.startsWith('svelte-'))
      .join(' ');
    out.disabled = (el as HTMLButtonElement).disabled;
  }
  if (el.tagName === 'SELECT') {
    out.options = Array.from(el.querySelectorAll('option')).map((o) => o.textContent?.trim());
  }
  return out;
}

function serializeForm(root: HTMLElement) {
  return Array.from(root.querySelectorAll('section.panel')).map((panel) => {
    const heading = panel.querySelector('h3')?.textContent?.trim() ?? '';
    const labels = Array.from(panel.querySelectorAll('label > span')).map((s) => s.textContent?.trim());
    const hints = Array.from(panel.querySelectorAll('p.hint')).map((p) => p.textContent?.replace(/\s+/g, ' ').trim());
    const controls = Array.from(panel.querySelectorAll('input, textarea, select'))
      .filter((el) => el.getAttribute('type') !== 'file')
      .map(describeControl);
    const buttons = Array.from(panel.querySelectorAll('button')).map((b) => b.textContent?.trim());
    const options = Array.from(panel.querySelectorAll('select')).map((sel) =>
      Array.from(sel.querySelectorAll('option')).map((o) => o.textContent?.trim()),
    );
    return { heading, labels, hints, controls, buttons, options };
  });
}

afterEach(() => cleanup());

beforeEach(() => {
  vi.clearAllMocks();
  m.getSettings.mockResolvedValue({ ...settings });
  m.listBackups.mockResolvedValue([]);
  m.getVersion.mockResolvedValue('0.6.6');
});

describe('SettingsView 结构契约', () => {
  it('十一个面板的顺序、标签与控件保持不变', async () => {
    const { container } = render(SettingsView);
    await screen.findByText('保存设置');
    await waitFor(() => expect(container.querySelectorAll('section.panel').length).toBe(11));
    expect(serializeForm(container)).toMatchSnapshot();
  });
});
