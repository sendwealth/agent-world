"use client";

import { useState, useEffect, useRef, useCallback } from "react";
import type { AgentNotification, NotificationType } from "@/types/world";
import { useSSEContext } from "@/components/SSEProvider";

const STORAGE_KEY = "agent-world-notifications";
const MAX_NOTIFICATIONS = 50;

function eventToNotification(event: {
  type: string;
  id?: string;
  agentId?: string;
  agentName?: string;
  description?: string;
  tick?: number;
  timestamp?: string;
}): AgentNotification | null {
  const typeMap: Record<string, NotificationType> = {
    agent_death: "agent_death",
    leadership_changed: "leadership_changed",
    treaty_signed: "treaty_signed",
    treaty_broken: "treaty_broken",
  };

  const notifType = typeMap[event.type];
  if (!notifType) return null;

  const titleMap: Record<NotificationType, string> = {
    agent_death: "Agent 死亡",
    leadership_changed: "领导更替",
    treaty_signed: "条约签署",
    treaty_broken: "条约撕毁",
    oracle_delivered: "神谕已送达",
    bounty_claimed: "悬赏已被认领",
  };

  return {
    id: event.id || `${Date.now()}-${Math.random()}`,
    type: notifType,
    title: titleMap[notifType],
    description: event.description || `${event.agentName || "未知 Agent"} 触发了 ${notifType}`,
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
      const notification = eventToNotification(event);
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
