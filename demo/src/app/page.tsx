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
import { useEffect, useRef } from "react";
import { loadAgents, loadEvents, loadOrganizations } from "@/lib/data";
import { useState } from "react";

function StatBlock({ value, label }: { value: string; label: string }) {
  return (
    <div className="text-center">
      <div className="text-3xl md:text-4xl font-bold tabular-nums bg-gradient-to-r from-blue-400 to-purple-400 bg-clip-text text-transparent">
        {value}
      </div>
      <div className="text-sm text-zinc-400 mt-1">{label}</div>
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
function ParticleField() {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    let animId: number;
    const particles: { x: number; y: number; vx: number; vy: number; r: number; color: string }[] = [];

    function resize() {
      if (!canvas) return;
      canvas.width = canvas.offsetWidth;
      canvas.height = canvas.offsetHeight;
    }
    resize();
    window.addEventListener("resize", resize);

    const colors = ["#3b82f6", "#a855f7", "#22c55e", "#f59e0b"];
    for (let i = 0; i < 60; i++) {
      particles.push({
        x: Math.random() * canvas.width,
        y: Math.random() * canvas.height,
        vx: (Math.random() - 0.5) * 0.5,
        vy: (Math.random() - 0.5) * 0.5,
        r: Math.random() * 2 + 1,
        color: colors[Math.floor(Math.random() * colors.length)],
      });
    }

    function draw() {
      if (!ctx || !canvas) return;
      ctx.clearRect(0, 0, canvas.width, canvas.height);

      for (const p of particles) {
        p.x += p.vx;
        p.y += p.vy;
        if (p.x < 0 || p.x > canvas.width) p.vx *= -1;
        if (p.y < 0 || p.y > canvas.height) p.vy *= -1;

        ctx.beginPath();
        ctx.arc(p.x, p.y, p.r, 0, Math.PI * 2);
        ctx.fillStyle = p.color;
        ctx.globalAlpha = 0.6;
        ctx.fill();
      }

      // Draw connections
      ctx.globalAlpha = 0.08;
      ctx.strokeStyle = "#3b82f6";
      ctx.lineWidth = 0.5;
      for (let i = 0; i < particles.length; i++) {
        for (let j = i + 1; j < particles.length; j++) {
          const dx = particles[i].x - particles[j].x;
          const dy = particles[i].y - particles[j].y;
          const dist = Math.sqrt(dx * dx + dy * dy);
          if (dist < 120) {
            ctx.beginPath();
            ctx.moveTo(particles[i].x, particles[i].y);
            ctx.lineTo(particles[j].x, particles[j].y);
            ctx.stroke();
          }
        }
      }
      ctx.globalAlpha = 1;
      animId = requestAnimationFrame(draw);
    }

    draw();
    return () => {
      cancelAnimationFrame(animId);
      window.removeEventListener("resize", resize);
    };
  }, []);

  return (
    <canvas
      ref={canvasRef}
      className="absolute inset-0 w-full h-full"
      aria-hidden="true"
    />
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
  const [stats, setStats] = useState({ agents: 50, ticks: 5000, events: 22, orgs: 7 });

  useEffect(() => {
    Promise.all([loadAgents(), loadEvents(), loadOrganizations()]).then(
      ([agents, events, orgs]) => {
        setStats({
          agents: agents.length,
          ticks: 5000,
          events: events.length,
          orgs: orgs.length,
        });
      }
    );
  }, []);

  return (
    <div className="relative">
      {/* Hero section */}
      <section className="relative min-h-[90vh] flex items-center justify-center overflow-hidden">
        <ParticleField />
        <div className="absolute inset-0 bg-gradient-to-b from-zinc-950/40 via-zinc-950/80 to-zinc-950" />
        <div className="relative z-10 text-center px-4 max-w-4xl mx-auto">
          <h1 className="text-4xl md:text-6xl lg:text-7xl font-bold tracking-tight mb-6">
            <span className="bg-gradient-to-r from-blue-400 via-purple-400 to-amber-400 bg-clip-text text-transparent">
              Agent World
            </span>
          </h1>
          <p className="text-lg md:text-xl text-zinc-300 max-w-2xl mx-auto mb-4">
            Watch 50 AI agents build a civilization from scratch.
          </p>
          <p className="text-sm text-zinc-500 max-w-xl mx-auto mb-10">
            No script, no plan — just agents with personalities, skills, and the freedom
            to trade, organize, govern, and create culture.
          </p>

          {/* Stats */}
          <div className="flex justify-center gap-6 md:gap-12 mb-10">
            <StatBlock value={`${stats.agents}`} label="AI Agents" />
            <StatBlock value={`${stats.ticks.toLocaleString()}`} label="World Ticks" />
            <StatBlock value={`${stats.events}`} label="Emergence Events" />
            <StatBlock value={`${stats.orgs}`} label="Organizations" />
          </div>

          {/* CTAs */}
          <div className="flex flex-col sm:flex-row gap-3 justify-center">
            <Link
              href="/timeline"
              className="px-6 py-3 rounded-xl bg-blue-500/10 text-blue-400 border border-blue-500/20 hover:bg-blue-500/20 transition-colors text-sm font-medium"
            >
              Explore Timeline &rarr;
            </Link>
            <Link
              href="/dashboard"
              className="px-6 py-3 rounded-xl bg-zinc-800/50 text-zinc-300 border border-zinc-700 hover:bg-zinc-800 transition-colors text-sm font-medium"
            >
              View Dashboard
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
      {/* Features section */}
      <section className="py-20 px-4">
        <div className="max-w-6xl mx-auto">
          <h2 className="text-2xl md:text-3xl font-bold text-center mb-12">
            Three Phases of Emergence
          </h2>
          <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
            {[
              {
                phase: "Exploration",
                tick: "0 – 1,200",
                desc: "Agents spawn into an empty world. They explore, learn skills, form first impressions, and discover each other.",
                emoji: "🧭",
                borderColor: "border-blue-500/30",
              },
              {
                phase: "Organization",
                tick: "1,200 – 3,200",
                desc: "Agents form guilds, companies, and alliances. Trade routes emerge. Economies develop. Power structures crystallize.",
                emoji: "🏛️",
                borderColor: "border-green-500/30",
              },
              {
                phase: "Governance",
                tick: "3,200 – 5,000",
                desc: "Organizations propose rules, hold votes, ratify constitutions. Culture diverges. Civilization takes root.",
                emoji: "⚖️",
                borderColor: "border-purple-500/30",
              },
            ].map((item) => (
              <div
                key={item.phase}
                className={`rounded-xl border border-zinc-800 ${item.borderColor} bg-zinc-900/50 p-6 transition-colors`}
              >
                <div className="text-3xl mb-3">{item.emoji}</div>
                <div className="text-xs text-zinc-500 mb-1">Tick {item.tick}</div>
                <h3 className="text-lg font-semibold mb-2 text-zinc-100">{item.phase}</h3>
                <p className="text-sm text-zinc-400">{item.desc}</p>
              </div>
            ))}
          </div>
        </div>
      </section>

      {/* Footer */}
      <footer className="border-t border-zinc-800 px-4 py-8 text-center text-sm text-zinc-500">
        Agent World Demo — 文明涌现交互式展示
      <footer className="border-t border-zinc-800 py-8 text-center text-xs text-zinc-600">
        Agent World &mdash; A multi-agent civilization simulation demo
      </footer>
    </div>
  );
}
