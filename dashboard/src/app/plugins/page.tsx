"use client";

import { useEffect, useRef, useState, useMemo, useCallback } from "react";
import type {
  PluginInfo,
  PluginListResponse,
  PluginState,
  SandboxListResponse,
} from "@/types/world";
import { fetchJSON, postJSON } from "@/lib/api";

// ── Constants ─────────────────────────────────────────────

const STATE_CONFIG: Record<PluginState, { label: string; color: string }> = {
  active: { label: "运行中", color: "bg-green-500/10 text-green-400 border-green-500/20" },
  registered: { label: "已注册", color: "bg-blue-500/10 text-blue-400 border-blue-500/20" },
  disabled: { label: "已禁用", color: "bg-yellow-500/10 text-yellow-400 border-yellow-500/20" },
  error: { label: "错误", color: "bg-red-500/10 text-red-400 border-red-500/20" },
  unloaded: { label: "已卸载", color: "bg-zinc-500/10 text-zinc-400 border-zinc-500/20" },
};

const PERMISSION_OPTIONS = [
  "read_agents",
  "read_world_state",
  "read_events",
  "write_agent_tokens",
  "write_agent_phase",
  "write_agent_skills",
  "emit_events",
  "intercept_actions",
  "intercept_transactions",
  "tick_subsystem",
  "admin_access",
];

// ── Register Plugin Dialog ────────────────────────────────

interface RegisterForm {
  id: string;
  name: string;
  version: string;
  description: string;
  author: string;
  priority: string;
  permissions: string[];
}

function RegisterPluginDialog({
  open,
  onClose,
  onRegistered,
}: {
  open: boolean;
  onClose: () => void;
  onRegistered: () => void;
}) {
  const [form, setForm] = useState<RegisterForm>({
    id: "",
    name: "",
    version: "0.1.0",
    description: "",
    author: "",
    priority: "100",
    permissions: [],
  });
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") onClose();
    }
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [open, onClose]);

  const togglePermission = useCallback((perm: string) => {
    setForm((f) => ({
      ...f,
      permissions: f.permissions.includes(perm)
        ? f.permissions.filter((p) => p !== perm)
        : [...f.permissions, perm],
    }));
  }, []);

  const handleSubmit = useCallback(
    async (e: React.FormEvent) => {
      e.preventDefault();
      setError(null);

      if (!form.id.trim()) {
        setError("请输入插件 ID");
        return;
      }
      if (!form.name.trim()) {
        setError("请输入插件名称");
        return;
      }
      if (!form.author.trim()) {
        setError("请输入作者");
        return;
      }

      setSubmitting(true);
      try {
        await postJSON("/api/v1/plugins/register", {
          id: form.id.trim(),
          name: form.name.trim(),
          version: form.version.trim() || "0.1.0",
          description: form.description.trim(),
          author: form.author.trim(),
          priority: Number(form.priority) || 100,
          permissions: form.permissions,
        });
        setForm({
          id: "",
          name: "",
          version: "0.1.0",
          description: "",
          author: "",
          priority: "100",
          permissions: [],
        });
        onRegistered();
        onClose();
      } catch (err) {
        setError(err instanceof Error ? err.message : "注册失败");
      } finally {
        setSubmitting(false);
      }
    },
    [form, onRegistered, onClose],
  );

  if (!open) return null;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60"
      onClick={onClose}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-label="注册插件"
        className="w-full max-w-lg rounded-xl border border-zinc-800 bg-zinc-900 p-6 shadow-xl max-h-[85vh] overflow-y-auto"
        onClick={(e) => e.stopPropagation()}
      >
        <h2 className="text-lg font-bold text-zinc-100 mb-4">注册插件</h2>

        <form onSubmit={handleSubmit} className="space-y-4">
          <div>
            <label className="block text-xs font-medium text-zinc-400 mb-1">
              插件 ID <span className="text-red-400">*</span>
            </label>
            <input
              type="text"
              value={form.id}
              onChange={(e) => setForm((f) => ({ ...f, id: e.target.value }))}
              placeholder="例如: author/my-plugin"
              className="w-full rounded-lg border border-zinc-800 bg-zinc-950 px-3 py-2 text-sm text-zinc-200 placeholder-zinc-600 outline-none focus:border-zinc-700 focus:ring-1 focus:ring-zinc-700"
            />
          </div>

          <div>
            <label className="block text-xs font-medium text-zinc-400 mb-1">
              名称 <span className="text-red-400">*</span>
            </label>
            <input
              type="text"
              value={form.name}
              onChange={(e) => setForm((f) => ({ ...f, name: e.target.value }))}
              placeholder="插件显示名称"
              className="w-full rounded-lg border border-zinc-800 bg-zinc-950 px-3 py-2 text-sm text-zinc-200 placeholder-zinc-600 outline-none focus:border-zinc-700 focus:ring-1 focus:ring-zinc-700"
            />
          </div>

          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="block text-xs font-medium text-zinc-400 mb-1">
                版本
              </label>
              <input
                type="text"
                value={form.version}
                onChange={(e) => setForm((f) => ({ ...f, version: e.target.value }))}
                placeholder="0.1.0"
                className="w-full rounded-lg border border-zinc-800 bg-zinc-950 px-3 py-2 text-sm text-zinc-200 placeholder-zinc-600 outline-none focus:border-zinc-700 focus:ring-1 focus:ring-zinc-700"
              />
            </div>
            <div>
              <label className="block text-xs font-medium text-zinc-400 mb-1">
                优先级
              </label>
              <input
                type="number"
                min={0}
                value={form.priority}
                onChange={(e) => setForm((f) => ({ ...f, priority: e.target.value }))}
                className="w-full rounded-lg border border-zinc-800 bg-zinc-950 px-3 py-2 text-sm text-zinc-200 placeholder-zinc-600 outline-none focus:border-zinc-700 focus:ring-1 focus:ring-zinc-700"
              />
            </div>
          </div>

          <div>
            <label className="block text-xs font-medium text-zinc-400 mb-1">
              描述
            </label>
            <textarea
              value={form.description}
              onChange={(e) => setForm((f) => ({ ...f, description: e.target.value }))}
              placeholder="插件功能描述"
              rows={3}
              className="w-full rounded-lg border border-zinc-800 bg-zinc-950 px-3 py-2 text-sm text-zinc-200 placeholder-zinc-600 outline-none focus:border-zinc-700 focus:ring-1 focus:ring-zinc-700 resize-none"
            />
          </div>

          <div>
            <label className="block text-xs font-medium text-zinc-400 mb-1">
              作者 <span className="text-red-400">*</span>
            </label>
            <input
              type="text"
              value={form.author}
              onChange={(e) => setForm((f) => ({ ...f, author: e.target.value }))}
              placeholder="作者名称或组织"
              className="w-full rounded-lg border border-zinc-800 bg-zinc-950 px-3 py-2 text-sm text-zinc-200 placeholder-zinc-600 outline-none focus:border-zinc-700 focus:ring-1 focus:ring-zinc-700"
            />
          </div>

          <div>
            <label className="block text-xs font-medium text-zinc-400 mb-2">
              权限
            </label>
            <div className="flex flex-wrap gap-1.5">
              {PERMISSION_OPTIONS.map((perm) => (
                <button
                  key={perm}
                  type="button"
                  onClick={() => togglePermission(perm)}
                  className={`rounded-full px-2.5 py-1 text-[11px] font-medium border transition-colors ${
                    form.permissions.includes(perm)
                      ? "bg-blue-500/15 text-blue-400 border-blue-500/30"
                      : "bg-zinc-800/50 text-zinc-500 border-zinc-800 hover:text-zinc-300 hover:border-zinc-700"
                  }`}
                >
                  {perm}
                </button>
              ))}
            </div>
          </div>

          {error && (
            <div className="rounded-lg bg-red-500/10 border border-red-500/20 px-3 py-1.5 text-xs text-red-400">
              {error}
            </div>
          )}

          <div className="flex items-center justify-end gap-3 pt-2">
            <button
              type="button"
              onClick={onClose}
              className="rounded-lg px-4 py-2 text-sm text-zinc-400 hover:text-zinc-200 transition-colors"
            >
              取消
            </button>
            <button
              type="submit"
              disabled={submitting}
              className="rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-500 disabled:opacity-50 transition-colors"
            >
              {submitting ? "注册中..." : "注册插件"}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}

// ── Plugin Detail Dialog ──────────────────────────────────

function PluginDetailDialog({
  plugin,
  onClose,
  onAction,
}: {
  plugin: PluginInfo;
  onClose: () => void;
  onAction: () => void;
}) {
  const [actionLoading, setActionLoading] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);

  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") onClose();
    }
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [onClose]);

  const stateCfg = STATE_CONFIG[plugin.state];

  const handleEnable = useCallback(async () => {
    setActionLoading(true);
    setActionError(null);
    try {
      await postJSON(`/api/v1/plugins/${encodeURIComponent(plugin.id)}/enable`, {});
      onAction();
    } catch (err) {
      setActionError(err instanceof Error ? err.message : "启用失败");
    } finally {
      setActionLoading(false);
    }
  }, [plugin.id, onAction]);

  const handleDisable = useCallback(async () => {
    setActionLoading(true);
    setActionError(null);
    try {
      await postJSON(`/api/v1/plugins/${encodeURIComponent(plugin.id)}/disable`, {});
      onAction();
    } catch (err) {
      setActionError(err instanceof Error ? err.message : "禁用失败");
    } finally {
      setActionLoading(false);
    }
  }, [plugin.id, onAction]);

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60"
      onClick={onClose}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-label={plugin.name}
        className="w-full max-w-lg rounded-xl border border-zinc-800 bg-zinc-900 p-6 shadow-xl"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-start justify-between mb-4">
          <h2 className="text-lg font-bold text-zinc-100 pr-4">{plugin.name}</h2>
          <button
            onClick={onClose}
            className="text-zinc-500 hover:text-zinc-300 transition-colors"
          >
            <svg className="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>

        <div className="space-y-3 mb-5">
          <div className="flex items-center gap-2">
            <span className="text-xs text-zinc-500 w-16 shrink-0">ID</span>
            <span className="text-sm text-zinc-300 font-mono">{plugin.id}</span>
          </div>
          <div className="flex items-center gap-2">
            <span className="text-xs text-zinc-500 w-16 shrink-0">状态</span>
            <span className={`inline-flex items-center rounded-full px-2 py-0.5 text-[11px] font-medium border ${stateCfg.color}`}>
              {stateCfg.label}
            </span>
          </div>
          <div className="flex items-center gap-2">
            <span className="text-xs text-zinc-500 w-16 shrink-0">版本</span>
            <span className="text-sm text-zinc-300">v{plugin.version}</span>
          </div>
          <div className="flex items-center gap-2">
            <span className="text-xs text-zinc-500 w-16 shrink-0">作者</span>
            <span className="text-sm text-zinc-300">{plugin.author}</span>
          </div>
          <div className="flex items-center gap-2">
            <span className="text-xs text-zinc-500 w-16 shrink-0">优先级</span>
            <span className="text-sm text-zinc-200">{plugin.priority}</span>
          </div>
          {plugin.description && (
            <div className="flex gap-2">
              <span className="text-xs text-zinc-500 w-16 shrink-0">描述</span>
              <p className="text-sm text-zinc-400">{plugin.description}</p>
            </div>
          )}
          {plugin.permissions.length > 0 && (
            <div className="flex items-start gap-2">
              <span className="text-xs text-zinc-500 w-16 shrink-0 pt-0.5">权限</span>
              <div className="flex flex-wrap gap-1">
                {plugin.permissions.map((perm) => (
                  <span key={perm} className="rounded bg-cyan-500/10 text-cyan-400 px-1.5 py-0.5 text-[10px] border border-cyan-500/20">
                    {perm}
                  </span>
                ))}
              </div>
            </div>
          )}
          {plugin.hooks.length > 0 && (
            <div className="flex items-start gap-2">
              <span className="text-xs text-zinc-500 w-16 shrink-0 pt-0.5">钩子</span>
              <div className="flex flex-wrap gap-1">
                {plugin.hooks.map((hook) => (
                  <span key={hook} className="rounded bg-purple-500/10 text-purple-400 px-1.5 py-0.5 text-[10px] border border-purple-500/20">
                    {hook}
                  </span>
                ))}
              </div>
            </div>
          )}
        </div>

        {/* Enable / Disable Actions */}
        <div className="flex items-center gap-3 border-t border-zinc-800 pt-4">
          {(plugin.state === "disabled" || plugin.state === "registered") && (
            <button
              onClick={handleEnable}
              disabled={actionLoading}
              className="rounded-lg bg-green-600 px-4 py-2 text-sm font-medium text-white hover:bg-green-500 disabled:opacity-50 transition-colors"
            >
              {actionLoading ? "处理中..." : "启用插件"}
            </button>
          )}
          {plugin.state === "active" && (
            <button
              onClick={handleDisable}
              disabled={actionLoading}
              className="rounded-lg bg-yellow-600 px-4 py-2 text-sm font-medium text-white hover:bg-yellow-500 disabled:opacity-50 transition-colors"
            >
              {actionLoading ? "处理中..." : "禁用插件"}
            </button>
          )}
        </div>

        {actionError && (
          <div className="mt-3 rounded-lg bg-red-500/10 border border-red-500/20 px-3 py-1.5 text-xs text-red-400">
            {actionError}
          </div>
        )}
      </div>
    </div>
  );
}

// ── Plugin Card ───────────────────────────────────────────

function PluginCard({
  plugin,
  onClick,
}: {
  plugin: PluginInfo;
  onClick: () => void;
}) {
  const stateCfg = STATE_CONFIG[plugin.state];

  return (
    <button
      onClick={onClick}
      className="w-full text-left rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 hover:bg-zinc-800/30 transition-colors space-y-3"
    >
      <div className="flex items-start justify-between gap-2">
        <h3 className="text-sm font-medium text-zinc-200 truncate">{plugin.name}</h3>
        <span className={`inline-flex items-center rounded-full px-2 py-0.5 text-[10px] font-medium border shrink-0 ${stateCfg.color}`}>
          {stateCfg.label}
        </span>
      </div>

      {plugin.description && (
        <p className="text-xs text-zinc-500 line-clamp-2">{plugin.description}</p>
      )}

      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <span className="text-xs text-zinc-400 font-mono">{plugin.author}</span>
          <span className="text-[10px] text-zinc-600">v{plugin.version}</span>
        </div>
        {plugin.permissions.length > 0 && (
          <span className="text-[10px] text-zinc-600">{plugin.permissions.length} 权限</span>
        )}
      </div>

      {(plugin.permissions.length > 0 || plugin.hooks.length > 0) && (
        <div className="flex flex-wrap gap-1">
          {plugin.hooks.slice(0, 3).map((hook) => (
            <span key={hook} className="rounded bg-purple-500/10 text-purple-400 px-1.5 py-0.5 text-[10px]">
              {hook}
            </span>
          ))}
          {plugin.hooks.length > 3 && (
            <span className="text-[10px] text-zinc-600">+{plugin.hooks.length - 3} hooks</span>
          )}
          {plugin.permissions.slice(0, 2).map((perm) => (
            <span key={perm} className="rounded bg-cyan-500/10 text-cyan-400 px-1.5 py-0.5 text-[10px]">
              {perm}
            </span>
          ))}
          {plugin.permissions.length > 2 && (
            <span className="text-[10px] text-zinc-600">+{plugin.permissions.length - 2} perms</span>
          )}
        </div>
      )}

      <div className="flex items-center justify-between">
        <span className="text-[10px] text-zinc-600 font-mono">{plugin.id}</span>
        <span className="text-[10px] text-zinc-600">P{plugin.priority}</span>
      </div>
    </button>
  );
}

// ── Main Plugins Page ─────────────────────────────────────

export default function PluginsPage() {
  const [plugins, setPlugins] = useState<PluginInfo[]>([]);
  const [sandboxPlugins, setSandboxPlugins] = useState<Record<string, unknown>[]>([]);
  const [loading, setLoading] = useState(true);
  const [sandboxLoading, setSandboxLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [stateFilter, setStateFilter] = useState<PluginState | "all">("all");
  const [search, setSearch] = useState("");
  const [showRegister, setShowRegister] = useState(false);
  const [selectedPlugin, setSelectedPlugin] = useState<PluginInfo | null>(null);
  const [activeTab, setActiveTab] = useState<"registered" | "sandbox">("registered");

  const loadRef = useRef<() => void>(() => {});
  const loadSandboxRef = useRef<() => void>(() => {});

  useEffect(() => {
    let cancelled = false;

    async function load() {
      try {
        const data = await fetchJSON<PluginListResponse>("/api/v1/plugins");
        if (!cancelled) {
          setPlugins(data.plugins);
          setError(null);
        }
      } catch {
        if (!cancelled) {
          setError("无法连接到插件系统");
        }
      } finally {
        if (!cancelled) {
          setLoading(false);
        }
      }
    }

    loadRef.current = load;
    load();
    const interval = setInterval(load, 5000);
    return () => {
      cancelled = true;
      clearInterval(interval);
    };
  }, []);

  useEffect(() => {
    let cancelled = false;

    async function loadSandbox() {
      try {
        const data = await fetchJSON<SandboxListResponse>("/api/v1/plugins/sandbox");
        if (!cancelled) {
          setSandboxPlugins(data.plugins);
        }
      } catch {
        // sandbox may not be initialized — silent
      } finally {
        if (!cancelled) {
          setSandboxLoading(false);
        }
      }
    }

    loadSandboxRef.current = loadSandbox;
    loadSandbox();
    const interval = setInterval(loadSandbox, 5000);
    return () => {
      cancelled = true;
      clearInterval(interval);
    };
  }, []);

  const loadPlugins = useCallback(() => loadRef.current(), []);
  const loadSandbox = useCallback(() => loadSandboxRef.current(), []);

  const filtered = useMemo(() => {
    let result = plugins;

    if (stateFilter !== "all") {
      result = result.filter((p) => p.state === stateFilter);
    }

    if (search.trim()) {
      const q = search.trim().toLowerCase();
      result = result.filter(
        (p) =>
          p.name.toLowerCase().includes(q) ||
          p.id.toLowerCase().includes(q) ||
          p.description.toLowerCase().includes(q) ||
          p.author.toLowerCase().includes(q),
      );
    }

    return result;
  }, [plugins, stateFilter, search]);

  const activeSelectedPlugin = useMemo(() => {
    if (!selectedPlugin) return null;
    return plugins.find((p) => p.id === selectedPlugin.id) ?? selectedPlugin;
  }, [plugins, selectedPlugin]);

  const counts = useMemo(() => {
    const c: Record<string, number> = { all: plugins.length };
    for (const key of Object.keys(STATE_CONFIG)) {
      c[key] = plugins.filter((p) => p.state === key).length;
    }
    return c;
  }, [plugins]);

  const stateFilterOptions: { value: PluginState | "all"; label: string }[] = [
    { value: "all", label: "全部" },
    { value: "active", label: "运行中" },
    { value: "registered", label: "已注册" },
    { value: "disabled", label: "已禁用" },
    { value: "error", label: "错误" },
    { value: "unloaded", label: "已卸载" },
  ];

  return (
    <div className="p-6 space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-zinc-100">插件市场</h1>
          <p className="text-sm text-zinc-500">
            {loading
              ? "正在加载..."
              : `${plugins.length} 个插件 · ${plugins.filter((p) => p.state === "active").length} 个运行中`}
          </p>
        </div>
        <div className="flex items-center gap-3">
          {error && (
            <div className="rounded-lg bg-red-500/10 border border-red-500/20 px-3 py-1.5 text-xs text-red-400">
              {error}
            </div>
          )}
          <button
            onClick={() => setShowRegister(true)}
            className="rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-500 transition-colors"
          >
            + 注册插件
          </button>
        </div>
      </div>

      {/* Tabs */}
      <div className="flex items-center gap-1.5 border-b border-zinc-800 pb-0">
        <button
          onClick={() => setActiveTab("registered")}
          className={`px-4 py-2.5 text-sm font-medium transition-colors border-b-2 -mb-px ${
            activeTab === "registered"
              ? "text-blue-400 border-blue-400"
              : "text-zinc-500 border-transparent hover:text-zinc-300"
          }`}
        >
          已注册插件
        </button>
        <button
          onClick={() => setActiveTab("sandbox")}
          className={`px-4 py-2.5 text-sm font-medium transition-colors border-b-2 -mb-px ${
            activeTab === "sandbox"
              ? "text-blue-400 border-blue-400"
              : "text-zinc-500 border-transparent hover:text-zinc-300"
          }`}
        >
          WASM 沙箱
        </button>
      </div>

      {/* Registered Plugins Tab */}
      {activeTab === "registered" && (
        <>
          {/* Filters & Search */}
          <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
            <div className="flex flex-wrap items-center gap-1.5">
              {stateFilterOptions.map((opt) => (
                <button
                  key={opt.value}
                  onClick={() => setStateFilter(opt.value)}
                  className={`rounded-lg px-3 py-1.5 text-xs font-medium transition-colors ${
                    stateFilter === opt.value
                      ? "bg-blue-500/15 text-blue-400 border border-blue-500/30"
                      : "bg-zinc-800/50 text-zinc-400 border border-zinc-800 hover:bg-zinc-800 hover:text-zinc-300"
                  }`}
                >
                  {opt.label} ({counts[opt.value] ?? 0})
                </button>
              ))}
            </div>
            <div className="relative">
              <input
                type="text"
                value={search}
                onChange={(e) => setSearch(e.target.value)}
                placeholder="搜索插件..."
                className="w-full rounded-lg border border-zinc-800 bg-zinc-900/50 px-3 py-2 pl-8 text-sm text-zinc-200 placeholder-zinc-600 outline-none transition-colors focus:border-zinc-700 focus:ring-1 focus:ring-zinc-700 sm:w-64"
              />
              <svg
                className="absolute left-2.5 top-2.5 h-3.5 w-3.5 text-zinc-600"
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth={2}
                  d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"
                />
              </svg>
            </div>
          </div>

          {/* Plugins Grid */}
          {loading ? (
            <div className="flex h-48 items-center justify-center text-sm text-zinc-600">
              正在加载插件列表...
            </div>
          ) : filtered.length === 0 ? (
            <div className="flex h-48 items-center justify-center text-sm text-zinc-600">
              {plugins.length === 0 ? "暂无已注册插件" : "没有匹配的插件"}
            </div>
          ) : (
            <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
              {filtered.map((plugin) => (
                <PluginCard
                  key={plugin.id}
                  plugin={plugin}
                  onClick={() => setSelectedPlugin(plugin)}
                />
              ))}
            </div>
          )}
        </>
      )}

      {/* WASM Sandbox Tab */}
      {activeTab === "sandbox" && (
        <>
          {sandboxLoading ? (
            <div className="flex h-48 items-center justify-center text-sm text-zinc-600">
              正在加载沙箱插件...
            </div>
          ) : sandboxPlugins.length === 0 ? (
            <div className="flex h-48 items-center justify-center text-sm text-zinc-600">
              沙箱中暂无 WASM 插件
            </div>
          ) : (
            <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
              {sandboxPlugins.map((plugin, idx) => (
                <div
                  key={idx}
                  className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 space-y-2"
                >
                  <pre className="text-xs text-zinc-400 overflow-x-auto">
                    {JSON.stringify(plugin, null, 2)}
                  </pre>
                </div>
              ))}
            </div>
          )}
        </>
      )}

      {/* Register Dialog */}
      <RegisterPluginDialog
        open={showRegister}
        onClose={() => setShowRegister(false)}
        onRegistered={loadPlugins}
      />

      {/* Plugin Detail Dialog */}
      {activeSelectedPlugin && (
        <PluginDetailDialog
          plugin={activeSelectedPlugin}
          onClose={() => setSelectedPlugin(null)}
          onAction={() => {
            loadPlugins();
            setSelectedPlugin(null);
          }}
        />
      )}
    </div>
  );
}
