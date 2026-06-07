"""Built-in tools for the Agent Runtime tools framework.

Provides:
- HttpRequestTool: Make HTTP requests (sandbox mode by default)
- FileOpsTool: Read/write/list/delete files within a sandboxed directory
- CodeExecTool: Execute Python code snippets (requires permission)
- GovernanceTool: Proposals, voting, governance metrics
- TaskTool: Create, claim, submit, review tasks
- OrganizationTool: Create, join, leave, dissolve organizations
- DiplomacyTool: Cross-world treaties, sanctions, war, peace
- InvestmentTool: Investment products, buy/sell shares, portfolios
- LegislationTool: Legislation cycles, rules, voting, repeal
- BankTool: Bank accounts, deposits, withdrawals, loans
- StocksTool: Stock listings, IPOs, buy/sell orders, dividends
- MarketplaceTool: Knowledge marketplace listings, purchases, ratings
- ReputationTool: Reputation scores, rankings
- TrustTool: Trust interactions, scores, allies/enemies
- EscrowTool: Escrow contracts, disputes, resolution
"""

from __future__ import annotations

from typing import List, Optional

from ..base import Tool
from .bank import BankTool
from .code_exec import CodeExecTool
from .diplomacy import DiplomacyTool
from .escrow import EscrowTool
from .file_ops import FileOpsTool
from .governance import GovernanceTool
from .http_request import HttpRequestTool
from .investment import InvestmentTool
from .legislation import LegislationTool
from .marketplace import MarketplaceTool
from .organization import OrganizationTool
from .reputation import ReputationTool
from .stocks import StocksTool
from .task import TaskTool
from .trust import TrustTool

__all__ = [
    # Original tools
    "HttpRequestTool",
    "FileOpsTool",
    "CodeExecTool",
    # P0 tools (agent survival)
    "GovernanceTool",
    "TaskTool",
    "OrganizationTool",
    # P1 tools
    "DiplomacyTool",
    "InvestmentTool",
    "LegislationTool",
    # P2 tools (remaining subsystems)
    "BankTool",
    "StocksTool",
    "MarketplaceTool",
    "ReputationTool",
    "TrustTool",
    "EscrowTool",
]


def create_builtin_tools(
    *,
    sandbox_http: bool = True,
    file_ops_base_dir: Optional[str] = None,
    world_engine_url: Optional[str] = None,
    sandbox_world_engine: bool = True,
) -> List[Tool]:
    """Create instances of all built-in tools.

    Args:
        sandbox_http: If True (default), HTTP tool returns simulated responses.
        file_ops_base_dir: Base directory for file operations. Defaults to
            ``.agent_workspace`` in the current working directory.
        world_engine_url: Base URL for the World Engine API. Defaults to
            ``http://localhost:3000``.
        sandbox_world_engine: If True (default), world-engine tools return
            simulated responses without making network calls.

    Returns:
        List of Tool instances ready to register.
    """
    engine_url = world_engine_url or "http://localhost:3000"

    return [
        # Original tools
        HttpRequestTool(sandbox=sandbox_http),
        FileOpsTool(base_dir=file_ops_base_dir),
        CodeExecTool(),
        # P0 tools (agent survival)
        GovernanceTool(base_url=engine_url, sandbox=sandbox_world_engine),
        TaskTool(base_url=engine_url, sandbox=sandbox_world_engine),
        OrganizationTool(base_url=engine_url, sandbox=sandbox_world_engine),
        # P1 tools
        DiplomacyTool(base_url=engine_url, sandbox=sandbox_world_engine),
        InvestmentTool(base_url=engine_url, sandbox=sandbox_world_engine),
        LegislationTool(base_url=engine_url, sandbox=sandbox_world_engine),
        # P2 tools (remaining subsystems)
        BankTool(base_url=engine_url, sandbox=sandbox_world_engine),
        StocksTool(base_url=engine_url, sandbox=sandbox_world_engine),
        MarketplaceTool(base_url=engine_url, sandbox=sandbox_world_engine),
        ReputationTool(base_url=engine_url, sandbox=sandbox_world_engine),
        TrustTool(base_url=engine_url, sandbox=sandbox_world_engine),
        EscrowTool(base_url=engine_url, sandbox=sandbox_world_engine),
    ]
