import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { DashboardData } from './lib/api';
import Dashboard from './Dashboard.svelte';

// 这里锁的是用户能看到的行为：载入后出数字、加载中不先闪「暂无数据」、
// 快速换日期时旧响应不许覆盖新结果、失败要显示错误和重试按钮。
const m = vi.hoisted(() => ({
  getDashboard: vi.fn(),
  listCategories: vi.fn(),
  getSettings: vi.fn(),
}));

vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }));

vi.mock('./lib/api', async (importOriginal) => {
  const actual = await importOriginal<typeof import('./lib/api')>();
  return {
    ...actual,
    api: {
      ...actual.api,
      getDashboard: m.getDashboard,
      listCategories: m.listCategories,
      getSettings: m.getSettings,
    },
  };
});

const settings = {
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

function day(total: number, appName: string): DashboardData {
  return {
    from: '2025-03-03',
    to: '2025-03-03',
    total_active_seconds: total,
    top_app: appName,
    apps: [
      {
        app_key: appName.toLowerCase(),
        display_name: appName,
        process_name: appName,
        exe_path: '',
        icon_data_url: null,
        zh_name: null,
        category: '工作',
        active_seconds: total,
        percent: 100,
      },
    ],
    categories: [{ name: '工作', seconds: total, percent: 100 }],
    trend: [],
  };
}

const emptyDay: DashboardData = {
  from: '2025-03-03',
  to: '2025-03-03',
  total_active_seconds: 0,
  top_app: null,
  apps: [],
  categories: [],
  trend: [],
};

// 没开 vitest globals，@testing-library 的自动清理不会注册，得自己卸掉上一个用例的 DOM。
afterEach(() => cleanup());

beforeEach(() => {
  vi.clearAllMocks();
  m.listCategories.mockResolvedValue(['工作']);
  m.getSettings.mockResolvedValue({ ...settings });
});

describe('Dashboard', () => {
  it('取到数据后显示总时长与排行', async () => {
    m.getDashboard.mockResolvedValue(day(3600, 'Code'));
    render(Dashboard);
    // 总时长、排行条、分类条用的是同一个格式化结果，卡片不是唯一出处。
    expect((await screen.findAllByText('1小时0分')).length).toBeGreaterThan(0);
    expect(screen.getAllByText('Code').length).toBeGreaterThan(0);
  });

  it('首帧显示加载中而不是暂无数据', async () => {
    let settle: (d: DashboardData) => void = () => {};
    m.getDashboard.mockReturnValue(new Promise<DashboardData>((r) => (settle = r)));
    render(Dashboard);
    expect(await screen.findByText('加载中…')).toBeTruthy();
    expect(screen.queryByText('暂无数据')).toBeNull();
    settle(emptyDay);
    await waitFor(() => expect(screen.queryByText('加载中…')).toBeNull());
  });

  it('空数据时显示暂无数据', async () => {
    m.getDashboard.mockResolvedValue(emptyDay);
    render(Dashboard);
    expect(await screen.findByText('该时间段暂无数据')).toBeTruthy();
  });

  it('加载失败时显示错误原文与重试按钮', async () => {
    m.getDashboard.mockRejectedValueOnce({ code: 'db', message: '数据库被占用' });
    render(Dashboard);
    expect(await screen.findByText('加载失败')).toBeTruthy();
    expect(screen.getByText('数据库被占用')).toBeTruthy();
    expect(screen.getByText('重试')).toBeTruthy();
  });

  it('快速换日期时旧响应不覆盖新结果', async () => {
    const pending: { resolve: (d: DashboardData) => void }[] = [];
    m.getDashboard.mockImplementation(() => new Promise<DashboardData>((resolve) => pending.push({ resolve })));

    const { container } = render(Dashboard);
    await waitFor(() => expect(pending.length).toBe(1));

    const fromInput = container.querySelector<HTMLInputElement>('input[type="date"]');
    if (!fromInput) throw new Error('缺少起始日期输入框');
    await fireEvent.input(fromInput, { target: { value: '2025-03-05' } });
    await waitFor(() => expect(pending.length).toBe(2));

    // 后发的先到，先发的后到：界面必须停在后发那次的结果上。
    pending[1].resolve(day(7200, 'Late'));
    await screen.findAllByText('2小时0分');

    pending[0].resolve(day(60, 'Stale'));
    await new Promise((r) => setTimeout(r, 20));
    expect(screen.getAllByText('2小时0分').length).toBeGreaterThan(0);
    expect(screen.queryByText('Stale')).toBeNull();
  });
});
