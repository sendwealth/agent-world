import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor, cleanup } from "@testing-library/react";
import type { Agent } from "@/types/world";

vi.mock("next/navigation", () => ({
  usePathname: () => "/agents",
  useRouter: () => ({ push: vi.fn(), replace: vi.fn(), prefetch: vi.fn() }),
  useSearchParams: () => new URLSearchParams(),
}));

vi.mock("next/link", () => ({
  default: ({ children, href }: { children: React.ReactNode; href: string }) => (
    <a href={href} data-testid="link">{children}</a>
  ),
}));

const mockSubscribe = vi.fn(() => () => {});
vi.mock("@/components/SSEProvider", () => ({
  SSEProvider: ({ children }: { children: React.ReactNode }) => <>{children}</>,
  useSSEContext: () => ({ events: [], connected: true, error: null, subscribe: mockSubscribe }),
}));

const mockFetch = vi.fn();
global.fetch = mockFetch;

function mockFetchResponse(data: unknown) {
  return { ok: true, status: 200, json: () => Promise.resolve(data), body: null };
}

const mockAgents: Agent[] = [
  { id: "agent-1", name: "Alpha", phase: "adult", money: 1500, tokens: 200, reputation: 0.8, skills: { mining: 5, trading: 3 }, alive: true, age: 100, createdAt: "2024-01-01" },
  { id: "agent-2", name: "Beta", phase: "child", money: 50, tokens: 10, reputation: 0.2, skills: {}, alive: false, age: 5, createdAt: "2024-01-02" },
  { id: "agent-3", name: "Gamma", phase: "elder", money: 500000, tokens: 5000, reputation: 0.95, skills: { crafting: 10, leadership: 8, trading: 7 }, alive: true, age: 800, createdAt: "2024-01-03" },
];

afterEach(() => { cleanup(); });

describe("AgentsPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockFetch.mockReset();
    mockFetch.mockResolvedValue({ ok: true, status: 200, json: () => Promise.resolve([]) });
  });

  it("renders loading state initially", async () => {
    mockFetch.mockImplementation(() => new Promise(() => {}));
    const { default: AgentsPage } = await import("@/app/agents/page");
    render(<AgentsPage />);
    expect(screen.getByText("正在加载 Agent 数据...")).toBeInTheDocument();
  });

  it("renders agent names after loading", async () => {
    mockFetch.mockImplementation((url: string) => {
      if (url.includes("/api/v1/agents")) return Promise.resolve(mockFetchResponse(mockAgents));
      return Promise.resolve(mockFetchResponse([]));
    });
    const { default: AgentsPage } = await import("@/app/agents/page");
    render(<AgentsPage />);
    await waitFor(() => { expect(screen.getAllByText("Alpha").length).toBeGreaterThanOrEqual(1); });
    expect(screen.getAllByText("Beta").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("Gamma").length).toBeGreaterThanOrEqual(1);
  });

  it("displays agent count summary", async () => {
    mockFetch.mockImplementation((url: string) => {
      if (url.includes("/api/v1/agents")) return Promise.resolve(mockFetchResponse(mockAgents));
      return Promise.resolve(mockFetchResponse([]));
    });
    const { default: AgentsPage } = await import("@/app/agents/page");
    render(<AgentsPage />);
    await waitFor(() => { expect(screen.getByText(/共 3 个 Agent/)).toBeInTheDocument(); });
  });

  it("renders alive/dead status badges", async () => {
    mockFetch.mockImplementation((url: string) => {
      if (url.includes("/api/v1/agents")) return Promise.resolve(mockFetchResponse(mockAgents));
      return Promise.resolve(mockFetchResponse([]));
    });
    const { default: AgentsPage } = await import("@/app/agents/page");
    render(<AgentsPage />);
    await waitFor(() => { expect(screen.getAllByText("Alpha").length).toBeGreaterThanOrEqual(1); });
    expect(screen.getAllByText("存活").length).toBeGreaterThanOrEqual(2);
    expect(screen.getAllByText("死亡").length).toBeGreaterThanOrEqual(1);
  });

  it("renders filter buttons with counts", async () => {
    mockFetch.mockImplementation((url: string) => {
      if (url.includes("/api/v1/agents")) return Promise.resolve(mockFetchResponse(mockAgents));
      return Promise.resolve(mockFetchResponse([]));
    });
    const { default: AgentsPage } = await import("@/app/agents/page");
    render(<AgentsPage />);
    await waitFor(() => { expect(screen.getByText(/全部.*3/)).toBeInTheDocument(); });
  });

  it("renders search input", async () => {
    mockFetch.mockImplementation((url: string) => {
      if (url.includes("/api/v1/agents")) return Promise.resolve(mockFetchResponse(mockAgents));
      return Promise.resolve(mockFetchResponse([]));
    });
    const { default: AgentsPage } = await import("@/app/agents/page");
    render(<AgentsPage />);
    await waitFor(() => { expect(screen.getAllByText("Alpha").length).toBeGreaterThanOrEqual(1); });
    expect(screen.getByPlaceholderText("搜索 Agent 名称...")).toBeInTheDocument();
  });

  it("renders Chinese phase labels", async () => {
    mockFetch.mockImplementation((url: string) => {
      if (url.includes("/api/v1/agents")) return Promise.resolve(mockFetchResponse(mockAgents));
      return Promise.resolve(mockFetchResponse([]));
    });
    const { default: AgentsPage } = await import("@/app/agents/page");
    render(<AgentsPage />);
    await waitFor(() => { expect(screen.getAllByText("Alpha").length).toBeGreaterThanOrEqual(1); });
    expect(screen.getAllByText("成年").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("幼年").length).toBeGreaterThanOrEqual(1);
  });

  it("shows empty state when no agents", async () => {
    mockFetch.mockImplementation((url: string) => {
      if (url.includes("/api/v1/agents")) return Promise.resolve(mockFetchResponse([]));
      return Promise.resolve(mockFetchResponse([]));
    });
    const { default: AgentsPage } = await import("@/app/agents/page");
    render(<AgentsPage />);
    await waitFor(() => { expect(screen.getByText("暂无 Agent 数据")).toBeInTheDocument(); });
  });

  it("renders page title", async () => {
    mockFetch.mockImplementation((url: string) => {
      if (url.includes("/api/v1/agents")) return Promise.resolve(mockFetchResponse(mockAgents));
      return Promise.resolve(mockFetchResponse([]));
    });
    const { default: AgentsPage } = await import("@/app/agents/page");
    render(<AgentsPage />);
    await waitFor(() => { expect(screen.getByText("Agent 列表")).toBeInTheDocument(); });
  });
});
