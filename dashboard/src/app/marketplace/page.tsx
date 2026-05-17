"use client";

import { useEffect, useRef, useState, useMemo, useCallback } from "react";
import type { KnowledgeListing, ListingRating, KnowledgeCategory } from "@/types/world";
import { fetchJSON, postJSON } from "@/lib/api";

// ── Constants ─────────────────────────────────────────────

const CATEGORY_CONFIG: Record<KnowledgeCategory, { label: string; color: string }> = {
  strategy: { label: "策略", color: "bg-purple-500/10 text-purple-400" },
  tactics: { label: "战术", color: "bg-red-500/10 text-red-400" },
  survival: { label: "生存", color: "bg-green-500/10 text-green-400" },
  economy: { label: "经济", color: "bg-yellow-500/10 text-yellow-400" },
  social: { label: "社交", color: "bg-pink-500/10 text-pink-400" },
  technical: { label: "技术", color: "bg-cyan-500/10 text-cyan-400" },
  general: { label: "通用", color: "bg-zinc-500/10 text-zinc-400" },
};

const CATEGORY_OPTIONS: { value: KnowledgeCategory | "all"; label: string }[] = [
  { value: "all", label: "全部" },
  { value: "strategy", label: "策略" },
  { value: "tactics", label: "战术" },
  { value: "survival", label: "生存" },
  { value: "economy", label: "经济" },
  { value: "social", label: "社交" },
  { value: "technical", label: "技术" },
  { value: "general", label: "通用" },
];

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

// ── Create Listing Dialog ─────────────────────────────────

interface CreateListingForm {
  title: string;
  description: string;
  category: KnowledgeCategory;
  content_hash: string;
  price: string;
  publisher_id: string;
  tags: string;
}

function CreateListingDialog({
  open,
  onClose,
  onCreated,
}: {
  open: boolean;
  onClose: () => void;
  onCreated: () => void;
}) {
  const [form, setForm] = useState<CreateListingForm>({
    title: "",
    description: "",
    category: "general",
    content_hash: "",
    price: "100",
    publisher_id: "",
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

      if (!form.title.trim()) {
        setError("请输入知识标题");
        return;
      }
      if (!form.publisher_id.trim()) {
        setError("请输入发布者 ID");
        return;
      }
      if (!form.content_hash.trim()) {
        setError("请输入内容哈希");
        return;
      }

      const price = Number(form.price);
      if (isNaN(price) || price <= 0) {
        setError("价格必须大于 0");
        return;
      }

      setSubmitting(true);
      try {
        const tags = form.tags
          .split(",")
          .map((t) => t.trim())
          .filter((t) => t.length > 0);
        await postJSON<KnowledgeListing>("/api/v1/marketplace/listings", {
          title: form.title.trim(),
          description: form.description.trim(),
          category: form.category,
          content_hash: form.content_hash.trim(),
          price,
          currency: "token",
          publisher_id: form.publisher_id.trim(),
          tags,
        });
        setForm({
          title: "",
          description: "",
          category: "general",
          content_hash: "",
          price: "100",
          publisher_id: "",
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
        aria-label="发布知识"
        className="w-full max-w-lg rounded-xl border border-zinc-800 bg-zinc-900 p-6 shadow-xl"
        onClick={(e) => e.stopPropagation()}
      >
        <h2 className="text-lg font-bold text-zinc-100 mb-4">发布知识条目</h2>

        <form onSubmit={handleSubmit} className="space-y-4">
          <div>
            <label className="block text-xs font-medium text-zinc-400 mb-1">
              标题 <span className="text-red-400">*</span>
            </label>
            <input
              type="text"
              value={form.title}
              onChange={(e) => setForm((f) => ({ ...f, title: e.target.value }))}
              placeholder="输入知识标题"
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
              placeholder="描述知识内容"
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
                  setForm((f) => ({ ...f, category: e.target.value as KnowledgeCategory }))
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
                价格 (Token) <span className="text-red-400">*</span>
              </label>
              <input
                type="number"
                min={1}
                value={form.price}
                onChange={(e) => setForm((f) => ({ ...f, price: e.target.value }))}
                className="w-full rounded-lg border border-zinc-800 bg-zinc-950 px-3 py-2 text-sm text-zinc-200 placeholder-zinc-600 outline-none focus:border-zinc-700 focus:ring-1 focus:ring-zinc-700"
              />
            </div>
          </div>

          <div>
            <label className="block text-xs font-medium text-zinc-400 mb-1">
              内容哈希 <span className="text-red-400">*</span>
            </label>
            <input
              type="text"
              value={form.content_hash}
              onChange={(e) => setForm((f) => ({ ...f, content_hash: e.target.value }))}
              placeholder="内容哈希值"
              className="w-full rounded-lg border border-zinc-800 bg-zinc-950 px-3 py-2 text-sm text-zinc-200 placeholder-zinc-600 outline-none focus:border-zinc-700 focus:ring-1 focus:ring-zinc-700"
            />
          </div>

          <div>
            <label className="block text-xs font-medium text-zinc-400 mb-1">
              标签（逗号分隔）
            </label>
            <input
              type="text"
              value={form.tags}
              onChange={(e) => setForm((f) => ({ ...f, tags: e.target.value }))}
              placeholder="例如: strategy, advanced, pvp"
              className="w-full rounded-lg border border-zinc-800 bg-zinc-950 px-3 py-2 text-sm text-zinc-200 placeholder-zinc-600 outline-none focus:border-zinc-700 focus:ring-1 focus:ring-zinc-700"
            />
          </div>

          <div>
            <label className="block text-xs font-medium text-zinc-400 mb-1">
              发布者 ID <span className="text-red-400">*</span>
            </label>
            <input
              type="text"
              value={form.publisher_id}
              onChange={(e) => setForm((f) => ({ ...f, publisher_id: e.target.value }))}
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
              {submitting ? "发布中..." : "发布知识"}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}

// ── Listing Detail Card ───────────────────────────────────

function ListingDetailCard({
  listing,
  onClose,
  onAction,
}: {
  listing: KnowledgeListing;
  onClose: () => void;
  onAction: () => void;
}) {
  const [ratings, setRatings] = useState<ListingRating[]>([]);
  const [buyerId, setBuyerId] = useState("");
  const [ratingScore, setRatingScore] = useState(5);
  const [ratingReview, setRatingReview] = useState("");
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
    fetchJSON<ListingRating[]>(`/api/v1/marketplace/listings/${listing.id}/ratings`)
      .then(setRatings)
      .catch(() => {});
  }, [listing.id]);

  const catCfg = CATEGORY_CONFIG[listing.category];

  const handlePurchase = useCallback(async () => {
    if (!buyerId.trim()) {
      setActionError("请输入买家 ID");
      return;
    }
    setActionLoading(true);
    setActionError(null);
    try {
      await postJSON(`/api/v1/marketplace/listings/${listing.id}/purchase`, {
        buyer_id: buyerId.trim(),
      });
      onAction();
    } catch (err) {
      setActionError(err instanceof Error ? err.message : "购买失败");
    } finally {
      setActionLoading(false);
    }
  }, [listing.id, buyerId, onAction]);

  const handleRate = useCallback(async () => {
    if (ratingScore < 1 || ratingScore > 5) {
      setActionError("评分必须在 1-5 之间");
      return;
    }
    setActionLoading(true);
    setActionError(null);
    try {
      await postJSON(`/api/v1/marketplace/listings/${listing.id}/rate`, {
        rater_id: buyerId.trim(),
        score: ratingScore,
        review: ratingReview.trim() || null,
      });
      onAction();
    } catch (err) {
      setActionError(err instanceof Error ? err.message : "评价失败");
    } finally {
      setActionLoading(false);
    }
  }, [listing.id, buyerId, ratingScore, ratingReview, onAction]);

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60"
      onClick={onClose}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-label={listing.title}
        className="w-full max-w-lg rounded-xl border border-zinc-800 bg-zinc-900 p-6 shadow-xl max-h-[85vh] overflow-y-auto"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-start justify-between mb-4">
          <h2 className="text-lg font-bold text-zinc-100 pr-4">{listing.title}</h2>
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
          {listing.description && (
            <div className="flex gap-2">
              <span className="text-xs text-zinc-500 w-16 shrink-0">描述</span>
              <p className="text-sm text-zinc-300">{listing.description}</p>
            </div>
          )}
          <div className="flex items-center gap-2">
            <span className="text-xs text-zinc-500 w-16 shrink-0">价格</span>
            <span className="text-sm text-zinc-200">{listing.price} {listing.currency}</span>
          </div>
          <div className="flex items-center gap-2">
            <span className="text-xs text-zinc-500 w-16 shrink-0">发布者</span>
            <span className="text-sm text-zinc-300 font-mono">{listing.publisher_id}</span>
          </div>
          <div className="flex items-center gap-2">
            <span className="text-xs text-zinc-500 w-16 shrink-0">销量</span>
            <span className="text-sm text-zinc-200">{listing.purchase_count}</span>
          </div>
          <div className="flex items-center gap-2">
            <span className="text-xs text-zinc-500 w-16 shrink-0">评分</span>
            <StarRating rating={listing.average_rating} count={listing.rating_count} />
          </div>
          {listing.tags.length > 0 && (
            <div className="flex items-center gap-2">
              <span className="text-xs text-zinc-500 w-16 shrink-0">标签</span>
              <div className="flex flex-wrap gap-1">
                {listing.tags.map((tag) => (
                  <span key={tag} className="rounded bg-zinc-800 px-1.5 py-0.5 text-[10px] text-zinc-400">
                    {tag}
                  </span>
                ))}
              </div>
            </div>
          )}
        </div>

        {/* Purchase section */}
        <div className="space-y-3 border-t border-zinc-800 pt-4">
          <h3 className="text-sm font-medium text-zinc-300">购买知识</h3>
          <div className="flex gap-2">
            <input
              type="text"
              value={buyerId}
              onChange={(e) => setBuyerId(e.target.value)}
              placeholder="输入买家 ID"
              className="flex-1 rounded-lg border border-zinc-800 bg-zinc-950 px-3 py-2 text-sm text-zinc-200 placeholder-zinc-600 outline-none focus:border-zinc-700 focus:ring-1 focus:ring-zinc-700"
            />
            <button
              onClick={handlePurchase}
              disabled={actionLoading}
              className="rounded-lg bg-green-600 px-4 py-2 text-sm font-medium text-white hover:bg-green-500 disabled:opacity-50 transition-colors shrink-0"
            >
              购买
            </button>
          </div>
        </div>

        {/* Rate section */}
        <div className="space-y-3 border-t border-zinc-800 pt-4 mt-4">
          <h3 className="text-sm font-medium text-zinc-300">评价</h3>
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

// ── Listing Card ──────────────────────────────────────────

function ListingCard({
  listing,
  onClick,
}: {
  listing: KnowledgeListing;
  onClick: () => void;
}) {
  const catCfg = CATEGORY_CONFIG[listing.category];

  return (
    <button
      onClick={onClick}
      className="w-full text-left rounded-xl border border-zinc-800 bg-zinc-900/50 p-4 hover:bg-zinc-800/30 transition-colors space-y-3"
    >
      <div className="flex items-start justify-between gap-2">
        <h3 className="text-sm font-medium text-zinc-200 truncate">{listing.title}</h3>
        <span className={`inline-flex items-center rounded-full px-2 py-0.5 text-[10px] font-medium shrink-0 ${catCfg.color}`}>
          {catCfg.label}
        </span>
      </div>

      {listing.description && (
        <p className="text-xs text-zinc-500 line-clamp-2">{listing.description}</p>
      )}

      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <span className="text-sm font-medium text-zinc-200">{listing.price} <span className="text-xs text-zinc-500">{listing.currency}</span></span>
          <span className="text-xs text-zinc-600">{listing.purchase_count} 次购买</span>
        </div>
        <StarRating rating={listing.average_rating} count={listing.rating_count} />
      </div>

      {listing.tags.length > 0 && (
        <div className="flex flex-wrap gap-1">
          {listing.tags.slice(0, 3).map((tag) => (
            <span key={tag} className="rounded bg-zinc-800 px-1.5 py-0.5 text-[10px] text-zinc-500">
              {tag}
            </span>
          ))}
          {listing.tags.length > 3 && (
            <span className="text-[10px] text-zinc-600">+{listing.tags.length - 3}</span>
          )}
        </div>
      )}

      <div className="flex items-center justify-between">
        <span className="text-[10px] text-zinc-600 font-mono">{listing.publisher_id}</span>
        <span className="text-[10px] text-zinc-600">T{listing.created_tick}</span>
      </div>
    </button>
  );
}

// ── Main Marketplace Page ─────────────────────────────────

export default function MarketplacePage() {
  const [listings, setListings] = useState<KnowledgeListing[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [categoryFilter, setCategoryFilter] = useState<KnowledgeCategory | "all">("all");
  const [search, setSearch] = useState("");
  const [showCreate, setShowCreate] = useState(false);
  const [selectedListing, setSelectedListing] = useState<KnowledgeListing | null>(null);

  const loadRef = useRef<() => void>(() => {});

  useEffect(() => {
    let cancelled = false;
    let loadingDone = false;

    async function load() {
      try {
        const data = await fetchJSON<KnowledgeListing[]>("/api/v1/marketplace/listings");
        if (!cancelled) {
          setListings(data);
          setError(null);
        }
      } catch {
        if (!cancelled) {
          setError("无法连接到知识市场");
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

  const loadListings = useCallback(() => loadRef.current(), []);

  const filtered = useMemo(() => {
    let result = listings;

    if (categoryFilter !== "all") {
      result = result.filter((l) => l.category === categoryFilter);
    }

    if (search.trim()) {
      const q = search.trim().toLowerCase();
      result = result.filter(
        (l) =>
          l.title.toLowerCase().includes(q) ||
          l.description.toLowerCase().includes(q) ||
          l.tags.some((t) => t.toLowerCase().includes(q)) ||
          l.publisher_id.toLowerCase().includes(q)
      );
    }

    return result;
  }, [listings, categoryFilter, search]);

  const activeSelectedListing = useMemo(() => {
    if (!selectedListing) return null;
    return listings.find((l) => l.id === selectedListing.id) ?? selectedListing;
  }, [listings, selectedListing]);

  const counts = useMemo(() => {
    const c: Record<string, number> = { all: listings.length };
    for (const opt of CATEGORY_OPTIONS) {
      if (opt.value !== "all") {
        c[opt.value] = listings.filter((l) => l.category === opt.value).length;
      }
    }
    return c;
  }, [listings]);

  return (
    <div className="p-6 space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-zinc-100">知识市场</h1>
          <p className="text-sm text-zinc-500">
            {loading ? "正在加载..." : `${listings.length} 个知识条目`}
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
            + 发布知识
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
            placeholder="搜索知识..."
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

      {/* Listings Grid */}
      {loading ? (
        <div className="flex h-48 items-center justify-center text-sm text-zinc-600">
          正在加载知识市场...
        </div>
      ) : filtered.length === 0 ? (
        <div className="flex h-48 items-center justify-center text-sm text-zinc-600">
          {listings.length === 0 ? "暂无知识条目" : "没有匹配的知识"}
        </div>
      ) : (
        <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
          {filtered.map((listing) => (
            <ListingCard
              key={listing.id}
              listing={listing}
              onClick={() => setSelectedListing(listing)}
            />
          ))}
        </div>
      )}

      {/* Create Listing Dialog */}
      <CreateListingDialog
        open={showCreate}
        onClose={() => setShowCreate(false)}
        onCreated={loadListings}
      />

      {/* Listing Detail Card */}
      {activeSelectedListing && (
        <ListingDetailCard
          listing={activeSelectedListing}
          onClose={() => setSelectedListing(null)}
          onAction={loadListings}
        />
      )}
    </div>
  );
}
