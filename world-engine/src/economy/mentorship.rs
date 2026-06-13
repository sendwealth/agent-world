//! Mentor-Apprentice System — skill transfer between agents.
//!
//! Adult and Elder agents (mentors) can teach skills to other agents (apprentices).
//! Teaching transfers skill experience from mentor to apprentice.
//!
//! Rules:
//! - Mentor must be Adult or Elder phase with `can_teach` ability.
//! - Apprentice must have `can_learn` ability.
//! - Mentor must have the skill at level >= 2.
//! - Teaching happens over multiple ticks (progress tracked).
//! - On completion, apprentice gains the skill at a fraction of mentor's level.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::economy::token_burn::AgentRecord;
use crate::world::enums::AgentPhase;
use crate::world::event::WorldEvent;
use crate::world::state::EventBus;

/// Status of a mentorship session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MentorshipStatus {
    /// Currently in progress.
    Active,
    /// Successfully completed.
    Completed,
    /// Cancelled (mentor or apprentice died, or mentor withdrew).
    Cancelled,
}

/// A single mentorship session where a mentor teaches a skill to an apprentice.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MentorshipSession {
    pub id: Uuid,
    pub mentor_id: String,
    pub apprentice_id: String,
    pub skill_name: String,
    pub mentor_skill_level: u32,
    pub target_level: u32,
    pub current_progress: u64,
    /// How many ticks of teaching are required.
    pub ticks_required: u64,
    pub status: MentorshipStatus,
    pub start_tick: u64,
    pub completion_tick: Option<u64>,
}

/// Configuration for the mentorship system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MentorshipConfig {
    /// How many ticks per skill level for teaching.
    #[serde(default = "default_ticks_per_level")]
    pub ticks_per_level: u64,
    /// Fraction of mentor's skill level transferred to apprentice.
    #[serde(default = "default_transfer_ratio")]
    pub transfer_ratio: f64,
    /// Maximum concurrent apprentices per mentor.
    #[serde(default = "default_max_apprentices")]
    pub max_apprentices_per_mentor: u32,
}

fn default_ticks_per_level() -> u64 {
    20
}
fn default_transfer_ratio() -> f64 {
    0.7
}
fn default_max_apprentices() -> u32 {
    3
}

impl Default for MentorshipConfig {
    fn default() -> Self {
        Self {
            ticks_per_level: default_ticks_per_level(),
            transfer_ratio: default_transfer_ratio(),
            max_apprentices_per_mentor: default_max_apprentices(),
        }
    }
}

/// Errors that can occur during mentorship operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MentorshipError {
    SessionNotFound(Uuid),
}

impl std::fmt::Display for MentorshipError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MentorshipError::SessionNotFound(id) => {
                write!(f, "mentorship session not found: {}", id)
            }
        }
    }
}

impl std::error::Error for MentorshipError {}

/// The mentorship system managing teaching relationships.
pub struct MentorshipSystem {
    sessions: HashMap<Uuid, MentorshipSession>,
    /// Active mentorship lookup: (mentor_id, apprentice_id, skill) -> session_id.
    active_index: HashMap<(String, String, String), Uuid>,
    config: MentorshipConfig,
    event_bus: Option<EventBus>,
}

impl MentorshipSystem {
    pub fn new(config: MentorshipConfig) -> Self {
        Self {
            sessions: HashMap::new(),
            active_index: HashMap::new(),
            config,
            event_bus: None,
        }
    }

    pub fn with_event_bus(config: MentorshipConfig, event_bus: EventBus) -> Self {
        Self {
            sessions: HashMap::new(),
            active_index: HashMap::new(),
            config,
            event_bus: Some(event_bus),
        }
    }

    /// Establish a mentorship relationship.
    pub fn establish(
        &mut self,
        mentor_id: &str,
        apprentice_id: &str,
        skill_name: &str,
        mentor_skill_level: u32,
        tick: u64,
    ) -> Result<Uuid, String> {
        if mentor_id == apprentice_id {
            return Err("cannot mentor yourself".into());
        }
        if mentor_skill_level < 2 {
            return Err("mentor skill level must be >= 2".into());
        }

        // Check if already mentoring this skill
        let key = (
            mentor_id.to_string(),
            apprentice_id.to_string(),
            skill_name.to_string(),
        );
        if self.active_index.contains_key(&key) {
            return Err("mentorship already active for this skill".into());
        }

        // Check concurrent apprentice limit
        let active_count = self
            .active_index
            .keys()
            .filter(|(m, _, _)| m == mentor_id)
            .count();
        if active_count >= self.config.max_apprentices_per_mentor as usize {
            return Err("mentor has too many active apprentices".into());
        }

        let target_level =
            ((mentor_skill_level as f64) * self.config.transfer_ratio).floor() as u32;
        let target_level = target_level.max(1);
        let ticks_required = (target_level as u64) * self.config.ticks_per_level;

        let id = Uuid::new_v4();
        let session = MentorshipSession {
            id,
            mentor_id: mentor_id.to_string(),
            apprentice_id: apprentice_id.to_string(),
            skill_name: skill_name.to_string(),
            mentor_skill_level,
            target_level,
            current_progress: 0,
            ticks_required,
            status: MentorshipStatus::Active,
            start_tick: tick,
            completion_tick: None,
        };

        self.sessions.insert(id, session);
        self.active_index.insert(key, id);

        self.emit(WorldEvent::MentorshipEstablished {
            mentor_id: mentor_id.to_string(),
            apprentice_id: apprentice_id.to_string(),
            skill: skill_name.to_string(),
        });

        Ok(id)
    }

    /// Progress active mentorships by one tick.
    /// Returns a list of completed session IDs.
    pub fn progress_tick(
        &mut self,
        tick: u64,
        agents: &mut [(uuid::Uuid, u64, AgentRecord)],
    ) -> Result<Vec<Uuid>, MentorshipError> {
        let mut completed = Vec::new();

        // Build a lookup for agent records
        let agent_map: HashMap<String, usize> = agents
            .iter()
            .enumerate()
            .map(|(i, (_, _, a))| (a.id.to_string(), i))
            .collect();

        let active_sessions: Vec<Uuid> = self
            .sessions
            .values()
            .filter(|s| s.status == MentorshipStatus::Active)
            .map(|s| s.id)
            .collect();

        for session_id in active_sessions {
            let should_cancel = {
                let session = match self.sessions.get(&session_id) {
                    Some(s) => s,
                    None => continue,
                };

                // Check if mentor or apprentice is dead
                let mentor_alive = agent_map
                    .get(&session.mentor_id)
                    .map(|&i| agents[i].2.phase != AgentPhase::Dead)
                    .unwrap_or(false);
                let apprentice_alive = agent_map
                    .get(&session.apprentice_id)
                    .map(|&i| agents[i].2.phase != AgentPhase::Dead)
                    .unwrap_or(false);

                // Check if mentor can still teach
                let mentor_can_teach = agent_map
                    .get(&session.mentor_id)
                    .map(|&i| {
                        let phase = agents[i].2.phase;
                        phase == AgentPhase::Adult || phase == AgentPhase::Elder
                    })
                    .unwrap_or(false);

                // Check if apprentice can still learn
                let apprentice_can_learn = agent_map
                    .get(&session.apprentice_id)
                    .map(|&i| {
                        let phase = agents[i].2.phase;
                        phase != AgentPhase::Dead && phase != AgentPhase::Dying
                    })
                    .unwrap_or(false);

                !mentor_alive || !apprentice_alive || !mentor_can_teach || !apprentice_can_learn
            };

            if should_cancel {
                if let Some(session) = self.sessions.get_mut(&session_id) {
                    session.status = MentorshipStatus::Cancelled;
                    let key = (
                        session.mentor_id.clone(),
                        session.apprentice_id.clone(),
                        session.skill_name.clone(),
                    );
                    self.active_index.remove(&key);
                }
                continue;
            }

            // Progress the session
            let is_complete = {
                let session = self.sessions.get_mut(&session_id).ok_or(MentorshipError::SessionNotFound(session_id))?;
                session.current_progress += 1;
                session.current_progress >= session.ticks_required
            };

            if is_complete {
                // Transfer skill to apprentice
                let (mentor_id, apprentice_id, skill_name, target_level) = {
                    let session = self.sessions.get(&session_id).ok_or(MentorshipError::SessionNotFound(session_id))?;
                    (
                        session.mentor_id.clone(),
                        session.apprentice_id.clone(),
                        session.skill_name.clone(),
                        session.target_level,
                    )
                };

                if let Some(&app_idx) = agent_map.get(&apprentice_id) {
                    let apprentice = &mut agents[app_idx].2;
                    use crate::economy::token_burn::SkillRecord;
                    apprentice.skills.insert(
                        skill_name.clone(),
                        SkillRecord {
                            name: skill_name.clone(),
                            level: target_level,
                            experience: 0.0,
                        },
                    );
                }

                // Mark completed
                {
                    let session = self.sessions.get_mut(&session_id).ok_or(MentorshipError::SessionNotFound(session_id))?;
                    session.status = MentorshipStatus::Completed;
                    session.completion_tick = Some(tick);
                    let key = (
                        session.mentor_id.clone(),
                        session.apprentice_id.clone(),
                        session.skill_name.clone(),
                    );
                    self.active_index.remove(&key);
                }

                self.emit(WorldEvent::MentorshipCompleted {
                    mentor_id,
                    apprentice_id,
                    skill: skill_name,
                    final_level: target_level,
                });

                completed.push(session_id);
            }
        }

        Ok(completed)
    }

    /// Get all sessions in the system.
    pub fn all_sessions(&self) -> Vec<&MentorshipSession> {
        self.sessions.values().collect()
    }

    /// Get active sessions for a mentor.
    pub fn mentor_active_sessions(&self, mentor_id: &str) -> Vec<&MentorshipSession> {
        self.sessions
            .values()
            .filter(|s| s.mentor_id == mentor_id && s.status == MentorshipStatus::Active)
            .collect()
    }

    /// Get sessions for an apprentice.
    pub fn apprentice_sessions(&self, apprentice_id: &str) -> Vec<&MentorshipSession> {
        self.sessions
            .values()
            .filter(|s| s.apprentice_id == apprentice_id)
            .collect()
    }

    /// Total number of sessions.
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    /// Count completed sessions.
    pub fn completed_count(&self) -> usize {
        self.sessions
            .values()
            .filter(|s| s.status == MentorshipStatus::Completed)
            .count()
    }

    /// Count active sessions.
    pub fn active_count(&self) -> usize {
        self.active_index.len()
    }

    fn emit(&self, event: WorldEvent) {
        if let Some(ref bus) = self.event_bus {
            bus.emit(event);
        }
    }
}

impl Default for MentorshipSystem {
    fn default() -> Self {
        Self::new(MentorshipConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::economy::token_burn::SkillRecord;

    #[allow(dead_code)]
    fn make_agent_with_skill(id: &str, skill: &str, level: u32) -> (Uuid, u64, AgentRecord) {
        let uuid = Uuid::parse_str(id).unwrap_or(Uuid::new_v4());
        (
            uuid,
            0,
            AgentRecord {
                id: uuid,
                name: id.to_string(),
                phase: AgentPhase::Adult,
                tokens: 1000,
                skills: {
                    let mut m = HashMap::new();
                    if !skill.is_empty() {
                        m.insert(
                            skill.to_string(),
                            SkillRecord {
                                name: skill.to_string(),
                                level,
                                experience: 0.0,
                            },
                        );
                    }
                    m
                },
                personality: String::new(),
                tasks_completed: 0,
                tasks_attempted: 0,
            },
        )
    }

    #[test]
    fn test_establish_mentorship() {
        let mut sys = MentorshipSystem::default();
        let result = sys.establish("mentor", "apprentice", "mining", 5, 0);
        assert!(result.is_ok());
        assert_eq!(sys.active_count(), 1);
    }

    #[test]
    fn test_cannot_mentor_yourself() {
        let mut sys = MentorshipSystem::default();
        let result = sys.establish("agent", "agent", "mining", 5, 0);
        assert!(result.is_err());
    }

    #[test]
    fn test_skill_level_too_low() {
        let mut sys = MentorshipSystem::default();
        let result = sys.establish("mentor", "apprentice", "mining", 1, 0);
        assert!(result.is_err());
    }

    #[test]
    fn test_duplicate_mentorship_rejected() {
        let mut sys = MentorshipSystem::default();
        sys.establish("mentor", "apprentice", "mining", 5, 0)
            .unwrap();
        let result = sys.establish("mentor", "apprentice", "mining", 5, 1);
        assert!(result.is_err());
    }

    #[test]
    fn test_progress_and_completion() {
        let mut sys = MentorshipSystem::with_event_bus(
            MentorshipConfig {
                ticks_per_level: 3,
                transfer_ratio: 0.7,
                max_apprentices_per_mentor: 3,
            },
            EventBus::new(64),
        );

        let mentor_uuid = Uuid::new_v4();
        let apprentice_uuid = Uuid::new_v4();

        sys.establish(
            &mentor_uuid.to_string(),
            &apprentice_uuid.to_string(),
            "mining",
            5,
            0,
        )
        .unwrap();

        // Target level: 5 * 0.7 = 3.5 -> 3, ticks: 3 * 3 = 9
        let mut agents = vec![
            (
                mentor_uuid,
                0,
                AgentRecord {
                    id: mentor_uuid,
                    name: "mentor".into(),
                    phase: AgentPhase::Adult,
                    tokens: 1000,
                    skills: {
                        let mut m = HashMap::new();
                        m.insert(
                            "mining".into(),
                            SkillRecord {
                                name: "mining".into(),
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
                apprentice_uuid,
                0,
                AgentRecord {
                    id: apprentice_uuid,
                    name: "apprentice".into(),
                    phase: AgentPhase::Adult,
                    tokens: 1000,
                    skills: HashMap::new(),
                    personality: String::new(),
                    tasks_completed: 0,
                    tasks_attempted: 0,
                },
            ),
        ];

        // Progress 8 ticks - not yet complete
        for tick in 1..=8 {
            let completed = sys.progress_tick(tick, &mut agents).unwrap();
            assert!(completed.is_empty());
        }

        // 9th tick should complete
        let completed = sys.progress_tick(9, &mut agents).unwrap();
        assert_eq!(completed.len(), 1);
        assert_eq!(sys.active_count(), 0);
        assert_eq!(sys.completed_count(), 1);

        // Check apprentice got the skill
        let apprentice = &agents[1].2;
        assert!(apprentice.skills.contains_key("mining"));
        assert_eq!(apprentice.skills.get("mining").unwrap().level, 3);
    }

    #[test]
    fn test_cancel_on_death() {
        let mut sys = MentorshipSystem::default();
        let mentor_uuid = Uuid::new_v4();
        let apprentice_uuid = Uuid::new_v4();

        sys.establish(
            &mentor_uuid.to_string(),
            &apprentice_uuid.to_string(),
            "mining",
            5,
            0,
        )
        .unwrap();

        let mut agents = vec![
            (
                mentor_uuid,
                0,
                AgentRecord {
                    id: mentor_uuid,
                    name: "mentor".into(),
                    phase: AgentPhase::Dead, // Dead mentor
                    tokens: 0,
                    skills: HashMap::new(),
                    personality: String::new(),
                    tasks_completed: 0,
                    tasks_attempted: 0,
                },
            ),
            (
                apprentice_uuid,
                0,
                AgentRecord {
                    id: apprentice_uuid,
                    name: "apprentice".into(),
                    phase: AgentPhase::Adult,
                    tokens: 1000,
                    skills: HashMap::new(),
                    personality: String::new(),
                    tasks_completed: 0,
                    tasks_attempted: 0,
                },
            ),
        ];

        sys.progress_tick(1, &mut agents).unwrap();
        assert_eq!(sys.active_count(), 0);
    }

    #[test]
    fn test_max_apprentices_limit() {
        let mut sys = MentorshipSystem::new(MentorshipConfig {
            max_apprentices_per_mentor: 2,
            ..Default::default()
        });

        sys.establish("m", "a1", "mining", 5, 0).unwrap();
        sys.establish("m", "a2", "crafting", 4, 0).unwrap();
        let result = sys.establish("m", "a3", "trading", 3, 0);
        assert!(result.is_err());
    }
}
