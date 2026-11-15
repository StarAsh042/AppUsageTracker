// 分类每日限额的纯逻辑：读取与写入 settings.category_daily_limit_minutes。

export type DailyLimits = Record<string, number>;

/** 未设置即视为 0（不限） */
export function limitOf(limits: DailyLimits | null | undefined, name: string): number {
  return limits?.[name] ?? 0;
}

/** 大于 0 才写入，否则删键（保持 settings.json 不堆积 0 值） */
export function setDailyLimit(limits: DailyLimits, name: string, minutes: number): void {
  if (minutes > 0) {
    limits[name] = minutes;
  } else {
    delete limits[name];
  }
}
