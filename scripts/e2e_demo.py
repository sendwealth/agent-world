#!/usr/bin/env python3
"""
E2E Demo: 2 Agents survive 1000 ticks with trading, tasks, death, and full lifecycle.

This script mirrors the Rust world-engine logic in Python:
- Token burn with phase-based multipliers (R001)
- Death judgment when tokens reach zero (R002)
- Newbie protection for first 50 ticks (R003)
- Lifecycle phases: Birth -> Childhood -> Adult -> Elder -> Dead
- Task board: create, claim, start, submit, review, complete
- Reward distribution: 2% platform fee, XP, reputation
- Trading between agents (token <-> money exchange)
- Performance metrics collection and reporting

Usage:
    python3 scripts/e2e_demo.py [--ticks N] [--agents N] [--json]

Run with: python3 scripts/e2e_demo.py
"""

import argparse
import json
import time
import uuid
from dataclasses import dataclass, field
from enum import Enum
from typing import Dict, List, Optional, Tuple
from collections import defaultdict


# ═══════════════════════════════════════════════════════════════
# Enums (mirrors world-engine/src/world/enums.rs)
# ═══════════════════════════════════════════════════════════════

class Currency(str, Enum):
    Token = "token"
    Money = "money"


class AgentPhase(str, Enum):
    Birth = "birth"
    Childhood = "childhood"
    Adult = "adult"
    Elder = "elder"
    Dying = "dying"
    Dead = "dead"


class DeathReason(str, Enum):
    TokenDepleted = "token_depleted"
    HumanTerminated = "human_terminated"
    VoteEvicted = "vote_evicted"


class TaskStatus(str, Enum):
    Published = "published"
    Claimed = "claimed"
    InProgress = "in_progress"
    Submitted = "submitted"
    Reviewed = "reviewed"
    Completed = "completed"
    Expired = "expired"


class TransactionType(str, Enum):
    TaskReward = "task_reward"
    PlatformFee = "platform_fee"
    EscrowRefund = "escrow_refund"
    Exchange = "exchange"


# ═══════════════════════════════════════════════════════════════
# Data Models
# ═══════════════════════════════════════════════════════════════

@dataclass
class Skill:
    name: str
    level: int
    experience: float = 0.0


@dataclass
class Agent:
    id: str
    name: str
    phase: AgentPhase
    tokens: int
    money: int = 0
    spawn_tick: int = 0
    death_tick: Optional[int] = None
    death_reason: Optional[DeathReason] = None
    skills: Dict[str, Skill] = field(default_factory=dict)
    tasks_created: int = 0
    tasks_completed: int = 0
    tasks_claimed: int = 0
    trades_made: int = 0
    reputation: float = 0.0
    xp: int = 0

    def age(self, tick: int) -> int:
        return tick - self.spawn_tick

    def is_alive(self) -> bool:
        return self.phase != AgentPhase.Dead


@dataclass
class Task:
    id: str
    title: str
    description: str
    status: TaskStatus
    reward: int
    currency: Currency
    escrow_held: bool
    publisher_id: str
    assignee_id: Optional[str] = None
    result: Optional[str] = None
    expires_at: Optional[int] = None
    created_tick: int = 0


@dataclass
class LedgerEntry:
    id: str
    from_agent: Optional[str]
    to_agent: Optional[str]
    amount: int
    currency: Currency
    tx_type: TransactionType
    description: str
    tick: int
    reference_id: Optional[str] = None


@dataclass
class WorldEvent:
    event_type: str
    payload: dict
    tick: int


# ═══════════════════════════════════════════════════════════════
# Consumption Config (mirrors token_burn.rs)
# ═══════════════════════════════════════════════════════════════

@dataclass
class ConsumptionConfig:
    base_burn_per_tick: float = 10.0
    childhood_multiplier: float = 0.5
    adult_multiplier: float = 1.0
    elder_multiplier: float = 0.7
    skill_cost_per_level: float = 0.5


@dataclass
class RewardConfig:
    platform_fee_bps: int = 200  # 2%
    base_xp: int = 50
    reputation_gain: float = 2.0

    def calculate_fee(self, reward: int) -> int:
        return (reward * self.platform_fee_bps) // 10000

    def calculate_net_reward(self, reward: int) -> int:
        return reward - self.calculate_fee(reward)


# ═══════════════════════════════════════════════════════════════
# Genesis Config (mirrors config/genesis.yaml)
# ═══════════════════════════════════════════════════════════════

@dataclass
class GenesisConfig:
    tick_interval_ms: int = 1000
    max_agents: int = 10
    initial_tokens: int = 100000
    childhood_ticks: int = 100
    adult_ticks: int = 1000
    elder_ticks: int = 200
    death_grace_ticks: int = 10
    new_agent_protection_ticks: int = 50
    token_price: int = 100  # 1 Money = 100 Tokens
    interest_rate: float = 0.001
    task_expiry_ticks: int = 500
    min_reward_money: int = 1


# ═══════════════════════════════════════════════════════════════
# World Simulation
# ═══════════════════════════════════════════════════════════════

class WorldSimulation:
    def __init__(self, genesis: GenesisConfig = None, max_ticks: int = 1000):
        self.genesis = genesis or GenesisConfig()
        self.consumption = ConsumptionConfig()
        self.reward_config = RewardConfig()
        self.max_ticks = max_ticks
        self.tick = 0
        self.agents: Dict[str, Agent] = {}
        self.tasks: Dict[str, Task] = {}
        self.balances: Dict[str, int] = {}
        self.escrows: Dict[str, int] = {}
        self.ledger: List[LedgerEntry] = []
        self.central_bank_fees: Dict[Currency, int] = {Currency.Token: 0, Currency.Money: 0}
        self.events: List[WorldEvent] = []

        # Metrics
        self.total_tokens_burned = 0
        self.total_money_transferred = 0
        self.total_platform_fees = 0
        self.task_ticks: set = set()
        self.trade_ticks: set = set()

    def emit(self, event_type: str, payload: dict):
        self.events.append(WorldEvent(
            event_type=event_type,
            payload=payload,
            tick=self.tick,
        ))

    def spawn_agent(self, name: str, tokens: int) -> str:
        agent_id = str(uuid.uuid4())
        agent = Agent(
            id=agent_id,
            name=name,
            phase=AgentPhase.Birth,
            tokens=tokens,
            money=0,
            spawn_tick=self.tick,
        )
        self.agents[agent_id] = agent
        self.balances[agent_id] = tokens
        self.emit("agent_spawned", {"agent_id": agent_id, "name": name})
        return agent_id

    def get_phase_multiplier(self, phase: AgentPhase) -> float:
        if phase == AgentPhase.Childhood:
            return self.consumption.childhood_multiplier
        elif phase == AgentPhase.Adult:
            return self.consumption.adult_multiplier
        elif phase == AgentPhase.Elder:
            return self.consumption.elder_multiplier
        return 0.0

    def calculate_tick_burn(self, agent: Agent) -> int:
        phase_mult = self.get_phase_multiplier(agent.phase)
        if phase_mult == 0.0:
            return 0
        base = self.consumption.base_burn_per_tick * phase_mult
        skill_cost = sum(s.level * self.consumption.skill_cost_per_level for s in agent.skills.values())
        return int(base + skill_cost)

    def is_protected(self, agent: Agent) -> bool:
        if agent.phase == AgentPhase.Dead:
            return False
        return agent.age(self.tick) < self.genesis.new_agent_protection_ticks

    # ── Rules (R001-R003) ────────────────────────────────────

    def process_lifecycle(self):
        """Phase transitions based on agent age."""
        for agent in self.agents.values():
            if not agent.is_alive():
                continue
            age = agent.age(self.tick)
            old_phase = agent.phase

            if agent.phase == AgentPhase.Birth and age >= 1:
                agent.phase = AgentPhase.Childhood
            elif agent.phase == AgentPhase.Childhood and age >= self.genesis.childhood_ticks:
                agent.phase = AgentPhase.Adult
            elif agent.phase == AgentPhase.Adult and age >= self.genesis.childhood_ticks + self.genesis.adult_ticks:
                agent.phase = AgentPhase.Elder

            if agent.phase != old_phase:
                self.emit("phase_changed", {
                    "agent_id": agent.id,
                    "old_phase": old_phase.value,
                    "new_phase": agent.phase.value,
                })

    def run_rules(self):
        """R001: Token Consumption, R002: Death Judgment, R003: Newbie Protection."""
        for agent in self.agents.values():
            if not agent.is_alive():
                continue

            # R003: Newbie protection — Birth -> Childhood transition
            if agent.phase == AgentPhase.Birth and self.is_protected(agent):
                old_phase = agent.phase
                agent.phase = AgentPhase.Childhood
                self.emit("phase_changed", {
                    "agent_id": agent.id,
                    "old_phase": old_phase.value,
                    "new_phase": AgentPhase.Childhood.value,
                })

            # R001: Token consumption
            if agent.phase not in (AgentPhase.Dead, AgentPhase.Birth):
                burn = self.calculate_tick_burn(agent)
                if burn > 0:
                    tokens_before = agent.tokens
                    actual_burn = min(burn, agent.tokens)
                    agent.tokens -= actual_burn
                    self.total_tokens_burned += actual_burn
                    self.emit("balance_changed", {
                        "agent_id": agent.id,
                        "currency": Currency.Token.value,
                        "old_balance": tokens_before,
                        "new_balance": agent.tokens,
                    })

            # R002: Death judgment
            if agent.phase not in (AgentPhase.Dead, AgentPhase.Birth):
                if agent.tokens == 0:
                    self.emit("agent_dying", {
                        "agent_id": agent.id,
                        "reason": DeathReason.TokenDepleted.value,
                        "grace_ticks": self.genesis.death_grace_ticks,
                    })
                    # With grace period > 0, agent enters Dying phase
                    if self.genesis.death_grace_ticks > 0:
                        agent.phase = AgentPhase.Dying
                        # Check if grace period expired
                        if agent.death_tick is None:
                            agent.death_tick = self.tick
                        elif self.tick - agent.death_tick >= self.genesis.death_grace_ticks:
                            agent.phase = AgentPhase.Dead
                            agent.death_reason = DeathReason.TokenDepleted
                            self.emit("agent_died", {
                                "agent_id": agent.id,
                                "reason": DeathReason.TokenDepleted.value,
                            })
                    else:
                        agent.phase = AgentPhase.Dead
                        agent.death_reason = DeathReason.TokenDepleted
                        agent.death_tick = self.tick
                        self.emit("agent_died", {
                            "agent_id": agent.id,
                            "reason": DeathReason.TokenDepleted.value,
                        })

    # ── Task Board ────────────────────────────────────────────

    def create_task(self, publisher_id: str, reward: int, title: str = None) -> Optional[str]:
        task_id = str(uuid.uuid4())
        title = title or f"task-tick-{self.tick}"
        escrow_held = reward > 0

        if escrow_held:
            available = self.balances.get(publisher_id, 0)
            self.balances[publisher_id] = max(0, available - reward)
            self.escrows[task_id] = reward

        task = Task(
            id=task_id,
            title=title,
            description=f"Complete work by tick {self.tick + self.genesis.task_expiry_ticks}",
            status=TaskStatus.Published,
            reward=reward,
            currency=Currency.Token,
            escrow_held=escrow_held,
            publisher_id=publisher_id,
            expires_at=self.tick + self.genesis.task_expiry_ticks,
            created_tick=self.tick,
        )
        self.tasks[task_id] = task
        self.emit("task_created", {"task_id": task_id, "publisher": publisher_id, "reward": reward})
        return task_id

    def claim_task(self, task_id: str, assignee_id: str) -> bool:
        task = self.tasks.get(task_id)
        if not task or task.status != TaskStatus.Published:
            return False
        task.status = TaskStatus.Claimed
        task.assignee_id = assignee_id
        self.emit("task_claimed", {"task_id": task_id, "assignee": assignee_id})
        return True

    def start_task(self, task_id: str) -> bool:
        task = self.tasks.get(task_id)
        if not task or task.status != TaskStatus.Claimed:
            return False
        task.status = TaskStatus.InProgress
        self.emit("task_started", {"task_id": task_id})
        return True

    def submit_task(self, task_id: str, result: str) -> bool:
        task = self.tasks.get(task_id)
        if not task or task.status != TaskStatus.InProgress:
            return False
        task.status = TaskStatus.Submitted
        task.result = result
        self.emit("task_submitted", {"task_id": task_id})
        return True

    def review_task(self, task_id: str, reviewer_id: str, approved: bool) -> bool:
        task = self.tasks.get(task_id)
        if not task or task.publisher_id != reviewer_id:
            return False
        if task.status != TaskStatus.Submitted:
            return False
        if approved:
            task.status = TaskStatus.Reviewed
        else:
            task.status = TaskStatus.InProgress
        self.emit("task_reviewed", {"task_id": task_id, "approved": approved})
        return True

    def complete_task(self, task_id: str) -> Optional[dict]:
        task = self.tasks.get(task_id)
        if not task or task.status != TaskStatus.Reviewed:
            return None

        escrow = self.escrows.pop(task_id, 0)
        fee = self.reward_config.calculate_fee(escrow)
        net = escrow - fee

        # Pay assignee
        if task.assignee_id:
            assignee = self.agents.get(task.assignee_id)
            if assignee:
                assignee.tokens += net
                assignee.xp += self.reward_config.base_xp
                assignee.reputation += self.reward_config.reputation_gain

            # Record ledger
            reward_ledger_id = str(uuid.uuid4())
            self.ledger.append(LedgerEntry(
                id=reward_ledger_id,
                from_agent=None,
                to_agent=task.assignee_id,
                amount=net,
                currency=task.currency,
                tx_type=TransactionType.TaskReward,
                description=f"Task {task_id} reward (net after {fee} fee)",
                tick=self.tick,
                reference_id=task_id,
            ))

        # Platform fee
        self.central_bank_fees[task.currency] += fee
        self.total_platform_fees += fee

        fee_ledger_id = str(uuid.uuid4())
        self.ledger.append(LedgerEntry(
            id=fee_ledger_id,
            from_agent=task.assignee_id,
            to_agent=None,
            amount=fee,
            currency=task.currency,
            tx_type=TransactionType.PlatformFee,
            description=f"Task {task_id} platform fee ({self.reward_config.platform_fee_bps}bps)",
            tick=self.tick,
            reference_id=task_id,
        ))

        task.status = TaskStatus.Completed
        task.escrow_held = False

        self.emit("reward_distributed", {
            "task_id": task_id,
            "assignee_id": task.assignee_id,
            "gross_reward": escrow,
            "net_reward": net,
            "platform_fee": fee,
            "xp_awarded": self.reward_config.base_xp,
            "reputation_change": self.reward_config.reputation_gain,
        })
        self.emit("task_completed", {"task_id": task_id})

        return {"gross": escrow, "net": net, "fee": fee}

    # ── Trading ───────────────────────────────────────────────

    def process_trading(self):
        if self.tick % 50 != 0 or self.tick < 60:
            return

        alive = [a for a in self.agents.values() if a.is_alive()]
        if len(alive) < 2:
            return

        a1, a2 = alive[0], alive[1]

        # Agent with more tokens sells some to the one with more money
        if a1.tokens > 500 and a2.money > 5:
            tokens_to_sell = 500
            money_amount = tokens_to_sell // self.genesis.token_price  # 5

            a1.tokens -= tokens_to_sell
            a1.money += money_amount
            a1.trades_made += 1
            a2.tokens += tokens_to_sell
            a2.money -= money_amount
            a2.trades_made += 1
            self.total_money_transferred += money_amount
            self.trade_ticks.add(self.tick)

            self.emit("transaction_completed", {
                "from": a1.id, "to": a2.id, "amount": tokens_to_sell, "currency": "token",
            })
            self.emit("transaction_completed", {
                "from": a2.id, "to": a1.id, "amount": money_amount, "currency": "money",
            })

    # ── Task Processing ───────────────────────────────────────

    def process_tasks(self):
        if self.tick % 100 != 0 or self.tick == 0:
            return

        alive = [a for a in self.agents.values() if a.is_alive() and a.age(self.tick) >= 50]
        if len(alive) < 2:
            return

        publisher, worker = alive[0], alive[1]
        reward = 200

        if publisher.tokens <= reward + 100:
            return

        task_id = self.create_task(publisher.id, reward)
        if not task_id:
            return

        publisher.tasks_created += 1
        self.task_ticks.add(self.tick)

        # Worker claims and completes
        if self.claim_task(task_id, worker.id):
            worker.tasks_claimed += 1
            self.start_task(task_id)
            self.submit_task(task_id, f"Completed at tick {self.tick}")
            self.review_task(task_id, publisher.id, True)
            result = self.complete_task(task_id)
            if result:
                worker.tasks_completed += 1

    # ── Survival Aid ──────────────────────────────────────────

    def process_survival_aid(self):
        for agent in self.agents.values():
            if not agent.is_alive():
                continue
            if agent.tokens < 20 and agent.age(self.tick) > 50 and self.tick % 10 == 0:
                aid = 50
                agent.tokens += aid
                self.emit("agent_rescued", {"agent_id": agent.id})

    # ── Tick Loop ─────────────────────────────────────────────

    def tick_step(self):
        self.tick += 1
        self.process_lifecycle()
        self.run_rules()
        self.process_trading()
        self.process_tasks()
        self.process_survival_aid()
        self.emit("tick_advanced", {"tick": self.tick})

    def run(self) -> dict:
        start_time = time.time()

        # Spawn agents
        self.spawn_agent("Alice", self.genesis.initial_tokens)
        self.spawn_agent("Bob", self.genesis.initial_tokens)
        # Give Bob some money for trading
        self.agents[list(self.agents.keys())[1]].money = 100

        milestones = {1, 10, 50, 100, 200, 500, 750, 1000}

        print("\n" + "=" * 64)
        print("  Agent World — E2E Demo: 2 Agents × 1000 Ticks")
        print("=" * 64 + "\n")

        print("  Agents spawned:")
        for a in self.agents.values():
            print(f"    {a.name} ({a.id[:8]}...): {a.tokens:,} tokens, phase={a.phase.value}")
        print()

        while self.tick < self.max_ticks:
            self.tick_step()
            if self.tick in milestones:
                self._print_status()

        elapsed = time.time() - start_time
        metrics = self._collect_metrics(elapsed)
        self._print_final_report(metrics)
        return metrics

    def _print_status(self):
        print(f"  ┌─ Tick {self.tick} {'─' * 40}")
        for a in self.agents.values():
            status = "ALIVE" if a.is_alive() else "DEAD"
            print(
                f"  │ {a.name:>8} [{status}] tokens={a.tokens:>6} money={a.money:>4} "
                f"phase={a.phase.value:<10} tasks=({a.tasks_completed}c/{a.tasks_created}d/{a.tasks_claimed}cl) "
                f"trades={a.trades_made} rep={a.reputation:.1f} xp={a.xp}"
            )
        print(f"  │ Events so far: {len(self.events)}")
        print(f"  └{'─' * 50}\n")

    def _collect_metrics(self, elapsed: float) -> dict:
        events_by_type = defaultdict(int)
        for e in self.events:
            events_by_type[e.event_type] += 1

        alive = sum(1 for a in self.agents.values() if a.is_alive())
        died = sum(1 for a in self.agents.values() if not a.is_alive())

        total_created = sum(a.tasks_created for a in self.agents.values())
        total_completed = sum(a.tasks_completed for a in self.agents.values())
        total_claimed = sum(a.tasks_claimed for a in self.agents.values())

        return {
            "total_ticks": self.max_ticks,
            "wall_time_ms": int(elapsed * 1000),
            "ticks_per_second": self.max_ticks / elapsed if elapsed > 0 else float("inf"),
            "total_events": len(self.events),
            "events_by_type": dict(events_by_type),
            "total_tokens_burned": self.total_tokens_burned,
            "total_money_transferred": self.total_money_transferred,
            "total_platform_fees": self.total_platform_fees,
            "tasks_created": total_created,
            "tasks_completed": total_completed,
            "tasks_claimed": total_claimed,
            "agents_alive": alive,
            "agents_died": died,
            "ticks_with_trades": len(self.trade_ticks),
            "ticks_with_tasks": len(self.task_ticks),
            "agents": {
                a.name: {
                    "id": a.id,
                    "alive": a.is_alive(),
                    "phase": a.phase.value,
                    "tokens": a.tokens,
                    "money": a.money,
                    "tasks_created": a.tasks_created,
                    "tasks_completed": a.tasks_completed,
                    "tasks_claimed": a.tasks_claimed,
                    "trades_made": a.trades_made,
                    "reputation": a.reputation,
                    "xp": a.xp,
                    "death_tick": a.death_tick,
                    "death_reason": a.death_reason.value if a.death_reason else None,
                }
                for a in self.agents.values()
            },
            "central_bank_fees": {c.value: f for c, f in self.central_bank_fees.items()},
            "ledger_entries": len(self.ledger),
        }

    def _print_final_report(self, m: dict):
        print("\n" + "=" * 64)
        print("  FINAL REPORT")
        print("=" * 64 + "\n")

        print(f"  Duration: {m['total_ticks']} ticks in {m['wall_time_ms']}ms "
              f"({m['ticks_per_second']:.0f} ticks/sec)")
        print()

        print("  Agents:")
        for name, a in m["agents"].items():
            status = "SURVIVED" if a["alive"] else "DIED"
            print(f"    {name} — {status}")
            print(f"      Phase: {a['phase']}")
            print(f"      Tokens: {a['tokens']:,} (started {self.genesis.initial_tokens:,})")
            print(f"      Money: {a['money']}")
            print(f"      Tasks created: {a['tasks_created']}, completed: {a['tasks_completed']}, claimed: {a['tasks_claimed']}")
            print(f"      Trades: {a['trades_made']}, Reputation: {a['reputation']:.1f}, XP: {a['xp']}")
            if a["death_tick"] is not None:
                print(f"      Died at tick {a['death_tick']} ({a['death_reason']})")
            print()

        print("  Economy:")
        print(f"    Total tokens burned: {m['total_tokens_burned']:,}")
        print(f"    Total money transferred: {m['total_money_transferred']}")
        print(f"    Total platform fees: {m['total_platform_fees']}")
        print(f"    Central bank: {m['central_bank_fees']}")
        print()

        print("  Tasks:")
        print(f"    Created: {m['tasks_created']}")
        print(f"    Completed: {m['tasks_completed']}")
        print(f"    Claimed: {m['tasks_claimed']}")
        print(f"    Ticks with task activity: {m['ticks_with_tasks']}")
        print()

        print("  Trading:")
        print(f"    Ticks with trades: {m['ticks_with_trades']}")
        print()

        print(f"  Events: {m['total_events']} total")
        sorted_events = sorted(m["events_by_type"].items(), key=lambda x: -x[1])
        for event_type, count in sorted_events:
            print(f"    {event_type:>30} {count}")
        print()

        print(f"  Ledger: {m['ledger_entries']} entries")
        print()

        print("=" * 64)
        print(f"  SUMMARY: {len(self.agents)} agents, {m['total_ticks']} ticks, "
              f"{m['ticks_per_second']:.0f} t/s, {m['total_events']} events")
        print(f"  ALIVE: {m['agents_alive']} | DIED: {m['agents_died']} | "
              f"TASKS: {m['tasks_completed']} | TRADES: {m['ticks_with_trades']} ticks")
        print("=" * 64 + "\n")


# ═══════════════════════════════════════════════════════════════
# Death Scenario
# ═══════════════════════════════════════════════════════════════

def run_death_scenario() -> dict:
    print("\n" + "=" * 64)
    print("  Death Scenario: Agent with only 30 tokens")
    print("=" * 64 + "\n")

    sim = WorldSimulation(max_ticks=100)
    sim.spawn_agent("Starving", 30)

    start = time.time()
    while sim.tick < sim.max_ticks:
        sim.tick_step()

    elapsed = time.time() - start
    agent = list(sim.agents.values())[0]

    print(f"  Agent 'Starving' died at tick {agent.death_tick} "
          f"(reason: {agent.death_reason.value if agent.death_reason else 'N/A'})")
    print(f"  Final phase: {agent.phase.value}")
    print(f"  Events: {len(sim.events)}")
    print()

    has_dying = any(e.event_type == "agent_dying" for e in sim.events)
    has_died = any(e.event_type == "agent_died" for e in sim.events)
    assert has_dying, "Should emit agent_dying event"
    assert has_died, "Should emit agent_died event"
    assert agent.phase == AgentPhase.Dead, f"Agent should be Dead, got {agent.phase}"

    print("  ✓ Death scenario assertions passed\n")
    return {
        "death_tick": agent.death_tick,
        "has_dying_event": has_dying,
        "has_died_event": has_died,
        "elapsed_ms": int(elapsed * 1000),
    }


# ═══════════════════════════════════════════════════════════════
# Main
# ═══════════════════════════════════════════════════════════════

def main():
    parser = argparse.ArgumentParser(description="Agent World E2E Demo")
    parser.add_argument("--ticks", type=int, default=1000, help="Number of ticks to simulate")
    parser.add_argument("--agents", type=int, default=2, help="Number of agents (currently only 2 supported)")
    parser.add_argument("--json", action="store_true", help="Output metrics as JSON")
    parser.add_argument("--death-scenario", action="store_true", help="Run death scenario")
    args = parser.parse_args()

    if args.death_scenario:
        result = run_death_scenario()
        if args.json:
            print(json.dumps(result, indent=2))
        return

    # Run main simulation
    sim = WorldSimulation(max_ticks=args.ticks)
    metrics = sim.run()

    # Assertions
    assert metrics["total_ticks"] == args.ticks, f"Expected {args.ticks} ticks"
    assert metrics["agents_alive"] >= 1, "At least one agent should survive"
    assert metrics["total_events"] > 0, "Events must be generated"
    assert metrics["total_tokens_burned"] > 0, "Tokens must be burned"
    assert metrics["ticks_per_second"] > 100, f"Too slow: {metrics['ticks_per_second']:.0f} t/s"
    assert "tick_advanced" in metrics["events_by_type"], "Must have tick_advanced events"
    assert "agent_spawned" in metrics["events_by_type"], "Must have agent_spawned events"
    assert "balance_changed" in metrics["events_by_type"], "Must have balance_changed events"

    print("  ✓ All assertions passed\n")

    # Run death scenario too
    death_result = run_death_scenario()

    # Combined summary
    combined = {
        "main_simulation": metrics,
        "death_scenario": death_result,
        "all_assertions_passed": True,
    }

    if args.json:
        # Remove agents detail for compact JSON output
        compact = {k: v for k, v in metrics.items() if k != "agents"}
        compact["agents_alive"] = metrics["agents_alive"]
        compact["agents_died"] = metrics["agents_died"]
        print(json.dumps(combined, indent=2))


if __name__ == "__main__":
    main()
