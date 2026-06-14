import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor, cleanup } from "@testing-library/react";
import type { Agent } from "@/types/world";

vi.mock("next/navigation", () => ({
  usePathname: () => "/evolution",
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
  { id: "agent-1", name: "Alpha", phase: "adult", money: 1000, tokens: 200, reputation: 0.8, skills: { mining: 5 }, alive: true, age: 100, createdAt: "2024-01-01" },
  { id: "agent-2", name: "Beta", phase: "childhood", money: 50, tokens: 10, reputation: 0.2, skills: {}, alive: true, age: 5, createdAt: "2024-01-02" },
];

afterEach(() => { cleanup(); });

describe("EvolutionPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockFetch.mockReset();
  });

  it("renders loading state initially", async () => {
    mockFetch.mockImplementation(() => new Promise(() => {}));
    const { default: EvolutionPage } = await import("@/app/evolution/page");
    render(<EvolutionPage />);
    expect(screen.getByText(/加载|Loading/i)).toBeTruthy();
  });

  it("renders evolution page header", async () => {
    mockFetch.mockImplementation((url: string) => {
      if (url.includes("/agents")) return Promise.resolve(mockFetchResponse(mockAgents));
      return Promise.resolve(mockFetchResponse([]));
    });
    const { default: EvolutionPage } = await import("@/app/evolution/page");
    render(<EvolutionPage />);
    await waitFor(() => { expect(screen.getByText("进化树")).toBeInTheDocument(); });
  });

  it("renders agent summary text", async () => {
    mockFetch.mockImplementation((url: string) => {
      if (url.includes("/agents")) return Promise.resolve(mockFetchResponse(mockAgents));
      return Promise.resolve(mockFetchResponse([]));
    });
    const { default: EvolutionPage } = await import("@/app/evolution/page");
    render(<EvolutionPage />);
    await waitFor(() => { expect(screen.getByText("进化树")).toBeInTheDocument(); });
    // Should display some stats text about the agents
    expect(screen.getAllByText(/存活 Agent/).length).toBeGreaterThanOrEqual(1);
  });
});
