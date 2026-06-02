import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor, cleanup } from "@testing-library/react";
import type { Agent } from "@/types/world";

vi.mock("next/navigation", () => ({
  usePathname: () => "/traces",
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
  { id: "agent-1", name: "Alpha", phase: "adult", money: 1000, tokens: 200, reputation: 0.8, skills: {}, alive: true, age: 100, createdAt: "2024-01-01" },
  { id: "agent-2", name: "Beta", phase: "elder", money: 500, tokens: 50, reputation: 0.5, skills: {}, alive: false, age: 300, createdAt: "2024-01-02" },
];

afterEach(() => { cleanup(); });

describe("TracesPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockFetch.mockReset();
  });

  it("renders loading state initially", async () => {
    mockFetch.mockImplementation(() => new Promise(() => {}));
    const { default: TracesPage } = await import("@/app/traces/page");
    render(<TracesPage />);
    expect(screen.getByText("正在加载 Agent 数据...")).toBeInTheDocument();
  });

  it("renders page header after loading", async () => {
    mockFetch.mockImplementation((url: string) => {
      if (url.includes("/agents") && !url.includes("/traces")) return Promise.resolve(mockFetchResponse(mockAgents));
      return Promise.resolve(mockFetchResponse([]));
    });
    const { default: TracesPage } = await import("@/app/traces/page");
    render(<TracesPage />);
    await waitFor(() => { expect(screen.getByText("决策轨迹")).toBeInTheDocument(); });
    expect(screen.getByText(/查看 Agent 每个 Tick 的决策过程/)).toBeInTheDocument();
  });

  it("renders agent selector dropdown", async () => {
    mockFetch.mockImplementation((url: string) => {
      if (url.includes("/agents") && !url.includes("/traces")) return Promise.resolve(mockFetchResponse(mockAgents));
      return Promise.resolve(mockFetchResponse([]));
    });
    const { default: TracesPage } = await import("@/app/traces/page");
    render(<TracesPage />);
    await waitFor(() => {
      const selectEl = screen.getByText("-- 选择一个 Agent --");
      expect(selectEl).toBeInTheDocument();
    });
  });

  it("shows empty state when no agent selected", async () => {
    mockFetch.mockImplementation((url: string) => {
      if (url.includes("/agents") && !url.includes("/traces")) return Promise.resolve(mockFetchResponse(mockAgents));
      return Promise.resolve(mockFetchResponse([]));
    });
    const { default: TracesPage } = await import("@/app/traces/page");
    render(<TracesPage />);
    await waitFor(() => {
      expect(screen.getByText("选择一个 Agent 以查看决策轨迹")).toBeInTheDocument();
    });
  });
});
