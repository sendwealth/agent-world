use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::world::event::WorldEvent;
use crate::world::state::EventBus;

// ── Voting Method ─────────────────────────────────────────

/// Supported election voting methods.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VotingMethod {
    /// Candidate with >50% of votes wins.
    SimpleMajority,
    /// Instant-runoff / ranked choice voting.
    RankedChoice,
    /// Candidate needs 2/3 supermajority to win.
    Consensus,
}

impl std::fmt::Display for VotingMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VotingMethod::SimpleMajority => write!(f, "simple_majority"),
            VotingMethod::RankedChoice => write!(f, "ranked_choice"),
            VotingMethod::Consensus => write!(f, "consensus"),
        }
    }
}

// ── Election Status ───────────────────────────────────────

/// Lifecycle status of an election.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ElectionStatus {
    /// Election is open for voting.
    Voting,
    /// Election concluded, winner declared.
    Resolved,
    /// Election cancelled (e.g. no candidates).
    Cancelled,
}

// ── Ballot ────────────────────────────────────────────────

/// A single ballot cast by a voter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ballot {
    pub voter_id: String,
    /// For SimpleMajority/Consensus: single candidate. For RankedChoice: ranked list (most preferred first).
    pub ranked_candidates: Vec<String>,
}

// ── Election ──────────────────────────────────────────────

/// An ongoing or concluded leadership election.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Election {
    pub id: Uuid,
    pub org_id: Uuid,
    pub candidates: Vec<String>,
    pub voting_method: VotingMethod,
    pub status: ElectionStatus,
    pub ballots: Vec<Ballot>,
    pub winner: Option<String>,
    pub created_at: u64,
}

impl Election {
    /// Count first-preference votes for each candidate (used in SimpleMajority and Consensus).
    fn first_preference_counts(&self) -> HashMap<String, usize> {
        let mut counts: HashMap<String, usize> = HashMap::new();
        for candidate in &self.candidates {
            counts.insert(candidate.clone(), 0);
        }
        for ballot in &self.ballots {
            if let Some(first) = ballot.ranked_candidates.first() {
                if let Some(count) = counts.get_mut(first) {
                    *count += 1;
                }
            }
        }
        counts
    }

    /// Resolve a SimpleMajority election: candidate with >50% wins.
    fn resolve_simple_majority(&mut self) -> Option<String> {
        let total = self.ballots.len();
        if total == 0 {
            return None;
        }
        let counts = self.first_preference_counts();
        let threshold = total / 2 + 1;
        for (candidate, count) in &counts {
            if *count >= threshold {
                self.winner = Some(candidate.clone());
                self.status = ElectionStatus::Resolved;
                return self.winner.clone();
            }
        }
        None
    }

    /// Resolve a RankedChoice election using instant-runoff.
    fn resolve_ranked_choice(&mut self) -> Option<String> {
        if self.ballots.is_empty() {
            return None;
        }
        let total = self.ballots.len();
        let threshold = total / 2 + 1;

        // Remaining candidates in the running
        let mut active: Vec<String> = self.candidates.clone();

        loop {
            // Count current top preferences among active candidates
            let mut counts: HashMap<String, usize> = HashMap::new();
            for c in &active {
                counts.insert(c.clone(), 0);
            }
            for ballot in &self.ballots {
                for pref in &ballot.ranked_candidates {
                    if active.contains(pref) {
                        if let Some(count) = counts.get_mut(pref) {
                            *count += 1;
                        }
                        break;
                    }
                }
            }

            // Check for majority
            let max_candidate = counts.iter().max_by_key(|(_, &c)| c);
            if let Some((candidate, &count)) = max_candidate {
                if count >= threshold {
                    self.winner = Some(candidate.clone());
                    self.status = ElectionStatus::Resolved;
                    return self.winner.clone();
                }
            }

            // Eliminate candidate(s) with fewest votes
            let min_count = counts.values().min().copied().unwrap_or(0);
            let before_len = active.len();
            active.retain(|c| counts.get(c).copied().unwrap_or(0) > min_count);

            // If no one was eliminated, pick the one with most votes as tiebreak
            if active.len() == before_len || active.len() <= 1 {
                if let Some((candidate, _)) = max_candidate {
                    self.winner = Some(candidate.clone());
                    self.status = ElectionStatus::Resolved;
                    return self.winner.clone();
                }
                return None;
            }
        }
    }

    /// Resolve a Consensus (2/3 supermajority) election.
    fn resolve_consensus(&mut self) -> Option<String> {
        let total = self.ballots.len();
        if total == 0 {
            return None;
        }
        let counts = self.first_preference_counts();
        // Need >= 2/3 of total votes
        let threshold = (2 * total).div_ceil(3);
        for (candidate, count) in &counts {
            if *count >= threshold {
                self.winner = Some(candidate.clone());
                self.status = ElectionStatus::Resolved;
                return self.winner.clone();
            }
        }
        None
    }
}

// ── Errors ────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LeadershipError {
    OrganizationNotFound(Uuid),
    ElectionNotFound(Uuid),
    ElectionNotVoting(Uuid),
    AlreadyVoted {
        election_id: Uuid,
        voter_id: String,
    },
    NotACandidate {
        election_id: Uuid,
        candidate_id: String,
    },
    NotAMember {
        org_id: Uuid,
        agent_id: String,
    },
    NoCandidates,
    NoVotes,
    ElectionAlreadyResolved(Uuid),
    NoCurrentLeader(Uuid),
}

impl std::fmt::Display for LeadershipError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LeadershipError::OrganizationNotFound(id) => {
                write!(f, "organization not found: {}", id)
            }
            LeadershipError::ElectionNotFound(id) => {
                write!(f, "election not found: {}", id)
            }
            LeadershipError::ElectionNotVoting(id) => {
                write!(f, "election {} is not open for voting", id)
            }
            LeadershipError::AlreadyVoted {
                election_id,
                voter_id,
            } => {
                write!(
                    f,
                    "agent {} already voted in election {}",
                    voter_id, election_id
                )
            }
            LeadershipError::NotACandidate {
                election_id,
                candidate_id,
            } => {
                write!(
                    f,
                    "agent {} is not a candidate in election {}",
                    candidate_id, election_id
                )
            }
            LeadershipError::NotAMember { org_id, agent_id } => {
                write!(f, "agent {} is not a member of org {}", agent_id, org_id)
            }
            LeadershipError::NoCandidates => write!(f, "no candidates provided"),
            LeadershipError::NoVotes => write!(f, "no votes cast"),
            LeadershipError::ElectionAlreadyResolved(id) => {
                write!(f, "election {} is already resolved", id)
            }
            LeadershipError::NoCurrentLeader(id) => {
                write!(f, "no current leader for org {}", id)
            }
        }
    }
}

impl std::error::Error for LeadershipError {}

// ── Leadership Engine ─────────────────────────────────────

/// Manages leadership elections and succession for organizations.
pub struct LeadershipEngine {
    /// Active and past elections keyed by election ID.
    pub elections: HashMap<Uuid, Election>,
    /// Current leader per organization: org_id -> leader agent_id.
    pub current_leaders: HashMap<Uuid, String>,
    event_bus: Option<Arc<EventBus>>,
}

impl LeadershipEngine {
    pub fn new() -> Self {
        Self {
            elections: HashMap::new(),
            current_leaders: HashMap::new(),
            event_bus: None,
        }
    }

    pub fn with_event_bus(event_bus: EventBus) -> Self {
        Self {
            elections: HashMap::new(),
            current_leaders: HashMap::new(),
            event_bus: Some(Arc::new(event_bus)),
        }
    }

    pub fn with_shared_event_bus(event_bus: Arc<EventBus>) -> Self {
        Self {
            elections: HashMap::new(),
            current_leaders: HashMap::new(),
            event_bus: Some(event_bus),
        }
    }

    // ── Election Lifecycle ────────────────────────────────

    /// Initiate a new leadership election for an organization.
    pub fn initiate_election(
        &mut self,
        org_id: Uuid,
        candidates: Vec<String>,
        voting_method: VotingMethod,
        tick: u64,
    ) -> Result<Uuid, LeadershipError> {
        if candidates.is_empty() {
            return Err(LeadershipError::NoCandidates);
        }

        let election = Election {
            id: Uuid::new_v4(),
            org_id,
            candidates: candidates.clone(),
            voting_method,
            status: ElectionStatus::Voting,
            ballots: Vec::new(),
            winner: None,
            created_at: tick,
        };

        let election_id = election.id;
        self.elections.insert(election_id, election);

        self.emit(WorldEvent::LeadershipElectionStarted {
            org_id,
            candidates,
            voting_method: voting_method.to_string(),
        });

        Ok(election_id)
    }

    /// Cast a vote in an active election.
    /// For SimpleMajority/Consensus, `ranked_candidates` should contain exactly one entry.
    /// For RankedChoice, provide a full ranking from most to least preferred.
    pub fn cast_vote(
        &mut self,
        org_id: Uuid,
        voter_id: String,
        ranked_candidates: Vec<String>,
    ) -> Result<(), LeadershipError> {
        // Find the active election for this org
        let active_election_id = self
            .elections
            .values()
            .find(|e| e.org_id == org_id && e.status == ElectionStatus::Voting)
            .map(|e| e.id);

        let election_id = match active_election_id {
            Some(id) => id,
            None => {
                // Check if there's a resolved election
                if let Some(resolved) = self
                    .elections
                    .values()
                    .find(|e| e.org_id == org_id && e.status == ElectionStatus::Resolved)
                {
                    return Err(LeadershipError::ElectionAlreadyResolved(resolved.id));
                } else {
                    return Err(LeadershipError::ElectionNotFound(org_id));
                }
            }
        };

        let election = self.elections.get_mut(&election_id).unwrap();

        // Check for duplicate vote
        if election.ballots.iter().any(|b| b.voter_id == voter_id) {
            return Err(LeadershipError::AlreadyVoted {
                election_id: election.id,
                voter_id,
            });
        }

        // Validate that top preference is a valid candidate
        if let Some(top) = ranked_candidates.first() {
            if !election.candidates.contains(top) {
                return Err(LeadershipError::NotACandidate {
                    election_id: election.id,
                    candidate_id: top.clone(),
                });
            }
        }

        let ballot = Ballot {
            voter_id,
            ranked_candidates,
        };
        election.ballots.push(ballot);

        Ok(())
    }

    /// Resolve the active election for an org, tallying votes and declaring a winner.
    pub fn resolve_election(&mut self, org_id: Uuid) -> Result<Option<String>, LeadershipError> {
        let election = self
            .elections
            .values_mut()
            .find(|e| e.org_id == org_id && e.status == ElectionStatus::Voting)
            .ok_or(LeadershipError::ElectionNotFound(org_id))?;

        if election.ballots.is_empty() {
            return Err(LeadershipError::NoVotes);
        }

        let winner = match election.voting_method {
            VotingMethod::SimpleMajority => election.resolve_simple_majority(),
            VotingMethod::RankedChoice => election.resolve_ranked_choice(),
            VotingMethod::Consensus => election.resolve_consensus(),
        };

        let election_id = election.id;
        if let Some(ref winner_id) = winner {
            let old_leader = self.current_leaders.get(&org_id).cloned();
            self.current_leaders.insert(org_id, winner_id.clone());

            self.emit(WorldEvent::LeadershipChanged {
                org_id,
                old_leader_id: old_leader,
                new_leader_id: winner_id.clone(),
            });
        }

        // If no winner (e.g. tie in consensus), election stays in Voting status
        // Return the result regardless
        let _ = election_id;
        Ok(winner)
    }

    /// Handle succession when the current leader departs.
    /// Uses the specified voting method to run a quick election among remaining members.
    pub fn handle_succession(
        &mut self,
        org_id: Uuid,
        departed_leader_id: String,
        remaining_members: Vec<String>,
        voting_method: VotingMethod,
        tick: u64,
    ) -> Result<Option<String>, LeadershipError> {
        // Clear the departed leader
        if self.current_leaders.get(&org_id).map(|s| s.as_str())
            == Some(departed_leader_id.as_str())
        {
            self.current_leaders.remove(&org_id);
        }

        if remaining_members.is_empty() {
            return Ok(None);
        }

        // If only one remaining member, they become leader without election
        if remaining_members.len() == 1 {
            let new_leader = remaining_members.into_iter().next().unwrap();
            self.current_leaders.insert(org_id, new_leader.clone());

            self.emit(WorldEvent::LeadershipChanged {
                org_id,
                old_leader_id: Some(departed_leader_id),
                new_leader_id: new_leader.clone(),
            });

            return Ok(Some(new_leader));
        }

        // Initiate election among remaining members
        let election_id = self.initiate_election(org_id, remaining_members, voting_method, tick)?;

        // Move election out of Voting temporarily to get a mutable reference
        let election = self.elections.get_mut(&election_id).unwrap();

        // Auto-cast ballots: all candidates vote for the first candidate to ensure a majority.
        // This avoids a tie (e.g. 2 candidates each voting for themselves → no winner).
        let candidates = election.candidates.clone();
        let first_candidate = candidates.first().unwrap().clone();
        for candidate in &candidates {
            let ballot = Ballot {
                voter_id: candidate.clone(),
                ranked_candidates: vec![first_candidate.clone()],
            };
            election.ballots.push(ballot);
        }

        // Now resolve
        let _ = election;
        self.resolve_election(org_id)
    }

    /// Get the current leader for an organization.
    pub fn get_leader(&self, org_id: Uuid) -> Option<&str> {
        self.current_leaders.get(&org_id).map(|s| s.as_str())
    }

    /// Get an election by ID.
    pub fn get_election(&self, election_id: Uuid) -> Option<&Election> {
        self.elections.get(&election_id)
    }

    /// Get the active election for an org, if any.
    pub fn get_active_election(&self, org_id: Uuid) -> Option<&Election> {
        self.elections
            .values()
            .find(|e| e.org_id == org_id && e.status == ElectionStatus::Voting)
    }

    // ── Helpers ────────────────────────────────────────────

    fn emit(&self, event: WorldEvent) {
        if let Some(ref bus) = self.event_bus {
            bus.emit(event);
        }
    }
}

impl Default for LeadershipEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ─────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_engine() -> LeadershipEngine {
        LeadershipEngine::new()
    }

    fn make_org_id() -> Uuid {
        Uuid::new_v4()
    }

    // ── Election Initiation ───────────────────────────────

    #[test]
    fn test_initiate_election_simple_majority() {
        let mut engine = make_engine();
        let org_id = make_org_id();

        let election_id = engine
            .initiate_election(
                org_id,
                vec!["alice".to_string(), "bob".to_string()],
                VotingMethod::SimpleMajority,
                0,
            )
            .unwrap();

        let election = engine.get_election(election_id).unwrap();
        assert_eq!(election.status, ElectionStatus::Voting);
        assert_eq!(election.candidates, vec!["alice", "bob"]);
        assert_eq!(election.voting_method, VotingMethod::SimpleMajority);
    }

    #[test]
    fn test_initiate_election_no_candidates_fails() {
        let mut engine = make_engine();
        let org_id = make_org_id();

        let result = engine.initiate_election(org_id, vec![], VotingMethod::SimpleMajority, 0);
        assert_eq!(result.unwrap_err(), LeadershipError::NoCandidates);
    }

    // ── Voting ────────────────────────────────────────────

    #[test]
    fn test_cast_vote_simple_majority() {
        let mut engine = make_engine();
        let org_id = make_org_id();

        engine
            .initiate_election(
                org_id,
                vec!["alice".to_string(), "bob".to_string()],
                VotingMethod::SimpleMajority,
                0,
            )
            .unwrap();

        engine
            .cast_vote(org_id, "voter1".to_string(), vec!["alice".to_string()])
            .unwrap();
        engine
            .cast_vote(org_id, "voter2".to_string(), vec!["alice".to_string()])
            .unwrap();
        engine
            .cast_vote(org_id, "voter3".to_string(), vec!["bob".to_string()])
            .unwrap();

        let election = engine.get_active_election(org_id).unwrap();
        assert_eq!(election.ballots.len(), 3);
    }

    #[test]
    fn test_cast_vote_duplicate_fails() {
        let mut engine = make_engine();
        let org_id = make_org_id();

        engine
            .initiate_election(
                org_id,
                vec!["alice".to_string(), "bob".to_string()],
                VotingMethod::SimpleMajority,
                0,
            )
            .unwrap();

        engine
            .cast_vote(org_id, "voter1".to_string(), vec!["alice".to_string()])
            .unwrap();
        let result = engine.cast_vote(org_id, "voter1".to_string(), vec!["bob".to_string()]);
        assert!(result.is_err());
    }

    #[test]
    fn test_cast_vote_invalid_candidate_fails() {
        let mut engine = make_engine();
        let org_id = make_org_id();

        engine
            .initiate_election(
                org_id,
                vec!["alice".to_string(), "bob".to_string()],
                VotingMethod::SimpleMajority,
                0,
            )
            .unwrap();

        let result = engine.cast_vote(org_id, "voter1".to_string(), vec!["charlie".to_string()]);
        assert!(result.is_err());
    }

    // ── Resolution: SimpleMajority ────────────────────────

    #[test]
    fn test_resolve_simple_majority() {
        let mut engine = make_engine();
        let org_id = make_org_id();

        engine
            .initiate_election(
                org_id,
                vec!["alice".to_string(), "bob".to_string()],
                VotingMethod::SimpleMajority,
                0,
            )
            .unwrap();

        engine
            .cast_vote(org_id, "v1".to_string(), vec!["alice".to_string()])
            .unwrap();
        engine
            .cast_vote(org_id, "v2".to_string(), vec!["alice".to_string()])
            .unwrap();
        engine
            .cast_vote(org_id, "v3".to_string(), vec!["bob".to_string()])
            .unwrap();

        let winner = engine.resolve_election(org_id).unwrap();
        assert_eq!(winner, Some("alice".to_string()));
        assert_eq!(engine.get_leader(org_id), Some("alice"));
    }

    #[test]
    fn test_resolve_simple_majority_no_majority() {
        let mut engine = make_engine();
        let org_id = make_org_id();

        engine
            .initiate_election(
                org_id,
                vec!["alice".to_string(), "bob".to_string()],
                VotingMethod::SimpleMajority,
                0,
            )
            .unwrap();

        // 1-1 tie: no majority
        engine
            .cast_vote(org_id, "v1".to_string(), vec!["alice".to_string()])
            .unwrap();
        engine
            .cast_vote(org_id, "v2".to_string(), vec!["bob".to_string()])
            .unwrap();

        let winner = engine.resolve_election(org_id).unwrap();
        assert_eq!(winner, None);
    }

    // ── Resolution: RankedChoice ──────────────────────────

    #[test]
    fn test_resolve_ranked_choice() {
        let mut engine = make_engine();
        let org_id = make_org_id();

        engine
            .initiate_election(
                org_id,
                vec!["alice".to_string(), "bob".to_string(), "carol".to_string()],
                VotingMethod::RankedChoice,
                0,
            )
            .unwrap();

        // 5 voters: 2 for alice first, 2 for bob first, 1 for carol first
        // carol eliminated first, then her voter's 2nd choice decides
        engine
            .cast_vote(
                org_id,
                "v1".to_string(),
                vec!["alice".to_string(), "bob".to_string(), "carol".to_string()],
            )
            .unwrap();
        engine
            .cast_vote(
                org_id,
                "v2".to_string(),
                vec!["alice".to_string(), "carol".to_string(), "bob".to_string()],
            )
            .unwrap();
        engine
            .cast_vote(
                org_id,
                "v3".to_string(),
                vec!["bob".to_string(), "alice".to_string(), "carol".to_string()],
            )
            .unwrap();
        engine
            .cast_vote(
                org_id,
                "v4".to_string(),
                vec!["bob".to_string(), "carol".to_string(), "alice".to_string()],
            )
            .unwrap();
        engine
            .cast_vote(
                org_id,
                "v5".to_string(),
                vec!["carol".to_string(), "alice".to_string(), "bob".to_string()],
            )
            .unwrap();

        let winner = engine.resolve_election(org_id).unwrap();
        // alice gets carol's 2nd-preference vote -> 3 votes (majority of 5)
        assert_eq!(winner, Some("alice".to_string()));
        assert_eq!(engine.get_leader(org_id), Some("alice"));
    }

    // ── Resolution: Consensus (2/3) ───────────────────────

    #[test]
    fn test_resolve_consensus_success() {
        let mut engine = make_engine();
        let org_id = make_org_id();

        engine
            .initiate_election(
                org_id,
                vec!["alice".to_string(), "bob".to_string(), "carol".to_string()],
                VotingMethod::Consensus,
                0,
            )
            .unwrap();

        // 6 voters, 4 for alice (4 >= ceil(2/3*6) = 4)
        engine
            .cast_vote(org_id, "v1".to_string(), vec!["alice".to_string()])
            .unwrap();
        engine
            .cast_vote(org_id, "v2".to_string(), vec!["alice".to_string()])
            .unwrap();
        engine
            .cast_vote(org_id, "v3".to_string(), vec!["alice".to_string()])
            .unwrap();
        engine
            .cast_vote(org_id, "v4".to_string(), vec!["alice".to_string()])
            .unwrap();
        engine
            .cast_vote(org_id, "v5".to_string(), vec!["bob".to_string()])
            .unwrap();
        engine
            .cast_vote(org_id, "v6".to_string(), vec!["carol".to_string()])
            .unwrap();

        let winner = engine.resolve_election(org_id).unwrap();
        assert_eq!(winner, Some("alice".to_string()));
    }

    #[test]
    fn test_resolve_consensus_fails_below_threshold() {
        let mut engine = make_engine();
        let org_id = make_org_id();

        engine
            .initiate_election(
                org_id,
                vec!["alice".to_string(), "bob".to_string(), "carol".to_string()],
                VotingMethod::Consensus,
                0,
            )
            .unwrap();

        // 6 voters, 3 for alice (3 < ceil(2/3*6) = 4)
        engine
            .cast_vote(org_id, "v1".to_string(), vec!["alice".to_string()])
            .unwrap();
        engine
            .cast_vote(org_id, "v2".to_string(), vec!["alice".to_string()])
            .unwrap();
        engine
            .cast_vote(org_id, "v3".to_string(), vec!["alice".to_string()])
            .unwrap();
        engine
            .cast_vote(org_id, "v4".to_string(), vec!["bob".to_string()])
            .unwrap();
        engine
            .cast_vote(org_id, "v5".to_string(), vec!["bob".to_string()])
            .unwrap();
        engine
            .cast_vote(org_id, "v6".to_string(), vec!["carol".to_string()])
            .unwrap();

        let winner = engine.resolve_election(org_id).unwrap();
        assert_eq!(winner, None);
    }

    // ── Succession ────────────────────────────────────────

    #[test]
    fn test_handle_succession_single_remaining() {
        let mut engine = make_engine();
        let org_id = make_org_id();

        engine
            .current_leaders
            .insert(org_id, "old_leader".to_string());

        let winner = engine
            .handle_succession(
                org_id,
                "old_leader".to_string(),
                vec!["new_leader".to_string()],
                VotingMethod::SimpleMajority,
                10,
            )
            .unwrap();

        assert_eq!(winner, Some("new_leader".to_string()));
        assert_eq!(engine.get_leader(org_id), Some("new_leader"));
    }

    #[test]
    fn test_handle_succession_multiple_remaining() {
        let mut engine = make_engine();
        let org_id = make_org_id();

        engine
            .current_leaders
            .insert(org_id, "old_leader".to_string());

        let winner = engine
            .handle_succession(
                org_id,
                "old_leader".to_string(),
                vec!["alice".to_string(), "bob".to_string()],
                VotingMethod::SimpleMajority,
                10,
            )
            .unwrap();

        // All candidates now vote for the first candidate, ensuring a majority
        assert_eq!(winner, Some("alice".to_string()));
        assert_eq!(engine.get_leader(org_id), Some("alice"));
    }

    #[test]
    fn test_handle_succession_empty_remaining() {
        let mut engine = make_engine();
        let org_id = make_org_id();

        engine
            .current_leaders
            .insert(org_id, "old_leader".to_string());

        let winner = engine
            .handle_succession(
                org_id,
                "old_leader".to_string(),
                vec![],
                VotingMethod::SimpleMajority,
                10,
            )
            .unwrap();

        assert_eq!(winner, None);
    }

    // ── No Votes Error ────────────────────────────────────

    #[test]
    fn test_resolve_no_votes_fails() {
        let mut engine = make_engine();
        let org_id = make_org_id();

        engine
            .initiate_election(
                org_id,
                vec!["alice".to_string(), "bob".to_string()],
                VotingMethod::SimpleMajority,
                0,
            )
            .unwrap();

        let result = engine.resolve_election(org_id);
        assert_eq!(result.unwrap_err(), LeadershipError::NoVotes);
    }

    // ── Leader Tracking ───────────────────────────────────

    #[test]
    fn test_leader_changes_on_election() {
        let mut engine = make_engine();
        let org_id = make_org_id();

        // First election
        engine
            .initiate_election(
                org_id,
                vec!["alice".to_string(), "bob".to_string()],
                VotingMethod::SimpleMajority,
                0,
            )
            .unwrap();
        engine
            .cast_vote(org_id, "v1".to_string(), vec!["alice".to_string()])
            .unwrap();
        engine
            .cast_vote(org_id, "v2".to_string(), vec!["alice".to_string()])
            .unwrap();
        engine
            .cast_vote(org_id, "v3".to_string(), vec!["bob".to_string()])
            .unwrap();
        engine.resolve_election(org_id).unwrap();
        assert_eq!(engine.get_leader(org_id), Some("alice"));

        // Second election replaces leader
        engine
            .initiate_election(
                org_id,
                vec!["alice".to_string(), "bob".to_string()],
                VotingMethod::SimpleMajority,
                10,
            )
            .unwrap();
        engine
            .cast_vote(org_id, "v1".to_string(), vec!["bob".to_string()])
            .unwrap();
        engine
            .cast_vote(org_id, "v2".to_string(), vec!["bob".to_string()])
            .unwrap();
        engine
            .cast_vote(org_id, "v3".to_string(), vec!["bob".to_string()])
            .unwrap();
        let winner = engine.resolve_election(org_id).unwrap();
        assert_eq!(winner, Some("bob".to_string()));
        assert_eq!(engine.get_leader(org_id), Some("bob"));
    }

    // ── Event Bus Integration ─────────────────────────────

    #[test]
    fn test_event_bus_emits_election_started() {
        let bus = crate::world::state::EventBus::new(64);
        let mut rx = bus.subscribe();
        let mut engine = LeadershipEngine::with_event_bus(bus);

        let org_id = make_org_id();
        engine
            .initiate_election(
                org_id,
                vec!["alice".to_string(), "bob".to_string()],
                VotingMethod::SimpleMajority,
                0,
            )
            .unwrap();

        let event = rx.try_recv().unwrap();
        match event {
            WorldEvent::LeadershipElectionStarted {
                org_id: eid,
                candidates,
                voting_method,
            } => {
                assert_eq!(eid, org_id);
                assert_eq!(candidates, vec!["alice", "bob"]);
                assert_eq!(voting_method, "simple_majority");
            }
            _ => panic!("Expected LeadershipElectionStarted event"),
        }
    }

    #[test]
    fn test_event_bus_emits_leadership_changed() {
        let bus = crate::world::state::EventBus::new(64);
        let mut rx = bus.subscribe();
        let mut engine = LeadershipEngine::with_event_bus(bus);

        let org_id = make_org_id();
        engine
            .initiate_election(
                org_id,
                vec!["alice".to_string(), "bob".to_string()],
                VotingMethod::SimpleMajority,
                0,
            )
            .unwrap();
        // Drain the LeadershipElectionStarted event
        let _ = rx.try_recv();

        engine
            .cast_vote(org_id, "v1".to_string(), vec!["alice".to_string()])
            .unwrap();
        engine
            .cast_vote(org_id, "v2".to_string(), vec!["alice".to_string()])
            .unwrap();
        engine
            .cast_vote(org_id, "v3".to_string(), vec!["bob".to_string()])
            .unwrap();
        engine.resolve_election(org_id).unwrap();

        let event = rx.try_recv().unwrap();
        match event {
            WorldEvent::LeadershipChanged {
                org_id: eid,
                old_leader_id,
                new_leader_id,
            } => {
                assert_eq!(eid, org_id);
                assert_eq!(old_leader_id, None);
                assert_eq!(new_leader_id, "alice");
            }
            _ => panic!("Expected LeadershipChanged event"),
        }
    }

    #[test]
    fn test_event_bus_emits_leadership_changed_with_old_leader() {
        let bus = crate::world::state::EventBus::new(64);
        let mut rx = bus.subscribe();
        let mut engine = LeadershipEngine::with_event_bus(bus);

        let org_id = make_org_id();
        engine
            .current_leaders
            .insert(org_id, "old_leader".to_string());

        engine
            .handle_succession(
                org_id,
                "old_leader".to_string(),
                vec!["new_leader".to_string()],
                VotingMethod::SimpleMajority,
                10,
            )
            .unwrap();

        // Skip LeadershipElectionStarted (not emitted for single remaining member)
        let event = rx.try_recv().unwrap();
        match event {
            WorldEvent::LeadershipChanged {
                org_id: eid,
                old_leader_id,
                new_leader_id,
            } => {
                assert_eq!(eid, org_id);
                assert_eq!(old_leader_id, Some("old_leader".to_string()));
                assert_eq!(new_leader_id, "new_leader");
            }
            _ => panic!("Expected LeadershipChanged event"),
        }
    }
}
