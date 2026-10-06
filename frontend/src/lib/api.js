// 简单的 API 封装: 自动带上当前门店头, 统一错误抛出
let currentStoreId = 1;

export function setStoreId(id) {
  currentStoreId = id;
  localStorage.setItem('store_id', String(id));
}

export function getStoreId() {
  const saved = Number(localStorage.getItem('store_id'));
  return saved > 0 ? saved : currentStoreId;
}

export async function api(path, options = {}) {
  const headers = { 'Content-Type': 'application/json', ...(options.headers || {}) };
  headers['x-store-id'] = String(getStoreId());
  const resp = await fetch(path, { ...options, headers });
  const text = await resp.text();
  const data = text ? JSON.parse(text) : null;
  if (!resp.ok) {
    const err = new Error(data?.error || `请求失败 (${resp.status})`);
    err.status = resp.status;
    err.code = data?.code || '';
    throw err;
  }
  return data;
}

export const get = (p) => api(p);
export const post = (p, body) => api(p, { method: 'POST', body: JSON.stringify(body) });
