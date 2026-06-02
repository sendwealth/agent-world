import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor, cleanup } from "@testing-library/react";
import type { Agent } from "@/types/world";

vi.mock("next/navigation", () => ({
  usePathname: () => "/feed",
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

afterEach(() => { cleanup(); });

describe("FeedPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockFetch.mockReset();
  });

  it("renders page header", async () => {
    mockFetch.mockImplementation((url: string) => {
      if (url.includes("/agents")) return Promise.resolve(mockFetchResponse(mockAgents));
      return Promise.resolve(mockFetchResponse([]));
    });
    const { default: FeedPage } = await import("@/app/feed/page");
    render(<FeedPage />);
    await waitFor(() => { expect(screen.getByText("Agent 动态")).toBeInTheDocument(); });
  });

  it("renders feed filter buttons", async () => {
    mockFetch.mockImplementation((url: string) => {
      if (url.includes("/agents")) return Promise.resolve(mockFetchResponse(mockAgents));
      return Promise.resolve(mockFetchResponse([]));
    });
    const { default: FeedPage } = await import("@/app/feed/page");
    render(<FeedPage />);
    await waitFor(() => { expect(screen.getByText("Agent 动态")).toBeInTheDocument(); });
    expect(screen.getByText("最新")).toBeInTheDocument();
    expect(screen.getByText("热门")).toBeInTheDocument();
  });

  it("renders empty state when no posts", async () => {
    mockFetch.mockImplementation((url: string) => {
      if (url.includes("/agents")) return Promise.resolve(mockFetchResponse(mockAgents));
      return Promise.resolve(mockFetchResponse([]));
    });
    const { default: FeedPage } = await import("@/app/feed/page");
    render(<FeedPage />);
    await waitFor(() => { expect(screen.getByText("还没有动态")).toBeInTheDocument(); });
  });
});
