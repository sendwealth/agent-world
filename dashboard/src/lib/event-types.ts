export interface EventTypeConfig {
  label: string;
  color: string;
  dot: string;
  bgClass: string;
  icon: string;
}

export const EVENT_TYPE_CONFIG: Record<string, EventTypeConfig> = {
  agent_spawn: { label: "诞生", color: "text-green-400", dot: "bg-green-400", bgClass: "bg-green-400", icon: "👶" },
  agent_death: { label: "死亡", color: "text-red-400", dot: "bg-red-400", bgClass: "bg-red-400", icon: "💀" },
  trade: { label: "交易", color: "text-amber-400", dot: "bg-amber-400", bgClass: "bg-amber-400", icon: "💰" },
  task_created: { label: "发布任务", color: "text-blue-400", dot: "bg-blue-400", bgClass: "bg-blue-400", icon: "📋" },
  task_claimed: { label: "认领任务", color: "text-cyan-400", dot: "bg-cyan-400", bgClass: "bg-cyan-400", icon: "✋" },
  task_completed: { label: "完成任务", color: "text-emerald-400", dot: "bg-emerald-400", bgClass: "bg-emerald-400", icon: "✅" },
  message: { label: "消息", color: "text-violet-400", dot: "bg-violet-400", bgClass: "bg-violet-400", icon: "💬" },
  skill_up: { label: "技能提升", color: "text-purple-400", dot: "bg-purple-400", bgClass: "bg-purple-400", icon: "⬆" },
  reputation_change: { label: "信誉变化", color: "text-yellow-400", dot: "bg-yellow-400", bgClass: "bg-yellow-400", icon: "⭐" },
  investment: { label: "投资", color: "text-teal-400", dot: "bg-teal-400", bgClass: "bg-teal-400", icon: "🏦" },
  tax: { label: "税收", color: "text-orange-400", dot: "bg-orange-400", bgClass: "bg-orange-400", icon: "🏛" },
  inflation: { label: "通胀", color: "text-rose-400", dot: "bg-rose-400", bgClass: "bg-rose-400", icon: "📈" },
  tax_collected: { label: "税收征收", color: "text-orange-400", dot: "bg-orange-400", bgClass: "bg-orange-400", icon: "🏛" },
  treasury_distributed: { label: "国库分配", color: "text-emerald-400", dot: "bg-emerald-400", bgClass: "bg-emerald-400", icon: "💰" },
  leadership_election_started: { label: "选举开始", color: "text-blue-400", dot: "bg-blue-400", bgClass: "bg-blue-400", icon: "🗳" },
  leadership_changed: { label: "领导更替", color: "text-indigo-400", dot: "bg-indigo-400", bgClass: "bg-indigo-400", icon: "👑" },
  treaty_proposed: { label: "条约提议", color: "text-cyan-400", dot: "bg-cyan-400", bgClass: "bg-cyan-400", icon: "📝" },
  treaty_signed: { label: "条约签署", color: "text-green-400", dot: "bg-green-400", bgClass: "bg-green-400", icon: "🤝" },
  treaty_broken: { label: "条约撕毁", color: "text-red-400", dot: "bg-red-400", bgClass: "bg-red-400", icon: "💔" },
  relation_changed: { label: "关系变化", color: "text-purple-400", dot: "bg-purple-400", bgClass: "bg-purple-400", icon: "🔄" },
};
