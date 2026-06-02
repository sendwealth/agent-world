"use client";

import { useEffect, useRef, useState, useMemo, useCallback } from "react";
import type {
  ToolListing,
  ToolRating,
  ToolCategory,
  ToolListingMode,
} from "@/types/world";
import { fetchJSON, postJSON } from "@/lib/api";

// ── Constants ─────────────────────────────────────────────

const CATEGORY_CONFIG: Record<ToolCategory, { label: string; color: string }> = {
  computation: { label: "计算", color: "bg-blue-500/10 text-blue-400" },
  communication: { label: "通信", color: "bg-indigo-500/10 text-indigo-400" },
  analysis: { label: "分析", color: "bg-purple-500/10 text-purple-400" },
  storage: { label: "存储", color: "bg-green-500/10 text-green-400" },
  automation: { label: "自动化", color: "bg-yellow-500/10 text-yellow-400" },
  defense: { label: "防御", color: "bg-red-500/10 text-red-400" },
  production: { label: "生产", color: "bg-orange-500/10 text-orange-400" },
  utility: { label: "工具", color: "bg-zinc-500/10 text-zinc-400" },
};

const CATEGORY_OPTIONS: { value: ToolCategory | "all"; label: string }[] = [
  { value: "all", label: "全部" },
  { value: "computation", label: "计算" },
  { value: "communication", label: "通信" },
  { value: "analysis", label: "分析" },
  { value: "storage", label: "存储" },
  { value: "automation", label: "自动化" },
  { value: "defense", label: "防御" },
  { value: "production", label: "生产" },
  { value: "utility", label: "工具" },
];

const MODE_LABELS: Record<ToolListingMode, string> = {
  sale: "出售",
  rent: "出租",
  both: "出售/出租",
};

// ── Star Rating Display ───────────────────────────────────

function StarRating({ rating, count }: { rating: number; count: number }) {
  return (
    <div className="flex items-center gap-1">
      <span className="text-xs text-amber-400">
        {count > 0 ? rating.toFixed(1) : "-"}
      </span>
      {count > 0 && (
        <span className="text-[10px] text-zinc-600">({count})</span>
      )}
    </div>
  );
}

// ── Create Tool Dialog ────────────────────────────────────

interface CreateToolForm {
  name: string;
  description: string;
  category: ToolCategory;
  owner_id: string;
  purchase_price: string;
  rental_price_per_tick: string;
  listing_mode: ToolListingMode;
  tags: string;
}

function CreateToolDialog({
  open,
  onClose,
  onCreated,
}: {
  open: boolean;
  onClose: () => void;
  onCreated: () => void;
}) {
  const [form, setForm] = useState<CreateToolForm>({
    name: "",
    description: "",
    category: "utility",
    owner_id: "",
    purchase_price: "100",
    rental_price_per_tick: "10",
    listing_mode: "both",
    tags: "",
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

      if (!form.name.trim()) {
        setError("请输入工具名称");
        return;
      }
      if (!form.owner_id.trim()) {
        setError("请输入所有者 ID");
        return;
      }

      const purchasePrice = Number(form.purchase_price);
      const rentalPrice = Number(form.rental_price_per_tick);

      if (form.listing_mode !== "rent" && (isNaN(purchasePrice) || purchasePrice <= 0)) {
        setError("出售价格必须大于 0");
        return;
      }
      if (form.listing_mode !== "sale" && (isNaN(rentalPrice) || rentalPrice <= 0)) {
        setError("出租价格必须大于 0");
        return;
      }

      setSubmitting(true);
      try {
        const tags = form.tags
          .split(",")
          .map((t) => t.trim())
          .filter((t) => t.length > 0);
        await postJSON("/api/v1/tool-marketplace/tools", {
          name: form.name.trim(),
          description: form.description.trim(),
          category: form.category,
          owner_id: form.owner_id.trim(),
          purchase_price: form.listing_mode === "rent" ? 0 : purchasePrice,
          rental_price_per_tick: form.listing_mode === "sale" ? 0 : rentalPrice,
          currency: "token",
          listing_mode: form.listing_mode,
          tags,
        });
        setForm({
          name: "",
          description: "",
          category: "utility",
          owner_id: "",
          purchase_price: "100",
          rental_price_per_tick: "10",
          listing_mode: "both",
          tags: "",
        });
        onCreated();
        onClose();
      } catch (err) {
        setError(err instanceof Error ? err.message : "发布失败");
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
        aria-label="发布工具"
        className="w-full max-w-lg rounded-xl border border-zinc-800 bg-zinc-900 p-6 shadow-xl"
        onClick={(e) => e.stopPropagation()}
      >
        <h2 className="text-lg font-bold text-zinc-100 mb-4">发布工具到市场</h2>

        <form onSubmit={handleSubmit} className="space-y-4">
          <div>
            <label className="block text-xs font-medium text-zinc-400 mb-1">
              工具名称 <span className="text-red-400">*</span>
            </label>
            <input
              type="text"
              value={form.name}
              onChange={(e) => setForm((f) => ({ ...f, name: e.target.value }))}
              placeholder="输入工具名称"
              className="w-full rounded-lg border border-zinc-800 bg-zinc-950 px-3 py-2 text-sm text-zinc-200 placeholder-zinc-600 outline-none focus:border-zinc-700 focus:ring-1 focus:ring-zinc-700"
            />
          </div>

          <div>
            <label className="block text-xs font-medium text-zinc-400 mb-1">
              描述
            </label>
            <textarea
              value={form.description}
              onChange={(e) => setForm((f) => ({ ...f, description: e.target.value }))}
              placeholder="描述工具功能和用途"
              rows={3}
              className="w-full rounded-lg border border-zinc-800 bg-zinc-950 px-3 py-2 text-sm text-zinc-200 placeholder-zinc-600 outline-none focus:border-zinc-700 focus:ring-1 focus:ring-zinc-700 resize-none"
            />
          </div>

          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="block text-xs font-medium text-zinc-400 mb-1">
                分类
              </label>
              <select
                value={form.category}
                onChange={(e) =>
                  setForm((f) => ({ ...f, category: e.target.value as ToolCategory }))
                }
                className="w-full rounded-lg border border-zinc-800 bg-zinc-950 px-3 py-2 text-sm text-zinc-200 outline-none focus:border-zinc-700 focus:ring-1 focus:ring-zinc-700"
              >
                {CATEGORY_OPTIONS.filter((o) => o.value !== "all").map((opt) => (
                  <option key={opt.value} value={opt.value}>
                    {opt.label}
                  </option>
                ))}
              </select>
            </div>
            <div>
              <label className="block text-xs font-medium text-zinc-400 mb-1">
                出售模式
              </label>
              <select
                value={form.listing_mode}
                onChange={(e) =>
                  setForm((f) => ({ ...f, listing_mode: e.target.value as ToolListingMode }))
                }
                className="w-full rounded-lg border border-zinc-800 bg-zinc-950 px-3 py-2 text-sm text-zinc-200 outline-none focus:border-zinc-700 focus:ring-1 focus:ring-zinc-700"
              >
                <option value="sale">仅出售</option>
                <option value="rent">仅出租</option>
                <option value="both">出售 + 出租</option>
              </select>
            </div>
          </div>

          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="block text-xs font-medium text-zinc-400 mb-1">
                出售价格 (Token)
              </label>
              <input
                type="number"
                min={0}
                value={form.purchase_price}
                onChange={(e) => setForm((f) => ({ ...f, purchase_price: e.target.value }))}
                className="w-full rounded-lg border border-zinc-800 bg-zinc-950 px-3 py-2 text-sm text-zinc-200 placeholder-zinc-600 outline-none focus:border-zinc-700 focus:ring-1 focus:ring-zinc-700"
              />
            </div>
            <div>
              <label className="block text-xs font-medium text-zinc-400 mb-1">
                出租价格/Tick
              </label>
              <input
                type="number"
                min={0}
                value={form.rental_price_per_tick}
                onChange={(e) => setForm((f) => ({ ...f, rental_price_per_tick: e.target.value }))}
                className="w-full rounded-lg border border-zinc-800 bg-zinc-950 px-3 py-2 text-sm text-zinc-200 placeholder-zinc-600 outline-none focus:border-zinc-700 focus:ring-1 focus:ring-zinc-700"
              />
            </div>
          </div>

          <div>
            <label className="block text-xs font-medium text-zinc-400 mb-1">
              标签（逗号分隔）
            </label>
            <input
              type="text"
              value={form.tags}
              onChange={(e) => setForm((f) => ({ ...f, tags: e.target.value }))}
              placeholder="例如: compute, ml, data"
              className="w-full rounded-lg border border-zinc-800 bg-zinc-950 px-3 py-2 text-sm text-zinc-200 placeholder-zinc-600 outline-none focus:border-zinc-700 focus:ring-1 focus:ring-zinc-700"
            />
          </div>

          <div>
            <label className="block text-xs font-medium text-zinc-400 mb-1">
              所有者 ID <span className="text-red-400">*</span>
            </label>
            <input
              type="text"
              value={form.owner_id}
              onChange={(e) => setForm((f) => ({ ...f, owner_id: e.target.value }))}
              placeholder="输入 Agent ID"
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
              {submitting ? "发布中..." : "发布工具"}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}

// ── Tool Detail Card ──────────────────────────────────────

function ToolDetailCard({
  tool,
  onClose,
  onAction,
}: {
  tool: ToolListing;
  onClose: () => void;
  onAction: () => void;
}) {
  const [ratings, setRatings] = useState<ToolRating[]>([]);
  const [agentId, setAgentId] = useState("");
  const [ratingScore, setRatingScore] = useState(5);
  const [ratingReview, setRatingReview] = useState("");
  const [rentDuration, setRentDuration] = useState("10");
  const [actionError, setActionError] = useState<string | null>(null);
  const [actionLoading, setActionLoading] = useState(false);

  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") onClose();
    }
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [onClose]);

  useEffect(() => {
    fetchJSON<ToolRating[]>(`/api/v1/tool-marketplace/tools/${tool.id}/ratings`)
      .then((res: unknown) => {
        const data = (res as { data?: ToolRating[] })?.data ?? res;
        setRatings(data as ToolRating[]);
      })
      .catch(() => {});
  }, [tool.id]);

  const catCfg = CATEGORY_CONFIG[tool.category];
  const avgRating = tool.rating_count > 0 ? tool.rating_sum / tool.rating_count : 0;

  const handlePurchase = useCallback(async () => {
    if (!agentId.trim()) {
      setActionError("请输入 Agent ID");
      return;
    }
    setActionLoading(true);
    setActionError(null);
    try {
      await postJSON(`/api/v1/tool-marketplace/tools/${tool.id}/purchase`, {
        buyer_id: agentId.trim(),
      });
      onAction();
    } catch (err) {
      setActionError(err instanceof Error ? err.message : "购买失败");
    } finally {
      setActionLoading(false);
    }
  }, [tool.id, agentId, onAction]);

  const handleRent = useCallback(async () => {
    if (!agentId.trim()) {
      setActionError("请输入 Agent ID");
      return;
    }
    const dur = Number(rentDuration);
    if (isNaN(dur) || dur <= 0) {
      setActionError("租期必须大于 0 tick");
      return;
    }
    setActionLoading(true);
    setActionError(null);
    try {
      await postJSON(`/api/v1/tool-marketplace/tools/${tool.id}/rent`, {
        renter_id: agentId.trim(),
        duration_ticks: dur,
      });
      onAction();
    } catch (err) {
      setActionError(err instanceof Error ? err.message : "租用失败");
    } finally {
      setActionLoading(false);
    }
  }, [tool.id, agentId, rentDuration, onAction]);

  const handleRate = useCallback(async () => {
    if (!agentId.trim()) {
      setActionError("请输入 Agent ID");
      return;
    }
    if (ratingScore < 1 || ratingScore > 5) {
      setActionError("评分必须在 1-5 之间");
      return;
    }
    setActionLoading(true);
    setActionError(null);
    try {
      await postJSON(`/api/v1/tool-marketplace/tools/${tool.id}/rate`, {
        rater_id: agentId.trim(),
        score: ratingScore,
        review: ratingReview.trim() || null,
      });
      onAction();
    } catch (err) {
      setActionError(err instanceof Error ? err.message : "评价失败");
    } finally {
      setActionLoading(false);
    }
  }, [tool.id, agentId, ratingScore, ratingReview, onAction]);

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60"
      onClick={onClose}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-label={tool.name}
        className="w-full max-w-lg rounded-xl border border-zinc-800 bg-zinc-900 p-6 shadow-xl max-h-[85vh] overflow-y-auto"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-start justify-between mb-4">
          <h2 className="text-lg font-bold text-zinc-100 pr-4">{tool.name}</h2>
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
            <span className="text-xs text-zinc-500 w-16 shrink-0">分类</span>
            <span className={`inline-flex items-center rounded-full px-2 py-0.5 text-[11px] font-medium ${catCfg.color}`}>
              {catCfg.label}
            </span>
          </div>
          <div className="flex items-center gap-2">
            <span className="text-xs text-zinc-500 w-16 shrink-0">模式</span>
            <span className="text-sm text-zinc-200">{MODE_LABELS[tool.listing_mode]}</span>
          </div>
          {tool.description && (
            <div className="flex gap-2">
              <span className="text-xs text-zinc-500 w-16 shrink-0">描述</span>
              <p className="text-sm text-zinc-300">{tool.description}</p>
            </div>
          )}
          {(tool.listing_mode === "sale" || tool.listing_mode === "both") && (
            <div className="flex items-center gap-2">
              <span className="text-xs text-zinc-500 w-16 shrink-0">售价</span>
              <span className="text-sm text-zinc-200">{tool.purchase_price} {tool.currency}</span>
            </div>
          )}
          {(tool.listing_mode === "rent" || tool.listing_mode === "both") && (
            <div className="flex items-center gap-2">
              <span className="text-xs text-zinc-500 w-16 shrink-0">租金</span>
              <span className="text-sm text-zinc-200">{tool.rental_price_per_tick} {tool.currency}/tick</span>
            </div>
          )}
          <div className="flex items-center gap-2">
            <span className="text-xs text-zinc-500 w-16 shrink-0">所有者</span>
            <span className="text-sm text-zinc-300 font-mono">{tool.owner_id}</span>
          </div>
          <div className="flex items-center gap-2">
            <span className="text-xs text-zinc-500 w-16 shrink-0">购买</span>
            <span className="text-sm text-zinc-200">{tool.total_purchases} 次</span>
          </div>
          <div className="flex items-center gap-2">
            <span className="text-xs text-zinc-500 w-16 shrink-0">出租</span>
            <span className="text-sm text-zinc-200">{tool.total_rentals} 次</span>
          </div>
          <div className="flex items-center gap-2">
            <span className="text-xs text-zinc-500 w-16 shrink-0">评分</span>
            <StarRating rating={avgRating} count={tool.rating_count} />
          </div>
          {tool.tags.length > 0 && (
            <div className="flex items-center gap-2">
              <span className="text-xs text-zinc-500 w-16 shrink-0">标签</span>
              <div className="flex flex-wrap gap-1">
                {tool.tags.map((tag) => (
                  <span key={tag} className="rounded bg-zinc-800 px-1.5 py-0.5 text-[10px] text-zinc-400">
                    {tag}
                  </span>
                ))}
              </div>
            </div>
          )}
        </div>

        {/* Agent ID input */}
        <div className="border-t border-zinc-800 pt-4">
          <label className="block text-xs font-medium text-zinc-400 mb-1">
            操作者 Agent ID
          </label>
          <input
            type="text"
            value={agentId}
            onChange={(e) => setAgentId(e.target.value)}
            placeholder="输入你的 Agent ID"
            className="w-full rounded-lg border border-zinc-800 bg-zinc-950 px-3 py-2 text-sm text-zinc-200 placeholder-zinc-600 outline-none focus:border-zinc-700 focus:ring-1 focus:ring-zinc-700"
          />
        </div>

        {/* Purchase / Rent section */}
        <div className="space-y-3 border-t border-zinc-800 pt-4 mt-4">
          <div className="flex gap-2">
            {(tool.listing_mode === "sale" || tool.listing_mode === "both") && (
              <button
                onClick={handlePurchase}
                disabled={actionLoading}
                className="flex-1 rounded-lg bg-green-600 px-4 py-2 text-sm font-medium text-white hover:bg-green-500 disabled:opacity-50 transition-colors"
              >
                购买 ({tool.purchase_price} Token)
              </button>
            )}
            {(tool.listing_mode === "rent" || tool.listing_mode === "both") && (
              <div className="flex flex-1 gap-2">
                <input
                  type="number"
                  min={1}
                  value={rentDuration}
                  onChange={(e) => setRentDuration(e.target.value)}
                  placeholder="Tick 数"
                  className="w-24 rounded-lg border border-zinc-800 bg-zinc-950 px-3 py-2 text-sm text-zinc-200 placeholder-zinc-600 outline-none focus:border-zinc-700 focus:ring-1 focus:ring-zinc-700"
                />
                <button
                  onClick={handleRent}
                  disabled={actionLoading}
                  className="flex-1 rounded-lg bg-cyan-600 px-4 py-2 text-sm font-medium text-white hover:bg-cyan-500 disabled:opacity-50 transition-colors"
                >
                  租用 ({tool.rental_price_per_tick}/tick)
                </button>
              </div>
            )}
          </div>
        </div>

        {/* Rate section */}
        <div className="space-y-3 border-t border-zinc-800 pt-4 mt-4">
          <h3 className="text-sm font-medium text-zinc-300">评价工具</h3>
          <div className="flex items-center gap-2">
            <span className="text-xs text-zinc-500">评分</span>
            <select
              value={ratingScore}
              onChange={(e) => setRatingScore(Number(e.target.value))}
              className="rounded-lg border border-zinc-800 bg-zinc-950 px-2 py-1.5 text-sm text-zinc-200 outline-none focus:border-zinc-700"
            >
              {[1, 2, 3, 4, 5].map((n) => (
                <option key={n} value={n}>
                  {n} 分
                </option>
              ))}
            </select>
          </div>
          <textarea
            value={ratingReview}
            onChange={(e) => setRatingReview(e.target.value)}
            placeholder="评价内容（可选）"
            rows={2}
            className="w-full rounded-lg border border-zinc-800 bg-zinc-950 px-3 py-2 text-sm text-zinc-200 placeholder-zinc-600 outline-none focus:border-zinc-700 focus:ring-1 focus:ring-zinc-700 resize-none"
          />
          <button
            onClick={handleRate}
            disabled={actionLoading}
            className="rounded-lg bg-amber-600 px-4 py-2 text-sm font-medium text-white hover:bg-amber-500 disabled:opacity-50 transition-colors"
          >
            提交评价
          </button>
        </div>

        {/* Ratings list */}
        {ratings.length > 0 && (
          <div className="space-y-2 border-t border-zinc-800 pt-4 mt-4">
            <h3 className="text-sm font-medium text-zinc-300">评价列表</h3>
            {ratings.map((r) => (
              <div key={r.id} className="rounded-lg bg-zinc-800/30 px-3 py-2">
                <div className="flex items-center gap-2 mb-1">
                  <span className="text-xs font-mono text-zinc-400">{r.rater_id}</span>
                  <span className="text-xs text-amber-400">{r.score}/5</span>
                </div>
                {r.review && <p className="text-xs text-zinc-400">{r.review}</p>}
              </div>
            ))}
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

// ── Tool Card ─────────────────────────────────────────────

function ToolCard({
  tool,
  onClick,
}: {
  tool: ToolListing;
  onClick: () => void;
}) {
  const catCfg = CATEGORY_CONFIG[tool.category];
  const avgRating = tool.rating_count > 0 ? tool.rating_sum / tool.rating_count : 0;

  return (
    <button
      onClick={onClick}
      className="w-full text-left rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 hover:bg-zinc-800/30 transition-colors space-y-3"
    >
      <div className="flex items-start justify-between gap-2">
        <h3 className="text-sm font-medium text-zinc-200 truncate">{tool.name}</h3>
        <span className={`inline-flex items-center rounded-full px-2 py-0.5 text-[10px] font-medium shrink-0 ${catCfg.color}`}>
          {catCfg.label}
        </span>
      </div>

      {tool.description && (
        <p className="text-xs text-zinc-500 line-clamp-2">{tool.description}</p>
      )}

      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          {(tool.listing_mode === "sale" || tool.listing_mode === "both") && (
            <span className="text-sm font-medium text-zinc-200">{tool.purchase_price} <span className="text-xs text-zinc-500">Token</span></span>
          )}
          {(tool.listing_mode === "rent" || tool.listing_mode === "both") && (
            <span className="text-xs text-cyan-400">{tool.rental_price_per_tick}/tick</span>
          )}
        </div>
        <span className="inline-flex items-center rounded-full px-1.5 py-0.5 text-[10px] font-medium bg-zinc-800 text-zinc-400">
          {MODE_LABELS[tool.listing_mode]}
        </span>
      </div>

      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <span className="text-xs text-zinc-600">{tool.total_purchases} 购买</span>
          <span className="text-xs text-zinc-600">{tool.total_rentals} 租用</span>
        </div>
        <StarRating rating={avgRating} count={tool.rating_count} />
      </div>

      {tool.tags.length > 0 && (
        <div className="flex flex-wrap gap-1">
          {tool.tags.slice(0, 3).map((tag) => (
            <span key={tag} className="rounded bg-zinc-800 px-1.5 py-0.5 text-[10px] text-zinc-500">
              {tag}
            </span>
          ))}
          {tool.tags.length > 3 && (
            <span className="text-[10px] text-zinc-600">+{tool.tags.length - 3}</span>
          )}
        </div>
      )}

      <div className="flex items-center justify-between">
        <span className="text-[10px] text-zinc-600 font-mono">{tool.owner_id}</span>
        <span className="text-[10px] text-zinc-600">T{tool.created_tick}</span>
      </div>
    </button>
  );
}

// ── Main Tool Marketplace Page ────────────────────────────

export default function ToolMarketplacePage() {
  const [tools, setTools] = useState<ToolListing[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [categoryFilter, setCategoryFilter] = useState<ToolCategory | "all">("all");
  const [search, setSearch] = useState("");
  const [showCreate, setShowCreate] = useState(false);
  const [selectedTool, setSelectedTool] = useState<ToolListing | null>(null);

  const loadRef = useRef<() => void>(() => {});

  useEffect(() => {
    let cancelled = false;
    let loadingDone = false;

    async function load() {
      try {
        const res = await fetchJSON<unknown>("/api/v1/tool-marketplace/tools");
        const data = (res as { data?: ToolListing[] })?.data ?? res;
        if (!cancelled) {
          setTools(data as ToolListing[]);
          setError(null);
        }
      } catch {
        if (!cancelled) {
          setError("无法连接到工具市场");
        }
      } finally {
        if (!cancelled && !loadingDone) {
          loadingDone = true;
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

  const loadTools = useCallback(() => loadRef.current(), []);

  const filtered = useMemo(() => {
    let result = tools;

    if (categoryFilter !== "all") {
      result = result.filter((t) => t.category === categoryFilter);
    }

    if (search.trim()) {
      const q = search.trim().toLowerCase();
      result = result.filter(
        (t) =>
          t.name.toLowerCase().includes(q) ||
          t.description.toLowerCase().includes(q) ||
          t.tags.some((tag) => tag.toLowerCase().includes(q)) ||
          t.owner_id.toLowerCase().includes(q)
      );
    }

    return result;
  }, [tools, categoryFilter, search]);

  const activeSelectedTool = useMemo(() => {
    if (!selectedTool) return null;
    return tools.find((t) => t.id === selectedTool.id) ?? selectedTool;
  }, [tools, selectedTool]);

  const counts = useMemo(() => {
    const c: Record<string, number> = { all: tools.length };
    for (const opt of CATEGORY_OPTIONS) {
      if (opt.value !== "all") {
        c[opt.value] = tools.filter((t) => t.category === opt.value).length;
      }
    }
    return c;
  }, [tools]);

  return (
    <div className="p-6 space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-zinc-100">工具市场</h1>
          <p className="text-sm text-zinc-500">
            {loading ? "正在加载..." : `${tools.length} 个工具`}
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
            + 发布工具
          </button>
        </div>
      </div>

      {/* Filters & Search */}
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <div className="flex flex-wrap items-center gap-1.5">
          {CATEGORY_OPTIONS.map((opt) => (
            <button
              key={opt.value}
              onClick={() => setCategoryFilter(opt.value)}
              className={`rounded-lg px-3 py-1.5 text-xs font-medium transition-colors ${
                categoryFilter === opt.value
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
            placeholder="搜索工具..."
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

      {/* Tools Grid */}
      {loading ? (
        <div className="flex h-48 items-center justify-center text-sm text-zinc-600">
          正在加载工具市场...
        </div>
      ) : filtered.length === 0 ? (
        <div className="flex h-48 items-center justify-center text-sm text-zinc-600">
          {tools.length === 0 ? "暂无工具" : "没有匹配的工具"}
        </div>
      ) : (
        <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
          {filtered.map((tool) => (
            <ToolCard
              key={tool.id}
              tool={tool}
              onClick={() => setSelectedTool(tool)}
            />
          ))}
        </div>
      )}

      {/* Create Tool Dialog */}
      <CreateToolDialog
        open={showCreate}
        onClose={() => setShowCreate(false)}
        onCreated={loadTools}
      />

      {/* Tool Detail Dialog */}
      {activeSelectedTool && (
        <ToolDetailCard
          tool={activeSelectedTool}
          onClose={() => setSelectedTool(null)}
          onAction={loadTools}
        />
      )}
    </div>
  );
}
