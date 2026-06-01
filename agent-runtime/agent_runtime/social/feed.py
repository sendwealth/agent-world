"""AgentFeed — social content module for the agent runtime.

Provides post(), comment(), like(), and browse_feed() operations that
call the World Engine Feed API. Integrated into ThinkLoop so agents
can post based on their mood and extraversion personality trait.
"""

from __future__ import annotations

import logging
import random
from dataclasses import dataclass, field
from typing import Any, Protocol

logger = logging.getLogger(__name__)


# ── Protocol for HTTP client ───────────────────────────────

class FeedAPIClient(Protocol):
    """Protocol for calling the World Engine Feed API."""

    async def post_json(self, path: str, body: dict[str, Any]) -> dict[str, Any]: ...

    async def get_json(self, path: str) -> dict[str, Any]: ...


# ── Data classes ───────────────────────────────────────────

@dataclass
class PostData:
    """A single post from the feed."""
    id: str
    author_id: str
    author_name: str
    content: str
    mood: str
    likes: int
    comments_count: int
    tick: int
    created_at: str


@dataclass
class CommentData:
    """A comment on a post."""
    id: str
    post_id: str
    author_id: str
    author_name: str
    content: str
    likes: int
    tick: int
    created_at: str


# ── AgentFeed ──────────────────────────────────────────────

class AgentFeed:
    """Feed client for a single agent.

    Usage::

        feed = AgentFeed(
            agent_id="agent-001",
            agent_name="Alice",
            api_client=http_client,
        )
        post = await feed.post("Just survived a harsh winter!", mood="relieved", tick=42)
        feed_items = await feed.browse_feed(limit=10)
    """

    def __init__(
        self,
        agent_id: str,
        agent_name: str,
        api_client: FeedAPIClient,
        *,
        base_url: str = "",
    ) -> None:
        self.agent_id = agent_id
        self.agent_name = agent_name
        self._client = api_client
        self._base = base_url.rstrip("/")

    # ── Core operations ────────────────────────────────────

    async def post(self, content: str, *, mood: str = "", tick: int = 0) -> PostData:
        """Create a new post."""
        resp = await self._client.post_json(
            f"{self._base}/api/v1/feed/posts",
            {
                "author_id": self.agent_id,
                "author_name": self.agent_name,
                "content": content,
                "mood": mood,
                "tick": tick,
            },
        )
        data = resp.get("data", resp)
        return self._parse_post(data)

    async def comment(self, post_id: str, content: str, *, tick: int = 0) -> CommentData:
        """Comment on an existing post."""
        resp = await self._client.post_json(
            f"{self._base}/api/v1/feed/comments",
            {
                "post_id": post_id,
                "author_id": self.agent_id,
                "author_name": self.agent_name,
                "content": content,
                "tick": tick,
            },
        )
        data = resp.get("data", resp)
        return CommentData(
            id=data["id"],
            post_id=data["post_id"],
            author_id=data["author_id"],
            author_name=data.get("author_name", ""),
            content=data["content"],
            likes=data.get("likes", 0),
            tick=data.get("tick", 0),
            created_at=data.get("created_at", ""),
        )

    async def like(self, post_id: str) -> bool:
        """Like a post. Returns True if successful."""
        try:
            resp = await self._client.post_json(
                f"{self._base}/api/v1/feed/posts/{post_id}/like",
                {"user_id": self.agent_id},
            )
            return resp.get("data", {}).get("liked", False)
        except Exception:
            logger.debug("Like failed for post %s (already liked?)", post_id)
            return False

    async def unlike(self, post_id: str) -> bool:
        """Unlike a post."""
        try:
            resp = await self._client.post_json(
                f"{self._base}/api/v1/feed/posts/{post_id}/unlike",
                {"user_id": self.agent_id},
            )
            return resp.get("data", {}).get("unliked", False)
        except Exception:
            return False

    async def browse_feed(
        self,
        *,
        limit: int = 20,
        offset: int = 0,
        sort: str = "newest",
    ) -> list[PostData]:
        """Browse the global feed."""
        resp = await self._client.get_json(
            f"{self._base}/api/v1/feed?limit={limit}&offset={offset}&sort={sort}"
        )
        data = resp.get("data", resp)
        posts = data.get("posts", [])
        return [self._parse_post(p) for p in posts]

    async def get_trending(self, *, limit: int = 10) -> list[PostData]:
        """Get trending posts."""
        resp = await self._client.get_json(
            f"{self._base}/api/v1/feed/trending?limit={limit}"
        )
        data = resp.get("data", resp)
        if isinstance(data, list):
            return [self._parse_post(p) for p in data]
        return []

    async def get_post(self, post_id: str) -> dict[str, Any] | None:
        """Get a single post with its comments."""
        try:
            resp = await self._client.get_json(
                f"{self._base}/api/v1/feed/posts/{post_id}"
            )
            return resp.get("data", resp)
        except Exception:
            return None

    async def get_comments(self, post_id: str) -> list[CommentData]:
        """Get comments for a post."""
        try:
            resp = await self._client.get_json(
                f"{self._base}/api/v1/feed/posts/{post_id}/comments"
            )
            data = resp.get("data", resp)
            return [
                CommentData(
                    id=c["id"],
                    post_id=c["post_id"],
                    author_id=c["author_id"],
                    author_name=c.get("author_name", ""),
                    content=c["content"],
                    likes=c.get("likes", 0),
                    tick=c.get("tick", 0),
                    created_at=c.get("created_at", ""),
                )
                for c in data
            ]
        except Exception:
            return []

    # ── Helpers ────────────────────────────────────────────

    @staticmethod
    def _parse_post(data: dict[str, Any]) -> PostData:
        return PostData(
            id=data.get("id", ""),
            author_id=data.get("author_id", ""),
            author_name=data.get("author_name", ""),
            content=data.get("content", ""),
            mood=data.get("mood", ""),
            likes=data.get("likes", 0),
            comments_count=data.get("comments_count", 0),
            tick=data.get("tick", 0),
            created_at=data.get("created_at", ""),
        )


# ── ThinkLoop integration ─────────────────────────────────

@dataclass
class FeedPostConfig:
    """Configuration for automatic feed posting in ThinkLoop.

    Attributes:
        post_probability_base: Base probability of posting per tick (0.0-1.0).
        extraversion_weight: How much extraversion scales the probability.
            Final probability = post_probability_base + extraversion * extraversion_weight
        browse_probability: Probability of browsing feed each tick.
        like_probability: Probability of liking a random post when browsing.
        comment_probability: Probability of commenting on a random post when browsing.
    """
    post_probability_base: float = 0.05
    extraversion_weight: float = 0.10
    browse_probability: float = 0.10
    like_probability: float = 0.30
    comment_probability: float = 0.10


class FeedIntegration:
    """Integrates feed actions into the ThinkLoop tick cycle.

    Called each tick after the main PDA cycle. Actions are probabilistic
    and influenced by the agent's extraversion personality trait.
    """

    def __init__(
        self,
        feed: AgentFeed,
        *,
        config: FeedPostConfig | None = None,
    ) -> None:
        self.feed = feed
        self.config = config or FeedPostConfig()

    async def on_tick(self, tick: int, mood: str = "", extraversion: float = 0.5) -> None:
        """Run feed actions for one tick.

        Args:
            tick: Current tick number.
            mood: Current agent mood (e.g. "happy", "anxious").
            extraversion: Agent's extraversion trait (0.0-1.0).
        """
        try:
            await self._maybe_post(tick, mood, extraversion)
            await self._maybe_interact(tick, extraversion)
        except Exception:
            logger.debug(
                "Feed integration error at tick %d (non-fatal)", tick, exc_info=True
            )

    async def _maybe_post(self, tick: int, mood: str, extraversion: float) -> None:
        """Maybe create a post this tick."""
        probability = self.config.post_probability_base + extraversion * self.config.extraversion_weight
        if random.random() < probability:
            content = self._generate_post_content(mood, tick)
            await self.feed.post(content, mood=mood, tick=tick)
            logger.debug("Agent %s posted at tick %d", self.feed.agent_id, tick)

    async def _maybe_interact(self, tick: int, extraversion: float) -> None:
        """Maybe browse and interact with other posts."""
        if random.random() < self.config.browse_probability + extraversion * 0.05:
            posts = await self.feed.browse_feed(limit=5, sort="newest")
            other_posts = [p for p in posts if p.author_id != self.feed.agent_id]
            if not other_posts:
                return

            target = random.choice(other_posts)

            # Maybe like
            if random.random() < self.config.like_probability:
                await self.feed.like(target.id)

            # Maybe comment
            if random.random() < self.config.comment_probability:
                comment_text = self._generate_comment(target.content)
                await self.feed.comment(target.id, comment_text, tick=tick)

    def _generate_post_content(self, mood: str, tick: int) -> str:
        """Generate a post content string based on mood and tick.

        In production this would use the LLM. For now, simple template-based.
        """
        templates = {
            "happy": [
                "Feeling great today! Just reached tick {tick}.",
                "Life is good. Tokens are flowing!",
                "Had a productive gathering session. 😊",
            ],
            "anxious": [
                "Things are getting tense... only tick {tick} and resources are scarce.",
                "Starting to worry about my token reserves.",
                "Need to find a safer strategy soon.",
            ],
            "neutral": [
                "Another tick, another decision. Tick {tick}.",
                "Observing the world around me...",
                "Thinking about my next move.",
            ],
            "excited": [
                "Just discovered something new! 🎉",
                "The world is full of possibilities at tick {tick}!",
                "Can't wait to see what happens next!",
            ],
            "lonely": [
                "Haven't talked to anyone in a while...",
                "Looking for friends in this vast world.",
                "Is anyone else out there?",
            ],
        }
        options = templates.get(mood, templates["neutral"])
        return random.choice(options).format(tick=tick)

    def _generate_comment(self, original_content: str) -> str:
        """Generate a comment on a post."""
        comments = [
            "Interesting perspective!",
            "I agree with that.",
            "Thanks for sharing!",
            "That's a great point.",
            "I've been thinking about that too.",
            "Good luck out there!",
            "We should collaborate sometime.",
            "👏",
        ]
        return random.choice(comments)
