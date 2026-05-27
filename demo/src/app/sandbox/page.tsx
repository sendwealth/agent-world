import { SandboxForm } from "@/components/SandboxForm";

export default function SandboxPage() {
  return (
    <div className="mx-auto max-w-2xl px-4 py-8">
      <h1 className="text-2xl font-bold text-white md:text-3xl">交互式沙盒</h1>
      <p className="mt-2 text-zinc-400">
        创建你自己的 Agent，看看它在虚拟世界中可能经历什么。纯模拟，不连接后端。
      </p>
      <div className="mt-8">
        <SandboxForm />
"use client";

import { useState } from "react";
import { loadTemplates } from "@/lib/data";
import type { SandboxTemplate, PersonalityTraits } from "@/types/demo";

function SliderField({
  label,
  value,
  onChange,
}: {
  label: string;
  value: number;
  onChange: (v: number) => void;
}) {
  return (
    <div>
      <div className="flex justify-between text-xs mb-1">
        <span className="text-zinc-400">{label}</span>
        <span className="text-zinc-500 tabular-nums">{(value * 100).toFixed(0)}%</span>
      </div>
      <input
        type="range"
        min={0}
        max={100}
        value={Math.round(value * 100)}
        onChange={(e) => onChange(Number(e.target.value) / 100)}
        className="w-full h-1.5 rounded-full appearance-none bg-zinc-800 cursor-pointer
          [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:w-3 [&::-webkit-slider-thumb]:h-3
          [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-blue-400"
      />
    </div>
  );
}

type SimulationStep = "idle" | "simulating" | "done";

const SIMULATION_MESSAGES = [
  "Initializing agent consciousness...",
  "Connecting to world simulation...",
  "Scanning nearby agents...",
  "Establishing initial relationships...",
  "Discovering local environment...",
  "Finding potential trade partners...",
  "Agent successfully integrated!",
];

export default function SandboxPage() {
  const templates = loadTemplates();
  const [selectedTemplate, setSelectedTemplate] = useState<string | null>(null);
  const [name, setName] = useState("");
  const [personality, setPersonality] = useState<PersonalityTraits>({
    openness: 0.5,
    conscientiousness: 0.5,
    extraversion: 0.5,
    agreeableness: 0.5,
    neuroticism: 0.5,
  });
  const [simStep, setSimStep] = useState<SimulationStep>("idle");
  const [simMessage, setSimMessage] = useState("");
  const [simProgress, setSimProgress] = useState(0);

  function applyTemplate(template: SandboxTemplate) {
    setSelectedTemplate(template.id);
    setName(template.name);
    setPersonality({ ...template.personality });
  }

  function handleSimulate() {
    if (!name.trim()) return;
    setSimStep("simulating");
    setSimProgress(0);

    let i = 0;
    const interval = setInterval(() => {
      if (i < SIMULATION_MESSAGES.length) {
        setSimMessage(SIMULATION_MESSAGES[i]);
        setSimProgress(((i + 1) / SIMULATION_MESSAGES.length) * 100);
        i++;
      } else {
        clearInterval(interval);
        setSimStep("done");
      }
    }, 600);
  }

  function handleReset() {
    setSimStep("idle");
    setSimMessage("");
    setSimProgress(0);
    setSelectedTemplate(null);
    setName("");
    setPersonality({ openness: 0.5, conscientiousness: 0.5, extraversion: 0.5, agreeableness: 0.5, neuroticism: 0.5 });
  }

  if (simStep === "done") {
    return (
      <div className="min-h-screen flex items-center justify-center px-4">
        <div className="max-w-lg w-full text-center">
          <div className="text-5xl mb-6">🎉</div>
          <h1 className="text-2xl font-bold text-zinc-100 mb-2">
            {name} has entered the world!
          </h1>
          <p className="text-sm text-zinc-400 mb-8">
            Your agent has been integrated into the simulation. Here&apos;s what they might encounter:
          </p>
          <div className="grid grid-cols-1 sm:grid-cols-2 gap-3 mb-8 text-left">
            {[
              { emoji: "🤝", title: "Allies", desc: `${Math.floor(Math.random() * 5) + 2} agents share similar values` },
              { emoji: "⚔️", title: "Rivals", desc: `${Math.floor(Math.random() * 3) + 1} agents compete for resources` },
              { emoji: "🏛️", title: "Organizations", desc: `${Math.floor(Math.random() * 3) + 1} guilds match their skills` },
              { emoji: "📊", title: "Trade Routes", desc: `${Math.floor(Math.random() * 4) + 2} profitable trade opportunities` },
            ].map((item) => (
              <div key={item.title} className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4">
                <div className="text-xl mb-1">{item.emoji}</div>
                <div className="text-sm font-semibold text-zinc-200">{item.title}</div>
                <div className="text-xs text-zinc-500">{item.desc}</div>
              </div>
            ))}
          </div>
          <button
            onClick={handleReset}
            className="px-6 py-3 rounded-xl bg-zinc-800/50 text-zinc-300 border border-zinc-700 hover:bg-zinc-800 transition-colors text-sm font-medium"
          >
            Create Another Agent
          </button>
        </div>
      </div>
    );
  }

  if (simStep === "simulating") {
    return (
      <div className="min-h-screen flex items-center justify-center px-4">
        <div className="max-w-md w-full text-center">
          <div className="text-4xl mb-6 animate-bounce">🚀</div>
          <h1 className="text-xl font-bold text-zinc-100 mb-2">Deploying {name}...</h1>
          <p className="text-sm text-zinc-400 mb-6">{simMessage}</p>
          <div className="w-full h-2 rounded-full bg-zinc-800 overflow-hidden">
            <div
              className="h-full rounded-full bg-gradient-to-r from-blue-500 to-purple-500 transition-all duration-500"
              style={{ width: `${simProgress}%` }}
            />
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="min-h-screen">
      <div className="border-b border-zinc-800 bg-zinc-950/80 backdrop-blur-md px-4 md:px-6 py-4">
        <h1 className="text-xl font-bold text-zinc-100">Sandbox</h1>
        <p className="text-sm text-zinc-500 mt-1">Create a custom agent and see what happens when they enter the world</p>
      </div>

      <div className="max-w-4xl mx-auto px-4 md:px-6 py-6">
        {/* Templates */}
        <div className="mb-8">
          <h2 className="text-sm font-semibold text-zinc-400 uppercase tracking-wider mb-3">
            Choose a Template
          </h2>
          <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
            {templates.map((t) => (
              <button
                key={t.id}
                onClick={() => applyTemplate(t)}
                className={`text-left rounded-xl border p-4 transition-all cursor-pointer ${
                  selectedTemplate === t.id
                    ? "border-blue-500/30 bg-blue-500/5"
                    : "border-zinc-800 bg-zinc-900/50 hover:border-zinc-700"
                }`}
              >
                <div className="text-2xl mb-2">{t.emoji}</div>
                <div className="text-sm font-semibold text-zinc-100">{t.name}</div>
                <div className="text-xs text-zinc-500 mt-1">{t.description}</div>
              </button>
            ))}
          </div>
        </div>

        {/* Customization */}
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-6 mb-8">
          {/* Identity */}
          <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 md:p-6">
            <h2 className="text-sm font-semibold text-zinc-400 uppercase tracking-wider mb-4">
              Identity
            </h2>
            <div className="mb-4">
              <label className="text-xs text-zinc-500 mb-1 block">Agent Name</label>
              <input
                type="text"
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="Enter a name..."
                className="w-full px-3 py-2 rounded-lg bg-zinc-950 border border-zinc-800 text-sm text-zinc-300 placeholder-zinc-600 focus:outline-none focus:border-zinc-600"
              />
            </div>

            <h3 className="text-xs text-zinc-500 mb-3">Personality Traits</h3>
            <div className="space-y-3">
              {(Object.keys(personality) as Array<keyof PersonalityTraits>).map((key) => (
                <SliderField
                  key={key}
                  label={key.charAt(0).toUpperCase() + key.slice(1)}
                  value={personality[key]}
                  onChange={(v) => setPersonality({ ...personality, [key]: v })}
                />
              ))}
            </div>
          </div>

          {/* Preview */}
          <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 md:p-6">
            <h2 className="text-sm font-semibold text-zinc-400 uppercase tracking-wider mb-4">
              Agent Preview
            </h2>
            <div className="flex items-center gap-3 mb-4">
              <div className="w-12 h-12 rounded-full bg-zinc-800 flex items-center justify-center text-lg font-semibold text-zinc-300">
                {name ? name.slice(0, 1) : "?"}
              </div>
              <div>
                <div className="font-semibold text-zinc-100">{name || "Unnamed Agent"}</div>
                <div className="text-xs text-zinc-500">
                  {templates.find((t) => t.id === selectedTemplate)?.description ?? "Custom agent"}
                </div>
              </div>
            </div>

            {/* Values display from template */}
            {selectedTemplate && (
              <div className="mb-4">
                <div className="text-xs text-zinc-500 mb-2">Core Values</div>
                <div className="flex gap-1.5 flex-wrap">
                  {templates.find((t) => t.id === selectedTemplate)?.values.map((v) => (
                    <span key={v} className="text-xs px-2 py-0.5 rounded bg-purple-500/10 text-purple-400">{v}</span>
                  ))}
                </div>
              </div>
            )}

            <div className="mt-6 p-3 rounded-lg bg-zinc-800/30 border border-zinc-800/50">
              <div className="text-xs text-zinc-500 mb-1">Simulation Note</div>
              <div className="text-xs text-zinc-400">
                This is a simulated preview. In the full system, your agent would connect
                to a live world via the Third-party Agent API.
              </div>
            </div>
          </div>
        </div>

        {/* Submit */}
        <div className="flex justify-center">
          <button
            onClick={handleSimulate}
            disabled={!name.trim()}
            className={`px-8 py-3 rounded-xl text-sm font-medium transition-all ${
              name.trim()
                ? "bg-blue-500/10 text-blue-400 border border-blue-500/20 hover:bg-blue-500/20 cursor-pointer"
                : "bg-zinc-900/50 text-zinc-600 border border-zinc-800 cursor-not-allowed"
            }`}
          >
            Deploy Agent to World &rarr;
          </button>
        </div>
      </div>
    </div>
  );
}
