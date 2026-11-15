import { describe, expect, it } from 'vitest';
import { limitOf, setDailyLimit } from './limits';

describe('limitOf', () => {
  it('未设置的分类返回 0（不限）', () => {
    expect(limitOf({}, '游戏')).toBe(0);
    expect(limitOf({ 游戏: 30 }, '社交')).toBe(0);
  });

  it('读取已设置的限额', () => {
    expect(limitOf({ 游戏: 30, 视频: 90 }, '视频')).toBe(90);
  });

  it('设置对象为空时也能安全读取', () => {
    expect(limitOf(null, '游戏')).toBe(0);
    expect(limitOf(undefined, '游戏')).toBe(0);
  });
});

describe('setDailyLimit', () => {
  it('大于 0 时写入', () => {
    const limits: Record<string, number> = {};
    setDailyLimit(limits, '游戏', 45);
    expect(limits).toEqual({ 游戏: 45 });
    expect(limitOf(limits, '游戏')).toBe(45);
  });

  it('传入 0 时删除键，避免 settings.json 堆积 0 值', () => {
    const limits: Record<string, number> = { 游戏: 45 };
    setDailyLimit(limits, '游戏', 0);
    expect('游戏' in limits).toBe(false);
    expect(limitOf(limits, '游戏')).toBe(0);
  });

  it('负数同样按「不限」处理', () => {
    const limits: Record<string, number> = { 游戏: 45 };
    setDailyLimit(limits, '游戏', -5);
    expect('游戏' in limits).toBe(false);
  });

  it('修改一个分类不影响其他分类', () => {
    const limits: Record<string, number> = { 游戏: 45, 视频: 90 };
    setDailyLimit(limits, '游戏', 0);
    expect(limits).toEqual({ 视频: 90 });
  });
});
