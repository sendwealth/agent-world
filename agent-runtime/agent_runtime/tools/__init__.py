"""Agent Runtime Tools Framework — pluggable utilities for agent-world agents.

Tools are distinct from skills: skills are *learned capabilities* that level up
with XP, while tools are *stateless utilities* the agent can invoke to interact
with the outside world (HTTP requests, file I/O, code execution, etc.).

Provides:
- Tool: Abstract base class for all tools.
- ToolResult: Immutable result of a tool invocation.
- ToolParameters: Base Pydantic model for parameter validation.
- ToolRegistry: Central registry for registration, discovery, and invocation.
- Built-in tools: http_request, file_ops, code_exec, governance, task,
  organization, diplomacy, investment, legislation, bank, stocks, marketplace,
  reputation, trust, escrow.

Integration points:
- **Think Loop**: The decide step can choose to invoke a tool via the registry.
- **Context Engine**: Tool results can be injected as context items for LLM prompts.
"""

from __future__ import annotations

from typing import Optional, Set

from .base import Tool, ToolParameters, ToolResult, ToolStatus
from .builtin import (
    BankTool,
    CodeExecTool,
    DiplomacyTool,
    EscrowTool,
    FileOpsTool,
    GovernanceTool,
    HttpRequestTool,
    InvestmentTool,
    LegislationTool,
    MarketplaceTool,
    OrganizationTool,
    ReputationTool,
    StocksTool,
    TaskTool,
    TrustTool,
    create_builtin_tools,
)
from .registry import ToolRegistry

# Convenience: pre-built registry with all built-in tools


def create_registry_with_builtins(
    *,
    sandbox_http: bool = True,
    file_ops_base_dir: Optional[str] = None,
    allowed_tools: Optional[Set[str]] = None,
    world_engine_url: Optional[str] = None,
    sandbox_world_engine: bool = True,
) -> ToolRegistry:
    """Create a ToolRegistry pre-loaded with all built-in tools.

    Args:
        sandbox_http: If True, HTTP tool returns simulated responses.
        file_ops_base_dir: Base directory for file operations.
        allowed_tools: If provided, only these tool names may be invoked.
        world_engine_url: Base URL for the World Engine API.
        sandbox_world_engine: If True, world-engine tools return simulated responses.

    Returns:
        A ToolRegistry with built-in tools registered.
    """
    registry = ToolRegistry(allowed_tools=allowed_tools)
    for tool in create_builtin_tools(
        sandbox_http=sandbox_http,
        file_ops_base_dir=file_ops_base_dir,
        world_engine_url=world_engine_url,
        sandbox_world_engine=sandbox_world_engine,
    ):
        registry.register(tool)
    return registry


__all__ = [
    # Base
    "Tool",
    "ToolParameters",
    "ToolResult",
    "ToolStatus",
    # Registry
    "ToolRegistry",
    # Built-in tools — original
    "HttpRequestTool",
    "FileOpsTool",
    "CodeExecTool",
    # Built-in tools — P0 (agent survival)
    "GovernanceTool",
    "TaskTool",
    "OrganizationTool",
    # Built-in tools — P1
    "DiplomacyTool",
    "InvestmentTool",
    "LegislationTool",
    # Built-in tools — P2 (remaining subsystems)
    "BankTool",
    "StocksTool",
    "MarketplaceTool",
    "ReputationTool",
    "TrustTool",
    "EscrowTool",
    # Factory functions
    "create_builtin_tools",
    "create_registry_with_builtins",
]
