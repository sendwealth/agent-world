"""Tool abstract base class — defines the interface, parameter schema, and execution
protocol for all agent tools.

Tools are distinct from skills: skills are *learned capabilities* that level up with
XP, while tools are *stateless utilities* the agent can invoke to interact with the
outside world (HTTP requests, file I/O, code execution, etc.).

The base class is a Pydantic model so parameter schemas are automatically
serialisable and validated at creation time.
"""

from __future__ import annotations

import asyncio
import logging
from abc import ABC
from dataclasses import dataclass, field
from enum import Enum
from typing import Any, Dict

from pydantic import BaseModel

logger = logging.getLogger(__name__)

# ---------------------------------------------------------------------------
# Tool execution result
# ---------------------------------------------------------------------------


class ToolStatus(str, Enum):
    """Outcome of a tool invocation."""

    SUCCESS = "success"
    ERROR = "error"
    TIMEOUT = "timeout"
    PERMISSION_DENIED = "permission_denied"


@dataclass(frozen=True)
class ToolResult:
    """Immutable result of a tool invocation.

    Attributes:
        tool_name: Name of the tool that was invoked.
        status: Execution outcome.
        output: The return value (structure depends on the tool).
        error: Human-readable error message if status != SUCCESS.
        metadata: Extra structured data (latency_ms, status_code, etc.).
    """

    tool_name: str
    status: ToolStatus = ToolStatus.SUCCESS
    output: Any = None
    error: str = ""
    metadata: Dict[str, Any] = field(default_factory=dict)

    @property
    def success(self) -> bool:
        return self.status == ToolStatus.SUCCESS

    def to_context_text(self) -> str:
        """Render a compact text representation suitable for context injection."""
        if self.success:
            return f"[Tool:{self.tool_name}] {self.output}"
        return f"[Tool:{self.tool_name}] ERROR: {self.error}"


# ---------------------------------------------------------------------------
# Parameter schema (Pydantic model for each tool)
# ---------------------------------------------------------------------------


class ToolParameters(BaseModel):
    """Base parameter schema — each tool defines a subclass with its own fields."""

    model_config = {"extra": "allow"}  # allow passthrough kwargs


# ---------------------------------------------------------------------------
# Abstract tool
# ---------------------------------------------------------------------------


class Tool(ABC):
    """Abstract base class for all agent tools.

    Subclasses must implement:
        - ``parameters_schema``: the Pydantic model class for parameter validation.
        - ``execute``: the actual execution logic (sync or async).

    Optionally override:
        - ``validate_params``: custom pre-execution validation.
        - ``timeout``: per-tool timeout in seconds (default 30).
    """

    def __init__(self) -> None:
        self._name: str = self.__class__.__name__

    # ------------------------------------------------------------------
    # Properties (override in subclass or set via decorator)
    # ------------------------------------------------------------------

    @property
    def name(self) -> str:
        """Unique tool identifier."""
        return self._name

    @property
    def description(self) -> str:
        """One-line description of what the tool does."""
        return ""

    @property
    def category(self) -> str:
        """Grouping label (e.g. 'io', 'network', 'compute')."""
        return "general"

    @property
    def timeout(self) -> float:
        """Default timeout in seconds for this tool's execution."""
        return 30.0

    @property
    def requires_permission(self) -> bool:
        """If True the agent must have explicit permission to use this tool."""
        return False

    # ------------------------------------------------------------------
    # Parameter schema
    # ------------------------------------------------------------------

    @property
    def parameters_schema(self) -> type[ToolParameters]:
        """Return the Pydantic model class for this tool's parameters."""
        return ToolParameters

    def validate_params(self, params: Dict[str, Any]) -> ToolParameters:
        """Validate and coerce raw params into the schema model.

        Raises:
            pydantic.ValidationError: If params don't match the schema.
        """
        return self.parameters_schema.model_validate(params)

    def get_schema_dict(self) -> Dict[str, Any]:
        """Return a JSON-serialisable schema description (for LLM function-calling)."""
        return {
            "name": self.name,
            "description": self.description,
            "category": self.category,
            "parameters": self.parameters_schema.model_json_schema(),
            "requires_permission": self.requires_permission,
        }

    # ------------------------------------------------------------------
    # Execution (subclass must implement at least one)
    # ------------------------------------------------------------------

    def execute(self, params: ToolParameters) -> ToolResult:
        """Synchronous execution. Override in subclass.

        Default implementation raises NotImplementedError so async-only
        tools can skip defining this.
        """
        raise NotImplementedError(
            f"Tool '{self.name}' does not implement synchronous execute()"
        )

    async def execute_async(self, params: ToolParameters) -> ToolResult:
        """Asynchronous execution. Override in subclass.

        Default falls back to the synchronous execute() wrapped in
        asyncio.to_thread for backward compat.
        """
        return await asyncio.to_thread(self.execute, params)

    # ------------------------------------------------------------------
    # Convenience entry point
    # ------------------------------------------------------------------

    async def run(self, raw_params: Dict[str, Any]) -> ToolResult:
        """Validate params, execute the tool, and return a ToolResult.

        This is the primary entry point called by ToolRegistry.
        """
        # 1. Validate parameters
        try:
            params = self.validate_params(raw_params)
        except Exception as exc:
            return ToolResult(
                tool_name=self.name,
                status=ToolStatus.ERROR,
                error=f"Parameter validation failed: {exc}",
            )

        # 2. Execute with timeout
        try:
            result = await asyncio.wait_for(
                self.execute_async(params),
                timeout=self.timeout,
            )
            return result
        except asyncio.TimeoutError:
            return ToolResult(
                tool_name=self.name,
                status=ToolStatus.TIMEOUT,
                error=f"Tool timed out after {self.timeout}s",
            )
        except PermissionError as exc:
            return ToolResult(
                tool_name=self.name,
                status=ToolStatus.PERMISSION_DENIED,
                error=str(exc),
            )
        except Exception as exc:
            logger.exception("Tool '%s' execution failed", self.name)
            return ToolResult(
                tool_name=self.name,
                status=ToolStatus.ERROR,
                error=str(exc),
            )

    # ------------------------------------------------------------------
    # Dunder helpers
    # ------------------------------------------------------------------

    def __repr__(self) -> str:
        return f"Tool(name={self.name!r}, category={self.category!r})"
