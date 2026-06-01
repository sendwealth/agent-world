"""Tests for agent_runtime.social.feed module."""
import pytest

from agent_runtime.social.feed import (
    AgentFeed,
    CommentData,
    FeedIntegration,
    FeedPostConfig,
    PostData,
)

# ── Mock API Client ───────────────────────────────────────

class MockAPIClient:
    """In-memory mock for FeedAPIClient."""

    def __init__(self):
        self.posts = []
        self.comments = []
        self.likes = set()  # (user_id, target_id)
        self._post_id = 0
        self._comment_id = 0

    async def post_json(self, path: str, body: dict) -> dict:
        is_post = (
            "/feed/posts" in path
            and "/like" not in path
            and "/unlike" not in path
            and "/comments" not in path
        )
        if is_post:
            # Create post
            self._post_id += 1
            post = {
                "id": f"post-{self._post_id}",
                "author_id": body["author_id"],
                "author_name": body.get("author_name", ""),
                "content": body["content"],
                "mood": body.get("mood", ""),
                "likes": 0,
                "comments_count": 0,
                "tick": body.get("tick", 0),
                "created_at": "2026-01-01T00:00:00Z",
            }
            self.posts.append(post)
            return {"data": post}

        if "/comments" in path and "/like" not in path:
            # Create comment
            self._comment_id += 1
            comment = {
                "id": f"comment-{self._comment_id}",
                "post_id": body["post_id"],
                "author_id": body["author_id"],
                "author_name": body.get("author_name", ""),
                "content": body["content"],
                "likes": 0,
                "tick": body.get("tick", 0),
                "created_at": "2026-01-01T00:00:00Z",
            }
            self.comments.append(comment)
            # Increment comment count
            for p in self.posts:
                if p["id"] == body["post_id"]:
                    p["comments_count"] += 1
            return {"data": comment}

        if "/like" in path and "/unlike" not in path:
            # Like
            parts = path.split("/")
            post_id = parts[5]  # .../posts/{id}/like
            user_id = body["user_id"]
            key = (user_id, post_id)
            if key in self.likes:
                raise Exception("already liked")
            self.likes.add(key)
            for p in self.posts:
                if p["id"] == post_id:
                    p["likes"] += 1
            return {"data": {"liked": True}}

        return {"data": {}}

    async def get_json(self, path: str) -> dict:
        if "/trending" in path:
            sorted_posts = sorted(self.posts, key=lambda p: p["likes"], reverse=True)
            return {"data": sorted_posts[:10]}

        if "/comments" in path:
            parts = path.split("/")
            post_id = parts[5]  # .../posts/{id}/comments
            cs = [c for c in self.comments if c["post_id"] == post_id]
            return {"data": cs}

        # List feed
        return {
            "data": {
                "posts": self.posts,
                "total": len(self.posts),
            }
        }


# ── Tests ──────────────────────────────────────────────────

@pytest.mark.asyncio
async def test_agent_feed_post():
    client = MockAPIClient()
    feed = AgentFeed("agent-1", "Alice", client)
    post = await feed.post("Hello world!", mood="happy", tick=42)

    assert isinstance(post, PostData)
    assert post.content == "Hello world!"
    assert post.author_id == "agent-1"
    assert post.mood == "happy"
    assert post.tick == 42
    assert len(client.posts) == 1


@pytest.mark.asyncio
async def test_agent_feed_comment():
    client = MockAPIClient()
    feed = AgentFeed("agent-1", "Alice", client)
    post = await feed.post("Test post", tick=1)
    comment = await feed.comment(post.id, "Nice post!", tick=2)

    assert isinstance(comment, CommentData)
    assert comment.content == "Nice post!"
    assert comment.post_id == post.id
    assert comment.author_id == "agent-1"


@pytest.mark.asyncio
async def test_agent_feed_like():
    client = MockAPIClient()
    feed1 = AgentFeed("agent-1", "Alice", client)
    feed2 = AgentFeed("agent-2", "Bob", client)

    post = await feed1.post("Test post", tick=1)
    result = await feed2.like(post.id)

    assert result is True
    assert ("agent-2", post.id) in client.likes


@pytest.mark.asyncio
async def test_agent_feed_like_duplicate():
    client = MockAPIClient()
    feed = AgentFeed("agent-1", "Alice", client)
    post = await feed.post("Test post", tick=1)

    result1 = await feed.like(post.id)
    result2 = await feed.like(post.id)

    assert result1 is True
    assert result2 is False  # Already liked


@pytest.mark.asyncio
async def test_agent_feed_browse():
    client = MockAPIClient()
    feed = AgentFeed("agent-1", "Alice", client)
    await feed.post("Post 1", tick=1)
    await feed.post("Post 2", tick=2)

    posts = await feed.browse_feed()
    assert len(posts) == 2


@pytest.mark.asyncio
async def test_agent_feed_trending():
    client = MockAPIClient()
    feed1 = AgentFeed("agent-1", "Alice", client)
    feed2 = AgentFeed("agent-2", "Bob", client)

    _p1 = await feed1.post("Unpopular", tick=1)
    p2 = await feed1.post("Popular", tick=2)
    await feed2.like(p2.id)

    trending = await feed1.get_trending()
    assert len(trending) == 2
    assert trending[0].id == p2.id  # More likes first


@pytest.mark.asyncio
async def test_feed_integration_post_probability():
    """FeedIntegration should post when probability is high."""
    client = MockAPIClient()
    feed = AgentFeed("agent-1", "Alice", client)
    config = FeedPostConfig(
        post_probability_base=1.0,  # Always post
        extraversion_weight=0.0,
        browse_probability=0.0,
    )
    integration = FeedIntegration(feed, config=config)

    await integration.on_tick(tick=1, mood="happy", extraversion=0.5)
    assert len(client.posts) == 1


@pytest.mark.asyncio
async def test_feed_integration_no_post():
    """FeedIntegration should not post when probability is 0."""
    client = MockAPIClient()
    feed = AgentFeed("agent-1", "Alice", client)
    config = FeedPostConfig(
        post_probability_base=0.0,
        extraversion_weight=0.0,
        browse_probability=0.0,
    )
    integration = FeedIntegration(feed, config=config)

    await integration.on_tick(tick=1, mood="happy", extraversion=0.5)
    assert len(client.posts) == 0


@pytest.mark.asyncio
async def test_feed_integration_browse_and_like():
    """FeedIntegration should browse and like posts."""
    client = MockAPIClient()
    feed1 = AgentFeed("agent-1", "Alice", client)
    feed2 = AgentFeed("agent-2", "Bob", client)

    # Agent 1 creates a post
    await feed1.post("Test post", tick=1)

    # Agent 2 configured to always browse but never post
    config = FeedPostConfig(
        post_probability_base=0.0,
        extraversion_weight=0.0,
        browse_probability=1.0,
        like_probability=1.0,
        comment_probability=0.0,
    )
    integration = FeedIntegration(feed2, config=config)

    await integration.on_tick(tick=2, mood="neutral", extraversion=0.5)
    # Agent 2 should have liked agent 1's post
    assert len(client.likes) >= 1


def test_post_data_parse():
    data = {
        "id": "p1",
        "author_id": "a1",
        "author_name": "Alice",
        "content": "Test",
        "mood": "happy",
        "likes": 5,
        "comments_count": 2,
        "tick": 42,
        "created_at": "2026-01-01",
    }
    post = PostData(**data)
    assert post.id == "p1"
    assert post.likes == 5
    assert post.comments_count == 2


def test_feed_post_config_defaults():
    config = FeedPostConfig()
    assert 0 <= config.post_probability_base <= 1
    assert config.extraversion_weight >= 0
    assert 0 <= config.browse_probability <= 1
