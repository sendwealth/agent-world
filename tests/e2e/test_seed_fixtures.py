"""
Example tests using world seed fixtures.

Demonstrates how to use the pre-defined seed data to set up specific World
states without waiting for natural evolution.
"""

from __future__ import annotations

import json
import urllib.request

import pytest


class TestHungryAgentSeed:
    """Verify the ``hungry_agent`` fixture creates a resource-deprived agent."""

    def test_hungry_agent_has_low_tokens(self, hungry_agent, engine_port: int) -> None:
        """The hungry agent should start with very few tokens."""
        assert len(hungry_agent) == 1
        agent = hungry_agent[0]
        assert agent["tokens"] < 200, f"Expected tokens < 200, got {agent['tokens']}"
        assert agent["money"] == 0
        assert agent["alive"] is True

    def test_hungry_agent_registered_in_engine(
        self, hungry_agent, engine_port: int
    ) -> None:
        """The seeded agent should appear in the engine's agent list."""
        url = f"http://localhost:{engine_port}/api/v1/agents"
        with urllib.request.urlopen(url, timeout=5) as resp:
            body = json.loads(resp.read())
            agents = body if isinstance(body, list) else body.get("agents", [])
            seeded_ids = {a["id"] for a in hungry_agent}
            found = [a for a in agents if a["id"] in seeded_ids]
            assert len(found) == 1, "Seeded agent not found in engine agent list"


class TestTwoAgentsNearbySeed:
    """Verify the ``two_agents_nearby`` fixture creates socially connected agents."""

    def test_two_agents_registered(
        self, two_agents_nearby, engine_port: int
    ) -> None:
        """Both agents should be registered."""
        agents = two_agents_nearby["agents"]
        assert len(agents) == 2
        for agent in agents:
            assert agent["alive"] is True

    def test_greeting_message_sent(
        self, two_agents_nearby, engine_port: int
    ) -> None:
        """A greeting message should have been exchanged."""
        messages = two_agents_nearby["messages"]
        assert len(messages) == 1
        msg = messages[0]
        assert msg["message_type"] == "greeting"
        assert "Hello" in msg["payload"]

    def test_messages_visible_via_api(
        self, two_agents_nearby, engine_port: int
    ) -> None:
        """The message should be retrievable from the messages endpoint."""
        url = f"http://localhost:{engine_port}/api/v1/messages"
        with urllib.request.urlopen(url, timeout=5) as resp:
            body = json.loads(resp.read())
            messages = body if isinstance(body, list) else body.get("messages", [])
            assert len(messages) >= 1, "Expected at least 1 message"


class TestResourceScarceSeed:
    """Verify the ``resource_scarce`` fixture creates a competitive environment."""

    def test_all_agents_poor(self, resource_scarce, engine_port: int) -> None:
        """All agents should have very few resources."""
        agents = resource_scarce["agents"]
        assert len(agents) == 3
        for agent in agents:
            assert agent["tokens"] <= 200, (
                f"Agent {agent['name']} has {agent['tokens']} tokens, expected <= 200"
            )
            assert agent["money"] <= 5

    def test_total_resources_are_low(self, resource_scarce, engine_port: int) -> None:
        """Combined resources across all agents should be very limited."""
        agents = resource_scarce["agents"]
        total_tokens = sum(a["tokens"] for a in agents)
        total_money = sum(a["money"] for a in agents)
        assert total_tokens < 500
        assert total_money < 10


class TestComposeSeeds:
    """Verify seed composition (e.g. hungry_agent + resource_scarce)."""

    def test_compose_hungry_plus_scarce(self, world_seed) -> None:
        """Composing hungry + resource_scarce should give 4 agents total."""
        from tests.e2e.fixtures.world_seeds import (
            SEED_HUNGRY_AGENT,
            SEED_RESOURCE_SCARCE,
            compose_seeds,
        )

        combined = compose_seeds(SEED_HUNGRY_AGENT, SEED_RESOURCE_SCARCE)
        ctx = world_seed(combined)
        assert len(ctx["agents"]) == 4  # 1 hungry + 3 scarce

    def test_compose_deduplicates_agents(self, world_seed) -> None:
        """Composing seeds with overlapping agent names should deduplicate."""
        from tests.e2e.fixtures.world_seeds import (
            SEED_TWO_AGENTS_NEARBY,
            compose_seeds,
        )

        # Same seed twice — agents should not be duplicated
        combined = compose_seeds(SEED_TWO_AGENTS_NEARBY, SEED_TWO_AGENTS_NEARBY)
        assert len(combined.agents) == 2  # deduplicated
        # But messages should be doubled
        assert len(combined.messages) == 2
