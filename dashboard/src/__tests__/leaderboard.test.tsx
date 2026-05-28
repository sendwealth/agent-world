import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, cleanup, waitFor, act } from "@testing-library/react";
import { LeaderboardSection } from "@/components/Leaderboard";
import type { Leaderboard, LeaderboardEntry } from "@/types/world";

afterEach(() => {
  cleanup();
});

// ── Mock fetch for API calls ────────────────────────────

const mockFetch = vi.fn();
global.fetch = mockFetch;

function mockFetchResponse(data: unknown) {
  return {
    ok: true,
    status: 200,
    json: () => Promise.resolve(data),
  };
}

// ── Leaderboard Tests ───────────────────────────────────

describe("LeaderboardSection", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockFetch.mockReset();
    // Default: return empty data to avoid unhandled rejections
    mockFetch.mockResolvedValue({
      ok: true,
      status: 200,
      json: () => Promise.resolve(null),
    });
  });

  it("renders all four leaderboard table titles", async () => {
    const leaderboard: Leaderboard = {
      richest: [],
      longestLived: [],
      highestSkill: [],
      highestReputation: [],
    };

    mockFetch.mockImplementation((url: string) => {
      if (url.includes("leaderboard")) {
        return Promise.resolve(mockFetchResponse(leaderboard));
      }
      return Promise.resolve(mockFetchResponse([]));
    });

    await act(async () => {
      render(<LeaderboardSection statsTick={100} />);
    });

    expect(screen.getByText("最富有")).toBeInTheDocument();
    expect(screen.getByText("最长寿")).toBeInTheDocument();
    expect(screen.getByText("最高技能")).toBeInTheDocument();
    expect(screen.getByText("最高信誉")).toBeInTheDocument();
  });

  it("shows empty state when no data", async () => {
    const leaderboard: Leaderboard = {
      richest: [],
      longestLived: [],
      highestSkill: [],
      highestReputation: [],
    };

    mockFetch.mockImplementation((url: string) => {
      if (url.includes("leaderboard")) {
        return Promise.resolve(mockFetchResponse(leaderboard));
      }
      return Promise.resolve(mockFetchResponse([]));
    });

    await act(async () => {
      render(<LeaderboardSection statsTick={100} />);
    });

    // All four tables should show "暂无数据"
    const emptyStates = screen.getAllByText("暂无数据");
    expect(emptyStates.length).toBe(4);
  });

  it("renders leaderboard entries with agent names and formatted values", async () => {
    const leaderboard: Leaderboard = {
      richest: [
        { agentId: "a1", agentName: "Alpha", value: 10000, rank: 1 },
        { agentId: "a2", agentName: "Beta", value: 5000, rank: 2 },
      ],
      longestLived: [
        { agentId: "a3", agentName: "Gamma", value: 500, rank: 1 },
      ],
      highestSkill: [],
      highestReputation: [],
    };

    mockFetch.mockImplementation((url: string) => {
      if (url.includes("leaderboard")) {
        return Promise.resolve(mockFetchResponse(leaderboard));
      }
      return Promise.resolve(mockFetchResponse([]));
    });

    await act(async () => {
      render(<LeaderboardSection statsTick={100} />);
    });

    // Wait for async state to update
    await waitFor(() => {
      expect(screen.getByText("Alpha")).toBeInTheDocument();
    });

    expect(screen.getByText("Beta")).toBeInTheDocument();
    expect(screen.getByText("Gamma")).toBeInTheDocument();

    // Rank badges appear across tables (rank 1 in both richest and longestLived)
    const rank1s = screen.getAllByText("1");
    expect(rank1s.length).toBeGreaterThanOrEqual(2);
    expect(screen.getByText("2")).toBeInTheDocument();

    // Rich table values formatted as currency
    expect(screen.getByText("$10,000")).toBeInTheDocument();
    expect(screen.getByText("$5,000")).toBeInTheDocument();

    // Longest lived values formatted as ticks
    expect(screen.getByText("500 Tick")).toBeInTheDocument();
  });

  it("renders rank badges and entries for multiple agents", async () => {
    const leaderboard: Leaderboard = {
      richest: [
        { agentId: "a1", agentName: "Gold", value: 1000, rank: 1 },
        { agentId: "a2", agentName: "Silver", value: 800, rank: 2 },
        { agentId: "a3", agentName: "Bronze", value: 600, rank: 3 },
        { agentId: "a4", agentName: "OtherAgent", value: 400, rank: 4 },
      ],
      longestLived: [],
      highestSkill: [],
      highestReputation: [],
    };

    mockFetch.mockImplementation((url: string) => {
      if (url.includes("leaderboard")) {
        return Promise.resolve(mockFetchResponse(leaderboard));
      }
      return Promise.resolve(mockFetchResponse([]));
    });

    await act(async () => {
      render(<LeaderboardSection statsTick={100} />);
    });

    await waitFor(() => {
      expect(screen.getByText("Gold")).toBeInTheDocument();
    });

    expect(screen.getByText("Silver")).toBeInTheDocument();
    expect(screen.getByText("Bronze")).toBeInTheDocument();
    expect(screen.getByText("OtherAgent")).toBeInTheDocument();
  });
});
