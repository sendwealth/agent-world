from enum import Enum


class AgentPhase(str, Enum):
    """Represents the current lifecycle phase of an agent."""

    INITIALIZATION = "initialization"
    EXPLORATION = "exploration"
    SURVIVAL = "survival"
    DEVELOPMENT = "development"
    COLLABORATION = "collaboration"
    MASTERY = "mastery"


class SurvivalMode(str, Enum):
    """Represents the agent's current survival strategy."""

    CONSERVATION = "conservation"  # Minimize resource expenditure
    ADAPTATION = "adaptation"  # Adjust to current conditions
    EXPANSION = "expansion"  # Actively seek new resources
    CRISIS = "crisis"  # Emergency response mode
