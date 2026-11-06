// AppUsageTracker Web Watcher —— 每 60s 上报当前活动标签页（URL + 标题）
const API = 'http://127.0.0.1:59598/api/web-heartbeat';

chrome.runtime.onInstalled.addListener(() => {
  chrome.alarms.create('hb', { periodInMinutes: 1 });
});

chrome.alarms.onAlarm.addListener(async (alarm) => {
  if (alarm.name !== 'hb') return;
  try {
    const [tab] = await chrome.tabs.query({ active: true, lastFocusedWindow: true });
    if (!tab || !tab.url || !/^https?:/i.test(tab.url)) return;
    await fetch(API, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ url: tab.url, title: tab.title || '' }),
    });
  } catch (e) {
    // AppUsageTracker 未运行或未开启网页统计：静默忽略
  }
});
