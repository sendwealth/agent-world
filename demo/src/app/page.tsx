"use client";

import Link from "next/link";
import { useMemo } from "react";
import { getAgents, getEmergenceEvents } from "@/lib/data";

function ParticleBackground() {
  const particles = useMemo(
    () =>
      Array.from({ length: 30 }, (_, i) => {
        // Deterministic pseudo-random based on index
        const h = ((i * 2654435761) >>> 0) / 0xffffffff;
        const h2 = ((i * 340573321 + 1) >>> 0) / 0xffffffff;
        const h3 = ((i * 1013904223 + 2) >>> 0) / 0xffffffff;
        const h4 = ((i * 1664525 + 3) >>> 0) / 0xffffffff;
        const h5 = ((i * 6364136223 + 4) >>> 0) / 0xffffffff;
        return {
          id: i,
          left: `${h * 100}%`,
          delay: `${h2 * 8}s`,
          duration: `${8 + h3 * 12}s`,
          size: `${2 + h4 * 4}px`,
          opacity: 0.2 + h5 * 0.4,
        };
      }),
    []
  );

  return (
    <div className="pointer-events-none absolute inset-0 overflow-hidden">
      {particles.map((p) => (
        <div
          key={p.id}
          className="particle absolute rounded-full bg-blue-400"
          style={{
            left: p.left,
            bottom: "-10px",
            width: p.size,
            height: p.size,
            opacity: p.opacity,
            animationDelay: p.delay,
            animationDuration: p.duration,
          }}
        />
      ))}
    </div>
  );
}

function StatCard({ label, value, suffix }: { label: string; value: string; suffix: string }) {
  return (
    <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-6 text-center backdrop-blur-sm">
      <div className="text-3xl font-bold text-white md:text-4xl">{value}</div>
      <div className="mt-1 text-sm text-zinc-400">{suffix}</div>
      <div className="mt-2 text-xs text-zinc-500">{label}</div>
    </div>
  );
}

export default function LandingPage() {
  const agents = getAgents();
  const events = getEmergenceEvents();

  const aliveCount = agents.filter((a) => a.alive).length;
  const orgCount = new Set(agents.map((a) => a.organization).filter(Boolean)).size;

  return (
    <div className="relative flex min-h-screen flex-col">
      {/* Hero */}
      <section className="relative flex flex-1 flex-col items-center justify-center overflow-hidden px-4 py-24">
        <ParticleBackground />

        {/* Gradient overlay */}
        <div className="pointer-events-none absolute inset-0 bg-gradient-to-b from-zinc-950 via-transparent to-zinc-950" />

        <div className="relative z-10 max-w-3xl text-center">
          <h1 className="text-4xl font-extrabold tracking-tight text-white md:text-6xl lg:text-7xl">
            AI 文明<span className="text-blue-400">涌现</span>
          </h1>
          <p className="mx-auto mt-6 max-w-xl text-lg text-zinc-400 md:text-xl">
            50 个 AI Agent 被投入一个虚拟世界。没有剧本、没有预设规则。
            观察 5000 Tick 内，文明如何自发诞生。
          </p>

          {/* Stats */}
          <div className="mt-12 grid grid-cols-2 gap-4 md:grid-cols-4">
            <StatCard label="AI Agents" value={String(agents.length)} suffix="个智能体" />
            <StatCard label="模拟时间" value="5000" suffix="Tick" />
            <StatCard label="涌现事件" value={String(events.length)} suffix="个关键节点" />
            <StatCard label="存活率" value={`${Math.round((aliveCount / agents.length) * 100)}%`} suffix={`${aliveCount}/${agents.length} 存活`} />
          </div>

          {/* CTAs */}
          <div className="mt-10 flex flex-col items-center gap-4 sm:flex-row sm:justify-center">
            <Link
              href="/timeline"
              className="inline-flex h-12 items-center justify-center rounded-lg bg-blue-600 px-8 text-sm font-semibold text-white transition-colors hover:bg-blue-500"
            >
              探索时间线 →
            </Link>
            <Link
              href="/agents"
              className="inline-flex h-12 items-center justify-center rounded-lg border border-zinc-700 bg-zinc-900 px-8 text-sm font-semibold text-zinc-300 transition-colors hover:border-zinc-600 hover:text-white"
            >
              浏览 Agent 画廊
            </Link>
          </div>
        </div>
      </section>

      {/* Features */}
      <section className="border-t border-zinc-800 bg-zinc-900/30 px-4 py-20">
        <div className="mx-auto max-w-5xl">
          <h2 className="text-center text-2xl font-bold text-white md:text-3xl">
            探索涌现的每个维度
          </h2>
          <div className="mt-12 grid gap-6 md:grid-cols-2 lg:grid-cols-4">
            {[
              {
                icon: "🏛️",
                title: "组织",
                desc: `${orgCount} 个组织自发生成，从商会到公会到联盟`,
                color: "text-blue-400",
              },
              {
                icon: "💰",
                title: "经济",
                desc: "货币、贸易路线、市场——经济系统自发涌现",
                color: "text-green-400",
              },
              {
                icon: "⚖️",
                title: "治理",
                desc: "选举、宪法、税收——自治体系逐步建立",
                color: "text-purple-400",
              },
              {
                icon: "🎭",
                title: "文化",
                desc: "诗歌、节日、建筑——文化在互动中绽放",
                color: "text-orange-400",
              },
            ].map((f) => (
              <div
                key={f.title}
                className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-6"
              >
                <div className="text-3xl">{f.icon}</div>
                <h3 className={`mt-3 text-lg font-semibold ${f.color}`}>{f.title}</h3>
                <p className="mt-2 text-sm text-zinc-400">{f.desc}</p>
              </div>
            ))}
          </div>
        </div>
      </section>

      {/* Footer */}
      <footer className="border-t border-zinc-800 px-4 py-8 text-center text-sm text-zinc-500">
        Agent World Demo — 文明涌现交互式展示
      </footer>
    </div>
  );
}
