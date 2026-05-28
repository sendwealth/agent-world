use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::world::event::WorldEvent;
use crate::world::state::EventBus;

// ── Relation Levels ─────────────────────────────────────────

/// Relationship level between two organizations.
/// Higher values indicate better relations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RelationLevel(pub i8);

impl RelationLevel {
    pub const HOSTILE: i8 = -3;
    pub const UNFRIENDLY: i8 = -2;
    pub const COLD: i8 = -1;
    pub const NEUTRAL: i8 = 0;
    pub const WARM: i8 = 1;
    pub const FRIENDLY: i8 = 2;
    pub const ALLIED: i8 = 3;

    pub fn is_positive(self) -> bool {
        self.0 > 0
    }

    pub fn is_negative(self) -> bool {
        self.0 < 0
    }
}

impl std::fmt::Display for RelationLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ── Treaty Types ────────────────────────────────────────────

/// Types of treaties that can be negotiated between organizations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TreatyType {
    /// Non-aggression pact.
    NonAggression,
    /// Trade agreement: reduced tariffs.
    TradeAgreement,
    /// Mutual defense: both parties defend each other.
    MutualDefense,
    /// Research sharing: exchange knowledge.
    ResearchSharing,
    /// Full alliance: economic + military cooperation.
    FullAlliance,
}

impl std::fmt::Display for TreatyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TreatyType::NonAggression => write!(f, "non_aggression"),
            TreatyType::TradeAgreement => write!(f, "trade_agreement"),
            TreatyType::MutualDefense => write!(f, "mutual_defense"),
            TreatyType::ResearchSharing => write!(f, "research_sharing"),
            TreatyType::FullAlliance => write!(f, "full_alliance"),
        }
    }
}

// ── Treaty Status ───────────────────────────────────────────

/// Status of a treaty.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TreatyStatus {
    /// Proposed but not yet ratified.
    Proposed,
    /// Signed and active.
    Active,
    /// Broken by one party.
    Broken,
    /// Expired after a duration.
    Expired,
}

// ── Treaty ──────────────────────────────────────────────────

/// A treaty between two organizations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Treaty {
    /// Unique treaty ID.
    pub id: String,
    /// First organization ID.
    pub org_a: String,
    /// Second organization ID.
    pub org_b: String,
    /// Type of treaty.
    pub treaty_type: TreatyType,
    /// Current status.
    pub status: TreatyStatus,
    /// Tick when proposed.
    pub proposed_tick: u64,
    /// Tick when signed (if signed).
    pub signed_tick: Option<u64>,
    /// Tick when broken or expired (if applicable).
    pub ended_tick: Option<u64>,
    /// Duration in ticks (None = indefinite).
    pub duration_ticks: Option<u64>,
}

// ── Diplomacy Error ─────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiplomacyError {
    /// One or both organizations not found.
    OrgNotFound(String),
    /// Cannot negotiate with self.
    SelfNegotiation,
    /// Treaty not found.
    TreatyNotFound(String),
    /// Treaty is not in a state that allows the requested action.
    InvalidTreatyStatus {
        treaty_id: String,
        expected: TreatyStatus,
        actual: TreatyStatus,
    },
    /// A treaty of this type already exists between these orgs.
    TreatyAlreadyExists {
        org_a: String,
        org_b: String,
        treaty_type: TreatyType,
    },
    /// Relation level too low for the requested treaty type.
    RelationTooLow { required: i8, actual: i8 },
    /// Only the proposing org's counterparty can sign.
    NotCounterparty { treaty_id: String, org_id: String },
    /// Organization is dissolved.
    OrgDissolved(String),
}

impl std::fmt::Display for DiplomacyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DiplomacyError::OrgNotFound(id) => write!(f, "organization not found: {}", id),
            DiplomacyError::SelfNegotiation => write!(f, "cannot negotiate with self"),
            DiplomacyError::TreatyNotFound(id) => write!(f, "treaty not found: {}", id),
            DiplomacyError::InvalidTreatyStatus {
                treaty_id,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "treaty {} status is {:?}, expected {:?}",
                    treaty_id, actual, expected
                )
            }
            DiplomacyError::TreatyAlreadyExists {
                org_a,
                org_b,
                treaty_type,
            } => {
                write!(
                    f,
                    "treaty of type {:?} already exists between {} and {}",
                    treaty_type, org_a, org_b
                )
            }
            DiplomacyError::RelationTooLow { required, actual } => {
                write!(
                    f,
                    "relation too low: required {}, actual {}",
                    required, actual
                )
            }
            DiplomacyError::NotCounterparty { treaty_id, org_id } => {
                write!(
                    f,
                    "org {} is not a counterparty to treaty {}",
                    org_id, treaty_id
                )
            }
            DiplomacyError::OrgDissolved(id) => write!(f, "organization dissolved: {}", id),
        }
    }
}

impl std::error::Error for DiplomacyError {}

// ── Diplomacy Engine ────────────────────────────────────────

/// Minimum relation level required for each treaty type.
fn min_relation_for_treaty(treaty_type: TreatyType) -> i8 {
    match treaty_type {
        TreatyType::NonAggression => RelationLevel::COLD,
        TreatyType::TradeAgreement => RelationLevel::NEUTRAL,
        TreatyType::MutualDefense => RelationLevel::WARM,
        TreatyType::ResearchSharing => RelationLevel::FRIENDLY,
        TreatyType::FullAlliance => RelationLevel::ALLIED,
    }
}

/// Manages inter-organization diplomacy: treaties, relations, and negotiations.
pub struct DiplomacyEngine {
    /// All treaties, keyed by treaty ID.
    pub treaties: HashMap<String, Treaty>,
    /// Bilateral relations: (org_a, org_b) -> relation level.
    /// Keys are stored in sorted order (min, max) to ensure uniqueness.
    pub relations: HashMap<(String, String), RelationLevel>,
    /// Event bus for emitting events.
    event_bus: Option<Arc<EventBus>>,
    /// Monotonic counter for treaty IDs.
    next_treaty_id: u64,
}

impl DiplomacyEngine {
    /// Create a new diplomacy engine.
    pub fn new() -> Self {
        Self {
            treaties: HashMap::new(),
            relations: HashMap::new(),
            event_bus: None,
            next_treaty_id: 1,
        }
    }

    /// Create a diplomacy engine wired to an EventBus.
    pub fn with_event_bus(event_bus: EventBus) -> Self {
        Self {
            treaties: HashMap::new(),
            relations: HashMap::new(),
            event_bus: Some(Arc::new(event_bus)),
            next_treaty_id: 1,
        }
    }

    /// Create a diplomacy engine with a shared Arc<EventBus>.
    pub fn with_shared_event_bus(event_bus: Arc<EventBus>) -> Self {
        Self {
            treaties: HashMap::new(),
            relations: HashMap::new(),
            event_bus: Some(event_bus),
            next_treaty_id: 1,
        }
    }

    // ── Relation Management ──────────────────────────────

    /// Normalize the key so (a, b) and (b, a) map to the same entry.
    fn relation_key(a: &str, b: &str) -> (String, String) {
        if a < b {
            (a.to_string(), b.to_string())
        } else {
            (b.to_string(), a.to_string())
        }
    }

    /// Get the current relation level between two organizations.
    /// Returns NEUTRAL if no explicit relation exists.
    pub fn get_relation(&self, org_a: &str, org_b: &str) -> RelationLevel {
        let key = Self::relation_key(org_a, org_b);
        self.relations
            .get(&key)
            .copied()
            .unwrap_or(RelationLevel(RelationLevel::NEUTRAL))
    }

    /// Manually set the relation level between two organizations.
    pub fn set_relation(&mut self, org_a: &str, org_b: &str, level: i8) {
        let key = Self::relation_key(org_a, org_b);
        let old = self
            .relations
            .get(&key)
            .map(|r| r.0)
            .unwrap_or(RelationLevel::NEUTRAL);
        self.relations.insert(key, RelationLevel(level));

        self.emit(WorldEvent::RelationChanged {
            org_a: org_a.to_string(),
            org_b: org_b.to_string(),
            old_level: old,
            new_level: level,
        });
    }

    /// Adjust the relation level by a delta (clamped to [-100, 100]).
    pub fn adjust_relation(&mut self, org_a: &str, org_b: &str, delta: i8) {
        let current = self.get_relation(org_a, org_b).0;
        let new_level = (current as i16 + delta as i16).clamp(-100, 100) as i8;
        self.set_relation(org_a, org_b, new_level);
    }

    // ── Treaty Lifecycle ─────────────────────────────────

    /// Propose a new treaty between two organizations.
    ///
    /// Checks that:
    /// - The two orgs are different
    /// - No active treaty of the same type exists
    /// - Relation level is sufficient for the treaty type
    pub fn propose_treaty(
        &mut self,
        org_a: &str,
        org_b: &str,
        treaty_type: TreatyType,
        tick: u64,
        duration_ticks: Option<u64>,
    ) -> Result<String, DiplomacyError> {
        if org_a == org_b {
            return Err(DiplomacyError::SelfNegotiation);
        }

        // Check for existing active treaty of same type
        let has_existing = self.treaties.values().any(|t| {
            t.treaty_type == treaty_type
                && t.status == TreatyStatus::Active
                && ((t.org_a == *org_a && t.org_b == *org_b)
                    || (t.org_a == *org_b && t.org_b == *org_a))
        });
        if has_existing {
            return Err(DiplomacyError::TreatyAlreadyExists {
                org_a: org_a.to_string(),
                org_b: org_b.to_string(),
                treaty_type,
            });
        }

        // Check relation level
        let required = min_relation_for_treaty(treaty_type);
        let current = self.get_relation(org_a, org_b).0;
        if current < required {
            return Err(DiplomacyError::RelationTooLow {
                required,
                actual: current,
            });
        }

        let treaty_id = format!("treaty-{}", self.next_treaty_id);
        self.next_treaty_id += 1;

        let treaty = Treaty {
            id: treaty_id.clone(),
            org_a: org_a.to_string(),
            org_b: org_b.to_string(),
            treaty_type,
            status: TreatyStatus::Proposed,
            proposed_tick: tick,
            signed_tick: None,
            ended_tick: None,
            duration_ticks,
        };

        self.treaties.insert(treaty_id.clone(), treaty);

        self.emit(WorldEvent::TreatyProposed {
            treaty_id: treaty_id.clone(),
            org_a: org_a.to_string(),
            org_b: org_b.to_string(),
            treaty_type: treaty_type.to_string(),
        });

        Ok(treaty_id)
    }

    /// Sign (ratify) a proposed treaty.
    ///
    /// Only the counterparty (org_b if org_a proposed, or vice versa) can sign.
    pub fn sign_treaty(
        &mut self,
        treaty_id: &str,
        signer_org: &str,
        tick: u64,
    ) -> Result<(), DiplomacyError> {
        let treaty = self
            .treaties
            .get(treaty_id)
            .ok_or_else(|| DiplomacyError::TreatyNotFound(treaty_id.to_string()))?;

        if treaty.status != TreatyStatus::Proposed {
            return Err(DiplomacyError::InvalidTreatyStatus {
                treaty_id: treaty_id.to_string(),
                expected: TreatyStatus::Proposed,
                actual: treaty.status,
            });
        }

        // Verify signer is one of the two parties (and not the proposer for realism,
        // though we allow either party to sign)
        if treaty.org_a != signer_org && treaty.org_b != signer_org {
            return Err(DiplomacyError::NotCounterparty {
                treaty_id: treaty_id.to_string(),
                org_id: signer_org.to_string(),
            });
        }

        let treaty = self.treaties.get_mut(treaty_id).unwrap();
        treaty.status = TreatyStatus::Active;
        treaty.signed_tick = Some(tick);

        let org_a = treaty.org_a.clone();
        let org_b = treaty.org_b.clone();

        // Improve relations when treaty is signed
        self.adjust_relation(&org_a, &org_b, 1);

        self.emit(WorldEvent::TreatySigned {
            treaty_id: treaty_id.to_string(),
            org_a,
            org_b,
        });

        Ok(())
    }

    /// Break an active or proposed treaty.
    pub fn break_treaty(
        &mut self,
        treaty_id: &str,
        breaker: &str,
        reason: &str,
        tick: u64,
    ) -> Result<(), DiplomacyError> {
        let treaty = self
            .treaties
            .get(treaty_id)
            .ok_or_else(|| DiplomacyError::TreatyNotFound(treaty_id.to_string()))?;

        if treaty.status != TreatyStatus::Active && treaty.status != TreatyStatus::Proposed {
            return Err(DiplomacyError::InvalidTreatyStatus {
                treaty_id: treaty_id.to_string(),
                expected: TreatyStatus::Active,
                actual: treaty.status,
            });
        }

        if treaty.org_a != breaker && treaty.org_b != breaker {
            return Err(DiplomacyError::NotCounterparty {
                treaty_id: treaty_id.to_string(),
                org_id: breaker.to_string(),
            });
        }

        let treaty = self.treaties.get_mut(treaty_id).unwrap();
        treaty.status = TreatyStatus::Broken;
        treaty.ended_tick = Some(tick);

        let org_a = treaty.org_a.clone();
        let org_b = treaty.org_b.clone();

        // Degrade relations when treaty is broken
        self.adjust_relation(&org_a, &org_b, -2);

        self.emit(WorldEvent::TreatyBroken {
            treaty_id: treaty_id.to_string(),
            breaker: breaker.to_string(),
            reason: reason.to_string(),
        });

        Ok(())
    }

    /// Check and expire treaties that have exceeded their duration.
    pub fn tick_expiry(&mut self, current_tick: u64) -> Vec<String> {
        let mut expired = Vec::new();
        for (id, treaty) in &mut self.treaties {
            if treaty.status == TreatyStatus::Active {
                if let (Some(signed), Some(duration)) = (treaty.signed_tick, treaty.duration_ticks)
                {
                    if current_tick >= signed + duration {
                        treaty.status = TreatyStatus::Expired;
                        treaty.ended_tick = Some(current_tick);
                        expired.push(id.clone());
                    }
                }
            }
        }
        expired
    }

    // ── Queries ──────────────────────────────────────────

    /// Get a treaty by ID.
    pub fn get_treaty(&self, treaty_id: &str) -> Option<&Treaty> {
        self.treaties.get(treaty_id)
    }

    /// Get all active treaties for an organization.
    pub fn get_active_treaties(&self, org_id: &str) -> Vec<&Treaty> {
        self.treaties
            .values()
            .filter(|t| {
                t.status == TreatyStatus::Active && (t.org_a == org_id || t.org_b == org_id)
            })
            .collect()
    }

    /// Get all treaties (any status) involving an organization.
    pub fn get_treaties_for_org(&self, org_id: &str) -> Vec<&Treaty> {
        self.treaties
            .values()
            .filter(|t| t.org_a == org_id || t.org_b == org_id)
            .collect()
    }

    /// Get all organizations that have positive relations with the given org.
    pub fn get_allies(&self, org_id: &str) -> Vec<(String, RelationLevel)> {
        self.relations
            .iter()
            .filter_map(|((a, b), level)| {
                if level.is_positive() {
                    if a == org_id {
                        Some((b.clone(), *level))
                    } else if b == org_id {
                        Some((a.clone(), *level))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect()
    }

    // ── Event Emission ───────────────────────────────────

    fn emit(&self, event: WorldEvent) {
        if let Some(ref bus) = self.event_bus {
            bus.emit(event);
        }
    }
}

impl Default for DiplomacyEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ─────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_engine() -> DiplomacyEngine {
        DiplomacyEngine::new()
    }

    // ── Relation Tests ───────────────────────────────────

    #[test]
    fn test_default_relation_is_neutral() {
        let engine = make_engine();
        assert_eq!(
            engine.get_relation("org-a", "org-b").0,
            RelationLevel::NEUTRAL
        );
    }

    #[test]
    fn test_set_relation() {
        let mut engine = make_engine();
        engine.set_relation("org-a", "org-b", RelationLevel::FRIENDLY);
        assert_eq!(
            engine.get_relation("org-a", "org-b").0,
            RelationLevel::FRIENDLY
        );
        // Symmetric
        assert_eq!(
            engine.get_relation("org-b", "org-a").0,
            RelationLevel::FRIENDLY
        );
    }

    #[test]
    fn test_adjust_relation() {
        let mut engine = make_engine();
        engine.adjust_relation("org-a", "org-b", 2);
        assert_eq!(engine.get_relation("org-a", "org-b").0, 2);
        engine.adjust_relation("org-a", "org-b", -1);
        assert_eq!(engine.get_relation("org-a", "org-b").0, 1);
    }

    #[test]
    fn test_adjust_relation_clamps() {
        let mut engine = make_engine();
        engine.adjust_relation("org-a", "org-b", 127);
        assert_eq!(engine.get_relation("org-a", "org-b").0, 100);
    }

    // ── Treaty Proposal Tests ────────────────────────────

    #[test]
    fn test_propose_treaty_success() {
        let mut engine = make_engine();
        engine.set_relation("org-a", "org-b", RelationLevel::WARM);

        let id = engine
            .propose_treaty("org-a", "org-b", TreatyType::TradeAgreement, 100, None)
            .unwrap();
        let treaty = engine.get_treaty(&id).unwrap();
        assert_eq!(treaty.org_a, "org-a");
        assert_eq!(treaty.org_b, "org-b");
        assert_eq!(treaty.treaty_type, TreatyType::TradeAgreement);
        assert_eq!(treaty.status, TreatyStatus::Proposed);
    }

    #[test]
    fn test_propose_treaty_self_negotiation_fails() {
        let mut engine = make_engine();
        let result = engine.propose_treaty("org-a", "org-a", TreatyType::NonAggression, 100, None);
        assert_eq!(result.unwrap_err(), DiplomacyError::SelfNegotiation);
    }

    #[test]
    fn test_propose_treaty_relation_too_low() {
        let mut engine = make_engine();
        // NEUTRAL is too low for FullAlliance (requires ALLIED=3)
        let result = engine.propose_treaty("org-a", "org-b", TreatyType::FullAlliance, 100, None);
        assert!(matches!(
            result.unwrap_err(),
            DiplomacyError::RelationTooLow { .. }
        ));
    }

    #[test]
    fn test_propose_treaty_duplicate_fails() {
        let mut engine = make_engine();
        engine.set_relation("org-a", "org-b", RelationLevel::WARM);
        engine
            .propose_treaty("org-a", "org-b", TreatyType::TradeAgreement, 100, None)
            .unwrap();
        engine.sign_treaty("treaty-1", "org-b", 101).unwrap();

        let result = engine.propose_treaty("org-a", "org-b", TreatyType::TradeAgreement, 110, None);
        assert!(matches!(
            result.unwrap_err(),
            DiplomacyError::TreatyAlreadyExists { .. }
        ));
    }

    // ── Treaty Signing Tests ─────────────────────────────

    #[test]
    fn test_sign_treaty_success() {
        let mut engine = make_engine();
        engine.set_relation("org-a", "org-b", RelationLevel::WARM);
        let id = engine
            .propose_treaty("org-a", "org-b", TreatyType::TradeAgreement, 100, None)
            .unwrap();

        engine.sign_treaty(&id, "org-b", 105).unwrap();

        let treaty = engine.get_treaty(&id).unwrap();
        assert_eq!(treaty.status, TreatyStatus::Active);
        assert_eq!(treaty.signed_tick, Some(105));

        // Signing improves relation
        assert!(engine.get_relation("org-a", "org-b").0 > RelationLevel::WARM);
    }

    #[test]
    fn test_sign_treaty_non_counterparty_fails() {
        let mut engine = make_engine();
        engine.set_relation("org-a", "org-b", RelationLevel::WARM);
        let id = engine
            .propose_treaty("org-a", "org-b", TreatyType::TradeAgreement, 100, None)
            .unwrap();

        let result = engine.sign_treaty(&id, "org-c", 105);
        assert!(matches!(
            result.unwrap_err(),
            DiplomacyError::NotCounterparty { .. }
        ));
    }

    #[test]
    fn test_sign_treaty_already_signed_fails() {
        let mut engine = make_engine();
        engine.set_relation("org-a", "org-b", RelationLevel::WARM);
        let id = engine
            .propose_treaty("org-a", "org-b", TreatyType::TradeAgreement, 100, None)
            .unwrap();
        engine.sign_treaty(&id, "org-b", 105).unwrap();

        let result = engine.sign_treaty(&id, "org-a", 106);
        assert!(matches!(
            result.unwrap_err(),
            DiplomacyError::InvalidTreatyStatus { .. }
        ));
    }

    // ── Treaty Breaking Tests ────────────────────────────

    #[test]
    fn test_break_treaty_success() {
        let mut engine = make_engine();
        engine.set_relation("org-a", "org-b", RelationLevel::FRIENDLY);
        let id = engine
            .propose_treaty("org-a", "org-b", TreatyType::NonAggression, 100, None)
            .unwrap();
        engine.sign_treaty(&id, "org-b", 105).unwrap();

        engine.break_treaty(&id, "org-a", "betrayal", 120).unwrap();

        let treaty = engine.get_treaty(&id).unwrap();
        assert_eq!(treaty.status, TreatyStatus::Broken);
        assert_eq!(treaty.ended_tick, Some(120));

        // Breaking degrades relations
        let rel = engine.get_relation("org-a", "org-b").0;
        assert!(rel < RelationLevel::FRIENDLY);
    }

    #[test]
    fn test_break_proposed_treaty() {
        let mut engine = make_engine();
        engine.set_relation("org-a", "org-b", RelationLevel::WARM);
        let id = engine
            .propose_treaty("org-a", "org-b", TreatyType::TradeAgreement, 100, None)
            .unwrap();

        engine
            .break_treaty(&id, "org-b", "changed mind", 105)
            .unwrap();
        assert_eq!(engine.get_treaty(&id).unwrap().status, TreatyStatus::Broken);
    }

    #[test]
    fn test_break_treaty_non_member_fails() {
        let mut engine = make_engine();
        engine.set_relation("org-a", "org-b", RelationLevel::WARM);
        let id = engine
            .propose_treaty("org-a", "org-b", TreatyType::TradeAgreement, 100, None)
            .unwrap();
        engine.sign_treaty(&id, "org-b", 105).unwrap();

        let result = engine.break_treaty(&id, "org-c", "interference", 120);
        assert!(matches!(
            result.unwrap_err(),
            DiplomacyError::NotCounterparty { .. }
        ));
    }

    // ── Treaty Expiry Tests ──────────────────────────────

    #[test]
    fn test_treaty_expiry() {
        let mut engine = make_engine();
        engine.set_relation("org-a", "org-b", RelationLevel::WARM);
        let id = engine
            .propose_treaty("org-a", "org-b", TreatyType::TradeAgreement, 100, Some(50))
            .unwrap();
        engine.sign_treaty(&id, "org-b", 110).unwrap();

        // Not expired yet
        let expired = engine.tick_expiry(159);
        assert!(expired.is_empty());

        // Expires at tick 160 (signed 110 + duration 50)
        let expired = engine.tick_expiry(160);
        assert_eq!(expired.len(), 1);
        assert_eq!(
            engine.get_treaty(&id).unwrap().status,
            TreatyStatus::Expired
        );
    }

    #[test]
    fn test_indefinite_treaty_never_expires() {
        let mut engine = make_engine();
        engine.set_relation("org-a", "org-b", RelationLevel::WARM);
        let id = engine
            .propose_treaty("org-a", "org-b", TreatyType::TradeAgreement, 100, None)
            .unwrap();
        engine.sign_treaty(&id, "org-b", 110).unwrap();

        let expired = engine.tick_expiry(10000);
        assert!(expired.is_empty());
    }

    // ── Query Tests ──────────────────────────────────────

    #[test]
    fn test_get_active_treaties() {
        let mut engine = make_engine();
        engine.set_relation("org-a", "org-b", RelationLevel::FRIENDLY);
        engine.set_relation("org-a", "org-c", RelationLevel::FRIENDLY);

        engine
            .propose_treaty("org-a", "org-b", TreatyType::TradeAgreement, 100, None)
            .unwrap();
        engine
            .propose_treaty("org-a", "org-c", TreatyType::NonAggression, 100, None)
            .unwrap();

        // No active yet
        assert!(engine.get_active_treaties("org-a").is_empty());

        engine.sign_treaty("treaty-1", "org-b", 105).unwrap();
        assert_eq!(engine.get_active_treaties("org-a").len(), 1);
        assert_eq!(engine.get_active_treaties("org-b").len(), 1);
    }

    #[test]
    fn test_get_allies() {
        let mut engine = make_engine();
        engine.set_relation("org-a", "org-b", RelationLevel::FRIENDLY);
        engine.set_relation("org-a", "org-c", RelationLevel::HOSTILE);
        engine.set_relation("org-a", "org-d", RelationLevel::WARM);

        let allies = engine.get_allies("org-a");
        assert_eq!(allies.len(), 2); // org-b (2) and org-d (1)
    }

    // ── Event Bus Integration ────────────────────────────

    #[test]
    fn test_event_bus_treaty_proposed() {
        let bus = crate::world::state::EventBus::new(64);
        let mut rx = bus.subscribe();
        let mut engine = DiplomacyEngine::with_event_bus(bus);
        engine.set_relation("org-a", "org-b", RelationLevel::WARM);

        engine
            .propose_treaty("org-a", "org-b", TreatyType::TradeAgreement, 100, None)
            .unwrap();

        // First event is RelationChanged from set_relation, second is TreatyProposed
        let _ = rx.try_recv(); // drain RelationChanged
        let event = rx.try_recv().unwrap();
        match event {
            WorldEvent::TreatyProposed {
                treaty_id,
                org_a,
                org_b,
                treaty_type,
            } => {
                assert_eq!(treaty_id, "treaty-1");
                assert_eq!(org_a, "org-a");
                assert_eq!(org_b, "org-b");
                assert_eq!(treaty_type, "trade_agreement");
            }
            _ => panic!("Expected TreatyProposed event"),
        }
    }

    #[test]
    fn test_event_bus_treaty_signed() {
        let bus = crate::world::state::EventBus::new(64);
        let mut rx = bus.subscribe();
        let mut engine = DiplomacyEngine::with_event_bus(bus);
        engine.set_relation("org-a", "org-b", RelationLevel::WARM);

        let id = engine
            .propose_treaty("org-a", "org-b", TreatyType::TradeAgreement, 100, None)
            .unwrap();
        // Drain events from proposal
        let _ = rx.try_recv(); // RelationChanged from set_relation
        let _ = rx.try_recv(); // TreatyProposed

        engine.sign_treaty(&id, "org-b", 105).unwrap();
        // RelationChanged from signing adjustment, then TreatySigned
        let _ = rx.try_recv(); // RelationChanged

        let event = rx.try_recv().unwrap();
        match event {
            WorldEvent::TreatySigned {
                treaty_id,
                org_a,
                org_b,
            } => {
                assert_eq!(treaty_id, id);
                assert_eq!(org_a, "org-a");
                assert_eq!(org_b, "org-b");
            }
            _ => panic!("Expected TreatySigned event"),
        }
    }

    #[test]
    fn test_event_bus_treaty_broken() {
        let bus = crate::world::state::EventBus::new(64);
        let mut rx = bus.subscribe();
        let mut engine = DiplomacyEngine::with_event_bus(bus);
        engine.set_relation("org-a", "org-b", RelationLevel::FRIENDLY);

        let id = engine
            .propose_treaty("org-a", "org-b", TreatyType::NonAggression, 100, None)
            .unwrap();
        engine.sign_treaty(&id, "org-b", 105).unwrap();

        // Drain all prior events
        while rx.try_recv().is_ok() {}

        engine.break_treaty(&id, "org-a", "betrayal", 120).unwrap();

        // RelationChanged from break penalty, then TreatyBroken
        let _ = rx.try_recv(); // RelationChanged
        let event = rx.try_recv().unwrap();
        match event {
            WorldEvent::TreatyBroken {
                treaty_id,
                breaker,
                reason,
            } => {
                assert_eq!(treaty_id, id);
                assert_eq!(breaker, "org-a");
                assert_eq!(reason, "betrayal");
            }
            _ => panic!("Expected TreatyBroken event"),
        }
    }

    // ── Full Workflow Test ───────────────────────────────

    #[test]
    fn test_full_diplomacy_workflow() {
        let mut engine = make_engine();

        // Build relations
        engine.set_relation("guild", "company", RelationLevel::WARM);
        assert_eq!(
            engine.get_relation("guild", "company").0,
            RelationLevel::WARM
        );

        // Propose and sign trade agreement
        let id1 = engine
            .propose_treaty("guild", "company", TreatyType::TradeAgreement, 10, None)
            .unwrap();
        engine.sign_treaty(&id1, "company", 15).unwrap();
        assert_eq!(engine.get_active_treaties("guild").len(), 1);

        // Improve relations, propose non-aggression with a third org
        engine.set_relation("guild", "alliance", RelationLevel::FRIENDLY);
        let id2 = engine
            .propose_treaty(
                "guild",
                "alliance",
                TreatyType::ResearchSharing,
                20,
                Some(100),
            )
            .unwrap();
        engine.sign_treaty(&id2, "alliance", 25).unwrap();

        // Check allies
        let allies = engine.get_allies("guild");
        assert!(allies.len() >= 2);

        // Break first treaty
        engine.break_treaty(&id1, "guild", "dispute", 50).unwrap();
        assert_eq!(engine.get_active_treaties("guild").len(), 1); // only id2 remains
        assert!(engine.get_relation("guild", "company").0 < RelationLevel::WARM);

        // Expire second treaty
        let expired = engine.tick_expiry(130);
        assert_eq!(expired.len(), 1);
    }
}
