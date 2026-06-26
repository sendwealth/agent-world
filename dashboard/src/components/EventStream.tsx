"use client";

import type { WorldEvent, EventType } from "@/types/world";

interface EventStreamProps {
  events: WorldEvent[];
  connected: boolean;
}

const eventTypeConfig: Record<
  EventType,
  { label: string; color: string; icon: string }
> = {
  tick_advanced: { label: "Tick 推进", color: "bg-zinc-500", icon: "⏱" },
  agent_spawned: { label: "诞生", color: "bg-green-500", icon: "👶" },
  agent_dying: { label: "濒死", color: "bg-orange-500", icon: "⚠" },
  agent_died: { label: "死亡", color: "bg-red-500", icon: "💀" },
  agent_rescued: { label: "营救", color: "bg-emerald-500", icon: "🆘" },
  transaction_completed: { label: "交易", color: "bg-blue-500", icon: "💰" },
  balance_changed: { label: "余额变更", color: "bg-yellow-500", icon: "💳" },
  phase_changed: { label: "阶段变更", color: "bg-blue-500", icon: "🔄" },
  rule_violated: { label: "违规", color: "bg-red-500", icon: "🚫" },
  snapshot_taken: { label: "快照", color: "bg-zinc-500", icon: "📸" },
  escrow_created: { label: "托管创建", color: "bg-cyan-500", icon: "🔒" },
  escrow_claimed: { label: "托管认领", color: "bg-blue-500", icon: "✋" },
  escrow_released: { label: "托管释放", color: "bg-green-500", icon: "🔓" },
  escrow_refunded: { label: "托管退款", color: "bg-amber-500", icon: "↩" },
  escrow_frozen: { label: "托管冻结", color: "bg-blue-500", icon: "❄" },
  task_created: { label: "新任务", color: "bg-purple-500", icon: "📋" },
  task_claimed: { label: "认领", color: "bg-indigo-500", icon: "✋" },
  task_started: { label: "开始", color: "bg-blue-500", icon: "▶" },
  task_submitted: { label: "提交", color: "bg-cyan-500", icon: "📤" },
  task_reviewed: { label: "审核", color: "bg-yellow-500", icon: "🔍" },
  task_completed: { label: "完成", color: "bg-emerald-500", icon: "✅" },
  task_expired: { label: "过期", color: "bg-zinc-500", icon: "⏰" },
  reward_distributed: { label: "奖励发放", color: "bg-amber-500", icon: "🎁" },
  reputation_changed: { label: "信誉更新", color: "bg-orange-500", icon: "⭐" },
  skill_level_up: { label: "技能提升", color: "bg-yellow-500", icon: "⬆" },
  skill_mutated: { label: "技能变异", color: "bg-purple-500", icon: "🧬" },
  tax_collected: { label: "税收征收", color: "bg-orange-500", icon: "🏛" },
  treasury_distributed: { label: "国库分配", color: "bg-emerald-500", icon: "💰" },
  leadership_election_started: { label: "选举开始", color: "bg-blue-500", icon: "🗳" },
  leadership_changed: { label: "领导更替", color: "bg-indigo-500", icon: "👑" },
  treaty_proposed: { label: "条约提议", color: "bg-cyan-500", icon: "📝" },
  treaty_signed: { label: "条约签署", color: "bg-green-500", icon: "🤝" },
  treaty_broken: { label: "条约撕毁", color: "bg-red-500", icon: "💔" },
  relation_changed: { label: "关系变化", color: "bg-purple-500", icon: "🔄" },
  coordination_task_created: { label: "团队任务创建", color: "bg-violet-500", icon: "🎯" },
  coordination_task_agent_joined: { label: "加入团队任务", color: "bg-blue-500", icon: "👥" },
  coordination_task_agent_submitted: { label: "提交团队任务", color: "bg-teal-500", icon: "📤" },
  coordination_task_completed: { label: "团队任务完成", color: "bg-emerald-500", icon: "✅" },
  coordination_task_cancelled: { label: "团队任务取消", color: "bg-red-500", icon: "🚫" },
  coordination_task_expired: { label: "团队任务过期", color: "bg-zinc-500", icon: "⏰" },
  soft_rule_proposed: { label: "法案提议", color: "bg-sky-500", icon: "📜" },
  soft_rule_activated: { label: "法案生效", color: "bg-green-500", icon: "✅" },
  soft_rule_expired: { label: "法案过期", color: "bg-zinc-500", icon: "⏰" },
  soft_rule_repealed: { label: "法案废除", color: "bg-red-500", icon: "❌" },
  investment_product_created: { label: "投资产品", color: "bg-teal-500", icon: "📊" },
  investment_purchased: { label: "投资", color: "bg-teal-500", icon: "🏦" },
  investment_sold: { label: "卖出", color: "bg-amber-500", icon: "💸" },
  investment_dividend: { label: "分红", color: "bg-green-500", icon: "💰" },
  feed_post_created: { label: "动态发布", color: "bg-blue-500", icon: "📝" },
  feed_post_liked: { label: "点赞", color: "bg-pink-500", icon: "❤" },
  feed_comment_created: { label: "评论", color: "bg-indigo-500", icon: "💬" },
  feed_comment_liked: { label: "评论点赞", color: "bg-pink-500", icon: "❤" },
};

function formatTime(ts: string): string {
  const d = new Date(ts);
  return d.toLocaleTimeString("zh-CN", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

export function EventStream({ events, connected }: EventStreamProps) {
  return (
    <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4">
      <div className="mb-3 flex items-center justify-between">
        <h2 id="event-stream-title" className="text-sm font-semibold text-zinc-200">实时事件流</h2>
        <div className="flex items-center gap-1.5">
          <span
            className={`inline-block h-2 w-2 rounded-full ${
              connected ? "bg-green-400 animate-pulse" : "bg-red-400"
            }`}
          />
          <span className="text-xs text-zinc-500">
            {connected ? "已连接" : "断开连接"}
          </span>
        </div>
      </div>

      <div
        id="event-log"
        className="max-h-[400px] space-y-1.5 overflow-y-auto pr-1 scrollbar-thin"
        role="log"
        aria-labelledby="event-stream-title"
        aria-live="polite"
      >
        {events.length === 0 ? (
          <div className="flex h-32 items-center justify-center text-sm text-zinc-600">
            等待事件...
          </div>
        ) : (
          events.map((event) => {
            const config = eventTypeConfig[event.type] ?? {
              label: event.type,
              color: "bg-zinc-500",
              icon: "•",
            };
            return (
              <div
                key={event.id}
                className="flex items-start gap-2.5 rounded-lg border border-zinc-800/50 bg-zinc-900/30 px-3 py-2 transition-colors hover:bg-zinc-800/40"
              >
                <span className="mt-0.5 text-sm leading-none">{config.icon}</span>
                <div className="min-w-0 flex-1">
                  <div className="flex flex-wrap items-center gap-2">
                    <span
                      className={`inline-block rounded px-1.5 py-0.5 text-[10px] font-medium text-white ${config.color}`}
                    >
                      {config.label}
                    </span>
                    <span className="text-xs text-zinc-500">
                      Tick #{event.tick}
                    </span>
                  </div>
                  <p className="mt-0.5 text-xs text-zinc-300 break-words">
                    {event.description}
                  </p>
                </div>
                <span className="hidden sm:shrink-0 sm:inline text-[10px] text-zinc-600">
                  {formatTime(event.timestamp)}
                </span>
              </div>
            );
          })
        )}
      </div>
    </div>
  );
}
