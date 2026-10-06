const req = async (method, url, body) => {
  const res = await fetch(url, {
    method,
    headers: body ? { 'content-type': 'application/json' } : undefined,
    body: body ? JSON.stringify(body) : undefined
  })
  const data = await res.json().catch(() => ({}))
  if (!res.ok) throw new Error(data.error || `请求失败 (${res.status})`)
  return data
}
export const api = {
  get: (url) => req('GET', url),
  post: (url, body) => req('POST', url, body)
}
