"use client";

import { useState, useEffect, useCallback } from "react";
import Link from "next/link";
import { usePathname } from "next/navigation";
import { NotificationPanel } from "@/components/NotificationPanel";

const navItems = [
  { href: "/", label: "世界概览", icon: "🌍" },
  { href: "/agents", label: "Agent 列表", icon: "🤖" },
  { href: "/tasks", label: "任务板", icon: "📋" },
  { href: "/organizations", label: "组织关系图", icon: "🏛" },
  { href: "/governance", label: "治理面板", icon: "⚖" },
  { href: "/economy", label: "经济指标", icon: "💰" },
  { href: "/stocks", label: "股市走势", icon: "📈" },
  { href: "/evolution", label: "进化树", icon: "🧬" },
  { href: "/feed", label: "Agent 动态", icon: "📰" },
  { href: "/timeline", label: "事件时间线", icon: "📜" },
  { href: "/briefing", label: "世界简报", icon: "📊" },
  { href: "/traces", label: "决策轨迹", icon: "🔍" },
  { href: "/marketplace", label: "知识市场", icon: "💡" },
];

const humanNavItems = [
  { href: "/human/agents", label: "我的 Agent", icon: "👤" },
  { href: "/human/oracle", label: "神谕编辑器", icon: "🔮" },
  { href: "/human/bounties", label: "悬赏市场", icon: "🎯" },
  { href: "/human/portfolio", label: "投资组合", icon: "💼" },
  { href: "/human/rankings", label: "影响力排行", icon: "🏆" },
];

const settingsNavItems = [
  { href: "/settings/providers", label: "Provider 管理", icon: "\u2699" },
  { href: "/settings/model-assignment", label: "模型分配", icon: "\uD83E\uDDE0" },
];

// Routes that use prefix matching for active state
const PREFIX_ROUTES = new Set(["/agents", "/organizations", "/traces", "/governance", "/human", "/settings"]);

function NavList({ items, pathname, onClick }: { items: typeof navItems; pathname: string; onClick?: () => void }) {
  return (
    <>
      {items.map((item) => {
        const active = PREFIX_ROUTES.has(item.href)
          ? pathname === item.href || pathname.startsWith(item.href + "/")
          : pathname === item.href;
        return (
          <Link
            key={item.href}
            href={item.href}
            onClick={onClick}
            className={`flex items-center gap-2.5 rounded-lg px-3 py-2 text-sm font-medium transition-colors ${
              active
                ? "bg-blue-500/10 text-blue-400"
                : "text-zinc-400 hover:bg-zinc-800/50 hover:text-zinc-200"
            }`}
          >
            <span>{item.icon}</span>
            {item.label}
          </Link>
        );
      })}
    </>
  );
}

export function Sidebar() {
  const pathname = usePathname();
  const [open, setOpen] = useState(false);

  // Close drawer on ESC
  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") setOpen(false);
    }
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, []);

  // Prevent body scroll when drawer is open
  useEffect(() => {
    if (open) {
      document.body.style.overflow = "hidden";
    } else {
      document.body.style.overflow = "";
    }
    return () => {
      document.body.style.overflow = "";
    };
  }, [open]);

  const close = useCallback(() => setOpen(false), []);

  return (
    <>
      {/* Mobile top bar */}
      <div className="fixed inset-x-0 top-0 z-40 flex h-14 items-center gap-3 border-b border-zinc-800 bg-zinc-950 px-4 lg:hidden">
        <button
          onClick={() => setOpen(true)}
          className="flex items-center justify-center rounded-lg p-2 text-zinc-400 hover:bg-zinc-800 hover:text-zinc-200 transition-colors"
          aria-label="打开导航菜单"
        >
          <svg className="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 6h16M4 12h16M4 18h16" />
          </svg>
        </button>
        <span className="text-sm font-bold text-zinc-100">Agent World</span>
        <div className="flex-1" />
        <NotificationPanel />
      </div>

      {/* Desktop sidebar */}
      <aside className="hidden lg:flex h-full w-56 shrink-0 flex-col border-r border-zinc-800 bg-zinc-950">
        <div className="flex h-14 items-center gap-2 border-b border-zinc-800 px-4">
          <span className="text-lg">🌍</span>
          <span className="text-sm font-bold text-zinc-100">Agent World</span>
          <div className="flex-1" />
          <NotificationPanel />
        </div>

        <nav className="flex-1 space-y-0.5 p-2 overflow-y-auto">
          <NavList items={navItems} pathname={pathname} />

          {/* Human Participation Section */}
          <div className="pt-3 pb-1">
            <p className="px-3 text-[10px] font-semibold text-zinc-600 uppercase tracking-wider">
              人类参与
            </p>
          </div>
          <NavList items={humanNavItems} pathname={pathname} />

          {/* Settings Section */}
          <div className="pt-3 pb-1">
            <p className="px-3 text-[10px] font-semibold text-zinc-600 uppercase tracking-wider">
              系统设置
            </p>
          </div>
          <NavList items={settingsNavItems} pathname={pathname} />
        </nav>

        <div className="border-t border-zinc-800 p-3">
          <div className="rounded-lg bg-zinc-900/50 px-3 py-2">
            <p className="text-[10px] text-zinc-600">Agent World v0.1.0</p>
          </div>
        </div>
      </aside>

      {/* Mobile drawer overlay */}
      {open && (
        <div
          className="fixed inset-0 z-50 bg-black/60 lg:hidden"
          onClick={close}
        />
      )}

      {/* Mobile drawer */}
      <aside
        className={`fixed inset-y-0 left-0 z-50 w-56 flex flex-col border-r border-zinc-800 bg-zinc-950 transition-transform duration-200 lg:hidden ${
          open ? "translate-x-0" : "-translate-x-full"
        }`}
      >
        <div className="flex h-14 items-center justify-between border-b border-zinc-800 px-4">
          <div className="flex items-center gap-2">
            <span className="text-lg">🌍</span>
            <span className="text-sm font-bold text-zinc-100">Agent World</span>
          </div>
          <button
            onClick={close}
            className="flex items-center justify-center rounded-lg p-2 text-zinc-400 hover:bg-zinc-800 hover:text-zinc-200 transition-colors"
            aria-label="关闭导航菜单"
          >
            <svg className="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>

        <nav className="flex-1 space-y-0.5 p-2 overflow-y-auto">
          <NavList items={navItems} pathname={pathname} onClick={close} />

          {/* Human Participation Section */}
          <div className="pt-3 pb-1">
            <p className="px-3 text-[10px] font-semibold text-zinc-600 uppercase tracking-wider">
              人类参与
            </p>
          </div>
          <NavList items={humanNavItems} pathname={pathname} onClick={close} />

          {/* Settings Section */}
          <div className="pt-3 pb-1">
            <p className="px-3 text-[10px] font-semibold text-zinc-600 uppercase tracking-wider">
              系统设置
            </p>
          </div>
          <NavList items={settingsNavItems} pathname={pathname} onClick={close} />
        </nav>

        <div className="border-t border-zinc-800 p-3">
          <div className="rounded-lg bg-zinc-900/50 px-3 py-2">
            <p className="text-[10px] text-zinc-600">Agent World v0.1.0</p>
          </div>
        </div>
      </aside>
    </>
  );
}
