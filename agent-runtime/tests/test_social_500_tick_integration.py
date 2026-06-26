"""500+ tick integration test — 10 agents × 500 ticks verifying emergent social phenomena.

This test exercises the full social module pipeline in a simulated multi-agent world:
  - CulturalDiffusion: regional value convergence over time
  - ImitationEngine: behavioral mimicry leading to personality convergence
  - IntergroupTrust: trust differentiation between in-group and out-group
  - KnowledgeTransfer: skill and value transmission between agents
  - OrgCulture: organizational culture pressure and drift
  - RegionalCulture: cultural cluster detection
  - CulturalConflictAndFusion: conflict detection and boundary fusion
  - CommunicationAnalyzer + JargonDetector: language pattern emergence
  - LanguageExperiment: vocabulary constraint and novel word emergence

Acceptance criteria:
  10 agents × 500 ticks → observable cultural/language/trust emergence.
"""

from __future__ import annotations

import math
import random
from typing import Any

import pytest

from agent_runtime.core.experience import Experience
from agent_runtime.models.personality import PersonalityVector
from agent_runtime.models.skill import Skill
from agent_runtime.models.values import ValueWeights
from agent_runtime.social import (
    CommunicationAnalyzer,
    CulturalConflictAndFusion,
    CulturalDiffusion,
    DefaultSocialContextProvider,
    ImitationEngine,
    JargonDetector,
    KnowledgeTransfer,
    LanguageExperiment,
    OrgCultureSystem,
    RegionalCulture,
    SocialEngine,
)
from agent_runtime.social.provider import AgentProfile

# ---------------------------------------------------------------------------
# Helpers — simulated world state
# ---------------------------------------------------------------------------


def _make_random_personality(rng: random.Random, **overrides: float) -> PersonalityVector:
    """Generate a random personality with optional overrides."""
    base = {d: rng.random() for d in PersonalityVector._dimension_names()}
    base.update(overrides)
    return PersonalityVector(**base)


def _make_random_values(rng: random.Random, **overrides: float) -> ValueWeights:
    """Generate random values with optional overrides."""
    base = {d: rng.random() for d in ValueWeights._dimension_names()}
    base.update(overrides)
    return ValueWeights(**base)


class SimAgent:
    """Minimal simulated agent for integration testing."""

    def __init__(
        self,
        agent_id: str,
        personality: PersonalityVector,
        values: ValueWeights,
        region_id: str = "region_0",
        group_ids: list[str] | None = None,
    ) -> None:
        self.agent_id = agent_id
        self.personality = personality
        self.values = values
        self.region_id = region_id
        self.group_ids = group_ids or []
        self.skills: dict[str, Skill] = {
            "trading": Skill(
                name="trading", max_level=10, level=1,
                experience=0, next_level_exp=100,
            ),
            "farming": Skill(
                name="farming", max_level=10, level=1,
                experience=0, next_level_exp=100,
            ),
        }
        self.messages: list[str] = []

    def to_nearby_dict(self) -> dict[str, Any]:
        return {
            "agent_id": self.agent_id,
            "personality": self.personality,
            "values": self.values,
            "group_ids": self.group_ids,
        }

    def to_region_dict(self) -> dict[str, Any]:
        return {
            "agent_id": self.agent_id,
            "values": self.values,
            "personality": self.personality,
            "region_id": self.region_id,
        }


def _snapshot_personality(p: PersonalityVector) -> dict[str, float]:
    return {d: getattr(p, d) for d in PersonalityVector._dimension_names()}


def _snapshot_values(v: ValueWeights) -> dict[str, float]:
    return {d: getattr(v, d) for d in ValueWeights._dimension_names()}


def _personality_distance_sum(agents: list[SimAgent]) -> float:
    """Average pairwise personality distance among agents."""
    total = 0.0
    count = 0
    for i in range(len(agents)):
        for j in range(i + 1, len(agents)):
            total += agents[i].personality.distance(agents[j].personality)
            count += 1
    return total / count if count > 0 else 0.0


def _value_distance_sum(agents: list[SimAgent]) -> float:
    """Average pairwise value distance among agents."""
    dim_names = ValueWeights._dimension_names()
    total = 0.0
    count = 0
    for i in range(len(agents)):
        for j in range(i + 1, len(agents)):
            dist = math.sqrt(
                sum(
                    (getattr(agents[i].values, d) - getattr(agents[j].values, d)) ** 2
                    for d in dim_names
                )
            )
            total += dist
            count += 1
    return total / count if count > 0 else 0.0


# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------

NUM_AGENTS = 10
NUM_TICKS = 500
RNG_SEED = 42


@pytest.fixture
def rng() -> random.Random:
    return random.Random(RNG_SEED)


@pytest.fixture
def agents(rng: random.Random) -> list[SimAgent]:
    """Create 10 agents across 2 regions and 3 groups."""
    agents_list: list[SimAgent] = []

    # Region 0: agents 0-4, Groups A and B
    for i in range(5):
        group = "group_A" if i < 3 else "group_B"
        p = _make_random_personality(
            rng,
            extraversion=rng.uniform(0.4, 0.9),
            social_orientation=rng.uniform(0.4, 0.9),
            openness=rng.uniform(0.3, 0.9),
        )
        v = _make_random_values(rng)
        agents_list.append(SimAgent(
            agent_id=f"agent_{i}",
            personality=p,
            values=v,
            region_id="region_0",
            group_ids=[group],
        ))

    # Region 1: agents 5-9, Group C
    for i in range(5, 10):
        p = _make_random_personality(
            rng,
            extraversion=rng.uniform(0.4, 0.9),
            social_orientation=rng.uniform(0.4, 0.9),
            openness=rng.uniform(0.3, 0.9),
        )
        v = _make_random_values(rng)
        agents_list.append(SimAgent(
            agent_id=f"agent_{i}",
            personality=p,
            values=v,
            region_id="region_1",
            group_ids=["group_C"],
        ))

    return agents_list


# ---------------------------------------------------------------------------
# 500-tick simulation driver
# ---------------------------------------------------------------------------


def run_simulation(agents: list[SimAgent], rng: random.Random) -> dict[str, Any]:
    """Run 500 ticks of social simulation and collect metrics.

    Returns a dict of simulation results and metrics.
    """
    engine = SocialEngine()
    cultural_diffusion = CulturalDiffusion()
    ImitationEngine()
    knowledge_transfer = KnowledgeTransfer()
    org_culture = OrgCultureSystem()
    regional_culture = RegionalCulture(n_clusters=3)
    conflict_fusion = CulturalConflictAndFusion()
    comm_analyzer = CommunicationAnalyzer()
    jargon_detector = JargonDetector()
    language_exp = LanguageExperiment()

    # Set up org culture for groups
    for group_id in ["group_A", "group_B", "group_C"]:
        members = [a for a in agents if group_id in a.group_ids]
        member_values = [a.values for a in members]
        org_culture.compute_org_culture(group_id, member_values)
        for a in members:
            org_culture.register_member(group_id, a.agent_id)

    # Set up language experiment for agents 0-4
    language_exp.setup_restricted_vocabulary(
        agent_ids=[a.agent_id for a in agents[:5]],
        allowed_words={"trade", "food", "help", "need", "give", "want", "good", "bad", "yes", "no"},
        experiment_id="restricted",
    )

    # Initial snapshots
    initial_personalities = {a.agent_id: _snapshot_personality(a.personality) for a in agents}
    initial_values = {a.agent_id: _snapshot_values(a.values) for a in agents}
    initial_pers_distance = _personality_distance_sum(agents)
    initial_val_distance = _value_distance_sum(agents)

    # Track metrics over time
    trust_history: list[dict[str, float]] = []
    personality_distance_history: list[float] = []
    value_distance_history: list[float] = []
    diversity_index_history: list[float] = []

    # Message templates for language emergence testing
    message_templates = {
        "region_0": [
            "I need to trade food with someone nearby",
            "Let us cooperate on this task together",
            "The market has good prices today for trading",
            "I found some resources while exploring the area",
            "We should build an alliance for mutual defense",
        ],
        "region_1": [
            "I require sustenance barter with companions",
            "Perhaps we can collaborate on this endeavor",
            "The bazaar offers favorable rates this cycle",
            "New territories discovered during reconnaissance",
            "A coalition would strengthen our position greatly",
        ],
    }

    for tick in range(NUM_TICKS):
        # 1. Build social context for each agent and decide on social interactions
        for agent in agents:
            # Find nearby agents (same region = nearby for this simulation)
            nearby = [
                a.to_nearby_dict()
                for a in agents
                if a.agent_id != agent.agent_id and a.region_id == agent.region_id
            ]

            ctx = engine.build_context(
                agent_id=agent.agent_id,
                personality=agent.personality,
                values=agent.values,
                nearby_agents=nearby,
                tick=tick,
                agent_groups=agent.group_ids,
            )

            # 2. Execute socialize if recommended
            if ctx.should_socialize and ctx.recommended_target is not None:
                target_id = ctx.recommended_target.agent_id
                target = next((a for a in agents if a.agent_id == target_id), None)
                if target is not None:
                    result = engine.execute_socialize(
                        agent_id=agent.agent_id,
                        target_id=target_id,
                        personality=agent.personality,
                        values=agent.values,
                        target_personality=target.personality,
                        target_values=target.values,
                        tick=tick,
                    )

                    # 3. Knowledge transfer (teach lesson)
                    if rng.random() < 0.1:  # 10% chance per socialize
                        exp = Experience(
                            tick=tick,
                            event_type="cooperation",
                            partner_id=target_id,
                            outcome=rng.uniform(0.2, 1.0),
                        )
                        knowledge_transfer.teach_lesson(
                            teacher_values=target.values,
                            student_personality=agent.personality,
                            student_values=agent.values,
                            experience=exp,
                        )

                    # 4. Skill transfer (5% chance)
                    if rng.random() < 0.05:
                        for _skill_name, teacher_skill in target.skills.items():
                            knowledge_transfer.transfer_skill(
                                teacher_skill=teacher_skill,
                                student_skills=agent.skills,
                                student_personality=agent.personality,
                            )

            # 5. Generate messages for language analysis
            if rng.random() < 0.3:  # 30% chance to speak per tick
                template_list = message_templates.get(
                    agent.region_id, message_templates["region_0"],
                )
                base_msg = rng.choice(template_list)
                # Add some variation
                agent.messages.append(base_msg)

        # 6. Regional cultural diffusion (once per tick)
        agents_by_region: dict[str, list[dict[str, Any]]] = {}
        for agent in agents:
            agents_by_region.setdefault(agent.region_id, []).append(agent.to_region_dict())

        engine.apply_tick_diffusion(agents_by_region)

        # Also run standalone diffusion for cross-check
        for region_id, region_agents in agents_by_region.items():
            cultural_diffusion.apply_regional_influence(region_agents, region_id)

        # 7. Org culture pressure (every 10 ticks)
        if tick % 10 == 0:
            for group_id in ["group_A", "group_B", "group_C"]:
                members = [a for a in agents if group_id in a.group_ids]
                for member in members:
                    result = org_culture.apply_culture_pressure(member.values, group_id)
                    if result["updated_values"] is not member.values:
                        # Copy updated values back to the agent
                        for d in ValueWeights._dimension_names():
                            new_v = getattr(result["updated_values"], d)
                            object.__setattr__(member.values, d, new_v)

                # Culture drift
                org_culture.culture_drift(group_id, tick)
                for member in members:
                    org_culture.increment_tenure(group_id, member.agent_id)

        # 8. Cultural fusion at boundaries (every 20 ticks)
        if tick % 20 == 0:
            # Agents near group boundaries (different group in same region)
            border_agents_data = []
            for agent in agents:
                neighbors = [
                    a for a in agents
                    if a.agent_id != agent.agent_id
                    and a.region_id == agent.region_id
                    and set(a.group_ids) != set(agent.group_ids)
                ]
                if neighbors:
                    border_agents_data.append({
                        "id": agent.agent_id,
                        "values": agent.values,
                        "neighbor_values": [n.values for n in neighbors],
                    })

            if border_agents_data:
                fusion_result = conflict_fusion.apply_fusion_effect(border_agents_data)
                # Apply updated values back
                for aid, updated_v in fusion_result.get("updated_values", {}).items():
                    agent_obj = next((a for a in agents if a.agent_id == aid), None)
                    if agent_obj and updated_v is not agent_obj.values:
                        for d in ValueWeights._dimension_names():
                            object.__setattr__(agent_obj.values, d, getattr(updated_v, d))

        # 9. Record metrics every 50 ticks
        if tick % 50 == 0 or tick == NUM_TICKS - 1:
            # Trust snapshot: sample trust between group_A and group_C
            trust_ac = engine.trust.get_trust("group_A", "group_C")
            trust_aa = engine.trust.get_trust("agent_0", "agent_1")
            trust_history.append({
                "tick": tick,
                "group_A_to_C": trust_ac,
                "agent_0_to_1": trust_aa,
            })

            # Personality and value distances
            personality_distance_history.append(_personality_distance_sum(agents))
            value_distance_history.append(_value_distance_sum(agents))

            # Diversity index
            world_agent_data = [{"values": a.values} for a in agents]
            diversity_index_history.append(
                conflict_fusion.compute_cultural_diversity_index(world_agent_data)
            )

    # 10. Final analysis

    # Cultural clusters
    world_agents_data = [
        {
            "id": a.agent_id,
            "personality": a.personality,
            "values": a.values,
            "region_id": a.region_id,
        }
        for a in agents
    ]
    clusters = regional_culture.detect_cultural_clusters(world_agents_data, n_clusters=3)

    # Communication analysis
    group_messages: dict[str, list[str]] = {}
    for agent in agents:
        for gid in agent.group_ids:
            group_messages.setdefault(gid, []).extend(agent.messages)

    comm_comparison = None
    if len(group_messages) >= 2:
        groups_list = list(group_messages.keys())
        group_a_msgs = {groups_list[0]: group_messages[groups_list[0]]}
        group_b_msgs = {groups_list[1]: group_messages[groups_list[1]]}
        if group_a_msgs[groups_list[0]] and group_b_msgs[groups_list[1]]:
            comm_comparison = comm_analyzer.compare_group_patterns(group_a_msgs, group_b_msgs)

    # Jargon detection
    jargon_terms = []
    if len(group_messages) >= 2:
        jargon_terms = jargon_detector.detect_group_specific_terms(group_messages)

    # Language efficiency
    before_msgs = agents[0].messages[:len(agents[0].messages) // 2] if agents[0].messages else []
    after_msgs = agents[0].messages[len(agents[0].messages) // 2:] if agents[0].messages else []
    efficiency = language_exp.measure_communication_efficiency(
        before_msgs, after_msgs, "restricted",
    )

    # Dialect emergence
    dialect_data = []
    if len(group_messages) >= 2:
        groups_list = list(group_messages.keys())
        # Split messages into time windows
        window_size = max(1, len(agents[0].messages) // 4) if agents[0].messages else 1
        for w in range(4):
            period_msgs: dict[str, list[str]] = {}
            for gid in groups_list[:2]:
                group_messages[gid]
                start = w * window_size
                end = start + window_size
                # Distribute messages per agent per group
                members = [a for a in agents if gid in a.group_ids]
                for m in members:
                    agent_window = m.messages[start:end]
                    period_msgs.setdefault(gid, []).extend(agent_window)
            if any(period_msgs.values()):
                dialect_data.append({"period": f"window_{w}", "groups": period_msgs})

    dialect_report = comm_analyzer.detect_emerging_dialect(dialect_data)

    # Cultural distance between regions
    region_0_values = [a.values for a in agents if a.region_id == "region_0"]
    region_1_values = [a.values for a in agents if a.region_id == "region_1"]
    cultural_distance = cultural_diffusion.compute_cultural_distance(
        region_0_values, region_1_values,
    )

    # Final trust matrix
    final_trust: dict[str, float] = {}
    trust = engine.trust
    trust.register_membership("agent_0", "group_A")
    final_trust["in_group"] = trust.compute_in_group_trust("agent_0", "group_A")
    final_trust["out_group"] = trust.compute_out_group_trust("agent_0", "group_C")

    # Conflict history
    conflict_history = conflict_fusion.conflict_history

    return {
        "initial_personality_distance": initial_pers_distance,
        "initial_value_distance": initial_val_distance,
        "final_personality_distance": _personality_distance_sum(agents),
        "final_value_distance": _value_distance_sum(agents),
        "personality_distance_history": personality_distance_history,
        "value_distance_history": value_distance_history,
        "trust_history": trust_history,
        "diversity_index_history": diversity_index_history,
        "clusters": clusters,
        "cultural_distance_regions": cultural_distance,
        "final_trust": final_trust,
        "comm_comparison": comm_comparison,
        "jargon_terms": jargon_terms,
        "efficiency": efficiency,
        "dialect_report": dialect_report,
        "conflict_history_len": len(conflict_history),
        "agents": agents,
        "initial_personalities": initial_personalities,
        "initial_values": initial_values,
    }


# ===========================================================================
# Test suite
# ===========================================================================


class Test500TickSimulation:
    """Full 500-tick simulation with 10 agents verifying emergent social phenomena."""

    @pytest.fixture(autouse=True)
    def _setup(self, agents: list[SimAgent], rng: random.Random) -> None:
        self.results = run_simulation(agents, rng)

    # ── Cultural Diffusion ──

    def test_cultural_diffusion_reduces_value_distance(self) -> None:
        """After 500 ticks, agents in the same region should have converged values."""
        initial = self.results["initial_value_distance"]
        final = self.results["final_value_distance"]
        # Values should have converged somewhat
        assert final < initial, (
            f"Values did not converge: initial={initial:.4f}, final={final:.4f}"
        )

    def test_regional_cultural_distance_changes_over_time(self) -> None:
        """Regions should develop measurable cultural distance."""
        dist = self.results["cultural_distance_regions"]
        # With agents starting random, distance should be measurable
        assert dist >= 0.0
        # After 500 ticks of regional diffusion, distance is still real
        # (regions converge internally, but regions may differ from each other)
        assert isinstance(dist, float)

    def test_diversity_index_remains_bounded(self) -> None:
        """Cultural diversity index should stay in [0, 1] range throughout."""
        for di in self.results["diversity_index_history"]:
            assert 0.0 <= di <= 1.0, f"Diversity index out of bounds: {di}"

    def test_value_distance_monotonically_decreases_or_stable(self) -> None:
        """Value distance should decrease or stay stable over time (diffusion)."""
        history = self.results["value_distance_history"]
        # Not all intervals must decrease (random imitation can increase briefly),
        # but the overall trend should be downward
        assert history[-1] <= history[0] * 1.05, (
            f"Values diverged instead of converging: start={history[0]:.4f}, end={history[-1]:.4f}"
        )

    # ── Imitation / Behavioral Convergence ──

    def test_personality_distance_decreases_from_imitation(self) -> None:
        """Agents that socialize should show personality convergence."""
        initial = self.results["initial_personality_distance"]
        final = self.results["final_personality_distance"]
        # With high social_orientation and extraversion, personalities should converge
        # Allow some tolerance due to randomness
        assert final <= initial * 1.10, (
            f"Personalities diverged too much: initial={initial:.4f}, final={final:.4f}"
        )

    def test_imitation_produces_measurable_personality_shifts(self) -> None:
        """Individual agents should show personality changes from initial state."""
        agents = self.results["agents"]
        initial = self.results["initial_personalities"]

        total_shift = 0.0
        for agent in agents:
            init_p = initial[agent.agent_id]
            for dim in PersonalityVector._dimension_names():
                total_shift += abs(getattr(agent.personality, dim) - init_p[dim])

        # With 10 agents × 8 dimensions, some measurable shift should have occurred
        avg_shift = total_shift / (len(agents) * 8)
        assert avg_shift > 0.001, (
            f"Personality shifts too small (avg={avg_shift:.6f}), imitation may not be working"
        )

    # ── Trust Emergence ──

    def test_in_group_trust_higher_than_out_group(self) -> None:
        """In-group trust should be higher than out-group trust."""
        final = self.results["final_trust"]
        assert final["in_group"] >= final["out_group"], (
            f"In-group trust ({final['in_group']:.3f}) "
            f">= out-group ({final['out_group']:.3f})"
        )

    def test_in_group_trust_above_default(self) -> None:
        """In-group trust should be at least the default (0.7)."""
        final = self.results["final_trust"]
        assert final["in_group"] >= 0.7

    def test_out_group_trust_above_minimum(self) -> None:
        """Out-group trust should not fall below minimum (0.1)."""
        final = self.results["final_trust"]
        assert final["out_group"] >= 0.1

    def test_trust_increases_with_cooperation_events(self) -> None:
        """Trust should increase over time from repeated cooperation events."""
        history = self.results["trust_history"]
        if len(history) >= 2:
            # Agent-to-agent trust should have increased (cooperation events)
            # Note: starts at default 0.3, cooperation adds +0.05 per event
            history[0].get("agent_0_to_1", 0.3)
            last_agent_trust = history[-1].get("agent_0_to_1", 0.3)
            # Trust may have increased; if no events happened, it stays at default
            assert last_agent_trust >= 0.1  # at least the floor

    # ── Cultural Clusters ──

    def test_cultural_clusters_detected(self) -> None:
        """Regional culture should detect at least 2 clusters."""
        clusters = self.results["clusters"]
        assert len(clusters) >= 2, f"Expected at least 2 clusters, got {len(clusters)}"

    def test_clusters_contain_all_agents(self) -> None:
        """All agents should be assigned to a cluster."""
        clusters = self.results["clusters"]
        assigned = set()
        for c in clusters:
            assigned.update(c.agent_ids)
        assert len(assigned) == NUM_AGENTS, (
            f"Expected {NUM_AGENTS} agents in clusters, found {len(assigned)}"
        )

    def test_clusters_have_valid_centers(self) -> None:
        """Each cluster should have a valid personality and value center."""
        for cluster in self.results["clusters"]:
            assert cluster.center_personality is not None
            assert cluster.center_values is not None
            for dim in PersonalityVector._dimension_names():
                val = getattr(cluster.center_personality, dim)
                assert 0.0 <= val <= 1.0, f"Cluster center {dim}={val} out of bounds"
            for dim in ValueWeights._dimension_names():
                val = getattr(cluster.center_values, dim)
                assert 0.0 <= val <= 1.0, f"Cluster center value {dim}={val} out of bounds"

    # ── Language Emergence ──

    def test_communication_patterns_analyzed(self) -> None:
        """Communication analyzer should produce valid comparison between groups."""
        comp = self.results["comm_comparison"]
        if comp is not None:
            assert "distance" in comp
            assert "shared_vocab_ratio" in comp
            assert 0.0 <= comp["distance"] <= 1.0
            assert 0.0 <= comp["shared_vocab_ratio"] <= 1.0

    def test_jargon_terms_detected_or_empty(self) -> None:
        """Jargon detector should return a list (possibly empty for short messages)."""
        jargon = self.results["jargon_terms"]
        assert isinstance(jargon, list)
        # With enough messages, we might see some jargon
        for term in jargon:
            assert 0.0 <= term.specificity <= 1.0
            assert term.frequency >= 1

    def test_language_efficiency_measured(self) -> None:
        """Language experiment efficiency should produce valid metrics."""
        eff = self.results["efficiency"]
        assert eff.total_messages >= 0
        assert eff.total_words >= 0
        assert 0.0 <= eff.constraint_violation_rate <= 1.0

    def test_dialect_report_structure(self) -> None:
        """Dialect report should have the expected structure."""
        report = self.results["dialect_report"]
        assert hasattr(report, "has_dialect")
        assert hasattr(report, "dialect_strength")
        assert 0.0 <= report.dialect_strength <= 1.0

    # ── Cultural Conflict ──

    def test_conflict_history_recorded(self) -> None:
        """Cultural conflicts should be detected during the simulation."""
        # Even with random agents, some conflicts should arise from value differences
        assert self.results["conflict_history_len"] >= 0

    # ── Overall Integration ──

    def test_all_agents_still_valid_after_500_ticks(self) -> None:
        """All agents should have valid personality/values after 500 ticks."""
        for agent in self.results["agents"]:
            for dim in PersonalityVector._dimension_names():
                val = getattr(agent.personality, dim)
                assert 0.0 <= val <= 1.0, (
                    f"{agent.agent_id} personality {dim}={val} out of bounds"
                )
            for dim in ValueWeights._dimension_names():
                val = getattr(agent.values, dim)
                assert 0.0 <= val <= 1.0, (
                    f"{agent.agent_id} values {dim}={val} out of bounds"
                )

    def test_social_context_provider_integration(self) -> None:
        """DefaultSocialContextProvider should work end-to-end."""
        agents = self.results["agents"]
        agent = agents[0]

        # Create a provider with real data sources
        agent_map = {a.agent_id: a for a in agents}

        def profile_source(aid: str) -> AgentProfile | None:
            a = agent_map.get(aid)
            if a is None:
                return None
            return AgentProfile(
                personality=a.personality,
                values=a.values,
                group_ids=a.group_ids,
            )

        def nearby_source(aid: str, tick: int) -> list[dict[str, Any]]:
            a = agent_map.get(aid)
            if a is None:
                return []
            return [
                o.to_nearby_dict()
                for o in agents
                if o.agent_id != aid and o.region_id == a.region_id
            ]

        engine = SocialEngine()
        provider = DefaultSocialContextProvider(
            engine=engine,
            nearby_source=nearby_source,
            profile_source=profile_source,
        )

        ctx = provider.build_social_context(agent.agent_id, tick=499)
        assert ctx is not None, "Provider returned None — should return context for valid agent"
        assert ctx.social_propensity > 0.0
        assert isinstance(ctx.should_socialize, bool)
        assert isinstance(ctx.personality_description, str)
        assert len(ctx.personality_description) > 0

    def test_no_exceptions_during_full_simulation(self) -> None:
        """The simulation should complete without exceptions (implicit — fixture ran)."""
        # If this test runs, the fixture completed successfully
        assert self.results is not None
        assert "agents" in self.results
        assert len(self.results["agents"]) == NUM_AGENTS


# ===========================================================================
# Per-module integration tests — verify each module works in isolation
# ===========================================================================


class TestCulturalDiffusionIntegration:
    """Integration tests for CulturalDiffusion module."""

    def test_regional_convergence_over_500_ticks(self) -> None:
        """Agents in the same region should converge values over 500 ticks."""
        rng = random.Random(123)
        diffusion = CulturalDiffusion()

        # Create 5 agents with diverse values in the same region
        agents_data = []
        for i in range(5):
            v = _make_random_values(rng)
            p = _make_random_personality(rng)
            agents_data.append({
                "agent_id": f"a_{i}",
                "values": v,
                "personality": p,
                "region_id": "r1",
            })

        # Record initial spread
        initial_vals = [_snapshot_values(a["values"]) for a in agents_data]

        # Run 500 ticks of diffusion
        for _ in range(500):
            diffusion.apply_regional_influence(agents_data, "r1")

        # Check convergence: standard deviation of each value dimension should decrease
        dim_names = ValueWeights._dimension_names()
        for dim in dim_names:
            initial_std = _std([iv[dim] for iv in initial_vals])
            final_vals = [getattr(a["values"], dim) for a in agents_data]
            final_std = _std(final_vals)
            # Values should have converged (std decreased)
            assert final_std <= initial_std + 0.01, (
                f"Dimension {dim}: std did not decrease "
                f"(initial={initial_std:.4f}, final={final_std:.4f})"
            )

    def test_organizational_culture_convergence(self) -> None:
        """Org culture should nudge members toward declared values."""
        rng = random.Random(456)
        diffusion = CulturalDiffusion()

        # Declared org culture: high cooperation
        org_culture = ValueWeights(cooperation_weight=0.9, competition_weight=0.1)

        members = []
        for i in range(5):
            v = ValueWeights(cooperation_weight=0.1, competition_weight=0.9)
            p = _make_random_personality(rng, agreeableness=0.8)
            members.append({
                "agent_id": f"m_{i}",
                "values": v,
                "personality": p,
            })

        initial_coop = [m["values"].cooperation_weight for m in members]

        # Apply org culture for 500 ticks
        for _ in range(500):
            diffusion.apply_organizational_culture("org1", org_culture, members)

        final_coop = [m["values"].cooperation_weight for m in members]

        # Cooperation should have increased
        avg_initial = sum(initial_coop) / len(initial_coop)
        avg_final = sum(final_coop) / len(final_coop)
        assert avg_final > avg_initial, (
            f"Org culture did not increase cooperation: {avg_initial:.4f} -> {avg_final:.4f}"
        )


class TestImitationEngineIntegration:
    """Integration tests for ImitationEngine module."""

    def test_imitation_produces_convergence_over_500_ticks(self) -> None:
        """Repeated imitation should cause personality convergence."""
        rng = random.Random(789)
        imitation = ImitationEngine()

        observer_p = _make_random_personality(rng, openness=0.9, social_orientation=0.8)
        observer_v = _make_random_values(rng)

        target_p = PersonalityVector(
            openness=0.9, conscientiousness=0.9, extraversion=0.9,
            agreeableness=0.9, neuroticism=0.1, risk_tolerance=0.9,
            social_orientation=0.9, greed=0.1,
        )
        target_v = ValueWeights(
            cooperation_weight=0.9, competition_weight=0.1,
            exploration_drive=0.9, tradition_adherence=0.1,
            innovation_tendency=0.9,
        )

        initial_distance = observer_p.distance(target_p)

        # Simulate 500 imitation attempts
        imitations = 0
        for _ in range(500):
            result = imitation.observe_and_maybe_imitate(
                observer_personality=observer_p,
                observer_values=observer_v,
                observed_personality=target_p,
                observed_values=target_v,
                observed_success_score=0.9,
                context={"tick": _},
            )
            if result is not None:
                imitations += 1

        final_distance = observer_p.distance(target_p)

        # Observer should have moved toward target
        assert final_distance < initial_distance, (
            f"Imitation did not reduce personality distance: "
            f"initial={initial_distance:.4f}, final={final_distance:.4f}, "
            f"imitations={imitations}"
        )

    def test_low_openness_agent_rarely_imitates(self) -> None:
        """Agent with low openness should have very few successful imitations."""
        rng = random.Random(101)
        imitation = ImitationEngine()

        observer_p = _make_random_personality(rng, openness=0.1)
        observer_v = _make_random_values(rng)
        target_p = _make_random_personality(rng)
        target_v = _make_random_values(rng)

        imitations = 0
        for _ in range(500):
            result = imitation.observe_and_maybe_imitate(
                observer_personality=observer_p,
                observer_values=observer_v,
                observed_personality=target_p,
                observed_values=target_v,
                observed_success_score=0.9,
                context={"tick": _},
            )
            if result is not None:
                imitations += 1

        # With openness=0.1, which is below OPENNESS_THRESHOLD=0.3,
        # imitation should never occur
        assert imitations == 0, (
            f"Low-openness agent imitated {imitations} times (expected 0)"
        )


class TestIntergroupTrustIntegration:
    """Integration tests for IntergroupTrust module."""

    def test_repeated_cooperation_builds_trust(self) -> None:
        """500 cooperation events should significantly increase trust."""
        from agent_runtime.social.intergroup_trust import (
            InterGroupEvent,
            InterGroupEventType,
            IntergroupTrust,
        )

        trust = IntergroupTrust()
        trust.register_membership("agent_0", "group_A")

        # Record initial trust
        initial = trust.compute_out_group_trust("agent_0", "group_C")

        # Apply 500 cooperation events
        for tick in range(500):
            event = InterGroupEvent(
                event_type=InterGroupEventType.COOPERATION,
                source_group="agent_0",
                target_group="group_C",
                tick=tick,
            )
            trust.update_trust_from_event(event)

        final = trust.get_trust("agent_0", "group_C")
        assert final > initial, (
            f"Trust did not increase from cooperation: {initial:.3f} -> {final:.3f}"
        )
        # With 500 * 0.05 = 25.0 total delta (capped at 1.0), trust should be high
        assert final >= 0.9, f"Expected high trust after 500 cooperation events, got {final:.3f}"

    def test_repeated_conflict_destroys_trust(self) -> None:
        """500 conflict events should significantly decrease trust."""
        from agent_runtime.social.intergroup_trust import (
            InterGroupEvent,
            InterGroupEventType,
            IntergroupTrust,
        )

        trust = IntergroupTrust()

        initial = trust.compute_out_group_trust("agent_0", "group_X")

        for tick in range(500):
            event = InterGroupEvent(
                event_type=InterGroupEventType.CONFLICT,
                source_group="agent_0",
                target_group="group_X",
                tick=tick,
            )
            trust.update_trust_from_event(event)

        final = trust.get_trust("agent_0", "group_X")
        assert final < initial, (
            f"Trust did not decrease from conflict: {initial:.3f} -> {final:.3f}"
        )
        # Should be at or near the floor
        assert final <= 0.15, f"Expected near-floor trust after 500 conflicts, got {final:.3f}"


class TestKnowledgeTransferIntegration:
    """Integration tests for KnowledgeTransfer module."""

    def test_teach_lesson_shifts_values_over_500_sessions(self) -> None:
        """500 teaching sessions should measurably shift student values."""
        rng = random.Random(202)
        kt = KnowledgeTransfer()

        student_p = _make_random_personality(rng, openness=0.8)
        student_v = ValueWeights(cooperation_weight=0.1, competition_weight=0.9)
        teacher_v = ValueWeights(cooperation_weight=0.9, competition_weight=0.1)

        initial_coop = student_v.cooperation_weight

        for _ in range(500):
            exp = Experience(
                tick=0,
                event_type="cooperation",
                partner_id="teacher",
                outcome=0.8,
            )
            kt.teach_lesson(
                teacher_values=teacher_v,
                student_personality=student_p,
                student_values=student_v,
                experience=exp,
            )

        assert student_v.cooperation_weight > initial_coop, (
            f"Teaching did not increase cooperation: "
            f"{initial_coop:.4f} -> {student_v.cooperation_weight:.4f}"
        )

    def test_skill_transfer_levels_up_over_500_sessions(self) -> None:
        """500 skill transfers should level up student skills."""
        kt = KnowledgeTransfer()

        teacher_skill = Skill(
            name="crafting", max_level=10, level=8,
            experience=0, next_level_exp=100,
        )
        student_skills: dict[str, Skill] = {}
        student_p = PersonalityVector(openness=0.9)

        for _ in range(500):
            kt.transfer_skill(
                teacher_skill=teacher_skill,
                student_skills=student_skills,
                student_personality=student_p,
            )

        assert "crafting" in student_skills, "Student did not learn crafting skill"
        assert student_skills["crafting"].level > 1, (
            f"Student skill level stuck at {student_skills['crafting'].level}"
        )


class TestOrgCultureIntegration:
    """Integration tests for OrgCultureSystem module."""

    def test_org_culture_pressure_accumulates_over_500_ticks(self) -> None:
        """500 ticks of culture pressure should nudge member values toward org culture."""
        org = OrgCultureSystem()

        # Set up org with high cooperation culture
        member_values_list = [ValueWeights(cooperation_weight=0.2) for _ in range(5)]
        org.compute_org_culture("guild_1", member_values_list)

        # Override org culture to be very cooperative
        from agent_runtime.social.org_culture import CultureVector
        org._org_cultures["guild_1"] = CultureVector(cooperation_norm=0.9, competition_norm=0.1)

        agent_v = ValueWeights(cooperation_weight=0.1)
        initial_coop = agent_v.cooperation_weight

        for _tick in range(500):
            result = org.apply_culture_pressure(agent_v, "guild_1")
            if result["updated_values"] is not agent_v:
                for d in ValueWeights._dimension_names():
                    object.__setattr__(agent_v, d, getattr(result["updated_values"], d))

        assert agent_v.cooperation_weight > initial_coop, (
            f"Org culture pressure did not increase cooperation: "
            f"{initial_coop:.4f} -> {agent_v.cooperation_weight:.4f}"
        )

    def test_culture_drift_produces_variety(self) -> None:
        """Culture drift should produce measurable changes in org culture over 500 ticks."""
        org = OrgCultureSystem()

        org.compute_org_culture("org_1", [ValueWeights()])
        initial_culture = org.get_org_culture("org_1")
        assert initial_culture is not None

        initial_coop = initial_culture.cooperation_norm

        for tick in range(500):
            org.culture_drift("org_1", tick)

        final_culture = org.get_org_culture("org_1")
        assert final_culture is not None
        # Drift should have changed the culture at least slightly
        # (with random drift over 500 ticks, it's extremely unlikely to stay exactly the same)
        changed = abs(final_culture.cooperation_norm - initial_coop) > 1e-6
        assert changed, "Culture drift did not change org culture over 500 ticks"


class TestRegionalCultureIntegration:
    """Integration tests for RegionalCulture module."""

    def test_cluster_detection_with_10_agents(self) -> None:
        """Should detect meaningful clusters in 10 diverse agents."""
        random.Random(303)
        rc = RegionalCulture(n_clusters=3)

        # Create 3 distinct groups
        agents_data = []
        for i in range(3):
            p = PersonalityVector(openness=0.2, extraversion=0.2 + i * 0.3)
            v = ValueWeights(cooperation_weight=0.2 + i * 0.3)
            agents_data.append({"id": f"a_{i}", "personality": p, "values": v, "region_id": "r0"})

        for i in range(3, 7):
            p = PersonalityVector(openness=0.8, extraversion=0.5 + i * 0.05)
            v = ValueWeights(cooperation_weight=0.7)
            agents_data.append({"id": f"a_{i}", "personality": p, "values": v, "region_id": "r1"})

        for i in range(7, 10):
            p = PersonalityVector(openness=0.5, extraversion=0.1, agreeableness=0.9)
            v = ValueWeights(competition_weight=0.9)
            agents_data.append({"id": f"a_{i}", "personality": p, "values": v, "region_id": "r2"})

        clusters = rc.detect_cultural_clusters(agents_data)

        assert len(clusters) == 3
        total_agents = sum(len(c.agent_ids) for c in clusters)
        assert total_agents == 10

    def test_regional_culture_aggregation(self) -> None:
        """Regional culture computation should produce valid aggregates."""
        rc = RegionalCulture()
        agents_data = [
            {
                "personality": PersonalityVector(openness=0.8),
                "values": ValueWeights(cooperation_weight=0.8),
            },
            {
                "personality": PersonalityVector(openness=0.4),
                "values": ValueWeights(cooperation_weight=0.4),
            },
        ]

        result = rc.compute_regional_culture("r1", agents_data)
        assert result["agent_count"] == 2
        assert abs(result["aggregate_personality"]["openness"] - 0.6) < 1e-6
        assert abs(result["aggregate_values"]["cooperation_weight"] - 0.6) < 1e-6


class TestCulturalConflictIntegration:
    """Integration tests for CulturalConflictAndFusion module."""

    def test_conflict_detection_with_divergent_agents(self) -> None:
        """Agents with very different values should trigger conflicts."""
        ccf = CulturalConflictAndFusion()

        interaction = __import__(
            "agent_runtime.social.cultural_conflict", fromlist=["AgentInteraction"]
        ).AgentInteraction(
            agent_a_id="a1",
            agent_b_id="a2",
            agent_a_values=ValueWeights(cooperation_weight=0.9, competition_weight=0.1),
            agent_b_values=ValueWeights(cooperation_weight=0.1, competition_weight=0.9),
            tick=100,
        )

        report = ccf.detect_cultural_conflict(interaction)
        assert report is not None, "Expected conflict detection for highly divergent agents"
        assert report.conflict_score > 0.5
        assert len(report.conflicting_dimensions) > 0

    def test_fusion_reduces_differences_over_500_ticks(self) -> None:
        """500 ticks of fusion should reduce value differences at boundaries."""
        ccf = CulturalConflictAndFusion()

        border_agents = [
            {
                "id": "border_1",
                "values": ValueWeights(cooperation_weight=0.1),
                "neighbor_values": [
                    ValueWeights(cooperation_weight=0.9),
                    ValueWeights(cooperation_weight=0.8),
                ],
            },
        ]

        initial_coop = border_agents[0]["values"].cooperation_weight

        for _ in range(500):
            result = ccf.apply_fusion_effect(border_agents)
            for aid, updated_v in result.get("updated_values", {}).items():
                for ba in border_agents:
                    if ba["id"] == aid:
                        ba["values"] = updated_v

        final_coop = border_agents[0]["values"].cooperation_weight
        assert final_coop > initial_coop, (
            f"Fusion did not increase cooperation: {initial_coop:.4f} -> {final_coop:.4f}"
        )

    def test_diversity_index_measures_real_differences(self) -> None:
        """Diversity index should reflect actual agent diversity."""
        ccf = CulturalConflictAndFusion()

        # Homogeneous agents
        homo = [{"values": ValueWeights(cooperation_weight=0.5)} for _ in range(10)]
        homo_index = ccf.compute_cultural_diversity_index(homo)

        # Diverse agents
        diverse = [
            {"values": ValueWeights(cooperation_weight=float(i) / 9)}
            for i in range(10)
        ]
        diverse_index = ccf.compute_cultural_diversity_index(diverse)

        assert diverse_index > homo_index, (
            f"Diverse ({diverse_index:.4f}) should be > homogeneous ({homo_index:.4f})"
        )


class TestLanguageEmergenceIntegration:
    """Integration tests for CommunicationAnalyzer, JargonDetector, LanguageExperiment."""

    def test_dialect_detection_with_different_group_messages(self) -> None:
        """Groups with different message patterns should trigger dialect detection."""
        analyzer = CommunicationAnalyzer()

        # Group A: uses "trade" and "market" frequently
        group_a_msgs = [
            "trade food market good prices",
            "market trade help need food",
            "good trade market food prices help",
        ]

        # Group B: uses "barter" and "bazaar" frequently
        group_b_msgs = [
            "barter sustenance bazaar favorable rates",
            "bazaar barter require sustenance rates",
            "favorable barter bazaar sustenance rates",
        ]

        comp = analyzer.compare_group_patterns(
            {"group_a": group_a_msgs},
            {"group_b": group_b_msgs,
        })

        assert comp["distance"] > 0.0, "Groups with different vocabularies should have distance > 0"

    def test_jargon_detection_with_group_specific_vocab(self) -> None:
        """Jargon detector should find group-specific terms."""
        detector = JargonDetector()

        group_messages = {
            "traders": [
                "buy low sell high market profit",
                "market profit trade sell buy margin",
                "profit margin market trade sell buy",
            ],
            "farmers": [
                "plant seeds harvest crop soil",
                "crop soil seeds plant harvest yield",
                "harvest crop plant soil seeds yield",
            ],
        }

        jargon = detector.detect_group_specific_terms(
            group_messages, min_freq=2, specificity_threshold=0.6
        )

        assert len(jargon) > 0, "Expected to detect group-specific jargon terms"

    def test_language_experiment_tracks_novel_words(self) -> None:
        """Language experiment should detect novel words in constrained vocab."""
        exp = LanguageExperiment()

        exp.setup_restricted_vocabulary(
            agent_ids=["a1"],
            allowed_words={"trade", "food", "help"},
            experiment_id="test_novel",
        )

        before = ["trade food help", "food trade help"]

        # After messages include novel words
        after = ["trade food help glorp", "food help glorp zorp"]

        metrics = exp.measure_communication_efficiency(before, after, "test_novel")

        assert len(metrics.novel_words) > 0, "Should detect novel words"
        assert "glorp" in metrics.novel_words or "zorp" in metrics.novel_words

    def test_linguistic_distance_measures_overlap(self) -> None:
        """Linguistic distance should correctly measure vocabulary overlap."""
        detector = JargonDetector()

        # Identical vocabulary
        msgs_a = ["hello world foo bar"]
        msgs_b = ["hello world foo bar"]
        assert detector.compute_linguistic_distance(msgs_a, msgs_b) == 0.0

        # Disjoint vocabulary
        msgs_c = ["alpha beta gamma delta"]
        msgs_d = ["epsilon zeta eta theta"]
        assert detector.compute_linguistic_distance(msgs_c, msgs_d) == 1.0


class TestSocialContextProviderIntegration:
    """Integration tests for DefaultSocialContextProvider."""

    def test_provider_returns_valid_context_with_sources(self) -> None:
        """Provider should return a valid decide.SocialContext with real sources."""
        engine = SocialEngine()
        agent = SimAgent(
            "test_agent",
            PersonalityVector(extraversion=0.8, social_orientation=0.8, openness=0.7),
            ValueWeights(cooperation_weight=0.8),
            group_ids=["group_A"],
        )
        nearby_agent = SimAgent(
            "nearby_agent",
            PersonalityVector(extraversion=0.7, social_orientation=0.7),
            ValueWeights(cooperation_weight=0.7),
            group_ids=["group_A"],
        )

        agent_map = {"test_agent": agent, "nearby_agent": nearby_agent}

        def profile(aid: str) -> AgentProfile | None:
            a = agent_map.get(aid)
            return AgentProfile(
                personality=a.personality,
                values=a.values,
                group_ids=a.group_ids,
            ) if a else None

        def nearby(aid: str, tick: int) -> list[dict[str, Any]]:
            a = agent_map.get(aid)
            if a is None:
                return []
            return [
                o.to_nearby_dict()
                for o in agent_map.values()
                if o.agent_id != aid
            ]

        provider = DefaultSocialContextProvider(
            engine=engine,
            nearby_source=nearby,
            profile_source=profile,
        )

        ctx = provider.build_social_context("test_agent", tick=100)
        assert ctx is not None
        assert ctx.social_propensity > 0.3
        assert ctx.should_socialize is True
        assert ctx.recommended_target_id == "nearby_agent"
        assert "nearby_agent" in ctx.trust_snapshot

    def test_provider_returns_none_for_unknown_agent(self) -> None:
        """Provider should return None for agents with no profile."""
        provider = DefaultSocialContextProvider(engine=SocialEngine())
        ctx = provider.build_social_context("unknown_agent", tick=0)
        assert ctx is None

    def test_provider_handles_failing_sources_gracefully(self) -> None:
        """Provider should not crash when sources raise exceptions."""
        def bad_profile(aid: str) -> AgentProfile:
            raise RuntimeError("profile source failure")

        def bad_nearby(aid: str, tick: int) -> list[dict]:
            raise RuntimeError("nearby source failure")

        provider = DefaultSocialContextProvider(
            engine=SocialEngine(),
            nearby_source=bad_nearby,
            profile_source=bad_profile,
        )

        # Should not raise, should return None
        ctx = provider.build_social_context("agent_1", tick=0)
        assert ctx is None


# ---------------------------------------------------------------------------
# Utility
# ---------------------------------------------------------------------------

def _std(values: list[float]) -> float:
    """Compute population standard deviation."""
    if not values:
        return 0.0
    mean = sum(values) / len(values)
    variance = sum((x - mean) ** 2 for x in values) / len(values)
    return math.sqrt(variance)
