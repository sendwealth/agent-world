//! Legislation cycle engine — End-to-end election → legislation → enforcement pipeline.
//!
//! This module provides the orchestrator that closes the self-legislation loop:
//! 1. **Election trigger**: Based on governance config (auto or manual).
//! 2. **Rule proposal collection**: Leader or members propose candidate rules.
//! 3. **Voting & tally**: Organization members vote; quorum + majority required.
//! 4. **Rule activation**: Passed rules are written to the RuleEngine.
//! 5. **Feedback loop**: Execution effects feed back into governance metrics.
//!
//! It ties together `LeadershipEngine`, `RuleEngine`, `GovernanceSystem`, and
//! `GovernanceMetricsCollector` into a single coherent cycle.

use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::organization::governance::{GovernanceSystem, ProposalStatus, ProposalType};
use crate::organization::leadership::{LeadershipEngine, VotingMethod};
use crate::organization::rule_engine::{
    RuleCondition, RuleEffect, RuleEngine, RuleStatus, RuleType,
};
use crate::world::event::WorldEvent;
use crate::world::state::EventBus;

// ── Cycle Status ──────────────────────────────────────────

/// Status of a legislation cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CycleStatus {
    /// No active cycle for this org.
    Idle,
    /// Election in progress to select a leader.
    ElectionInProgress,
    /// Leader elected, collecting rule proposals.
    CollectingProposals,
    /// Proposals collected, voting is open.
    VotingOpen,
    /// Tallying votes and activating rules.
    Tallying,
    /// Cycle completed — rules enacted.
    Enacted,
    /// Cycle completed — proposals rejected.
    Rejected,
    /// Cycle failed due to an error.
    Failed,
}

impl std::fmt::Display for CycleStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CycleStatus::Idle => write!(f, "idle"),
            CycleStatus::ElectionInProgress => write!(f, "election_in_progress"),
            CycleStatus::CollectingProposals => write!(f, "collecting_proposals"),
            CycleStatus::VotingOpen => write!(f, "voting_open"),
            CycleStatus::Tallying => write!(f, "tallying"),
            CycleStatus::Enacted => write!(f, "enacted"),
            CycleStatus::Rejected => write!(f, "rejected"),
            CycleStatus::Failed => write!(f, "failed"),
        }
    }
}

// ── Rule Proposal Input ───────────────────────────────────

/// A candidate rule submitted during the legislation cycle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateRule {
    pub proposer_id: String,
    pub title: String,
    pub description: String,
    pub rule_type: RuleType,
    pub conditions: Vec<RuleCondition>,
    pub effects: Vec<RuleEffect>,
    pub expires_tick: Option<u64>,
}

/// Type of candidate rule proposal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CandidateRuleType {
    #[default]
    NewRule,
    RepealRule,
    AmendRule,
}

impl CandidateRule {
    /// Create a repeal proposal targeting an existing rule.
    pub fn repeal(proposer_id: String, target_rule_id: String, reason: String) -> Self {
        CandidateRule {
            proposer_id,
            title: format!("Repeal rule {}", target_rule_id),
            description: reason,
            rule_type: RuleType::Custom,
            conditions: vec![],
            effects: vec![RuleEffect {
                target: format!("rule.{}", target_rule_id),
                action: "repeal".to_string(),
                value: serde_json::json!(true),
            }],
            expires_tick: None,
        }
    }
}

// ── Cycle Record ──────────────────────────────────────────

/// Record of a single legislation cycle for an organization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegislationCycleRecord {
    pub cycle_id: Uuid,
    pub org_id: Uuid,
    pub status: CycleStatus,
    /// The leader elected for this cycle (set after election resolves).
    pub leader_id: Option<String>,
    /// Candidate rules submitted during the proposal phase.
    pub candidate_rules: Vec<CandidateRule>,
    /// The governance proposal ID (created when entering voting phase).
    pub governance_proposal_id: Option<Uuid>,
    /// Rule IDs activated by this cycle.
    pub enacted_rule_ids: Vec<String>,
    /// Tick when the cycle was created.
    pub started_at_tick: u64,
    /// Tick when the cycle completed (enacted/rejected/failed).
    pub completed_at_tick: Option<u64>,
    /// Human-readable reason for the cycle.
    pub trigger_reason: String,
}

// ── Errors ────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegislationCycleError {
    NoActiveCycle { org_id: Uuid },
    CycleAlreadyActive { org_id: Uuid },
    OrganizationNotFound(Uuid),
    NotALeader { org_id: Uuid, agent_id: String },
    NoLeaderElected { org_id: Uuid },
    NoCandidateRules { org_id: Uuid },
    ProposalSubmissionFailed { reason: String },
    VotingFailed { reason: String },
    GovernanceError(String),
}

impl std::fmt::Display for LegislationCycleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LegislationCycleError::NoActiveCycle { org_id } => {
                write!(f, "no active legislation cycle for org {}", org_id)
            }
            LegislationCycleError::CycleAlreadyActive { org_id } => {
                write!(f, "legislation cycle already active for org {}", org_id)
            }
            LegislationCycleError::OrganizationNotFound(id) => {
                write!(f, "organization not found: {}", id)
            }
            LegislationCycleError::NotALeader { org_id, agent_id } => {
                write!(f, "agent {} is not the leader of org {}", agent_id, org_id)
            }
            LegislationCycleError::NoLeaderElected { org_id } => {
                write!(f, "no leader elected for org {}", org_id)
            }
            LegislationCycleError::NoCandidateRules { org_id } => {
                write!(f, "no candidate rules submitted for org {}", org_id)
            }
            LegislationCycleError::ProposalSubmissionFailed { reason } => {
                write!(f, "proposal submission failed: {}", reason)
            }
            LegislationCycleError::VotingFailed { reason } => {
                write!(f, "voting failed: {}", reason)
            }
            LegislationCycleError::GovernanceError(msg) => {
                write!(f, "governance error: {}", msg)
            }
        }
    }
}

impl std::error::Error for LegislationCycleError {}

// ── Legislation Cycle Configuration ───────────────────────

/// Configuration for the legislation cycle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegislationCycleConfig {
    /// Minimum number of candidate rules before voting can start.
    pub min_proposals: usize,
    /// Quorum for auto-activation (number of votes).
    pub quorum: usize,
    /// Auto-trigger: if true, start a new cycle automatically when conditions are met.
    pub auto_trigger: bool,
    /// Voting method for the leadership election.
    pub election_method: VotingMethod,
    /// Interval in ticks between automatic election triggers (0 = disabled).
    pub election_interval_ticks: u64,
    /// Minimum number of members required to auto-trigger a cycle.
    pub min_members_for_auto_trigger: usize,
    /// Whether to allow rule repeal proposals through the legislation cycle.
    pub allow_repeal_proposals: bool,
}

impl Default for LegislationCycleConfig {
    fn default() -> Self {
        LegislationCycleConfig {
            min_proposals: 1,
            quorum: 3,
            auto_trigger: true,
            election_method: VotingMethod::SimpleMajority,
            election_interval_ticks: 100,
            min_members_for_auto_trigger: 3,
            allow_repeal_proposals: true,
        }
    }
}

// ── Legislation Cycle Engine ──────────────────────────────

/// Orchestrator for the complete self-legislation cycle.
///
/// Manages the end-to-end flow: election → rule proposal → voting → activation → feedback.
///
/// Usage:
/// ```ignore
/// // Ignored: requires external types (leadership, governance) not constructible inline.
/// let engine = LegislationCycleEngine::new(config);
/// // Start a cycle (triggers election)
/// engine.start_cycle(org_id, candidates, tick)?;
/// // Resolve election (after external voting)
/// engine.resolve_election(&mut leadership, org_id)?;
/// // Submit candidate rules
/// engine.submit_candidate_rule(org_id, rule)?;
/// // Move to voting phase
/// engine.start_voting_phase(&mut governance, org_id, tick)?;
/// // Tally and enact
/// engine.tally_and_enact(&mut governance, org_id, tick)?;
/// ```
pub struct LegislationCycleEngine {
    /// Active and completed cycles keyed by org_id.
    cycles: HashMap<Uuid, LegislationCycleRecord>,
    config: LegislationCycleConfig,
    event_bus: Option<Arc<EventBus>>,
}

impl LegislationCycleEngine {
    pub fn new(config: LegislationCycleConfig) -> Self {
        Self {
            cycles: HashMap::new(),
            config,
            event_bus: None,
        }
    }

    pub fn with_event_bus(config: LegislationCycleConfig, event_bus: Arc<EventBus>) -> Self {
        Self {
            cycles: HashMap::new(),
            config,
            event_bus: Some(event_bus),
        }
    }

    // ── Phase 1: Start Cycle (Election) ────────────────────

    /// Start a new legislation cycle by initiating a leadership election.
    ///
    /// This begins Phase 1: the organization elects a leader who will drive
    /// the legislation process.
    pub fn start_cycle(
        &mut self,
        org_id: Uuid,
        _candidates: Vec<String>,
        tick: u64,
        reason: &str,
    ) -> Result<Uuid, LegislationCycleError> {
        // Check no active cycle exists
        if let Some(existing) = self.cycles.get(&org_id) {
            if !matches!(
                existing.status,
                CycleStatus::Enacted
                    | CycleStatus::Rejected
                    | CycleStatus::Failed
                    | CycleStatus::Idle
            ) {
                return Err(LegislationCycleError::CycleAlreadyActive { org_id });
            }
        }

        let cycle_id = Uuid::new_v4();
        let record = LegislationCycleRecord {
            cycle_id,
            org_id,
            status: CycleStatus::ElectionInProgress,
            leader_id: None,
            candidate_rules: Vec::new(),
            governance_proposal_id: None,
            enacted_rule_ids: Vec::new(),
            started_at_tick: tick,
            completed_at_tick: None,
            trigger_reason: reason.to_string(),
        };

        self.cycles.insert(org_id, record);
        Ok(cycle_id)
    }

    /// Start a cycle with a pre-elected leader (skips election phase).
    ///
    /// Use this when the leader is already known (e.g. appointed leader, or
    /// election was conducted externally).
    pub fn start_cycle_with_leader(
        &mut self,
        org_id: Uuid,
        leader_id: String,
        tick: u64,
        reason: &str,
    ) -> Result<Uuid, LegislationCycleError> {
        if let Some(existing) = self.cycles.get(&org_id) {
            if !matches!(
                existing.status,
                CycleStatus::Enacted
                    | CycleStatus::Rejected
                    | CycleStatus::Failed
                    | CycleStatus::Idle
            ) {
                return Err(LegislationCycleError::CycleAlreadyActive { org_id });
            }
        }

        let cycle_id = Uuid::new_v4();
        let record = LegislationCycleRecord {
            cycle_id,
            org_id,
            status: CycleStatus::CollectingProposals,
            leader_id: Some(leader_id),
            candidate_rules: Vec::new(),
            governance_proposal_id: None,
            enacted_rule_ids: Vec::new(),
            started_at_tick: tick,
            completed_at_tick: None,
            trigger_reason: reason.to_string(),
        };

        self.cycles.insert(org_id, record);
        Ok(cycle_id)
    }

    // ── Phase 1b: Resolve Election ─────────────────────────

    /// Resolve the election phase and transition to proposal collection.
    ///
    /// Must be called after the election is resolved in the `LeadershipEngine`.
    pub fn resolve_election(
        &mut self,
        leadership: &LeadershipEngine,
        org_id: Uuid,
    ) -> Result<(), LegislationCycleError> {
        let record = self
            .cycles
            .get_mut(&org_id)
            .ok_or(LegislationCycleError::NoActiveCycle { org_id })?;

        if record.status != CycleStatus::ElectionInProgress {
            return Err(LegislationCycleError::GovernanceError(format!(
                "cycle is in {} status, expected ElectionInProgress",
                record.status
            )));
        }

        let leader_id = leadership
            .get_leader(org_id)
            .ok_or(LegislationCycleError::NoLeaderElected { org_id })?
            .to_string();

        record.leader_id = Some(leader_id);
        record.status = CycleStatus::CollectingProposals;
        Ok(())
    }

    // ── Phase 2: Collect Candidate Rules ───────────────────

    /// Submit a candidate rule proposal for the active legislation cycle.
    ///
    /// Only the elected leader (or any member in democratic mode) can submit.
    pub fn submit_candidate_rule(
        &mut self,
        org_id: Uuid,
        rule: CandidateRule,
    ) -> Result<(), LegislationCycleError> {
        let record = self
            .cycles
            .get_mut(&org_id)
            .ok_or(LegislationCycleError::NoActiveCycle { org_id })?;

        if record.status != CycleStatus::CollectingProposals {
            return Err(LegislationCycleError::GovernanceError(format!(
                "cycle is in {} status, expected CollectingProposals",
                record.status
            )));
        }

        record.candidate_rules.push(rule);
        Ok(())
    }

    /// Get the current candidate rules for an org's legislation cycle.
    pub fn get_candidate_rules(
        &self,
        org_id: Uuid,
    ) -> Result<&[CandidateRule], LegislationCycleError> {
        let record = self
            .cycles
            .get(&org_id)
            .ok_or(LegislationCycleError::NoActiveCycle { org_id })?;
        Ok(&record.candidate_rules)
    }

    // ── Phase 3: Start Voting ──────────────────────────────

    /// Transition from proposal collection to voting phase.
    ///
    /// Creates a governance proposal of type `SoftRuleProposal` containing all
    /// candidate rules as a batch. The proposal goes through the standard
    /// governance voting flow.
    pub fn start_voting_phase(
        &mut self,
        governance: &mut GovernanceSystem,
        org_id: Uuid,
        tick: u64,
    ) -> Result<Uuid, LegislationCycleError> {
        let record = self
            .cycles
            .get(&org_id)
            .ok_or(LegislationCycleError::NoActiveCycle { org_id })?;

        if record.status != CycleStatus::CollectingProposals {
            return Err(LegislationCycleError::GovernanceError(format!(
                "cycle is in {} status, expected CollectingProposals",
                record.status
            )));
        }

        if record.candidate_rules.is_empty() {
            return Err(LegislationCycleError::NoCandidateRules { org_id });
        }

        let leader_id = record
            .leader_id
            .clone()
            .ok_or(LegislationCycleError::NoLeaderElected { org_id })?;

        // Build the payload containing all candidate rules
        let rules_payload: Vec<Value> = record
            .candidate_rules
            .iter()
            .enumerate()
            .map(|(i, rule)| {
                serde_json::json!({
                    "rule_index": i,
                    "proposer_id": rule.proposer_id,
                    "title": rule.title,
                    "description": rule.description,
                    "rule_type": rule.rule_type.to_string(),
                    "conditions": rule.conditions,
                    "effects": rule.effects,
                    "expires_tick": rule.expires_tick,
                })
            })
            .collect();

        let title = format!(
            "Legislation Cycle: {} rule(s)",
            record.candidate_rules.len()
        );
        let description = format!(
            "Self-legislation cycle for org {}. Trigger: {}",
            org_id, record.trigger_reason
        );

        let payload = serde_json::json!({
            "legislation_cycle": true,
            "rules": rules_payload,
            // Intentionally omit "rule_id" so execute_proposal_side_effect
            // does NOT create a duplicate rule — we enact via enact_candidate_rules.
            "proposer_id": leader_id.clone(),
            "title": title,
            "description": description,
        });

        // Create a governance proposal
        let proposal_id = governance
            .create_proposal(
                org_id,
                leader_id.clone(),
                ProposalType::SoftRuleProposal,
                title,
                description,
                tick,
                Some(payload),
            )
            .map_err(|e| LegislationCycleError::GovernanceError(e.to_string()))?;

        // Start voting immediately (skip discussion for auto-triggered legislation)
        governance
            .start_voting(proposal_id, &leader_id, tick)
            .map_err(|e| LegislationCycleError::VotingFailed {
                reason: e.to_string(),
            })?;

        // Update cycle record
        let record = self
            .cycles
            .get_mut(&org_id)
            .ok_or(LegislationCycleError::OrganizationNotFound(org_id))?;
        record.governance_proposal_id = Some(proposal_id);
        record.status = CycleStatus::VotingOpen;

        Ok(proposal_id)
    }

    // ── Phase 4: Cast Votes via Governance ─────────────────

    /// Cast a vote on the active legislation proposal via the governance system.
    pub fn cast_vote(
        &self,
        governance: &mut GovernanceSystem,
        org_id: Uuid,
        voter_id: String,
        in_favor: bool,
        tick: u64,
    ) -> Result<(), LegislationCycleError> {
        let record = self
            .cycles
            .get(&org_id)
            .ok_or(LegislationCycleError::NoActiveCycle { org_id })?;

        if record.status != CycleStatus::VotingOpen {
            return Err(LegislationCycleError::GovernanceError(format!(
                "cycle is in {} status, expected VotingOpen",
                record.status
            )));
        }

        let proposal_id = record
            .governance_proposal_id
            .ok_or(LegislationCycleError::VotingFailed {
                reason: "no governance proposal".to_string(),
            })?;

        governance
            .vote(proposal_id, voter_id, in_favor, tick)
            .map_err(|e| LegislationCycleError::VotingFailed {
                reason: e.to_string(),
            })
    }

    // ── Phase 5: Tally and Enact ───────────────────────────

    /// Tally votes and enact the rules if the proposal passes.
    ///
    /// This is the final phase of the cycle. If the governance proposal passes,
    /// all candidate rules are created in the RuleEngine and auto-activated.
    /// If it fails, the cycle is marked as rejected.
    ///
    /// Returns the list of enacted rule IDs (empty if rejected).
    pub fn tally_and_enact(
        &mut self,
        governance: &mut GovernanceSystem,
        org_id: Uuid,
        tick: u64,
    ) -> Result<Vec<String>, LegislationCycleError> {
        let record = self
            .cycles
            .get(&org_id)
            .ok_or(LegislationCycleError::NoActiveCycle { org_id })?;

        if record.status != CycleStatus::VotingOpen {
            return Err(LegislationCycleError::GovernanceError(format!(
                "cycle is in {} status, expected VotingOpen",
                record.status
            )));
        }

        let proposal_id = record
            .governance_proposal_id
            .ok_or(LegislationCycleError::VotingFailed {
                reason: "no governance proposal".to_string(),
            })?;

        // Tally the governance proposal
        let result = governance
            .tally_proposal(proposal_id)
            .map_err(|e| LegislationCycleError::GovernanceError(e.to_string()))?;

        if result == ProposalStatus::Executed {
            // Enact the candidate rules in the RuleEngine
            let enacted = self.enact_candidate_rules(governance, org_id, tick)?;

            // Update cycle record
            let record = self
                .cycles
                .get_mut(&org_id)
                .ok_or(LegislationCycleError::OrganizationNotFound(org_id))?;
            let cycle_id = record.cycle_id;
            record.enacted_rule_ids = enacted.clone();
            record.status = CycleStatus::Enacted;
            record.completed_at_tick = Some(tick);

            self.emit(WorldEvent::SoftRuleActivated {
                rule_id: format!("legislation-cycle-{}", cycle_id),
                org_id: org_id.to_string(),
            });

            Ok(enacted)
        } else {
            let record = self
                .cycles
                .get_mut(&org_id)
                .ok_or(LegislationCycleError::OrganizationNotFound(org_id))?;
            record.status = CycleStatus::Rejected;
            record.completed_at_tick = Some(tick);
            Ok(Vec::new())
        }
    }

    /// Create and activate individual rules from the candidate rules in the RuleEngine.
    fn enact_candidate_rules(
        &self,
        governance: &mut GovernanceSystem,
        org_id: Uuid,
        tick: u64,
    ) -> Result<Vec<String>, LegislationCycleError> {
        let record = self
            .cycles
            .get(&org_id)
            .ok_or(LegislationCycleError::OrganizationNotFound(org_id))?;
        let mut enacted_ids = Vec::new();

        for rule in &record.candidate_rules {
            let rule_id = governance.active_rules.propose_rule(
                rule.proposer_id.clone(),
                org_id.to_string(),
                rule.title.clone(),
                rule.description.clone(),
                rule.rule_type,
                rule.conditions.clone(),
                rule.effects.clone(),
                tick,
                rule.expires_tick,
            );

            // Activate immediately since the proposal already passed governance vote
            if let Err(e) = governance.active_rules.activate_rule(&rule_id) {
                tracing::warn!("Failed to activate rule {}: {}", rule_id, e);
            } else {
                // Emit activation event
                self.emit(WorldEvent::SoftRuleActivated {
                    rule_id: rule_id.clone(),
                    org_id: org_id.to_string(),
                });
            }

            enacted_ids.push(rule_id);
        }

        Ok(enacted_ids)
    }

    // ── Run Full Cycle (convenience) ───────────────────────

    /// Run the complete legislation cycle from election through enactment in one call.
    ///
    /// This is a convenience method for testing or automated scenarios where
    /// all votes are known upfront.
    #[allow(clippy::too_many_arguments)]
    pub fn run_full_cycle(
        &mut self,
        governance: &mut GovernanceSystem,
        leadership: &mut LeadershipEngine,
        org_id: Uuid,
        candidates: Vec<String>,
        member_votes: &[(String, bool)],
        candidate_rules: Vec<CandidateRule>,
        tick: u64,
        reason: &str,
    ) -> Result<(Uuid, Vec<String>), LegislationCycleError> {
        // Phase 1: Start cycle
        let cycle_id = self.start_cycle(org_id, candidates.clone(), tick, reason)?;

        // Phase 1b: Run election
        leadership
            .initiate_election(org_id, candidates.clone(), self.config.election_method, tick)
            .map_err(|e| LegislationCycleError::GovernanceError(e.to_string()))?;

        // Auto-cast election votes (each candidate votes for first candidate to ensure resolution)
        let election_candidates: Vec<String> = leadership
            .get_active_election(org_id)
            .map(|e| e.candidates.clone())
            .unwrap_or_default();

        if let Some(first) = election_candidates.first() {
            for candidate in &election_candidates {
                leadership
                    .cast_vote(org_id, candidate.clone(), vec![first.clone()])
                    .ok();
            }
        }

        leadership
            .resolve_election(org_id)
            .map_err(|e| LegislationCycleError::GovernanceError(e.to_string()))?;

        self.resolve_election(leadership, org_id)?;

        // Phase 2: Submit candidate rules
        for rule in candidate_rules {
            self.submit_candidate_rule(org_id, rule)?;
        }

        // Phase 3: Start voting
        self.start_voting_phase(governance, org_id, tick)?;

        // Phase 4: Cast member votes
        for (voter_id, in_favor) in member_votes {
            self.cast_vote(governance, org_id, voter_id.clone(), *in_favor, tick)?;
        }

        // Phase 5: Tally and enact
        let enacted = self.tally_and_enact(governance, org_id, tick)?;

        Ok((cycle_id, enacted))
    }

    // ── Feedback ───────────────────────────────────────────

    /// Evaluate the effects of rules enacted by a completed cycle.
    ///
    /// Returns a summary of how many rules are still active, how many have
    /// expired, and the overall trigger rate. This data feeds back into
    /// the governance metrics for future cycle decisions.
    pub fn evaluate_cycle_effects(
        &self,
        rule_engine: &RuleEngine,
        org_id: Uuid,
    ) -> CycleEffectSummary {
        let record = match self.cycles.get(&org_id) {
            Some(r) => r,
            None => {
                return CycleEffectSummary {
                    org_id,
                    total_enacted: 0,
                    still_active: 0,
                    expired: 0,
                    repealed: 0,
                    suspended: 0,
                }
            }
        };

        let mut summary = CycleEffectSummary {
            org_id,
            total_enacted: record.enacted_rule_ids.len(),
            still_active: 0,
            expired: 0,
            repealed: 0,
            suspended: 0,
        };

        for rule_id in &record.enacted_rule_ids {
            if let Some(rule) = rule_engine.get_rule(rule_id) {
                match rule.status {
                    RuleStatus::Active => summary.still_active += 1,
                    RuleStatus::Suspended => summary.suspended += 1,
                    RuleStatus::Repealed => summary.repealed += 1,
                    RuleStatus::Proposed => {} // shouldn't happen for enacted rules
                }
            } else {
                // Rule was removed or never created
                summary.expired += 1;
            }
        }

        summary
    }

    // ── Auto-Trigger ────────────────────────────────────────

    /// Check if auto-trigger conditions are met for an organization.
    pub fn should_auto_trigger(&self, org_id: Uuid, current_tick: u64, member_count: usize) -> Option<String> {
        if !self.config.auto_trigger { return None; }
        if member_count < self.config.min_members_for_auto_trigger { return None; }
        if let Some(cycle) = self.cycles.get(&org_id) {
            if !matches!(cycle.status,
                CycleStatus::Enacted | CycleStatus::Rejected | CycleStatus::Failed | CycleStatus::Idle
            ) { return None; }
        }
        if self.config.election_interval_ticks == 0 { return None; }
        let last_tick = self.cycles.get(&org_id).and_then(|c| c.completed_at_tick);
        match last_tick {
            None => {
                if current_tick >= self.config.election_interval_ticks {
                    Some(format!("auto-trigger: first cycle at tick {}", current_tick))
                } else { None }
            }
            Some(completed) => {
                let elapsed = current_tick.saturating_sub(completed);
                if elapsed >= self.config.election_interval_ticks {
                    Some(format!("auto-trigger: {} ticks since last cycle", elapsed))
                } else { None }
            }
        }
    }

    /// Batch auto-trigger for multiple organizations.
    pub fn tick_auto_trigger(&mut self, current_tick: u64, org_member_counts: &[(Uuid, usize)]) -> Vec<(Uuid, String)> {
        let mut triggered = Vec::new();
        for &(org_id, mc) in org_member_counts {
            if let Some(reason) = self.should_auto_trigger(org_id, current_tick, mc) {
                if self.start_cycle(org_id, vec![], current_tick, &reason).is_ok() {
                    triggered.push((org_id, reason));
                }
            }
        }
        triggered
    }

    /// Tick-based trigger for a single organization.
    pub fn tick_org(&mut self, org_id: Uuid, current_tick: u64, member_count: usize, candidates: Vec<String>) -> Option<Uuid> {
        let reason = self.should_auto_trigger(org_id, current_tick, member_count)?;
        self.start_cycle(org_id, candidates, current_tick, &reason).ok()
    }

    /// Event-based trigger: start a cycle in response to a governance event.
    pub fn trigger_from_event(&mut self, org_id: Uuid, event_description: &str, tick: u64, candidates: Vec<String>) -> Result<Uuid, LegislationCycleError> {
        let reason = format!("event-trigger: {}", event_description);
        self.start_cycle(org_id, candidates, tick, &reason)
    }

    /// Full auto-trigger pipeline: check -> start -> elect -> resolve.
    pub fn auto_trigger_and_elect(
        &mut self, _governance: &mut GovernanceSystem, leadership: &mut LeadershipEngine,
        org_id: Uuid, current_tick: u64, member_count: usize, candidates: Vec<String>,
    ) -> Option<(Uuid, String)> {
        let reason = self.should_auto_trigger(org_id, current_tick, member_count)?;
        let cycle_id = self.start_cycle(org_id, candidates.clone(), current_tick, &reason).ok()?;
        leadership.initiate_election(org_id, candidates.clone(), self.config.election_method, current_tick).ok()?;
        let election_candidates: Vec<String> = leadership.get_active_election(org_id).map(|e| e.candidates.clone()).unwrap_or_default();
        if let Some(first) = election_candidates.first() {
            for candidate in &election_candidates {
                leadership.cast_vote(org_id, candidate.clone(), vec![first.clone()]).ok();
            }
        }
        leadership.resolve_election(org_id).ok()?;
        self.resolve_election(leadership, org_id).ok()?;
        let leader_id = self.cycles.get(&org_id).and_then(|c| c.leader_id.clone())?;
        Some((cycle_id, leader_id))
    }

    // ── Rule Repeal via Cycle ───────────────────────────────

    /// Submit a repeal proposal for an existing rule.
    pub fn submit_repeal_proposal(&mut self, org_id: Uuid, proposer_id: String, target_rule_id: String, reason: String) -> Result<(), LegislationCycleError> {
        if !self.config.allow_repeal_proposals {
            return Err(LegislationCycleError::GovernanceError("repeal proposals are not allowed by the current configuration".to_string()));
        }
        self.submit_candidate_rule(org_id, CandidateRule::repeal(proposer_id, target_rule_id, reason))
    }

    /// Process repeal effects after enactment.
    pub fn process_repeal_effects(&self, rule_engine: &mut RuleEngine, org_id: Uuid, current_tick: u64) -> Vec<String> {
        let record = match self.cycles.get(&org_id) { Some(r) => r, None => return Vec::new() };
        let mut repealed = Vec::new();
        for rule in &record.candidate_rules {
            for effect in &rule.effects {
                if effect.action == "repeal" {
                    if let Some(target_id) = effect.target.strip_prefix("rule.") {
                        if rule_engine.repeal_rule(target_id, current_tick).is_ok() {
                            repealed.push(target_id.to_string());
                        }
                    }
                }
            }
        }
        repealed
    }

        // ── Query ──────────────────────────────────────────────

    /// Get the current cycle record for an organization.
    pub fn get_cycle(&self, org_id: Uuid) -> Option<&LegislationCycleRecord> {
        self.cycles.get(&org_id)
    }

    /// Get the current cycle status for an organization.
    pub fn get_cycle_status(&self, org_id: Uuid) -> CycleStatus {
        self.cycles
            .get(&org_id)
            .map(|r| r.status)
            .unwrap_or(CycleStatus::Idle)
    }

    /// List all active (non-completed) cycles.
    pub fn active_cycles(&self) -> Vec<&LegislationCycleRecord> {
        self.cycles
            .values()
            .filter(|r| {
                !matches!(
                    r.status,
                    CycleStatus::Enacted
                        | CycleStatus::Rejected
                        | CycleStatus::Failed
                        | CycleStatus::Idle
                )
            })
            .collect()
    }

    /// List all completed cycles.
    pub fn completed_cycles(&self) -> Vec<&LegislationCycleRecord> {
        self.cycles
            .values()
            .filter(|r| {
                matches!(
                    r.status,
                    CycleStatus::Enacted | CycleStatus::Rejected | CycleStatus::Failed
                )
            })
            .collect()
    }

    /// Get the cycle configuration.
    pub fn config(&self) -> &LegislationCycleConfig {
        &self.config
    }

    // ── Helpers ────────────────────────────────────────────

    fn emit(&self, event: WorldEvent) {
        if let Some(ref bus) = self.event_bus {
            bus.emit(event);
        }
    }
}

// ── Cycle Effect Summary ──────────────────────────────────

/// Summary of the effects of a completed legislation cycle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CycleEffectSummary {
    pub org_id: Uuid,
    pub total_enacted: usize,
    pub still_active: usize,
    pub expired: usize,
    pub repealed: usize,
    pub suspended: usize,
}

// ── Tests ─────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::organization::governance::DecisionMode;
    use serde_json::json;

    fn make_candidate_rule(proposer: &str, title: &str) -> CandidateRule {
        CandidateRule {
            proposer_id: proposer.to_string(),
            title: title.to_string(),
            description: format!("{} for the org", title),
            rule_type: RuleType::Tax,
            conditions: vec![RuleCondition {
                field: "agent.resources".to_string(),
                operator: ">".to_string(),
                value: json!(200),
            }],
            effects: vec![RuleEffect {
                target: "agent.tax_bonus".to_string(),
                action: "set".to_string(),
                value: json!(0.1),
            }],
            expires_tick: None,
        }
    }

    fn setup_engines() -> (
        LegislationCycleEngine,
        GovernanceSystem,
        LeadershipEngine,
    ) {
        let config = LegislationCycleConfig {
            min_proposals: 1,
            quorum: 2,
            auto_trigger: true,
            election_method: VotingMethod::SimpleMajority,
            election_interval_ticks: 100,
            min_members_for_auto_trigger: 3,
            allow_repeal_proposals: true,
        };
        let cycle_engine = LegislationCycleEngine::new(config);
        let governance = GovernanceSystem::new();
        let leadership = LeadershipEngine::new();
        (cycle_engine, governance, leadership)
    }

    // ── Phase 1: Start Cycle ───────────────────────────────

    #[test]
    fn test_start_cycle() {
        let (mut engine, _, _) = setup_engines();
        let org_id = Uuid::new_v4();

        let cycle_id = engine
            .start_cycle(
                org_id,
                vec!["alice".to_string(), "bob".to_string()],
                10,
                "governance review",
            )
            .unwrap();

        let record = engine.get_cycle(org_id).unwrap();
        assert_eq!(record.cycle_id, cycle_id);
        assert_eq!(record.status, CycleStatus::ElectionInProgress);
        assert!(record.leader_id.is_none());
        assert_eq!(record.trigger_reason, "governance review");
    }

    #[test]
    fn test_start_cycle_already_active_fails() {
        let (mut engine, _, _) = setup_engines();
        let org_id = Uuid::new_v4();

        engine
            .start_cycle(org_id, vec!["alice".to_string()], 10, "test")
            .unwrap();

        let result = engine.start_cycle(org_id, vec!["bob".to_string()], 20, "test2");
        assert!(result.is_err());
    }

    #[test]
    fn test_start_cycle_with_leader() {
        let (mut engine, _, _) = setup_engines();
        let org_id = Uuid::new_v4();

        let cycle_id = engine
            .start_cycle_with_leader(
                org_id,
                "alice".to_string(),
                10,
                "appointed leader",
            )
            .unwrap();

        let record = engine.get_cycle(org_id).unwrap();
        assert_eq!(record.cycle_id, cycle_id);
        assert_eq!(record.status, CycleStatus::CollectingProposals);
        assert_eq!(record.leader_id, Some("alice".to_string()));
    }

    // ── Phase 1b: Resolve Election ─────────────────────────

    #[test]
    fn test_resolve_election() {
        let (mut engine, _, mut leadership) = setup_engines();
        let org_id = Uuid::new_v4();

        engine
            .start_cycle(
                org_id,
                vec!["alice".to_string(), "bob".to_string()],
                10,
                "test",
            )
            .unwrap();

        // Run election
        leadership
            .initiate_election(
                org_id,
                vec!["alice".to_string(), "bob".to_string()],
                VotingMethod::SimpleMajority,
                10,
            )
            .unwrap();
        leadership
            .cast_vote(org_id, "v1".to_string(), vec!["alice".to_string()])
            .unwrap();
        leadership
            .cast_vote(org_id, "v2".to_string(), vec!["alice".to_string()])
            .unwrap();
        leadership
            .cast_vote(org_id, "v3".to_string(), vec!["bob".to_string()])
            .unwrap();
        leadership.resolve_election(org_id).unwrap();

        engine.resolve_election(&leadership, org_id).unwrap();

        let record = engine.get_cycle(org_id).unwrap();
        assert_eq!(record.status, CycleStatus::CollectingProposals);
        assert_eq!(record.leader_id, Some("alice".to_string()));
    }

    // ── Phase 2: Collect Proposals ─────────────────────────

    #[test]
    fn test_submit_candidate_rule() {
        let (mut engine, _, _) = setup_engines();
        let org_id = Uuid::new_v4();

        engine
            .start_cycle_with_leader(org_id, "alice".to_string(), 10, "test")
            .unwrap();

        let rule = make_candidate_rule("alice", "Wealth Tax");
        engine.submit_candidate_rule(org_id, rule).unwrap();

        let rules = engine.get_candidate_rules(org_id).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].title, "Wealth Tax");
    }

    // ── Phase 3: Start Voting ──────────────────────────────

    #[test]
    fn test_start_voting_phase() {
        let (mut engine, mut governance, _) = setup_engines();

        // Setup org with members
        governance
            .create_org("Test Org".to_string(), "founder".to_string(), DecisionMode::Vote, 0)
            .unwrap();
        // Update the org ID to match our test org_id
        // We'll just use the returned org_id
        let gov_org_id = *governance.organizations.keys().next().unwrap();
        governance.join_org(gov_org_id, "leader".to_string(), 1).unwrap();
        governance.join_org(gov_org_id, "voter1".to_string(), 1).unwrap();

        engine
            .start_cycle_with_leader(gov_org_id, "leader".to_string(), 10, "test")
            .unwrap();

        let rule = make_candidate_rule("leader", "Tax Rule");
        engine.submit_candidate_rule(gov_org_id, rule).unwrap();

        let proposal_id = engine
            .start_voting_phase(&mut governance, gov_org_id, 15)
            .unwrap();

        let record = engine.get_cycle(gov_org_id).unwrap();
        assert_eq!(record.status, CycleStatus::VotingOpen);
        assert_eq!(record.governance_proposal_id, Some(proposal_id));
    }

    // ── Phase 5: Tally and Enact ───────────────────────────

    #[test]
    fn test_tally_and_enact_success() {
        let (mut engine, mut governance, _) = setup_engines();
        let gov_org_id = governance
            .create_org(
                "Test Org".to_string(),
                "founder".to_string(),
                DecisionMode::Vote,
                0,
            )
            .unwrap();
        governance.join_org(gov_org_id, "leader".to_string(), 1).unwrap();
        governance.join_org(gov_org_id, "voter1".to_string(), 1).unwrap();
        governance.join_org(gov_org_id, "voter2".to_string(), 1).unwrap();

        engine
            .start_cycle_with_leader(gov_org_id, "leader".to_string(), 10, "test")
            .unwrap();

        let rule = make_candidate_rule("leader", "Tax Rule");
        engine.submit_candidate_rule(gov_org_id, rule).unwrap();

        engine
            .start_voting_phase(&mut governance, gov_org_id, 15)
            .unwrap();

        // Cast votes (founder=3, leader=2, voter1=1, voter2=1; need majority)
        engine
            .cast_vote(&mut governance, gov_org_id, "founder".to_string(), true, 16)
            .unwrap();
        engine
            .cast_vote(&mut governance, gov_org_id, "leader".to_string(), true, 16)
            .unwrap();

        let enacted = engine
            .tally_and_enact(&mut governance, gov_org_id, 20)
            .unwrap();

        assert!(!enacted.is_empty());
        let record = engine.get_cycle(gov_org_id).unwrap();
        assert_eq!(record.status, CycleStatus::Enacted);
        assert_eq!(record.enacted_rule_ids.len(), enacted.len());
        assert_eq!(governance.active_rules.active_rule_count(), 1);
    }

    #[test]
    fn test_tally_and_enact_rejected() {
        let (mut engine, mut governance, _) = setup_engines();
        let gov_org_id = governance
            .create_org(
                "Test Org".to_string(),
                "founder".to_string(),
                DecisionMode::Vote,
                0,
            )
            .unwrap();
        governance.join_org(gov_org_id, "leader".to_string(), 1).unwrap();
        governance.join_org(gov_org_id, "voter1".to_string(), 1).unwrap();

        engine
            .start_cycle_with_leader(gov_org_id, "leader".to_string(), 10, "test")
            .unwrap();

        let rule = make_candidate_rule("leader", "Bad Rule");
        engine.submit_candidate_rule(gov_org_id, rule).unwrap();

        engine
            .start_voting_phase(&mut governance, gov_org_id, 15)
            .unwrap();

        // Vote against (founder weight=3, leader weight=2 → founder alone can reject)
        engine
            .cast_vote(&mut governance, gov_org_id, "founder".to_string(), false, 16)
            .unwrap();
        engine
            .cast_vote(&mut governance, gov_org_id, "leader".to_string(), true, 16)
            .unwrap();

        let enacted = engine
            .tally_and_enact(&mut governance, gov_org_id, 20)
            .unwrap();

        assert!(enacted.is_empty());
        let record = engine.get_cycle(gov_org_id).unwrap();
        assert_eq!(record.status, CycleStatus::Rejected);
    }

    // ── Full Cycle ─────────────────────────────────────────

    #[test]
    fn test_run_full_cycle() {
        let (mut engine, mut governance, mut leadership) = setup_engines();
        let gov_org_id = governance
            .create_org(
                "Full Cycle Org".to_string(),
                "founder".to_string(),
                DecisionMode::Vote,
                0,
            )
            .unwrap();
        governance.join_org(gov_org_id, "member1".to_string(), 1).unwrap();
        governance.join_org(gov_org_id, "member2".to_string(), 1).unwrap();

        let rules = vec![
            make_candidate_rule("leader", "Rule 1"),
            make_candidate_rule("leader", "Rule 2"),
        ];

        let member_votes = vec![
            ("founder".to_string(), true),
            ("member1".to_string(), true),
            ("member2".to_string(), true),
        ];

        let (cycle_id, enacted) = engine
            .run_full_cycle(
                &mut governance,
                &mut leadership,
                gov_org_id,
                vec!["founder".to_string(), "member1".to_string(), "member2".to_string()],
                &member_votes,
                rules,
                10,
                "governance review",
            )
            .unwrap();

        assert!(!cycle_id.is_nil());
        assert_eq!(enacted.len(), 2);
        assert_eq!(governance.active_rules.active_rule_count(), 2);

        let record = engine.get_cycle(gov_org_id).unwrap();
        assert_eq!(record.status, CycleStatus::Enacted);
        assert!(record.leader_id.is_some());
    }

    // ── Feedback ───────────────────────────────────────────

    #[test]
    fn test_evaluate_cycle_effects() {
        let (mut engine, mut governance, mut leadership) = setup_engines();
        let gov_org_id = governance
            .create_org(
                "Effect Org".to_string(),
                "founder".to_string(),
                DecisionMode::Vote,
                0,
            )
            .unwrap();
        governance.join_org(gov_org_id, "member1".to_string(), 1).unwrap();

        let rules = vec![make_candidate_rule("leader", "Test Rule")];
        let member_votes = vec![("founder".to_string(), true)];

        let (_, enacted) = engine
            .run_full_cycle(
                &mut governance,
                &mut leadership,
                gov_org_id,
                vec!["founder".to_string(), "member1".to_string()],
                &member_votes,
                rules,
                10,
                "test",
            )
            .unwrap();

        assert_eq!(enacted.len(), 1);

        // Evaluate effects
        let summary = engine.evaluate_cycle_effects(&governance.active_rules, gov_org_id);
        assert_eq!(summary.total_enacted, 1);
        assert_eq!(summary.still_active, 1);
        assert_eq!(summary.expired, 0);
    }

    // ── Query ──────────────────────────────────────────────

    #[test]
    fn test_active_and_completed_cycles() {
        let (mut engine, _, _) = setup_engines();
        let org1 = Uuid::new_v4();
        let org2 = Uuid::new_v4();

        engine
            .start_cycle(org1, vec!["a".to_string()], 10, "test1")
            .unwrap();
        engine
            .start_cycle_with_leader(org2, "leader".to_string(), 10, "test2")
            .unwrap();

        assert_eq!(engine.active_cycles().len(), 2);
        assert!(engine.completed_cycles().is_empty());
    }

    #[test]
    fn test_no_candidate_rules_fails_voting() {
        let (mut engine, mut governance, _) = setup_engines();
        let gov_org_id = governance
            .create_org(
                "Test".to_string(),
                "founder".to_string(),
                DecisionMode::Vote,
                0,
            )
            .unwrap();
        governance.join_org(gov_org_id, "leader".to_string(), 1).unwrap();

        engine
            .start_cycle_with_leader(gov_org_id, "leader".to_string(), 10, "test")
            .unwrap();

        // No rules submitted — should fail
        let result = engine.start_voting_phase(&mut governance, gov_org_id, 15);
        assert!(result.is_err());
    }

    // ── Auto-Trigger Tests ─────────────────────────────────

    #[test]
    fn test_auto_trigger_time_based() {
        let (engine, _, _) = setup_engines();
        let org_id = Uuid::new_v4();
        assert!(engine.should_auto_trigger(org_id, 50, 5).is_none());
        let reason = engine.should_auto_trigger(org_id, 100, 5);
        assert!(reason.is_some());
        assert!(reason.unwrap().contains("first cycle"));
    }

    #[test]
    fn test_auto_trigger_minimum_members() {
        let (engine, _, _) = setup_engines();
        let org_id = Uuid::new_v4();
        assert!(engine.should_auto_trigger(org_id, 100, 2).is_none());
        assert!(engine.should_auto_trigger(org_id, 100, 3).is_some());
    }

    #[test]
    fn test_auto_trigger_after_completed_cycle() {
        let (mut engine, mut governance, _) = setup_engines();
        let gov_org_id = governance.create_org("Test".to_string(), "founder".to_string(), DecisionMode::Vote, 0).unwrap();
        governance.join_org(gov_org_id, "leader".to_string(), 1).unwrap();
        governance.join_org(gov_org_id, "voter1".to_string(), 1).unwrap();
        engine.start_cycle_with_leader(gov_org_id, "leader".to_string(), 10, "test").unwrap();
        engine.submit_candidate_rule(gov_org_id, make_candidate_rule("leader", "Rule 1")).unwrap();
        engine.start_voting_phase(&mut governance, gov_org_id, 15).unwrap();
        engine.cast_vote(&mut governance, gov_org_id, "founder".to_string(), true, 16).unwrap();
        engine.cast_vote(&mut governance, gov_org_id, "leader".to_string(), true, 16).unwrap();
        engine.tally_and_enact(&mut governance, gov_org_id, 20).unwrap();
        assert!(engine.should_auto_trigger(gov_org_id, 119, 5).is_none());
        let reason = engine.should_auto_trigger(gov_org_id, 120, 5);
        assert!(reason.is_some());
        assert!(reason.unwrap().contains("ticks since last cycle"));
    }

    #[test]
    fn test_auto_trigger_no_double_trigger() {
        let (mut engine, _, _) = setup_engines();
        let org_id = Uuid::new_v4();
        engine.start_cycle(org_id, vec!["a".to_string()], 100, "test").unwrap();
        assert!(engine.should_auto_trigger(org_id, 200, 5).is_none());
    }

    #[test]
    fn test_tick_auto_trigger_batch() {
        let (mut engine, _, _) = setup_engines();
        let org1 = Uuid::new_v4();
        let org2 = Uuid::new_v4();
        let triggered = engine.tick_auto_trigger(100, &[(org1, 5), (org2, 2)]);
        assert_eq!(triggered.len(), 1);
        assert_eq!(triggered[0].0, org1);
    }

    #[test]
    fn test_event_based_trigger() {
        let (mut engine, _, _) = setup_engines();
        let org_id = Uuid::new_v4();
        let cycle_id = engine.trigger_from_event(org_id, "crisis: treasury depletion", 50, vec!["a".to_string()]).unwrap();
        let record = engine.get_cycle(org_id).unwrap();
        assert_eq!(record.cycle_id, cycle_id);
        assert!(record.trigger_reason.contains("event-trigger"));
    }

    #[test]
    fn test_submit_repeal_proposal() {
        let (mut engine, _, _) = setup_engines();
        let org_id = Uuid::new_v4();
        engine.start_cycle_with_leader(org_id, "leader".to_string(), 10, "repeal test").unwrap();
        engine.submit_repeal_proposal(org_id, "leader".to_string(), "rule-123".to_string(), "economic harm".to_string()).unwrap();
        let rules = engine.get_candidate_rules(org_id).unwrap();
        assert_eq!(rules.len(), 1);
        assert!(rules[0].title.contains("rule-123"));
    }

    #[test]
    fn test_process_repeal_effects() {
        let (mut engine, _, _) = setup_engines();
        let org_id = Uuid::new_v4();
        let mut rule_engine = RuleEngine::new();
        let rule_id = rule_engine.propose_rule("proposer".to_string(), org_id.to_string(),
            "Test Rule".to_string(), "A rule to repeal".to_string(), RuleType::Tax, vec![], vec![], 0, None);
        rule_engine.activate_rule(&rule_id).unwrap();
        assert_eq!(rule_engine.active_rule_count(), 1);
        engine.start_cycle_with_leader(org_id, "leader".to_string(), 10, "repeal").unwrap();
        engine.submit_repeal_proposal(org_id, "leader".to_string(), rule_id.clone(), "outdated".to_string()).unwrap();
        let repealed = engine.process_repeal_effects(&mut rule_engine, org_id, 20);
        assert_eq!(repealed.len(), 1);
        assert_eq!(repealed[0], rule_id);
        assert_eq!(rule_engine.active_rule_count(), 0);
    }

    #[test]
    fn test_auto_trigger_and_elect_pipeline() {
        let (mut engine, mut governance, mut leadership) = setup_engines();
        let gov_org_id = governance.create_org("Auto Org".to_string(), "founder".to_string(), DecisionMode::Vote, 0).unwrap();
        governance.join_org(gov_org_id, "member1".to_string(), 1).unwrap();
        governance.join_org(gov_org_id, "member2".to_string(), 1).unwrap();
        let result = engine.auto_trigger_and_elect(&mut governance, &mut leadership, gov_org_id, 100, 5,
            vec!["founder".to_string(), "member1".to_string(), "member2".to_string()]);
        assert!(result.is_some());
        let (cycle_id, leader_id) = result.unwrap();
        assert!(!cycle_id.is_nil());
        assert!(!leader_id.is_empty());
        let record = engine.get_cycle(gov_org_id).unwrap();
        assert_eq!(record.status, CycleStatus::CollectingProposals);
        assert!(record.leader_id.is_some());
    }
}