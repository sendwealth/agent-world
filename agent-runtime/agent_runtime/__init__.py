"""Agent World — Agent Runtime

The brain of each agent: perceive, decide, act.
"""

__version__ = "1.0.0"

# ── Shared token defaults (single source of truth) ───────────────────────────
#
# These values must match config/genesis.yaml (economy.initial_tokens,
# lifecycle.birth_tokens) and AgentSpawnConfig defaults.
# If you change them here, keep genesis.yaml in sync.

DEFAULT_INITIAL_TOKENS: int = 100_000
DEFAULT_MAX_TOKENS: int = 200_000
