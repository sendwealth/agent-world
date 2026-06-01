"use client";

import { useEffect, useState, useCallback } from "react";
import type {
  Provider,
  ProviderProtocol,
  ConnectionStatus,
  ConnectionTestResult,
  DiscoverModelsResult,
} from "@/types/world";
import { fetchJSON, postJSON, putJSON, deleteJSON } from "@/lib/api";

// ── Constants ─────────────────────────────────────────────

const PROTOCOL_OPTIONS: { value: ProviderProtocol; label: string }[] = [
  { value: "openai_compatible", label: "OpenAI 兼容" },
  { value: "anthropic", label: "Anthropic" },
  { value: "ollama", label: "Ollama" },
  { value: "google", label: "Google" },
  { value: "azure", label: "Azure" },
];

const STATUS_CONFIG: Record<
  ConnectionStatus,
  { label: string; dot: string; color: string }
> = {
  online: { label: "在线", dot: "bg-green-400", color: "bg-green-500/10 text-green-400" },
  offline: { label: "离线", dot: "bg-red-400", color: "bg-red-500/10 text-red-400" },
  untested: { label: "未测试", dot: "bg-zinc-500", color: "bg-zinc-500/10 text-zinc-400" },
};

const PRESET_TEMPLATES: {
  label: string;
  protocol: ProviderProtocol;
  base_url: string;
  needs_api_key: boolean;
}[] = [
  {
    label: "Ollama 本地",
    protocol: "ollama",
    base_url: "http://localhost:11434",
    needs_api_key: false,
  },
  {
    label: "智谱 AI",
    protocol: "openai_compatible",
    base_url: "https://open.bigmodel.cn/api/paas/v4",
    needs_api_key: true,
  },
  {
    label: "DeepSeek",
    protocol: "openai_compatible",
    base_url: "https://api.deepseek.com",
    needs_api_key: true,
  },
  {
    label: "OpenRouter",
    protocol: "openai_compatible",
    base_url: "https://openrouter.ai/api/v1",
    needs_api_key: true,
  },
];

// ── Spinner Component ─────────────────────────────────────

function Spinner({ className = "h-4 w-4" }: { className?: string }) {
  return (
    <svg
      className={`animate-spin ${className}`}
      fill="none"
      viewBox="0 0 24 24"
    >
      <circle
        className="opacity-25"
        cx="12"
        cy="12"
        r="10"
        stroke="currentColor"
        strokeWidth="4"
      />
      <path
        className="opacity-75"
        fill="currentColor"
        d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"
      />
    </svg>
  );
}

// ── Add Provider Dialog ───────────────────────────────────

interface AddProviderForm {
  id: string;
  display_name: string;
  protocol: ProviderProtocol;
  base_url: string;
  api_key: string;
}

function AddProviderDialog({
  open,
  onClose,
  onCreated,
}: {
  open: boolean;
  onClose: () => void;
  onCreated: () => void;
}) {
  const [form, setForm] = useState<AddProviderForm>({
    id: "",
    display_name: "",
    protocol: "openai_compatible",
    base_url: "",
    api_key: "",
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

  const handleSubmit = useCallback(
    async (e: React.FormEvent) => {
      e.preventDefault();
      setError(null);

      if (!form.id.trim()) {
        setError("请输入 Provider ID");
        return;
      }
      if (!form.display_name.trim()) {
        setError("请输入显示名称");
        return;
      }
      if (!form.base_url.trim()) {
        setError("请输入 Base URL");
        return;
      }

      setSubmitting(true);
      try {
        const body: Record<string, unknown> = {
          id: form.id.trim(),
          display_name: form.display_name.trim(),
          protocol: form.protocol,
          base_url: form.base_url.trim(),
        };
        if (form.api_key.trim()) {
          body.api_key = form.api_key.trim();
        }
        await postJSON<Provider>("/api/v1/providers", body);
        setForm({
          id: "",
          display_name: "",
          protocol: "openai_compatible",
          base_url: "",
          api_key: "",
        });
        onCreated();
        onClose();
      } catch (err) {
        setError(err instanceof Error ? err.message : "创建失败");
      } finally {
        setSubmitting(false);
      }
    },
    [form, onCreated, onClose],
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
        aria-label="添加 Provider"
        className="w-[calc(100vw-2rem)] max-w-lg rounded-xl border border-zinc-800 bg-zinc-900 p-4 sm:p-6 shadow-xl"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-lg font-bold text-zinc-100">添加 Provider</h2>
          <button
            onClick={onClose}
            className="text-zinc-500 hover:text-zinc-300 transition-colors"
          >
            <svg className="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>

        <form onSubmit={handleSubmit} className="space-y-4">
          <div>
            <label className="block text-xs font-medium text-zinc-400 mb-1">
              协议类型
            </label>
            <select
              value={form.protocol}
              onChange={(e) =>
                setForm((f) => ({ ...f, protocol: e.target.value as ProviderProtocol }))
              }
              className="w-full rounded-lg border border-zinc-800 bg-zinc-950 px-3 py-2 text-sm text-zinc-200 outline-none focus:border-zinc-700 focus:ring-1 focus:ring-zinc-700"
            >
              {PROTOCOL_OPTIONS.map((opt) => (
                <option key={opt.value} value={opt.value}>
                  {opt.label}
                </option>
              ))}
            </select>
          </div>

          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="block text-xs font-medium text-zinc-400 mb-1">
                ID <span className="text-red-400">*</span>
              </label>
              <input
                type="text"
                value={form.id}
                onChange={(e) => setForm((f) => ({ ...f, id: e.target.value }))}
                placeholder="例如 ollama-local"
                className="w-full rounded-lg border border-zinc-800 bg-zinc-950 px-3 py-2 text-sm text-zinc-200 placeholder-zinc-600 outline-none focus:border-zinc-700 focus:ring-1 focus:ring-zinc-700"
              />
            </div>
            <div>
              <label className="block text-xs font-medium text-zinc-400 mb-1">
                显示名称 <span className="text-red-400">*</span>
              </label>
              <input
                type="text"
                value={form.display_name}
                onChange={(e) => setForm((f) => ({ ...f, display_name: e.target.value }))}
                placeholder="例如 Ollama 本地"
                className="w-full rounded-lg border border-zinc-800 bg-zinc-950 px-3 py-2 text-sm text-zinc-200 placeholder-zinc-600 outline-none focus:border-zinc-700 focus:ring-1 focus:ring-zinc-700"
              />
            </div>
          </div>

          <div>
            <label className="block text-xs font-medium text-zinc-400 mb-1">
              Base URL <span className="text-red-400">*</span>
            </label>
            <input
              type="url"
              value={form.base_url}
              onChange={(e) => setForm((f) => ({ ...f, base_url: e.target.value }))}
              placeholder="https://api.example.com/v1"
              className="w-full rounded-lg border border-zinc-800 bg-zinc-950 px-3 py-2 text-sm text-zinc-200 placeholder-zinc-600 outline-none focus:border-zinc-700 focus:ring-1 focus:ring-zinc-700"
            />
          </div>

          <div>
            <label className="block text-xs font-medium text-zinc-400 mb-1">
              API Key
            </label>
            <input
              type="password"
              value={form.api_key}
              onChange={(e) => setForm((f) => ({ ...f, api_key: e.target.value }))}
              placeholder="留空则不设置 API Key"
              className="w-full rounded-lg border border-zinc-800 bg-zinc-950 px-3 py-2 text-sm text-zinc-200 placeholder-zinc-600 outline-none focus:border-zinc-700 focus:ring-1 focus:ring-zinc-700"
            />
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
              {submitting ? "保存中..." : "保存"}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}

// ── Delete Confirm Dialog ─────────────────────────────────

function DeleteConfirmDialog({
  provider,
  onClose,
  onDeleted,
}: {
  provider: Provider;
  onClose: () => void;
  onDeleted: () => void;
}) {
  const [deleting, setDeleting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") onClose();
    }
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [onClose]);

  const handleDelete = useCallback(async () => {
    setDeleting(true);
    setError(null);
    try {
      await deleteJSON(`/api/v1/providers/${provider.id}`);
      onDeleted();
      onClose();
    } catch (err) {
      setError(err instanceof Error ? err.message : "删除失败");
    } finally {
      setDeleting(false);
    }
  }, [provider.id, onDeleted, onClose]);

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60"
      onClick={onClose}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-label="确认删除"
        className="w-[calc(100vw-2rem)] max-w-md rounded-xl border border-zinc-800 bg-zinc-900 p-4 sm:p-6 shadow-xl"
        onClick={(e) => e.stopPropagation()}
      >
        <h2 className="text-lg font-bold text-zinc-100 mb-3">确认删除</h2>
        <p className="text-sm text-zinc-400 mb-4">
          确定要删除 Provider{" "}
          <span className="text-zinc-200 font-medium">{provider.display_name}</span>{" "}
          ({provider.id}) 吗？此操作不可撤销。
        </p>

        {error && (
          <div className="rounded-lg bg-red-500/10 border border-red-500/20 px-3 py-1.5 text-xs text-red-400 mb-4">
            {error}
          </div>
        )}

        <div className="flex items-center justify-end gap-3">
          <button
            type="button"
            onClick={onClose}
            className="rounded-lg px-4 py-2 text-sm text-zinc-400 hover:text-zinc-200 transition-colors"
          >
            取消
          </button>
          <button
            onClick={handleDelete}
            disabled={deleting}
            className="rounded-lg bg-red-600 px-4 py-2 text-sm font-medium text-white hover:bg-red-500 disabled:opacity-50 transition-colors"
          >
            {deleting ? "删除中..." : "删除"}
          </button>
        </div>
      </div>
    </div>
  );
}

// ── Provider Card ─────────────────────────────────────────

function ProviderCard({
  provider,
  onRefresh,
}: {
  provider: Provider;
  onRefresh: () => void;
}) {
  const [editing, setEditing] = useState(false);
  const [baseUrl, setBaseUrl] = useState(provider.base_url);
  const [apiKey, setApiKey] = useState(provider.api_key ?? "");
  const [showApiKey, setShowApiKey] = useState(false);
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);
  const [models, setModels] = useState<string[]>(provider.models ?? []);

  // Connection test state
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<ConnectionTestResult | null>(null);

  // Discover models state
  const [discovering, setDiscovering] = useState(false);
  const [discoverError, setDiscoverError] = useState<string | null>(null);

  // Edit/save state
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);

  const handleTestConnection = useCallback(async () => {
    setTesting(true);
    setTestResult(null);
    try {
      const result = await postJSON<ConnectionTestResult>(
        `/api/v1/providers/${provider.id}/test`,
        {},
      );
      setTestResult(result);
      onRefresh();
    } catch (err) {
      setTestResult({
        success: false,
        latency_ms: 0,
        error: err instanceof Error ? err.message : "测试失败",
      });
    } finally {
      setTesting(false);
    }
  }, [provider.id, onRefresh]);

  const handleDiscoverModels = useCallback(async () => {
    setDiscovering(true);
    setDiscoverError(null);
    try {
      const result = await postJSON<DiscoverModelsResult>(
        `/api/v1/providers/${provider.id}/models`,
        {},
      );
      setModels(result.models);
      onRefresh();
    } catch (err) {
      setDiscoverError(err instanceof Error ? err.message : "发现模型失败");
    } finally {
      setDiscovering(false);
    }
  }, [provider.id, onRefresh]);

  const handleSave = useCallback(async () => {
    setSaving(true);
    setSaveError(null);
    try {
      const body: Record<string, unknown> = {
        base_url: baseUrl.trim(),
      };
      if (apiKey.trim()) {
        body.api_key = apiKey.trim();
      }
      await putJSON(`/api/v1/providers/${provider.id}`, body);
      setEditing(false);
      onRefresh();
    } catch (err) {
      setSaveError(err instanceof Error ? err.message : "保存失败");
    } finally {
      setSaving(false);
    }
  }, [provider.id, baseUrl, apiKey, onRefresh]);

  const status: ConnectionStatus = provider.status ?? "untested";
  const statusCfg = STATUS_CONFIG[status];

  return (
    <>
      <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 sm:p-5">
        {/* Header row */}
        <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between mb-4">
          <div className="flex items-center gap-2">
            <span
              className={`inline-block h-2.5 w-2.5 rounded-full ${statusCfg.dot}`}
              title={statusCfg.label}
            />
            <h3 className="text-base font-bold text-zinc-100">
              {provider.display_name}
            </h3>
            <span className="text-xs text-zinc-500 font-mono">({provider.id})</span>
            <span
              className={`inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-[11px] font-medium ${statusCfg.color}`}
            >
              {statusCfg.label}
            </span>
          </div>
          <div className="flex items-center gap-2">
            <span className="text-[11px] text-zinc-500 bg-zinc-800/50 rounded px-2 py-0.5">
              {PROTOCOL_OPTIONS.find((o) => o.value === provider.protocol)?.label ?? provider.protocol}
            </span>
            <button
              onClick={() => setEditing(!editing)}
              className="rounded-lg px-2 py-1 text-xs text-zinc-400 hover:bg-zinc-800 hover:text-zinc-200 transition-colors"
            >
              {editing ? "取消编辑" : "编辑"}
            </button>
          </div>
        </div>

        {/* Config fields */}
        <div className="space-y-3">
          <div>
            <label className="block text-xs font-medium text-zinc-400 mb-1">
              Base URL
            </label>
            {editing ? (
              <input
                type="url"
                value={baseUrl}
                onChange={(e) => setBaseUrl(e.target.value)}
                className="w-full rounded-lg border border-zinc-800 bg-zinc-950 px-3 py-2 text-sm text-zinc-200 placeholder-zinc-600 outline-none focus:border-zinc-700 focus:ring-1 focus:ring-zinc-700"
              />
            ) : (
              <p className="text-sm text-zinc-300 font-mono break-all">
                {provider.base_url}
              </p>
            )}
          </div>

          <div>
            <label className="block text-xs font-medium text-zinc-400 mb-1">
              API Key
            </label>
            <div className="flex items-center gap-2">
              {editing ? (
                <>
                  <input
                    type={showApiKey ? "text" : "password"}
                    value={apiKey}
                    onChange={(e) => setApiKey(e.target.value)}
                    placeholder={provider.api_key ? "••••••••" : "未设置"}
                    className="flex-1 rounded-lg border border-zinc-800 bg-zinc-950 px-3 py-2 text-sm text-zinc-200 placeholder-zinc-600 outline-none focus:border-zinc-700 focus:ring-1 focus:ring-zinc-700"
                  />
                  <button
                    type="button"
                    onClick={() => setShowApiKey(!showApiKey)}
                    className="shrink-0 rounded-lg border border-zinc-800 px-2 py-2 text-xs text-zinc-400 hover:bg-zinc-800 hover:text-zinc-200 transition-colors"
                  >
                    {showApiKey ? "隐藏" : "显示"}
                  </button>
                </>
              ) : (
                <>
                  <p className="text-sm text-zinc-300 font-mono">
                    {provider.api_key
                      ? showApiKey
                        ? provider.api_key
                        : "••••••••••••"
                      : "未设置"}
                  </p>
                  {provider.api_key && (
                    <button
                      type="button"
                      onClick={() => setShowApiKey(!showApiKey)}
                      className="shrink-0 text-xs text-zinc-500 hover:text-zinc-300 transition-colors"
                    >
                      {showApiKey ? "隐藏" : "显示"}
                    </button>
                  )}
                </>
              )}
            </div>
          </div>

          {/* Models dropdown */}
          {models.length > 0 && (
            <div>
              <label className="block text-xs font-medium text-zinc-400 mb-1">
                可用模型
              </label>
              <select
                className="w-full rounded-lg border border-zinc-800 bg-zinc-950 px-3 py-2 text-sm text-zinc-200 outline-none focus:border-zinc-700 focus:ring-1 focus:ring-zinc-700"
                disabled
              >
                <option value="">-- {models.length} 个模型 --</option>
                {models.map((m) => (
                  <option key={m} value={m}>
                    {m}
                  </option>
                ))}
              </select>
            </div>
          )}
        </div>

        {/* Connection test result */}
        {testResult && (
          <div
            className={`mt-3 rounded-lg border px-3 py-2 text-xs ${
              testResult.success
                ? "bg-green-500/10 border-green-500/20 text-green-400"
                : "bg-red-500/10 border-red-500/20 text-red-400"
            }`}
          >
            <div className="flex items-center gap-2">
              <span>{testResult.success ? "\u2713" : "\u2717"}</span>
              {testResult.success ? (
                <span>
                  连接成功 · 延迟{" "}
                  <span className="font-mono font-medium">
                    {testResult.latency_ms}ms
                  </span>
                  {testResult.sample && (
                    <span className="text-green-400/70 ml-2">
                      {testResult.sample.slice(0, 60)}
                      {testResult.sample.length > 60 ? "..." : ""}
                    </span>
                  )}
                </span>
              ) : (
                <span>
                  {testResult.error ?? "连接失败"}
                  {testResult.error?.includes("auth") && " (认证失败，请检查 API Key)"}
                  {testResult.error?.includes("connect") && " (无法连接，请检查 Base URL)"}
                  {testResult.error?.includes("timeout") && " (连接超时)"}
                </span>
              )}
            </div>
          </div>
        )}

        {/* Discover error */}
        {discoverError && (
          <div className="mt-3 rounded-lg bg-red-500/10 border border-red-500/20 px-3 py-1.5 text-xs text-red-400">
            {discoverError}
          </div>
        )}

        {/* Save error */}
        {saveError && (
          <div className="mt-3 rounded-lg bg-red-500/10 border border-red-500/20 px-3 py-1.5 text-xs text-red-400">
            {saveError}
          </div>
        )}

        {/* Action buttons */}
        <div className="flex flex-wrap items-center gap-2 mt-4 pt-4 border-t border-zinc-800">
          <button
            onClick={handleTestConnection}
            disabled={testing}
            className="inline-flex items-center gap-1.5 rounded-lg bg-zinc-800 px-3 py-1.5 text-xs font-medium text-zinc-200 hover:bg-zinc-700 disabled:opacity-50 transition-colors"
          >
            {testing ? <Spinner /> : <span>\u26A1</span>}
            {testing ? "测试中..." : "Test Connection"}
          </button>

          <button
            onClick={handleDiscoverModels}
            disabled={discovering}
            className="inline-flex items-center gap-1.5 rounded-lg bg-zinc-800 px-3 py-1.5 text-xs font-medium text-zinc-200 hover:bg-zinc-700 disabled:opacity-50 transition-colors"
          >
            {discovering ? <Spinner /> : <span>\U0001F50D</span>}
            {discovering ? "发现中..." : "Discover Models"}
          </button>

          {editing && (
            <button
              onClick={handleSave}
              disabled={saving}
              className="inline-flex items-center gap-1.5 rounded-lg bg-blue-600 px-3 py-1.5 text-xs font-medium text-white hover:bg-blue-500 disabled:opacity-50 transition-colors"
            >
              {saving && <Spinner />}
              {saving ? "保存中..." : "保存"}
            </button>
          )}

          <div className="flex-1" />

          <button
            onClick={() => setShowDeleteConfirm(true)}
            className="inline-flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-xs font-medium text-red-400 hover:bg-red-500/10 transition-colors"
          >
            删除
          </button>
        </div>

        {/* Preset template quick buttons */}
        {editing && (
          <div className="mt-3 flex flex-wrap gap-2">
            <span className="text-[11px] text-zinc-500 self-center">快捷预设:</span>
            {PRESET_TEMPLATES.map((preset) => (
              <button
                key={preset.label}
                type="button"
                onClick={() => {
                  setBaseUrl(preset.base_url);
                  if (!preset.needs_api_key) setApiKey("");
                }}
                className="rounded-lg border border-zinc-800 bg-zinc-800/50 px-2.5 py-1 text-[11px] text-zinc-400 hover:bg-zinc-800 hover:text-zinc-200 transition-colors"
              >
                {preset.label}
              </button>
            ))}
          </div>
        )}
      </div>

      {/* Delete confirm dialog */}
      {showDeleteConfirm && (
        <DeleteConfirmDialog
          provider={provider}
          onClose={() => setShowDeleteConfirm(false)}
          onDeleted={onRefresh}
        />
      )}
    </>
  );
}

// ── Main Providers Page ───────────────────────────────────

export default function ProvidersPage() {
  const [providers, setProviders] = useState<Provider[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [showAdd, setShowAdd] = useState(false);

  const loadProviders = useCallback(async () => {
    try {
      const data = await fetchJSON<Provider[]>("/api/v1/providers");
      setProviders(data);
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : "加载 Provider 列表失败");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    let cancelled = false;

    async function load() {
      try {
        const data = await fetchJSON<Provider[]>("/api/v1/providers");
        if (!cancelled) {
          setProviders(data);
          setError(null);
        }
      } catch (err) {
        if (!cancelled) {
          setError(err instanceof Error ? err.message : "加载 Provider 列表失败");
        }
      } finally {
        if (!cancelled) {
          setLoading(false);
        }
      }
    }

    load();
    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <div className="p-4 md:p-6 space-y-6">
      {/* Header */}
      <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
        <div>
          <h1 className="text-xl md:text-2xl font-bold text-zinc-100">
            Provider 管理
          </h1>
          <p className="text-sm text-zinc-500">
            {loading
              ? "正在加载..."
              : `共 ${providers.length} 个 Provider`}
          </p>
        </div>
        <div className="flex items-center gap-3">
          {error && (
            <div className="rounded-lg bg-red-500/10 border border-red-500/20 px-3 py-1.5 text-xs text-red-400">
              {error}
            </div>
          )}
          <button
            onClick={() => setShowAdd(true)}
            className="rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-500 transition-colors"
          >
            + 添加 Provider
          </button>
        </div>
      </div>

      {/* Provider list */}
      {loading ? (
        <div className="flex h-48 items-center justify-center text-sm text-zinc-600">
          <Spinner className="h-6 w-6 mr-2" />
          正在加载 Provider 数据...
        </div>
      ) : providers.length === 0 ? (
        <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-12 text-center">
          <p className="text-sm text-zinc-500 mb-3">暂无 Provider</p>
          <p className="text-xs text-zinc-600">
            点击上方 &quot;添加 Provider&quot; 按钮开始配置
          </p>
        </div>
      ) : (
        <div className="grid gap-4 lg:grid-cols-2">
          {providers.map((provider) => (
            <ProviderCard
              key={provider.id}
              provider={provider}
              onRefresh={loadProviders}
            />
          ))}
        </div>
      )}

      {/* Add Provider Dialog */}
      <AddProviderDialog
        open={showAdd}
        onClose={() => setShowAdd(false)}
        onCreated={loadProviders}
      />
    </div>
  );
}
