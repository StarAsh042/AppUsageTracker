// 词条只有一份：src-tauri/locales/en-US.json，后端托盘菜单和前端界面读同一个文件。
// 中文即词条本身，zh 直接返回 key，所以 zh-CN.json 里不放对照表。
import { writable } from 'svelte/store';
import en from '../../src-tauri/locales/en-US.json';

export type Lang = 'zh' | 'en';

export const lang = writable<Lang>('zh');

/** 取词条。params 替换文案里的 {name} 这类占位符，句子结构由译文自己决定。 */
export function translate(l: Lang, key: string, params?: Record<string, string | number>): string {
  const text = l === 'en' ? ((en as Record<string, string>)[key] ?? key) : key;
  if (!params) return text;
  return text.replace(/\{(\w+)\}/g, (_m, k: string) => String(params[k] ?? ''));
}
