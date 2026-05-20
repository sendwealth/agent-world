"""Social interaction modules — knowledge transfer, imitation, cultural diffusion."""

from agent_runtime.social.cultural_diffusion import CulturalDiffusion
from agent_runtime.social.imitation import ImitationEngine
from agent_runtime.social.knowledge_transfer import KnowledgeTransfer

__all__ = [
    "CulturalDiffusion",
    "ImitationEngine",
    "KnowledgeTransfer",
]
