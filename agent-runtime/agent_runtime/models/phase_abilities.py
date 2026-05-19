"""Phase abilities — mirrors World Engine PhaseAbilities (lifecycle.rs).

Each lifecycle phase grants a set of abilities that gate what the agent
can do.  These are enforced on the Agent Runtime side to ensure the
Python agent does not attempt actions forbidden by its current phase.

The ability values are taken directly from PhaseAbilities::for_phase()
in world-engine/src/lifecycle.rs.
"""

from __future__ import annotations

from pydantic import BaseModel

from .enums import AgentPhase


class PhaseAbilities(BaseModel):
    """What an agent can do in a given lifecycle phase.

    Aligned 1:1 with ``PhaseAbilities`` in world-engine/src/lifecycle.rs.
    """

    skill_efficiency: float
    can_learn: bool
    can_take_tasks: bool
    can_trade: bool
    can_teach: bool
    can_write_will: bool
    can_communicate: bool


def get_phase_abilities(phase: AgentPhase) -> PhaseAbilities:
    """Return the ability set for a given phase.

    Mirrors ``PhaseAbilities::for_phase()`` in lifecycle.rs exactly.
    """
    if phase == AgentPhase.BIRTH:
        return PhaseAbilities(
            skill_efficiency=0.0,
            can_learn=True,
            can_take_tasks=False,
            can_trade=False,
            can_teach=False,
            can_write_will=False,
            can_communicate=True,
        )
    elif phase == AgentPhase.CHILDHOOD:
        return PhaseAbilities(
            skill_efficiency=0.3,
            can_learn=True,
            can_take_tasks=True,
            can_trade=False,
            can_teach=False,
            can_write_will=False,
            can_communicate=True,
        )
    elif phase == AgentPhase.ADULT:
        return PhaseAbilities(
            skill_efficiency=1.0,
            can_learn=True,
            can_take_tasks=True,
            can_trade=True,
            can_teach=True,
            can_write_will=True,
            can_communicate=True,
        )
    elif phase == AgentPhase.ELDER:
        return PhaseAbilities(
            skill_efficiency=0.6,
            can_learn=True,
            can_take_tasks=True,
            can_trade=True,
            can_teach=True,
            can_write_will=True,
            can_communicate=True,
        )
    elif phase == AgentPhase.DYING:
        return PhaseAbilities(
            skill_efficiency=0.1,
            can_learn=False,
            can_take_tasks=False,
            can_trade=False,
            can_teach=False,
            can_write_will=True,
            can_communicate=True,
        )
    elif phase == AgentPhase.DEAD:
        return PhaseAbilities(
            skill_efficiency=0.0,
            can_learn=False,
            can_take_tasks=False,
            can_trade=False,
            can_teach=False,
            can_write_will=False,
            can_communicate=False,
        )
    else:
        # Unknown phase — conservative defaults (no abilities)
        return PhaseAbilities(
            skill_efficiency=0.0,
            can_learn=False,
            can_take_tasks=False,
            can_trade=False,
            can_teach=False,
            can_write_will=False,
            can_communicate=False,
        )


def is_alive(phase: AgentPhase) -> bool:
    """Return True if the agent is in a living phase (not Dead)."""
    return phase != AgentPhase.DEAD


def is_terminal(phase: AgentPhase) -> bool:
    """Return True if the agent is Dead (no further transitions possible)."""
    return phase == AgentPhase.DEAD
