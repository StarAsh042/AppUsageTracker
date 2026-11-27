import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { TrackerSettings } from './lib/api';
import SettingsView from './SettingsView.svelte';

// 锁的是用户能看到的行为：加载中不先闪空表单、保存成功有回执、
// 失败把后端给的原话显示出来、多行输入框里的内容按行规整后再提交。
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
  excluded_app_keys: [],
  data_directory: '',
  theme: 'dark',
  rest_reminder_minutes: 0,
  retention_days: 365,
  show_floating_widget: false,
  category_daily_limit_minutes: {},
  app_category_overrides: {},
  category_rules: [],
  daily_goal_minutes: 0,
  hotkeys_enabled: false,
  hotkey_main: '',
  hotkey_widget: '',
  api_enabled: true,
  api_port: 59598,
  category_nature: {},
  app_display_names: {},
  record_window_titles: false,
  record_web: false,
  web_heartbeat_interval_secs: 30,
  title_exclude_patterns: [],
  language: 'zh-CN',
  update_channel: 'stable',
  update_use_proxy: false,
  schema_version: 1,
  widget: { always_on_top: false, locked: false, edge_snap: true, compact: false, opacity: 0.95, accent: 'blue' },
};

// 没开 vitest globals，@testing-library 的自动清理不会注册，得自己卸掉上一个用例的 DOM。
afterEach(() => cleanup());

beforeEach(() => {
  vi.clearAllMocks();
  m.getSettings.mockResolvedValue({ ...settings });
  m.listBackups.mockResolvedValue([]);
  m.getVersion.mockResolvedValue('0.6.6');
  m.saveSettings.mockResolvedValue(undefined);
  m.saveCategoryRules.mockResolvedValue(undefined);
});

describe('SettingsView', () => {
  it('首帧显示加载中而不是空表单', async () => {
    let settle: (v: TrackerSettings) => void = () => {};
    m.getSettings.mockReturnValue(new Promise<TrackerSettings>((r) => (settle = r)));
    render(SettingsView);
    expect(await screen.findByText('加载中…')).toBeTruthy();
    expect(screen.queryByText('保存设置')).toBeNull();
    settle({ ...settings });
    await waitFor(() => expect(screen.queryByText('加载中…')).toBeNull());
  });

  it('取到设置后显示保存按钮与当前版本', async () => {
    render(SettingsView);
    expect(await screen.findByText('保存设置')).toBeTruthy();
    // 版本号是在备份列表之后才取回来的，出现得比表单晚一拍。
    await waitFor(() => expect(screen.getByText(/v0\.6\.6/)).toBeTruthy());
  });

  it('保存成功后给出已保存回执', async () => {
    render(SettingsView);
    await fireEvent.click(await screen.findByText('保存设置'));
    expect(await screen.findByText('已保存，设置即时生效')).toBeTruthy();
  });

  it('保存失败时显示后端错误原文', async () => {
    m.saveSettings.mockRejectedValueOnce({ code: 'hotkey-invalid', message: '主界面热键无效' });
    render(SettingsView);
    await fireEvent.click(await screen.findByText('保存设置'));
    // 展示拿 message，不把 code 摊给用户看。
    expect(await screen.findByText('保存失败: 主界面热键无效')).toBeTruthy();
    expect(screen.queryByText(/hotkey-invalid/)).toBeNull();
  });

  it('保存时把多行排除项按行规整后提交', async () => {
    render(SettingsView);
    await screen.findByText('保存设置');
    const box = screen.getByLabelText('排除应用') as HTMLTextAreaElement;
    await fireEvent.input(box, { target: { value: ' Code.EXE \n\n  Slack  ' } });
    await fireEvent.click(screen.getByText('保存设置'));
    await waitFor(() => expect(m.saveSettings).toHaveBeenCalledTimes(1));
    const sent = m.saveSettings.mock.calls[0][0] as TrackerSettings;
    expect(sent.excluded_app_keys).toEqual(['code.exe', 'slack']);
  });
});
