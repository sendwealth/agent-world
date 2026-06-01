"""Emotion subsystem — dynamic agent mood and affective state.

Provides emotional modeling for agents using a PAD (Pleasure-Arousal-Dominance)
model with discrete emotion labels. The EmotionEngine transforms game events
into emotional states, modulated by personality traits, and decays over time
toward a personality-derived baseline.

Key types:
    EmotionalState: The current affective state (valence, arousal, dominance + labels).
    EmotionType: Discrete emotion categories (happy, sad, angry, etc.).
    EmotionEngine: Maintains and updates emotional state over the agent's lifecycle.
"""

from agent_runtime.emotion.engine import EmotionEngine, ThinkLoopEmotionHook
from agent_runtime.emotion.mood import EmotionalState, EmotionType

__all__ = [
    "EmotionEngine",
    "EmotionalState",
    "EmotionType",
    "ThinkLoopEmotionHook",
]
