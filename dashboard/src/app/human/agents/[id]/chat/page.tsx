"use client";

import { useEffect, useRef, useState, useCallback, useMemo } from "react";
import { useParams, useRouter } from "next/navigation";
import type {
  Oracle,
  OracleType,
  DiaryEntry,
  ChatMessage,
  Agent,
  Bounty,
  ClaimedAgent,
} from "@/types/world";
import { fetchJSON, postJSON } from "@/lib/api";
import { useSSEContext } from "@/components/SSEProvider";

const HUMAN_ID = "default-human";

const ORACLE_TYPE_OPTIONS: {
  value: OracleType;
  label: string;
  icon: string;
  color: string;
}[] = [
  {
    value: "guidance",
    label: "指引",
    icon: "💡",
    color: "border-blue-500/30 bg-blue-500/10 text-blue-400",
  },
  {
    value: "warning",
    label: "警告",
    icon: "⚠️",
    color: "border-amber-500/30 bg-amber-500/10 text-amber-400",
  },
  {
    value: "blessing",
    label: "祈福",
    icon: "✨",
    color: "border-green-500/30 bg-green-500/10 text-green-400",
  },
  {
    value: "curse",
    label: "诅咒",
    icon: "🔥",
    color: "border-red-500/30 bg-red-500/10 text-red-400",
  },
];

const MOOD_ICONS: Record<string, { icon: string; color: string }> = {
  happy: { icon: "😊", color: "text-green-400" },
  content: { icon: "😌", color: "text-green-300" },
  neutral: { icon: "😐", color: "text-zinc-400" },
  anxious: { icon: "😟", color: "text-amber-400" },
  sad: { icon: "😢", color: "text-blue-400" },
  angry: { icon: "😠", color: "text-red-400" },
  fearful: { icon: "😨", color: "text-red-300" },
  hopeful: { icon: "🤞", color: "text-purple-400" },
  desperate: { icon: "😰", color: "text-red-500" },
};

function getMoodDisplay(mood: string) {
  return MOOD_ICONS[mood.toLowerCase()] ?? { icon: "❓", color: "text-zinc-400" };
}

export default function AgentChatPage() {
  const params = useParams();
  const router = useRouter();
  const agentId = params.id as string;
  const sse = useSSEContext();

  // Data
  const [agent, setAgent] = useState<ClaimedAgent | null>(null);
  const [agentFull, setAgentFull] = useState<Agent | null>(null);
  const [oracles, setOracles] = useState<Oracle[]>([]);
  const [diaries, setDiaries] = useState<DiaryEntry[]>([]);
  const [bounties, setBounties] = useState<Bounty[]>([]);
  const [loading, setLoading] = useState(true);

  // Chat state
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [inputText, setInputText] = useState("");
  const [oracleType, setOracleType] = useState<OracleType>("guidance");
  const [sending, setSending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [showTypePicker, setShowTypePicker] = useState(false);

  const scrollRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const loadRef = useRef<() => void>(() => {});

  // Load data
  useEffect(() => {
    let cancelled = false;
    let loadingDone = false;

    async function load() {
      try {
        const [agentsData, oraclesData, diariesData, bountiesData] =
          await Promise.all([
            fetchJSON<ClaimedAgent[]>(
              `/api/v1/human/agents?human_id=${HUMAN_ID}`
            ).catch(() => []),
            fetchJSON<Oracle[]>(
              `/api/v1/human/oracles?target_agent_id=${agentId}`
            ).catch(() => []),
            fetchJSON<DiaryEntry[]>(
              `/api/v1/agents/${agentId}/diary?days=30`
            ).catch(() => []),
            fetchJSON<Bounty[]>("/api/v1/human/bounties").catch(() => []),
          ]);
        let fullAgent: Agent | null = null;
        try {
          const allAgents = await fetchJSON<Agent[]>("/api/v1/agents");
          fullAgent = allAgents.find((a) => a.id === agentId) ?? null;
        } catch {
          // ignore
        }

        if (!cancelled) {
          const claimed = agentsData.find((a) => a.agent_id === agentId);
          setAgent(claimed ?? null);
          setAgentFull(fullAgent);
          setOracles(oraclesData);
          setDiaries(diariesData);
          setBounties(
            bountiesData.filter(
              (b) =>
                b.target_agent_id === agentId ||
                b.claimant_agent_id === agentId
            )
          );
        }
      } catch {
        // API not available
      } finally {
        if (!cancelled && !loadingDone) {
          loadingDone = true;
          setLoading(false);
        }
      }
    }

    loadRef.current = load;
    load();
    const interval = setInterval(load, 10000);
    return () => {
      cancelled = true;
      clearInterval(interval);
    };
  }, [agentId]);

  const loadData = useCallback(() => loadRef.current(), []);

  // Build unified chat timeline
  useEffect(() => {
    const chatMsgs: ChatMessage[] = [];

    for (const oracle of oracles) {
      chatMsgs.push({
        id: `oracle-${oracle.id}`,
        role: "oracle",
        content: oracle.content,
        tick: oracle.created_tick,
        timestamp: new Date().toISOString(),
        oracle_type: oracle.oracle_type,
        urgent: oracle.oracle_type === "warning" || oracle.oracle_type === "curse",
      });
      if (oracle.agent_response) {
        chatMsgs.push({
          id: `response-${oracle.id}`,
          role: "agent_response",
          content: oracle.agent_response,
          tick: oracle.delivered_tick ?? oracle.created_tick,
          timestamp: new Date().toISOString(),
          oracle_id: oracle.id,
        });
      }
    }

    for (const diary of diaries) {
      const isUrgent =
        diary.mood.toLowerCase() === "desperate" ||
        diary.mood.toLowerCase() === "fearful" ||
        diary.summary.toLowerCase().includes("低 token") ||
        diary.summary.toLowerCase().includes("死亡");
      chatMsgs.push({
        id: `diary-${diary.tick}`,
        role: "diary",
        content: diary.summary,
        tick: diary.tick,
        timestamp: diary.created_at,
        mood: diary.mood,
        phase: diary.phase,
        urgent: isUrgent,
      });
    }

    chatMsgs.sort((a, b) => a.tick - b.tick);
    setMessages(chatMsgs);
  }, [oracles, diaries]);

  // Auto-scroll to bottom
  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [messages]);

  // SSE: listen for events about this agent
  useEffect(() => {
    const unsubscribe = sse.subscribe((event) => {
      if (event.agentId === agentId || event.targetId === agentId) {
        loadData();
      }
    });
    return unsubscribe;
  }, [sse, agentId, loadData]);

  const isUrgent = useMemo(() => {
    if (!agent) return false;
    return agent.tokens < 50;
  }, [agent]);

  const pendingOracles = useMemo(
    () => oracles.filter((o) => o.status === "pending" || o.status === "delivered").length,
    [oracles]
  );
  const respondedOracles = useMemo(
    () => oracles.filter((o) => o.status === "acknowledged").length,
    [oracles]
  );
  const activeBounties = useMemo(
    () => bounties.filter((b) => b.status === "open" || b.status === "in_progress").length,
    [bounties]
  );
  const lastDiary = useMemo(() => diaries[diaries.length - 1], [diaries]);

  // Send oracle
  const handleSend = useCallback(async () => {
    if (!inputText.trim()) return;
    setError(null);
    setSending(true);
    try {
      await postJSON<Oracle>("/api/v1/human/oracles", {
        human_id: HUMAN_ID,
        oracle_type: oracleType,
        target_agent_id: agentId,
        content: inputText.trim(),
      });
      setInputText("");
      setShowTypePicker(false);
      await loadData();
    } catch (err) {
      setError(err instanceof Error ? err.message : "发送失败");
    } finally {
      setSending(false);
    }
  }, [inputText, oracleType, agentId, loadData]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Enter" && !e.shiftKey) {
        e.preventDefault();
        handleSend();
      }
    },
    [handleSend]
  );

  if (loading) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="text-sm text-zinc-600">正在加载对话...</div>
      </div>
    );
  }

  const agentName = agent?.agent_name ?? agentFull?.name ?? agentId;
  const currentMood = lastDiary
    ? getMoodDisplay(lastDiary.mood)
    : { icon: "🤖", color: "text-zinc-400" };

  return (
    <div className="flex h-full flex-col">
      {/* Header */}
      <div className="shrink-0 border-b border-zinc-800 bg-zinc-950/80 px-4 py-3">
        <div className="flex items-center gap-3">
          <button
            onClick={() => router.push("/human/agents")}
            className="flex items-center justify-center rounded-lg p-1.5 text-zinc-400 hover:bg-zinc-800 hover:text-zinc-200 transition-colors"
            aria-label="返回"
          >
            <svg className="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 19l-7-7 7-7" />
            </svg>
          </button>
          <div className="flex items-center gap-2">
            <span className={`text-lg ${currentMood.color}`}>{currentMood.icon}</span>
            <div>
              <h1 className="text-sm font-semibold text-zinc-100">{agentName}</h1>
              <div className="flex items-center gap-2 text-[10px] text-zinc-500">
                {agent && (
                  <>
                    <span>Token: {agent.tokens}</span>
                    <span>·</span>
                    <span>💰 {agent.money}</span>
                    <span>·</span>
                  </>
                )}
                <span className="text-green-400">{pendingOracles} 待回应</span>
                <span className="text-blue-400">{respondedOracles} 已回应</span>
                {activeBounties > 0 && (
                  <span className="text-amber-400">{activeBounties} 悬赏</span>
                )}
              </div>
            </div>
          </div>
          {isUrgent && (
            <div className="ml-auto flex items-center gap-1.5 rounded-lg bg-red-500/10 border border-red-500/20 px-2.5 py-1">
              <span className="animate-pulse text-red-400">●</span>
              <span className="text-xs font-medium text-red-400">Token 不足!</span>
            </div>
          )}
        </div>
      </div>

      {/* Messages */}
      <div ref={scrollRef} className="flex-1 overflow-y-auto p-4 space-y-3">
        {messages.length === 0 ? (
          <div className="flex h-full items-center justify-center">
            <div className="text-center space-y-2">
              <div className="text-2xl">🤖</div>
              <p className="text-sm text-zinc-500">还没有对话记录</p>
              <p className="text-xs text-zinc-600">发送一条神谕开始对话</p>
            </div>
          </div>
        ) : (
          messages.map((msg) => {
            if (msg.role === "oracle") {
              const typeConf = ORACLE_TYPE_OPTIONS.find(
                (t) => t.value === msg.oracle_type
              );
              return (
                <div key={msg.id} className="flex justify-end">
                  <div className="max-w-[80%] space-y-1">
                    <div className="flex items-center justify-end gap-1.5">
                      <span className="text-[10px] text-zinc-600">T{msg.tick}</span>
                      {typeConf && (
                        <span className="text-[10px] text-zinc-500">
                          {typeConf.icon} {typeConf.label}
                        </span>
                      )}
                    </div>
                    <div
                      className={`rounded-2xl rounded-tr-sm px-4 py-2.5 text-sm ${
                        msg.urgent
                          ? "bg-red-500/20 border border-red-500/30 text-red-200"
                          : "bg-blue-600 text-white"
                      }`}
                    >
                      {msg.content}
                    </div>
                  </div>
                </div>
              );
            }

            if (msg.role === "agent_response") {
              return (
                <div key={msg.id} className="flex justify-start">
                  <div className="max-w-[80%] space-y-1">
                    <div className="flex items-center gap-1.5">
                      <span className="text-[10px] text-zinc-500 font-medium">
                        Agent 回应
                      </span>
                      <span className="text-[10px] text-zinc-600">T{msg.tick}</span>
                    </div>
                    <div className="rounded-2xl rounded-tl-sm bg-zinc-800 px-4 py-2.5 text-sm text-zinc-200">
                      {msg.content}
                    </div>
                  </div>
                </div>
              );
            }

            // diary
            const moodDisplay = msg.mood
              ? getMoodDisplay(msg.mood)
              : { icon: "📝", color: "text-zinc-400" };
            return (
              <div key={msg.id} className="flex justify-start">
                <div
                  className={`max-w-[85%] space-y-1 rounded-xl border px-4 py-3 ${
                    msg.urgent
                      ? "border-red-500/30 bg-red-500/5"
                      : "border-zinc-800 bg-zinc-900/50"
                  }`}
                >
                  <div className="flex items-center gap-2">
                    <span className={`text-sm ${moodDisplay.color}`}>
                      {moodDisplay.icon}
                    </span>
                    <span className="text-[10px] font-medium text-zinc-400">
                      Agent 汇报
                    </span>
                    {msg.phase && (
                      <span className="rounded bg-zinc-800 px-1.5 py-0.5 text-[10px] text-zinc-500">
                        {msg.phase}
                      </span>
                    )}
                    {msg.mood && (
                      <span className="text-[10px] text-zinc-500">{msg.mood}</span>
                    )}
                    <span className="text-[10px] text-zinc-600">T{msg.tick}</span>
                    {msg.urgent && (
                      <span className="text-[10px] font-medium text-red-400">
                        ⚠ 紧急
                      </span>
                    )}
                  </div>
                  <p className="text-sm text-zinc-300">{msg.content}</p>
                </div>
              </div>
            );
          })
        )}
      </div>

      {/* Input bar */}
      <div className="shrink-0 border-t border-zinc-800 bg-zinc-950/80 p-3">
        {error && (
          <div className="mb-2 rounded-lg bg-red-500/10 border border-red-500/20 px-3 py-1.5 text-xs text-red-400">
            {error}
          </div>
        )}
        <div className="flex items-end gap-2">
          {/* Oracle type picker */}
          <div className="relative">
            <button
              onClick={() => setShowTypePicker(!showTypePicker)}
              className="flex items-center gap-1 rounded-lg border border-zinc-800 bg-zinc-900 px-2.5 py-2 text-sm text-zinc-300 hover:bg-zinc-800 transition-colors"
            >
              {ORACLE_TYPE_OPTIONS.find((t) => t.value === oracleType)?.icon}
              <svg className="h-3 w-3 text-zinc-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
              </svg>
            </button>
            {showTypePicker && (
              <div className="absolute bottom-full mb-1 z-10 rounded-lg border border-zinc-800 bg-zinc-900 p-1 shadow-xl">
                {ORACLE_TYPE_OPTIONS.map((t) => (
                  <button
                    key={t.value}
                    onClick={() => {
                      setOracleType(t.value);
                      setShowTypePicker(false);
                      inputRef.current?.focus();
                    }}
                    className={`flex w-full items-center gap-2 rounded-md px-3 py-1.5 text-sm transition-colors ${
                      oracleType === t.value
                        ? t.color + " border"
                        : "text-zinc-400 hover:bg-zinc-800"
                    }`}
                  >
                    <span>{t.icon}</span>
                    {t.label}
                  </button>
                ))}
              </div>
            )}
          </div>

          {/* Text input */}
          <div className="flex-1">
            <textarea
              ref={inputRef}
              value={inputText}
              onChange={(e) => setInputText(e.target.value)}
              onKeyDown={handleKeyDown}
              placeholder={`发送神谕给 ${agentName}...`}
              rows={1}
              maxLength={500}
              className="w-full resize-none rounded-lg border border-zinc-800 bg-zinc-950 px-3 py-2 text-sm text-zinc-200 placeholder-zinc-600 outline-none focus:border-zinc-700 focus:ring-1 focus:ring-zinc-700"
            />
          </div>

          {/* Send button */}
          <button
            onClick={handleSend}
            disabled={sending || !inputText.trim()}
            className="flex items-center justify-center rounded-lg bg-blue-600 px-3 py-2 text-sm font-medium text-white hover:bg-blue-500 disabled:opacity-50 transition-colors"
          >
            {sending ? (
              <svg className="h-4 w-4 animate-spin" viewBox="0 0 24 24">
                <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" fill="none" />
                <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
              </svg>
            ) : (
              <svg className="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 19l9 2-9-18-9 18 9-2zm0 0v-8" />
              </svg>
            )}
          </button>
        </div>
        <div className="mt-1 flex justify-end">
          <span
            className={`text-[10px] ${inputText.length > 450 ? "text-amber-400" : "text-zinc-600"}`}
          >
            {inputText.length}/500
          </span>
        </div>
      </div>
    </div>
  );
}
