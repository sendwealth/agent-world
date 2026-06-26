// API client for the world-engine backend
// Next.js rewrites proxy /api/v1/* to the world-engine on port 3000.
// Use NEXT_PUBLIC_API_URL only when bypassing the proxy (e.g. direct SSE).

const API_BASE = process.env.NEXT_PUBLIC_API_URL ?? "";

const MAX_RETRIES = 3;
const RETRY_DELAY_MS = 1000;

async function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

// Retry logic only for GET requests — POST/PUT/DELETE are not retried
// to avoid duplicate side effects.
async function fetchWithRetry(
  url: string,
  retries: number = MAX_RETRIES,
): Promise<Response> {
  let lastError: Error | null = null;
  for (let attempt = 0; attempt <= retries; attempt++) {
    try {
      const res = await fetch(url);
      if (res.status >= 500 && attempt < retries) {
        await delay(RETRY_DELAY_MS * (attempt + 1));
        continue;
      }
      return res;
    } catch (err) {
      lastError = err instanceof Error ? err : new Error(String(err));
      if (attempt < retries) {
        await delay(RETRY_DELAY_MS * (attempt + 1));
      }
    }
  }
  throw lastError ?? new Error("Request failed after retries");
}

export async function fetchJSON<T>(path: string): Promise<T> {
  const res = await fetchWithRetry(`${API_BASE}${path}`);
  if (!res.ok) {
    throw new Error(`API error: ${res.status} ${res.statusText}`);
  }
  const json = await res.json();
  // Auto-unwrap {data: ..., error: ..., request_id: ...} envelope from world-engine
  if (json && typeof json === "object" && !Array.isArray(json) && "data" in json && "error" in json) {
    if (json.error) {
      throw new Error(json.error);
    }
    return json.data as T;
  }
  return json as T;
}

export async function postJSON<T>(path: string, body: unknown): Promise<T> {
  // No retry for POST — avoids duplicate resource creation
  const res = await fetch(`${API_BASE}${path}`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
  if (!res.ok) {
    const err = await res.json().catch(() => ({ error: res.statusText }));
    throw new Error(err.error ?? `API error: ${res.status}`);
  }
  const json = await res.json();
  // Auto-unwrap envelope
  if (json && typeof json === "object" && !Array.isArray(json) && "data" in json && "error" in json) {
    if (json.error) throw new Error(json.error);
    return json.data as T;
  }
  return json as T;
}

export async function putJSON<T>(path: string, body: unknown): Promise<T> {
  const res = await fetch(`${API_BASE}${path}`, {
    method: "PUT",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
  if (!res.ok) {
    const err = await res.json().catch(() => ({ error: res.statusText }));
    throw new Error(err.error ?? `API error: ${res.status}`);
  }
  const json = await res.json();
  // Auto-unwrap {data: ..., error: ..., request_id: ...} envelope from world-engine
  if (json && typeof json === "object" && !Array.isArray(json) && "data" in json && "error" in json) {
    if (json.error) throw new Error(json.error);
    return json.data as T;
  }
  return json as T;
}

export async function deleteJSON<T>(path: string): Promise<T> {
  const res = await fetch(`${API_BASE}${path}`, {
    method: "DELETE",
  });
  if (!res.ok) {
    const err = await res.json().catch(() => ({ error: res.statusText }));
    throw new Error(err.error ?? `API error: ${res.status}`);
  }
  const json = await res.json();
  // Auto-unwrap {data: ..., error: ..., request_id: ...} envelope from world-engine
  if (json && typeof json === "object" && !Array.isArray(json) && "data" in json && "error" in json) {
    if (json.error) throw new Error(json.error);
    return json.data as T;
  }
  return json as T;
}

export function sseEndpoint(path: string): string {
  return `${API_BASE}${path}`;
}
