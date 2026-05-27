"""
{{ cookiecutter.project_name }} — Agent World Plugin

{{ cookiecutter.project_description }}
"""

from __future__ import annotations

from typing import Dict, List, Optional
from dataclasses import dataclass, field

# ─── Plugin API Types ───────────────────────────────────────────────────
# When the SDK package is published, these will be imported from:
#   from agent_world_plugin_sdk import (
#       SkillPlugin, PluginInfo, ActionContext, ActionResult,
#       WorldContext, AgentSnapshot, TokenCost, StateMutation, MutationKind,
#       PluginError,
#   )

@dataclass
class AgentSnapshot:
    id: str
    name: str
    phase: str
    money: int
    tokens: int
    reputation: float
    skills: Dict[str, int] = field(default_factory=dict)
    alive: bool = True
    age: int = 0


@dataclass
class WorldContext:
    tick: int
    agent: Optional[AgentSnapshot] = None
    visible_agents: List[AgentSnapshot] = field(default_factory=list)
    globals: Dict[str, str] = field(default_factory=dict)
    recent_events: List[str] = field(default_factory=list)


@dataclass
class ActionContext:
    world: WorldContext
    params: Dict[str, str] = field(default_factory=dict)
    config: Dict[str, str] = field(default_factory=dict)


from enum import Enum


class MutationKind(Enum):
    CREDIT_TOKENS = "credit_tokens"
    DEBIT_TOKENS = "debit_tokens"
    CREDIT_MONEY = "credit_money"
    DEBIT_MONEY = "debit_money"
    SET_SKILL = "set_skill"
    ADJUST_REPUTATION = "adjust_reputation"
    SET_GLOBAL = "set_global"
    EMIT_EVENT = "emit_event"


@dataclass
class StateMutation:
    kind: MutationKind
    target_agent: Optional[str] = None
    field: str = ""
    value: str = ""


@dataclass
class ActionResult:
    success: bool
    message: str
    mutations: List[StateMutation] = field(default_factory=list)
    events: List[str] = field(default_factory=list)
    data: Dict[str, str] = field(default_factory=dict)
    tokens_consumed: int = 0


@dataclass
class TokenCost:
    estimated: int
    confidence: float = 1.0
    breakdown: Optional[str] = None


@dataclass
class PluginInfo:
    id: str
    name: str
    version: str
    description: str
    author: str
    min_engine_version: str
    required_skills: List[str] = field(default_factory=list)
    config_schema: Optional[str] = None
    tags: List[str] = field(default_factory=list)


class PluginError(Exception):
    def __init__(self, code: str = "custom", message: str = ""):
        self.code = code
        self.message = message
        super().__init__(f"[{code}] {message}")


# ─── Plugin Implementation ──────────────────────────────────────────────
# ✏️ Edit below to implement your plugin logic.

class {{ cookiecutter.project_slug.replace('_', ' ').title().replace(' ', '') }}Plugin:
    """Hello World plugin for Agent World."""

    @classmethod
    def init(cls, config: Dict[str, str]) -> PluginInfo:
        """Return plugin metadata. Called once after loading."""
        greeting = config.get("greeting", "Hello")

        return PluginInfo(
            id="{{ cookiecutter.plugin_id }}",
            name="{{ cookiecutter.project_name }}",
            version="0.1.0",
            description="{{ cookiecutter.project_description }}",
            author="{{ cookiecutter.author_name }}",
            min_engine_version="1.0.0",
            required_skills=[],
            config_schema='{"type":"object","properties":{"greeting":{"type":"string","default":"Hello"}}}',
            tags=["example", "tutorial"],
        )

    @classmethod
    def register(cls) -> List[str]:
        """Return the skill IDs this plugin provides."""
        return ["{{ cookiecutter.skill_id }}"]

    @classmethod
    def execute(cls, ctx: ActionContext) -> ActionResult:
        """Execute the plugin's core logic."""
        greeting = ctx.config.get("greeting", "Hello")

        agent_name = "stranger"
        if ctx.world.agent:
            agent_name = ctx.world.agent.name

        message = f"{greeting}, {agent_name}! (tick #{ctx.world.tick})"

        return ActionResult(
            success=True,
            message=message,
            mutations=[],
            events=[f'{{"type":"plugin_greeting","plugin":"{{ cookiecutter.plugin_id }}"}}'],
            data={"greeting": greeting},
            tokens_consumed=1,
        )

    @classmethod
    def cost_estimate(cls, ctx: ActionContext) -> TokenCost:
        """Estimate token cost before execution."""
        return TokenCost(
            estimated=1,
            confidence=1.0,
            breakdown="Fixed cost: 1 token per execution",
        )

    @classmethod
    def shutdown(cls) -> None:
        """Graceful shutdown. Override if cleanup is needed."""
        pass

    @classmethod
    def on_event(cls, event: str, ctx: WorldContext) -> Optional[ActionResult]:
        """Handle a world event. Override to react to events."""
        return None


# ─── WASM Entry Points (when use_wasm=yes) ──────────────────────────────
# These functions are called by the WASM runtime via ComponentizePy.

PLUGIN = {{ cookiecutter.project_slug.replace('_', ' ').title().replace(' ', '') }}Plugin


def init(config_json: str) -> str:
    import json
    config = json.loads(config_json)
    info = PLUGIN.init(config)
    return json.dumps(info.__dict__)


def register() -> str:
    import json
    return json.dumps(PLUGIN.register())


def execute(ctx_json: str) -> str:
    import json
    data = json.loads(ctx_json)
    world_data = data["world"]
    world = WorldContext(
        tick=world_data["tick"],
        agent=AgentSnapshot(**world_data["agent"]) if world_data.get("agent") else None,
        visible_agents=[AgentSnapshot(**a) for a in world_data.get("visible_agents", [])],
        globals=world_data.get("globals", {}),
        recent_events=world_data.get("recent_events", []),
    )
    ctx = ActionContext(
        world=world,
        params=data.get("params", {}),
        config=data.get("config", {}),
    )
    result = PLUGIN.execute(ctx)
    return json.dumps({
        "success": result.success,
        "message": result.message,
        "mutations": [{"kind": m.kind.value, "target_agent": m.target_agent, "field": m.field, "value": m.value} for m in result.mutations],
        "events": result.events,
        "data": result.data,
        "tokens_consumed": result.tokens_consumed,
    })


def cost_estimate(ctx_json: str) -> str:
    import json
    data = json.loads(ctx_json)
    world_data = data["world"]
    world = WorldContext(
        tick=world_data["tick"],
        agent=AgentSnapshot(**world_data["agent"]) if world_data.get("agent") else None,
        visible_agents=[AgentSnapshot(**a) for a in world_data.get("visible_agents", [])],
        globals=world_data.get("globals", {}),
        recent_events=world_data.get("recent_events", []),
    )
    ctx = ActionContext(
        world=world,
        params=data.get("params", {}),
        config=data.get("config", {}),
    )
    cost = PLUGIN.cost_estimate(ctx)
    return json.dumps({
        "estimated": cost.estimated,
        "confidence": cost.confidence,
        "breakdown": cost.breakdown,
    })


def shutdown() -> None:
    PLUGIN.shutdown()
