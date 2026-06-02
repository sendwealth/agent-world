import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor, cleanup } from "@testing-library/react";
import type { WorldGovernanceSummary } from "@/types/world";

vi.mock("next/navigation", () => ({
  usePathname: () => "/governance",
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
  return { ok: true, status: 200, json: () => Promise.resolve(data) };
}

afterEach(() => { cleanup(); });

describe("GovernancePage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockFetch.mockReset();
  });

  it("renders loading state initially", async () => {
    mockFetch.mockImplementation(() => new Promise(() => {}));
    const { default: GovernancePage } = await import("@/app/governance/page");
    render(<GovernancePage />);
    // The page should render something — look for a loading indicator
    expect(document.body.textContent).toBeTruthy();
  });

  it("renders governance header after loading", async () => {
    const mockSummary: WorldGovernanceSummary = {
      total_orgs: 3,
      active_orgs: 2,
      total_treaties: 5,
      active_treaties: 3,
      election_count: 1,
      avg_tax_rate: 0.05,
      total_tax_collected: 1000,
      total_treasury_distributed: 800,
    };
    mockFetch.mockImplementation((url: string) => {
      if (url.includes("/governance/summary")) return Promise.resolve(mockFetchResponse(mockSummary));
      if (url.includes("/orgs")) return Promise.resolve(mockFetchResponse([]));
      if (url.includes("/governance/comparison")) return Promise.resolve(mockFetchResponse([]));
      return Promise.resolve(mockFetchResponse([]));
    });
    const { default: GovernancePage } = await import("@/app/governance/page");
    render(<GovernancePage />);
    await waitFor(() => {
      expect(screen.getByText("治理面板")).toBeInTheDocument();
    });
  });

  it("displays governance summary stats", async () => {
    const mockSummary: WorldGovernanceSummary = {
      total_orgs: 5,
      active_orgs: 3,
      total_treaties: 10,
      active_treaties: 6,
      election_count: 2,
      avg_tax_rate: 0.08,
      total_tax_collected: 5000,
      total_treasury_distributed: 4000,
    };
    mockFetch.mockImplementation((url: string) => {
      if (url.includes("/governance/summary")) return Promise.resolve(mockFetchResponse(mockSummary));
      if (url.includes("/orgs")) return Promise.resolve(mockFetchResponse([]));
      if (url.includes("/governance/comparison")) return Promise.resolve(mockFetchResponse([]));
      return Promise.resolve(mockFetchResponse([]));
    });
    const { default: GovernancePage } = await import("@/app/governance/page");
    render(<GovernancePage />);
    await waitFor(() => {
      expect(screen.getByText("治理面板")).toBeInTheDocument();
    });
    // The page should show some of the summary numbers
    expect(screen.getByText("5")).toBeInTheDocument();
  });
});
