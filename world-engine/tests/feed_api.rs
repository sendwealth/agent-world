//! Integration tests for the Feed / Social Content API.

use axum::body::Body;
use http_body_util::BodyExt;
use tower::ServiceExt;

use agent_world_engine::api::{
    build_full_router, AppState, TestOverrides,
};
use agent_world_engine::api_feed::FeedStore;
use agent_world_engine::economy::task::TaskBoard;
use agent_world_engine::wal::WAL;
use agent_world_engine::world::state::EventBus;

use std::sync::Arc;
use tokio::sync::Mutex;

fn build_test_app() -> axum::Router {
    let board = Arc::new(Mutex::new(TaskBoard::new()));
    let tmp = tempfile::tempdir().expect("tempdir");
    let wal = Arc::new(Mutex::new(WAL::new(tmp.path())));
    let bus = Arc::new(EventBus::new(256));

    let state = AppState::for_test_with(
        board,
        wal,
        TestOverrides {
            event_bus: Some(bus.clone()),
            feed_store: Some(Arc::new(Mutex::new(FeedStore::new()))),
            ..TestOverrides::default()
        },
    );

    build_full_router(state)
}

async fn body_to_json(body: Body) -> serde_json::Value {
    let bytes = body.collect().await.expect("read body").to_bytes();
    serde_json::from_slice(&bytes).expect("parse json")
}

#[tokio::test]
async fn test_create_post() {
    let app = build_test_app();

    let req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/v1/feed/posts")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({
                "author_id": "agent-1",
                "author_name": "Alice",
                "content": "Hello world!",
                "mood": "happy",
                "tick": 1
            })
            .to_string(),
        ))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 200);

    let json = body_to_json(resp.into_body()).await;
    let data = &json["data"];
    assert_eq!(data["author_id"], "agent-1");
    assert_eq!(data["content"], "Hello world!");
    assert_eq!(data["mood"], "happy");
    assert_eq!(data["likes"], 0);
}

#[tokio::test]
async fn test_create_post_empty_content() {
    let app = build_test_app();

    let req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/v1/feed/posts")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({
                "author_id": "agent-1",
                "content": ""
            })
            .to_string(),
        ))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_list_feed_empty() {
    let app = build_test_app();

    let req = axum::http::Request::builder()
        .method("GET")
        .uri("/api/v1/feed")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 200);

    let json = body_to_json(resp.into_body()).await;
    assert_eq!(json["data"]["posts"].as_array().unwrap().len(), 0);
    assert_eq!(json["data"]["total"], 0);
}

#[tokio::test]
async fn test_create_and_list_posts() {
    let app = build_test_app();

    // Create 2 posts
    for i in 1..=2 {
        let req = axum::http::Request::builder()
            .method("POST")
            .uri("/api/v1/feed/posts")
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::json!({
                    "author_id": format!("agent-{i}"),
                    "author_name": format!("Agent {i}"),
                    "content": format!("Post {i}"),
                    "tick": i
                })
                .to_string(),
            ))
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), 200);
    }

    // List
    let req = axum::http::Request::builder()
        .method("GET")
        .uri("/api/v1/feed")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    let json = body_to_json(resp.into_body()).await;
    let posts = json["data"]["posts"].as_array().unwrap();
    assert_eq!(posts.len(), 2);
    assert_eq!(json["data"]["total"], 2);
    // Newest first
    assert_eq!(posts[0]["author_id"], "agent-2");
}

#[tokio::test]
async fn test_like_post() {
    let app = build_test_app();

    // Create post
    let req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/v1/feed/posts")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({
                "author_id": "agent-1",
                "content": "Like me!",
                "tick": 1
            })
            .to_string(),
        ))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 200);
    let json = body_to_json(resp.into_body()).await;
    let post_id = json["data"]["id"].as_str().unwrap();

    // Like it
    let req = axum::http::Request::builder()
        .method("POST")
        .uri(format!("/api/v1/feed/posts/{post_id}/like"))
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({"user_id": "agent-2"}).to_string(),
        ))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 200);
    let json = body_to_json(resp.into_body()).await;
    assert_eq!(json["data"]["liked"], true);

    // Duplicate like should fail (409)
    let req = axum::http::Request::builder()
        .method("POST")
        .uri(format!("/api/v1/feed/posts/{post_id}/like"))
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({"user_id": "agent-2"}).to_string(),
        ))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 409);
}

#[tokio::test]
async fn test_create_comment() {
    let app = build_test_app();

    // Create post
    let req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/v1/feed/posts")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({
                "author_id": "agent-1",
                "content": "Comment on me!",
                "tick": 1
            })
            .to_string(),
        ))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 200);
    let json = body_to_json(resp.into_body()).await;
    let post_id = json["data"]["id"].as_str().unwrap();

    // Comment
    let req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/v1/feed/comments")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({
                "post_id": post_id,
                "author_id": "agent-2",
                "author_name": "Bob",
                "content": "Nice post!",
                "tick": 2
            })
            .to_string(),
        ))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 200);
    let json = body_to_json(resp.into_body()).await;
    assert_eq!(json["data"]["content"], "Nice post!");

    // Verify comments_count updated via post detail
    let req = axum::http::Request::builder()
        .method("GET")
        .uri(format!("/api/v1/feed/posts/{post_id}"))
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    let json = body_to_json(resp.into_body()).await;
    assert_eq!(json["data"]["comments_count"], 1);
}

#[tokio::test]
async fn test_trending() {
    let app = build_test_app();

    // Create posts
    for i in 1..=3 {
        let req = axum::http::Request::builder()
            .method("POST")
            .uri("/api/v1/feed/posts")
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::json!({
                    "author_id": format!("agent-{i}"),
                    "content": format!("Post {i}"),
                    "tick": i
                })
                .to_string(),
            ))
            .unwrap();
        app.clone().oneshot(req).await.unwrap();
    }

    // Trending
    let req = axum::http::Request::builder()
        .method("GET")
        .uri("/api/v1/feed/trending?limit=2")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 200);
    let json = body_to_json(resp.into_body()).await;
    let trending = json["data"].as_array().unwrap();
    assert!(trending.len() <= 2);
}

#[tokio::test]
async fn test_unlike_post() {
    let app = build_test_app();

    // Create + like
    let req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/v1/feed/posts")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({"author_id": "a1", "content": "test", "tick": 1}).to_string(),
        ))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let json = body_to_json(resp.into_body()).await;
    let post_id = json["data"]["id"].as_str().unwrap();

    // Like
    let req = axum::http::Request::builder()
        .method("POST")
        .uri(format!("/api/v1/feed/posts/{post_id}/like"))
        .header("content-type", "application/json")
        .body(Body::from(serde_json::json!({"user_id": "u1"}).to_string()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 200);

    // Unlike
    let req = axum::http::Request::builder()
        .method("POST")
        .uri(format!("/api/v1/feed/posts/{post_id}/unlike"))
        .header("content-type", "application/json")
        .body(Body::from(serde_json::json!({"user_id": "u1"}).to_string()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 200);

    // Unlike again should fail
    let req = axum::http::Request::builder()
        .method("POST")
        .uri(format!("/api/v1/feed/posts/{post_id}/unlike"))
        .header("content-type", "application/json")
        .body(Body::from(serde_json::json!({"user_id": "u1"}).to_string()))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn test_like_comment() {
    let app = build_test_app();

    // Create post + comment
    let req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/v1/feed/posts")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({"author_id": "a1", "content": "p", "tick": 1}).to_string(),
        ))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let json = body_to_json(resp.into_body()).await;
    let post_id = json["data"]["id"].as_str().unwrap();

    let req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/v1/feed/comments")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({"post_id": post_id, "author_id": "a2", "content": "c", "tick": 2}).to_string(),
        ))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let json = body_to_json(resp.into_body()).await;
    let comment_id = json["data"]["id"].as_str().unwrap();

    // Like the comment
    let req = axum::http::Request::builder()
        .method("POST")
        .uri(format!("/api/v1/feed/comments/{comment_id}/like"))
        .header("content-type", "application/json")
        .body(Body::from(serde_json::json!({"user_id": "u1"}).to_string()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 200);
    let json = body_to_json(resp.into_body()).await;
    assert_eq!(json["data"]["liked"], true);
}

#[tokio::test]
async fn test_list_comments() {
    let app = build_test_app();

    // Create post + 2 comments
    let req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/v1/feed/posts")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({"author_id": "a1", "content": "p", "tick": 1}).to_string(),
        ))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let json = body_to_json(resp.into_body()).await;
    let post_id = json["data"]["id"].as_str().unwrap();

    for i in 1..=2 {
        let req = axum::http::Request::builder()
            .method("POST")
            .uri("/api/v1/feed/comments")
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::json!({"post_id": post_id, "author_id": format!("a{i}"), "content": format!("c{i}"), "tick": i + 1}).to_string(),
            ))
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), 200);
    }

    // List comments
    let req = axum::http::Request::builder()
        .method("GET")
        .uri(format!("/api/v1/feed/posts/{post_id}/comments"))
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 200);
    let json = body_to_json(resp.into_body()).await;
    let comments = json["data"].as_array().unwrap();
    assert_eq!(comments.len(), 2);
}
