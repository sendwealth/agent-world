"use client";

import { useState, useEffect, useCallback, useRef } from "react";
import Link from "next/link";
import { usePathname } from "next/navigation";

const navItems = [
  { href: "/", label: "世界概览", icon: "🌍" },
  { href: "/agents", label: "Agent 列表", icon: "🤖" },
  { href: "/tasks", label: "任务板", icon: "📋" },
  { href: "/timeline", label: "事件时间线", icon: "📜" },
];

function NavLinks({ pathname, onNavigate }: { pathname: string; onNavigate?: () => void }) {
  return (
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
            onClick={onNavigate}
            className={`flex min-h-[44px] items-center gap-2.5 rounded-lg px-3 py-2 text-sm font-medium transition-colors ${
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
  );
}

export function Sidebar() {
  const pathname = usePathname();
  const [open, setOpen] = useState(false);
  const openRef = useRef(false);

  // Keep ref in sync — accessing a ref in setState callback is allowed
  useEffect(() => {
    openRef.current = open;
  }, [open]);

  // Close drawer on route change (handles browser back/forward via popstate)
  useEffect(() => {
    if (openRef.current) {
      setOpen(false);
    }
  }, [pathname]);

  // Close on ESC
  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    },
    []
  );

  useEffect(() => {
    if (open) {
      document.addEventListener("keydown", handleKeyDown);
      document.body.style.overflow = "hidden";
    }
    return () => {
      document.removeEventListener("keydown", handleKeyDown);
      document.body.style.overflow = "";
    };
  }, [open, handleKeyDown]);

  const close = useCallback(() => setOpen(false), []);

  return (
    <>
      {/* Mobile top bar */}
      <div className="fixed inset-x-0 top-0 z-40 flex h-14 items-center border-b border-zinc-800 bg-zinc-950 px-4 lg:hidden">
        <button
          onClick={() => setOpen(true)}
          className="flex min-h-[44px] min-w-[44px] items-center justify-center rounded-lg text-zinc-400 hover:bg-zinc-800 hover:text-zinc-200 transition-colors"
          aria-label="打开导航菜单"
        >
          <svg className="h-6 w-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 6h16M4 12h16M4 18h16" />
          </svg>
        </button>
        <span className="ml-3 text-sm font-bold text-zinc-100">Agent World</span>
      </div>

      {/* Drawer overlay */}
      {open && (
        <div
          className="fixed inset-0 z-50 bg-black/60 lg:hidden"
          onClick={close}
        />
      )}

      {/* Drawer sidebar (mobile) */}
      <aside
        className={`fixed inset-y-0 left-0 z-50 flex w-56 flex-col border-r border-zinc-800 bg-zinc-950 transition-transform duration-200 ease-out lg:hidden ${
          open ? "translate-x-0" : "-translate-x-full"
        }`}
      >
        {/* Logo */}
        <div className="flex h-14 items-center justify-between border-b border-zinc-800 px-4">
          <div className="flex items-center gap-2">
            <span className="text-lg">🌍</span>
            <span className="text-sm font-bold text-zinc-100">Agent World</span>
          </div>
          <button
            onClick={close}
            className="flex min-h-[44px] min-w-[44px] items-center justify-center rounded-lg text-zinc-400 hover:bg-zinc-800 hover:text-zinc-200 transition-colors"
            aria-label="关闭导航菜单"
          >
            <svg className="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>
        <NavLinks pathname={pathname} onNavigate={close} />
        <div className="border-t border-zinc-800 p-3">
          <div className="rounded-lg bg-zinc-900/50 px-3 py-2">
            <p className="text-[10px] text-zinc-600">Agent World v0.1.0</p>
          </div>
        </div>
      </aside>

      {/* Desktop static sidebar */}
      <aside className="hidden lg:flex h-full w-56 shrink-0 flex-col border-r border-zinc-800 bg-zinc-950">
        <div className="flex h-14 items-center gap-2 border-b border-zinc-800 px-4">
          <span className="text-lg">🌍</span>
          <span className="text-sm font-bold text-zinc-100">Agent World</span>
        </div>
        <NavLinks pathname={pathname} />
        <div className="border-t border-zinc-800 p-3">
          <div className="rounded-lg bg-zinc-900/50 px-3 py-2">
            <p className="text-[10px] text-zinc-600">Agent World v0.1.0</p>
          </div>
        </div>
      </aside>
    </>
  );
}
