//! InterventionChecker subsystem — post-dispatch safety gate (engine side).
//!
//! This is the **world-engine-side** defense layer that runs as the first
//! subsystem in the tick pipeline. It validates agent state each tick and
//! emits `RuleViolated` events for any agent that should be blocked from
//! acting.
//!
//! Safety rules (mirrors the Python-side `InterventionChecker`):
//!   IC-01: Broadcast rate-limit (tracks broadcasts per tick)
//!   IC-02: Message size limit (noted but enforced at A2A layer)
//!   IC-03: Newbie protection (agents within first N ticks are protected)
//!   IC-04: Token sufficiency (zero-token agents flagged)
//!   IC-05: Death lock (dead/dying agents cannot act — enforced here too)
//!
//! This subsystem does NOT mutate agent state — it only emits events
//! for audit and monitoring. Actual action blocking happens at the
//! A2A service layer via the `MessageInterventionGuard`.

use std::collections::HashMap;

use uuid::Uuid;

use crate::economy::token_burn::AgentRecord;
use crate::world::enums::AgentPhase;
use crate::world::event::WorldEvent;
use crate::world::subsystem::Subsystem;

// ═══════════════════════════════════════════════════════════════════════════
// Configuration
// ═══════════════════════════════════════════════════════════════════════════

/// Configuration for the InterventionChecker subsystem.
#[derive(Debug, Clone)]
pub struct InterventionConfig {
    /// IC-01: Maximum broadcasts allowed per agent per tick.
    pub broadcast_max_per_tick: u32,

    /// IC-03: Number of ticks after spawn during which an agent is protected.
    pub newbie_protection_ticks: u64,

    /// IC-04: Token threshold below which agents are flagged.
    pub token_threshold: u64,
}

impl Default for InterventionConfig {
    fn default() -> Self {
        Self {
            broadcast_max_per_tick: 5,
            newbie_protection_ticks: 10,
            token_threshold: 0,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// InterventionChecker Subsystem
// ═══════════════════════════════════════════════════════════════════════════

/// Post-dispatch safety checker that runs as the first subsystem each tick.
///
/// It validates the state of every agent and emits `RuleViolated` events
/// for any agent that should be blocked from future actions. This is the
/// engine-side "second opinion" that catches anything the runtime-side
/// checker missed.
pub struct InterventionCheckerSubsystem {
    config: InterventionConfig,
    /// Tracks broadcast counts per agent per tick.
    broadcast_counts: std::sync::Mutex<HashMap<String, u32>>,
    /// Last tick we processed (for resetting broadcast counts).
    last_tick: std::sync::Mutex<u64>,
}

impl InterventionCheckerSubsystem {
    pub fn new(config: InterventionConfig) -> Self {
        Self {
            config,
            broadcast_counts: std::sync::Mutex::new(HashMap::new()),
            last_tick: std::sync::Mutex::new(0),
        }
    }

    /// Record a broadcast from an agent. Called by the A2A layer.
    /// Returns `true` if the broadcast is allowed, `false` if rate-limited.
    pub fn record_broadcast(&self, agent_id: &str, current_tick: u64) -> bool {
        // Reset counts if we're in a new tick
        {
            let mut last = self.last_tick.lock().unwrap();
            if *last != current_tick {
                self.broadcast_counts.lock().unwrap().clear();
                *last = current_tick;
            }
        }

        let mut counts = self.broadcast_counts.lock().unwrap();
        let count = counts.entry(agent_id.to_string()).or_insert(0);
        if *count >= self.config.broadcast_max_per_tick {
            return false;
        }
        *count += 1;
        true
    }
}

impl Subsystem for InterventionCheckerSubsystem {
    fn name(&self) -> &str {
        "intervention_checker"
    }

    fn on_tick(&self, tick: u64, agents: &mut [(Uuid, u64, AgentRecord)]) -> Vec<WorldEvent> {
        let mut events = Vec::new();

        // Reset broadcast tracking for new tick
        {
            let mut last = self.last_tick.lock().unwrap();
            if *last != tick {
                self.broadcast_counts.lock().unwrap().clear();
                *last = tick;
            }
        }

        for (_id, spawn_tick, agent) in agents.iter() {
            let agent_id_str = agent.id.to_string();

            // IC-05: Death lock — dead/dying agents cannot act
            if agent.phase == AgentPhase::Dead {
                // Emit audit event (the agent is already dead, but this is
                // for logging/monitoring purposes)
                events.push(WorldEvent::RuleViolated {
                    agent_id: agent_id_str.clone(),
                    rule: "IC-05".to_string(),
                    details: format!("Agent is Dead — no actions allowed (tick {})", tick),
                });
                continue; // No further checks needed for dead agents
            }

            if agent.phase == AgentPhase::Dying {
                events.push(WorldEvent::RuleViolated {
                    agent_id: agent_id_str.clone(),
                    rule: "IC-05".to_string(),
                    details: format!("Agent is Dying — no actions allowed (tick {})", tick),
                });
                continue;
            }

            // IC-03: Newbie protection check
            let age = tick.saturating_sub(*spawn_tick);
            if age < self.config.newbie_protection_ticks {
                // Not a violation per se — just emit an informational event
                // that this agent is still in protection period.
                // We don't block their own actions, just flag them.
                tracing::debug!(
                    agent_id = %agent_id_str,
                    age = age,
                    protection = self.config.newbie_protection_ticks,
                    "Agent in newbie protection period"
                );
            }

            // IC-04: Token sufficiency — flag agents at zero tokens
            if agent.tokens <= self.config.token_threshold && agent.phase != AgentPhase::Birth {
                events.push(WorldEvent::RuleViolated {
                    agent_id: agent_id_str.clone(),
                    rule: "IC-04".to_string(),
                    details: format!(
                        "Agent has {} tokens (threshold: {}) — high-cost actions blocked",
                        agent.tokens, self.config.token_threshold
                    ),
                });
            }
        }

        events
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Message Intervention Guard (used by A2A service layer)
// ═══════════════════════════════════════════════════════════════════════════

/// Lightweight guard that the A2A service layer calls before routing messages.
/// Performs IC-01 (broadcast rate-limit) and IC-02 (message size limit).
pub struct MessageInterventionGuard {
    /// IC-01: Maximum broadcasts per agent per tick.
    pub broadcast_max_per_tick: u32,
    /// IC-02: Maximum payload size in bytes.
    pub max_payload_bytes: usize,
}

impl Default for MessageInterventionGuard {
    fn default() -> Self {
        Self {
            broadcast_max_per_tick: 5,
            max_payload_bytes: 65_536, // 64 KiB
        }
    }
}

impl MessageInterventionGuard {
    pub fn new(broadcast_max_per_tick: u32, max_payload_bytes: usize) -> Self {
        Self {
            broadcast_max_per_tick,
            max_payload_bytes,
        }
    }

    /// Check if a broadcast is allowed for the given agent.
    /// Returns `Ok(())` if allowed, `Err(reason)` if blocked.
    pub fn check_broadcast(&self, agent_id: &str, broadcast_count: u32) -> Result<(), String> {
        if broadcast_count >= self.broadcast_max_per_tick {
            return Err(format!(
                "[IC-01] Broadcast rate limit exceeded for agent {} (max {}/tick)",
                agent_id, self.broadcast_max_per_tick
            ));
        }
        Ok(())
    }

    /// Check if a message payload is within size limits.
    /// Returns `Ok(())` if allowed, `Err(reason)` if blocked.
    pub fn check_message_size(&self, payload_size: usize) -> Result<(), String> {
        if payload_size > self.max_payload_bytes {
            return Err(format!(
                "[IC-02] Payload too large: {} bytes (max {})",
                payload_size, self.max_payload_bytes
            ));
        }
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_agent(phase: AgentPhase, tokens: u64) -> (Uuid, u64, AgentRecord) {
        (
            Uuid::new_v4(),
            0,
            AgentRecord {
                id: Uuid::new_v4(),
                name: "test".to_string(),
                phase,
                tokens,
                skills: HashMap::new(),
                personality: String::new(),
                tasks_completed: 0,
                tasks_attempted: 0,
            },
        )
    }

    fn make_agent_with_spawn_tick(
        phase: AgentPhase,
        tokens: u64,
        spawn_tick: u64,
    ) -> (Uuid, u64, AgentRecord) {
        (
            Uuid::new_v4(),
            spawn_tick,
            AgentRecord {
                id: Uuid::new_v4(),
                name: "test".to_string(),
                phase,
                tokens,
                skills: HashMap::new(),
                personality: String::new(),
                tasks_completed: 0,
                tasks_attempted: 0,
            },
        )
    }

    // --- Subsystem tests ---

    #[test]
    fn intervention_checker_flags_dead_agents() {
        let sub = InterventionCheckerSubsystem::new(InterventionConfig::default());
        let mut agents = vec![make_agent(AgentPhase::Dead, 100)];

        let events = sub.on_tick(1, &mut agents);

        assert_eq!(events.len(), 1);
        assert!(matches!(
            &events[0],
            WorldEvent::RuleViolated { rule, .. } if rule == "IC-05"
        ));
    }

    #[test]
    fn intervention_checker_flags_dying_agents() {
        let sub = InterventionCheckerSubsystem::new(InterventionConfig::default());
        let mut agents = vec![make_agent(AgentPhase::Dying, 0)];

        let events = sub.on_tick(1, &mut agents);

        assert_eq!(events.len(), 1);
        assert!(matches!(
            &events[0],
            WorldEvent::RuleViolated { rule, .. } if rule == "IC-05"
        ));
    }

    #[test]
    fn intervention_checker_flags_zero_token_agents() {
        let sub = InterventionCheckerSubsystem::new(InterventionConfig::default());
        let mut agents = vec![make_agent(AgentPhase::Adult, 0)];

        let events = sub.on_tick(1, &mut agents);

        // Should get IC-04 violation
        assert!(events.iter().any(|e| matches!(
            e,
            WorldEvent::RuleViolated { rule, .. } if rule == "IC-04"
        )));
    }

    #[test]
    fn intervention_checker_allows_healthy_agents() {
        let sub = InterventionCheckerSubsystem::new(InterventionConfig::default());
        let mut agents = vec![make_agent(AgentPhase::Adult, 500)];

        let events = sub.on_tick(1, &mut agents);

        // Healthy adult with tokens should produce no violation events
        assert!(events.is_empty());
    }

    #[test]
    fn intervention_checker_newbie_protection_period() {
        let config = InterventionConfig {
            newbie_protection_ticks: 10,
            ..Default::default()
        };
        let sub = InterventionCheckerSubsystem::new(config);
        // Agent spawned at tick 5, current tick is 8 => age = 3 < 10
        let mut agents = vec![make_agent_with_spawn_tick(AgentPhase::Adult, 100, 5)];

        let events = sub.on_tick(8, &mut agents);

        // No violation events for the newbie (just debug logging)
        assert!(events.is_empty());
    }

    // --- Broadcast tracking ---

    #[test]
    fn broadcast_tracking_allows_under_limit() {
        let sub = InterventionCheckerSubsystem::new(InterventionConfig {
            broadcast_max_per_tick: 3,
            ..Default::default()
        });

        assert!(sub.record_broadcast("agent-1", 1));
        assert!(sub.record_broadcast("agent-1", 1));
        assert!(sub.record_broadcast("agent-1", 1));
    }

    #[test]
    fn broadcast_tracking_blocks_over_limit() {
        let sub = InterventionCheckerSubsystem::new(InterventionConfig {
            broadcast_max_per_tick: 2,
            ..Default::default()
        });

        assert!(sub.record_broadcast("agent-1", 1));
        assert!(sub.record_broadcast("agent-1", 1));
        assert!(!sub.record_broadcast("agent-1", 1)); // 3rd blocked
    }

    #[test]
    fn broadcast_tracking_resets_per_tick() {
        let sub = InterventionCheckerSubsystem::new(InterventionConfig {
            broadcast_max_per_tick: 2,
            ..Default::default()
        });

        assert!(sub.record_broadcast("agent-1", 1));
        assert!(sub.record_broadcast("agent-1", 1));
        assert!(!sub.record_broadcast("agent-1", 1)); // blocked in tick 1
        assert!(sub.record_broadcast("agent-1", 2)); // allowed in tick 2
    }

    // --- MessageInterventionGuard tests ---

    #[test]
    fn guard_allows_broadcast_under_limit() {
        let guard = MessageInterventionGuard::default();
        assert!(guard.check_broadcast("a1", 0).is_ok());
        assert!(guard.check_broadcast("a1", 4).is_ok());
    }

    #[test]
    fn guard_blocks_broadcast_at_limit() {
        let guard = MessageInterventionGuard::default();
        assert!(guard.check_broadcast("a1", 5).is_err());
    }

    #[test]
    fn guard_allows_normal_payload() {
        let guard = MessageInterventionGuard::default();
        assert!(guard.check_message_size(1024).is_ok());
    }

    #[test]
    fn guard_blocks_oversized_payload() {
        let guard = MessageInterventionGuard::default();
        assert!(guard.check_message_size(100_000).is_err());
    }
}
