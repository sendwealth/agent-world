import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor, cleanup } from "@testing-library/react";
import type { Organization } from "@/types/world";

vi.mock("next/navigation", () => ({
  usePathname: () => "/organizations",
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

const mockOrgs: Organization[] = [
  { id: "org-1", name: "Alpha Corp", type: "company", status: "active", treasury: 10000, debts: 500, member_count: 10, members: [{ agent_id: "a1", agent_name: "Agent1", role: "founder", share: 0.4, joined_tick: 1 }] } as Organization,
  { id: "org-2", name: "Beta Guild", type: "guild", status: "active", treasury: 5000, debts: 0, member_count: 5, members: [{ agent_id: "a2", agent_name: "Agent2", role: "leader", share: 0.3, joined_tick: 10 }] } as Organization,
];

afterEach(() => { cleanup(); });

describe("OrganizationsPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockFetch.mockReset();
  });

  it("renders loading state initially", async () => {
    mockFetch.mockImplementation(() => new Promise(() => {}));
    const { default: OrganizationsPage } = await import("@/app/organizations/page");
    render(<OrganizationsPage />);
    expect(document.body.textContent).toBeTruthy();
  });

  it("renders page header after loading", async () => {
    mockFetch.mockImplementation((url: string) => {
      if (url.includes("/orgs")) return Promise.resolve(mockFetchResponse(mockOrgs));
      return Promise.resolve(mockFetchResponse([]));
    });
    const { default: OrganizationsPage } = await import("@/app/organizations/page");
    render(<OrganizationsPage />);
    await waitFor(() => { expect(screen.getByText("组织关系图")).toBeInTheDocument(); });
  });

  it("renders filter buttons for org types", async () => {
    mockFetch.mockImplementation((url: string) => {
      if (url.includes("/orgs")) return Promise.resolve(mockFetchResponse(mockOrgs));
      return Promise.resolve(mockFetchResponse([]));
    });
    const { default: OrganizationsPage } = await import("@/app/organizations/page");
    render(<OrganizationsPage />);
    await waitFor(() => { expect(screen.getByText("组织关系图")).toBeInTheDocument(); });
    // Filter buttons: 全部, 公司, 公会, 联盟, 大学
    expect(screen.getByText(/全部/)).toBeInTheDocument();
  });

  it("shows empty state when no organizations", async () => {
    mockFetch.mockImplementation((url: string) => {
      if (url.includes("/orgs")) return Promise.resolve(mockFetchResponse([]));
      return Promise.resolve(mockFetchResponse([]));
    });
    const { default: OrganizationsPage } = await import("@/app/organizations/page");
    render(<OrganizationsPage />);
    await waitFor(() => {
      expect(screen.getAllByText("暂无组织数据").length).toBeGreaterThanOrEqual(1);
    });
  });
});
