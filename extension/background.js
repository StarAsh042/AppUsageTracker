// AppUsageTracker Web Watcher —— 心跳上报当前标签页（URL + 标题）
// 频率自适应：后端响应携带 interval（秒），扩展按其调整 alarm 周期（MV3 下限 30s）
const API = 'http://127.0.0.1:59598/api/web-heartbeat';
let currentIntervalSec = 30;

chrome.runtime.onInstalled.addListener(() => {
  chrome.alarms.create('hb', { periodInMinutes: 0.5 });
});

chrome.alarms.onAlarm.addListener(async (alarm) => {
  if (alarm.name !== 'hb') return;
  await sendHeartbeat();
});

// 标签页切换 / 加载完成 / 窗口聚焦变化 → 立即心跳（事件驱动，更精准）
chrome.tabs.onActivated.addListener(() => sendHeartbeat());
chrome.tabs.onUpdated.addListener((_id, info) => {
  if (info.url || info.title) sendHeartbeat();
});
chrome.windows.onFocusChanged.addListener(() => sendHeartbeat());

async function sendHeartbeat() {
  try {
    const [tab] = await chrome.tabs.query({ active: true, lastFocusedWindow: true });
    if (!tab || !tab.url || !/^https?:/i.test(tab.url)) return;
    const resp = await fetch(API, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ url: tab.url, title: tab.title || '' }),
    });
    if (!resp.ok) return;
    const data = await resp.json();
    if (typeof data.interval === 'number' && data.interval !== currentIntervalSec) {
      currentIntervalSec = Math.max(30, data.interval); // MV3 alarms 下限 30s
      chrome.alarms.clear('hb');
      chrome.alarms.create('hb', { periodInMinutes: currentIntervalSec / 60 });
    }
  } catch (e) {
    // AppUsageTracker 未运行或未开启网页统计：静默忽略
  }
}
