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
  agent_spawn: { label: "诞生", color: "bg-green-500", icon: "👶" },
  agent_death: { label: "死亡", color: "bg-red-500", icon: "💀" },
  trade: { label: "交易", color: "bg-blue-500", icon: "💰" },
  task_created: { label: "新任务", color: "bg-purple-500", icon: "📋" },
  task_claimed: { label: "认领", color: "bg-indigo-500", icon: "✋" },
  task_completed: { label: "完成", color: "bg-emerald-500", icon: "✅" },
  message: { label: "消息", color: "bg-sky-500", icon: "💬" },
  skill_up: { label: "技能提升", color: "bg-yellow-500", icon: "⬆" },
  reputation_change: { label: "信誉变化", color: "bg-orange-500", icon: "⭐" },
  reputation_changed: { label: "信誉更新", color: "bg-orange-500", icon: "⭐" },
  inflation: { label: "通胀", color: "bg-amber-500", icon: "📈" },
  investment: { label: "投资", color: "bg-teal-500", icon: "🏦" },
  tax: { label: "税收", color: "bg-rose-500", icon: "🏛" },
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
        <h2 className="text-sm font-semibold text-zinc-200">实时事件流</h2>
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

      <div className="max-h-[400px] space-y-1.5 overflow-y-auto pr-1 scrollbar-thin">
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
