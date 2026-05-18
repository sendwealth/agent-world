"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";

const navItems = [
  { href: "/", label: "世界概览", icon: "🌍" },
  { href: "/agents", label: "Agent 列表", icon: "🤖" },
  { href: "/tasks", label: "任务板", icon: "📋" },
  { href: "/marketplace", label: "知识市场", icon: "🏪" },
  { href: "/timeline", label: "事件时间线", icon: "📜" },
];

export function Sidebar() {
  const pathname = usePathname();

  return (
    <aside className="flex h-full w-56 shrink-0 flex-col border-r border-zinc-800 bg-zinc-950">
      {/* Logo */}
      <div className="flex h-14 items-center gap-2 border-b border-zinc-800 px-4">
        <span className="text-lg">🌍</span>
        <span className="text-sm font-bold text-zinc-100">Agent World</span>
      </div>

      {/* Navigation */}
      <nav className="flex-1 space-y-0.5 p-2">
        {navItems.map((item) => {
          const active =
            item.href === "/agents"
              ? pathname === "/agents" || pathname.startsWith("/agents/")
              : pathname === item.href;
          return (
            <Link
              key={item.href}
              href={item.href}
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
      </nav>

      {/* Footer */}
      <div className="border-t border-zinc-800 p-3">
        <div className="rounded-lg bg-zinc-900/50 px-3 py-2">
          <p className="text-[10px] text-zinc-600">Agent World v1.0.0</p>
        </div>
      </div>
    </aside>
  );
}
