use serde::{Deserialize, Serialize};

/// Supported currencies in the world economy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Currency {
    Token,
    Money,
}

/// Life phases of an agent, affecting token consumption rates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentPhase {
    Birth,
    Childhood,
    Adult,
    Elder,
    Dying,
    Dead,
}

/// Reason an agent has died.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeathReason {
    TokenDepleted,
    HumanTerminated,
    VoteEvicted,
}
