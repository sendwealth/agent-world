import { describe, it, expect, afterEach } from "vitest";
import { render, screen, cleanup } from "@testing-library/react";
import { StatCard } from "@/components/StatCard";
import { StatCards } from "@/components/StatCards";
import type { WorldStats } from "@/types/world";

afterEach(() => {
  cleanup();
});

// ── StatCard Tests ─────────────────────────────────────

describe("StatCard", () => {
  it("renders title and numeric value", () => {
    render(
      <StatCard title="Agent 总数" value={42} icon={<span>📊</span>} color="blue" />
    );

    expect(screen.getByText("Agent 总数")).toBeInTheDocument();
    expect(screen.getByText("42")).toBeInTheDocument();
  });

  it("renders string value as-is", () => {
    render(
      <StatCard title="状态" value="运行中" icon={<span>🟢</span>} color="green" />
    );

    expect(screen.getByText("运行中")).toBeInTheDocument();
  });

  it("renders subtitle when provided", () => {
    render(
      <StatCard
        title="死亡数"
        value={5}
        icon={<span>💀</span>}
        subtitle="Tick #100"
        color="red"
      />
    );

    expect(screen.getByText("Tick #100")).toBeInTheDocument();
  });

  it("does not render subtitle when omitted", () => {
    const { container } = render(
      <StatCard title="测试" value={1} icon={<span>🎯</span>} color="amber" />
    );

    // Should not have any element with text-xs class containing a subtitle
    const subtitles = container.querySelectorAll("p.text-xs");
    expect(subtitles.length).toBe(0);
  });

  it("localizes large numbers with toLocaleString", () => {
    render(
      <StatCard title="GDP" value={1234567} icon={<span>💰</span>} color="green" />
    );

    // toLocaleString on 1234567 in most locales produces "1,234,567" or similar
    const valueEl = screen.getByText(/1.234.567|1,234,567/);
    expect(valueEl).toBeInTheDocument();
  });

  it("applies correct color class for each color variant", () => {
    const { container: blueContainer } = render(
      <StatCard title="Blue" value={1} icon={<span>B</span>} color="blue" />
    );
    expect(blueContainer.querySelector(".bg-blue-500\\/10")).toBeTruthy();

    const { container: greenContainer } = render(
      <StatCard title="Green" value={1} icon={<span>G</span>} color="green" />
    );
    expect(greenContainer.querySelector(".bg-green-500\\/10")).toBeTruthy();

    const { container: redContainer } = render(
      <StatCard title="Red" value={1} icon={<span>R</span>} color="red" />
    );
    expect(redContainer.querySelector(".bg-red-500\\/10")).toBeTruthy();

    const { container: amberContainer } = render(
      <StatCard title="Amber" value={1} icon={<span>A</span>} color="amber" />
    );
    expect(amberContainer.querySelector(".bg-amber-500\\/10")).toBeTruthy();
  });
});

// ── StatCards (composite) Tests ─────────────────────────

describe("StatCards", () => {
  it("renders loading placeholders when stats is null", () => {
    render(<StatCards stats={null} />);

    // All four cards show "—" when stats is null
    const dashes = screen.getAllByText("—");
    expect(dashes.length).toBe(4);

    // Subtitles show "加载中..."
    const loadingTexts = screen.getAllByText("加载中...");
    expect(loadingTexts.length).toBeGreaterThanOrEqual(3);
  });

  it("renders all four stat cards with real data", () => {
    const stats: WorldStats = {
      agentCount: 100,
      aliveCount: 80,
      deadCount: 20,
      totalMoney: 200000,
      totalTokens: 50000,
      tick: 500,
      taskCount: 12,
    };

    render(<StatCards stats={stats} />);

    expect(screen.getByText("Agent 总数")).toBeInTheDocument();
    expect(screen.getByText("总货币")).toBeInTheDocument();
    expect(screen.getByText("任务数")).toBeInTheDocument();
    expect(screen.getByText("死亡数")).toBeInTheDocument();

    // Value rendering
    expect(screen.getByText("100")).toBeInTheDocument();
    expect(screen.getByText("20")).toBeInTheDocument();
    expect(screen.getByText("12")).toBeInTheDocument();
  });

  it("shows alive count in agent card subtitle", () => {
    const stats: WorldStats = {
      agentCount: 100,
      aliveCount: 80,
      deadCount: 20,
      totalMoney: 200000,
      totalTokens: 50000,
      tick: 500,
      taskCount: 12,
    };

    render(<StatCards stats={stats} />);

    expect(screen.getByText("存活: 80")).toBeInTheDocument();
  });

  it("shows tick number in deaths card subtitle", () => {
    const stats: WorldStats = {
      agentCount: 100,
      aliveCount: 80,
      deadCount: 20,
      totalMoney: 200000,
      totalTokens: 50000,
      tick: 500,
      taskCount: 12,
    };

    render(<StatCards stats={stats} />);

    // Tick #500 appears in both the task count and deaths card subtitles
    const tickTexts = screen.getAllByText("Tick #500");
    expect(tickTexts.length).toBeGreaterThanOrEqual(1);
  });
});
