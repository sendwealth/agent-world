import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, cleanup } from "@testing-library/react";

vi.mock("next/navigation", () => ({
  usePathname: () => "/agents",
  useRouter: () => ({ push: vi.fn(), replace: vi.fn(), prefetch: vi.fn() }),
  useSearchParams: () => new URLSearchParams(),
}));

vi.mock("next/link", () => ({
  default: ({ children, href }: { children: React.ReactNode; href: string }) => (
    <a href={href} data-testid="nav-link">{children}</a>
  ),
}));

// Mock NotificationPanel
vi.mock("@/components/NotificationPanel", () => ({
  NotificationPanel: () => <div data-testid="notification-panel" />,
}));

// Mock useNotifications
vi.mock("@/components/useNotifications", () => ({
  useNotifications: () => ({ notifications: [], unreadCount: 0, markAllRead: vi.fn() }),
}));

const mockSubscribe = vi.fn(() => () => {});
vi.mock("@/components/SSEProvider", () => ({
  SSEProvider: ({ children }: { children: React.ReactNode }) => <>{children}</>,
  useSSEContext: () => ({ events: [], connected: true, error: null, subscribe: mockSubscribe }),
}));

afterEach(() => { cleanup(); });

describe("Sidebar Navigation", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renders all main navigation items", async () => {
    const { Sidebar } = await import("@/components/Sidebar");
    render(<Sidebar />);

    // Main nav items appear twice (desktop + mobile), so use getAllByText
    expect(screen.getAllByText("世界概览").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("Agent 列表").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("任务板").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("组织关系图").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("治理面板").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("经济指标").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("进化树").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("Agent 动态").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("事件时间线").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("决策轨迹").length).toBeGreaterThanOrEqual(1);
  });

  it("renders settings nav items", async () => {
    const { Sidebar } = await import("@/components/Sidebar");
    render(<Sidebar />);
    expect(screen.getAllByText("Provider 管理").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("模型分配").length).toBeGreaterThanOrEqual(1);
  });

  it("renders nav links with correct hrefs", async () => {
    const { Sidebar } = await import("@/components/Sidebar");
    render(<Sidebar />);
    const links = screen.getAllByTestId("nav-link");
    const hrefs = links.map((l) => l.getAttribute("href"));
    expect(hrefs).toContain("/agents");
    expect(hrefs).toContain("/tasks");
    expect(hrefs).toContain("/economy");
    expect(hrefs).toContain("/governance");
    expect(hrefs).toContain("/organizations");
    expect(hrefs).toContain("/evolution");
    expect(hrefs).toContain("/feed");
    expect(hrefs).toContain("/timeline");
    expect(hrefs).toContain("/traces");
  });

  it("renders notification panel", async () => {
    const { Sidebar } = await import("@/components/Sidebar");
    render(<Sidebar />);
    expect(screen.getAllByTestId("notification-panel").length).toBeGreaterThanOrEqual(1);
  });
});
