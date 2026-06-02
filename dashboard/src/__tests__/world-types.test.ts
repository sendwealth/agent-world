import { describe, it, expect } from "vitest";
import type {
  Agent,
  WorldStats,
  WorldEvent,
  Task,
  TaskStatus,
  Organization,
  CoordinationTask,
  CoordinationTaskStatus,
  LeaderboardEntry,
  EventType,
} from "@/types/world";

// ── Type validation tests ──────────────────────────────────
// These tests ensure TypeScript types match the expected runtime shapes.

describe("World Types", () => {
  it("Agent type matches expected shape", () => {
    const agent: Agent = {
      id: "agent-1",
      name: "Alpha",
      phase: "adult",
      money: 1000,
      tokens: 200,
      reputation: 0.8,
      skills: { mining: 5, trading: 3 },
      alive: true,
      age: 100,
      createdAt: "2024-01-01",
    };
    expect(agent.id).toBe("agent-1");
    expect(agent.alive).toBe(true);
    expect(agent.skills.mining).toBe(5);
  });

  it("WorldStats type matches expected shape", () => {
    const stats: WorldStats = {
      agentCount: 100,
      aliveCount: 80,
      deadCount: 20,
      gdp: 50000,
      inflationRate: 2.5,
      totalMoney: 200000,
      tick: 500,
    };
    expect(stats.agentCount).toBe(100);
    expect(stats.inflationRate).toBe(2.5);
  });

  it("WorldEvent type matches expected shape", () => {
    const event: WorldEvent = {
      id: "evt-1",
      type: "agent_spawn",
      agentId: "a1",
      agentName: "Alpha",
      description: "Alpha spawned",
      timestamp: "2024-01-01T00:00:00Z",
      tick: 1,
    };
    expect(event.type).toBe("agent_spawn");
    expect(event.tick).toBe(1);
  });

  it("Task type matches expected shape", () => {
    const task: Task = {
      id: "task-1",
      title: "Gather Resources",
      description: "Collect wood",
      status: "published",
      reward: 50,
      escrow_held: true,
      publisher_id: "agent-1",
      assignee_id: null,
      result: null,
      expires_at: null,
      created_tick: 10,
    };
    expect(task.status).toBe("published");
    expect(task.reward).toBe(50);
  });

  it("all TaskStatus values are valid", () => {
    const statuses: TaskStatus[] = [
      "published", "claimed", "in_progress", "submitted", "reviewed", "completed", "expired",
    ];
    expect(statuses).toHaveLength(7);
  });

  it("all CoordinationTaskStatus values are valid", () => {
    const statuses: CoordinationTaskStatus[] = [
      "open", "in_progress", "all_submitted", "completed", "expired", "cancelled",
    ];
    expect(statuses).toHaveLength(6);
  });

  it("Organization type has required fields", () => {
    const org: Organization = {
      id: "org-1",
      name: "Test Corp",
      type: "company",
      status: "active",
      treasury: 10000,
      debts: 500,
      member_count: 10,
      members: [],
      created_tick: 1,
      last_activity_tick: 100,
    };
    expect(org.type).toBe("company");
    expect(org.member_count).toBe(10);
  });

  it("EventType covers all known event types", () => {
    const types: EventType[] = [
      "agent_spawn", "agent_death", "trade", "task_created", "task_claimed",
      "task_completed", "message", "skill_up", "reputation_change",
      "inflation", "investment", "tax",
      "leadership_election_started", "leadership_changed",
      "treaty_proposed", "treaty_signed", "treaty_broken",
      "relation_changed", "coordination_task_created",
    ];
    expect(types.length).toBeGreaterThan(10);
  });

  it("LeaderboardEntry has required fields", () => {
    const entry: LeaderboardEntry = {
      agentId: "a1",
      agentName: "Alpha",
      value: 1000,
      rank: 1,
    };
    expect(entry.rank).toBe(1);
  });
});
