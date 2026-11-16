// 对比视图的纯逻辑：日期区间、变化率。
// 抽出来是为了能在 vitest 里直接测，不依赖 Svelte 组件与真实系统时钟。

export type ComparePreset = 'today' | 'yesterday' | 'week' | 'month';

export interface DateRange {
  from: string;
  to: string;
}

export interface CompareRanges {
  /** 较早的一侧 */
  a: DateRange;
  /** 较新的一侧 */
  b: DateRange;
}

const pad = (n: number) => String(n).padStart(2, '0');

/** 本地时区的 YYYY-MM-DD */
export function dateKey(d: Date): string {
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

/** 相对今天偏移 offset 天的本地日期键；now 可注入以便测试 */
export function localDateKey(offset = 0, now: Date = new Date()): string {
  const d = new Date(now.getTime());
  d.setDate(d.getDate() + offset);
  return dateKey(d);
}

/** 各预设区间；A 恒为较早的一侧，B 恒为较新的一侧 */
export function presetRange(kind: ComparePreset, now: Date = new Date()): CompareRanges {
  const today = localDateKey(0, now);
  if (kind === 'today') {
    const y = localDateKey(-1, now);
    return { a: { from: y, to: y }, b: { from: today, to: today } };
  }
  if (kind === 'yesterday') {
    const d2 = localDateKey(-2, now);
    const y = localDateKey(-1, now);
    return { a: { from: d2, to: d2 }, b: { from: y, to: y } };
  }
  const y = now.getFullYear();
  const m = now.getMonth();
  if (kind === 'week') {
    const dow = (now.getDay() + 6) % 7; // 0 = 周一
    const mon = new Date(y, m, now.getDate() - dow);
    const lastMon = new Date(y, m, now.getDate() - dow - 7);
    const lastSun = new Date(y, m, now.getDate() - dow - 1);
    return { a: { from: dateKey(lastMon), to: dateKey(lastSun) }, b: { from: dateKey(mon), to: today } };
  }
  return {
    a: { from: dateKey(new Date(y, m - 1, 1)), to: dateKey(new Date(y, m, 0)) },
    b: { from: dateKey(new Date(y, m, 1)), to: today },
  };
}

/** 变化率：始终按「较新 vs 较旧」计算，A/B 左右顺序不影响符号；较旧为 0 时返回 null */
export function orderedDelta(a: number, b: number, aIsOlder: boolean): number | null {
  const older = aIsOlder ? a : b;
  const newer = aIsOlder ? b : a;
  if (older <= 0) return null;
  return ((newer - older) / older) * 100;
}

/** 变化率的展示文本；null（上一周期为 0）显示「新增」 */
export function formatDelta(d: number | null): string {
  return d === null ? '新增' : `${d >= 0 ? '+' : ''}${d.toFixed(0)}%`;
}

/** 变化率对应的样式类 */
export function deltaClass(d: number | null): string {
  return d === null ? '' : d >= 0 ? 'up' : 'down';
}
