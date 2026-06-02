import "@testing-library/jest-dom/vitest";

// Polyfill ResizeObserver for jsdom (used by some components)
class ResizeObserverMock {
  observe() {}
  unobserve() {}
  disconnect() {}
}
global.ResizeObserver = ResizeObserverMock as unknown as typeof ResizeObserver;
