"use client";

import type { ReactNode } from "react";

interface StatCardProps {
  title: string;
  value: string | number;
  icon: ReactNode;
  subtitle?: string;
  trend?: "up" | "down" | "neutral";
  color: "blue" | "green" | "amber" | "red";
}

const colorMap = {
  blue: {
    bg: "bg-blue-500/10",
    border: "border-blue-500/20",
    icon: "text-blue-400",
    glow: "shadow-blue-500/5",
  },
  green: {
    bg: "bg-green-500/10",
    border: "border-green-500/20",
    icon: "text-green-400",
    glow: "shadow-green-500/5",
  },
  amber: {
    bg: "bg-amber-500/10",
    border: "border-amber-500/20",
    icon: "text-amber-400",
    glow: "shadow-amber-500/5",
  },
  red: {
    bg: "bg-red-500/10",
    border: "border-red-500/20",
    icon: "text-red-400",
    glow: "shadow-red-500/5",
  },
};

export function StatCard({ title, value, icon, subtitle, color }: StatCardProps) {
  const c = colorMap[color];

  return (
    <div
      className={`relative overflow-hidden rounded-xl border ${c.border} ${c.bg} p-4 md:p-5 shadow-lg ${c.glow} transition-all hover:scale-[1.02]`}
    >
      <div className="flex items-start justify-between">
        <div className="space-y-1">
          <p className="text-sm font-medium text-zinc-400">{title}</p>
          <p className="text-2xl md:text-3xl font-bold tracking-tight text-zinc-100">
            {typeof value === "number" ? value.toLocaleString() : value}
          </p>
          {subtitle && (
            <p className="text-xs text-zinc-500">{subtitle}</p>
          )}
        </div>
        <div className={`rounded-lg ${c.bg} p-2.5 ${c.icon}`}>
          {icon}
        </div>
      </div>
    </div>
  );
}
