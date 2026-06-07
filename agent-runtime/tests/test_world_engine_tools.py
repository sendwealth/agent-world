"""Tests for world-engine built-in tools.

Covers all new tools: governance, task, organization, diplomacy, investment,
legislation, bank, stocks, marketplace, reputation, trust, escrow.

All tests run in sandbox mode (no real HTTP calls).
"""

from __future__ import annotations

import asyncio
import json

import pytest

from agent_runtime.tools import (
    BankTool,
    DiplomacyTool,
    EscrowTool,
    GovernanceTool,
    InvestmentTool,
    LegislationTool,
    MarketplaceTool,
    OrganizationTool,
    ReputationTool,
    StocksTool,
    TaskTool,
    ToolRegistry,
    ToolStatus,
    TrustTool,
    create_builtin_tools,
    create_registry_with_builtins,
)


# ============================================================
# Async helper
# ============================================================


def _run_async(coro):
    """Run an async coroutine safely."""
    loop = asyncio.new_event_loop()
    try:
        return loop.run_until_complete(coro)
    finally:
        loop.close()


def _sandbox_tool(tool_cls, **kwargs):
    """Create a tool instance in sandbox mode."""
    return tool_cls(sandbox=True, **kwargs)


# ============================================================
# GovernanceTool tests
# ============================================================


class TestGovernanceTool:
    def setup_method(self):
        self.tool = _sandbox_tool(GovernanceTool)

    def test_name_and_category(self):
        assert self.tool.name == "governance"
        assert self.tool.category == "governance"

    def test_create_proposal(self):
        result = _run_async(self.tool.run({
            "action": "create_proposal",
            "org_id": "org-1",
            "proposal_type": "amend_charter",
            "title": "Update charter",
        }))
        assert result.success
        assert result.output["sandbox"] is True
        assert result.output["action"] == "create_proposal"

    def test_vote(self):
        result = _run_async(self.tool.run({
            "action": "vote",
            "proposal_id": "prop-1",
            "vote": "for",
            "voter_id": "agent-1",
        }))
        assert result.success

    def test_start_voting(self):
        result = _run_async(self.tool.run({
            "action": "start_voting",
            "proposal_id": "prop-1",
        }))
        assert result.success

    def test_tally(self):
        result = _run_async(self.tool.run({
            "action": "tally",
            "proposal_id": "prop-1",
        }))
        assert result.success

    def test_cancel_proposal(self):
        result = _run_async(self.tool.run({
            "action": "cancel_proposal",
            "proposal_id": "prop-1",
        }))
        assert result.success

    def test_add_argument(self):
        result = _run_async(self.tool.run({
            "action": "add_argument",
            "proposal_id": "prop-1",
            "argument": "I support this",
            "argument_side": "for",
        }))
        assert result.success

    def test_list_proposals(self):
        result = _run_async(self.tool.run({
            "action": "list_proposals",
            "org_id": "org-1",
        }))
        assert result.success

    def test_get_proposal(self):
        result = _run_async(self.tool.run({
            "action": "get_proposal",
            "proposal_id": "prop-1",
        }))
        assert result.success

    def test_summary(self):
        result = _run_async(self.tool.run({"action": "summary"}))
        assert result.success

    def test_org_metrics(self):
        result = _run_async(self.tool.run({
            "action": "org_metrics",
            "org_id": "org-1",
        }))
        assert result.success

    def test_timeline(self):
        result = _run_async(self.tool.run({
            "action": "timeline",
            "org_id": "org-1",
        }))
        assert result.success

    def test_comparison(self):
        result = _run_async(self.tool.run({
            "action": "comparison",
            "org_ids": ["org-1", "org-2"],
        }))
        assert result.success

    def test_list_legislation(self):
        result = _run_async(self.tool.run({
            "action": "list_legislation",
            "org_id": "org-1",
            "status": "enacted",
        }))
        assert result.success

    def test_unknown_action(self):
        result = _run_async(self.tool.run({"action": "invalid"}))
        assert result.status == ToolStatus.ERROR
        assert "Unknown" in result.error

    def test_schema(self):
        schema = self.tool.get_schema_dict()
        assert schema["name"] == "governance"
        assert "action" in schema["parameters"]["properties"]


# ============================================================
# TaskTool tests
# ============================================================


class TestTaskTool:
    def setup_method(self):
        self.tool = _sandbox_tool(TaskTool)

    def test_name_and_category(self):
        assert self.tool.name == "task"
        assert self.tool.category == "task"

    def test_create(self):
        result = _run_async(self.tool.run({
            "action": "create",
            "title": "Gather resources",
            "description": "Collect 10 wood",
            "reward": 50.0,
        }))
        assert result.success
        assert result.output["sandbox"] is True

    def test_list(self):
        result = _run_async(self.tool.run({"action": "list"}))
        assert result.success

    def test_get(self):
        result = _run_async(self.tool.run({
            "action": "get",
            "task_id": "task-1",
        }))
        assert result.success

    def test_claim(self):
        result = _run_async(self.tool.run({
            "action": "claim",
            "task_id": "task-1",
            "claimer_id": "agent-1",
        }))
        assert result.success

    def test_start(self):
        result = _run_async(self.tool.run({
            "action": "start",
            "task_id": "task-1",
        }))
        assert result.success

    def test_submit(self):
        result = _run_async(self.tool.run({
            "action": "submit",
            "task_id": "task-1",
            "result_data": "completed",
        }))
        assert result.success

    def test_review(self):
        result = _run_async(self.tool.run({
            "action": "review",
            "task_id": "task-1",
            "approved": True,
            "rating": 5,
        }))
        assert result.success

    def test_complete(self):
        result = _run_async(self.tool.run({
            "action": "complete",
            "task_id": "task-1",
        }))
        assert result.success

    def test_delete(self):
        result = _run_async(self.tool.run({
            "action": "delete",
            "task_id": "task-1",
        }))
        assert result.success

    def test_list_coordination(self):
        result = _run_async(self.tool.run({"action": "list_coordination"}))
        assert result.success

    def test_join_coordination(self):
        result = _run_async(self.tool.run({
            "action": "join_coordination",
            "coordination_task_id": "ct-1",
            "claimer_id": "agent-1",
        }))
        assert result.success

    def test_contribute_coordination(self):
        result = _run_async(self.tool.run({
            "action": "contribute_coordination",
            "coordination_task_id": "ct-1",
            "contribution_data": "partial work",
            "contributor_id": "agent-1",
        }))
        assert result.success

    def test_unknown_action(self):
        result = _run_async(self.tool.run({"action": "invalid"}))
        assert result.status == ToolStatus.ERROR

    def test_schema(self):
        schema = self.tool.get_schema_dict()
        assert schema["name"] == "task"
        assert "action" in schema["parameters"]["properties"]


# ============================================================
# OrganizationTool tests
# ============================================================


class TestOrganizationTool:
    def setup_method(self):
        self.tool = _sandbox_tool(OrganizationTool)

    def test_name_and_category(self):
        assert self.tool.name == "organization"
        assert self.tool.category == "organization"

    def test_create(self):
        result = _run_async(self.tool.run({
            "action": "create",
            "name": "Traders Guild",
            "org_type": "guild",
            "description": "A guild for trading",
        }))
        assert result.success

    def test_list(self):
        result = _run_async(self.tool.run({"action": "list"}))
        assert result.success

    def test_get(self):
        result = _run_async(self.tool.run({
            "action": "get",
            "org_id": "org-1",
        }))
        assert result.success

    def test_join(self):
        result = _run_async(self.tool.run({
            "action": "join",
            "org_id": "org-1",
            "agent_id": "agent-1",
        }))
        assert result.success

    def test_leave(self):
        result = _run_async(self.tool.run({
            "action": "leave",
            "org_id": "org-1",
            "agent_id": "agent-1",
        }))
        assert result.success

    def test_dissolve(self):
        result = _run_async(self.tool.run({
            "action": "dissolve",
            "org_id": "org-1",
        }))
        assert result.success

    def test_distribution(self):
        result = _run_async(self.tool.run({
            "action": "distribution",
            "org_id": "org-1",
        }))
        assert result.success

    def test_unknown_action(self):
        result = _run_async(self.tool.run({"action": "invalid"}))
        assert result.status == ToolStatus.ERROR

    def test_schema(self):
        schema = self.tool.get_schema_dict()
        assert schema["name"] == "organization"
        assert schema["category"] == "organization"


# ============================================================
# DiplomacyTool tests
# ============================================================


class TestDiplomacyTool:
    def setup_method(self):
        self.tool = _sandbox_tool(DiplomacyTool)

    def test_name_and_category(self):
        assert self.tool.name == "diplomacy"
        assert self.tool.category == "diplomacy"

    def test_register_world(self):
        result = _run_async(self.tool.run({
            "action": "register_world",
            "world_name": "Earth-2",
            "world_url": "http://earth2:3000",
        }))
        assert result.success

    def test_list_worlds(self):
        result = _run_async(self.tool.run({"action": "list_worlds"}))
        assert result.success

    def test_propose_treaty(self):
        result = _run_async(self.tool.run({
            "action": "propose_treaty",
            "treaty_type": "trade_pact",
            "from_world_id": "w-1",
            "to_world_id": "w-2",
            "terms": "Free trade of knowledge",
        }))
        assert result.success

    def test_accept_treaty(self):
        result = _run_async(self.tool.run({
            "action": "accept_treaty",
            "treaty_id": "treaty-1",
        }))
        assert result.success

    def test_reject_treaty(self):
        result = _run_async(self.tool.run({
            "action": "reject_treaty",
            "treaty_id": "treaty-1",
        }))
        assert result.success

    def test_break_treaty(self):
        result = _run_async(self.tool.run({
            "action": "break_treaty",
            "treaty_id": "treaty-1",
        }))
        assert result.success

    def test_impose_sanctions(self):
        result = _run_async(self.tool.run({
            "action": "impose_sanctions",
            "from_world_id": "w-1",
            "target_world_id": "w-2",
            "sanction_type": "trade_embargo",
        }))
        assert result.success

    def test_declare_war(self):
        result = _run_async(self.tool.run({
            "action": "declare_war",
            "aggressor_world_id": "w-1",
            "defender_world_id": "w-2",
            "reason": "Territorial dispute",
        }))
        assert result.success

    def test_propose_peace(self):
        result = _run_async(self.tool.run({
            "action": "propose_peace",
            "from_world_id": "w-1",
            "to_world_id": "w-2",
            "peace_terms": "Status quo ante",
        }))
        assert result.success

    def test_summary(self):
        result = _run_async(self.tool.run({"action": "summary"}))
        assert result.success

    def test_unknown_action(self):
        result = _run_async(self.tool.run({"action": "invalid"}))
        assert result.status == ToolStatus.ERROR

    def test_schema(self):
        schema = self.tool.get_schema_dict()
        assert schema["name"] == "diplomacy"
        assert schema["category"] == "diplomacy"


# ============================================================
# InvestmentTool tests
# ============================================================


class TestInvestmentTool:
    def setup_method(self):
        self.tool = _sandbox_tool(InvestmentTool)

    def test_name_and_category(self):
        assert self.tool.name == "investment"
        assert self.tool.category == "economy"

    def test_create_product(self):
        result = _run_async(self.tool.run({
            "action": "create_product",
            "product_name": "Tech Fund",
            "product_type": "fund",
            "min_investment": 10.0,
            "risk_level": "medium",
        }))
        assert result.success

    def test_list_products(self):
        result = _run_async(self.tool.run({"action": "list_products"}))
        assert result.success

    def test_buy(self):
        result = _run_async(self.tool.run({
            "action": "buy",
            "product_id": "prod-1",
            "investor_id": "agent-1",
            "amount": 100.0,
        }))
        assert result.success

    def test_sell(self):
        result = _run_async(self.tool.run({
            "action": "sell",
            "product_id": "prod-1",
            "investor_id": "agent-1",
            "shares": 5.0,
        }))
        assert result.success

    def test_portfolio(self):
        result = _run_async(self.tool.run({
            "action": "portfolio",
            "investor_id": "agent-1",
        }))
        assert result.success

    def test_leaderboard(self):
        result = _run_async(self.tool.run({"action": "leaderboard"}))
        assert result.success

    def test_close_product(self):
        result = _run_async(self.tool.run({
            "action": "close_product",
            "product_id": "prod-1",
        }))
        assert result.success

    def test_update_performance(self):
        result = _run_async(self.tool.run({
            "action": "update_performance",
            "product_id": "prod-1",
            "performance_score": 0.85,
        }))
        assert result.success

    def test_freeze_product(self):
        result = _run_async(self.tool.run({
            "action": "freeze_product",
            "product_id": "prod-1",
        }))
        assert result.success

    def test_list_transactions(self):
        result = _run_async(self.tool.run({"action": "list_transactions"}))
        assert result.success

    def test_list_dividends(self):
        result = _run_async(self.tool.run({"action": "list_dividends"}))
        assert result.success

    def test_unknown_action(self):
        result = _run_async(self.tool.run({"action": "invalid"}))
        assert result.status == ToolStatus.ERROR


# ============================================================
# LegislationTool tests
# ============================================================


class TestLegislationTool:
    def setup_method(self):
        self.tool = _sandbox_tool(LegislationTool)

    def test_name_and_category(self):
        assert self.tool.name == "legislation"
        assert self.tool.category == "governance"

    def test_start_cycle(self):
        result = _run_async(self.tool.run({
            "action": "start_cycle",
            "org_id": "org-1",
        }))
        assert result.success

    def test_start_cycle_with_leader(self):
        result = _run_async(self.tool.run({
            "action": "start_cycle_with_leader",
            "org_id": "org-1",
            "leader_id": "agent-1",
        }))
        assert result.success

    def test_full_cycle(self):
        result = _run_async(self.tool.run({
            "action": "full_cycle",
            "org_id": "org-1",
        }))
        assert result.success

    def test_list_active(self):
        result = _run_async(self.tool.run({"action": "list_active"}))
        assert result.success

    def test_list_completed(self):
        result = _run_async(self.tool.run({"action": "list_completed"}))
        assert result.success

    def test_get_cycle(self):
        result = _run_async(self.tool.run({
            "action": "get_cycle",
            "org_id": "org-1",
        }))
        assert result.success

    def test_submit_rule(self):
        result = _run_async(self.tool.run({
            "action": "submit_rule",
            "org_id": "org-1",
            "rule_type": "tax",
            "rule_name": "Flat Tax",
            "rule_description": "10% flat tax",
            "proposer_id": "agent-1",
        }))
        assert result.success

    def test_start_voting(self):
        result = _run_async(self.tool.run({
            "action": "start_voting",
            "org_id": "org-1",
        }))
        assert result.success

    def test_cast_vote(self):
        result = _run_async(self.tool.run({
            "action": "cast_vote",
            "org_id": "org-1",
            "voter_id": "agent-1",
            "vote": "for",
            "rule_id": "rule-1",
        }))
        assert result.success

    def test_tally(self):
        result = _run_async(self.tool.run({
            "action": "tally",
            "org_id": "org-1",
        }))
        assert result.success

    def test_effects(self):
        result = _run_async(self.tool.run({
            "action": "effects",
            "org_id": "org-1",
        }))
        assert result.success

    def test_repeal(self):
        result = _run_async(self.tool.run({
            "action": "repeal",
            "org_id": "org-1",
            "rule_id": "rule-1",
            "repeal_reason": "No longer needed",
        }))
        assert result.success

    def test_unknown_action(self):
        result = _run_async(self.tool.run({"action": "invalid"}))
        assert result.status == ToolStatus.ERROR


# ============================================================
# BankTool tests
# ============================================================


class TestBankTool:
    def setup_method(self):
        self.tool = _sandbox_tool(BankTool)

    def test_name_and_category(self):
        assert self.tool.name == "bank"
        assert self.tool.category == "economy"

    def test_open_account(self):
        result = _run_async(self.tool.run({
            "action": "open_account",
            "agent_id": "agent-1",
            "account_type": "savings",
        }))
        assert result.success

    def test_list_accounts(self):
        result = _run_async(self.tool.run({"action": "list_accounts"}))
        assert result.success

    def test_get_account(self):
        result = _run_async(self.tool.run({
            "action": "get_account",
            "account_id": "acc-1",
        }))
        assert result.success

    def test_deposit(self):
        result = _run_async(self.tool.run({
            "action": "deposit",
            "account_id": "acc-1",
            "amount": 100.0,
        }))
        assert result.success

    def test_withdraw(self):
        result = _run_async(self.tool.run({
            "action": "withdraw",
            "account_id": "acc-1",
            "amount": 50.0,
        }))
        assert result.success

    def test_apply_loan(self):
        result = _run_async(self.tool.run({
            "action": "apply_loan",
            "agent_id": "agent-1",
            "loan_amount": 500.0,
            "loan_purpose": "expansion",
            "term_ticks": 100,
        }))
        assert result.success

    def test_approve_loan(self):
        result = _run_async(self.tool.run({
            "action": "approve_loan",
            "loan_id": "loan-1",
        }))
        assert result.success

    def test_repay_loan(self):
        result = _run_async(self.tool.run({
            "action": "repay_loan",
            "loan_id": "loan-1",
            "amount": 50.0,
        }))
        assert result.success

    def test_set_rates(self):
        result = _run_async(self.tool.run({
            "action": "set_rates",
            "savings_rate": 0.02,
            "loan_rate": 0.05,
        }))
        assert result.success

    def test_mint(self):
        result = _run_async(self.tool.run({
            "action": "mint",
            "mint_amount": 1000.0,
        }))
        assert result.success

    def test_stats(self):
        result = _run_async(self.tool.run({"action": "stats"}))
        assert result.success

    def test_unknown_action(self):
        result = _run_async(self.tool.run({"action": "invalid"}))
        assert result.status == ToolStatus.ERROR


# ============================================================
# StocksTool tests
# ============================================================


class TestStocksTool:
    def setup_method(self):
        self.tool = _sandbox_tool(StocksTool)

    def test_name_and_category(self):
        assert self.tool.name == "stocks"
        assert self.tool.category == "economy"

    def test_list_stocks(self):
        result = _run_async(self.tool.run({"action": "list_stocks"}))
        assert result.success

    def test_issue_shares(self):
        result = _run_async(self.tool.run({
            "action": "issue_shares",
            "org_id": "org-1",
            "symbol": "TRD",
            "total_shares": 1000,
            "initial_price": 10.0,
        }))
        assert result.success

    def test_get_stock(self):
        result = _run_async(self.tool.run({
            "action": "get_stock",
            "stock_id": "stock-1",
        }))
        assert result.success

    def test_perform_ipo(self):
        result = _run_async(self.tool.run({
            "action": "perform_ipo",
            "stock_id": "stock-1",
        }))
        assert result.success

    def test_distribute_dividend(self):
        result = _run_async(self.tool.run({
            "action": "distribute_dividend",
            "stock_id": "stock-1",
            "dividend_per_share": 0.5,
        }))
        assert result.success

    def test_buy_order(self):
        result = _run_async(self.tool.run({
            "action": "buy_order",
            "stock_id": "stock-1",
            "agent_id": "agent-1",
            "quantity": 10,
            "order_type": "limit",
            "price": 12.0,
        }))
        assert result.success

    def test_sell_order(self):
        result = _run_async(self.tool.run({
            "action": "sell_order",
            "stock_id": "stock-1",
            "agent_id": "agent-1",
            "quantity": 5,
            "order_type": "market",
        }))
        assert result.success

    def test_cancel_order(self):
        result = _run_async(self.tool.run({
            "action": "cancel_order",
            "order_id": "order-1",
        }))
        assert result.success

    def test_unknown_action(self):
        result = _run_async(self.tool.run({"action": "invalid"}))
        assert result.status == ToolStatus.ERROR


# ============================================================
# MarketplaceTool tests
# ============================================================


class TestMarketplaceTool:
    def setup_method(self):
        self.tool = _sandbox_tool(MarketplaceTool)

    def test_name_and_category(self):
        assert self.tool.name == "marketplace"
        assert self.tool.category == "economy"

    def test_publish(self):
        result = _run_async(self.tool.run({
            "action": "publish",
            "title": "Advanced Trading Algorithm",
            "description": "A proven algorithm",
            "listing_type": "algorithm",
            "price": 100.0,
            "tags": ["trading", "ai"],
        }))
        assert result.success

    def test_search(self):
        result = _run_async(self.tool.run({
            "action": "search",
            "query": "trading",
            "category": "algorithm",
        }))
        assert result.success

    def test_get_listing(self):
        result = _run_async(self.tool.run({
            "action": "get_listing",
            "listing_id": "list-1",
        }))
        assert result.success

    def test_update_listing(self):
        result = _run_async(self.tool.run({
            "action": "update_listing",
            "listing_id": "list-1",
            "price": 80.0,
        }))
        assert result.success

    def test_delist(self):
        result = _run_async(self.tool.run({
            "action": "delist",
            "listing_id": "list-1",
        }))
        assert result.success

    def test_purchase(self):
        result = _run_async(self.tool.run({
            "action": "purchase",
            "listing_id": "list-1",
            "agent_id": "agent-1",
        }))
        assert result.success

    def test_rate(self):
        result = _run_async(self.tool.run({
            "action": "rate",
            "listing_id": "list-1",
            "rating": 5,
            "review": "Excellent",
            "agent_id": "agent-1",
        }))
        assert result.success

    def test_list_ratings(self):
        result = _run_async(self.tool.run({
            "action": "list_ratings",
            "listing_id": "list-1",
        }))
        assert result.success

    def test_get_balance(self):
        result = _run_async(self.tool.run({
            "action": "get_balance",
            "agent_id": "agent-1",
        }))
        assert result.success

    def test_transfer(self):
        result = _run_async(self.tool.run({
            "action": "transfer",
            "agent_id": "agent-1",
            "to_agent_id": "agent-2",
            "transfer_amount": 25.0,
        }))
        assert result.success

    def test_unknown_action(self):
        result = _run_async(self.tool.run({"action": "invalid"}))
        assert result.status == ToolStatus.ERROR


# ============================================================
# ReputationTool tests
# ============================================================


class TestReputationTool:
    def setup_method(self):
        self.tool = _sandbox_tool(ReputationTool)

    def test_name_and_category(self):
        assert self.tool.name == "reputation"
        assert self.tool.category == "social"

    def test_get_score(self):
        result = _run_async(self.tool.run({
            "action": "get_score",
            "agent_id": "agent-1",
        }))
        assert result.success

    def test_rankings(self):
        result = _run_async(self.tool.run({
            "action": "rankings",
            "limit": 10,
        }))
        assert result.success

    def test_low_reputation(self):
        result = _run_async(self.tool.run({"action": "low_reputation"}))
        assert result.success

    def test_config(self):
        result = _run_async(self.tool.run({"action": "config"}))
        assert result.success

    def test_unknown_action(self):
        result = _run_async(self.tool.run({"action": "invalid"}))
        assert result.status == ToolStatus.ERROR

    def test_schema(self):
        schema = self.tool.get_schema_dict()
        assert schema["name"] == "reputation"
        assert schema["category"] == "social"


# ============================================================
# TrustTool tests
# ============================================================


class TestTrustTool:
    def setup_method(self):
        self.tool = _sandbox_tool(TrustTool)

    def test_name_and_category(self):
        assert self.tool.name == "trust"
        assert self.tool.category == "social"

    def test_interact(self):
        result = _run_async(self.tool.run({
            "action": "interact",
            "from_agent_id": "agent-1",
            "to_agent_id": "agent-2",
            "interaction_type": "trade",
            "value": 0.8,
            "context": "Successful trade deal",
        }))
        assert result.success

    def test_get_score(self):
        result = _run_async(self.tool.run({
            "action": "get_score",
            "from_agent_id": "agent-1",
            "to_agent_id": "agent-2",
        }))
        assert result.success

    def test_relationships(self):
        result = _run_async(self.tool.run({
            "action": "relationships",
            "agent_id": "agent-1",
        }))
        assert result.success

    def test_allies(self):
        result = _run_async(self.tool.run({
            "action": "allies",
            "agent_id": "agent-1",
        }))
        assert result.success

    def test_enemies(self):
        result = _run_async(self.tool.run({
            "action": "enemies",
            "agent_id": "agent-1",
        }))
        assert result.success

    def test_stats(self):
        result = _run_async(self.tool.run({"action": "stats"}))
        assert result.success

    def test_unknown_action(self):
        result = _run_async(self.tool.run({"action": "invalid"}))
        assert result.status == ToolStatus.ERROR


# ============================================================
# EscrowTool tests
# ============================================================


class TestEscrowTool:
    def setup_method(self):
        self.tool = _sandbox_tool(EscrowTool)

    def test_name_and_category(self):
        assert self.tool.name == "escrow"
        assert self.tool.category == "economy"

    def test_create(self):
        result = _run_async(self.tool.run({
            "action": "create",
            "agent_id": "agent-1",
            "counterparty_id": "agent-2",
            "amount": 100.0,
            "description": "Payment for services",
        }))
        assert result.success

    def test_list(self):
        result = _run_async(self.tool.run({"action": "list"}))
        assert result.success

    def test_get(self):
        result = _run_async(self.tool.run({
            "action": "get",
            "escrow_id": "esc-1",
        }))
        assert result.success

    def test_claim(self):
        result = _run_async(self.tool.run({
            "action": "claim",
            "escrow_id": "esc-1",
            "agent_id": "agent-2",
        }))
        assert result.success

    def test_complete(self):
        result = _run_async(self.tool.run({
            "action": "complete",
            "escrow_id": "esc-1",
        }))
        assert result.success

    def test_refund(self):
        result = _run_async(self.tool.run({
            "action": "refund",
            "escrow_id": "esc-1",
        }))
        assert result.success

    def test_dispute(self):
        result = _run_async(self.tool.run({
            "action": "dispute",
            "escrow_id": "esc-1",
            "agent_id": "agent-1",
            "reason": "Service not delivered",
        }))
        assert result.success

    def test_resolve(self):
        result = _run_async(self.tool.run({
            "action": "resolve",
            "escrow_id": "esc-1",
            "resolution": "partial_refund",
            "refund_amount": 50.0,
        }))
        assert result.success

    def test_unknown_action(self):
        result = _run_async(self.tool.run({"action": "invalid"}))
        assert result.status == ToolStatus.ERROR

    def test_schema(self):
        schema = self.tool.get_schema_dict()
        assert schema["name"] == "escrow"
        assert schema["category"] == "economy"


# ============================================================
# Integration: registry with all tools
# ============================================================


class TestRegistryWithAllTools:
    def test_create_registry_with_builtins(self):
        registry = create_registry_with_builtins(sandbox_world_engine=True)
        # 3 original + 3 P0 + 3 P1 + 6 P2 = 15 tools
        assert registry.count == 15

    def test_all_tools_registered(self):
        registry = create_registry_with_builtins(sandbox_world_engine=True)
        expected_names = {
            "http_request", "file_ops", "code_exec",
            "governance", "task", "organization",
            "diplomacy", "investment", "legislation",
            "bank", "stocks", "marketplace",
            "reputation", "trust", "escrow",
        }
        actual_names = {t.name for t in registry.list_tools()}
        assert actual_names == expected_names

    def test_categories_include_new_ones(self):
        registry = create_registry_with_builtins(sandbox_world_engine=True)
        cats = registry.categories()
        assert "governance" in cats
        assert "task" in cats
        assert "organization" in cats
        assert "diplomacy" in cats
        assert "economy" in cats
        assert "social" in cats

    def test_invoke_governance_via_registry(self):
        registry = create_registry_with_builtins(sandbox_world_engine=True)
        result = _run_async(registry.invoke("governance", {
            "action": "summary",
        }))
        assert result.success

    def test_invoke_task_via_registry(self):
        registry = create_registry_with_builtins(sandbox_world_engine=True)
        result = _run_async(registry.invoke("task", {
            "action": "list",
        }))
        assert result.success

    def test_invoke_organization_via_registry(self):
        registry = create_registry_with_builtins(sandbox_world_engine=True)
        result = _run_async(registry.invoke("organization", {
            "action": "list",
        }))
        assert result.success

    def test_invoke_bank_via_registry(self):
        registry = create_registry_with_builtins(sandbox_world_engine=True)
        result = _run_async(registry.invoke("bank", {
            "action": "stats",
        }))
        assert result.success

    def test_all_schemas_valid_json(self):
        """All tools export valid JSON-serialisable schemas."""
        registry = create_registry_with_builtins(sandbox_world_engine=True)
        schemas = registry.get_all_schemas(enabled_only=False)
        assert len(schemas) == 15
        for schema in schemas:
            assert "name" in schema
            assert "description" in schema
            assert "parameters" in schema
            assert "properties" in schema["parameters"]
            # Ensure JSON-serialisable
            json.dumps(schema)

    def test_backward_compatible_create_builtin_tools(self):
        """Old create_builtin_tools() signature still works."""
        tools = create_builtin_tools()
        # Default is sandbox_world_engine=True
        assert len(tools) == 15

    def test_tool_stats_after_invocation(self):
        registry = create_registry_with_builtins(sandbox_world_engine=True)
        _run_async(registry.invoke("reputation", {"action": "config"}))
        _run_async(registry.invoke("trust", {"action": "stats"}))
        stats = registry.get_stats()
        assert stats["reputation"]["invoke_count"] == 1
        assert stats["trust"]["invoke_count"] == 1
