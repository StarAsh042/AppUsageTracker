import { invoke } from '@tauri-apps/api/core';

export interface AppIdentity {
  app_key: string;
  process_name: string;
  display_name: string;
  exe_path: string;
}

export interface CategoryRule {
  name: string;
  keywords: string[];
}

export interface WidgetPrefs {
  always_on_top: boolean;
  locked: boolean;
  edge_snap: boolean;
  compact: boolean;
  opacity: number;
  accent: string;
}

export interface AppDetail {
  app_key: string;
  display_name: string;
  exe_path: string;
  category: string;
  total_seconds: number;
  daily: { date: string; seconds: number }[];
}

export interface ScannedApp {
  app_key: string;
  display_name: string;
  category: string;
  source: string;
}

export type CategoryNature = 'productive' | 'neutral' | 'distracting';

export interface TrackerSettings {
  idle_threshold_seconds: number;
  foreground_poll_ms: number;
  start_with_windows: boolean;
  excluded_app_keys: string[];
  data_directory: string;
  theme: string;
  rest_reminder_minutes: number;
  retention_days: number;
  show_floating_widget: boolean;
  category_daily_limit_minutes: Record<string, number>;
  app_category_overrides: Record<string, string>;
  category_rules: CategoryRule[];
  daily_goal_minutes: number;
  hotkeys_enabled: boolean;
  category_nature: Record<string, string>;
  widget: WidgetPrefs;
}

export interface LiveTopApp {
  display_name: string;
  category: string;
  active_seconds: number;
}

export interface LiveStatus {
  state: string;
  current_app: AppIdentity | null;
  current_category: string | null;
  current_seconds: number;
  today_total_seconds: number;
  top_apps: LiveTopApp[];
}

export interface DashboardApp {
  app_key: string;
  display_name: string;
  process_name: string;
  exe_path: string;
  category: string;
  active_seconds: number;
  percent: number;
}

export interface DashboardCategory {
  name: string;
  seconds: number;
  percent: number;
}

export interface DashboardTrendPoint {
  date: string;
  seconds: number;
}

export interface DashboardData {
  from: string;
  to: string;
  total_active_seconds: number;
  top_app: string | null;
  apps: DashboardApp[];
  categories: DashboardCategory[];
  trend: DashboardTrendPoint[];
}

export interface TimelineSegment {
  app_key: string;
  display_name: string;
  category: string;
  start_time: string;
  end_time: string;
  duration_seconds: number;
}

export interface TimelineState {
  state: string;
  start_time: string;
  end_time: string;
  duration_seconds: number;
}

export interface TimelineData {
  date: string;
  segments: TimelineSegment[];
  states: TimelineState[];
}

export const api = {
  getLiveStatus: () => invoke<LiveStatus>('get_live_status'),
  getDashboard: (from: string, to: string) => invoke<DashboardData>('get_dashboard', { from, to }),
  getTimeline: (date: string) => invoke<TimelineData>('get_timeline', { date }),
  getAppDetail: (appKey: string, from: string, to: string) =>
    invoke<AppDetail>('get_app_detail', { appKey, from, to }),
  scanInstalledApps: () => invoke<ScannedApp[]>('scan_installed_apps'),
  getSettings: () => invoke<TrackerSettings>('get_settings'),
  saveSettings: (settings: TrackerSettings) => invoke<void>('save_settings', { settings }),
  setPaused: (paused: boolean) => invoke<void>('set_paused', { paused }),
  setAppCategory: (appKey: string, category: string) =>
    invoke<void>('set_app_category', { appKey, category }),
  saveCategoryRules: (rules: CategoryRule[]) => invoke<void>('save_category_rules', { rules }),
  exportCsv: (from: string, to: string) => invoke<string>('export_csv', { from, to }),
  clearAllData: () => invoke<void>('clear_all_data'),
  listCategories: () => invoke<string[]>('list_available_categories'),
  openPath: (path: string) => invoke<void>('open_path', { path }),
  getVersion: () => invoke<string>('get_app_version'),
  showWidgetMenu: () => invoke<void>('show_widget_menu'),
  setWidgetCollapsed: (collapsed: boolean) => invoke<void>('set_widget_collapsed', { collapsed }),
  toggleMainWindow: () => invoke<void>('toggle_main_window'),
  excludeApp: (appKey: string) => invoke<void>('exclude_app', { appKey }),
  updateWidgetSettings: (widget: WidgetPrefs) => invoke<void>('update_widget_settings', { widget }),
  exportBackupJson: () => invoke<string>('export_backup_json'),
  importBackupJson: () => invoke<string>('import_backup_json'),
  listBackups: () => invoke<string[]>('list_backups'),
};

export const CATEGORY_PALETTE = [
  '#4f8cff',
  '#3ecf8e',
  '#f5a623',
  '#a78bfa',
  '#22d3ee',
  '#fb923c',
  '#f472b6',
  '#94a3b8',
];

export const WIDGET_ACCENTS: Record<string, string> = {
  blue: '#4f8cff',
  cyan: '#22d3ee',
  violet: '#a78bfa',
  green: '#34d399',
  orange: '#fb923c',
  pink: '#f472b6',
};

export const UNCATEGORIZED = '未分类';

export function fmtDuration(sec: number): string {
  const t = Math.floor(sec);
  const h = Math.floor(t / 3600);
  const m = Math.floor((t % 3600) / 60);
  const s = t % 60;
  if (h > 0) return `${h}小时${m}分`;
  if (m > 0) return `${m}分${s}秒`;
  return `${s}秒`;
}

export function stateLabel(state: string): string {
  const map: Record<string, string> = {
    Active: '统计中',
    Idle: '空闲',
    Locked: '已锁定',
    Suspended: '系统挂起',
    Paused: '已暂停',
  };
  return map[state] ?? state;
}

export function localDateKey(offset = 0): string {
  const d = new Date();
  d.setDate(d.getDate() + offset);
  const p = (n: number) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
}
