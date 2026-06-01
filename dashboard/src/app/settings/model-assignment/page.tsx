"use client";

import { useEffect, useState, useCallback } from "react";
import type {
  AgentRecord,
  AgentModelAssignment,
  ProviderResponse,
  SetAgentModelRequest,
} from "@/types/world";
import { fetchJSON, putJSON } from "@/lib/api";

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

// ── Status Indicator ──────────────────────────────────────

type ModelStatus = "online" | "untested" | "unavailable";

const STATUS_CONFIG: Record<
  ModelStatus,
  { label: string; dot: string; color: string }
> = {
  online: {
    label: "在线",
    dot: "bg-green-400",
    color: "text-green-400",
  },
  untested: {
    label: "未测试",
    dot: "bg-yellow-400",
    color: "text-yellow-400",
  },
  unavailable: {
    label: "不可用",
    dot: "bg-red-400",
    color: "text-red-400",
  },
};

function StatusDot({ status }: { status: ModelStatus }) {
  const cfg = STATUS_CONFIG[status];
  return (
    <span className="inline-flex items-center gap-1.5" title={cfg.label}>
      <span
        className={`inline-block h-2.5 w-2.5 rounded-full ${cfg.dot}`}
      />
      <span className={`text-xs ${cfg.color}`}>{cfg.label}</span>
    </span>
  );
}

// ── Per-row edit state ────────────────────────────────────

interface RowEdit {
  provider_id: string;
  model_id: string;
  custom_model: string;
}

// ── Default Model Selector ────────────────────────────────

function DefaultModelSelector({
  providers,
  defaultProvider,
  onSave,
  saving,
}: {
  providers: ProviderResponse[];
  defaultProvider: ProviderResponse | null;
  onSave: (providerId: string, modelId: string) => void;
  saving: boolean;
}) {
  const [selectedProvider, setSelectedProvider] = useState(() =>
    defaultProvider?.id ?? "",
  );
  const [modelInput, setModelInput] = useState("");

  const currentProvider = providers.find((p) => p.id === selectedProvider);

  return (
    <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 sm:p-5">
      <h2 className="text-base font-bold text-zinc-100 mb-3">
        Pool 默认模型
      </h2>
      <p className="text-xs text-zinc-500 mb-3">
        未单独分配模型的 Agent 将使用此默认配置。
        当前默认 Provider：
        <span className="text-zinc-300 font-medium">
          {defaultProvider
            ? ` ${defaultProvider.display_name ?? defaultProvider.id}`
            : " 未设置"}
        </span>
      </p>
      <div className="flex flex-col sm:flex-row gap-3">
        <select
          value={selectedProvider}
          onChange={(e) => {
            setSelectedProvider(e.target.value);
            setModelInput("");
          }}
          className="flex-1 rounded-lg border border-zinc-800 bg-zinc-950 px-3 py-2 text-sm text-zinc-200 outline-none focus:border-zinc-700 focus:ring-1 focus:ring-zinc-700"
        >
          <option value="">-- 选择 Provider --</option>
          {providers.map((p) => (
            <option key={p.id} value={p.id}>
              {p.display_name ?? p.id}
            </option>
          ))}
        </select>
        <input
          type="text"
          value={modelInput}
          onChange={(e) => setModelInput(e.target.value)}
          placeholder="模型 ID (如 gpt-4o, qwen3:8b)"
          className="flex-1 rounded-lg border border-zinc-800 bg-zinc-950 px-3 py-2 text-sm text-zinc-200 placeholder-zinc-600 outline-none focus:border-zinc-700 focus:ring-1 focus:ring-zinc-700"
        />
        <button
          onClick={() => {
            if (selectedProvider && modelInput.trim()) {
              onSave(selectedProvider, modelInput.trim());
            }
          }}
          disabled={saving || !selectedProvider || !modelInput.trim()}
          className="rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-500 disabled:opacity-50 transition-colors shrink-0"
        >
          {saving ? "保存中..." : "设置默认"}
        </button>
      </div>
      {currentProvider && (
        <p className="text-xs text-zinc-600 mt-2">
          Provider: {currentProvider.protocol} · {currentProvider.base_url}
        </p>
      )}
    </div>
  );
}

// ── Batch Assign Dialog ───────────────────────────────────

function BatchAssignDialog({
  open,
  onClose,
  agentCount,
  providers,
  onAssign,
}: {
  open: boolean;
  onClose: () => void;
  agentCount: number;
  providers: ProviderResponse[];
  onAssign: (providerId: string, modelId: string) => void;
}) {
  const [providerId, setProviderId] = useState("");
  const [modelInput, setModelInput] = useState("");
  const [assigning, setAssigning] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") onClose();
    }
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [open, onClose]);

  const handleAssign = useCallback(async () => {
    if (!providerId || !modelInput.trim()) return;
    setAssigning(true);
    setError(null);
    try {
      onAssign(providerId, modelInput.trim());
      onClose();
      setProviderId("");
      setModelInput("");
    } catch (err) {
      setError(err instanceof Error ? err.message : "批量分配失败");
    } finally {
      setAssigning(false);
    }
  }, [providerId, modelInput, onAssign, onClose]);

  if (!open) return null;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60"
      onClick={onClose}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-label="批量分配模型"
        className="w-[calc(100vw-2rem)] max-w-lg rounded-xl border border-zinc-800 bg-zinc-900 p-4 sm:p-6 shadow-xl"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-lg font-bold text-zinc-100">批量分配模型</h2>
          <button
            onClick={onClose}
            className="text-zinc-500 hover:text-zinc-300 transition-colors"
          >
            <svg className="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>
        <p className="text-sm text-zinc-400 mb-4">
          将为选中的 <span className="text-zinc-200 font-medium">{agentCount}</span> 个 Agent 分配以下模型。
        </p>
        <div className="space-y-4">
          <div>
            <label className="block text-xs font-medium text-zinc-400 mb-1">
              Provider
            </label>
            <select
              value={providerId}
              onChange={(e) => {
                setProviderId(e.target.value);
                setModelInput("");
              }}
              className="w-full rounded-lg border border-zinc-800 bg-zinc-950 px-3 py-2 text-sm text-zinc-200 outline-none focus:border-zinc-700 focus:ring-1 focus:ring-zinc-700"
            >
              <option value="">-- 选择 Provider --</option>
              {providers.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.display_name ?? p.id}
                </option>
              ))}
            </select>
          </div>
          <div>
            <label className="block text-xs font-medium text-zinc-400 mb-1">
              模型 ID
            </label>
            <input
              type="text"
              value={modelInput}
              onChange={(e) => setModelInput(e.target.value)}
              placeholder="如 gpt-4o, qwen3:8b"
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
              onClick={handleAssign}
              disabled={assigning || !providerId || !modelInput.trim()}
              className="rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-500 disabled:opacity-50 transition-colors"
            >
              {assigning ? "分配中..." : "确认分配"}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

// ── Main Page ─────────────────────────────────────────────

export default function ModelAssignmentPage() {
  const [agents, setAgents] = useState<AgentRecord[]>([]);
  const [providers, setProviders] = useState<ProviderResponse[]>([]);
  const [assignments, setAssignments] = useState<
    Record<string, AgentModelAssignment>
  >({});
  // provider_id -> models[] cache
  const [providerModels, setProviderModels] = useState<
    Record<string, string[]>
  >({});
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Selection state for batch operations
  const [selectedAgents, setSelectedAgents] = useState<Set<string>>(new Set());
  const [showBatchDialog, setShowBatchDialog] = useState(false);

  // Per-row edit state
  const [rowEdits, setRowEdits] = useState<Record<string, RowEdit>>({});
  const [savingRows, setSavingRows] = useState<Set<string>>(new Set());
  const [rowErrors, setRowErrors] = useState<Record<string, string>>({});

  // Default model saving
  const [savingDefault, setSavingDefault] = useState(false);

  const defaultProvider = providers.find((p) => p.is_default) ?? null;

  // ── Data Loading ──────────────────────────────────────

  const loadData = useCallback(async () => {
    try {
      const [agentsData, providersData] = await Promise.all([
        fetchJSON<AgentRecord[]>("/api/v1/agents"),
        fetchJSON<ProviderResponse[]>("/api/v1/providers"),
      ]);

      setAgents(agentsData);
      setProviders(providersData);

      // Load model assignments for each agent
      const assignmentMap: Record<string, AgentModelAssignment> = {};
      const modelPromises = providersData.map(async (p) => {
        try {
          const result = await fetchJSON<{ models: string[] }>(
            `/api/v1/providers/${p.id}/models`,
          );
          return { id: p.id, models: result.models };
        } catch {
          return { id: p.id, models: [] };
        }
      });
      const modelResults = await Promise.all(modelPromises);
      const modelMap: Record<string, string[]> = {};
      for (const r of modelResults) {
        modelMap[r.id] = r.models;
      }
      setProviderModels(modelMap);

      // Load per-agent model assignments
      const assignPromises = agentsData.map(async (agent) => {
        try {
          const a = await fetchJSON<AgentModelAssignment | null>(
            `/api/v1/agents/${agent.id}/model`,
          );
          if (a) {
            assignmentMap[agent.id] = a;
          }
        } catch {
          // Agent may not have an assignment yet
        }
      });
      await Promise.all(assignPromises);
      setAssignments(assignmentMap);
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : "加载数据失败");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    let cancelled = false;
    async function load() {
      try {
        const [agentsData, providersData] = await Promise.all([
          fetchJSON<AgentRecord[]>("/api/v1/agents"),
          fetchJSON<ProviderResponse[]>("/api/v1/providers"),
        ]);
        if (cancelled) return;

        setAgents(agentsData);
        setProviders(providersData);

        // Discover models for each provider
        const modelPromises = providersData.map(async (p) => {
          try {
            const result = await fetchJSON<{ models: string[] }>(
              `/api/v1/providers/${p.id}/models`,
            );
            return { id: p.id, models: result.models };
          } catch {
            return { id: p.id, models: [] };
          }
        });
        const modelResults = await Promise.all(modelPromises);
        if (cancelled) return;

        const modelMap: Record<string, string[]> = {};
        for (const r of modelResults) {
          modelMap[r.id] = r.models;
        }
        setProviderModels(modelMap);

        // Load per-agent model assignments
        const assignmentMap: Record<string, AgentModelAssignment> = {};
        const assignPromises = agentsData.map(async (agent) => {
          try {
            const a = await fetchJSON<AgentModelAssignment | null>(
              `/api/v1/agents/${agent.id}/model`,
            );
            if (a) {
              assignmentMap[agent.id] = a;
            }
          } catch {
            // no assignment yet
          }
        });
        await Promise.all(assignPromises);
        if (!cancelled) {
          setAssignments(assignmentMap);
          setError(null);
        }
      } catch (err) {
        if (!cancelled) {
          setError(err instanceof Error ? err.message : "加载数据失败");
        }
      } finally {
        if (!cancelled) setLoading(false);
      }
    }
    load();
    return () => {
      cancelled = true;
    };
  }, []);

  // ── Save single agent model assignment ────────────────

  const handleSaveRow = useCallback(
    async (agentId: string) => {
      const edit = rowEdits[agentId];
      if (!edit) return;

      const modelId = edit.custom_model.trim() || edit.model_id;
      if (!edit.provider_id || !modelId) return;

      setSavingRows((prev) => new Set(prev).add(agentId));
      setRowErrors((prev) => {
        const next = { ...prev };
        delete next[agentId];
        return next;
      });

      try {
        const body: SetAgentModelRequest = {
          provider_id: edit.provider_id,
          model_id: modelId,
        };
        const result = await putJSON<AgentModelAssignment>(
          `/api/v1/agents/${agentId}/model`,
          body,
        );
        setAssignments((prev) => ({ ...prev, [agentId]: result }));
        // Clear the edit state
        setRowEdits((prev) => {
          const next = { ...prev };
          delete next[agentId];
          return next;
        });
      } catch (err) {
        setRowErrors((prev) => ({
          ...prev,
          [agentId]: err instanceof Error ? err.message : "保存失败",
        }));
      } finally {
        setSavingRows((prev) => {
          const next = new Set(prev);
          next.delete(agentId);
          return next;
        });
      }
    },
    [rowEdits],
  );

  // ── Reset to default (clear override) ─────────────────

  const handleResetRow = useCallback(
    async (agentId: string) => {
      // To reset, we assign the default provider and a generic model
      // or use the PUT endpoint with the default provider's info
      if (!defaultProvider) return;

      setSavingRows((prev) => new Set(prev).add(agentId));
      setRowErrors((prev) => {
        const next = { ...prev };
        delete next[agentId];
        return next;
      });

      try {
        // Clear the override by re-setting to default
        const body: SetAgentModelRequest = {
          provider_id: defaultProvider.id,
          model_id: "default",
        };
        await putJSON<AgentModelAssignment>(
          `/api/v1/agents/${agentId}/model`,
          body,
        );
        // Remove the assignment from local state
        setAssignments((prev) => {
          const next = { ...prev };
          delete next[agentId];
          return next;
        });
        setRowEdits((prev) => {
          const next = { ...prev };
          delete next[agentId];
          return next;
        });
      } catch (err) {
        setRowErrors((prev) => ({
          ...prev,
          [agentId]: err instanceof Error ? err.message : "重置失败",
        }));
      } finally {
        setSavingRows((prev) => {
          const next = new Set(prev);
          next.delete(agentId);
          return next;
        });
      }
    },
    [defaultProvider],
  );

  // ── Batch assign ──────────────────────────────────────

  const handleBatchAssign = useCallback(
    async (providerId: string, modelId: string) => {
      const ids = Array.from(selectedAgents);
      const promises = ids.map(async (agentId) => {
        try {
          const body: SetAgentModelRequest = {
            provider_id: providerId,
            model_id: modelId,
          };
          const result = await putJSON<AgentModelAssignment>(
            `/api/v1/agents/${agentId}/model`,
            body,
          );
          return { agentId, result };
        } catch (err) {
          return { agentId, error: err instanceof Error ? err.message : "分配失败" };
        }
      });

      const results = await Promise.all(promises);
      const newAssignments = { ...assignments };
      for (const r of results) {
        if ("result" in r && r.result) {
          newAssignments[r.agentId] = r.result;
        }
      }
      setAssignments(newAssignments);
      setSelectedAgents(new Set());
      // Clear any row edits for affected agents
      setRowEdits((prev) => {
        const next = { ...prev };
        for (const id of ids) delete next[id];
        return next;
      });
    },
    [selectedAgents, assignments],
  );

  // ── Default model save ────────────────────────────────

  const handleSaveDefault = useCallback(
    async (providerId: string, _modelId: string) => {
      setSavingDefault(true);
      try {
        // Update the provider to be the default
        await putJSON(`/api/v1/providers/${providerId}`, {
          is_default: true,
        });
        await loadData();
      } catch (err) {
        setError(err instanceof Error ? err.message : "设置默认模型失败");
      } finally {
        setSavingDefault(false);
      }
    },
    [loadData],
  );

  // ── Selection helpers ─────────────────────────────────

  const toggleSelectAll = useCallback(() => {
    if (selectedAgents.size === agents.length) {
      setSelectedAgents(new Set());
    } else {
      setSelectedAgents(new Set(agents.map((a) => a.id)));
    }
  }, [agents, selectedAgents]);

  const toggleSelect = useCallback((id: string) => {
    setSelectedAgents((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }, []);

  // ── Determine model status for an agent ───────────────

  const getModelStatus = useCallback(
    (agentId: string): ModelStatus => {
      const assignment = assignments[agentId];
      if (!assignment) return "untested";
      const provider = providers.find((p) => p.id === assignment.provider_id);
      if (!provider) return "unavailable";
      // If the provider exists and has models, we consider it online
      // (actual connectivity test would be done via the test endpoint)
      const models = providerModels[provider.id];
      if (models && models.length > 0) return "online";
      return "untested";
    },
    [assignments, providers, providerModels],
  );

  // ── Get current display info for an agent ─────────────

  const getCurrentModelDisplay = useCallback(
    (agentId: string): string => {
      const assignment = assignments[agentId];
      if (!assignment) return "默认";
      const provider = providers.find(
        (p) => p.id === assignment.provider_id,
      );
      const providerName = provider?.display_name ?? assignment.provider_id;
      return `${providerName} / ${assignment.model_id}`;
    },
    [assignments, providers],
  );

  // ── Row edit helpers ──────────────────────────────────

  const getRowEdit = useCallback(
    (agentId: string): RowEdit => {
      if (rowEdits[agentId]) return rowEdits[agentId];
      const assignment = assignments[agentId];
      return {
        provider_id: assignment?.provider_id ?? "",
        model_id: assignment?.model_id ?? "",
        custom_model: "",
      };
    },
    [rowEdits, assignments],
  );

  const updateRowEdit = useCallback(
    (agentId: string, updates: Partial<RowEdit>) => {
      setRowEdits((prev) => ({
        ...prev,
        [agentId]: { ...getRowEdit(agentId), ...updates },
      }));
    },
    [getRowEdit],
  );

  // ── Render ────────────────────────────────────────────

  const allSelected = agents.length > 0 && selectedAgents.size === agents.length;

  return (
    <div className="p-4 md:p-6 space-y-6">
      {/* Header */}
      <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
        <div>
          <h1 className="text-xl md:text-2xl font-bold text-zinc-100">
            Agent 模型分配
          </h1>
          <p className="text-sm text-zinc-500">
            {loading
              ? "正在加载..."
              : `共 ${agents.length} 个 Agent · ${providers.length} 个 Provider`}
          </p>
        </div>
        <div className="flex items-center gap-3">
          {error && (
            <div className="rounded-lg bg-red-500/10 border border-red-500/20 px-3 py-1.5 text-xs text-red-400">
              {error}
            </div>
          )}
          {selectedAgents.size > 0 && (
            <button
              onClick={() => setShowBatchDialog(true)}
              className="rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-500 transition-colors"
            >
              批量分配 ({selectedAgents.size})
            </button>
          )}
          <button
            onClick={loadData}
            className="rounded-lg bg-zinc-800 px-4 py-2 text-sm font-medium text-zinc-200 hover:bg-zinc-700 transition-colors"
          >
            刷新
          </button>
        </div>
      </div>

      {/* No providers guidance */}
      {!loading && providers.length === 0 && (
        <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-8 text-center">
          <p className="text-sm text-zinc-400 mb-2">
            尚未注册任何 Provider。请先前往 Provider 管理页面添加至少一个 Provider。
          </p>
          <a
            href="/settings/providers"
            className="text-sm text-blue-400 hover:text-blue-300 underline"
          >
            前往 Provider 管理 &rarr;
          </a>
        </div>
      )}

      {/* Default Model Selector */}
      {!loading && providers.length > 0 && (
        <DefaultModelSelector
          providers={providers}
          defaultProvider={defaultProvider}
          onSave={handleSaveDefault}
          saving={savingDefault}
        />
      )}

      {/* Batch operations toolbar */}
      {!loading && providers.length > 0 && agents.length > 0 && (
        <div className="flex items-center gap-3">
          <label className="inline-flex items-center gap-2 cursor-pointer">
            <input
              type="checkbox"
              checked={allSelected}
              onChange={toggleSelectAll}
              className="h-4 w-4 rounded border-zinc-700 bg-zinc-950 text-blue-600 focus:ring-blue-500 focus:ring-offset-0"
            />
            <span className="text-xs text-zinc-400">
              {allSelected ? "取消全选" : "全选"}
            </span>
          </label>
          {selectedAgents.size > 0 && (
            <button
              onClick={() => {
                if (defaultProvider) {
                  handleBatchAssign(defaultProvider.id, "default");
                }
              }}
              disabled={!defaultProvider}
              className="text-xs text-zinc-400 hover:text-zinc-200 transition-colors disabled:opacity-50"
            >
              全部使用默认
            </button>
          )}
        </div>
      )}

      {/* Agent Assignment Table */}
      {loading ? (
        <div className="flex h-48 items-center justify-center text-sm text-zinc-600">
          <Spinner className="h-6 w-6 mr-2" />
          正在加载 Agent 和模型数据...
        </div>
      ) : agents.length === 0 ? (
        <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-12 text-center">
          <p className="text-sm text-zinc-500 mb-3">暂无 Agent</p>
          <p className="text-xs text-zinc-600">
            当有 Agent 注册后将自动显示在此处
          </p>
        </div>
      ) : providers.length > 0 ? (
        <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 overflow-hidden">
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-zinc-800 text-left">
                  <th className="px-4 py-3 text-xs font-medium text-zinc-500 w-10">
                    <span className="sr-only">选择</span>
                  </th>
                  <th className="px-4 py-3 text-xs font-medium text-zinc-500">
                    Agent
                  </th>
                  <th className="px-4 py-3 text-xs font-medium text-zinc-500">
                    当前模型
                  </th>
                  <th className="px-4 py-3 text-xs font-medium text-zinc-500">
                    状态
                  </th>
                  <th className="px-4 py-3 text-xs font-medium text-zinc-500">
                    Provider
                  </th>
                  <th className="px-4 py-3 text-xs font-medium text-zinc-500">
                    模型
                  </th>
                  <th className="px-4 py-3 text-xs font-medium text-zinc-500 text-right">
                    操作
                  </th>
                </tr>
              </thead>
              <tbody>
                {agents.map((agent) => {
                  const edit = getRowEdit(agent.id);
                  const saving = savingRows.has(agent.id);
                  const rowError = rowErrors[agent.id];
                  const selected = selectedAgents.has(agent.id);
                  const status = getModelStatus(agent.id);
                  const currentModels = edit.provider_id
                    ? providerModels[edit.provider_id] ?? []
                    : [];

                  return (
                    <tr
                      key={agent.id}
                      className={`border-b border-zinc-800/50 hover:bg-zinc-800/30 ${
                        selected ? "bg-blue-500/5" : ""
                      }`}
                    >
                      {/* Checkbox */}
                      <td className="px-4 py-3">
                        <input
                          type="checkbox"
                          checked={selected}
                          onChange={() => toggleSelect(agent.id)}
                          className="h-4 w-4 rounded border-zinc-700 bg-zinc-950 text-blue-600 focus:ring-blue-500 focus:ring-offset-0"
                        />
                      </td>

                      {/* Agent Name */}
                      <td className="px-4 py-3">
                        <div>
                          <p className="text-zinc-200 font-medium">
                            {agent.name}
                          </p>
                          <p className="text-xs text-zinc-600 font-mono">
                            {agent.id.slice(0, 8)}
                          </p>
                        </div>
                      </td>

                      {/* Current Model */}
                      <td className="px-4 py-3">
                        <span className="text-zinc-400 text-xs font-mono">
                          {getCurrentModelDisplay(agent.id)}
                        </span>
                      </td>

                      {/* Status */}
                      <td className="px-4 py-3">
                        <StatusDot status={status} />
                      </td>

                      {/* Provider Select */}
                      <td className="px-4 py-3">
                        <select
                          value={edit.provider_id}
                          onChange={(e) => {
                            updateRowEdit(agent.id, {
                              provider_id: e.target.value,
                              model_id: "",
                              custom_model: "",
                            });
                          }}
                          className="w-full min-w-[140px] rounded-lg border border-zinc-800 bg-zinc-950 px-3 py-1.5 text-xs text-zinc-200 outline-none focus:border-zinc-700 focus:ring-1 focus:ring-zinc-700"
                        >
                          <option value="">-- 选择 --</option>
                          {providers.map((p) => (
                            <option key={p.id} value={p.id}>
                              {p.display_name ?? p.id}
                            </option>
                          ))}
                        </select>
                      </td>

                      {/* Model Select / Custom Input */}
                      <td className="px-4 py-3">
                        <div className="flex items-center gap-2 min-w-[180px]">
                          <select
                            value={edit.model_id}
                            onChange={(e) =>
                              updateRowEdit(agent.id, {
                                model_id: e.target.value,
                                custom_model: "",
                              })
                            }
                            className="flex-1 rounded-lg border border-zinc-800 bg-zinc-950 px-3 py-1.5 text-xs text-zinc-200 outline-none focus:border-zinc-700 focus:ring-1 focus:ring-zinc-700"
                          >
                            <option value="">-- 选择模型 --</option>
                            {currentModels.map((m) => (
                              <option key={m} value={m}>
                                {m}
                              </option>
                            ))}
                            <option value="__custom__">手动输入...</option>
                          </select>
                          {edit.model_id === "__custom__" && (
                            <input
                              type="text"
                              value={edit.custom_model}
                              onChange={(e) =>
                                updateRowEdit(agent.id, {
                                  custom_model: e.target.value,
                                })
                              }
                              placeholder="model-id"
                              className="w-28 rounded-lg border border-zinc-800 bg-zinc-950 px-3 py-1.5 text-xs text-zinc-200 placeholder-zinc-600 outline-none focus:border-zinc-700 focus:ring-1 focus:ring-zinc-700"
                            />
                          )}
                        </div>
                      </td>

                      {/* Actions */}
                      <td className="px-4 py-3 text-right">
                        <div className="flex items-center justify-end gap-2">
                          {rowError && (
                            <span
                              className="text-xs text-red-400"
                              title={rowError}
                            >
                              错误
                            </span>
                          )}
                          <button
                            onClick={() => handleSaveRow(agent.id)}
                            disabled={
                              saving ||
                              !edit.provider_id ||
                              (!edit.model_id && !edit.custom_model.trim())
                            }
                            className="rounded-lg bg-blue-600 px-3 py-1.5 text-xs font-medium text-white hover:bg-blue-500 disabled:opacity-50 transition-colors"
                          >
                            {saving ? "..." : "保存"}
                          </button>
                          <button
                            onClick={() => handleResetRow(agent.id)}
                            disabled={saving || !defaultProvider}
                            title="重置为默认模型"
                            className="rounded-lg px-3 py-1.5 text-xs font-medium text-zinc-400 hover:bg-zinc-800 hover:text-zinc-200 disabled:opacity-50 transition-colors"
                          >
                            重置
                          </button>
                        </div>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        </div>
      ) : null}

      {/* Batch Assign Dialog */}
      <BatchAssignDialog
        open={showBatchDialog}
        onClose={() => setShowBatchDialog(false)}
        agentCount={selectedAgents.size}
        providers={providers}
        onAssign={handleBatchAssign}
      />
    </div>
  );
}
