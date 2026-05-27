import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor, cleanup } from "@testing-library/react";
import type { Oracle, Bounty, HumanInfluenceEntry } from "@/types/world";

// ── Mock Next.js dependencies ────────────────────────────

vi.mock("next/navigation", () => ({
  usePathname: () => "/human/agents",
  useRouter: () => ({ push: vi.fn(), replace: vi.fn(), prefetch: vi.fn() }),
  useSearchParams: () => new URLSearchParams(),
}));

// Mock SSEProvider context
const mockSubscribe = vi.fn(() => () => {});
vi.mock("@/components/SSEProvider", () => ({
  SSEProvider: ({ children }: { children: React.ReactNode }) => <>{children}</>,
  useSSEContext: () => ({
    events: [],
    connected: true,
    error: null,
    subscribe: mockSubscribe,
  }),
}));

// Mock fetch for API calls
const mockFetch = vi.fn();
global.fetch = mockFetch;

function mockFetchResponse(data: unknown, ok = true, status = 200) {
  return {
    ok,
    status,
    json: () => Promise.resolve(data),
    body: null,
  };
}

// ── Human Types Tests ─────────────────────────────────────

afterEach(() => {
  cleanup();
});

describe("Human Participation Types", () => {
  it("exports correct oracle types", () => {
    const oracle: Oracle = {
      id: "test-id",
      human_id: "human-1",
      oracle_type: "guidance",
      target_agent_id: "agent-1",
      content: "Test oracle",
      status: "pending",
      agent_response: null,
      created_tick: 1,
      delivered_tick: null,
    };
    expect(oracle.oracle_type).toBe("guidance");
    expect(oracle.status).toBe("pending");
  });

  it("exports correct bounty types", () => {
    const bounty: Bounty = {
      id: "bounty-1",
      human_id: "human-1",
      title: "Test Bounty",
      description: "A test",
      reward: 100,
      target_agent_id: null,
      status: "open",
      claimant_agent_id: null,
      result: null,
      expires_tick: null,
      created_tick: 1,
    };
    expect(bounty.status).toBe("open");
    expect(bounty.reward).toBe(100);
  });

  it("exports correct human influence entry", () => {
    const entry: HumanInfluenceEntry = {
      human_id: "h1",
      display_name: "Test Human",
      total_influence: 100,
      oracle_count: 5,
      bounty_count: 3,
      agents_affected: 10,
      economic_impact: 50,
      political_impact: 30,
      cultural_impact: 20,
    };
    expect(entry.total_influence).toBe(100);
    expect(entry.oracle_count).toBe(5);
  });
});

// ── Human Agents Page Tests ───────────────────────────────

describe("Human Agents Page", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockFetch.mockReset();
  });

  it("renders loading state initially", async () => {
    mockFetch.mockImplementation(() => new Promise(() => {})); // Never resolves

    const { default: HumanAgentsPage } = await import(
      "@/app/human/agents/page"
    );
    render(<HumanAgentsPage />);

    expect(screen.getByText(/正在加载 Agent 数据/i)).toBeInTheDocument();
  });

  it("renders claimed agents section", async () => {
    const mockAgents = [
      {
        id: "agent-1",
        name: "TestAgent",
        phase: "adult",
        money: 100,
        tokens: 50,
        reputation: 0.8,
        skills: {},
        alive: true,
        age: 10,
        createdAt: "2024-01-01",
      },
    ];
    const mockClaimed: never[] = [];

    mockFetch
      .mockImplementationOnce(() =>
        Promise.resolve(mockFetchResponse(mockAgents))
      )
      .mockImplementationOnce(() =>
        Promise.resolve(mockFetchResponse(mockClaimed))
      );

    const { default: HumanAgentsPage } = await import(
      "@/app/human/agents/page"
    );
    render(<HumanAgentsPage />);

    await waitFor(() => {
      expect(screen.getByText("TestAgent")).toBeInTheDocument();
    });
  });
});

// ── API Client Tests ──────────────────────────────────────

describe("API Client", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockFetch.mockReset();
  });

  it("fetchJSON makes GET request with retry", async () => {
    const { fetchJSON } = await import("@/lib/api");
    const testData = [{ id: "1", name: "Test" }];

    mockFetch.mockResolvedValueOnce({
      ok: true,
      status: 200,
      json: () => Promise.resolve(testData),
    });

    const result = await fetchJSON<typeof testData>("/api/v1/human/oracles");
    expect(result).toEqual(testData);
    expect(mockFetch).toHaveBeenCalledWith("/api/v1/human/oracles");
  });

  it("postJSON makes POST request without retry", async () => {
    const { postJSON } = await import("@/lib/api");
    const responseData = { id: "1", status: "created" };

    mockFetch.mockResolvedValueOnce({
      ok: true,
      status: 201,
      json: () => Promise.resolve(responseData),
    });

    const result = await postJSON("/api/v1/human/oracles", {
      human_id: "h1",
      oracle_type: "guidance",
      target_agent_id: "a1",
      content: "test",
    });

    expect(result).toEqual(responseData);
    expect(mockFetch).toHaveBeenCalledWith("/api/v1/human/oracles", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        human_id: "h1",
        oracle_type: "guidance",
        target_agent_id: "a1",
        content: "test",
      }),
    });
  });
});

// ── Notification Hook Tests ───────────────────────────────

describe("useNotifications Hook", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    localStorage.clear();
    mockSubscribe.mockReset();
    mockSubscribe.mockReturnValue(() => {});
  });

  it("starts with empty notifications when localStorage is empty", async () => {
    const { useNotifications } = await import(
      "@/components/useNotifications"
    );

    function TestComponent() {
      const { notifications, unreadCount } = useNotifications();
      return (
        <div>
          <span data-testid="count">{notifications.length}</span>
          <span data-testid="unread">{unreadCount}</span>
        </div>
      );
    }

    render(<TestComponent />);

    expect(screen.getByTestId("count").textContent).toBe("0");
    expect(screen.getByTestId("unread").textContent).toBe("0");
  });

  it("loads notifications from localStorage", async () => {
    const stored = [
      {
        id: "n1",
        type: "agent_death",
        title: "Agent Death",
        description: "Test agent died",
        tick: 10,
        timestamp: Date.now(),
        read: false,
      },
    ];
    localStorage.setItem("agent-world-notifications", JSON.stringify(stored));

    const { useNotifications } = await import(
      "@/components/useNotifications"
    );

    function TestComponent() {
      const { notifications, unreadCount } = useNotifications();
      return (
        <div>
          <span data-testid="count">{notifications.length}</span>
          <span data-testid="unread">{unreadCount}</span>
        </div>
      );
    }

    render(<TestComponent />);

    expect(screen.getByTestId("count").textContent).toBe("1");
    expect(screen.getByTestId("unread").textContent).toBe("1");
  });
});

// ── Sidebar Navigation Tests ──────────────────────────────

describe("Sidebar Navigation", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renders human participation nav section", async () => {
    const { Sidebar } = await import("@/components/Sidebar");
    render(<Sidebar />);

    // Check human section heading (appears twice: desktop + mobile)
    expect(screen.getAllByText("人类参与").length).toBeGreaterThanOrEqual(1);

    // Check human nav items
    expect(screen.getAllByText("我的 Agent").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("神谕编辑器").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("悬赏市场").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("投资组合").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("影响力排行").length).toBeGreaterThanOrEqual(1);
  });

  it("renders notification bell", async () => {
    const { Sidebar } = await import("@/components/Sidebar");
    render(<Sidebar />);

    const bellButtons = screen.getAllByLabelText("通知");
    expect(bellButtons.length).toBeGreaterThan(0);
  });
});
