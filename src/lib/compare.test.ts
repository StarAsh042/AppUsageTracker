import { describe, expect, it } from 'vitest';
import {
  dateKey,
  deltaClass,
  formatDelta,
  localDateKey,
  orderedDelta,
  presetRange,
} from './compare';

// 用本地时间构造，避免时区把日期推前一天
const at = (y: number, m: number, d: number) => new Date(y, m - 1, d);

describe('localDateKey', () => {
  it('格式化为本地时区的 YYYY-MM-DD', () => {
    expect(localDateKey(0, at(2026, 11, 16))).toBe('2026-11-16');
    expect(localDateKey(0, at(2026, 1, 5))).toBe('2026-01-05');
  });

  it('支持负偏移（向前取日期）', () => {
    expect(localDateKey(-1, at(2026, 11, 16))).toBe('2026-11-15');
    expect(localDateKey(-7, at(2026, 11, 16))).toBe('2026-11-09');
  });

  it('跨月与跨年都正确', () => {
    expect(localDateKey(-1, at(2026, 11, 1))).toBe('2026-10-31');
    expect(localDateKey(-1, at(2026, 1, 1))).toBe('2025-12-31');
    expect(localDateKey(1, at(2026, 12, 31))).toBe('2027-01-01');
  });

  it('不修改传入的 Date', () => {
    const now = at(2026, 11, 16);
    localDateKey(-3, now);
    expect(now.getDate()).toBe(16);
  });
});

describe('dateKey', () => {
  it('与 localDateKey(0) 一致', () => {
    const now = at(2026, 11, 16);
    expect(dateKey(now)).toBe(localDateKey(0, now));
  });
});

describe('orderedDelta', () => {
  it('较旧为左侧时，正增长为正数', () => {
    expect(orderedDelta(100, 150, true)).toBeCloseTo(50, 10);
  });

  it('较旧为左侧时，下降为负负数', () => {
    expect(orderedDelta(200, 100, true)).toBeCloseTo(-50, 10);
  });

  it('较旧为右侧时，方向依然正确', () => {
    // a 是较新的一侧：100（新） vs 150（旧）→ -33.33…%
    expect(orderedDelta(100, 150, false)).toBeCloseTo(-100 / 3, 10);
    expect(orderedDelta(300, 200, false)).toBeCloseTo(50, 10);
  });

  it('较旧一侧为 0 时返回 null（无法计算变化率）', () => {
    expect(orderedDelta(0, 120, true)).toBeNull();
    expect(orderedDelta(120, 0, false)).toBeNull();
  });

  it('两边相等时为 0', () => {
    expect(orderedDelta(600, 600, true)).toBe(0);
  });
});

describe('formatDelta / deltaClass', () => {
  it('null 显示新增且无样式类', () => {
    expect(formatDelta(null)).toBe('新增');
    expect(deltaClass(null)).toBe('');
  });

  it('正负号与取整', () => {
    expect(formatDelta(12.3)).toBe('+12%');
    expect(formatDelta(-8.7)).toBe('-9%');
    expect(formatDelta(0)).toBe('+0%');
  });

  it('上升为 up，下降为 down', () => {
    expect(deltaClass(0)).toBe('up');
    expect(deltaClass(5)).toBe('up');
    expect(deltaClass(-5)).toBe('down');
  });
});

describe('presetRange', () => {
  it('today：A = 昨天，B = 今天', () => {
    expect(presetRange('today', at(2026, 11, 16))).toEqual({
      a: { from: '2026-11-15', to: '2026-11-15' },
      b: { from: '2026-11-16', to: '2026-11-16' },
    });
  });

  it('yesterday：A = 前天，B = 昨天', () => {
    expect(presetRange('yesterday', at(2026, 11, 16))).toEqual({
      a: { from: '2026-11-14', to: '2026-11-14' },
      b: { from: '2026-11-15', to: '2026-11-15' },
    });
  });

  it('today 在月初跨月也正确', () => {
    expect(presetRange('today', at(2026, 11, 1))).toEqual({
      a: { from: '2026-10-31', to: '2026-10-31' },
      b: { from: '2026-11-01', to: '2026-11-01' },
    });
  });

  it('week：A = 上周一~上周日，B = 本周一~今天', () => {
    // 2026-11-16 是周一
    expect(presetRange('week', at(2026, 11, 16))).toEqual({
      a: { from: '2026-11-09', to: '2026-11-15' },
      b: { from: '2026-11-16', to: '2026-11-16' },
    });
    // 2026-11-18 是周三，本周仍从周一算起
    expect(presetRange('week', at(2026, 11, 18))).toEqual({
      a: { from: '2026-11-09', to: '2026-11-15' },
      b: { from: '2026-11-16', to: '2026-11-18' },
    });
    // 周日属于本周末尾（dow = 6）
    expect(presetRange('week', at(2026, 11, 22))).toEqual({
      a: { from: '2026-11-09', to: '2026-11-15' },
      b: { from: '2026-11-16', to: '2026-11-22' },
    });
  });

  it('week 跨年时仍能算出上一周', () => {
    // 2027-01-01 是周五
    expect(presetRange('week', at(2027, 1, 1))).toEqual({
      a: { from: '2026-12-21', to: '2026-12-27' },
      b: { from: '2026-12-28', to: '2027-01-01' },
    });
  });

  it('month：A = 上月整月，B = 本月 1 日~今天', () => {
    expect(presetRange('month', at(2026, 11, 16))).toEqual({
      a: { from: '2026-10-01', to: '2026-10-31' },
      b: { from: '2026-11-01', to: '2026-11-16' },
    });
  });

  it('month 在一月时上月为去年 12 月', () => {
    expect(presetRange('month', at(2026, 1, 31))).toEqual({
      a: { from: '2025-12-01', to: '2025-12-31' },
      b: { from: '2026-01-01', to: '2026-01-31' },
    });
  });

  it('month 在闰年二月能取到 29 日', () => {
    expect(presetRange('month', at(2028, 3, 5))).toEqual({
      a: { from: '2028-02-01', to: '2028-02-29' },
      b: { from: '2028-03-01', to: '2028-03-05' },
    });
  });

  it('所有预设都满足 A 不晚于 B', () => {
    const now = at(2026, 11, 16);
    for (const kind of ['today', 'yesterday', 'week', 'month'] as const) {
      const r = presetRange(kind, now);
      expect(r.a.from <= r.b.from).toBe(true);
    }
  });
});
