"use client";

import { useState, useEffect, useRef, useCallback } from "react";
import type { AgentNotification, NotificationType } from "@/types/world";
import { useSSEContext } from "@/components/SSEProvider";

const STORAGE_KEY = "agent-world-notifications";
const MAX_NOTIFICATIONS = 50;

const TYPE_MAP: Record<string, NotificationType> = {
  agent_died: "agent_death",
  leadership_changed: "leadership_changed",
  treaty_signed: "treaty_signed",
  treaty_broken: "treaty_broken",
  oracle_delivered: "oracle_delivered",
  bounty_claimed: "bounty_claimed",
  low_token_warning: "low_token_warning",
  agent_help_request: "agent_help_request",
  agent_diary: "agent_diary",
};

const TITLE_MAP: Record<NotificationType, string> = {
  agent_death: "Agent 死亡",
  leadership_changed: "领导更替",
  treaty_signed: "条约签署",
  treaty_broken: "条约撕毁",
  oracle_delivered: "神谕已送达",
  bounty_claimed: "悬赏已被认领",
  low_token_warning: "⚡ Token 不足警告",
  agent_help_request: "🆘 Agent 求助",
  agent_diary: "📝 Agent 日记",
};

function eventToNotification(event: {
  type: string;
  id?: string;
  agentId?: string;
  agentName?: string;
  description?: string;
  tick?: number;
  timestamp?: string;
  data?: Record<string, unknown>;
}): AgentNotification | null {
  // Check standard type map first
  let notifType = TYPE_MAP[event.type];

  // Auto-detect low token warning from event data
  if (!notifType) {
    if (event.data) {
      const data = event.data;
      if (
        data.token_count !== undefined &&
        typeof data.token_count === "number" &&
        data.token_count < 50
      ) {
        notifType = "low_token_warning";
      }
      if (
        data.requesting_help === true ||
        data.help_type !== undefined
      ) {
        notifType = "agent_help_request";
      }
    }
  }

  if (!notifType) return null;

  // Build description for auto-detected types
  let description = event.description || `${event.agentName || "未知 Agent"} 触发了 ${notifType}`;
  if (notifType === "low_token_warning" && event.data) {
    const tokens = event.data.token_count ?? "?";
    description = `${event.agentName ?? "Agent"} Token 不足: ${tokens} — 即将面临死亡风险!`;
  }
  if (notifType === "agent_help_request" && event.data) {
    const helpType = event.data.help_type ?? "通用";
    description = `${event.agentName ?? "Agent"} 请求帮助: ${helpType}`;
  }

  return {
    id: event.id || `${Date.now()}-${Math.random()}`,
    type: notifType,
    title: TITLE_MAP[notifType],
    description,
    tick: event.tick || 0,
    timestamp: event.timestamp ? new Date(event.timestamp).getTime() : Date.now(),
    read: false,
    agent_id: event.agentId,
    agent_name: event.agentName,
  };
}

export function useNotifications() {
  const seenIds = useRef<Set<string>>(new Set());

  const [notifications, setNotifications] = useState<AgentNotification[]>(() => {
    try {
      const stored = localStorage.getItem(STORAGE_KEY);
      if (stored) {
        return JSON.parse(stored) as AgentNotification[];
      }
    } catch {
      // Ignore parse errors
    }
    return [];
  });

  const sse = useSSEContext();

  // Populate seenIds from initial notifications
  useEffect(() => {
    notifications.forEach((n) => seenIds.current.add(n.id));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Save to localStorage on change
  useEffect(() => {
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(notifications));
    } catch {
      // Ignore storage errors
    }
  }, [notifications]);

  // Subscribe to SSE events
  useEffect(() => {
    const unsubscribe = sse.subscribe((event) => {
      const notification = eventToNotification(event as Parameters<typeof eventToNotification>[0]);
      if (!notification) return;
      if (seenIds.current.has(notification.id)) return;

      seenIds.current.add(notification.id);
      setNotifications((prev) =>
        [notification, ...prev].slice(0, MAX_NOTIFICATIONS)
      );
    });
    return unsubscribe;
  }, [sse]);

  const markRead = useCallback((id: string) => {
    setNotifications((prev) =>
      prev.map((n) => (n.id === id ? { ...n, read: true } : n))
    );
  }, []);

  const markAllRead = useCallback(() => {
    setNotifications((prev) => prev.map((n) => ({ ...n, read: true })));
  }, []);

  const clearAll = useCallback(() => {
    setNotifications([]);
    seenIds.current.clear();
  }, []);

  const unreadCount = notifications.filter((n) => !n.read).length;

  return { notifications, unreadCount, markRead, markAllRead, clearAll };
}
