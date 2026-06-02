import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor, cleanup } from "@testing-library/react";
import type { WorldSnapshotData, WorldStats } from "@/types/world";

vi.mock("next/navigation", () => ({
  usePathname: () => "/economy",
  useRouter: () => ({ push: vi.fn(), replace: vi.fn(), prefetch: vi.fn() }),
  useSearchParams: () => new URLSearchParams(),
}));

const mockSubscribe = vi.fn(() => () => {});
vi.mock("@/components/SSEProvider", () => ({
  SSEProvider: ({ children }: { children: React.ReactNode }) => <>{children}</>,
  useSSEContext: () => ({ events: [], connected: true, error: null, subscribe: mockSubscribe }),
}));

vi.mock("recharts", () => ({
  LineChart: ({ children }: { children: React.ReactNode }) => <div data-testid="line-chart">{children}</div>,
  Line: () => null, XAxis: () => null, YAxis: () => null, CartesianGrid: () => null, Tooltip: () => null,
  ResponsiveContainer: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
  AreaChart: ({ children }: { children: React.ReactNode }) => <div data-testid="area-chart">{children}</div>,
  Area: () => null, BarChart: ({ children }: { children: React.ReactNode }) => <div>{children}</div>, Bar: () => null,
}));

const mockFetch = vi.fn();
global.fetch = mockFetch;

function mockFetchResponse(data: unknown) {
  return { ok: true, status: 200, json: () => Promise.resolve(data) };
}

const mockSnapshots: WorldSnapshotData[] = [
  { tick: 100, timestamp: Date.now(), total_population: 50, active_agents: 45, gdp: 10000, gini_coefficient: 0.35, skill_distribution_top5: [{ skill_name: "mining", agent_count: 20, avg_level: 3.5 }], key_events: [] },
  { tick: 200, timestamp: Date.now(), total_population: 60, active_agents: 55, gdp: 15000, gini_coefficient: 0.40, skill_distribution_top5: [{ skill_name: "trading", agent_count: 30, avg_level: 4.2 }], key_events: [] },
];

const mockStats: WorldStats = { agentCount: 60, aliveCount: 55, deadCount: 5, gdp: 15000, inflationRate: 2.5, totalMoney: 200000, tick: 200 };

afterEach(() => { cleanup(); });

describe("EconomyPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockFetch.mockReset();
  });

  it("renders loading state initially", async () => {
    mockFetch.mockImplementation(() => new Promise(() => {}));
    const { default: EconomyPage } = await import("@/app/economy/page");
    render(<EconomyPage />);
    expect(screen.getByText("正在加载经济数据...")).toBeInTheDocument();
  });

  it("renders header and key indicators after loading", async () => {
    mockFetch.mockImplementation((url: string) => {
      if (url.includes("/snapshots")) return Promise.resolve(mockFetchResponse(mockSnapshots));
      if (url.includes("/world/stats")) return Promise.resolve(mockFetchResponse(mockStats));
      return Promise.resolve(mockFetchResponse([]));
    });
    const { default: EconomyPage } = await import("@/app/economy/page");
    render(<EconomyPage />);
    await waitFor(() => { expect(screen.getByText("经济指标面板")).toBeInTheDocument(); });
    expect(screen.getByText("GDP")).toBeInTheDocument();
    expect(screen.getByText("通胀率")).toBeInTheDocument();
    expect(screen.getByText("基尼系数")).toBeInTheDocument();
    expect(screen.getByText("总货币量")).toBeInTheDocument();
  });

  it("displays formatted GDP value", async () => {
    mockFetch.mockImplementation((url: string) => {
      if (url.includes("/snapshots")) return Promise.resolve(mockFetchResponse(mockSnapshots));
      if (url.includes("/world/stats")) return Promise.resolve(mockFetchResponse(mockStats));
      return Promise.resolve(mockFetchResponse([]));
    });
    const { default: EconomyPage } = await import("@/app/economy/page");
    render(<EconomyPage />);
    await waitFor(() => {
      expect(screen.getAllByText(/\$15,000/).length).toBeGreaterThanOrEqual(1);
    });
  });

  it("displays inflation rate formatted", async () => {
    mockFetch.mockImplementation((url: string) => {
      if (url.includes("/snapshots")) return Promise.resolve(mockFetchResponse(mockSnapshots));
      if (url.includes("/world/stats")) return Promise.resolve(mockFetchResponse(mockStats));
      return Promise.resolve(mockFetchResponse([]));
    });
    const { default: EconomyPage } = await import("@/app/economy/page");
    render(<EconomyPage />);
    await waitFor(() => { expect(screen.getByText("2.50%")).toBeInTheDocument(); });
  });

  it("displays gini coefficient interpretation", async () => {
    mockFetch.mockImplementation((url: string) => {
      if (url.includes("/snapshots")) return Promise.resolve(mockFetchResponse(mockSnapshots));
      if (url.includes("/world/stats")) return Promise.resolve(mockFetchResponse(mockStats));
      return Promise.resolve(mockFetchResponse([]));
    });
    const { default: EconomyPage } = await import("@/app/economy/page");
    render(<EconomyPage />);
    await waitFor(() => { expect(screen.getByText("中等")).toBeInTheDocument(); });
  });

  it("renders chart sections", async () => {
    mockFetch.mockImplementation((url: string) => {
      if (url.includes("/snapshots")) return Promise.resolve(mockFetchResponse(mockSnapshots));
      if (url.includes("/world/stats")) return Promise.resolve(mockFetchResponse(mockStats));
      return Promise.resolve(mockFetchResponse([]));
    });
    const { default: EconomyPage } = await import("@/app/economy/page");
    render(<EconomyPage />);
    await waitFor(() => { expect(screen.getByText("GDP 走势")).toBeInTheDocument(); });
    expect(screen.getByText("基尼系数走势")).toBeInTheDocument();
  });
});
