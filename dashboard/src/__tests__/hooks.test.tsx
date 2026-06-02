import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";
import type { Agent, Task, CoordinationTask } from "@/types/world";

vi.mock("next/navigation", () => ({
  usePathname: () => "/",
  useRouter: () => ({ push: vi.fn(), replace: vi.fn(), prefetch: vi.fn() }),
  useSearchParams: () => new URLSearchParams(),
}));

const mockSubscribe = vi.fn(() => () => {});
vi.mock("@/components/SSEProvider", () => ({
  SSEProvider: ({ children }: { children: React.ReactNode }) => <>{children}</>,
  useSSEContext: () => ({ events: [], connected: true, error: null, subscribe: mockSubscribe }),
}));

const mockFetch = vi.fn();
global.fetch = mockFetch;

function mockFetchResponse(data: unknown) {
  return { ok: true, status: 200, json: () => Promise.resolve(data) };
}

const mockAgents: Agent[] = [
  { id: "a1", name: "Alpha", phase: "adult", money: 100, tokens: 50, reputation: 0.8, skills: {}, alive: true, age: 10, createdAt: "2024-01-01" },
];

const mockTasks: Task[] = [
  { id: "t1", title: "Test Task", description: "desc", status: "published", reward: 50, escrow_held: false, publisher_id: "a1", assignee_id: null, result: null, expires_at: null, created_tick: 1 },
];

const mockCoordTasks: CoordinationTask[] = [
  { id: "ct1", title: "Team Task", description: "desc", status: "open", reward_pool: 100, currency: "tokens", escrow_held: false, coordinator_id: "a1", max_agents: 3, participants: [], contributions: {}, reward_overrides: {}, org_id: null, expires_at: null, created_tick: 1 } as CoordinationTask,
];

describe("useAgentStream hook", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockFetch.mockReset();
  });

  it("fetches and returns agents", async () => {
    mockFetch.mockImplementation((url: string) => {
      if (url.includes("/api/v1/agents")) return Promise.resolve(mockFetchResponse(mockAgents));
      return Promise.resolve(mockFetchResponse([]));
    });
    const { useAgentStream } = await import("@/hooks/useAgentStream");
    const { result } = renderHook(() => useAgentStream());
    await waitFor(() => { expect(result.current.loading).toBe(false); });
    expect(result.current.agents).toEqual(mockAgents);
    expect(result.current.error).toBeNull();
  });
});

describe("useTaskStream hook", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockFetch.mockReset();
  });

  it("fetches and returns tasks", async () => {
    mockFetch.mockImplementation((url: string) => {
      if (url.includes("/api/v1/tasks")) return Promise.resolve(mockFetchResponse(mockTasks));
      return Promise.resolve(mockFetchResponse([]));
    });
    const { useTaskStream } = await import("@/hooks/useTaskStream");
    const { result } = renderHook(() => useTaskStream());
    await waitFor(() => { expect(result.current.loading).toBe(false); });
    expect(result.current.tasks).toEqual(mockTasks);
  });
});

describe("useCoordinationTaskStream hook", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockFetch.mockReset();
  });

  it("fetches and returns coordination tasks", async () => {
    mockFetch.mockImplementation((url: string) => {
      if (url.includes("/coordination-tasks")) return Promise.resolve(mockFetchResponse(mockCoordTasks));
      return Promise.resolve(mockFetchResponse([]));
    });
    const { useCoordinationTaskStream } = await import("@/hooks/useCoordinationTaskStream");
    const { result } = renderHook(() => useCoordinationTaskStream());
    await waitFor(() => { expect(result.current.loading).toBe(false); });
    expect(result.current.tasks).toEqual(mockCoordTasks);
  });
});
