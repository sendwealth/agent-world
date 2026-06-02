import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, cleanup, waitFor, act } from "@testing-library/react";

vi.mock("next/navigation", () => ({
  usePathname: () => "/",
  useRouter: () => ({ push: vi.fn(), replace: vi.fn(), prefetch: vi.fn() }),
  useSearchParams: () => new URLSearchParams(),
}));

// We'll test SSEProvider context value by consuming it
afterEach(() => { cleanup(); });

describe("SSEProvider", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("exports useSSEContext hook", async () => {
    const { useSSEContext, SSEProvider } = await import("@/components/SSEProvider");
    expect(typeof useSSEContext).toBe("function");
    expect(typeof SSEProvider).toBe("function");
  });

  it("provides default context values", async () => {
    const { useSSEContext, SSEProvider } = await import("@/components/SSEProvider");

    let contextValue: ReturnType<typeof useSSEContext> | null = null;
    function TestConsumer() {
      contextValue = useSSEContext();
      return <div data-testid="context">{contextValue?.connected ? "connected" : "disconnected"}</div>;
    }

    render(
      <SSEProvider>
        <TestConsumer />
      </SSEProvider>
    );

    // Initial state should be disconnected (no real SSE connection in test)
    expect(contextValue).not.toBeNull();
    expect(typeof contextValue!.subscribe).toBe("function");
    expect(Array.isArray(contextValue!.events)).toBe(true);
  });

  it("subscribe returns unsubscribe function", async () => {
    const { useSSEContext, SSEProvider } = await import("@/components/SSEProvider");

    let subscribeFn: ((cb: (event: unknown) => void) => () => void) | null = null;
    function TestConsumer() {
      const ctx = useSSEContext();
      subscribeFn = ctx.subscribe;
      return <div />;
    }

    render(
      <SSEProvider>
        <TestConsumer />
      </SSEProvider>
    );

    expect(subscribeFn).not.toBeNull();
    const unsubscribe = subscribeFn!((event: unknown) => {});
    expect(typeof unsubscribe).toBe("function");
    unsubscribe();
  });
});
