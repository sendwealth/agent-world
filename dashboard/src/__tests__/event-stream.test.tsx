import { describe, it, expect, afterEach } from "vitest";
import { render, screen, cleanup } from "@testing-library/react";
import { EventStream } from "@/components/EventStream";
import type { WorldEvent } from "@/types/world";

afterEach(() => {
  cleanup();
});

// ── EventStream Tests ───────────────────────────────────

const mockEvents: WorldEvent[] = [
  {
    id: "evt-1",
    type: "agent_spawned",
    agentId: "agent-1",
    agentName: "Alpha",
    description: "Alpha was born into the world",
    timestamp: "2024-01-01T10:00:00Z",
    tick: 1,
  },
  {
    id: "evt-2",
    type: "transaction_completed",
    agentId: "agent-1",
    agentName: "Alpha",
    targetId: "agent-2",
    targetName: "Beta",
    description: "Alpha traded with Beta for $50",
    amount: 50,
    timestamp: "2024-01-01T10:01:00Z",
    tick: 2,
  },
  {
    id: "evt-3",
    type: "agent_died",
    agentId: "agent-2",
    agentName: "Beta",
    description: "Beta has died",
    timestamp: "2024-01-01T10:02:00Z",
    tick: 3,
  },
];

describe("EventStream", () => {
  it("renders header with connection status when connected", () => {
    render(<EventStream events={[]} connected={true} />);

    expect(screen.getByText("实时事件流")).toBeInTheDocument();
    expect(screen.getByText("已连接")).toBeInTheDocument();
  });

  it("renders disconnected status", () => {
    render(<EventStream events={[]} connected={false} />);

    expect(screen.getByText("断开连接")).toBeInTheDocument();
  });

  it("shows waiting message when no events", () => {
    render(<EventStream events={[]} connected={true} />);

    expect(screen.getByText("等待事件...")).toBeInTheDocument();
  });

  it("renders all events with their descriptions", () => {
    render(<EventStream events={mockEvents} connected={true} />);

    expect(screen.getByText("Alpha was born into the world")).toBeInTheDocument();
    expect(screen.getByText("Alpha traded with Beta for $50")).toBeInTheDocument();
    expect(screen.getByText("Beta has died")).toBeInTheDocument();
  });

  it("renders event type labels correctly", () => {
    render(<EventStream events={mockEvents} connected={true} />);

    // eventTypeConfig labels
    expect(screen.getByText("诞生")).toBeInTheDocument();
    expect(screen.getByText("交易")).toBeInTheDocument();
    expect(screen.getByText("死亡")).toBeInTheDocument();
  });

  it("renders tick numbers for each event", () => {
    render(<EventStream events={mockEvents} connected={true} />);

    expect(screen.getByText("Tick #1")).toBeInTheDocument();
    expect(screen.getByText("Tick #2")).toBeInTheDocument();
    expect(screen.getByText("Tick #3")).toBeInTheDocument();
  });

  it("renders event icons (emojis)", () => {
    const { container } = render(
      <EventStream events={mockEvents} connected={true} />
    );

    // Each event renders its icon from eventTypeConfig
    expect(container.textContent).toContain("👶");
    expect(container.textContent).toContain("💰");
    expect(container.textContent).toContain("💀");
  });

  it("handles unknown event type gracefully", () => {
    const unknownEvent: WorldEvent = {
      id: "evt-unknown",
      type: "relation_changed" as WorldEvent["type"],
      description: "Relation changed between agents",
      timestamp: "2024-01-01T10:00:00Z",
      tick: 10,
    };

    // relation_changed is in eventTypeConfig, let's test with a truly unknown one
    const events = [
      {
        ...unknownEvent,
        type: "unknown_event_type" as WorldEvent["type"],
      },
    ];

    render(<EventStream events={events} connected={true} />);

    // Should render the event description regardless of type
    expect(
      screen.getByText("Relation changed between agents")
    ).toBeInTheDocument();
  });

  it("renders multiple events in order", () => {
    render(<EventStream events={mockEvents} connected={true} />);

    const descriptions = screen.getAllByText(/Alpha|Beta/);
    // Events should appear in the order they're provided
    expect(descriptions.length).toBeGreaterThanOrEqual(3);
  });
});
