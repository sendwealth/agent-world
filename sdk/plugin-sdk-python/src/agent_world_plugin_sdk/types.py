"""Agent World Plugin SDK — Type definitions.

All data types used by the plugin interface, serializable to/from JSON.
Mirrors the Rust types defined in the Plugin Interface Specification (v1.0.0-draft).
"""
from __future__ import annotations

import json
from dataclasses import asdict, dataclass, field
from enum import Enum
from typing import Any, Dict, List, Optional, Type, TypeVar

# ─── Type Aliases ────────────────────────────────────────────────────────

PluginId = str
"""Unique plugin identifier in ``"<author>/<plugin-name>"`` format."""

SemVer = str
"""Semantic version string (MAJOR.MINOR.PATCH)."""

SkillId = str
"""Identifier for a built-in or registered skill."""

AgentId = str
"""Unique identifier for an agent in the simulation."""

T = TypeVar("T", bound="JsonSerializable")

# ─── Mixin for JSON serialization ────────────────────────────────────────


class JsonSerializable:
    """Mixin providing ``to_dict`` / ``to_json`` / ``from_dict`` / ``from_json``."""

    def to_dict(self) -> Dict[str, Any]:
        """Recursively convert to a JSON-compatible dict."""
        return _deep_asdict(self)

    def to_json(self, **kwargs: Any) -> str:
        """Serialize to a JSON string."""
        return json.dumps(self.to_dict(), **kwargs)

    @classmethod
    def from_dict(cls: Type[T], data: Dict[str, Any]) -> T:
        """Construct an instance from a plain dict."""
        return _construct(cls, data)

    @classmethod
    def from_json(cls: Type[T], json_str: str) -> T:
        """Construct an instance from a JSON string."""
        return cls.from_dict(json.loads(json_str))


# ─── Data Classes ────────────────────────────────────────────────────────


@dataclass
class PluginInfo(JsonSerializable):
    """Metadata returned from ``init()``."""

    id: PluginId
    name: str
    version: SemVer
    description: str
    author: str
    min_engine_version: SemVer
    required_skills: List[SkillId] = field(default_factory=list)
    config_schema: Optional[str] = None
    tags: List[str] = field(default_factory=list)


@dataclass
class AgentSnapshot(JsonSerializable):
    """Read-only snapshot of an agent's public state."""

    id: AgentId
    name: str
    phase: str
    money: int
    tokens: int
    reputation: float
    skills: Dict[str, int] = field(default_factory=dict)
    alive: bool = True
    age: int = 0


@dataclass
class WorldContext(JsonSerializable):
    """Read-only world state snapshot provided to the plugin."""

    tick: int
    agent: Optional[AgentSnapshot] = None
    visible_agents: List[AgentSnapshot] = field(default_factory=list)
    globals: Dict[str, str] = field(default_factory=dict)
    recent_events: List[str] = field(default_factory=list)


@dataclass
class ActionContext(JsonSerializable):
    """Full context passed to ``execute()`` and ``cost_estimate()``."""

    world: WorldContext
    params: Dict[str, str] = field(default_factory=dict)
    config: Dict[str, str] = field(default_factory=dict)


class MutationKind(Enum):
    """Kinds of state mutations a plugin can request."""

    CREDIT_TOKENS = "credit_tokens"
    DEBIT_TOKENS = "debit_tokens"
    CREDIT_MONEY = "credit_money"
    DEBIT_MONEY = "debit_money"
    SET_SKILL = "set_skill"
    ADJUST_REPUTATION = "adjust_reputation"
    SET_GLOBAL = "set_global"
    EMIT_EVENT = "emit_event"


@dataclass
class StateMutation(JsonSerializable):
    """A state mutation requested by the plugin.

    The engine validates and applies these; plugins cannot directly
    mutate world state.
    """

    kind: MutationKind
    target_agent: Optional[AgentId] = None
    field: str = ""
    value: str = ""


@dataclass
class ActionResult(JsonSerializable):
    """Result of ``execute()``."""

    success: bool
    message: str
    mutations: List[StateMutation] = field(default_factory=list)
    events: List[str] = field(default_factory=list)
    data: Dict[str, str] = field(default_factory=dict)
    tokens_consumed: int = 0


@dataclass
class TokenCost(JsonSerializable):
    """Token cost estimate returned from ``cost_estimate()``."""

    estimated: int
    confidence: float = 1.0
    breakdown: Optional[str] = None


class PluginError(Exception):
    """Error raised by plugin operations.

    Mirrors the Rust ``PluginError`` enum with well-known error codes
    from the specification (§4 Error Handling).
    """

    # Well-known error codes from the spec
    INIT_FAILED = "init_failed"
    EXECUTION_FAILED = "execution_failed"
    CONFIG_ERROR = "config_error"
    MISSING_SKILL = "missing_skill"
    COST_ESTIMATE_FAILED = "cost_estimate_failed"
    INVALID_STATE = "invalid_state"
    CUSTOM = "custom"

    def __init__(self, code: str = "custom", message: str = ""):
        self.code = code
        self.message = message
        super().__init__(f"[{code}] {message}")

    def to_dict(self) -> Dict[str, str]:
        """Serialize to dict."""
        return {"code": self.code, "message": self.message}

    def to_json(self, **kwargs: Any) -> str:
        """Serialize to JSON."""
        return json.dumps(self.to_dict(), **kwargs)

    @classmethod
    def from_dict(cls, data: Dict[str, str]) -> "PluginError":
        """Construct from a dict."""
        return cls(code=data.get("code", "custom"), message=data.get("message", ""))

    @classmethod
    def from_json(cls, json_str: str) -> "PluginError":
        """Construct from a JSON string."""
        return cls.from_dict(json.loads(json_str))


# ─── Serialization Helpers ───────────────────────────────────────────────


def _deep_asdict(obj: Any) -> Any:
    """Recursively convert dataclasses, enums, and nested structures."""
    if isinstance(obj, Enum):
        return obj.value
    if hasattr(obj, "__dataclass_fields__"):
        return {k: _deep_asdict(v) for k, v in asdict(obj).items()}  # type: ignore[arg-type]
    if isinstance(obj, list):
        return [_deep_asdict(v) for v in obj]
    if isinstance(obj, dict):
        return {k: _deep_asdict(v) for k, v in obj.items()}
    return obj


def _construct(cls: Type[T], data: Dict[str, Any]) -> T:
    """Construct a dataclass from a dict, handling nested types."""
    import dataclasses
    import typing

    if not dataclasses.is_dataclass(cls):
        raise TypeError(f"{cls} is not a dataclass")

    # Use get_type_hints to properly resolve forward refs and string annotations
    try:
        type_hints = typing.get_type_hints(cls)
    except Exception:
        type_hints = {f.name: f.type for f in dataclasses.fields(cls)}

    fields = dataclasses.fields(cls)
    kwargs: Dict[str, Any] = {}

    for f in fields:
        name = f.name
        if name not in data:
            continue

        raw = data[name]
        field_type = type_hints.get(name, f.type)

        kwargs[name] = _coerce(field_type, raw)

    return cls(**kwargs)  # type: ignore[call-arg]


def _coerce(field_type: Any, value: Any) -> Any:
    """Coerce a raw JSON value to the expected Python type."""
    if value is None:
        return None

    origin = getattr(field_type, "__origin__", None)

    # Handle Optional[X]
    if origin is Union:
        args = [a for a in field_type.__args__ if a is not type(None)]
        if len(args) == 1:
            return _coerce(args[0], value)
        return value

    # Dataclass types
    import dataclasses

    if dataclasses.is_dataclass(field_type) and isinstance(value, dict):
        return _construct(field_type, value)

    # List[X] — recurse on elements
    if origin is list:
        args = getattr(field_type, "__args__", None)
        if args and isinstance(value, list):
            return [_coerce(args[0], v) for v in value]
        return value

    # Dict[K, V]
    if origin is dict:
        return value

    # Enum types
    if isinstance(field_type, type) and issubclass(field_type, Enum):
        return field_type(value)

    return value


# Late import for _coerce
import sys
from typing import Union
