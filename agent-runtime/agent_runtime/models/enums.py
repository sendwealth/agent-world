"""Lifecycle and survival enums — aligned with World Engine (lifecycle.rs).

The AgentPhase enum mirrors the Rust ``AgentPhase`` in world-engine/src/world/enums.rs:
    Birth → Childhood → Adult → Elder → Dying → Dead

DeathReason mirrors ``DeathReason`` in the same file.

These enums are the ground truth for the Python Agent Runtime; all lifecycle
logic should reference these values rather than hard-coding strings.
"""

from enum import StrEnum


class AgentPhase(StrEnum):
    """Lifecycle phases — aligned 1:1 with World Engine AgentPhase.

    Phase transition rules (from lifecycle.rs LifecycleMachine::can_transition):
        Birth → Childhood           (tick 1 after spawn)
        Childhood → Adult           (after childhood_ticks)
        Adult → Elder               (after adult_ticks)
        Elder → Dead                (natural death, after elder_ticks)
        Any living → Dying          (token depletion or other triggers)
        Dying → Dead                (grace period expired)
        Dying → Adult               (rescued)

    Dead is terminal — no transitions out.
    """

    BIRTH = "birth"
    CHILDHOOD = "childhood"
    ADULT = "adult"
    ELDER = "elder"
    DYING = "dying"
    DEAD = "dead"


class DeathReason(StrEnum):
    """Reason an agent has died — mirrors World Engine DeathReason."""

    TOKEN_DEPLETED = "token_depleted"
    HUMAN_TERMINATED = "human_terminated"
    VOTE_EVICTED = "vote_evicted"
    NATURAL_DEATH = "natural_death"


class SurvivalMode(StrEnum):
    """Agent survival strategy — used by SurvivalInstinct.

    This is a *runtime-local* concern (not mirrored in World Engine).
    It determines how the agent prioritises token expenditure.
    """

    CONSERVATION = "conservation"  # Minimize resource expenditure
    ADAPTATION = "adaptation"  # Adjust to current conditions
    EXPANSION = "expansion"  # Actively seek new resources
    CRISIS = "crisis"  # Emergency response mode
