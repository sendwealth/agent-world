"""Economy metrics time series exporter.

Supports:
- Aggregate tick-by-tick economy metrics (GDP, Gini, population)
- Per-agent wealth distribution and asset breakdown
- Transaction history from marketplace purchases
- Banking system data (accounts, loans, money supply)
- Stock market data (listings, price history, holdings)
"""

from __future__ import annotations

import csv
import io
import json
from dataclasses import dataclass, field
from typing import Any


# ── Data Classes ──────────────────────────────────────────────

@dataclass
class EconomyDataPoint:
    """Single tick economy metrics."""
    tick: int
    total_money: int
    total_tokens: int
    agent_count: int
    alive_count: int
    gini_coefficient: float
    task_count: int


@dataclass
class TransactionRecord:
    """A single marketplace transaction."""
    tick: int
    buyer_id: str
    seller_id: str
    listing_id: str
    price: int
    item_title: str


@dataclass
class BankAccountRecord:
    """A bank account snapshot."""
    account_id: str
    owner_id: str
    account_type: str
    label: str
    balance: int
    created_tick: int


@dataclass
class LoanRecord:
    """A loan record."""
    loan_id: str
    borrower_id: str
    principal: int
    outstanding_balance: int
    interest_rate: float
    status: str
    total_repaid: int
    created_tick: int


@dataclass
class StockRecord:
    """A stock listing."""
    stock_id: str
    org_id: str
    ticker: str
    total_shares: int
    price: float
    status: str
    listed_tick: int


@dataclass
class StockOrderRecord:
    """A stock market order."""
    order_id: str
    stock_id: str
    agent_id: str
    order_type: str
    price: float
    quantity: int
    filled_quantity: int
    status: str
    created_tick: int


# ── Gini Computation ──────────────────────────────────────────

def compute_gini(values: list[float]) -> float:
    """Compute Gini coefficient for wealth distribution.

    Uses the standard formula: G = (2 * sum(i * x_i)) / (n * sum(x_i)) - (n+1)/n
    where x_i are sorted values.

    Returns 0.0 for empty or uniform distributions.
    """
    if not values or len(values) < 2:
        return 0.0

    sorted_vals = sorted(values)
    n = len(sorted_vals)
    total = sum(sorted_vals)

    if total == 0:
        return 0.0

    weighted_sum = sum((i + 1) * x for i, x in enumerate(sorted_vals))
    gini = (2.0 * weighted_sum) / (n * total) - (n + 1) / n
    return round(max(0.0, gini), 6)


# ── Main Exporter ─────────────────────────────────────────────

class EconomyExporter:
    """Export economy metrics time series.

    Computes wealth distribution, Gini coefficient, and resource metrics
    from agent state data. Also supports transaction history, banking,
    and stock market data export.
    """

    def __init__(self) -> None:
        self._data_points: list[EconomyDataPoint] = []
        self._transactions: list[TransactionRecord] = []
        self._bank_accounts: list[BankAccountRecord] = []
        self._loans: list[LoanRecord] = []
        self._stocks: list[StockRecord] = []
        self._stock_orders: list[StockOrderRecord] = []
        self._price_history: list[dict[str, Any]] = []

    # ── Aggregate tick data ───────────────────────────────────

    def add_tick_data(self, tick: int, agents: list[dict],
                      task_count: int = 0) -> EconomyExporter:
        """Add economy data for a single tick.

        Args:
            tick: The tick number.
            agents: List of agent dicts with 'money', 'tokens', 'alive' keys.
            task_count: Number of active tasks.
        """
        total_money = sum(a.get("money", 0) for a in agents)
        total_tokens = sum(a.get("tokens", 0) for a in agents)
        alive_count = sum(1 for a in agents if a.get("alive", True))

        wealth = [float(a.get("money", 0)) for a in agents]
        gini = compute_gini(wealth)

        self._data_points.append(EconomyDataPoint(
            tick=tick,
            total_money=total_money,
            total_tokens=total_tokens,
            agent_count=len(agents),
            alive_count=alive_count,
            gini_coefficient=gini,
            task_count=task_count,
        ))
        return self

    # ── Transaction data ──────────────────────────────────────

    def add_transaction(self, tick: int, buyer_id: str, seller_id: str,
                        listing_id: str, price: int,
                        item_title: str = "") -> EconomyExporter:
        """Add a marketplace transaction record."""
        self._transactions.append(TransactionRecord(
            tick=tick,
            buyer_id=buyer_id,
            seller_id=seller_id,
            listing_id=listing_id,
            price=price,
            item_title=item_title,
        ))
        return self

    def add_transactions(self, transactions: list[dict]) -> EconomyExporter:
        """Add multiple transaction records from dicts.

        Each dict should have keys: tick, buyer_id, seller_id, listing_id,
        price, item_title (optional).
        """
        for t in transactions:
            self.add_transaction(
                tick=t["tick"],
                buyer_id=t["buyer_id"],
                seller_id=t["seller_id"],
                listing_id=t.get("listing_id", ""),
                price=t["price"],
                item_title=t.get("item_title", ""),
            )
        return self

    # ── Banking data ──────────────────────────────────────────

    def add_bank_account(self, account_id: str, owner_id: str,
                         account_type: str, label: str,
                         balance: int, created_tick: int) -> EconomyExporter:
        """Add a bank account record."""
        self._bank_accounts.append(BankAccountRecord(
            account_id=account_id,
            owner_id=owner_id,
            account_type=account_type,
            label=label,
            balance=balance,
            created_tick=created_tick,
        ))
        return self

    def add_loan(self, loan_id: str, borrower_id: str, principal: int,
                 outstanding_balance: int, interest_rate: float,
                 status: str, total_repaid: int,
                 created_tick: int) -> EconomyExporter:
        """Add a loan record."""
        self._loans.append(LoanRecord(
            loan_id=loan_id,
            borrower_id=borrower_id,
            principal=principal,
            outstanding_balance=outstanding_balance,
            interest_rate=interest_rate,
            status=status,
            total_repaid=total_repaid,
            created_tick=created_tick,
        ))
        return self

    # ── Stock market data ─────────────────────────────────────

    def add_stock(self, stock_id: str, org_id: str, ticker: str,
                  total_shares: int, price: float, status: str,
                  listed_tick: int) -> EconomyExporter:
        """Add a stock listing."""
        self._stocks.append(StockRecord(
            stock_id=stock_id,
            org_id=org_id,
            ticker=ticker,
            total_shares=total_shares,
            price=price,
            status=status,
            listed_tick=listed_tick,
        ))
        return self

    def add_stock_order(self, order_id: str, stock_id: str, agent_id: str,
                        order_type: str, price: float, quantity: int,
                        filled_quantity: int, status: str,
                        created_tick: int) -> EconomyExporter:
        """Add a stock market order."""
        self._stock_orders.append(StockOrderRecord(
            order_id=order_id,
            stock_id=stock_id,
            agent_id=agent_id,
            order_type=order_type,
            price=price,
            quantity=quantity,
            filled_quantity=filled_quantity,
            status=status,
            created_tick=created_tick,
        ))
        return self

    def add_price_history(self, stock_id: str, ticker: str,
                          prices: list[dict[str, Any]]) -> EconomyExporter:
        """Add price history entries for a stock.

        Each entry should have: tick, price, volume (optional).
        """
        for p in prices:
            self._price_history.append({
                "stock_id": stock_id,
                "ticker": ticker,
                "tick": p["tick"],
                "price": p["price"],
                "volume": p.get("volume", 0),
            })
        return self

    # ── Wealth distribution analysis ──────────────────────────

    def export_wealth_distribution(self, agents: list[dict]) -> dict[str, Any]:
        """Export per-agent wealth distribution analysis.

        Args:
            agents: List of agent dicts with 'id', 'name', 'money', 'tokens', 'alive' keys.

        Returns:
            Dict with wealth distribution, percentiles, and breakdown.
        """
        alive_agents = [a for a in agents if a.get("alive", True)]

        if not alive_agents:
            return {
                "total_money": 0,
                "total_tokens": 0,
                "alive_agents": 0,
                "gini_coefficient": 0.0,
                "percentiles": {},
                "agents": [],
            }

        total_money = sum(a.get("money", 0) for a in alive_agents)
        total_tokens = sum(a.get("tokens", 0) for a in alive_agents)

        wealth_list = sorted(
            [
                {
                    "agent_id": a.get("id", ""),
                    "name": a.get("name", ""),
                    "money": a.get("money", 0),
                    "tokens": a.get("tokens", 0),
                    "total_wealth": a.get("money", 0) + a.get("tokens", 0),
                }
                for a in alive_agents
            ],
            key=lambda x: x["total_wealth"],
            reverse=True,
        )

        wealth_values = [w["total_wealth"] for w in wealth_list]
        gini = compute_gini([float(v) for v in wealth_values])

        # Compute percentiles
        n = len(wealth_values)
        percentiles: dict[str, Any] = {}
        if n > 0:
            percentiles = {
                "top_1_pct_share": _top_share(wealth_values, 0.01),
                "top_10_pct_share": _top_share(wealth_values, 0.10),
                "top_25_pct_share": _top_share(wealth_values, 0.25),
                "bottom_50_pct_share": _bottom_share(wealth_values, 0.50),
                "median": wealth_values[n // 2] if n % 2 == 1
                else (wealth_values[n // 2 - 1] + wealth_values[n // 2]) / 2,
                "mean": total_money / n + total_tokens / n,
            }

        return {
            "total_money": total_money,
            "total_tokens": total_tokens,
            "alive_agents": len(alive_agents),
            "gini_coefficient": gini,
            "percentiles": percentiles,
            "agents": wealth_list,
        }

    # ── Format exports ────────────────────────────────────────

    def export_json(self) -> str:
        """Export all aggregate data points as JSON."""
        data = [
            {
                "tick": dp.tick,
                "total_money": dp.total_money,
                "total_tokens": dp.total_tokens,
                "agent_count": dp.agent_count,
                "alive_count": dp.alive_count,
                "gini_coefficient": dp.gini_coefficient,
                "task_count": dp.task_count,
            }
            for dp in self._data_points
        ]
        return json.dumps(data, indent=2, ensure_ascii=False)

    def export_csv(self) -> str:
        """Export aggregate data as CSV compatible with Pandas read_csv()."""
        output = io.StringIO()
        writer = csv.writer(output, lineterminator="\n")
        writer.writerow([
            "tick", "total_money", "total_tokens", "agent_count",
            "alive_count", "gini_coefficient", "task_count",
        ])
        for dp in self._data_points:
            writer.writerow([
                dp.tick, dp.total_money, dp.total_tokens,
                dp.agent_count, dp.alive_count, dp.gini_coefficient,
                dp.task_count,
            ])
        return output.getvalue()

    def export_transactions_json(self) -> str:
        """Export transaction records as JSON."""
        data = [
            {
                "tick": t.tick,
                "buyer_id": t.buyer_id,
                "seller_id": t.seller_id,
                "listing_id": t.listing_id,
                "price": t.price,
                "item_title": t.item_title,
            }
            for t in self._transactions
        ]
        return json.dumps(data, indent=2, ensure_ascii=False)

    def export_transactions_csv(self) -> str:
        """Export transaction records as CSV."""
        output = io.StringIO()
        writer = csv.writer(output, lineterminator="\n")
        writer.writerow([
            "tick", "buyer_id", "seller_id", "listing_id",
            "price", "item_title",
        ])
        for t in self._transactions:
            writer.writerow([
                t.tick, t.buyer_id, t.seller_id, t.listing_id,
                t.price, t.item_title,
            ])
        return output.getvalue()

    def export_banking_json(self) -> dict[str, Any]:
        """Export banking data as a structured dict."""
        total_supply = sum(a.balance for a in self._bank_accounts)
        total_debt = sum(l.outstanding_balance for l in self._loans)

        return {
            "total_money_supply": total_supply,
            "total_loan_debt": total_debt,
            "accounts": [
                {
                    "account_id": a.account_id,
                    "owner_id": a.owner_id,
                    "account_type": a.account_type,
                    "label": a.label,
                    "balance": a.balance,
                    "created_tick": a.created_tick,
                }
                for a in self._bank_accounts
            ],
            "loans": [
                {
                    "loan_id": l.loan_id,
                    "borrower_id": l.borrower_id,
                    "principal": l.principal,
                    "outstanding_balance": l.outstanding_balance,
                    "interest_rate": l.interest_rate,
                    "status": l.status,
                    "total_repaid": l.total_repaid,
                    "created_tick": l.created_tick,
                }
                for l in self._loans
            ],
        }

    def export_banking_csv(self) -> str:
        """Export banking data as multi-section CSV."""
        output = io.StringIO()

        output.write("# Bank Accounts\n")
        writer = csv.writer(output, lineterminator="\n")
        writer.writerow([
            "account_id", "owner_id", "type", "label", "balance", "created_tick",
        ])
        for a in self._bank_accounts:
            writer.writerow([
                a.account_id, a.owner_id, a.account_type,
                a.label, a.balance, a.created_tick,
            ])

        output.write("\n# Loans\n")
        writer = csv.writer(output, lineterminator="\n")
        writer.writerow([
            "loan_id", "borrower_id", "principal", "outstanding_balance",
            "interest_rate", "status", "total_repaid", "created_tick",
        ])
        for l in self._loans:
            writer.writerow([
                l.loan_id, l.borrower_id, l.principal,
                l.outstanding_balance, l.interest_rate, l.status,
                l.total_repaid, l.created_tick,
            ])

        return output.getvalue()

    def export_stock_market_json(self) -> dict[str, Any]:
        """Export stock market data as a structured dict."""
        return {
            "stocks": [
                {
                    "stock_id": s.stock_id,
                    "org_id": s.org_id,
                    "ticker": s.ticker,
                    "total_shares": s.total_shares,
                    "price": s.price,
                    "status": s.status,
                    "listed_tick": s.listed_tick,
                }
                for s in self._stocks
            ],
            "orders": [
                {
                    "order_id": o.order_id,
                    "stock_id": o.stock_id,
                    "agent_id": o.agent_id,
                    "order_type": o.order_type,
                    "price": o.price,
                    "quantity": o.quantity,
                    "filled_quantity": o.filled_quantity,
                    "status": o.status,
                    "created_tick": o.created_tick,
                }
                for o in self._stock_orders
            ],
            "price_history": self._price_history,
        }

    def export_stock_market_csv(self) -> str:
        """Export stock market data as multi-section CSV."""
        output = io.StringIO()

        output.write("# Stocks\n")
        writer = csv.writer(output, lineterminator="\n")
        writer.writerow([
            "stock_id", "org_id", "ticker", "total_shares",
            "price", "status", "listed_tick",
        ])
        for s in self._stocks:
            writer.writerow([
                s.stock_id, s.org_id, s.ticker, s.total_shares,
                s.price, s.status, s.listed_tick,
            ])

        output.write("\n# Stock Orders\n")
        writer = csv.writer(output, lineterminator="\n")
        writer.writerow([
            "order_id", "stock_id", "agent_id", "order_type",
            "price", "quantity", "filled_quantity", "status", "created_tick",
        ])
        for o in self._stock_orders:
            writer.writerow([
                o.order_id, o.stock_id, o.agent_id, o.order_type,
                o.price, o.quantity, o.filled_quantity, o.status,
                o.created_tick,
            ])

        if self._price_history:
            output.write("\n# Price History\n")
            writer = csv.writer(output, lineterminator="\n")
            writer.writerow(["stock_id", "ticker", "tick", "price", "volume"])
            for p in self._price_history:
                writer.writerow([
                    p["stock_id"], p["ticker"], p["tick"],
                    p["price"], p.get("volume", 0),
                ])

        return output.getvalue()

    def export_full_report(self, agents: list[dict] | None = None) -> dict[str, Any]:
        """Export a comprehensive economy report combining all data sources.

        Args:
            agents: Optional list of agent dicts for wealth distribution.

        Returns:
            Complete economy report dict.
        """
        report: dict[str, Any] = {
            "timeseries": [
                {
                    "tick": dp.tick,
                    "total_money": dp.total_money,
                    "total_tokens": dp.total_tokens,
                    "agent_count": dp.agent_count,
                    "alive_count": dp.alive_count,
                    "gini_coefficient": dp.gini_coefficient,
                    "task_count": dp.task_count,
                }
                for dp in self._data_points
            ],
            "transactions": [
                {
                    "tick": t.tick,
                    "buyer_id": t.buyer_id,
                    "seller_id": t.seller_id,
                    "listing_id": t.listing_id,
                    "price": t.price,
                    "item_title": t.item_title,
                }
                for t in self._transactions
            ],
        }

        if agents is not None:
            report["wealth_distribution"] = self.export_wealth_distribution(agents)

        if self._bank_accounts:
            report["banking"] = self.export_banking_json()

        if self._stocks:
            report["stock_market"] = self.export_stock_market_json()

        report["summary"] = self.get_summary()
        return report

    def get_summary(self) -> dict[str, Any]:
        """Get summary statistics across all data points."""
        if not self._data_points:
            return {"total_ticks": 0}

        summary: dict[str, Any] = {
            "total_ticks": len(self._data_points),
            "tick_range": [self._data_points[0].tick, self._data_points[-1].tick],
            "max_gini": max(dp.gini_coefficient for dp in self._data_points),
            "min_gini": min(dp.gini_coefficient for dp in self._data_points),
            "avg_alive": sum(dp.alive_count for dp in self._data_points) / len(self._data_points),
            "total_money_final": self._data_points[-1].total_money,
            "total_tokens_final": self._data_points[-1].total_tokens,
        }

        if self._transactions:
            total_volume = sum(t.price for t in self._transactions)
            summary["transaction_count"] = len(self._transactions)
            summary["total_trade_volume"] = total_volume

        if self._bank_accounts:
            summary["total_bank_accounts"] = len(self._bank_accounts)
            summary["total_bank_deposits"] = sum(a.balance for a in self._bank_accounts)

        if self._stocks:
            summary["listed_stocks"] = len(self._stocks)
            summary["total_market_cap"] = sum(
                s.price * s.total_shares for s in self._stocks
            )

        return summary

    def clear(self) -> None:
        """Reset all stored data."""
        self._data_points.clear()
        self._transactions.clear()
        self._bank_accounts.clear()
        self._loans.clear()
        self._stocks.clear()
        self._stock_orders.clear()
        self._price_history.clear()


# ── Helpers ───────────────────────────────────────────────────

def _top_share(sorted_desc: list[int], fraction: float) -> float:
    """Compute the wealth share of the top fraction of agents.

    Args:
        sorted_desc: Wealth values sorted descending.
        fraction: Fraction of population (e.g. 0.10 for top 10%).

    Returns:
        Share of total wealth held by the top fraction (0.0-1.0).
    """
    if not sorted_desc:
        return 0.0
    total = sum(sorted_desc)
    if total == 0:
        return 0.0
    n = max(1, int(len(sorted_desc) * fraction))
    return sum(sorted_desc[:n]) / total


def _bottom_share(sorted_desc: list[int], fraction: float) -> float:
    """Compute the wealth share of the bottom fraction of agents.

    Args:
        sorted_desc: Wealth values sorted descending.
        fraction: Fraction of population (e.g. 0.50 for bottom 50%).

    Returns:
        Share of total wealth held by the bottom fraction (0.0-1.0).
    """
    if not sorted_desc:
        return 0.0
    total = sum(sorted_desc)
    if total == 0:
        return 0.0
    n = max(1, int(len(sorted_desc) * fraction))
    return sum(sorted_desc[-n:]) / total
