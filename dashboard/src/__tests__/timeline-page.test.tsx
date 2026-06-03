import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor, cleanup } from "@testing-library/react";
import type { WorldEvent } from "@/types/world";

vi.mock("next/navigation", () => ({
  usePathname: () => "/timeline",
  useRouter: () => ({ push: vi.fn(), replace: vi.fn(), prefetch: vi.fn() }),
  useSearchParams: () => new URLSearchParams(),
}));

const mockEvents: WorldEvent[] = [
  { id: "e1", type: "agent_spawned", agentId: "a1", agentName: "Alpha", description: "Alpha spawned", timestamp: "2024-01-01T10:00:00Z", tick: 1 },
  { id: "e2", type: "transaction_completed", agentId: "a1", agentName: "Alpha", targetId: "a2", targetName: "Beta", description: "Trade occurred", amount: 100, timestamp: "2024-01-01T10:01:00Z", tick: 2 },
];

const mockSubscribe = vi.fn(() => () => {});
vi.mock("@/components/SSEProvider", () => ({
  SSEProvider: ({ children }: { children: React.ReactNode }) => <>{children}</>,
  useSSEContext: () => ({ events: mockEvents, connected: true, error: null, subscribe: mockSubscribe }),
}));

const mockFetch = vi.fn();
global.fetch = mockFetch;

function mockFetchResponse(data: unknown) {
  return { ok: true, status: 200, json: () => Promise.resolve(data) };
}

afterEach(() => { cleanup(); });

describe("TimelinePage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockFetch.mockReset();
  });

  it("renders timeline page header", async () => {
    mockFetch.mockImplementation((url: string) => {
      if (url.includes("/world/events")) return Promise.resolve(mockFetchResponse(mockEvents));
      return Promise.resolve(mockFetchResponse([]));
    });
    const { default: TimelinePage } = await import("@/app/timeline/page");
    render(<TimelinePage />);
    await waitFor(() => {
      expect(screen.getByText("事件时间线")).toBeInTheDocument();
    });
  });

  it("renders event descriptions", async () => {
    mockFetch.mockImplementation((url: string) => {
      if (url.includes("/world/events")) return Promise.resolve(mockFetchResponse(mockEvents));
      return Promise.resolve(mockFetchResponse([]));
    });
    const { default: TimelinePage } = await import("@/app/timeline/page");
    render(<TimelinePage />);
    await waitFor(() => {
      expect(screen.getByText("Alpha spawned")).toBeInTheDocument();
    });
    expect(screen.getByText("Trade occurred")).toBeInTheDocument();
  });

  it("renders tick numbers for events", async () => {
    mockFetch.mockImplementation((url: string) => {
      if (url.includes("/world/events")) return Promise.resolve(mockFetchResponse(mockEvents));
      return Promise.resolve(mockFetchResponse([]));
    });
    const { default: TimelinePage } = await import("@/app/timeline/page");
    render(<TimelinePage />);
    await waitFor(() => {
      expect(screen.getByText(/Tick.*1|^#1/i)).toBeTruthy();
    });
  });
});
