"use client";

import { useState, useEffect, useRef } from "react";
import { useNotifications } from "@/components/useNotifications";
import type { NotificationType } from "@/types/world";

const TYPE_STYLES: Record<NotificationType, string> = {
  agent_death: "text-red-400 bg-red-500/10",
  leadership_changed: "text-amber-400 bg-amber-500/10",
  treaty_signed: "text-green-400 bg-green-500/10",
  treaty_broken: "text-red-400 bg-red-500/10",
  oracle_delivered: "text-blue-400 bg-blue-500/10",
  bounty_claimed: "text-purple-400 bg-purple-500/10",
};

const TYPE_ICONS: Record<NotificationType, string> = {
  agent_death: "💀",
  leadership_changed: "👑",
  treaty_signed: "📜",
  treaty_broken: "💥",
  oracle_delivered: "🔮",
  bounty_claimed: "🎯",
};

export function NotificationPanel() {
  const [open, setOpen] = useState(false);
  const panelRef = useRef<HTMLDivElement>(null);
  const { notifications, unreadCount, markRead, markAllRead, clearAll } =
    useNotifications();

  // Close on Escape
  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") setOpen(false);
    }
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, []);

  // Close on outside click
  useEffect(() => {
    if (!open) return;
    function onClick(e: MouseEvent) {
      if (panelRef.current && !panelRef.current.contains(e.target as Node)) {
        setOpen(false);
      }
    }
    document.addEventListener("mousedown", onClick);
    return () => document.removeEventListener("mousedown", onClick);
  }, [open]);

  return (
    <div className="relative" ref={panelRef}>
      <button
        onClick={() => setOpen(!open)}
        className="relative flex items-center justify-center rounded-lg p-2 text-zinc-400 hover:bg-zinc-800 hover:text-zinc-200 transition-colors"
        aria-label="通知"
      >
        <svg
          className="h-5 w-5"
          fill="none"
          stroke="currentColor"
          viewBox="0 0 24 24"
        >
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth={2}
            d="M15 17h5l-1.405-1.405A2.032 2.032 0 0118 14.158V11a6.002 6.002 0 00-4-5.659V5a2 2 0 10-4 0v.341C7.67 6.165 6 8.388 6 11v3.159c0 .538-.214 1.055-.595 1.436L4 17h5m6 0v1a3 3 0 11-6 0v-1m6 0H9"
          />
        </svg>
        {unreadCount > 0 && (
          <span className="absolute -right-0.5 -top-0.5 flex items-center justify-center rounded-full bg-red-500 px-1.5 text-[10px] font-bold text-white min-w-[18px]">
            {unreadCount > 9 ? "9+" : unreadCount}
          </span>
        )}
      </button>

      {open && (
        <div className="absolute right-0 top-full mt-2 z-50 w-80 rounded-xl border border-zinc-800 bg-zinc-900 shadow-xl">
          <div className="flex items-center justify-between border-b border-zinc-800 px-4 py-3">
            <h3 className="text-sm font-semibold text-zinc-200">通知</h3>
            <div className="flex items-center gap-2">
              {unreadCount > 0 && (
                <button
                  onClick={markAllRead}
                  className="text-[10px] text-blue-400 hover:text-blue-300"
                >
                  全部已读
                </button>
              )}
              {notifications.length > 0 && (
                <button
                  onClick={clearAll}
                  className="text-[10px] text-zinc-500 hover:text-zinc-300"
                >
                  清空
                </button>
              )}
            </div>
          </div>
          <div className="max-h-80 overflow-y-auto">
            {notifications.length === 0 ? (
              <div className="px-4 py-8 text-center text-xs text-zinc-600">
                暂无通知
              </div>
            ) : (
              notifications.map((n) => (
                <button
                  key={n.id}
                  onClick={() => markRead(n.id)}
                  className={`w-full text-left px-4 py-3 border-b border-zinc-800/50 last:border-0 hover:bg-zinc-800/30 transition-colors ${
                    !n.read ? "bg-zinc-800/20" : ""
                  }`}
                >
                  <div className="flex items-start gap-2">
                    <span className="text-sm shrink-0 mt-0.5">
                      {TYPE_ICONS[n.type]}
                    </span>
                    <div className="min-w-0 flex-1">
                      <div className="flex items-center gap-2">
                        <span className="text-xs font-medium text-zinc-200">
                          {n.title}
                        </span>
                        {!n.read && (
                          <span className="rounded-full bg-blue-500 h-1.5 w-1.5 shrink-0" />
                        )}
                      </div>
                      <p className="text-[11px] text-zinc-500 mt-0.5 line-clamp-2">
                        {n.description}
                      </p>
                      {n.agent_name && (
                        <span
                          className={`inline-flex items-center rounded-full px-1.5 py-0.5 text-[10px] font-medium mt-1 ${
                            TYPE_STYLES[n.type] ?? "text-zinc-400 bg-zinc-800"
                          }`}
                        >
                          {n.agent_name}
                        </span>
                      )}
                    </div>
                  </div>
                </button>
              ))
            )}
          </div>
        </div>
      )}
    </div>
  );
}
