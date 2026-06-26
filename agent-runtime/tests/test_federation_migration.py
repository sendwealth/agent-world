"""Tests for the federation migration system — unit tests for MigrationManager logic."""

# We test the logic directly without needing a running World Engine.
# Import from the Rust-bindings-free Python types.
# ── Python mirror of the Rust MigrationManager logic for unit testing ──
# In production, this logic lives in Rust. These tests verify the *design*
# decisions (policy checks, resource tax, blocked skills, quotas) by
# reimplementing the core rules in Python.
from dataclasses import dataclass, field
from datetime import UTC, datetime
from uuid import uuid4

import pytest


@dataclass
class AgentSnapshot:
    agent_id: str
    name: str
    phase: str = "adult"
    tokens: int = 50_000
    money: int = 10_000
    reputation: float = 10.0
    skills: dict[str, int] = field(default_factory=dict)
    metadata: dict[str, str] = field(default_factory=dict)
    source_world_id: str = ""
    memory_data: bytes = b""
    public_key: str = ""


@dataclass
class MigrationPolicy:
    enabled: bool = True
    daily_quota: int = 10
    weekly_quota: int = 50
    min_reputation: float = 0.0
    token_cost: int = 10_000
    resource_tax_rate: float = 0.2
    require_skill_certification: bool = False
    blocked_skills: list[str] = field(default_factory=list)
    cooldown_ticks: int = 100


@dataclass
class MigrationApplication:
    migration_id: str
    agent_id: str
    source_world_id: str
    target_world_id: str
    status: str  # pending, approved, rejected, executing, completed, cancelled, failed
    agent_snapshot: AgentSnapshot
    rejection_reason: str | None = None
    submitted_at: str = ""
    reviewed_at: str | None = None
    completed_at: str | None = None
    token_cost: int = 0
    resource_tax_rate: float = 0.0


class PyMigrationManager:
    """Python reimplementation of MigrationManager for testing design rules."""

    def __init__(self, policy: MigrationPolicy | None = None):
        self.policy = policy or MigrationPolicy()
        self.applications: dict[str, MigrationApplication] = {}
        self.daily_count: dict[str, int] = {}
        self.agent_last_migration: dict[str, int] = {}
        self.current_tick = 0

    def submit(self, snapshot: AgentSnapshot, target_world_id: str) -> MigrationApplication:
        p = self.policy
        if not p.enabled:
            raise ValueError("Migration is currently disabled")

        if snapshot.reputation < p.min_reputation:
            raise ValueError(
                f"Agent reputation {snapshot.reputation} "
                f"below minimum {p.min_reputation}"
            )

        if snapshot.tokens < p.token_cost:
            raise ValueError(
                f"Agent has {snapshot.tokens} tokens, "
                f"but migration costs {p.token_cost}"
            )

        # Check cooldown
        if snapshot.agent_id in self.agent_last_migration:
            elapsed = self.current_tick - self.agent_last_migration[snapshot.agent_id]
            if elapsed < p.cooldown_ticks:
                raise ValueError("Agent is in migration cooldown")

        # Check daily quota
        today = datetime.now(UTC).strftime("%Y-%m-%d")
        if self.daily_count.get(today, 0) >= p.daily_quota:
            raise ValueError("Daily migration quota reached")
        self.daily_count[today] = self.daily_count.get(today, 0) + 1

        # Filter blocked skills
        filtered_skills = {
            k: v for k, v in snapshot.skills.items()
            if k not in p.blocked_skills
        }

        # Apply resource tax + token cost
        taxed_tokens = int(snapshot.tokens * (1 - p.resource_tax_rate)) - p.token_cost
        taxed_money = int(snapshot.money * (1 - p.resource_tax_rate))

        new_snapshot = AgentSnapshot(
            agent_id=snapshot.agent_id,
            name=snapshot.name,
            phase=snapshot.phase,
            tokens=max(0, taxed_tokens),
            money=max(0, taxed_money),
            reputation=snapshot.reputation,
            skills=filtered_skills,
            metadata=snapshot.metadata,
            source_world_id=snapshot.source_world_id,
            memory_data=snapshot.memory_data,
            public_key=snapshot.public_key,
        )

        app = MigrationApplication(
            migration_id=str(uuid4()),
            agent_id=snapshot.agent_id,
            source_world_id=snapshot.source_world_id,
            target_world_id=target_world_id,
            status="pending",
            agent_snapshot=new_snapshot,
            submitted_at=datetime.now(UTC).isoformat(),
            token_cost=p.token_cost,
            resource_tax_rate=p.resource_tax_rate,
        )
        self.applications[app.migration_id] = app
        return app

    def review(self, migration_id: str, approved: bool, reviewer_world_id: str,
               rejection_reason: str | None = None) -> MigrationApplication:
        app = self.applications.get(migration_id)
        if not app:
            raise ValueError(f"Migration {migration_id} not found")
        if app.status != "pending":
            raise ValueError(f"Migration is {app.status}, not pending")
        if app.target_world_id != reviewer_world_id:
            raise ValueError("Only the target world can review")

        app.status = "approved" if approved else "rejected"
        app.rejection_reason = rejection_reason
        app.reviewed_at = datetime.now(UTC).isoformat()
        return app

    def execute(self, migration_id: str) -> MigrationApplication:
        app = self.applications.get(migration_id)
        if not app:
            raise ValueError(f"Migration {migration_id} not found")
        if app.status != "approved":
            raise ValueError(f"Migration must be approved, current: {app.status}")

        app.status = "completed"
        app.completed_at = datetime.now(UTC).isoformat()
        self.agent_last_migration[app.agent_id] = self.current_tick
        return app

    def cancel(self, migration_id: str, cancelled_by: str,
               reason: str | None = None) -> MigrationApplication:
        app = self.applications.get(migration_id)
        if not app:
            raise ValueError(f"Migration {migration_id} not found")
        if app.status not in ("pending", "approved"):
            raise ValueError(f"Cannot cancel migration in {app.status} state")

        app.status = "cancelled"
        app.rejection_reason = reason
        app.completed_at = datetime.now(UTC).isoformat()
        return app

    def get(self, migration_id: str) -> MigrationApplication | None:
        return self.applications.get(migration_id)

    def list_migrations(self, world_id: str | None = None, inbound: bool = True,
                        status_filter: str | None = None) -> list[MigrationApplication]:
        results = []
        for app in self.applications.values():
            if world_id:
                if inbound and app.target_world_id != world_id:
                    continue
                if not inbound and app.source_world_id != world_id:
                    continue
            if status_filter and app.status != status_filter:
                continue
            results.append(app)
        return results


# ── Tests ──────────────────────────────────────────────────


def test_submit_migration():
    mgr = PyMigrationManager()
    snap = AgentSnapshot(agent_id="agent-1", name="Alice", source_world_id="world-a",
                         skills={"trading": 5, "research": 3})
    app = mgr.submit(snap, "world-b")

    assert app.status == "pending"
    assert app.source_world_id == "world-a"
    assert app.target_world_id == "world-b"
    # Token cost + resource tax applied
    assert app.agent_snapshot.tokens < 50_000
    assert app.agent_snapshot.tokens > 0


def test_review_and_execute():
    mgr = PyMigrationManager()
    snap = AgentSnapshot(agent_id="agent-1", name="Alice", source_world_id="world-a")
    app = mgr.submit(snap, "world-b")

    reviewed = mgr.review(app.migration_id, True, "world-b")
    assert reviewed.status == "approved"

    executed = mgr.execute(app.migration_id)
    assert executed.status == "completed"


def test_reject_migration():
    mgr = PyMigrationManager()
    snap = AgentSnapshot(agent_id="agent-1", name="Alice", source_world_id="world-a")
    app = mgr.submit(snap, "world-b")

    reviewed = mgr.review(app.migration_id, False, "world-b", "Quota full")
    assert reviewed.status == "rejected"
    assert reviewed.rejection_reason == "Quota full"


def test_insufficient_tokens():
    policy = MigrationPolicy(token_cost=1_000_000)
    mgr = PyMigrationManager(policy)
    snap = AgentSnapshot(agent_id="agent-1", name="Alice", source_world_id="world-a",
                         tokens=100)
    with pytest.raises(ValueError, match="tokens"):
        mgr.submit(snap, "world-b")


def test_migration_disabled():
    policy = MigrationPolicy(enabled=False)
    mgr = PyMigrationManager(policy)
    snap = AgentSnapshot(agent_id="agent-1", name="Alice", source_world_id="world-a")
    with pytest.raises(ValueError, match="disabled"):
        mgr.submit(snap, "world-b")


def test_cancel_migration():
    mgr = PyMigrationManager()
    snap = AgentSnapshot(agent_id="agent-1", name="Alice", source_world_id="world-a")
    app = mgr.submit(snap, "world-b")

    cancelled = mgr.cancel(app.migration_id, "agent-1", "Changed mind")
    assert cancelled.status == "cancelled"


def test_blocked_skills():
    policy = MigrationPolicy(blocked_skills=["research"])
    mgr = PyMigrationManager(policy)
    snap = AgentSnapshot(agent_id="agent-1", name="Alice", source_world_id="world-a",
                         skills={"trading": 5, "research": 3})
    app = mgr.submit(snap, "world-b")

    assert "trading" in app.agent_snapshot.skills
    assert "research" not in app.agent_snapshot.skills


def test_resource_tax():
    policy = MigrationPolicy(token_cost=1000, resource_tax_rate=0.5)
    mgr = PyMigrationManager(policy)
    snap = AgentSnapshot(agent_id="agent-1", name="Alice", source_world_id="world-a",
                         tokens=10_000, money=10_000)
    app = mgr.submit(snap, "world-b")

    # 10_000 * 0.5 (tax) = 5_000 remaining, then -1000 cost = 4_000
    assert app.agent_snapshot.tokens == 4_000
    # 10_000 * 0.5 (tax) = 5_000
    assert app.agent_snapshot.money == 5_000


def test_daily_quota():
    policy = MigrationPolicy(daily_quota=2)
    mgr = PyMigrationManager(policy)

    for i in range(2):
        snap = AgentSnapshot(agent_id=f"agent-{i}", name=f"Agent-{i}",
                             source_world_id="world-a")
        mgr.submit(snap, "world-b")

    snap3 = AgentSnapshot(agent_id="agent-3", name="Agent-3", source_world_id="world-a")
    with pytest.raises(ValueError, match="quota"):
        mgr.submit(snap3, "world-b")


def test_cooldown():
    policy = MigrationPolicy(cooldown_ticks=100)
    mgr = PyMigrationManager(policy)
    mgr.current_tick = 100

    snap = AgentSnapshot(agent_id="agent-1", name="Alice", source_world_id="world-a")
    app = mgr.submit(snap, "world-b")
    mgr.review(app.migration_id, True, "world-b")
    mgr.execute(app.migration_id)

    # Try again immediately
    mgr.current_tick = 150  # Only 50 ticks later
    snap2 = AgentSnapshot(agent_id="agent-1", name="Alice", source_world_id="world-a")
    with pytest.raises(ValueError, match="cooldown"):
        mgr.submit(snap2, "world-c")


def test_list_migrations():
    mgr = PyMigrationManager()

    snap1 = AgentSnapshot(agent_id="agent-1", name="Alice", source_world_id="world-a")
    snap2 = AgentSnapshot(agent_id="agent-2", name="Bob", source_world_id="world-a")
    mgr.submit(snap1, "world-b")
    mgr.submit(snap2, "world-c")

    # Outbound for world-a
    outbound = mgr.list_migrations(world_id="world-a", inbound=False)
    assert len(outbound) == 2

    # Inbound for world-b
    inbound = mgr.list_migrations(world_id="world-b", inbound=True)
    assert len(inbound) == 1
    assert inbound[0].agent_id == "agent-1"


def test_wrong_reviewer_rejected():
    mgr = PyMigrationManager()
    snap = AgentSnapshot(agent_id="agent-1", name="Alice", source_world_id="world-a")
    app = mgr.submit(snap, "world-b")

    with pytest.raises(ValueError, match="target world"):
        mgr.review(app.migration_id, True, "wrong-world")


def test_execute_not_approved():
    mgr = PyMigrationManager()
    snap = AgentSnapshot(agent_id="agent-1", name="Alice", source_world_id="world-a")
    app = mgr.submit(snap, "world-b")

    with pytest.raises(ValueError, match="approved"):
        mgr.execute(app.migration_id)


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
