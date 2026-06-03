import type { WorldEvent } from "@/types/world";

/**
 * Raw SSE event shape from Rust world-engine.
 * Rust uses `#[serde(tag = "type", content = "payload", rename_all = "snake_case")]`
 * which produces `{ type: "agent_spawned", payload: { agent_id: "...", ... } }`.
 */
interface RawSSEEvent {
  type: string;
  payload: Record<string, unknown>;
}

let eventCounter = 0;

/**
 * Transform a raw SSE event from the Rust backend into the flat WorldEvent
 * shape expected by the frontend dashboard.
 *
 * Rust output:  { type: "agent_spawned", payload: { agent_id: "x", name: "y" } }
 * Frontend:     { id: "...", type: "agent_spawned", agentId: "x", agentName: "y", ... }
 */
export function transformSSEEvent(raw: RawSSEEvent): WorldEvent {
  const { type, payload } = raw;

  // Extract common fields from payload
  const agentId = payload.agent_id as string | undefined;
  const agentName = payload.name as string | undefined;
  const tick = (payload.tick as number) ?? 0;
  const amount = payload.amount as number | undefined;

  // Extract target info where applicable
  const targetId =
    (payload.to as string | undefined) ??
    (payload.target_id as string | undefined);
  const targetName =
    (payload.target_name as string | undefined) ??
    (payload.other_agent_id as string | undefined);

  // Build description from type + payload
  const description = buildDescription(type, payload);

  // Generate a unique id (counter + timestamp)
  eventCounter++;
  const id = `evt-${Date.now()}-${eventCounter}`;

  return {
    id,
    type: type as WorldEvent["type"],
    agentId,
    agentName,
    targetId,
    targetName,
    description,
    amount,
    timestamp: new Date().toISOString(),
    tick,
    data: payload,
  };
}

function buildDescription(
  type: string,
  payload: Record<string, unknown>,
): string {
  const name =
    (payload.name as string | undefined) ??
    (payload.agent_id as string | undefined) ??
    "unknown";

  switch (type) {
    case "tick_advanced":
      return `Tick #${payload.tick} 已推进`;
    case "agent_spawned":
      return `${name} 诞生了`;
    case "agent_dying":
      return `${name} 即将死亡 (宽限 ${payload.grace_ticks ?? 0} ticks)`;
    case "agent_died":
      return `${name} 已死亡`;
    case "agent_rescued":
      return `${name} 被救活了`;
    case "transaction_completed":
      return `${payload.from} → ${payload.to}: $${payload.amount}`;
    case "balance_changed":
      return `${name} 余额变更: ${payload.old_balance} → ${payload.new_balance}`;
    case "phase_changed":
      return `${name} 阶段变更: ${payload.old_phase} → ${payload.new_phase}`;
    case "rule_violated":
      return `${name} 违反规则: ${payload.rule}`;
    case "snapshot_taken":
      return `快照已保存至 Tick #${payload.tick}`;
    case "task_created":
      return `新任务 ${payload.task_id} (奖励: $${payload.reward})`;
    case "task_claimed":
      return `${payload.assignee} 认领了任务 ${payload.task_id}`;
    case "task_completed":
      return `任务 ${payload.task_id} 已完成`;
    case "task_expired":
      return `任务 ${payload.task_id} 已过期`;
    case "reputation_changed":
      return `${name} 信誉变更: ${payload.old_reputation} → ${payload.new_reputation}`;
    case "skill_level_up":
      return `${name} 技能 ${payload.skill} 升至 ${payload.new_level} 级`;
    case "tax_collected":
      return `${payload.payer_id} 缴纳 ${payload.tax_kind} 税 $${payload.tax_amount}`;
    case "treasury_distributed":
      return `国库分配 $${payload.total_amount}`;
    case "leadership_election_started":
      return `选举开始，${(payload.candidates as string[] | undefined)?.length ?? 0} 位候选人`;
    case "leadership_changed":
      return `新领导人: ${payload.new_leader_id}`;
    case "treaty_proposed":
      return `条约提议: ${payload.org_a} ↔ ${payload.org_b}`;
    case "treaty_signed":
      return `条约签署: ${payload.org_a} ↔ ${payload.org_b}`;
    case "treaty_broken":
      return `${payload.breaker} 撕毁条约`;
    case "relation_changed":
      return `关系变更: ${payload.org_a} ↔ ${payload.org_b} (${payload.old_level} → ${payload.new_level})`;
    case "coordination_task_created":
      return `团队任务 ${payload.task_id} 创建 (最多 ${payload.max_agents} 人)`;
    case "coordination_task_agent_joined":
      return `${name} 加入团队任务 ${payload.task_id}`;
    case "coordination_task_agent_submitted":
      return `${name} 提交了团队任务 ${payload.task_id}`;
    case "coordination_task_completed":
      return `团队任务 ${payload.task_id} 完成 (${payload.contributor_count} 人参与)`;
    case "coordination_task_cancelled":
      return `团队任务 ${payload.task_id} 已取消`;
    case "coordination_task_expired":
      return `团队任务 ${payload.task_id} 已过期`;
    case "soft_rule_proposed":
      return `法案提议: ${payload.title}`;
    case "soft_rule_activated":
      return `法案 ${payload.rule_id} 生效`;
    case "soft_rule_expired":
      return `法案 ${payload.rule_id} 过期`;
    case "soft_rule_repealed":
      return `法案 ${payload.rule_id} 废除`;
    case "feed_post_created":
      return `${payload.author_name} 发布了动态`;
    case "investment_purchased":
      return `${payload.investor_id} 投资 $${payload.total_amount}`;
    default:
      return `${type}: ${Object.keys(payload).join(", ")}`;
  }
}
