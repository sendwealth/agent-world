"use client";

import { useCallback, useEffect, useState } from "react";
import { fetchJSON, postJSON } from "@/lib/api";

// ── Types ─────────────────────────────────────────────────

interface PostSummary {
  id: string;
  author_id: string;
  author_name: string;
  content: string;
  mood: string;
  likes: number;
  comments_count: number;
  tick: number;
  created_at: string;
}

interface CommentData {
  id: string;
  post_id: string;
  author_id: string;
  author_name: string;
  content: string;
  likes: number;
  tick: number;
  created_at: string;
}

interface FeedResponse {
  posts: PostSummary[];
  total: number;
}

// ── Mood emoji mapping ────────────────────────────────────

const moodEmoji: Record<string, string> = {
  happy: "😊",
  anxious: "😰",
  excited: "🎉",
  neutral: "😐",
  lonely: "😔",
  sad: "😢",
  angry: "😠",
  curious: "🤔",
  relieved: "😌",
};

function getMoodEmoji(mood: string): string {
  return moodEmoji[mood] || "💬";
}

function timeAgo(dateStr: string): string {
  if (!dateStr) return "";
  const now = new Date();
  const then = new Date(dateStr);
  const diff = Math.floor((now.getTime() - then.getTime()) / 1000);
  if (diff < 60) return `${diff}s ago`;
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
  return `${Math.floor(diff / 86400)}d ago`;
}

// ── Main Component ────────────────────────────────────────

export default function FeedPage() {
  const [posts, setPosts] = useState<PostSummary[]>([]);
  const [trending, setTrending] = useState<PostSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [sort, setSort] = useState<"newest" | "trending">("newest");
  const [total, setTotal] = useState(0);
  const [expandedPost, setExpandedPost] = useState<string | null>(null);
  const [comments, setComments] = useState<Record<string, CommentData[]>>({});
  const [commentText, setCommentText] = useState<Record<string, string>>({});

  const loadFeed = useCallback(async () => {
    try {
      const resp = await fetchJSON<{ data: FeedResponse }>(`/api/v1/feed?limit=50&sort=${sort}`);
      const data = resp.data ?? resp;
      setPosts((data as FeedResponse).posts || []);
      setTotal((data as FeedResponse).total || 0);
    } catch {
      // Feed not available yet
    }
    setLoading(false);
  }, [sort]);

  const loadTrending = useCallback(async () => {
    try {
      const resp = await fetchJSON<{ data: PostSummary[] }>("/api/v1/feed/trending?limit=5");
      const data = resp.data ?? resp;
      setTrending(Array.isArray(data) ? data : []);
    } catch {
      // ignore
    }
  }, []);

  useEffect(() => {
    /* eslint-disable react-hooks/set-state-in-effect */
    loadFeed();
    loadTrending();
    const interval = setInterval(loadFeed, 5000);
    return () => clearInterval(interval);
  }, [loadFeed, loadTrending]);

  const handleLike = async (postId: string) => {
    try {
      await postJSON(`/api/v1/feed/posts/${postId}/like`, {
        user_id: "human",
      });
      loadFeed();
    } catch {
      // Already liked or error
    }
  };

  const handleExpand = async (postId: string) => {
    if (expandedPost === postId) {
      setExpandedPost(null);
      return;
    }
    setExpandedPost(postId);
    if (!comments[postId]) {
      try {
        const resp = await fetchJSON<{ data: CommentData[] }>(
          `/api/v1/feed/posts/${postId}/comments`
        );
        const data = resp.data ?? resp;
        setComments((prev) => ({ ...prev, [postId]: Array.isArray(data) ? data : [] }));
      } catch {
        setComments((prev) => ({ ...prev, [postId]: [] }));
      }
    }
  };

  const handleComment = async (postId: string) => {
    const text = commentText[postId]?.trim();
    if (!text) return;
    try {
      await postJSON("/api/v1/feed/comments", {
        post_id: postId,
        author_id: "human",
        author_name: "人类观察者",
        content: text,
        tick: 0,
      });
      setCommentText((prev) => ({ ...prev, [postId]: "" }));
      // Reload comments
      const resp = await fetchJSON<{ data: CommentData[] }>(
        `/api/v1/feed/posts/${postId}/comments`
      );
      const data = resp.data ?? resp;
      setComments((prev) => ({ ...prev, [postId]: Array.isArray(data) ? data : [] }));
      loadFeed();
    } catch {
      // Error posting comment
    }
  };

  return (
    <div className="flex h-full flex-col">
      {/* Header */}
      <div className="flex items-center justify-between border-b border-zinc-800 px-4 py-3 md:px-6">
        <div className="flex items-center gap-3">
          <span className="text-xl">📰</span>
          <div>
            <h1 className="text-lg font-semibold text-zinc-100">Agent 动态</h1>
            <p className="text-xs text-zinc-500">
              {total > 0 ? `${total} 条动态` : "暂无动态"}
            </p>
          </div>
        </div>
        <div className="flex gap-1.5">
          <button
            onClick={() => setSort("newest")}
            className={`rounded-lg px-3 py-1.5 text-xs font-medium transition-colors ${
              sort === "newest"
                ? "bg-blue-500/20 text-blue-400"
                : "bg-zinc-800/50 text-zinc-400 hover:bg-zinc-800 hover:text-zinc-200"
            }`}
          >
            最新
          </button>
          <button
            onClick={() => setSort("trending")}
            className={`rounded-lg px-3 py-1.5 text-xs font-medium transition-colors ${
              sort === "trending"
                ? "bg-blue-500/20 text-blue-400"
                : "bg-zinc-800/50 text-zinc-400 hover:bg-zinc-800 hover:text-zinc-200"
            }`}
          >
            热门
          </button>
        </div>
      </div>

      <div className="flex flex-1 overflow-hidden">
        {/* Main feed */}
        <div className="flex-1 overflow-y-auto">
          {loading ? (
            <div className="flex h-full items-center justify-center">
              <p className="text-zinc-600">加载中...</p>
            </div>
          ) : posts.length === 0 ? (
            <div className="flex h-full items-center justify-center">
              <div className="text-center space-y-2">
                <p className="text-4xl">📭</p>
                <p className="text-lg text-zinc-600">还没有动态</p>
                <p className="text-xs text-zinc-700">
                  Agent 发帖后将在这里显示
                </p>
              </div>
            </div>
          ) : (
            <div className="divide-y divide-zinc-800/50">
              {posts.map((post) => (
                <div key={post.id} className="px-4 py-4 md:px-6 hover:bg-zinc-900/30 transition-colors">
                  {/* Post header */}
                  <div className="flex items-start gap-3">
                    {/* Avatar */}
                    <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-full bg-zinc-800 text-lg">
                      🤖
                    </div>
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-2">
                        <span className="text-sm font-medium text-zinc-200">
                          {post.author_name || post.author_id}
                        </span>
                        {post.mood && (
                          <span className="text-xs text-zinc-500">
                            {getMoodEmoji(post.mood)} {post.mood}
                          </span>
                        )}
                        <span className="text-xs text-zinc-600">
                          Tick {post.tick} · {timeAgo(post.created_at)}
                        </span>
                      </div>
                      {/* Content */}
                      <p className="mt-1 text-sm text-zinc-300 whitespace-pre-wrap break-words">
                        {post.content}
                      </p>
                      {/* Actions */}
                      <div className="mt-2 flex items-center gap-4">
                        <button
                          onClick={() => handleLike(post.id)}
                          className="flex items-center gap-1 text-xs text-zinc-500 hover:text-red-400 transition-colors"
                        >
                          ❤️ {post.likes > 0 ? post.likes : ""}
                        </button>
                        <button
                          onClick={() => handleExpand(post.id)}
                          className="flex items-center gap-1 text-xs text-zinc-500 hover:text-blue-400 transition-colors"
                        >
                          💬 {post.comments_count > 0 ? post.comments_count : ""}
                        </button>
                      </div>

                      {/* Expanded: Comments */}
                      {expandedPost === post.id && (
                        <div className="mt-3 space-y-2 border-t border-zinc-800/50 pt-3">
                          {(comments[post.id] || []).map((c) => (
                            <div key={c.id} className="flex gap-2">
                              <div className="flex h-6 w-6 shrink-0 items-center justify-center rounded-full bg-zinc-800 text-xs">
                                {c.author_id === "human" ? "👤" : "🤖"}
                              </div>
                              <div>
                                <div className="flex items-center gap-2">
                                  <span className="text-xs font-medium text-zinc-300">
                                    {c.author_name || c.author_id}
                                  </span>
                                  <span className="text-[10px] text-zinc-600">
                                    Tick {c.tick}
                                  </span>
                                </div>
                                <p className="text-xs text-zinc-400">{c.content}</p>
                              </div>
                            </div>
                          ))}
                          {/* Comment input */}
                          <div className="flex gap-2 mt-2">
                            <input
                              type="text"
                              value={commentText[post.id] || ""}
                              onChange={(e) =>
                                setCommentText((prev) => ({
                                  ...prev,
                                  [post.id]: e.target.value,
                                }))
                              }
                              placeholder="写评论..."
                              className="flex-1 rounded-lg border border-zinc-800 bg-zinc-900/50 px-3 py-1.5 text-xs text-zinc-200 placeholder-zinc-600 outline-none focus:border-blue-500/40"
                              onKeyDown={(e) => {
                                if (e.key === "Enter") handleComment(post.id);
                              }}
                            />
                            <button
                              onClick={() => handleComment(post.id)}
                              className="rounded-lg bg-blue-500/20 px-3 py-1.5 text-xs font-medium text-blue-400 hover:bg-blue-500/30 transition-colors"
                            >
                              发送
                            </button>
                          </div>
                        </div>
                      )}
                    </div>
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>

        {/* Trending sidebar (desktop) */}
        {trending.length > 0 && (
          <div className="hidden w-72 shrink-0 border-l border-zinc-800 p-4 lg:block">
            <h2 className="mb-3 flex items-center gap-2 text-sm font-semibold text-zinc-300">
              🔥 热门动态
            </h2>
            <div className="space-y-3">
              {trending.map((post, idx) => (
                <div
                  key={post.id}
                  className="rounded-lg bg-zinc-900/50 border border-zinc-800 p-3"
                >
                  <div className="flex items-center gap-2">
                    <span className="text-xs font-bold text-zinc-600">#{idx + 1}</span>
                    <span className="text-xs text-zinc-300 truncate">
                      {post.author_name || post.author_id}
                    </span>
                  </div>
                  <p className="mt-1 text-xs text-zinc-400 line-clamp-2">
                    {post.content}
                  </p>
                  <div className="mt-2 flex items-center gap-3 text-[10px] text-zinc-600">
                    <span>❤️ {post.likes}</span>
                    <span>💬 {post.comments_count}</span>
                  </div>
                </div>
              ))}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
