import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor, cleanup } from "@testing-library/react";
import type { Task } from "@/types/world";

vi.mock("next/navigation", () => ({
  usePathname: () => "/tasks",
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

const mockTasks: Task[] = [
  { id: "task-1", title: "Gather Resources", description: "Collect 100 wood", status: "published", reward: 50, escrow_held: true, publisher_id: "agent-1", assignee_id: null, result: null, expires_at: null, created_tick: 10 },
  { id: "task-2", title: "Build House", description: "Build a house", status: "claimed", reward: 200, escrow_held: true, publisher_id: "agent-2", assignee_id: "agent-3", result: null, expires_at: null, created_tick: 20 },
  { id: "task-3", title: "Trade Goods", description: "Trade with neighbor", status: "completed", reward: 100, escrow_held: false, publisher_id: "agent-1", assignee_id: "agent-2", result: "Done", expires_at: null, created_tick: 30 },
];

afterEach(() => { cleanup(); });

describe("TasksPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockFetch.mockReset();
    // Default mock to avoid unhandled rejections
    mockFetch.mockResolvedValue({ ok: true, status: 200, json: () => Promise.resolve([]) });
  });

  it("renders page header", async () => {
    mockFetch.mockImplementation((url: string) => {
      if (url.includes("/tasks")) return Promise.resolve(mockFetchResponse(mockTasks));
      if (url.includes("/coordination-tasks")) return Promise.resolve(mockFetchResponse([]));
      return Promise.resolve(mockFetchResponse([]));
    });
    const { default: TasksPage } = await import("@/app/tasks/page");
    render(<TasksPage />);
    await waitFor(() => { expect(screen.getByText("任务板")).toBeInTheDocument(); });
  });

  it("renders task titles in list", async () => {
    mockFetch.mockImplementation((url: string) => {
      if (url.includes("/tasks")) return Promise.resolve(mockFetchResponse(mockTasks));
      if (url.includes("/coordination-tasks")) return Promise.resolve(mockFetchResponse([]));
      return Promise.resolve(mockFetchResponse([]));
    });
    const { default: TasksPage } = await import("@/app/tasks/page");
    render(<TasksPage />);
    await waitFor(() => { expect(screen.getByText("Gather Resources")).toBeInTheDocument(); });
    expect(screen.getByText("Build House")).toBeInTheDocument();
    expect(screen.getByText("Trade Goods")).toBeInTheDocument();
  });

  it("renders task count summary", async () => {
    mockFetch.mockImplementation((url: string) => {
      if (url.includes("/tasks")) return Promise.resolve(mockFetchResponse(mockTasks));
      if (url.includes("/coordination-tasks")) return Promise.resolve(mockFetchResponse([]));
      return Promise.resolve(mockFetchResponse([]));
    });
    const { default: TasksPage } = await import("@/app/tasks/page");
    render(<TasksPage />);
    await waitFor(() => { expect(screen.getByText(/共 3 个任务/)).toBeInTheDocument(); });
  });

  it("renders create task button", async () => {
    mockFetch.mockImplementation((url: string) => {
      if (url.includes("/tasks")) return Promise.resolve(mockFetchResponse(mockTasks));
      if (url.includes("/coordination-tasks")) return Promise.resolve(mockFetchResponse([]));
      return Promise.resolve(mockFetchResponse([]));
    });
    const { default: TasksPage } = await import("@/app/tasks/page");
    render(<TasksPage />);
    await waitFor(() => { expect(screen.getByText("+ 创建任务")).toBeInTheDocument(); });
  });

  it("renders filter buttons", async () => {
    mockFetch.mockImplementation((url: string) => {
      if (url.includes("/tasks")) return Promise.resolve(mockFetchResponse(mockTasks));
      if (url.includes("/coordination-tasks")) return Promise.resolve(mockFetchResponse([]));
      return Promise.resolve(mockFetchResponse([]));
    });
    const { default: TasksPage } = await import("@/app/tasks/page");
    render(<TasksPage />);
    await waitFor(() => { expect(screen.getByText(/全部.*3/)).toBeInTheDocument(); });
  });

  it("renders status labels for tasks", async () => {
    mockFetch.mockImplementation((url: string) => {
      if (url.includes("/tasks")) return Promise.resolve(mockFetchResponse(mockTasks));
      if (url.includes("/coordination-tasks")) return Promise.resolve(mockFetchResponse([]));
      return Promise.resolve(mockFetchResponse([]));
    });
    const { default: TasksPage } = await import("@/app/tasks/page");
    render(<TasksPage />);
    await waitFor(() => { expect(screen.getByText("Gather Resources")).toBeInTheDocument(); });
    // STATUS_CONFIG labels: published=已发布, claimed=已认领, completed=已完成
    expect(screen.getAllByText(/已发布|已认领|已完成/).length).toBeGreaterThanOrEqual(1);
  });

  it("renders team tasks section", async () => {
    mockFetch.mockImplementation((url: string) => {
      if (url.includes("/tasks")) return Promise.resolve(mockFetchResponse(mockTasks));
      if (url.includes("/coordination-tasks")) return Promise.resolve(mockFetchResponse([]));
      return Promise.resolve(mockFetchResponse([]));
    });
    const { default: TasksPage } = await import("@/app/tasks/page");
    render(<TasksPage />);
    await waitFor(() => { expect(screen.getByText("团队任务")).toBeInTheDocument(); });
  });

  it("shows empty state when no tasks", async () => {
    mockFetch.mockImplementation((url: string) => {
      if (url.includes("/tasks")) return Promise.resolve(mockFetchResponse([]));
      if (url.includes("/coordination-tasks")) return Promise.resolve(mockFetchResponse([]));
      return Promise.resolve(mockFetchResponse([]));
    });
    const { default: TasksPage } = await import("@/app/tasks/page");
    render(<TasksPage />);
    await waitFor(() => { expect(screen.getByText("暂无任务数据")).toBeInTheDocument(); });
  });
});
