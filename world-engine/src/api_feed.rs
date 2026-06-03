//! Feed / Social Content API — `/api/v1/feed/*`.
//!
//! Provides post creation, feed browsing, commenting, liking, and trending.
//! Data is stored in-memory on AppState (via SharedFeedStore) and broadcast
//! to the EventBus for real-time SSE consumers.

use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::api::{api_err, api_ok, AppState};
use crate::world::event::WorldEvent;

// ── Data Types ────────────────────────────────────────────

/// A single post in the social feed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Post {
    pub id: String,
    pub author_id: String,
    pub author_name: String,
    pub content: String,
    #[serde(default)]
    pub mood: String,
    #[serde(default)]
    pub likes: u64,
    #[serde(default)]
    pub comments_count: u64,
    pub tick: u64,
    pub created_at: DateTime<Utc>,
}

/// A comment on a post.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Comment {
    pub id: String,
    pub post_id: String,
    pub author_id: String,
    pub author_name: String,
    pub content: String,
    #[serde(default)]
    pub likes: u64,
    pub tick: u64,
    pub created_at: DateTime<Utc>,
}

/// A like on a post or comment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Like {
    pub user_id: String,
    pub target_id: String,
    pub target_type: String,
}

// ── Feed Store ────────────────────────────────────────────

#[derive(Debug, Default)]
pub struct FeedStore {
    posts: Vec<Post>,
    comments: Vec<Comment>,
    likes: Vec<Like>,
}

impl FeedStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create_post(
        &mut self,
        author_id: String,
        author_name: String,
        content: String,
        mood: String,
        tick: u64,
    ) -> Post {
        let post = Post {
            id: Uuid::new_v4().to_string(),
            author_id,
            author_name,
            content,
            mood,
            likes: 0,
            comments_count: 0,
            tick,
            created_at: Utc::now(),
        };
        self.posts.push(post.clone());
        post
    }

    pub fn list_posts(
        &self,
        author_id: Option<&str>,
        limit: usize,
        offset: usize,
        sort: &str,
    ) -> Vec<&Post> {
        let mut filtered: Vec<&Post> = if let Some(aid) = author_id {
            self.posts.iter().filter(|p| p.author_id == aid).collect()
        } else {
            self.posts.iter().collect()
        };
        match sort {
            "trending" => filtered.sort_by(|a, b| b.likes.cmp(&a.likes).then(b.tick.cmp(&a.tick))),
            "oldest" => filtered.sort_by_key(|a| a.tick),
            _ => filtered.sort_by_key(|b| std::cmp::Reverse(b.tick)),
        }
        filtered.into_iter().skip(offset).take(limit).collect()
    }

    pub fn get_post(&self, id: &str) -> Option<&Post> {
        self.posts.iter().find(|p| p.id == id)
    }

    pub fn like_post(&mut self, user_id: &str, post_id: &str) -> bool {
        if self.likes.iter().any(|l| l.user_id == user_id && l.target_id == post_id) {
            return false;
        }
        if let Some(post) = self.posts.iter_mut().find(|p| p.id == post_id) {
            post.likes += 1;
            self.likes.push(Like {
                user_id: user_id.to_string(),
                target_id: post_id.to_string(),
                target_type: "post".to_string(),
            });
            return true;
        }
        false
    }

    pub fn unlike_post(&mut self, user_id: &str, post_id: &str) -> bool {
        let idx = self.likes.iter().position(|l| l.user_id == user_id && l.target_id == post_id);
        if let Some(i) = idx {
            self.likes.swap_remove(i);
            if let Some(post) = self.posts.iter_mut().find(|p| p.id == post_id) {
                post.likes = post.likes.saturating_sub(1);
            }
            return true;
        }
        false
    }

    pub fn add_comment(
        &mut self,
        post_id: &str,
        author_id: String,
        author_name: String,
        content: String,
        tick: u64,
    ) -> Option<Comment> {
        if !self.posts.iter().any(|p| p.id == post_id) {
            return None;
        }
        let comment = Comment {
            id: Uuid::new_v4().to_string(),
            post_id: post_id.to_string(),
            author_id,
            author_name,
            content,
            likes: 0,
            tick,
            created_at: Utc::now(),
        };
        if let Some(post) = self.posts.iter_mut().find(|p| p.id == post_id) {
            post.comments_count += 1;
        }
        self.comments.push(comment.clone());
        Some(comment)
    }

    pub fn list_comments(&self, post_id: &str) -> Vec<&Comment> {
        let mut cs: Vec<&Comment> = self.comments.iter().filter(|c| c.post_id == post_id).collect();
        cs.sort_by_key(|a| a.tick);
        cs
    }

    pub fn trending(&self, limit: usize) -> Vec<&Post> {
        let mut posts: Vec<&Post> = self.posts.iter().collect();
        posts.sort_by(|a, b| b.likes.cmp(&a.likes).then(b.tick.cmp(&a.tick)));
        posts.into_iter().take(limit).collect()
    }

    pub fn like_comment(&mut self, user_id: &str, comment_id: &str) -> bool {
        if self.likes.iter().any(|l| l.user_id == user_id && l.target_id == comment_id) {
            return false;
        }
        if let Some(comment) = self.comments.iter_mut().find(|c| c.id == comment_id) {
            comment.likes += 1;
            self.likes.push(Like {
                user_id: user_id.to_string(),
                target_id: comment_id.to_string(),
                target_type: "comment".to_string(),
            });
            return true;
        }
        false
    }

    pub fn post_count(&self, author_id: Option<&str>) -> usize {
        if let Some(aid) = author_id {
            self.posts.iter().filter(|p| p.author_id == aid).count()
        } else {
            self.posts.len()
        }
    }
}

pub type SharedFeedStore = Arc<Mutex<FeedStore>>;

// ── Request / Response Types ──────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreatePostRequest {
    pub author_id: String,
    #[serde(default)]
    pub author_name: String,
    pub content: String,
    #[serde(default)]
    pub mood: String,
    #[serde(default)]
    pub tick: u64,
}

#[derive(Debug, Deserialize)]
pub struct CreateCommentRequest {
    pub post_id: String,
    pub author_id: String,
    #[serde(default)]
    pub author_name: String,
    pub content: String,
    #[serde(default)]
    pub tick: u64,
}

#[derive(Debug, Deserialize)]
pub struct LikeRequest {
    pub user_id: String,
}

#[derive(Debug, Deserialize)]
pub struct UnlikeRequest {
    pub user_id: String,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct FeedQuery {
    pub author_id: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    pub sort: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct FeedResponse {
    pub posts: Vec<PostSummary>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct PostSummary {
    pub id: String,
    pub author_id: String,
    pub author_name: String,
    pub content: String,
    pub mood: String,
    pub likes: u64,
    pub comments_count: u64,
    pub tick: u64,
    pub created_at: DateTime<Utc>,
}

impl From<&Post> for PostSummary {
    fn from(p: &Post) -> Self {
        PostSummary {
            id: p.id.clone(),
            author_id: p.author_id.clone(),
            author_name: p.author_name.clone(),
            content: p.content.clone(),
            mood: p.mood.clone(),
            likes: p.likes,
            comments_count: p.comments_count,
            tick: p.tick,
            created_at: p.created_at,
        }
    }
}

// ── Router ────────────────────────────────────────────────

pub fn feed_routes() -> Router<AppState> {
    Router::new()
        .route("/feed/posts", post(create_post))
        .route("/feed", get(list_feed))
        .route("/feed/trending", get(get_trending))
        .route("/feed/posts/:id", get(get_post_detail))
        .route("/feed/posts/:id/like", post(like_post_handler))
        .route("/feed/posts/:id/unlike", post(unlike_post_handler))
        .route("/feed/comments", post(create_comment))
        .route("/feed/comments/:id/like", post(like_comment_handler))
        .route("/feed/posts/:id/comments", get(list_comments_handler))
}

// ── Handlers ──────────────────────────────────────────────

async fn create_post(
    State(state): State<AppState>,
    Json(body): Json<CreatePostRequest>,
) -> impl IntoResponse {
    if body.content.is_empty() {
        return api_err(StatusCode::BAD_REQUEST, "content is required");
    }
    if body.author_id.is_empty() {
        return api_err(StatusCode::BAD_REQUEST, "author_id is required");
    }
    let store = match &state.feed_store {
        Some(s) => s,
        None => return api_err(StatusCode::SERVICE_UNAVAILABLE, "feed store not configured"),
    };
    let mut store = store.lock().await;
    let post = store.create_post(
        body.author_id.clone(),
        body.author_name.clone(),
        body.content.clone(),
        body.mood.clone(),
        body.tick,
    );
    let summary = PostSummary::from(&post);
    drop(store); // release lock before publishing event

    state.event_bus.publish(WorldEvent::FeedPostCreated {
        post_id: post.id,
        author_id: body.author_id,
        author_name: body.author_name,
        content: body.content,
        mood: body.mood,
        tick: body.tick,
    });

    api_ok(summary)
}

async fn list_feed(
    State(state): State<AppState>,
    Query(query): Query<FeedQuery>,
) -> impl IntoResponse {
    let store = match &state.feed_store {
        Some(s) => s,
        None => return api_err(StatusCode::SERVICE_UNAVAILABLE, "feed store not configured"),
    };
    let limit = query.limit.unwrap_or(20).min(100);
    let offset = query.offset.unwrap_or(0);
    let sort = query.sort.as_deref().unwrap_or("newest");
    let store = store.lock().await;
    let total = store.post_count(query.author_id.as_deref());
    let posts: Vec<PostSummary> = store
        .list_posts(query.author_id.as_deref(), limit, offset, sort)
        .into_iter()
        .map(PostSummary::from)
        .collect();
    api_ok(FeedResponse { posts, total })
}

async fn get_trending(
    State(state): State<AppState>,
    Query(query): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let store = match &state.feed_store {
        Some(s) => s,
        None => return api_err(StatusCode::SERVICE_UNAVAILABLE, "feed store not configured"),
    };
    let limit: usize = query
        .get("limit")
        .and_then(|v| v.parse().ok())
        .unwrap_or(10)
        .min(50);
    let store = store.lock().await;
    let posts: Vec<PostSummary> = store
        .trending(limit)
        .into_iter()
        .map(PostSummary::from)
        .collect();
    api_ok(posts)
}

async fn get_post_detail(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let store = match &state.feed_store {
        Some(s) => s,
        None => return api_err(StatusCode::SERVICE_UNAVAILABLE, "feed store not configured"),
    };
    let store = store.lock().await;
    match store.get_post(&id) {
        Some(post) => {
            let comments = store.list_comments(&id);
            #[derive(Serialize)]
            struct PostDetail {
                #[serde(flatten)]
                post: PostSummary,
                comments: Vec<Comment>,
            }
            api_ok(PostDetail {
                post: PostSummary::from(post),
                comments: comments.into_iter().cloned().collect(),
            })
        }
        None => api_err(StatusCode::NOT_FOUND, "post not found"),
    }
}

async fn like_post_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<LikeRequest>,
) -> impl IntoResponse {
    let store = match &state.feed_store {
        Some(s) => s,
        None => return api_err(StatusCode::SERVICE_UNAVAILABLE, "feed store not configured"),
    };
    let mut store = store.lock().await;
    if store.like_post(&body.user_id, &id) {
        drop(store);
        state.event_bus.publish(WorldEvent::FeedPostLiked {
            post_id: id,
            user_id: body.user_id,
        });
        api_ok(serde_json::json!({"liked": true}))
    } else {
        api_err(StatusCode::CONFLICT, "already liked or post not found")
    }
}

async fn unlike_post_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<UnlikeRequest>,
) -> impl IntoResponse {
    let store = match &state.feed_store {
        Some(s) => s,
        None => return api_err(StatusCode::SERVICE_UNAVAILABLE, "feed store not configured"),
    };
    let mut store = store.lock().await;
    if store.unlike_post(&body.user_id, &id) {
        api_ok(serde_json::json!({"unliked": true}))
    } else {
        api_err(StatusCode::NOT_FOUND, "like not found")
    }
}

async fn create_comment(
    State(state): State<AppState>,
    Json(body): Json<CreateCommentRequest>,
) -> impl IntoResponse {
    if body.content.is_empty() {
        return api_err(StatusCode::BAD_REQUEST, "content is required");
    }
    if body.author_id.is_empty() {
        return api_err(StatusCode::BAD_REQUEST, "author_id is required");
    }
    let store = match &state.feed_store {
        Some(s) => s,
        None => return api_err(StatusCode::SERVICE_UNAVAILABLE, "feed store not configured"),
    };
    let mut store = store.lock().await;
    match store.add_comment(
        &body.post_id,
        body.author_id.clone(),
        body.author_name.clone(),
        body.content.clone(),
        body.tick,
    ) {
        Some(comment) => {
            let comment_id = comment.id.clone();
            let post_id = comment.post_id.clone();
            let c_author_id = comment.author_id.clone();
            let c_author_name = comment.author_name.clone();
            let c_content = comment.content.clone();
            let c_tick = comment.tick;
            drop(store);
            state.event_bus.publish(WorldEvent::FeedCommentCreated {
                comment_id,
                post_id,
                author_id: c_author_id,
                author_name: c_author_name,
                content: c_content,
                tick: c_tick,
            });
            api_ok(comment)
        }
        None => api_err(StatusCode::NOT_FOUND, "post not found"),
    }
}

async fn list_comments_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let store = match &state.feed_store {
        Some(s) => s,
        None => return api_err(StatusCode::SERVICE_UNAVAILABLE, "feed store not configured"),
    };
    let store = store.lock().await;
    let comments: Vec<Comment> = store.list_comments(&id).into_iter().cloned().collect();
    api_ok(comments)
}

async fn like_comment_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<LikeRequest>,
) -> impl IntoResponse {
    let store = match &state.feed_store {
        Some(s) => s,
        None => return api_err(StatusCode::SERVICE_UNAVAILABLE, "feed store not configured"),
    };
    let mut store = store.lock().await;
    if store.like_comment(&body.user_id, &id) {
        drop(store);
        state.event_bus.publish(WorldEvent::FeedCommentLiked {
            comment_id: id,
            user_id: body.user_id,
        });
        api_ok(serde_json::json!({"liked": true}))
    } else {
        api_err(StatusCode::CONFLICT, "already liked or comment not found")
    }
}
