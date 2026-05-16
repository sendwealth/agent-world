// API client for the world-engine backend
// Next.js rewrites proxy /api/v1/* to the world-engine on port 3000.
// Use NEXT_PUBLIC_API_URL only when bypassing the proxy (e.g. direct SSE).

const API_BASE = process.env.NEXT_PUBLIC_API_URL ?? "";

export async function fetchJSON<T>(path: string): Promise<T> {
  const res = await fetch(`${API_BASE}${path}`);
  if (!res.ok) {
    throw new Error(`API error: ${res.status} ${res.statusText}`);
  }
  return res.json() as Promise<T>;
}

export async function postJSON<T>(path: string, body: unknown): Promise<T> {
  const res = await fetch(`${API_BASE}${path}`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
  if (!res.ok) {
    const err = await res.json().catch(() => ({ error: res.statusText }));
    throw new Error(err.error ?? `API error: ${res.status}`);
  }
  return res.json() as Promise<T>;
}

export function sseEndpoint(path: string): string {
  return `${API_BASE}${path}`;
}
