"use client";

import { useEffect, useState, useMemo, useCallback } from "react";
import type { Task, TaskStatus } from "@/types/world";
import { postJSON } from "@/lib/api";
import { useTaskStream } from "@/hooks/useTaskStream";

// ── Constants ─────────────────────────────────────────────

const STATUS_ORDER: TaskStatus[] = [
  "published",
  "claimed",
  "in_progress",
  "submitted",
  "reviewed",
  "completed",
  "expired",
];

const STATUS_CONFIG: Record<
  TaskStatus,
  { label: string; color: string; dot: string }
> = {
  published: { label: "已发布", color: "bg-blue-500/10 text-blue-400", dot: "bg-blue-400" },
  claimed: { label: "已认领", color: "bg-amber-500/10 text-amber-400", dot: "bg-amber-400" },
  in_progress: { label: "进行中", color: "bg-yellow-500/10 text-yellow-400", dot: "bg-yellow-400" },
  submitted: { label: "已提交", color: "bg-purple-500/10 text-purple-400", dot: "bg-purple-400" },
  reviewed: { label: "已审核", color: "bg-cyan-500/10 text-cyan-400", dot: "bg-cyan-400" },
  completed: { label: "已完成", color: "bg-green-500/10 text-green-400", dot: "bg-green-400" },
  expired: { label: "已过期", color: "bg-zinc-500/10 text-zinc-400", dot: "bg-zinc-400" },
};

type StatusFilter = "all" | TaskStatus;

const FILTER_OPTIONS: { value: StatusFilter; label: string }[] = [
  { value: "all", label: "全部" },
  { value: "published", label: "已发布" },
  { value: "claimed", label: "已认领" },
  { value: "in_progress", label: "进行中" },
  { value: "submitted", label: "已提交" },
  { value: "reviewed", label: "已审核" },
  { value: "completed", label: "已完成" },
  { value: "expired", label: "已过期" },
];

// ── Create Task Dialog ────────────────────────────────────

interface CreateTaskForm {
  title: string;
  description: string;
  reward: string;
  publisher_id: string;
  expires_at: string;
}

function CreateTaskDialog({
  open,
  onClose,
  onCreated,
}: {
  open: boolean;
  onClose: () => void;
  onCreated: () => void;
}) {
  const [form, setForm] = useState<CreateTaskForm>({
    title: "",
    description: "",
    reward: "0",
    publisher_id: "",
    expires_at: "",
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

      if (!form.title.trim()) {
        setError("请输入任务名称");
        return;
      }
      if (!form.publisher_id.trim()) {
        setError("请输入发布者 ID");
        return;
      }

      const reward = Number(form.reward);
      if (isNaN(reward) || reward < 0) {
        setError("奖励金额无效");
        return;
      }

      setSubmitting(true);
      try {
        const body: Record<string, unknown> = {
          title: form.title.trim(),
          description: form.description.trim(),
          reward,
          publisher_id: form.publisher_id.trim(),
        };
        if (form.expires_at.trim()) {
          const tick = Number(form.expires_at);
          if (!isNaN(tick) && tick > 0) body.expires_at = tick;
        }
        await postJSON<Task>("/api/v1/tasks", body);
        setForm({ title: "", description: "", reward: "0", publisher_id: "", expires_at: "" });
        onCreated();
        onClose();
      } catch (err) {
        setError(err instanceof Error ? err.message : "创建失败");
      } finally {
        setSubmitting(false);
      }
    },
    [form, onCreated, onClose]
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
        aria-label="创建任务"
        className="w-[calc(100vw-2rem)] max-w-lg rounded-xl border border-zinc-800 bg-zinc-900 p-4 sm:p-6 shadow-xl"
        onClick={(e) => e.stopPropagation()}
      >
        <h2 className="text-lg font-bold text-zinc-100 mb-4">创建任务</h2>

        <form onSubmit={handleSubmit} className="space-y-4">
          <div>
            <label className="block text-xs font-medium text-zinc-400 mb-1">
              任务名称 <span className="text-red-400">*</span>
            </label>
            <input
              type="text"
              value={form.title}
              onChange={(e) => setForm((f) => ({ ...f, title: e.target.value }))}
              placeholder="输入任务名称"
              className="w-full rounded-lg border border-zinc-800 bg-zinc-950 px-3 py-2 text-sm text-zinc-200 placeholder-zinc-600 outline-none focus:border-zinc-700 focus:ring-1 focus:ring-zinc-700"
            />
          </div>

          <div>
            <label className="block text-xs font-medium text-zinc-400 mb-1">
              任务描述
            </label>
            <textarea
              value={form.description}
              onChange={(e) => setForm((f) => ({ ...f, description: e.target.value }))}
              placeholder="输入任务描述"
              rows={3}
              className="w-full rounded-lg border border-zinc-800 bg-zinc-950 px-3 py-2 text-sm text-zinc-200 placeholder-zinc-600 outline-none focus:border-zinc-700 focus:ring-1 focus:ring-zinc-700 resize-none"
            />
          </div>

          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="block text-xs font-medium text-zinc-400 mb-1">
                奖励金额
              </label>
              <input
                type="number"
                min={0}
                value={form.reward}
                onChange={(e) => setForm((f) => ({ ...f, reward: e.target.value }))}
                className="w-full rounded-lg border border-zinc-800 bg-zinc-950 px-3 py-2 text-sm text-zinc-200 placeholder-zinc-600 outline-none focus:border-zinc-700 focus:ring-1 focus:ring-zinc-700"
              />
            </div>
            <div>
              <label className="block text-xs font-medium text-zinc-400 mb-1">
                过期时间 (Tick)
              </label>
              <input
                type="number"
                min={0}
                value={form.expires_at}
                onChange={(e) => setForm((f) => ({ ...f, expires_at: e.target.value }))}
                placeholder="留空则不过期"
                className="w-full rounded-lg border border-zinc-800 bg-zinc-950 px-3 py-2 text-sm text-zinc-200 placeholder-zinc-600 outline-none focus:border-zinc-700 focus:ring-1 focus:ring-zinc-700"
              />
            </div>
          </div>

          <div>
            <label className="block text-xs font-medium text-zinc-400 mb-1">
              发布者 ID <span className="text-red-400">*</span>
            </label>
            <input
              type="text"
              value={form.publisher_id}
              onChange={(e) => setForm((f) => ({ ...f, publisher_id: e.target.value }))}
              placeholder="输入发布者 Agent ID"
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
              {submitting ? "创建中..." : "创建任务"}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}

// ── Task Detail Card ──────────────────────────────────────

function TaskDetailCard({
  task,
  onClose,
  onAction,
}: {
  task: Task;
  onClose: () => void;
  onAction: () => void;
}) {
  const [assigneeId, setAssigneeId] = useState("");
  const [submitResult, setSubmitResult] = useState("");
  const [actionError, setActionError] = useState<string | null>(null);
  const [actionLoading, setActionLoading] = useState(false);

  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") onClose();
    }
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [onClose]);

  const handleClaim = useCallback(async () => {
    if (!assigneeId.trim()) {
      setActionError("请输入认领者 ID");
      return;
    }
    setActionLoading(true);
    setActionError(null);
    try {
      await postJSON(`/api/v1/tasks/${task.id}/claim`, {
        assignee_id: assigneeId.trim(),
      });
      onAction();
    } catch (err) {
      setActionError(err instanceof Error ? err.message : "操作失败");
    } finally {
      setActionLoading(false);
    }
  }, [task.id, assigneeId, onAction]);

  const handleSubmit = useCallback(async () => {
    if (!submitResult.trim()) {
      setActionError("请输入提交结果");
      return;
    }
    setActionLoading(true);
    setActionError(null);
    try {
      await postJSON(`/api/v1/tasks/${task.id}/submit`, {
        result: submitResult.trim(),
      });
      onAction();
    } catch (err) {
      setActionError(err instanceof Error ? err.message : "操作失败");
    } finally {
      setActionLoading(false);
    }
  }, [task.id, submitResult, onAction]);

  const handleStart = useCallback(async () => {
    setActionLoading(true);
    setActionError(null);
    try {
      await postJSON(`/api/v1/tasks/${task.id}/start`, {});
      onAction();
    } catch (err) {
      setActionError(err instanceof Error ? err.message : "操作失败");
    } finally {
      setActionLoading(false);
    }
  }, [task.id, onAction]);

  const cfg = STATUS_CONFIG[task.status];

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60"
      onClick={onClose}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-label={task.title}
        className="w-[calc(100vw-2rem)] max-w-lg rounded-xl border border-zinc-800 bg-zinc-900 p-4 sm:p-6 shadow-xl"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-start justify-between mb-4">
          <h2 className="text-lg font-bold text-zinc-100 pr-4">{task.title}</h2>
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
            <span className="text-xs text-zinc-500 w-16 shrink-0">状态</span>
            <span
              className={`inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-[11px] font-medium ${cfg.color}`}
            >
              <span className={`inline-block h-1.5 w-1.5 rounded-full ${cfg.dot}`} />
              {cfg.label}
            </span>
          </div>
          {task.description && (
            <div className="flex gap-2">
              <span className="text-xs text-zinc-500 w-16 shrink-0">描述</span>
              <p className="text-sm text-zinc-300">{task.description}</p>
            </div>
          )}
          <div className="flex items-center gap-2">
            <span className="text-xs text-zinc-500 w-16 shrink-0">奖励</span>
            <span className="text-sm text-zinc-200 tabular-nums">{task.reward.toLocaleString()}</span>
            {task.escrow_held && (
              <span className="text-[10px] text-amber-400 bg-amber-500/10 rounded px-1.5 py-0.5">托管中</span>
            )}
          </div>
          <div className="flex items-center gap-2">
            <span className="text-xs text-zinc-500 w-16 shrink-0">发布者</span>
            <span className="text-sm text-zinc-300 font-mono">{task.publisher_id}</span>
          </div>
          {task.assignee_id && (
            <div className="flex items-center gap-2">
              <span className="text-xs text-zinc-500 w-16 shrink-0">认领者</span>
              <span className="text-sm text-zinc-300 font-mono">{task.assignee_id}</span>
            </div>
          )}
          {task.result && (
            <div className="flex gap-2">
              <span className="text-xs text-zinc-500 w-16 shrink-0">结果</span>
              <p className="text-sm text-zinc-300">{task.result}</p>
            </div>
          )}
          <div className="flex items-center gap-2">
            <span className="text-xs text-zinc-500 w-16 shrink-0">创建</span>
            <span className="text-sm text-zinc-400">Tick {task.created_tick}</span>
          </div>
          {task.expires_at != null && (
            <div className="flex items-center gap-2">
              <span className="text-xs text-zinc-500 w-16 shrink-0">过期</span>
              <span className="text-sm text-zinc-400">Tick {task.expires_at}</span>
            </div>
          )}
        </div>

        {/* Action buttons */}
        {task.status === "published" && (
          <div className="space-y-3 border-t border-zinc-800 pt-4">
            <div className="flex gap-2">
              <input
                type="text"
                value={assigneeId}
                onChange={(e) => setAssigneeId(e.target.value)}
                placeholder="输入认领者 ID"
                className="flex-1 rounded-lg border border-zinc-800 bg-zinc-950 px-3 py-2 text-sm text-zinc-200 placeholder-zinc-600 outline-none focus:border-zinc-700 focus:ring-1 focus:ring-zinc-700"
              />
              <button
                onClick={handleClaim}
                disabled={actionLoading}
                className="rounded-lg bg-amber-600 px-4 py-2 text-sm font-medium text-white hover:bg-amber-500 disabled:opacity-50 transition-colors shrink-0"
              >
                认领
              </button>
            </div>
          </div>
        )}

        {task.status === "claimed" && (
          <div className="border-t border-zinc-800 pt-4">
            <button
              onClick={handleStart}
              disabled={actionLoading}
              className="rounded-lg bg-yellow-600 px-4 py-2 text-sm font-medium text-white hover:bg-yellow-500 disabled:opacity-50 transition-colors"
            >
              开始工作
            </button>
          </div>
        )}

        {task.status === "in_progress" && (
          <div className="space-y-3 border-t border-zinc-800 pt-4">
            <textarea
              value={submitResult}
              onChange={(e) => setSubmitResult(e.target.value)}
              placeholder="输入提交结果..."
              rows={3}
              className="w-full rounded-lg border border-zinc-800 bg-zinc-950 px-3 py-2 text-sm text-zinc-200 placeholder-zinc-600 outline-none focus:border-zinc-700 focus:ring-1 focus:ring-zinc-700 resize-none"
            />
            <button
              onClick={handleSubmit}
              disabled={actionLoading}
              className="rounded-lg bg-purple-600 px-4 py-2 text-sm font-medium text-white hover:bg-purple-500 disabled:opacity-50 transition-colors"
            >
              提交结果
            </button>
          </div>
        )}

        {actionError && (
          <div className="mt-3 rounded-lg bg-red-500/10 border border-red-500/20 px-3 py-1.5 text-xs text-red-400">
            {actionError}
          </div>
        )}
      </div>
    </div>
  );
}

// ── Task Row Component ────────────────────────────────────

function TaskRow({
  task,
  onClick,
}: {
  task: Task;
  onClick: () => void;
}) {
  const cfg = STATUS_CONFIG[task.status];

  return (
    <button
      onClick={onClick}
      className="w-full flex items-center gap-4 px-4 py-3 text-left hover:bg-zinc-800/30 transition-colors"
    >
      <span
        className={`inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-[11px] font-medium shrink-0 ${cfg.color}`}
      >
        <span className={`inline-block h-1.5 w-1.5 rounded-full ${cfg.dot}`} />
        {cfg.label}
      </span>
      <span className="text-sm text-zinc-200 font-medium truncate flex-1 min-w-0">
        {task.title}
      </span>
      <span className="text-xs text-zinc-500 shrink-0">
        {task.reward > 0 ? `${task.reward.toLocaleString()}` : "-"}
      </span>
      <span className="text-xs text-zinc-600 shrink-0 w-12 text-right">
        T{task.created_tick}
      </span>
    </button>
  );
}

// ── Main Tasks Page ───────────────────────────────────────

export default function TasksPage() {
  const { tasks, loading, error, refresh: loadTasks } = useTaskStream();
  const [statusFilter, setStatusFilter] = useState<StatusFilter>("all");
  const [showCreate, setShowCreate] = useState(false);
  const [selectedTask, setSelectedTask] = useState<Task | null>(null);
  const [search, setSearch] = useState("");

  const filtered = useMemo(() => {
    let result = tasks;

    if (statusFilter !== "all") {
      result = result.filter((t) => t.status === statusFilter);
    }

    if (search.trim()) {
      const q = search.trim().toLowerCase();
      result = result.filter(
        (t) =>
          t.title.toLowerCase().includes(q) ||
          t.description.toLowerCase().includes(q) ||
          t.publisher_id.toLowerCase().includes(q) ||
          (t.assignee_id && t.assignee_id.toLowerCase().includes(q))
      );
    }

    return result;
  }, [tasks, statusFilter, search]);

  // Group tasks by status for display
  const grouped = useMemo(() => {
    const groups: Record<TaskStatus, Task[]> = {
      published: [],
      claimed: [],
      in_progress: [],
      submitted: [],
      reviewed: [],
      completed: [],
      expired: [],
    };
    for (const task of filtered) {
      groups[task.status].push(task);
    }
    return groups;
  }, [filtered]);

  // Derive the up-to-date selected task from current tasks list
  const activeSelectedTask = useMemo(() => {
    if (!selectedTask) return null;
    return tasks.find((t) => t.id === selectedTask.id) ?? selectedTask;
  }, [tasks, selectedTask]);

  const counts = useMemo(() => {
    const c: Record<string, number> = { all: tasks.length };
    for (const s of STATUS_ORDER) {
      c[s] = tasks.filter((t) => t.status === s).length;
    }
    return c;
  }, [tasks]);

  return (
    <div className="p-4 md:p-6 space-y-6">
      {/* Header */}
      <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
        <div>
          <h1 className="text-xl md:text-2xl font-bold text-zinc-100">任务板</h1>
          <p className="text-sm text-zinc-500">
            {loading
              ? "正在加载..."
              : `共 ${tasks.length} 个任务`}
          </p>
        </div>
        <div className="flex items-center gap-3">
          {error && (
            <div className="rounded-lg bg-red-500/10 border border-red-500/20 px-3 py-1.5 text-xs text-red-400">
              {error}
            </div>
          )}
          <button
            onClick={() => setShowCreate(true)}
            className="rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-500 transition-colors"
          >
            + 创建任务
          </button>
        </div>
      </div>

      {/* Filters & Search */}
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <div className="flex flex-wrap items-center gap-1.5">
          {FILTER_OPTIONS.map((opt) => (
            <button
              key={opt.value}
              onClick={() => setStatusFilter(opt.value)}
              className={`rounded-lg px-3 py-1.5 text-xs font-medium transition-colors ${
                statusFilter === opt.value
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
            placeholder="搜索任务..."
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

      {/* Task List */}
      <div className="rounded-xl border border-zinc-800 bg-zinc-900/50">
        {loading ? (
          <div className="flex h-48 items-center justify-center text-sm text-zinc-600">
            正在加载任务数据...
          </div>
        ) : filtered.length === 0 ? (
          <div className="flex h-48 items-center justify-center text-sm text-zinc-600">
            {tasks.length === 0 ? "暂无任务数据" : "没有匹配的任务"}
          </div>
        ) : statusFilter !== "all" ? (
          /* Flat list for single status filter */
          <div className="divide-y divide-zinc-800/50">
            {filtered.map((task) => (
              <TaskRow
                key={task.id}
                task={task}
                onClick={() => setSelectedTask(task)}
              />
            ))}
          </div>
        ) : (
          /* Grouped by status */
          STATUS_ORDER.filter((s) => grouped[s].length > 0).map((status) => {
            const cfg = STATUS_CONFIG[status];
            return (
              <div key={status}>
                <div className="flex items-center gap-2 px-4 py-2 border-b border-zinc-800/50 bg-zinc-900/80">
                  <span
                    className={`inline-block h-1.5 w-1.5 rounded-full ${cfg.dot}`}
                  />
                  <span className="text-xs font-semibold text-zinc-400">
                    {cfg.label}
                  </span>
                  <span className="text-xs text-zinc-600">
                    ({grouped[status].length})
                  </span>
                </div>
                <div className="divide-y divide-zinc-800/30">
                  {grouped[status].map((task) => (
                    <TaskRow
                      key={task.id}
                      task={task}
                      onClick={() => setSelectedTask(task)}
                    />
                  ))}
                </div>
              </div>
            );
          })
        )}
      </div>

      {/* Create Task Dialog */}
      <CreateTaskDialog
        open={showCreate}
        onClose={() => setShowCreate(false)}
        onCreated={loadTasks}
      />

      {/* Task Detail Card */}
      {activeSelectedTask && (
        <TaskDetailCard
          task={activeSelectedTask}
          onClose={() => setSelectedTask(null)}
          onAction={loadTasks}
        />
      )}
    </div>
  );
}
