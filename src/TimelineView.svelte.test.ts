import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { TimelineData } from './lib/api';
import TimelineView from './TimelineView.svelte';

// 锁住三件事：载入后画出应用条、旧响应不能覆盖新日期、失败要能看到错误与重试。
const m = vi.hoisted(() => ({
  getTimeline: vi.fn(),
  getTopTitles: vi.fn(),
  getTopWeb: vi.fn(),
}));

vi.mock('./lib/api', async (importOriginal) => {
  const actual = await importOriginal<typeof import('./lib/api')>();
  return {
    ...actual,
    api: { ...actual.api, getTimeline: m.getTimeline, getTopTitles: m.getTopTitles, getTopWeb: m.getTopWeb },
  };
});

function day(date: string, appName: string): TimelineData {
  return {
    date,
    segments: [
      {
        app_key: appName.toLowerCase(),
        display_name: appName,
        category: '工作',
        start_time: `${date} 09:00:00`,
        end_time: `${date} 10:00:00`,
        duration_seconds: 3600,
      },
    ],
    states: [{ state: 'Active', start_time: `${date} 09:00:00`, end_time: `${date} 10:00:00`, duration_seconds: 3600 }],
  };
}

// 没开 vitest globals，@testing-library 的自动清理不会注册，得自己卸掉上一个用例的 DOM。
afterEach(() => cleanup());

beforeEach(() => {
  vi.clearAllMocks();
  m.getTopTitles.mockResolvedValue([]);
  m.getTopWeb.mockResolvedValue([]);
});

describe('TimelineView', () => {
  it('取到数据后在图例里列出应用', async () => {
    m.getTimeline.mockResolvedValue(day('2025-03-03', 'Code'));
    render(TimelineView, { initialDate: '2025-03-03' });
    expect(await screen.findByText('Code')).toBeTruthy();
    expect(screen.getByText('图例（按时长）')).toBeTruthy();
  });

  it('空的一天把状态区标成暂无记录', async () => {
    m.getTimeline.mockResolvedValue({ date: '2025-03-03', segments: [], states: [] });
    render(TimelineView, { initialDate: '2025-03-03' });
    // 空的一天仍然有时间带骨架，缺的只是段；状态汇总区会显式说明没有记录。
    expect(await screen.findByText('该日暂无状态记录')).toBeTruthy();
  });

  it('加载失败时显示错误原文与重试按钮', async () => {
    m.getTimeline.mockRejectedValueOnce({ code: 'db', message: '查询超时' });
    render(TimelineView, { initialDate: '2025-03-03' });
    expect(await screen.findByText('加载失败')).toBeTruthy();
    expect(screen.getByText('查询超时')).toBeTruthy();
    expect(screen.getByText('重试')).toBeTruthy();
  });

  it('快速翻日期时旧响应不覆盖新日期', async () => {
    const pending: { resolve: (d: TimelineData) => void }[] = [];
    m.getTimeline.mockImplementation(() => new Promise<TimelineData>((resolve) => pending.push({ resolve })));

    const { container } = render(TimelineView, { initialDate: '2025-03-03' });
    await waitFor(() => expect(pending.length).toBe(1));

    const dateInput = container.querySelector<HTMLInputElement>('input[type="date"]');
    if (!dateInput) throw new Error('缺少日期输入框');
    await fireEvent.input(dateInput, { target: { value: '2025-03-07' } });
    await waitFor(() => expect(pending.length).toBe(2));

    pending[1].resolve(day('2025-03-07', 'Newer'));
    await screen.findByText('Newer');

    pending[0].resolve(day('2025-03-03', 'Older'));
    await new Promise((r) => setTimeout(r, 20));
    expect(screen.getByText('Newer')).toBeTruthy();
    expect(screen.queryByText('Older')).toBeNull();
  });
});
