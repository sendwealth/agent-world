import { describe, it, expect, vi, beforeEach } from "vitest";

const mockFetch = vi.fn();
global.fetch = mockFetch;

function mockFetchResponse(data: unknown, ok = true, status = 200) {
  return { ok, status, statusText: ok ? "OK" : "Error", json: () => Promise.resolve(data) };
}

describe("API Client", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockFetch.mockReset();
  });

  it("fetchJSON makes GET request and returns parsed JSON", async () => {
    const testData = [{ id: "1", name: "Test" }];
    mockFetch.mockResolvedValueOnce(mockFetchResponse(testData));
    const { fetchJSON } = await import("@/lib/api");
    const result = await fetchJSON("/api/v1/agents");
    expect(result).toEqual(testData);
  });

  it("fetchJSON throws on non-ok response", async () => {
    mockFetch.mockResolvedValueOnce({ ok: false, status: 404, statusText: "Not Found", json: () => Promise.resolve({}) });
    const { fetchJSON } = await import("@/lib/api");
    await expect(fetchJSON("/api/v1/nonexistent")).rejects.toThrow("API error: 404");
  });

  it("fetchJSON retries on 5xx errors", async () => {
    mockFetch
      .mockResolvedValueOnce({ ok: false, status: 500, statusText: "Internal Server Error", json: () => Promise.resolve({}) })
      .mockResolvedValueOnce({ ok: false, status: 500, statusText: "Internal Server Error", json: () => Promise.resolve({}) })
      .mockResolvedValueOnce(mockFetchResponse([{ id: "1" }]));
    const { fetchJSON } = await import("@/lib/api");
    const result = await fetchJSON("/api/v1/agents");
    expect(result).toEqual([{ id: "1" }]);
    expect(mockFetch).toHaveBeenCalledTimes(3);
  });

  it("postJSON makes POST request with correct body", async () => {
    const responseData = { id: "1", status: "created" };
    mockFetch.mockResolvedValueOnce(mockFetchResponse(responseData, true, 201));
    const { postJSON } = await import("@/lib/api");
    const result = await postJSON("/api/v1/tasks", { title: "Test" });
    expect(result).toEqual(responseData);
    expect(mockFetch).toHaveBeenCalledWith("/api/v1/tasks", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ title: "Test" }),
    });
  });

  it("postJSON throws on non-ok response", async () => {
    mockFetch.mockResolvedValueOnce({ ok: false, status: 400, statusText: "Bad Request", json: () => Promise.resolve({ error: "Invalid input" }) });
    const { postJSON } = await import("@/lib/api");
    await expect(postJSON("/api/v1/tasks", {})).rejects.toThrow("Invalid input");
  });

  it("putJSON makes PUT request with correct body", async () => {
    const responseData = { id: "1", updated: true };
    mockFetch.mockResolvedValueOnce(mockFetchResponse({ data: responseData, error: null }));
    const { putJSON } = await import("@/lib/api");
    const result = await putJSON("/api/v1/tasks/1", { status: "done" });
    expect(result).toEqual(responseData);
    expect(mockFetch).toHaveBeenCalledWith("/api/v1/tasks/1", {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ status: "done" }),
    });
  });

  it("deleteJSON makes DELETE request", async () => {
    const responseData = { deleted: true };
    mockFetch.mockResolvedValueOnce(mockFetchResponse({ data: responseData, error: null }));
    const { deleteJSON } = await import("@/lib/api");
    const result = await deleteJSON("/api/v1/tasks/1");
    expect(result).toEqual(responseData);
    expect(mockFetch).toHaveBeenCalledWith("/api/v1/tasks/1", { method: "DELETE" });
  });

  it("sseEndpoint returns correct URL", async () => {
    const { sseEndpoint } = await import("@/lib/api");
    expect(sseEndpoint("/api/v1/world/events")).toBe("/api/v1/world/events");
  });
});
