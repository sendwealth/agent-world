"""
World seed fixtures for E2E tests.

Provides pre-defined World states injected via the World Engine REST API,
inspired by LobeChat's direct-DB seed pattern.

Each fixture creates a specific scenario by POST-ing agents, messages, and
tick state through the engine's HTTP endpoints, so tests can skip the slow
natural evolution process and start from an interesting known state.

Design notes
------------
- The World Engine tracks ``tokens`` and ``money`` per agent but has no
  explicit "hunger" field.  We model hunger/deprivation as **low tokens +
  low money** — an agent with tokens=100 and money=0 is effectively
  "starving".
- There is no spatial coordinate system in the current API.  "Nearby" is
  modelled as agents that have already exchanged at least one message,
  simulating an existing social bond.
- Fixtures are *composable*: the ``compose_seeds`` helper merges multiple
  seed specifications and injects them in a single batch.

Usage::

    # In a test file:
    def test_hungry_agent_seeks_food(hungry_agent, engine_port):
        agents = hungry_agent
        assert len(agents) == 1
        assert agents[0]["tokens"] < 200  # effectively starving
"""

from __future__ import annotations

import json
import urllib.request
import urllib.error
from dataclasses import dataclass, field
from typing import Any

import pytest


# ── Data models ──────────────────────────────────────────────────

@dataclass
class AgentSpec:
    """Blueprint for an agent to be injected via ``POST /api/v1/agents``."""

    name: str
    tokens: int = 100_000
    money: int = 0
    personality: str = ""

    def to_payload(self) -> dict[str, Any]:
        return {
            "name": self.name,
            "tokens": self.tokens,
            "money": self.money,
        }


@dataclass
class MessageSpec:
    """Blueprint for an A2A message via ``POST /api/v1/messages``."""

    from_agent: str  # agent name (resolved to id at inject time)
    to_agent: str
    message_type: str = "greeting"
    payload: str = ""


@dataclass
class WorldSeed:
    """Complete seed specification for a world state."""

    agents: list[AgentSpec] = field(default_factory=list)
    messages: list[MessageSpec] = field(default_factory=list)
    advance_ticks: int = 0


# ── HTTP helpers ─────────────────────────────────────────────────

def _post_json(base_url: str, path: str, body: dict[str, Any]) -> dict[str, Any]:
    """POST JSON to *base_url/path* and return parsed response."""
    url = f"{base_url}{path}"
    data = json.dumps(body).encode()
    req = urllib.request.Request(
        url,
        data=data,
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    with urllib.request.urlopen(req, timeout=10) as resp:
        return json.loads(resp.read().decode())


def _get_json(base_url: str, path: str) -> dict[str, Any]:
    """GET JSON from *base_url/path*."""
    url = f"{base_url}{path}"
    req = urllib.request.Request(url, headers={"Accept": "application/json"})
    with urllib.request.urlopen(req, timeout=10) as resp:
        return json.loads(resp.read().decode())


# ── Seed injection ───────────────────────────────────────────────

def inject_seed(base_url: str, seed: WorldSeed) -> dict[str, Any]:
    """Inject a ``WorldSeed`` into the engine and return a context dict.

    Returns ``{"agents": [...], "messages": [...]}`` with full records.
    """
    name_to_id: dict[str, str] = {}
    agent_records: list[dict[str, Any]] = []

    # 1. Spawn agents
    for spec in seed.agents:
        resp = _post_json(base_url, "/api/v1/agents", spec.to_payload())
        name_to_id[spec.name] = resp["id"]
        agent_records.append(resp)

    # 2. Send messages (resolve names → ids)
    msg_records: list[dict[str, Any]] = []
    for msg in seed.messages:
        from_id = name_to_id.get(msg.from_agent, msg.from_agent)
        to_id = name_to_id.get(msg.to_agent, msg.to_agent)
        resp = _post_json(base_url, "/api/v1/messages", {
            "from_agent": from_id,
            "to_agent": to_id,
            "message_type": msg.message_type,
            "payload": msg.payload,
        })
        msg_records.append(resp)

    # 3. Advance ticks if requested
    if seed.advance_ticks > 0:
        _post_json(base_url, "/api/v1/tick", {"count": seed.advance_ticks})

    return {
        "agents": agent_records,
        "messages": msg_records,
        "name_to_id": name_to_id,
    }


def compose_seeds(*seeds: WorldSeed) -> WorldSeed:
    """Merge multiple ``WorldSeed`` objects into one.

    Agent names are de-duplicated (first occurrence wins).  Ticks are
    summed.  Messages are concatenated.
    """
    seen_names: set[str] = set()
    merged_agents: list[AgentSpec] = []
    merged_messages: list[MessageSpec] = []
    total_ticks = 0

    for seed in seeds:
        for agent in seed.agents:
            if agent.name not in seen_names:
                merged_agents.append(agent)
                seen_names.add(agent.name)
        merged_messages.extend(seed.messages)
        total_ticks += seed.advance_ticks

    return WorldSeed(
        agents=merged_agents,
        messages=merged_messages,
        advance_ticks=total_ticks,
    )


# ── Pre-defined seed specifications ─────────────────────────────

SEED_HUNGRY_AGENT = WorldSeed(
    agents=[
        AgentSpec(name="hungry-1", tokens=100, money=0),
    ],
)

SEED_TWO_AGENTS_NEARBY = WorldSeed(
    agents=[
        AgentSpec(name="nearby-a", tokens=50_000, money=100),
        AgentSpec(name="nearby-b", tokens=50_000, money=100),
    ],
    messages=[
        MessageSpec(
            from_agent="nearby-a",
            to_agent="nearby-b",
            message_type="greeting",
            payload="Hello! I'm nearby.",
        ),
    ],
)

SEED_GROUP_OF_FIVE = WorldSeed(
    agents=[
        AgentSpec(name="group-1", tokens=80_000, money=500),
        AgentSpec(name="group-2", tokens=60_000, money=300),
        AgentSpec(name="group-3", tokens=70_000, money=400),
        AgentSpec(name="group-4", tokens=90_000, money=200),
        AgentSpec(name="group-5", tokens=50_000, money=100),
    ],
    messages=[
        MessageSpec("group-1", "group-2", "greeting", "Let's cooperate."),
        MessageSpec("group-2", "group-3", "greeting", "Join us?"),
        MessageSpec("group-3", "group-4", "greeting", "We're forming a group."),
        MessageSpec("group-4", "group-5", "greeting", "Come along!"),
    ],
)

SEED_RESOURCE_SCARCE = WorldSeed(
    agents=[
        AgentSpec(name="scarce-1", tokens=200, money=5),
        AgentSpec(name="scarce-2", tokens=150, money=3),
        AgentSpec(name="scarce-3", tokens=100, money=1),
    ],
)


# ── pytest fixtures ──────────────────────────────────────────────

@pytest.fixture()
def hungry_agent(
    world_engine_process,  # noqa: ANN001 — from conftest
    engine_port: int,
) -> list[dict[str, Any]]:
    """A single agent with very low resources (tokens=100, money=0).

    Represents an agent on the brink of starvation, useful for testing
    survival-critical decision-making.
    """
    base_url = f"http://localhost:{engine_port}"
    result = inject_seed(base_url, SEED_HUNGRY_AGENT)
    return result["agents"]


@pytest.fixture()
def two_agents_nearby(
    world_engine_process,
    engine_port: int,
) -> dict[str, Any]:
    """Two agents that have already exchanged a greeting message.

    Useful for testing social interaction triggers, A2A messaging, and
    cooperative behavior between familiar agents.
    """
    base_url = f"http://localhost:{engine_port}"
    return inject_seed(base_url, SEED_TWO_AGENTS_NEARBY)


@pytest.fixture()
def group_of_five(
    world_engine_process,
    engine_port: int,
) -> dict[str, Any]:
    """Five agents with pre-existing social connections (4 greeting messages).

    Tests social structure formation, group dynamics, and resource
    distribution within a community.
    """
    base_url = f"http://localhost:{engine_port}"
    return inject_seed(base_url, SEED_GROUP_OF_FIVE)


@pytest.fixture()
def resource_scarce(
    world_engine_process,
    engine_port: int,
) -> dict[str, Any]:
    """Three agents with extremely limited resources competing for survival.

    Tokens range from 100–200 and money from 1–5.  Ideal for testing
    competitive behavior, resource conflict, and survival pressure.
    """
    base_url = f"http://localhost:{engine_port}"
    return inject_seed(base_url, SEED_RESOURCE_SCARCE)


@pytest.fixture()
def world_seed(
    world_engine_process,
    engine_port: int,
):
    """Factory fixture: inject an arbitrary ``WorldSeed``.

    Usage::

        def test_custom(world_seed):
            seed = WorldSeed(agents=[AgentSpec("custom-1", tokens=500)])
            ctx = world_seed(seed)
            assert len(ctx["agents"]) == 1
    """
    base_url = f"http://localhost:{engine_port}"

    def _inject(seed: WorldSeed) -> dict[str, Any]:
        return inject_seed(base_url, seed)

    return _inject
