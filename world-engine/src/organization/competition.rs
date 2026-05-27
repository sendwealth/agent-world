use serde::{Deserialize, Serialize};

use super::org::{OrgType, Organization};
use super::members::OrgMember;
use crate::world::state::WorldState;

// ── Constants ─────────────────────────────────────────────

/// Resource bonus multiplier for the winner of a conflict (30%).
pub const WINNER_RESOURCE_BONUS: f64 = 0.30;
/// Resource penalty multiplier for the loser of a conflict (20%).
pub const LOSER_RESOURCE_PENALTY: f64 = 0.20;
/// How many ticks between formation scans.
pub const FORMATION_SCAN_INTERVAL: u64 = 50;
/// Minimum number of agents to suggest forming an org.
pub const MIN_FORMATION_AGENTS: usize = 2;

// ── Result Types ──────────────────────────────────────────

/// Result of evaluating a resource conflict between two organizations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResourceConflictResult {
    /// Competition intensity (0.0–1.0).
    pub intensity: f64,
    /// Winning organization ID.
    pub winner_org_id: String,
    /// Losing organization ID.
    pub loser_org_id: String,
    /// Resource yield bonus for the winner (0.0–1.0).
    pub resource_bonus: f64,
    /// Resource yield penalty for the loser (0.0–1.0).
    pub resource_penalty: f64,
}

/// An invitation extended to an agent to join an organization.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrgInvitation {
    /// Organization extending the invitation.
    pub org_id: String,
    /// A human-readable label for why this org is a good fit.
    pub pitch: String,
}

/// Result of evaluating which organization an agent should join.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecruitmentResult {
    /// The chosen organization ID.
    pub chosen_org_id: String,
    /// Human-readable reason for the choice.
    pub reason: String,
    /// Score breakdown (org_id, attractiveness_score).
    pub scores: Vec<(String, f64)>,
}

/// A single territorial region controlled (or claimed) by an organization.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TerritoryRegion {
    /// Region identifier (e.g. a grid cell or named area).
    pub region_id: String,
    /// Influence level in this region (0.0–1.0).
    pub influence: f64,
}

/// Result of evaluating an organization's territorial influence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TerritoryResult {
    /// Organization ID.
    pub org_id: String,
    /// Regions where the organization has influence.
    pub regions: Vec<TerritoryRegion>,
    /// Total influence across all regions.
    pub total_influence: f64,
}

/// Suggested organization formation detected by the scan engine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FormationSuggestion {
    /// Agents that could form a new organization.
    pub agents: Vec<String>,
    /// Suggested organization type.
    pub suggested_type: OrgType,
    /// Reason for the suggestion.
    pub reason: String,
}

// ── Scoring Weights ───────────────────────────────────────

/// Weights for recruitment attractiveness scoring.
mod weights {
    /// Weight for organization member count.
    pub const MEMBER_COUNT: f64 = 0.4;
    /// Weight for organization treasury (funds).
    pub const TREASURY: f64 = 0.3;
    /// Weight for culture/type affinity.
    pub const CULTURE_MATCH: f64 = 0.3;
}

// ── Competition Engine ────────────────────────────────────

/// Engine for evaluating competition, recruitment, and territory dynamics
/// between organizations in the world.
pub struct CompetitionEngine {
    /// Tick of the last formation scan.
    last_scan_tick: u64,
}

impl Default for CompetitionEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl CompetitionEngine {
    /// Create a new competition engine.
    pub fn new() -> Self {
        Self {
            last_scan_tick: 0,
        }
    }

    // ── Resource Conflict ─────────────────────────────────

    /// Evaluate competition between two organizations over the same resource point.
    ///
    /// Organization power = member_count * average_skill * treasury_factor.
    /// The winner gets a resource bonus; the loser gets a penalty.
    ///
    /// Average skill is computed from each member agent's `AgentRecord.skills`,
    /// aggregated across all members. Falls back to 1.0 when no skill data
    /// is available (e.g. agents not found in the world state).
    pub fn evaluate_resource_conflict(
        &mut self,
        org_a: &Organization,
        org_b: &Organization,
        _resource_point_id: &str,
        world_state: &WorldState,
    ) -> ResourceConflictResult {
        let power_a = Self::org_power(org_a, world_state);
        let power_b = Self::org_power(org_b, world_state);

        let total = power_a + power_b;
        let intensity = if total > 0.0 {
            let diff = (power_a - power_b).abs();
            1.0 - (diff / total)
        } else {
            0.0
        };

        let (winner, loser) = if power_a >= power_b {
            (org_a, org_b)
        } else {
            (org_b, org_a)
        };

        ResourceConflictResult {
            intensity,
            winner_org_id: winner.id.clone(),
            loser_org_id: loser.id.clone(),
            resource_bonus: WINNER_RESOURCE_BONUS,
            resource_penalty: LOSER_RESOURCE_PENALTY,
        }
    }

    /// Compute a composite power score for an organization.
    ///
    /// Power = member_count * avg_skill_level * treasury_factor
    /// where treasury_factor = ln(1 + treasury / 100).
    ///
    /// `avg_skill_level` is the mean of each member's individual average
    /// skill level (mean of all their skill levels). Agents not found in
    /// the world state contribute a default skill level of 1.0.
    fn org_power(org: &Organization, world_state: &WorldState) -> f64 {
        let member_count = org.members.len() as f64;
        let avg_skill = Self::compute_avg_skill(org, world_state);
        let treasury_factor = (1.0 + org.treasury as f64 / 100.0).ln();
        member_count * avg_skill * treasury_factor
    }

    /// Compute the average skill level across all members of an organization.
    ///
    /// For each member, looks up their `AgentRecord` in `world_state.agents`
    /// and averages all skill levels in their `skills` map. If the agent has
    /// no skills or is not found, they contribute a default level of 1.0.
    /// The final result is the mean across all members.
    fn compute_avg_skill(org: &Organization, world_state: &WorldState) -> f64 {
        if org.members.is_empty() {
            return 1.0;
        }

        let mut total: f64 = 0.0;
        let mut count: usize = 0;

        for member in &org.members {
            let agent_skill_avg = world_state
                .agents
                .iter()
                .find(|(id, _, _)| id.to_string() == member.agent_id)
                .map(|(_, _, record)| {
                    if record.skills.is_empty() {
                        1.0
                    } else {
                        let skill_sum: f64 =
                            record.skills.values().map(|s| s.level as f64).sum();
                        skill_sum / record.skills.len() as f64
                    }
                })
                .unwrap_or(1.0);

            total += agent_skill_avg;
            count += 1;
        }

        if count == 0 {
            1.0
        } else {
            total / count as f64
        }
    }

    // ── Recruitment Conflict ──────────────────────────────

    /// Evaluate which organization an agent should join when receiving
    /// multiple invitations.
    ///
    /// Attractiveness = member_count_weight * norm(member_count)
    ///                 + treasury_weight * norm(treasury)
    ///                 + culture_match_weight * culture_match
    pub fn evaluate_recruitment_conflict(
        &self,
        agent_id: &str,
        invitations: &[OrgInvitation],
        organizations: &[Organization],
    ) -> RecruitmentResult {
        if invitations.is_empty() {
            return RecruitmentResult {
                chosen_org_id: String::new(),
                reason: "no invitations".to_string(),
                scores: vec![],
            };
        }

        if invitations.len() == 1 {
            return RecruitmentResult {
                chosen_org_id: invitations[0].org_id.clone(),
                reason: "only one invitation".to_string(),
                scores: vec![(invitations[0].org_id.clone(), 1.0)],
            };
        }

        // Normalize values across all invited orgs
        let max_members = organizations
            .iter()
            .map(|o| o.members.len() as f64)
            .fold(1.0, f64::max);
        let max_treasury = organizations
            .iter()
            .map(|o| o.treasury as f64)
            .fold(1.0, f64::max);

        let mut scores: Vec<(String, f64)> = Vec::with_capacity(invitations.len());

        for inv in invitations {
            let org = match organizations.iter().find(|o| o.id == inv.org_id) {
                Some(o) => o,
                None => {
                    scores.push((inv.org_id.clone(), 0.0));
                    continue;
                }
            };

            let norm_members = org.members.len() as f64 / max_members;
            let norm_treasury = org.treasury as f64 / max_treasury;

            // Culture match bonus: agents gravitate toward org types that match
            // their skills/interests. Use a deterministic but varied score based
            // on agent_id + org_id hash for now.
            let culture_match = Self::culture_match_score(agent_id, &org.id);

            let score = weights::MEMBER_COUNT * norm_members
                + weights::TREASURY * norm_treasury
                + weights::CULTURE_MATCH * culture_match;

            scores.push((org.id.clone(), score));
        }

        // Pick the highest scoring org
        let (chosen_id, chosen_score) = scores
            .iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .cloned()
            .unwrap_or((invitations[0].org_id.clone(), 0.0));

        let reason = format!(
            "agent {} chose org {} (score {:.3}) based on size, funds, and culture fit",
            agent_id, chosen_id, chosen_score
        );

        RecruitmentResult {
            chosen_org_id: chosen_id,
            reason,
            scores,
        }
    }

    /// Compute a deterministic culture-match score in [0.0, 1.0].
    ///
    /// This is a placeholder heuristic: hash(agent_id, org_id) mapped to [0, 1].
    fn culture_match_score(agent_id: &str, org_id: &str) -> f64 {
        let combined = format!("{}:{}", agent_id, org_id);
        let hash = combined.bytes().fold(0u64, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u64));
        (hash % 1000) as f64 / 1000.0
    }

    // ── Territory Evaluation ──────────────────────────────

    /// Evaluate an organization's territorial influence based on member positions.
    ///
    /// Members at the same position contribute cumulatively to that region's
    /// influence. Influence is clamped to [0.0, 1.0].
    pub fn evaluate_territory(
        &self,
        org_id: &str,
        members: &[OrgMember],
        world_state: &WorldState,
    ) -> TerritoryResult {
        // Group members by region (region = agent spawn position / location).
        // Since OrgMember doesn't carry position data, we derive region from
        // the world_state agent roster using agent_id matching.
        let mut region_influence: std::collections::HashMap<String, f64> =
            std::collections::HashMap::new();

        for member in members {
            // Find the agent's region in the world state
            let region = world_state
                .agents
                .iter()
                .find(|(id, _, _)| id.to_string() == member.agent_id)
                .map(|(_, spawn_tick, _)| format!("region_{}", spawn_tick / 100))
                .unwrap_or_else(|| format!("region_{}", member.agent_id.bytes().fold(0u64, |a, b| a.wrapping_add(b as u64)) % 10));

            *region_influence.entry(region).or_insert(0.0) += 0.3;
        }

        // Clamp influence to [0.0, 1.0]
        let regions: Vec<TerritoryRegion> = region_influence
            .into_iter()
            .map(|(region_id, influence)| TerritoryRegion {
                region_id,
                influence: influence.min(1.0),
            })
            .collect();

        let total_influence = regions.iter().map(|r| r.influence).sum();

        TerritoryResult {
            org_id: org_id.to_string(),
            regions,
            total_influence,
        }
    }

    // ── Formation Scan ────────────────────────────────────

    /// Scan for agents that could form a new organization.
    ///
    /// This is a simplified scan that groups unaffiliated agents and checks
    /// whether enough of them are in proximity (same region). Returns
    /// formation suggestions.
    pub fn scan_for_formation(
        &mut self,
        current_tick: u64,
        organizations: &[Organization],
        world_state: &WorldState,
    ) -> Vec<FormationSuggestion> {
        if current_tick < self.last_scan_tick + FORMATION_SCAN_INTERVAL {
            return vec![];
        }
        self.last_scan_tick = current_tick;

        // Collect agents that are already in an organization
        let affiliated: std::collections::HashSet<String> = organizations
            .iter()
            .flat_map(|org| org.members.iter().map(|m| m.agent_id.clone()))
            .collect();

        // Group unaffiliated agents by region
        let mut region_agents: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();

        for (id, spawn_tick, record) in &world_state.agents {
            if affiliated.contains(&id.to_string()) {
                continue;
            }
            // Skip dead agents
            if record.phase == crate::world::enums::AgentPhase::Dead {
                continue;
            }
            let region = format!("region_{}", spawn_tick / 100);
            region_agents
                .entry(region)
                .or_default()
                .push(id.to_string());
        }

        // Generate suggestions for regions with enough agents
        let mut suggestions = Vec::new();
        for (_region, agents) in region_agents {
            if agents.len() >= MIN_FORMATION_AGENTS {
                let suggested_type = Self::suggest_org_type(&agents, current_tick);
                let reason = format!(
                    "{} unaffiliated agents in proximity could form a {:?}",
                    agents.len(),
                    suggested_type
                );
                suggestions.push(FormationSuggestion {
                    agents,
                    suggested_type,
                    reason,
                });
            }
        }

        suggestions
    }

    /// Suggest an organization type based on agent count and context.
    fn suggest_org_type(agents: &[String], _current_tick: u64) -> OrgType {
        match agents.len() {
            0..=2 => OrgType::Guild,
            3..=5 => OrgType::Company,
            6..=10 => OrgType::Alliance,
            _ => OrgType::University,
        }
    }
}

// ── Tests ─────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::organization::org::{Organization, OrgType, OrgStatus};
    use crate::organization::charter::{Charter, GovernanceModel, ProfitSharing};
    use crate::organization::members::{MemberRole, OrgMember};
    use crate::world::state::{EventBus, WorldState};
    use crate::world::subsystem::SubsystemRegistry;
    use crate::economy::token_burn::AgentRecord;
    use crate::world::enums::AgentPhase;
    use std::collections::HashMap;
    use std::sync::Arc;
    use uuid::Uuid;

    fn test_charter() -> Charter {
        Charter {
            purpose: "Test org".to_string(),
            governance: GovernanceModel::Vote,
            profit_sharing: ProfitSharing::Equal,
            membership_fee: 0,
        }
    }

    fn make_org(name: &str, org_type: OrgType, member_count: usize, treasury: u64, tick: u64) -> Organization {
        let id = Uuid::new_v4().to_string();
        let members: Vec<OrgMember> = (0..member_count)
            .map(|i| OrgMember {
                agent_id: format!("{}-agent-{}", id, i),
                agent_name: format!("Agent {}", i),
                role: if i == 0 { MemberRole::Founder } else { MemberRole::Member },
                share: 1.0 / member_count as f64,
                joined_tick: tick,
            })
            .collect();

        Organization {
            id,
            name: name.to_string(),
            org_type,
            charter: test_charter(),
            treasury,
            debts: 0,
            status: OrgStatus::Active,
            created_tick: tick,
            last_activity_tick: tick,
            members,
        }
    }

    fn make_world_state() -> WorldState {
        let bus = Arc::new(EventBus::new(256));
        let registry = SubsystemRegistry::new();
        WorldState::new(bus, registry, vec![])
    }

    // ── Resource Conflict Tests ───────────────────────────

    #[test]
    fn test_resource_conflict_two_orgs_same_resource() {
        let mut engine = CompetitionEngine::new();
        let org_a = make_org("Alpha Corp", OrgType::Company, 5, 500, 100);
        let org_b = make_org("Beta Guild", OrgType::Guild, 3, 200, 100);
        let world_state = make_world_state();

        let result = engine.evaluate_resource_conflict(&org_a, &org_b, "resource-1", &world_state);

        assert_eq!(result.winner_org_id, org_a.id);
        assert_eq!(result.loser_org_id, org_b.id);
        assert!(result.intensity > 0.0 && result.intensity <= 1.0);
        assert_eq!(result.resource_bonus, WINNER_RESOURCE_BONUS);
        assert_eq!(result.resource_penalty, LOSER_RESOURCE_PENALTY);
    }

    #[test]
    fn test_resource_conflict_clear_winner() {
        let mut engine = CompetitionEngine::new();
        let org_big = make_org("BigCorp", OrgType::Company, 20, 10000, 100);
        let org_small = make_org("SmallGuild", OrgType::Guild, 2, 50, 100);
        let world_state = make_world_state();

        let result = engine.evaluate_resource_conflict(&org_big, &org_small, "mine-1", &world_state);

        assert_eq!(result.winner_org_id, org_big.id);
        assert_eq!(result.loser_org_id, org_small.id);
        // Large power gap → low intensity (not a close contest)
        assert!(result.intensity < 0.5, "expected low intensity for a blowout, got {}", result.intensity);
    }

    // ── Recruitment Tests ─────────────────────────────────

    #[test]
    fn test_recruitment_prefers_larger_org() {
        let engine = CompetitionEngine::new();
        let org_large = make_org("BigOrg", OrgType::Company, 10, 1000, 100);
        let org_small = make_org("SmallOrg", OrgType::Guild, 2, 100, 100);

        let invitations = vec![
            OrgInvitation { org_id: org_large.id.clone(), pitch: "Join us!".to_string() },
            OrgInvitation { org_id: org_small.id.clone(), pitch: "We're cozy".to_string() },
        ];

        let result = engine.evaluate_recruitment_conflict(
            "agent-test",
            &invitations,
            &[org_large.clone(), org_small.clone()],
        );

        assert_eq!(result.chosen_org_id, org_large.id);
        assert!(result.scores.len() == 2);
    }

    #[test]
    fn test_recruitment_culture_match_bonus() {
        let engine = CompetitionEngine::new();

        // Two orgs with same size/treasury — winner is decided by culture match
        let org_a = make_org("OrgA", OrgType::Company, 5, 500, 100);
        let org_b = make_org("OrgB", OrgType::Company, 5, 500, 100);

        let invitations = vec![
            OrgInvitation { org_id: org_a.id.clone(), pitch: "A".to_string() },
            OrgInvitation { org_id: org_b.id.clone(), pitch: "B".to_string() },
        ];

        let result = engine.evaluate_recruitment_conflict(
            "test-agent-culture",
            &invitations,
            &[org_a.clone(), org_b.clone()],
        );

        // Both orgs have equal size/treasury, so the winner is determined by
        // culture match score (deterministic hash). Just verify a result is chosen.
        assert!(!result.chosen_org_id.is_empty());
        assert_eq!(result.scores.len(), 2);
        // Verify the culture match makes a difference (scores should differ)
        let score_a = result.scores.iter().find(|(id, _)| id == &org_a.id).map(|(_, s)| *s).unwrap_or(0.0);
        let score_b = result.scores.iter().find(|(id, _)| id == &org_b.id).map(|(_, s)| *s).unwrap_or(0.0);
        // With equal size and treasury, the scores will differ only by culture match
        assert_ne!(score_a, score_b, "culture match should differentiate equal orgs");
    }

    // ── Territory Tests ───────────────────────────────────

    #[test]
    fn test_territory_single_org() {
        let engine = CompetitionEngine::new();
        let world_state = make_world_state();
        let org = make_org("TerritoryOrg", OrgType::Alliance, 3, 500, 100);

        let result = engine.evaluate_territory(&org.id, &org.members, &world_state);

        assert_eq!(result.org_id, org.id);
        assert!(!result.regions.is_empty());
        assert!(result.total_influence > 0.0);
        // Each region influence is clamped to [0, 1]
        for region in &result.regions {
            assert!(region.influence >= 0.0 && region.influence <= 1.0);
        }
    }

    #[test]
    fn test_territory_overlapping_regions() {
        let engine = CompetitionEngine::new();
        let bus = Arc::new(EventBus::new(256));
        let registry = SubsystemRegistry::new();

        // Place agents so they map to the same region
        let agents = vec![
            (
                Uuid::new_v4(),
                50u64,  // spawn_tick = 50 → region_0
                AgentRecord {
                    id: Uuid::new_v4(),
                    name: "a1".to_string(),
                    phase: AgentPhase::Adult,
                    tokens: 100,
                    skills: HashMap::new(),
                    personality: String::new(),
            tasks_completed: 0,
            tasks_attempted: 0,
                },
            ),
            (
                Uuid::new_v4(),
                80u64,  // spawn_tick = 80 → region_0
                AgentRecord {
                    id: Uuid::new_v4(),
                    name: "a2".to_string(),
                    phase: AgentPhase::Adult,
                    tokens: 100,
                    skills: HashMap::new(),
                    personality: String::new(),
            tasks_completed: 0,
            tasks_attempted: 0,
                },
            ),
        ];

        let world_state = WorldState::new(bus, registry, agents);

        let org = Organization {
            id: Uuid::new_v4().to_string(),
            name: "OverlapOrg".to_string(),
            org_type: OrgType::Alliance,
            charter: test_charter(),
            treasury: 500,
            debts: 0,
            status: OrgStatus::Active,
            created_tick: 100,
            last_activity_tick: 100,
            members: vec![
                OrgMember {
                    agent_id: world_state.agents[0].0.to_string(),
                    agent_name: "a1".to_string(),
                    role: MemberRole::Founder,
                    share: 0.5,
                    joined_tick: 100,
                },
                OrgMember {
                    agent_id: world_state.agents[1].0.to_string(),
                    agent_name: "a2".to_string(),
                    role: MemberRole::Member,
                    share: 0.5,
                    joined_tick: 100,
                },
            ],
        };

        let result = engine.evaluate_territory(&org.id, &org.members, &world_state);

        assert_eq!(result.org_id, org.id);
        // Both agents are in region_0, so there should be one region
        assert_eq!(result.regions.len(), 1);
        // 2 members × 0.3 = 0.6 influence
        let expected_influence = 0.6_f64;
        assert!(
            (result.regions[0].influence - expected_influence).abs() < 0.001,
            "expected {} influence, got {}",
            expected_influence,
            result.regions[0].influence
        );
    }

    // ── Formation Scan Tests ──────────────────────────────

    #[test]
    fn test_scan_detects_formation_opportunity() {
        let mut engine = CompetitionEngine::new();
        let bus = Arc::new(EventBus::new(256));
        let registry = SubsystemRegistry::new();

        // Create agents in the same region (spawn_tick / 100 = same region)
        let agents: Vec<(Uuid, u64, AgentRecord)> = (0..3)
            .map(|i| {
                (
                    Uuid::new_v4(),
                    50u64, // all in region_0
                    AgentRecord {
                        id: Uuid::new_v4(),
                        name: format!("agent-{}", i),
                        phase: AgentPhase::Adult,
                        tokens: 100,
                        skills: HashMap::new(),
                        personality: String::new(),
            tasks_completed: 0,
            tasks_attempted: 0,
                    },
                )
            })
            .collect();

        let world_state = WorldState::new(bus, registry, agents);

        // Tick 50 — first scan should trigger
        let suggestions = engine.scan_for_formation(50, &[], &world_state);

        assert_eq!(suggestions.len(), 1);
        assert!(suggestions[0].agents.len() >= MIN_FORMATION_AGENTS);
        // 3 agents → Company (3..=5 range)
        assert_eq!(suggestions[0].suggested_type, OrgType::Company);
    }

    #[test]
    fn test_scan_respects_interval() {
        let mut engine = CompetitionEngine::new();
        let bus = Arc::new(EventBus::new(256));
        let registry = SubsystemRegistry::new();

        let agents: Vec<(Uuid, u64, AgentRecord)> = (0..3)
            .map(|i| {
                (
                    Uuid::new_v4(),
                    50u64,
                    AgentRecord {
                        id: Uuid::new_v4(),
                        name: format!("agent-{}", i),
                        phase: AgentPhase::Adult,
                        tokens: 100,
                        skills: HashMap::new(),
                        personality: String::new(),
            tasks_completed: 0,
            tasks_attempted: 0,
                    },
                )
            })
            .collect();

        let world_state = WorldState::new(bus, registry, agents);

        // First scan at tick 100
        let first = engine.scan_for_formation(100, &[], &world_state);
        assert!(!first.is_empty());

        // Second scan at tick 120 — should be skipped (interval = 50)
        let second = engine.scan_for_formation(120, &[], &world_state);
        assert!(second.is_empty());

        // Third scan at tick 150 — should trigger again
        let third = engine.scan_for_formation(150, &[], &world_state);
        assert!(!third.is_empty());
    }

    // ── Edge Case Tests ───────────────────────────────────

    #[test]
    fn test_recruitment_no_invitations() {
        let engine = CompetitionEngine::new();
        let result = engine.evaluate_recruitment_conflict("agent-1", &[], &[]);
        assert!(result.chosen_org_id.is_empty());
        assert_eq!(result.reason, "no invitations");
    }

    #[test]
    fn test_recruitment_single_invitation() {
        let engine = CompetitionEngine::new();
        let result = engine.evaluate_recruitment_conflict(
            "agent-1",
            &[OrgInvitation {
                org_id: "org-1".to_string(),
                pitch: "Join us".to_string(),
            }],
            &[],
        );
        assert_eq!(result.chosen_org_id, "org-1");
        assert_eq!(result.reason, "only one invitation");
    }

    #[test]
    fn test_resource_conflict_equal_orgs() {
        let mut engine = CompetitionEngine::new();
        let org_a = make_org("SameA", OrgType::Company, 5, 500, 100);
        let org_b = make_org("SameB", OrgType::Company, 5, 500, 100);
        let world_state = make_world_state();

        let result = engine.evaluate_resource_conflict(&org_a, &org_b, "res-1", &world_state);

        // Equal power → intensity should be 1.0 (perfect contest)
        assert!(
            (result.intensity - 1.0).abs() < 0.001,
            "expected intensity ~1.0 for equal orgs, got {}",
            result.intensity
        );
    }

    #[test]
    fn test_resource_conflict_skill_data_affects_power() {
        use crate::economy::token_burn::SkillRecord;

        let mut engine = CompetitionEngine::new();
        let bus = Arc::new(EventBus::new(256));
        let registry = SubsystemRegistry::new();

        // Create agent UUIDs first, then build orgs that reference them
        let agent_id_a0 = Uuid::new_v4();
        let agent_id_a1 = Uuid::new_v4();
        let agent_id_b0 = Uuid::new_v4();
        let agent_id_b1 = Uuid::new_v4();

        let org_a = Organization {
            id: Uuid::new_v4().to_string(),
            name: "SkilledOrg".to_string(),
            org_type: OrgType::Company,
            charter: test_charter(),
            treasury: 500,
            debts: 0,
            status: OrgStatus::Active,
            created_tick: 100,
            last_activity_tick: 100,
            members: vec![
                OrgMember {
                    agent_id: agent_id_a0.to_string(),
                    agent_name: "a0".to_string(),
                    role: MemberRole::Founder,
                    share: 0.5,
                    joined_tick: 100,
                },
                OrgMember {
                    agent_id: agent_id_a1.to_string(),
                    agent_name: "a1".to_string(),
                    role: MemberRole::Member,
                    share: 0.5,
                    joined_tick: 100,
                },
            ],
        };

        let org_b = Organization {
            id: Uuid::new_v4().to_string(),
            name: "UnskilledOrg".to_string(),
            org_type: OrgType::Company,
            charter: test_charter(),
            treasury: 500,
            debts: 0,
            status: OrgStatus::Active,
            created_tick: 100,
            last_activity_tick: 100,
            members: vec![
                OrgMember {
                    agent_id: agent_id_b0.to_string(),
                    agent_name: "b0".to_string(),
                    role: MemberRole::Founder,
                    share: 0.5,
                    joined_tick: 100,
                },
                OrgMember {
                    agent_id: agent_id_b1.to_string(),
                    agent_name: "b1".to_string(),
                    role: MemberRole::Member,
                    share: 0.5,
                    joined_tick: 100,
                },
            ],
        };

        // org_a members have high-level skills, org_b members have no skills (default 1.0)
        let mut skills_a: HashMap<String, SkillRecord> = HashMap::new();
        skills_a.insert("coding".to_string(), SkillRecord { name: "coding".to_string(), level: 8, experience: 0.0 });
        skills_a.insert("backend".to_string(), SkillRecord { name: "backend".to_string(), level: 7, experience: 0.0 });

        let agents = vec![
            (agent_id_a0, 0u64, AgentRecord {
                id: agent_id_a0, name: "a0".to_string(), phase: AgentPhase::Adult,
                tokens: 100, skills: skills_a.clone(), personality: String::new(),
                tasks_completed: 0, tasks_attempted: 0,
            }),
            (agent_id_a1, 0u64, AgentRecord {
                id: agent_id_a1, name: "a1".to_string(), phase: AgentPhase::Adult,
                tokens: 100, skills: skills_a, personality: String::new(),
                tasks_completed: 0, tasks_attempted: 0,
            }),
            (agent_id_b0, 0u64, AgentRecord {
                id: agent_id_b0, name: "b0".to_string(), phase: AgentPhase::Adult,
                tokens: 100, skills: HashMap::new(), personality: String::new(),
                tasks_completed: 0, tasks_attempted: 0,
            }),
            (agent_id_b1, 0u64, AgentRecord {
                id: agent_id_b1, name: "b1".to_string(), phase: AgentPhase::Adult,
                tokens: 100, skills: HashMap::new(), personality: String::new(),
                tasks_completed: 0, tasks_attempted: 0,
            }),
        ];

        let world_state = WorldState::new(bus, registry, agents);

        let result = engine.evaluate_resource_conflict(&org_a, &org_b, "skill-res-1", &world_state);

        // org_a has higher skill levels (avg ~7.5) vs org_b (default 1.0),
        // so org_a should win despite same member count and treasury
        assert_eq!(result.winner_org_id, org_a.id);
        assert_eq!(result.loser_org_id, org_b.id);
    }

    #[test]
    fn test_formation_suggests_correct_types() {
        // 2 agents → Guild
        let agents_2: Vec<String> = (0..2).map(|i| format!("agent-{}", i)).collect();
        assert_eq!(CompetitionEngine::suggest_org_type(&agents_2, 100), OrgType::Guild);
        // 4 agents → Company
        let agents_4: Vec<String> = (0..4).map(|i| format!("agent-{}", i)).collect();
        assert_eq!(CompetitionEngine::suggest_org_type(&agents_4, 100), OrgType::Company);
        // 7 agents → Alliance
        let agents_7: Vec<String> = (0..7).map(|i| format!("agent-{}", i)).collect();
        assert_eq!(CompetitionEngine::suggest_org_type(&agents_7, 100), OrgType::Alliance);
        // 15 agents → University
        let agents_15: Vec<String> = (0..15).map(|i| format!("agent-{}", i)).collect();
        assert_eq!(CompetitionEngine::suggest_org_type(&agents_15, 100), OrgType::University);
    }

    #[test]
    fn test_culture_match_deterministic() {
        let score1 = CompetitionEngine::culture_match_score("agent-1", "org-1");
        let score2 = CompetitionEngine::culture_match_score("agent-1", "org-1");
        assert_eq!(score1, score2, "culture match should be deterministic");

        let score3 = CompetitionEngine::culture_match_score("agent-1", "org-2");
        assert_ne!(score1, score3, "different org should give different score");
    }
}
