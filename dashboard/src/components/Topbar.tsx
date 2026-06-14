"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { useSSEContext } from "@/components/SSEProvider";

const navItems = [
  { href: "/", label: "概览" },
  { href: "/dashboard", label: "仪表盘" },
  { href: "/agents", label: "Agent" },
  { href: "/marketplace", label: "市场" },
];

// Routes that use prefix matching for active state
const PREFIX_ROUTES = new Set(["/agents"]);

/** Find the most recent reliable tick from tick_advanced events */
function latestTick(events: { type: string; tick: number }[]): number | null {
  for (const e of events) {
    if (e.type === "tick_advanced" && e.tick > 0) return e.tick;
  }
  return null;
}

export function Topbar() {
  const pathname = usePathname();
  const { connected, events } = useSSEContext();

  const currentTick = latestTick(events);

  return (
    <header className="fixed inset-x-0 top-0 z-40 flex h-[52px] items-center border-b border-border bg-surface px-4">
      {/* Left: Logo + Nav */}
      <div className="flex items-center gap-6">
        <Link href="/" className="flex items-center gap-1.5 text-base font-bold tracking-tight text-fg">
          <span>Agent</span>
          <span className="text-accent">World</span>
        </Link>

        <nav className="flex items-center gap-1">
          {navItems.map((item) => {
            const active = PREFIX_ROUTES.has(item.href)
              ? pathname === item.href || pathname.startsWith(item.href + "/")
              : pathname === item.href;

            return (
              <Link
                key={item.href}
                href={item.href}
                className={`rounded-[var(--radius-sm)] px-3 py-1.5 text-sm font-medium transition-colors ${
                  active
                    ? "bg-accent/10 text-accent"
                    : "text-fg2 hover:bg-card hover:text-fg"
                }`}
              >
                {item.label}
              </Link>
            );
          })}
        </nav>
      </div>

      {/* Right: Live status */}
      <div className="ml-auto flex items-center gap-4">
        {currentTick != null && (
          <span className="text-xs font-medium text-muted">
            Tick <span className="tabular-nums text-fg2">{currentTick}</span>
          </span>
        )}
        <div className="flex items-center gap-1.5">
          <span
            className={`inline-block h-2 w-2 rounded-full ${
              connected ? "bg-success" : "bg-danger"
            }`}
          />
          <span className="text-xs text-muted">
            {connected ? "Live" : "Offline"}
          </span>
        </div>
      </div>
    </header>
  );
}
