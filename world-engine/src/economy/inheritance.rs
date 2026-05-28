//! Inheritance System — wills and asset transfer upon agent death.
//!
//! When an agent dies, their remaining assets (tokens and skills) are
//! distributed to designated beneficiaries according to their will.
//!
//! - Agents with `can_write_will` ability (Adult, Elder, Dying) can create wills.
//! - A will specifies beneficiaries and their share percentages.
//! - On death, `inheritance_ratio` of remaining tokens are distributed.
//! - Skills are transferred at `skill_transfer_ratio` of original level.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::economy::token_burn::AgentRecord;
use crate::world::enums::AgentPhase;
use crate::world::event::WorldEvent;
use crate::world::state::EventBus;

/// A single beneficiary in a will.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Beneficiary {
    pub agent_id: String,
    /// Share percentage (0.0 to 1.0). All shares must sum to <= 1.0.
    pub share: f64,
}

/// A key value-related experience to be passed on via inheritance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValueExperience {
    pub event_type: String,
    pub outcome: f64,
    pub learned: String,
}

/// A will created by an agent specifying beneficiaries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Will {
    pub id: Uuid,
    pub testator_id: String,
    pub beneficiaries: Vec<Beneficiary>,
    pub created_tick: u64,
    pub updated_tick: u64,
    pub executed: bool,
    /// Snapshot of the testator's value weights at will-creation time.
    #[serde(default)]
    pub values_snapshot: HashMap<String, f64>,
    /// Most important experiences to pass on (capped at 5).
    #[serde(default)]
    pub key_experiences: Vec<ValueExperience>,
    /// LLM-generated life lesson summary.
    #[serde(default)]
    pub life_lessons: String,
}

/// Result of executing an inheritance.
#[derive(Debug, Clone)]
pub struct InheritanceResult {
    pub deceased_id: String,
    pub tokens_distributed: u64,
    pub tokens_destroyed: u64,
    pub beneficiaries_paid: Vec<(String, u64, u32)>, // (agent_id, tokens, skills_count)
    pub events: Vec<WorldEvent>,
}

/// Configuration for the inheritance system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InheritanceConfig {
    /// Fraction of tokens transferred to beneficiaries (rest destroyed).
    #[serde(default = "default_inheritance_ratio")]
    pub inheritance_ratio: f64,
    /// Fraction of skill level transferred to beneficiaries.
    #[serde(default = "default_skill_transfer_ratio")]
    pub skill_transfer_ratio: f64,
}

fn default_inheritance_ratio() -> f64 {
    0.5
}
fn default_skill_transfer_ratio() -> f64 {
    0.3
}

impl Default for InheritanceConfig {
    fn default() -> Self {
        Self {
            inheritance_ratio: default_inheritance_ratio(),
            skill_transfer_ratio: default_skill_transfer_ratio(),
        }
    }
}

/// The inheritance system managing wills and asset distribution.
pub struct InheritanceSystem {
    wills: HashMap<String, Will>,
    config: InheritanceConfig,
    event_bus: Option<EventBus>,
}

impl InheritanceSystem {
    pub fn new(config: InheritanceConfig) -> Self {
        Self {
            wills: HashMap::new(),
            config,
            event_bus: None,
        }
    }

    pub fn with_event_bus(config: InheritanceConfig, event_bus: EventBus) -> Self {
        Self {
            wills: HashMap::new(),
            config,
            event_bus: Some(event_bus),
        }
    }

    /// Create or update a will for an agent.
    pub fn create_will(
        &mut self,
        testator_id: &str,
        beneficiaries: Vec<Beneficiary>,
        tick: u64,
    ) -> Result<Uuid, String> {
        // Validate shares
        let total_share: f64 = beneficiaries.iter().map(|b| b.share).sum();
        if total_share > 1.0 {
            return Err("total beneficiary shares exceed 1.0".into());
        }
        if beneficiaries.is_empty() {
            return Err("will must have at least one beneficiary".into());
        }
        // Check for self-beneficiary
        if beneficiaries.iter().any(|b| b.agent_id == testator_id) {
            return Err("cannot name yourself as beneficiary".into());
        }

        if self.wills.contains_key(testator_id) {
            let will = self.wills.get_mut(testator_id).unwrap();
            will.beneficiaries = beneficiaries;
            will.updated_tick = tick;
            let count = will.beneficiaries.len();
            let id = will.id;
            self.emit(WorldEvent::WillCreated {
                agent_id: testator_id.to_string(),
                beneficiaries_count: count,
            });
            Ok(id)
        } else {
            let id = Uuid::new_v4();
            let will = Will {
                id,
                testator_id: testator_id.to_string(),
                beneficiaries,
                created_tick: tick,
                updated_tick: tick,
                executed: false,
                values_snapshot: HashMap::new(),
                key_experiences: Vec::new(),
                life_lessons: String::new(),
            };
            let count = will.beneficiaries.len();
            self.emit(WorldEvent::WillCreated {
                agent_id: testator_id.to_string(),
                beneficiaries_count: count,
            });
            self.wills.insert(testator_id.to_string(), will);
            Ok(id)
        }
    }

    /// Execute inheritance for a deceased agent.
    /// Distributes tokens and skills to beneficiaries, updates agent records.
    pub fn execute_inheritance(
        &mut self,
        deceased_id: &str,
        agents: &mut [(Uuid, u64, AgentRecord)],
        _tick: u64,
    ) -> InheritanceResult {
        let mut events = Vec::new();
        let mut beneficiaries_paid = Vec::new();

        // Find deceased agent's tokens
        let (deceased_tokens, deceased_skills) = {
            let deceased = agents
                .iter()
                .find(|(_, _, a)| a.id.to_string() == deceased_id);
            match deceased {
                Some((_, _, a)) => (a.tokens, a.skills.clone()),
                None => {
                    return InheritanceResult {
                        deceased_id: deceased_id.to_string(),
                        tokens_distributed: 0,
                        tokens_destroyed: 0,
                        beneficiaries_paid: vec![],
                        events,
                    };
                }
            }
        };

        let inheritable_tokens = ((deceased_tokens as f64) * self.config.inheritance_ratio) as u64;
        let tokens_destroyed = deceased_tokens - inheritable_tokens;

        // Get will or use default equal distribution among all living agents
        let beneficiaries = if let Some(will) = self.wills.get_mut(deceased_id) {
            will.executed = true;
            will.beneficiaries.clone()
        } else {
            // No will: equal split among living agents (excluding self)
            let living: Vec<String> = agents
                .iter()
                .filter(|(_, _, a)| a.id.to_string() != deceased_id && a.phase != AgentPhase::Dead)
                .map(|(_, _, a)| a.id.to_string())
                .collect();

            if living.is_empty() {
                return InheritanceResult {
                    deceased_id: deceased_id.to_string(),
                    tokens_distributed: 0,
                    tokens_destroyed: deceased_tokens,
                    beneficiaries_paid: vec![],
                    events,
                };
            }

            let share = 1.0 / living.len() as f64;
            living
                .into_iter()
                .map(|id| Beneficiary {
                    agent_id: id,
                    share,
                })
                .collect()
        };

        // Distribute tokens and skills
        let mut total_distributed: u64 = 0;
        for beneficiary in &beneficiaries {
            // Check beneficiary is alive
            let beneficiary_alive = agents.iter().any(|(_, _, a)| {
                a.id.to_string() == beneficiary.agent_id && a.phase != AgentPhase::Dead
            });
            if !beneficiary_alive {
                continue;
            }

            let token_share = ((inheritable_tokens as f64) * beneficiary.share) as u64;
            if token_share == 0 {
                continue;
            }

            // Transfer tokens
            let mut skills_transferred: u32 = 0;
            for (_, _, agent) in agents.iter_mut() {
                if agent.id.to_string() == beneficiary.agent_id {
                    agent.tokens += token_share;
                    total_distributed += token_share;

                    // Transfer skills
                    use crate::economy::token_burn::SkillRecord;
                    for (skill_name, skill) in &deceased_skills {
                        let transferred_level = ((skill.level as f64)
                            * self.config.skill_transfer_ratio)
                            .floor() as u32;
                        if transferred_level == 0 {
                            continue;
                        }
                        let existing = agent.skills.get(skill_name).map(|s| s.level).unwrap_or(0);
                        // Take the higher of existing or transferred level
                        if transferred_level > existing {
                            agent.skills.insert(
                                skill_name.clone(),
                                SkillRecord {
                                    name: skill_name.clone(),
                                    level: transferred_level,
                                    experience: 0.0,
                                },
                            );
                        }
                        skills_transferred += 1;
                    }
                    break;
                }
            }

            events.push(WorldEvent::InheritanceTriggered {
                deceased_id: deceased_id.to_string(),
                beneficiary_id: beneficiary.agent_id.clone(),
                tokens_transferred: token_share,
                skills_transferred,
            });

            self.emit(WorldEvent::InheritanceTriggered {
                deceased_id: deceased_id.to_string(),
                beneficiary_id: beneficiary.agent_id.clone(),
                tokens_transferred: token_share,
                skills_transferred,
            });

            beneficiaries_paid.push((
                beneficiary.agent_id.clone(),
                token_share,
                skills_transferred,
            ));
        }

        // Remove deceased tokens (already handled in death cleanup, but ensure here)
        for (_, _, agent) in agents.iter_mut() {
            if agent.id.to_string() == deceased_id {
                agent.tokens = 0;
                break;
            }
        }

        InheritanceResult {
            deceased_id: deceased_id.to_string(),
            tokens_distributed: total_distributed,
            tokens_destroyed,
            beneficiaries_paid,
            events,
        }
    }

    /// Check if an agent has a will.
    pub fn has_will(&self, agent_id: &str) -> bool {
        self.wills.contains_key(agent_id)
    }

    /// Get an agent's will.
    pub fn get_will(&self, agent_id: &str) -> Option<&Will> {
        self.wills.get(agent_id)
    }

    /// Count total wills.
    pub fn will_count(&self) -> usize {
        self.wills.len()
    }

    fn emit(&self, event: WorldEvent) {
        if let Some(ref bus) = self.event_bus {
            bus.emit(event);
        }
    }
}

impl Default for InheritanceSystem {
    fn default() -> Self {
        Self::new(InheritanceConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::economy::token_burn::SkillRecord;

    fn make_agents() -> Vec<(Uuid, u64, AgentRecord)> {
        let mentor = Uuid::new_v4();
        let heir = Uuid::new_v4();
        let bystander = Uuid::new_v4();

        vec![
            (
                mentor,
                0,
                AgentRecord {
                    id: mentor,
                    name: "mentor".into(),
                    phase: AgentPhase::Adult,
                    tokens: 1000,
                    skills: {
                        let mut m = HashMap::new();
                        m.insert(
                            "mining".into(),
                            SkillRecord {
                                name: "mining".into(),
                                level: 8,
                                experience: 0.0,
                            },
                        );
                        m.insert(
                            "crafting".into(),
                            SkillRecord {
                                name: "crafting".into(),
                                level: 5,
                                experience: 0.0,
                            },
                        );
                        m
                    },
                    personality: String::new(),
                    tasks_completed: 0,
                    tasks_attempted: 0,
                },
            ),
            (
                heir,
                0,
                AgentRecord {
                    id: heir,
                    name: "heir".into(),
                    phase: AgentPhase::Adult,
                    tokens: 100,
                    skills: HashMap::new(),
                    personality: String::new(),
                    tasks_completed: 0,
                    tasks_attempted: 0,
                },
            ),
            (
                bystander,
                0,
                AgentRecord {
                    id: bystander,
                    name: "bystander".into(),
                    phase: AgentPhase::Adult,
                    tokens: 50,
                    skills: HashMap::new(),
                    personality: String::new(),
                    tasks_completed: 0,
                    tasks_attempted: 0,
                },
            ),
        ]
    }

    #[test]
    fn test_create_will() {
        let mut sys = InheritanceSystem::default();
        let mentor_id = make_agents()[0].2.id.to_string();
        let heir_id = make_agents()[1].2.id.to_string();

        let result = sys.create_will(
            &mentor_id,
            vec![Beneficiary {
                agent_id: heir_id,
                share: 1.0,
            }],
            100,
        );
        assert!(result.is_ok());
        assert!(sys.has_will(&mentor_id));
    }

    #[test]
    fn test_will_validates_shares() {
        let mut sys = InheritanceSystem::default();
        let result = sys.create_will(
            "a",
            vec![
                Beneficiary {
                    agent_id: "b".into(),
                    share: 0.8,
                },
                Beneficiary {
                    agent_id: "c".into(),
                    share: 0.3,
                },
            ],
            100,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_will_rejects_self_beneficiary() {
        let mut sys = InheritanceSystem::default();
        let result = sys.create_will(
            "a",
            vec![Beneficiary {
                agent_id: "a".into(),
                share: 1.0,
            }],
            100,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_will_rejects_empty_beneficiaries() {
        let mut sys = InheritanceSystem::default();
        let result = sys.create_will("a", vec![], 100);
        assert!(result.is_err());
    }

    #[test]
    fn test_execute_inheritance_with_will() {
        let mut sys = InheritanceSystem::new(InheritanceConfig {
            inheritance_ratio: 0.5,
            skill_transfer_ratio: 0.3,
        });
        let mut agents = make_agents();
        let mentor_id = agents[0].2.id;
        let heir_id = agents[1].2.id;

        sys.create_will(
            &mentor_id.to_string(),
            vec![Beneficiary {
                agent_id: heir_id.to_string(),
                share: 1.0,
            }],
            100,
        )
        .unwrap();

        // Mark mentor as dead
        agents[0].2.phase = AgentPhase::Dead;
        agents[0].2.tokens = 1000;

        let result = sys.execute_inheritance(&mentor_id.to_string(), &mut agents, 200);

        // 50% of 1000 = 500 distributed
        assert_eq!(result.tokens_distributed, 500);
        assert_eq!(result.tokens_destroyed, 500);
        assert_eq!(result.beneficiaries_paid.len(), 1);

        // Heir should have received tokens
        assert_eq!(agents[1].2.tokens, 100 + 500); // 100 initial + 500 inherited

        // Heir should have received skills (8 * 0.3 = 2.4 -> 2)
        assert!(agents[1].2.skills.contains_key("mining"));
        assert_eq!(agents[1].2.skills.get("mining").unwrap().level, 2);
    }

    #[test]
    fn test_execute_inheritance_without_will() {
        let mut sys = InheritanceSystem::new(InheritanceConfig {
            inheritance_ratio: 0.5,
            skill_transfer_ratio: 0.3,
        });
        let mut agents = make_agents();
        let mentor_id = agents[0].2.id;

        agents[0].2.phase = AgentPhase::Dead;

        let result = sys.execute_inheritance(&mentor_id.to_string(), &mut agents, 200);

        // Without a will, equal split among 2 living agents
        // 50% of 1000 = 500, split: 250 each
        assert_eq!(result.tokens_distributed, 500);
        assert_eq!(result.beneficiaries_paid.len(), 2);
    }

    #[test]
    fn test_update_existing_will() {
        let mut sys = InheritanceSystem::default();
        sys.create_will(
            "a",
            vec![Beneficiary {
                agent_id: "b".into(),
                share: 1.0,
            }],
            100,
        )
        .unwrap();

        let _id2 = sys
            .create_will(
                "a",
                vec![Beneficiary {
                    agent_id: "c".into(),
                    share: 1.0,
                }],
                200,
            )
            .unwrap();

        let will = sys.get_will("a").unwrap();
        assert_eq!(will.beneficiaries[0].agent_id, "c");
        assert_eq!(will.updated_tick, 200);
    }

    #[test]
    fn test_no_living_beneficiaries() {
        let mut sys = InheritanceSystem::default();
        let mut agents = make_agents();
        let mentor_id = agents[0].2.id;

        // Kill all other agents
        agents[1].2.phase = AgentPhase::Dead;
        agents[2].2.phase = AgentPhase::Dead;
        agents[0].2.phase = AgentPhase::Dead;

        let result = sys.execute_inheritance(&mentor_id.to_string(), &mut agents, 200);
        assert_eq!(result.tokens_distributed, 0);
        assert_eq!(result.tokens_destroyed, 1000);
    }

    #[test]
    fn test_event_bus_integration() {
        let bus = EventBus::new(64);
        let mut rx = bus.subscribe();
        let mut sys = InheritanceSystem::with_event_bus(InheritanceConfig::default(), bus);

        sys.create_will(
            "a",
            vec![Beneficiary {
                agent_id: "b".into(),
                share: 1.0,
            }],
            100,
        )
        .unwrap();

        let event = rx.try_recv().unwrap();
        assert!(matches!(event, WorldEvent::WillCreated { .. }));
    }
}
