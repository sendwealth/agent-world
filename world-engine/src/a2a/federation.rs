use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::world::state::EventBus;
use crate::world::event::WorldEvent;

// ── Diplomatic Status ──────────────────────────────────────

/// Diplomatic status between this world and a foreign world.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiplomaticStatus {
    /// No contact or formal relations.
    Neutral,
    /// Formal peaceful relations established.
    Peace,
    /// Active trade agreement in place.
    TradeAgreement,
    /// Full military and economic alliance.
    Alliance,
    /// Diplomatic freeze — limited communication.
    ColdWar,
    /// Active conflict — cross-world trade blocked.
    War,
}

impl std::fmt::Display for DiplomaticStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DiplomaticStatus::Neutral => write!(f, "neutral"),
            DiplomaticStatus::Peace => write!(f, "peace"),
            DiplomaticStatus::TradeAgreement => write!(f, "trade_agreement"),
            DiplomaticStatus::Alliance => write!(f, "alliance"),
            DiplomaticStatus::ColdWar => write!(f, "cold_war"),
            DiplomaticStatus::War => write!(f, "war"),
        }
    }
}

// ── Cross-World Treaty Types ───────────────────────────────

/// Types of treaties between worlds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CrossWorldTreatyType {
    /// Non-aggression pact.
    NonAggression,
    /// Trade agreement — enables cross-world commerce.
    TradePact,
    /// Military alliance — mutual defense.
    MilitaryAlliance,
    /// Research exchange — shared knowledge.
    ResearchExchange,
    /// Cultural exchange — shared traditions.
    CulturalExchange,
}

impl std::fmt::Display for CrossWorldTreatyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CrossWorldTreatyType::NonAggression => write!(f, "non_aggression"),
            CrossWorldTreatyType::TradePact => write!(f, "trade_pact"),
            CrossWorldTreatyType::MilitaryAlliance => write!(f, "military_alliance"),
            CrossWorldTreatyType::ResearchExchange => write!(f, "research_exchange"),
            CrossWorldTreatyType::CulturalExchange => write!(f, "cultural_exchange"),
        }
    }
}

// ── Treaty Status ──────────────────────────────────────────

/// Status of a cross-world treaty.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CrossWorldTreatyStatus {
    /// Proposed but awaiting acceptance.
    Proposed,
    /// Accepted and in effect.
    Active,
    /// Rejected by the counterparty.
    Rejected,
    /// Broken by one party.
    Broken,
    /// Expired after duration elapsed.
    Expired,
}

// ── Cross-World Treaty ─────────────────────────────────────

/// A treaty between this world and a foreign world.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossWorldTreaty {
    /// Unique treaty ID.
    pub id: String,
    /// Foreign world ID this treaty is with.
    pub foreign_world_id: String,
    /// Type of treaty.
    pub treaty_type: CrossWorldTreatyType,
    /// Current status.
    pub status: CrossWorldTreatyStatus,
    /// Tick when proposed.
    pub proposed_tick: u64,
    /// Tick when accepted (if accepted).
    pub accepted_tick: Option<u64>,
    /// Tick when ended (broken/expired).
    pub ended_tick: Option<u64>,
    /// Duration in ticks (None = indefinite).
    pub duration_ticks: Option<u64>,
    /// Terms of the treaty (free-form JSON).
    pub terms: String,
}

// ── Foreign World ──────────────────────────────────────────

/// Record for a discovered foreign world.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForeignWorld {
    /// Unique world identifier.
    pub id: String,
    /// Human-readable world name.
    pub name: String,
    /// Endpoint for A2A communication.
    pub endpoint: String,
    /// Current diplomatic status.
    pub diplomatic_status: DiplomaticStatus,
    /// Relation score [-100, 100].
    pub relation_score: i16,
    /// Whether the world is currently reachable.
    pub online: bool,
    /// Tick when first discovered.
    pub discovered_tick: u64,
    /// Last heartbeat tick from this world.
    pub last_seen_tick: u64,
}

// ── Federation Error ───────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FederationError {
    /// Foreign world not found in registry.
    WorldNotFound(String),
    /// World already registered.
    WorldAlreadyRegistered(String),
    /// Cannot perform action on self.
    SelfAction,
    /// Treaty not found.
    TreatyNotFound(String),
    /// Treaty is in the wrong status for the requested action.
    InvalidTreatyStatus { treaty_id: String, expected: CrossWorldTreatyStatus, actual: CrossWorldTreatyStatus },
    /// A treaty of this type already exists with this world.
    TreatyAlreadyExists { world_id: String, treaty_type: CrossWorldTreatyType },
    /// Diplomatic status does not allow the requested action.
    InvalidDiplomaticStatus { world_id: String, required: String, actual: DiplomaticStatus },
    /// Relation score too low.
    RelationTooLow { required: i16, actual: i16 },
    /// Cannot establish relations with a world at war.
    AtWar(String),
    /// Sanction already active.
    SanctionAlreadyActive(String),
    /// No active sanction to lift.
    NoActiveSanction(String),
    /// No active peace proposal to accept.
    NoPeaceProposal(String),
}

impl std::fmt::Display for FederationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FederationError::WorldNotFound(id) => write!(f, "foreign world not found: {}", id),
            FederationError::WorldAlreadyRegistered(id) => write!(f, "world already registered: {}", id),
            FederationError::SelfAction => write!(f, "cannot perform action on self"),
            FederationError::TreatyNotFound(id) => write!(f, "treaty not found: {}", id),
            FederationError::InvalidTreatyStatus { treaty_id, expected, actual } => {
                write!(f, "treaty {} status is {:?}, expected {:?}", treaty_id, actual, expected)
            }
            FederationError::TreatyAlreadyExists { world_id, treaty_type } => {
                write!(f, "treaty of type {:?} already exists with world {}", treaty_type, world_id)
            }
            FederationError::InvalidDiplomaticStatus { world_id, required, actual } => {
                write!(f, "world {} requires {} status, got {:?}", world_id, required, actual)
            }
            FederationError::RelationTooLow { required, actual } => {
                write!(f, "relation too low: required {}, actual {}", required, actual)
            }
            FederationError::AtWar(id) => write!(f, "at war with world: {}", id),
            FederationError::SanctionAlreadyActive(id) => write!(f, "sanction already active on world: {}", id),
            FederationError::NoActiveSanction(id) => write!(f, "no active sanction on world: {}", id),
            FederationError::NoPeaceProposal(id) => write!(f, "no pending peace proposal with world: {}", id),
        }
    }
}

impl std::error::Error for FederationError {}

// ── Federation Engine ──────────────────────────────────────

/// Manages cross-world diplomatic relations, treaties, and foreign world registry.
pub struct FederationEngine {
    /// Registry of known foreign worlds.
    pub foreign_worlds: HashMap<String, ForeignWorld>,
    /// All treaties with foreign worlds.
    pub treaties: HashMap<String, CrossWorldTreaty>,
    /// Active sanctions (world_id -> reason).
    pub sanctions: HashMap<String, String>,
    /// Pending peace proposals (world_id -> treaty_id).
    pub peace_proposals: HashMap<String, String>,
    /// Event bus for emitting events.
    event_bus: Option<Arc<EventBus>>,
    /// Monotonic counter for treaty IDs.
    next_treaty_id: u64,
}

impl FederationEngine {
    /// Create a new federation engine.
    pub fn new() -> Self {
        Self {
            foreign_worlds: HashMap::new(),
            treaties: HashMap::new(),
            sanctions: HashMap::new(),
            peace_proposals: HashMap::new(),
            event_bus: None,
            next_treaty_id: 1,
        }
    }

    /// Create with an EventBus.
    pub fn with_event_bus(event_bus: EventBus) -> Self {
        Self {
            foreign_worlds: HashMap::new(),
            treaties: HashMap::new(),
            sanctions: HashMap::new(),
            peace_proposals: HashMap::new(),
            event_bus: Some(Arc::new(event_bus)),
            next_treaty_id: 1,
        }
    }

    /// Create with a shared Arc<EventBus>.
    pub fn with_shared_event_bus(event_bus: Arc<EventBus>) -> Self {
        Self {
            foreign_worlds: HashMap::new(),
            treaties: HashMap::new(),
            sanctions: HashMap::new(),
            peace_proposals: HashMap::new(),
            event_bus: Some(event_bus),
            next_treaty_id: 1,
        }
    }

    fn allocate_treaty_id(&mut self) -> String {
        let id = format!("cw-treaty-{}", self.next_treaty_id);
        self.next_treaty_id += 1;
        id
    }

    // ── Foreign World Registry ──────────────────────────

    /// Register a new foreign world.
    pub fn register_world(
        &mut self,
        id: String,
        name: String,
        endpoint: String,
        tick: u64,
    ) -> Result<(), FederationError> {
        if self.foreign_worlds.contains_key(&id) {
            return Err(FederationError::WorldAlreadyRegistered(id));
        }

        let world = ForeignWorld {
            id: id.clone(),
            name,
            endpoint,
            diplomatic_status: DiplomaticStatus::Neutral,
            relation_score: 0,
            online: true,
            discovered_tick: tick,
            last_seen_tick: tick,
        };

        self.foreign_worlds.insert(id.clone(), world);

        self.emit(WorldEvent::ForeignWorldDiscovered {
            world_id: id.clone(),
            name: self.foreign_worlds.get(&id).map(|w| w.name.clone()).unwrap_or_default(),
            endpoint: self.foreign_worlds.get(&id).map(|w| w.endpoint.clone()).unwrap_or_default(),
        });

        Ok(())
    }

    /// Deregister (remove) a foreign world.
    pub fn deregister_world(&mut self, world_id: &str) -> Result<(), FederationError> {
        let world = self.foreign_worlds.remove(world_id)
            .ok_or_else(|| FederationError::WorldNotFound(world_id.to_string()))?;

        // Remove all treaties with this world
        self.treaties.retain(|_, t| t.foreign_world_id != world_id);
        self.sanctions.remove(world_id);
        self.peace_proposals.remove(world_id);

        self.emit(WorldEvent::ForeignWorldDeregistered {
            world_id: world_id.to_string(),
            name: world.name,
        });

        Ok(())
    }

    /// Update online status of a foreign world.
    pub fn update_online_status(&mut self, world_id: &str, online: bool, tick: u64) -> Result<(), FederationError> {
        let world = self.foreign_worlds.get_mut(world_id)
            .ok_or_else(|| FederationError::WorldNotFound(world_id.to_string()))?;
        world.online = online;
        if online {
            world.last_seen_tick = tick;
        }
        Ok(())
    }

    /// Get a foreign world by ID.
    pub fn get_world(&self, world_id: &str) -> Option<&ForeignWorld> {
        self.foreign_worlds.get(world_id)
    }

    /// List all foreign worlds.
    pub fn list_worlds(&self) -> Vec<&ForeignWorld> {
        self.foreign_worlds.values().collect()
    }

    // ── Diplomatic Relations ────────────────────────────

    /// Establish formal diplomatic relations (Neutral → Peace).
    pub fn establish_relations(&mut self, world_id: &str, _tick: u64) -> Result<(), FederationError> {
        let world = self.foreign_worlds.get_mut(world_id)
            .ok_or_else(|| FederationError::WorldNotFound(world_id.to_string()))?;

        if world.diplomatic_status == DiplomaticStatus::Peace
            || world.diplomatic_status == DiplomaticStatus::TradeAgreement
            || world.diplomatic_status == DiplomaticStatus::Alliance {
            return Err(FederationError::InvalidDiplomaticStatus {
                world_id: world_id.to_string(),
                required: "neutral or cold_war".to_string(),
                actual: world.diplomatic_status,
            });
        }

        if world.diplomatic_status == DiplomaticStatus::War {
            return Err(FederationError::AtWar(world_id.to_string()));
        }

        let old_status = world.diplomatic_status;
        world.diplomatic_status = DiplomaticStatus::Peace;
        world.relation_score = (world.relation_score + 10).clamp(-100, 100);

        self.emit(WorldEvent::DiplomaticRelationsEstablished {
            world_id: world_id.to_string(),
            old_status,
            new_status: DiplomaticStatus::Peace,
        });

        Ok(())
    }

    /// Adjust relation score with a foreign world (clamped to [-100, 100]).
    pub fn adjust_relation(&mut self, world_id: &str, delta: i16) -> Result<i16, FederationError> {
        let world = self.foreign_worlds.get_mut(world_id)
            .ok_or_else(|| FederationError::WorldNotFound(world_id.to_string()))?;

        let old_score = world.relation_score;
        world.relation_score = (world.relation_score + delta).clamp(-100, 100);
        let new_score = world.relation_score;

        self.emit(WorldEvent::CrossWorldRelationChanged {
            world_id: world_id.to_string(),
            old_score,
            new_score,
        });

        Ok(new_score)
    }

    // ── Treaty Lifecycle ────────────────────────────────

    /// Minimum relation score required for each treaty type.
    fn min_relation_for_treaty(treaty_type: CrossWorldTreatyType) -> i16 {
        match treaty_type {
            CrossWorldTreatyType::NonAggression => -50,
            CrossWorldTreatyType::TradePact => 0,
            CrossWorldTreatyType::MilitaryAlliance => 50,
            CrossWorldTreatyType::ResearchExchange => 20,
            CrossWorldTreatyType::CulturalExchange => 10,
        }
    }

    /// Propose a new treaty with a foreign world.
    pub fn propose_treaty(
        &mut self,
        world_id: &str,
        treaty_type: CrossWorldTreatyType,
        tick: u64,
        duration_ticks: Option<u64>,
        terms: String,
    ) -> Result<String, FederationError> {
        let world = self.foreign_worlds.get(world_id)
            .ok_or_else(|| FederationError::WorldNotFound(world_id.to_string()))?;

        if world.diplomatic_status == DiplomaticStatus::War {
            return Err(FederationError::AtWar(world_id.to_string()));
        }

        // Check relation score
        let required = Self::min_relation_for_treaty(treaty_type);
        if world.relation_score < required {
            return Err(FederationError::RelationTooLow {
                required,
                actual: world.relation_score,
            });
        }

        // Check for existing active treaty of same type
        let has_existing = self.treaties.values().any(|t| {
            t.foreign_world_id == world_id
                && t.treaty_type == treaty_type
                && t.status == CrossWorldTreatyStatus::Active
        });
        if has_existing {
            return Err(FederationError::TreatyAlreadyExists {
                world_id: world_id.to_string(),
                treaty_type,
            });
        }

        let treaty_id = self.allocate_treaty_id();
        let treaty = CrossWorldTreaty {
            id: treaty_id.clone(),
            foreign_world_id: world_id.to_string(),
            treaty_type,
            status: CrossWorldTreatyStatus::Proposed,
            proposed_tick: tick,
            accepted_tick: None,
            ended_tick: None,
            duration_ticks,
            terms,
        };

        self.treaties.insert(treaty_id.clone(), treaty);

        self.emit(WorldEvent::CrossWorldTreatyProposed {
            treaty_id: treaty_id.clone(),
            world_id: world_id.to_string(),
            treaty_type: treaty_type.to_string(),
        });

        Ok(treaty_id)
    }

    /// Accept a proposed treaty.
    pub fn accept_treaty(&mut self, treaty_id: &str, tick: u64) -> Result<(), FederationError> {
        let treaty = self.treaties.get(treaty_id)
            .ok_or_else(|| FederationError::TreatyNotFound(treaty_id.to_string()))?;

        if treaty.status != CrossWorldTreatyStatus::Proposed {
            return Err(FederationError::InvalidTreatyStatus {
                treaty_id: treaty_id.to_string(),
                expected: CrossWorldTreatyStatus::Proposed,
                actual: treaty.status,
            });
        }

        let world_id = treaty.foreign_world_id.clone();
        let treaty_type = treaty.treaty_type;

        // Determine new diplomatic status based on treaty type
        let current_status = self.foreign_worlds.get(&world_id)
            .map(|w| w.diplomatic_status)
            .unwrap_or(DiplomaticStatus::Neutral);

        let new_status = match treaty_type {
            CrossWorldTreatyType::NonAggression => DiplomaticStatus::Peace,
            CrossWorldTreatyType::TradePact => DiplomaticStatus::TradeAgreement,
            CrossWorldTreatyType::MilitaryAlliance => DiplomaticStatus::Alliance,
            CrossWorldTreatyType::ResearchExchange => {
                if current_status as u8 >= DiplomaticStatus::Peace as u8 { current_status } else { DiplomaticStatus::Peace }
            }
            CrossWorldTreatyType::CulturalExchange => DiplomaticStatus::Peace,
        };

        // Update treaty status
        let treaty = self.treaties.get_mut(treaty_id).unwrap();
        treaty.status = CrossWorldTreatyStatus::Active;
        treaty.accepted_tick = Some(tick);

        // Update world diplomatic status and relation
        if let Some(world) = self.foreign_worlds.get_mut(&world_id) {
            let old_status = world.diplomatic_status;
            world.relation_score = (world.relation_score + 10).clamp(-100, 100);
            // Only upgrade, never downgrade via treaty acceptance
            if new_status as u8 > old_status as u8 {
                world.diplomatic_status = new_status;
            }
        }

        // Emit events after mutable borrows are done
        let old_status = current_status;
        if new_status as u8 > old_status as u8 {
            self.emit(WorldEvent::DiplomaticStatusChanged {
                world_id: world_id.clone(),
                old_status,
                new_status,
            });
        }

        self.emit(WorldEvent::CrossWorldTreatySigned {
            treaty_id: treaty_id.to_string(),
            world_id: world_id.clone(),
            treaty_type: treaty_type.to_string(),
        });

        Ok(())
    }

    /// Reject a proposed treaty.
    pub fn reject_treaty(&mut self, treaty_id: &str) -> Result<(), FederationError> {
        let treaty = self.treaties.get(treaty_id)
            .ok_or_else(|| FederationError::TreatyNotFound(treaty_id.to_string()))?;

        if treaty.status != CrossWorldTreatyStatus::Proposed {
            return Err(FederationError::InvalidTreatyStatus {
                treaty_id: treaty_id.to_string(),
                expected: CrossWorldTreatyStatus::Proposed,
                actual: treaty.status,
            });
        }

        let world_id = treaty.foreign_world_id.clone();
        let treaty_type = treaty.treaty_type;

        let treaty = self.treaties.get_mut(treaty_id).unwrap();
        treaty.status = CrossWorldTreatyStatus::Rejected;

        // Minor relation penalty for rejection
        if let Some(world) = self.foreign_worlds.get_mut(&world_id) {
            world.relation_score = (world.relation_score - 5).clamp(-100, 100);
        }

        self.emit(WorldEvent::CrossWorldTreatyRejected {
            treaty_id: treaty_id.to_string(),
            world_id,
            treaty_type: treaty_type.to_string(),
        });

        Ok(())
    }

    /// Break an active treaty.
    pub fn break_treaty(&mut self, treaty_id: &str, tick: u64) -> Result<(), FederationError> {
        let treaty = self.treaties.get(treaty_id)
            .ok_or_else(|| FederationError::TreatyNotFound(treaty_id.to_string()))?;

        if treaty.status != CrossWorldTreatyStatus::Active && treaty.status != CrossWorldTreatyStatus::Proposed {
            return Err(FederationError::InvalidTreatyStatus {
                treaty_id: treaty_id.to_string(),
                expected: CrossWorldTreatyStatus::Active,
                actual: treaty.status,
            });
        }

        let world_id = treaty.foreign_world_id.clone();
        let treaty_type = treaty.treaty_type;

        let treaty = self.treaties.get_mut(treaty_id).unwrap();
        treaty.status = CrossWorldTreatyStatus::Broken;
        treaty.ended_tick = Some(tick);

        // Significant relation penalty
        if let Some(world) = self.foreign_worlds.get_mut(&world_id) {
            world.relation_score = (world.relation_score - 20).clamp(-100, 100);
        }

        self.emit(WorldEvent::CrossWorldTreatyBroken {
            treaty_id: treaty_id.to_string(),
            world_id,
            treaty_type: treaty_type.to_string(),
        });

        Ok(())
    }

    /// Get a treaty by ID.
    pub fn get_treaty(&self, treaty_id: &str) -> Option<&CrossWorldTreaty> {
        self.treaties.get(treaty_id)
    }

    /// List all treaties, optionally filtered by world_id or status.
    pub fn list_treaties(&self, world_id: Option<&str>, status: Option<CrossWorldTreatyStatus>) -> Vec<&CrossWorldTreaty> {
        self.treaties.values()
            .filter(|t| {
                let world_match = world_id.map_or(true, |w| t.foreign_world_id == w);
                let status_match = status.map_or(true, |s| t.status == s);
                world_match && status_match
            })
            .collect()
    }

    // ── Sanctions ───────────────────────────────────────

    /// Impose sanctions on a foreign world (downgrades to ColdWar).
    pub fn impose_sanctions(&mut self, world_id: &str, reason: String, tick: u64) -> Result<(), FederationError> {
        let world = self.foreign_worlds.get_mut(world_id)
            .ok_or_else(|| FederationError::WorldNotFound(world_id.to_string()))?;

        if self.sanctions.contains_key(world_id) {
            return Err(FederationError::SanctionAlreadyActive(world_id.to_string()));
        }

        let old_status = world.diplomatic_status;
        world.diplomatic_status = DiplomaticStatus::ColdWar;
        world.relation_score = (world.relation_score - 30).clamp(-100, 100);

        self.sanctions.insert(world_id.to_string(), reason.clone());

        // Break all trade and alliance treaties
        let treaties_to_break: Vec<String> = self.treaties.values()
            .filter(|t| {
                t.foreign_world_id == world_id
                    && t.status == CrossWorldTreatyStatus::Active
                    && matches!(t.treaty_type, CrossWorldTreatyType::TradePact | CrossWorldTreatyType::MilitaryAlliance)
            })
            .map(|t| t.id.clone())
            .collect();

        for tid in treaties_to_break {
            if let Some(treaty) = self.treaties.get_mut(&tid) {
                treaty.status = CrossWorldTreatyStatus::Broken;
                treaty.ended_tick = Some(tick);
            }
        }

        self.emit(WorldEvent::SanctionsImposed {
            world_id: world_id.to_string(),
            reason,
            old_status,
            new_status: DiplomaticStatus::ColdWar,
        });

        Ok(())
    }

    /// Lift sanctions on a foreign world.
    pub fn lift_sanctions(&mut self, world_id: &str) -> Result<DiplomaticStatus, FederationError> {
        if !self.sanctions.contains_key(world_id) {
            return Err(FederationError::NoActiveSanction(world_id.to_string()));
        }

        self.sanctions.remove(world_id);

        // Read current status, update, collect event data
        let (old_status, new_status, result_status) = {
            let world = self.foreign_worlds.get_mut(world_id)
                .ok_or_else(|| FederationError::WorldNotFound(world_id.to_string()))?;

            let current = world.diplomatic_status;
            if current != DiplomaticStatus::War {
                world.diplomatic_status = DiplomaticStatus::Peace;
                world.relation_score = (world.relation_score + 15).clamp(-100, 100);
                (current, DiplomaticStatus::Peace, DiplomaticStatus::Peace)
            } else {
                (current, current, current)
            }
        };

        // Emit outside the mutable borrow scope
        if old_status != new_status {
            self.emit(WorldEvent::SanctionsLifted {
                world_id: world_id.to_string(),
                old_status,
                new_status,
            });
        }

        Ok(result_status)
    }

    // ── Diplomatic Actions ──────────────────────────────

    /// Sever all diplomatic ties with a foreign world (reset to Neutral).
    pub fn sever_ties(&mut self, world_id: &str, tick: u64) -> Result<(), FederationError> {
        let world = self.foreign_worlds.get_mut(world_id)
            .ok_or_else(|| FederationError::WorldNotFound(world_id.to_string()))?;

        if world.diplomatic_status == DiplomaticStatus::War {
            return Err(FederationError::AtWar(world_id.to_string()));
        }

        let old_status = world.diplomatic_status;
        world.diplomatic_status = DiplomaticStatus::Neutral;
        world.relation_score = 0;

        // Break all active treaties
        for treaty in self.treaties.values_mut() {
            if treaty.foreign_world_id == world_id && treaty.status == CrossWorldTreatyStatus::Active {
                treaty.status = CrossWorldTreatyStatus::Broken;
                treaty.ended_tick = Some(tick);
            }
        }

        self.sanctions.remove(world_id);
        self.peace_proposals.remove(world_id);

        self.emit(WorldEvent::DiplomaticTiesSevered {
            world_id: world_id.to_string(),
            old_status,
            new_status: DiplomaticStatus::Neutral,
        });

        Ok(())
    }

    /// Declare war on a foreign world.
    pub fn declare_war(&mut self, world_id: &str, tick: u64) -> Result<(), FederationError> {
        let world = self.foreign_worlds.get_mut(world_id)
            .ok_or_else(|| FederationError::WorldNotFound(world_id.to_string()))?;

        if world.diplomatic_status == DiplomaticStatus::War {
            return Err(FederationError::InvalidDiplomaticStatus {
                world_id: world_id.to_string(),
                required: "not war".to_string(),
                actual: DiplomaticStatus::War,
            });
        }

        let old_status = world.diplomatic_status;
        world.diplomatic_status = DiplomaticStatus::War;
        world.relation_score = -100;

        // Break ALL active treaties
        for treaty in self.treaties.values_mut() {
            if treaty.foreign_world_id == world_id
                && (treaty.status == CrossWorldTreatyStatus::Active || treaty.status == CrossWorldTreatyStatus::Proposed)
            {
                treaty.status = CrossWorldTreatyStatus::Broken;
                treaty.ended_tick = Some(tick);
            }
        }

        self.sanctions.remove(world_id);
        self.peace_proposals.remove(world_id);

        self.emit(WorldEvent::WarDeclared {
            world_id: world_id.to_string(),
            old_status,
        });

        Ok(())
    }

    /// Propose peace to a world we are at war with.
    pub fn propose_peace(&mut self, world_id: &str, tick: u64) -> Result<String, FederationError> {
        let world = self.foreign_worlds.get(world_id)
            .ok_or_else(|| FederationError::WorldNotFound(world_id.to_string()))?;

        if world.diplomatic_status != DiplomaticStatus::War {
            return Err(FederationError::InvalidDiplomaticStatus {
                world_id: world_id.to_string(),
                required: "war".to_string(),
                actual: world.diplomatic_status,
            });
        }

        let treaty_id = self.allocate_treaty_id();
        let treaty = CrossWorldTreaty {
            id: treaty_id.clone(),
            foreign_world_id: world_id.to_string(),
            treaty_type: CrossWorldTreatyType::NonAggression,
            status: CrossWorldTreatyStatus::Proposed,
            proposed_tick: tick,
            accepted_tick: None,
            ended_tick: None,
            duration_ticks: Some(1000),
            terms: "peace_agreement".to_string(),
        };

        self.treaties.insert(treaty_id.clone(), treaty);
        self.peace_proposals.insert(world_id.to_string(), treaty_id.clone());

        self.emit(WorldEvent::PeaceProposed {
            world_id: world_id.to_string(),
            treaty_id: treaty_id.clone(),
        });

        Ok(treaty_id)
    }

    /// Accept a peace proposal.
    pub fn accept_peace(&mut self, world_id: &str, tick: u64) -> Result<(), FederationError> {
        let treaty_id = self.peace_proposals.remove(world_id)
            .ok_or_else(|| FederationError::NoPeaceProposal(world_id.to_string()))?;

        // Accept the treaty
        let treaty = self.treaties.get_mut(&treaty_id)
            .ok_or_else(|| FederationError::TreatyNotFound(treaty_id.clone()))?;
        treaty.status = CrossWorldTreatyStatus::Active;
        treaty.accepted_tick = Some(tick);

        // Restore to Peace
        let world = self.foreign_worlds.get_mut(world_id)
            .ok_or_else(|| FederationError::WorldNotFound(world_id.to_string()))?;
        let old_status = world.diplomatic_status;
        world.diplomatic_status = DiplomaticStatus::Peace;
        world.relation_score = 0;

        self.emit(WorldEvent::DiplomaticStatusChanged {
            world_id: world_id.to_string(),
            old_status,
            new_status: DiplomaticStatus::Peace,
        });

        self.emit(WorldEvent::PeaceEstablished {
            world_id: world_id.to_string(),
            treaty_id,
        });

        Ok(())
    }

    // ── Tick / Expiry ───────────────────────────────────

    /// Check and expire treaties that have exceeded their duration.
    pub fn tick_expiry(&mut self, current_tick: u64) -> Vec<String> {
        let mut expired = Vec::new();
        for (id, treaty) in &mut self.treaties {
            if treaty.status == CrossWorldTreatyStatus::Active {
                if let (Some(accepted), Some(duration)) = (treaty.accepted_tick, treaty.duration_ticks) {
                    if current_tick >= accepted + duration {
                        treaty.status = CrossWorldTreatyStatus::Expired;
                        treaty.ended_tick = Some(current_tick);
                        expired.push(id.clone());
                    }
                }
            }
        }

        for treaty_id in &expired {
            if let Some(treaty) = self.treaties.get(treaty_id) {
                self.emit(WorldEvent::CrossWorldTreatyExpired {
                    treaty_id: treaty_id.clone(),
                    world_id: treaty.foreign_world_id.clone(),
                    treaty_type: treaty.treaty_type.to_string(),
                });
            }
        }

        expired
    }

    // ── Summary ─────────────────────────────────────────

    /// Get a summary of the federation state.
    pub fn summary(&self) -> FederationSummary {
        let total_worlds = self.foreign_worlds.len();
        let online_worlds = self.foreign_worlds.values().filter(|w| w.online).count();
        let active_treaties = self.treaties.values().filter(|t| t.status == CrossWorldTreatyStatus::Active).count();
        let pending_treaties = self.treaties.values().filter(|t| t.status == CrossWorldTreatyStatus::Proposed).count();
        let active_sanctions = self.sanctions.len();
        let pending_peace = self.peace_proposals.len();

        let at_war_with: Vec<String> = self.foreign_worlds.values()
            .filter(|w| w.diplomatic_status == DiplomaticStatus::War)
            .map(|w| w.id.clone())
            .collect();

        let allied_with: Vec<String> = self.foreign_worlds.values()
            .filter(|w| w.diplomatic_status == DiplomaticStatus::Alliance)
            .map(|w| w.id.clone())
            .collect();

        FederationSummary {
            total_worlds,
            online_worlds,
            active_treaties,
            pending_treaties,
            active_sanctions,
            pending_peace,
            at_war_with,
            allied_with,
        }
    }

    // ── Event Emission ──────────────────────────────────

    fn emit(&self, event: WorldEvent) {
        if let Some(ref bus) = self.event_bus {
            bus.emit(event);
        }
    }
}

impl Default for FederationEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ── Summary ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FederationSummary {
    pub total_worlds: usize,
    pub online_worlds: usize,
    pub active_treaties: usize,
    pub pending_treaties: usize,
    pub active_sanctions: usize,
    pub pending_peace: usize,
    pub at_war_with: Vec<String>,
    pub allied_with: Vec<String>,
}

// ── Tests ──────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_engine() -> FederationEngine {
        FederationEngine::new()
    }

    fn make_engine_with_bus() -> (FederationEngine, crate::world::state::EventBus) {
        let bus = crate::world::state::EventBus::new(64);
        let engine = FederationEngine::with_event_bus(bus.clone());
        (engine, bus)
    }

    fn register_world(engine: &mut FederationEngine, id: &str) {
        engine.register_world(id.to_string(), format!("World-{}", id), format!("http://{}:8080", id), 0).unwrap();
    }

    // ── World Registry Tests ────────────────────────────

    #[test]
    fn test_register_world_success() {
        let mut engine = make_engine();
        register_world(&mut engine, "w1");
        assert!(engine.foreign_worlds.contains_key("w1"));
        let world = engine.get_world("w1").unwrap();
        assert_eq!(world.diplomatic_status, DiplomaticStatus::Neutral);
        assert_eq!(world.relation_score, 0);
        assert!(world.online);
    }

    #[test]
    fn test_register_world_duplicate_fails() {
        let mut engine = make_engine();
        register_world(&mut engine, "w1");
        let result = engine.register_world("w1".into(), "Dup".into(), "http://w1".into(), 0);
        assert!(matches!(result.unwrap_err(), FederationError::WorldAlreadyRegistered(_)));
    }

    #[test]
    fn test_deregister_world() {
        let mut engine = make_engine();
        register_world(&mut engine, "w1");
        engine.deregister_world("w1").unwrap();
        assert!(!engine.foreign_worlds.contains_key("w1"));
    }

    #[test]
    fn test_deregister_world_not_found() {
        let mut engine = make_engine();
        let result = engine.deregister_world("nonexistent");
        assert!(matches!(result.unwrap_err(), FederationError::WorldNotFound(_)));
    }

    #[test]
    fn test_deregister_world_removes_treaties() {
        let mut engine = make_engine();
        register_world(&mut engine, "w1");
        let tid = engine.propose_treaty("w1", CrossWorldTreatyType::TradePact, 10, None, "terms".into()).unwrap();
        engine.accept_treaty(&tid, 11).unwrap();
        engine.deregister_world("w1").unwrap();
        assert!(engine.get_treaty(&tid).is_none());
    }

    #[test]
    fn test_update_online_status() {
        let mut engine = make_engine();
        register_world(&mut engine, "w1");
        engine.update_online_status("w1", false, 5).unwrap();
        assert!(!engine.get_world("w1").unwrap().online);
        engine.update_online_status("w1", true, 10).unwrap();
        assert!(engine.get_world("w1").unwrap().online);
        assert_eq!(engine.get_world("w1").unwrap().last_seen_tick, 10);
    }

    // ── Diplomatic Relations Tests ──────────────────────

    #[test]
    fn test_establish_relations_success() {
        let mut engine = make_engine();
        register_world(&mut engine, "w1");
        engine.establish_relations("w1", 10).unwrap();
        assert_eq!(engine.get_world("w1").unwrap().diplomatic_status, DiplomaticStatus::Peace);
        assert_eq!(engine.get_world("w1").unwrap().relation_score, 10);
    }

    #[test]
    fn test_establish_relations_already_at_peace() {
        let mut engine = make_engine();
        register_world(&mut engine, "w1");
        engine.establish_relations("w1", 10).unwrap();
        let result = engine.establish_relations("w1", 20);
        assert!(matches!(result.unwrap_err(), FederationError::InvalidDiplomaticStatus { .. }));
    }

    #[test]
    fn test_establish_relations_at_war_fails() {
        let mut engine = make_engine();
        register_world(&mut engine, "w1");
        engine.declare_war("w1", 10).unwrap();
        let result = engine.establish_relations("w1", 20);
        assert!(matches!(result.unwrap_err(), FederationError::AtWar(_)));
    }

    #[test]
    fn test_adjust_relation() {
        let mut engine = make_engine();
        register_world(&mut engine, "w1");
        let new = engine.adjust_relation("w1", 25).unwrap();
        assert_eq!(new, 25);
        let new = engine.adjust_relation("w1", -10).unwrap();
        assert_eq!(new, 15);
    }

    #[test]
    fn test_adjust_relation_clamps() {
        let mut engine = make_engine();
        register_world(&mut engine, "w1");
        let new = engine.adjust_relation("w1", 200).unwrap();
        assert_eq!(new, 100);
        let new = engine.adjust_relation("w1", -300).unwrap();
        assert_eq!(new, -100);
    }

    // ── Treaty Lifecycle Tests ──────────────────────────

    #[test]
    fn test_propose_treaty_success() {
        let mut engine = make_engine();
        register_world(&mut engine, "w1");
        engine.establish_relations("w1", 10).unwrap();
        let id = engine.propose_treaty("w1", CrossWorldTreatyType::TradePact, 20, Some(100), "free_trade".into()).unwrap();
        let treaty = engine.get_treaty(&id).unwrap();
        assert_eq!(treaty.status, CrossWorldTreatyStatus::Proposed);
        assert_eq!(treaty.treaty_type, CrossWorldTreatyType::TradePact);
        assert_eq!(treaty.foreign_world_id, "w1");
        assert_eq!(treaty.duration_ticks, Some(100));
    }

    #[test]
    fn test_propose_treaty_relation_too_low() {
        let mut engine = make_engine();
        register_world(&mut engine, "w1");
        // MilitaryAlliance requires 50, but we're at 0
        let result = engine.propose_treaty("w1", CrossWorldTreatyType::MilitaryAlliance, 10, None, "alliance".into());
        assert!(matches!(result.unwrap_err(), FederationError::RelationTooLow { .. }));
    }

    #[test]
    fn test_propose_treaty_at_war_fails() {
        let mut engine = make_engine();
        register_world(&mut engine, "w1");
        engine.declare_war("w1", 10).unwrap();
        let result = engine.propose_treaty("w1", CrossWorldTreatyType::NonAggression, 20, None, "pact".into());
        assert!(matches!(result.unwrap_err(), FederationError::AtWar(_)));
    }

    #[test]
    fn test_accept_treaty_success() {
        let mut engine = make_engine();
        register_world(&mut engine, "w1");
        engine.establish_relations("w1", 10).unwrap();
        let id = engine.propose_treaty("w1", CrossWorldTreatyType::TradePact, 20, None, "trade".into()).unwrap();
        engine.accept_treaty(&id, 25).unwrap();
        let treaty = engine.get_treaty(&id).unwrap();
        assert_eq!(treaty.status, CrossWorldTreatyStatus::Active);
        assert_eq!(treaty.accepted_tick, Some(25));
        // TradePact upgrades to TradeAgreement
        assert_eq!(engine.get_world("w1").unwrap().diplomatic_status, DiplomaticStatus::TradeAgreement);
    }

    #[test]
    fn test_accept_military_alliance_upgrades_status() {
        let mut engine = make_engine();
        register_world(&mut engine, "w1");
        engine.establish_relations("w1", 10).unwrap();
        engine.adjust_relation("w1", 50).unwrap();
        let id = engine.propose_treaty("w1", CrossWorldTreatyType::MilitaryAlliance, 20, None, "ally".into()).unwrap();
        engine.accept_treaty(&id, 25).unwrap();
        assert_eq!(engine.get_world("w1").unwrap().diplomatic_status, DiplomaticStatus::Alliance);
    }

    #[test]
    fn test_reject_treaty() {
        let mut engine = make_engine();
        register_world(&mut engine, "w1");
        engine.establish_relations("w1", 10).unwrap();
        let id = engine.propose_treaty("w1", CrossWorldTreatyType::TradePact, 20, None, "trade".into()).unwrap();
        engine.reject_treaty(&id).unwrap();
        assert_eq!(engine.get_treaty(&id).unwrap().status, CrossWorldTreatyStatus::Rejected);
        // Relation penalty
        assert!(engine.get_world("w1").unwrap().relation_score < 10);
    }

    #[test]
    fn test_break_treaty() {
        let mut engine = make_engine();
        register_world(&mut engine, "w1");
        engine.establish_relations("w1", 10).unwrap();
        let id = engine.propose_treaty("w1", CrossWorldTreatyType::TradePact, 20, None, "trade".into()).unwrap();
        engine.accept_treaty(&id, 25).unwrap();
        engine.break_treaty(&id, 30).unwrap();
        assert_eq!(engine.get_treaty(&id).unwrap().status, CrossWorldTreatyStatus::Broken);
        assert_eq!(engine.get_treaty(&id).unwrap().ended_tick, Some(30));
        // Relation: establish(+10) + accept(+10) + break(-20) = 0
        assert!(engine.get_world("w1").unwrap().relation_score <= 0);
    }

    #[test]
    fn test_treaty_expiry() {
        let mut engine = make_engine();
        register_world(&mut engine, "w1");
        engine.establish_relations("w1", 10).unwrap();
        let id = engine.propose_treaty("w1", CrossWorldTreatyType::TradePact, 20, Some(50), "trade".into()).unwrap();
        engine.accept_treaty(&id, 25).unwrap();
        // Not expired yet
        let expired = engine.tick_expiry(74);
        assert!(expired.is_empty());
        // Expires at tick 75 (accepted 25 + duration 50)
        let expired = engine.tick_expiry(75);
        assert_eq!(expired.len(), 1);
        assert_eq!(engine.get_treaty(&id).unwrap().status, CrossWorldTreatyStatus::Expired);
    }

    #[test]
    fn test_duplicate_active_treaty_fails() {
        let mut engine = make_engine();
        register_world(&mut engine, "w1");
        engine.establish_relations("w1", 10).unwrap();
        engine.propose_treaty("w1", CrossWorldTreatyType::TradePact, 20, None, "trade".into()).unwrap();
        engine.accept_treaty("cw-treaty-1", 25).unwrap();
        let result = engine.propose_treaty("w1", CrossWorldTreatyType::TradePact, 30, None, "trade2".into());
        assert!(matches!(result.unwrap_err(), FederationError::TreatyAlreadyExists { .. }));
    }

    // ── Sanctions Tests ─────────────────────────────────

    #[test]
    fn test_impose_sanctions() {
        let mut engine = make_engine();
        register_world(&mut engine, "w1");
        engine.establish_relations("w1", 10).unwrap();
        engine.impose_sanctions("w1", "human_rights".into(), 20).unwrap();
        assert_eq!(engine.get_world("w1").unwrap().diplomatic_status, DiplomaticStatus::ColdWar);
        assert!(engine.sanctions.contains_key("w1"));
    }

    #[test]
    fn test_sanctions_break_trade_treaties() {
        let mut engine = make_engine();
        register_world(&mut engine, "w1");
        engine.establish_relations("w1", 10).unwrap();
        let tid = engine.propose_treaty("w1", CrossWorldTreatyType::TradePact, 15, None, "trade".into()).unwrap();
        engine.accept_treaty(&tid, 16).unwrap();
        engine.impose_sanctions("w1", "violation".into(), 20).unwrap();
        assert_eq!(engine.get_treaty(&tid).unwrap().status, CrossWorldTreatyStatus::Broken);
    }

    #[test]
    fn test_lift_sanctions() {
        let mut engine = make_engine();
        register_world(&mut engine, "w1");
        engine.establish_relations("w1", 10).unwrap();
        engine.impose_sanctions("w1", "reason".into(), 20).unwrap();
        engine.lift_sanctions("w1").unwrap();
        assert_eq!(engine.get_world("w1").unwrap().diplomatic_status, DiplomaticStatus::Peace);
        assert!(!engine.sanctions.contains_key("w1"));
    }

    #[test]
    fn test_lift_sanctions_no_active() {
        let mut engine = make_engine();
        register_world(&mut engine, "w1");
        let result = engine.lift_sanctions("w1");
        assert!(matches!(result.unwrap_err(), FederationError::NoActiveSanction(_)));
    }

    // ── Diplomatic Actions Tests ────────────────────────

    #[test]
    fn test_sever_ties() {
        let mut engine = make_engine();
        register_world(&mut engine, "w1");
        engine.establish_relations("w1", 10).unwrap();
        let tid = engine.propose_treaty("w1", CrossWorldTreatyType::TradePact, 15, None, "trade".into()).unwrap();
        engine.accept_treaty(&tid, 16).unwrap();
        engine.sever_ties("w1", 20).unwrap();
        assert_eq!(engine.get_world("w1").unwrap().diplomatic_status, DiplomaticStatus::Neutral);
        assert_eq!(engine.get_world("w1").unwrap().relation_score, 0);
        assert_eq!(engine.get_treaty(&tid).unwrap().status, CrossWorldTreatyStatus::Broken);
    }

    #[test]
    fn test_sever_ties_at_war_fails() {
        let mut engine = make_engine();
        register_world(&mut engine, "w1");
        engine.declare_war("w1", 10).unwrap();
        let result = engine.sever_ties("w1", 20);
        assert!(matches!(result.unwrap_err(), FederationError::AtWar(_)));
    }

    #[test]
    fn test_declare_war() {
        let mut engine = make_engine();
        register_world(&mut engine, "w1");
        engine.establish_relations("w1", 10).unwrap();
        let tid = engine.propose_treaty("w1", CrossWorldTreatyType::TradePact, 15, None, "trade".into()).unwrap();
        engine.accept_treaty(&tid, 16).unwrap();
        engine.declare_war("w1", 20).unwrap();
        assert_eq!(engine.get_world("w1").unwrap().diplomatic_status, DiplomaticStatus::War);
        assert_eq!(engine.get_world("w1").unwrap().relation_score, -100);
        assert_eq!(engine.get_treaty(&tid).unwrap().status, CrossWorldTreatyStatus::Broken);
    }

    #[test]
    fn test_declare_war_already_at_war() {
        let mut engine = make_engine();
        register_world(&mut engine, "w1");
        engine.declare_war("w1", 10).unwrap();
        let result = engine.declare_war("w1", 20);
        assert!(matches!(result.unwrap_err(), FederationError::InvalidDiplomaticStatus { .. }));
    }

    // ── Peace Process Tests ─────────────────────────────

    #[test]
    fn test_peace_process() {
        let mut engine = make_engine();
        register_world(&mut engine, "w1");
        engine.declare_war("w1", 10).unwrap();
        let tid = engine.propose_peace("w1", 20).unwrap();
        assert!(engine.peace_proposals.contains_key("w1"));
        engine.accept_peace("w1", 25).unwrap();
        assert_eq!(engine.get_world("w1").unwrap().diplomatic_status, DiplomaticStatus::Peace);
        assert_eq!(engine.get_world("w1").unwrap().relation_score, 0);
        assert!(!engine.peace_proposals.contains_key("w1"));
    }

    #[test]
    fn test_propose_peace_not_at_war() {
        let mut engine = make_engine();
        register_world(&mut engine, "w1");
        let result = engine.propose_peace("w1", 10);
        assert!(matches!(result.unwrap_err(), FederationError::InvalidDiplomaticStatus { .. }));
    }

    #[test]
    fn test_accept_peace_no_proposal() {
        let mut engine = make_engine();
        register_world(&mut engine, "w1");
        let result = engine.accept_peace("w1", 10);
        assert!(matches!(result.unwrap_err(), FederationError::NoPeaceProposal(_)));
    }

    // ── Summary Tests ───────────────────────────────────

    #[test]
    fn test_summary() {
        let mut engine = make_engine();
        register_world(&mut engine, "w1");
        register_world(&mut engine, "w2");
        engine.establish_relations("w1", 10).unwrap();
        engine.adjust_relation("w1", 50).unwrap();
        let tid = engine.propose_treaty("w1", CrossWorldTreatyType::MilitaryAlliance, 20, None, "ally".into()).unwrap();
        engine.accept_treaty(&tid, 21).unwrap();
        engine.declare_war("w2", 15).unwrap();

        let summary = engine.summary();
        assert_eq!(summary.total_worlds, 2);
        assert_eq!(summary.active_treaties, 1);
        assert_eq!(summary.at_war_with, vec!["w2"]);
        assert_eq!(summary.allied_with, vec!["w1"]);
    }

    // ── Event Bus Integration Tests ─────────────────────

    #[test]
    fn test_event_bus_world_discovered() {
        let (mut engine, bus) = make_engine_with_bus();
        let mut rx = bus.subscribe();
        engine.register_world("w1".into(), "World1".into(), "http://w1:8080".into(), 0).unwrap();
        let event = rx.try_recv().unwrap();
        match event {
            WorldEvent::ForeignWorldDiscovered { world_id, name, .. } => {
                assert_eq!(world_id, "w1");
                assert_eq!(name, "World1");
            }
            _ => panic!("Expected ForeignWorldDiscovered event"),
        }
    }

    #[test]
    fn test_event_bus_treaty_signed() {
        let (mut engine, bus) = make_engine_with_bus();
        let mut rx = bus.subscribe();
        register_world(&mut engine, "w1");
        engine.establish_relations("w1", 10).unwrap();
        // Drain establish events
        while rx.try_recv().is_ok() {}

        let id = engine.propose_treaty("w1", CrossWorldTreatyType::TradePact, 20, None, "trade".into()).unwrap();
        // Drain proposal event
        while rx.try_recv().is_ok() {}

        engine.accept_treaty(&id, 25).unwrap();
        // Should get CrossWorldRelationChanged, DiplomaticStatusChanged, CrossWorldTreatySigned
        let mut found_signed = false;
        while let Ok(event) = rx.try_recv() {
            if let WorldEvent::CrossWorldTreatySigned { treaty_id, world_id, treaty_type } = event {
                assert_eq!(treaty_id, id);
                assert_eq!(world_id, "w1");
                assert_eq!(treaty_type, "trade_pact");
                found_signed = true;
            }
        }
        assert!(found_signed);
    }

    #[test]
    fn test_event_bus_war_declared() {
        let (mut engine, bus) = make_engine_with_bus();
        let mut rx = bus.subscribe();
        register_world(&mut engine, "w1");
        engine.establish_relations("w1", 10).unwrap();
        // Drain events
        while rx.try_recv().is_ok() {}

        engine.declare_war("w1", 20).unwrap();
        let mut found_war = false;
        while let Ok(event) = rx.try_recv() {
            if let WorldEvent::WarDeclared { world_id, old_status } = event {
                assert_eq!(world_id, "w1");
                assert_eq!(old_status, DiplomaticStatus::Peace);
                found_war = true;
            }
        }
        assert!(found_war);
    }

    // ── Full Diplomatic Workflow Test ───────────────────

    #[test]
    fn test_full_diplomatic_workflow() {
        let mut engine = make_engine();

        // Register two foreign worlds
        register_world(&mut engine, "earth");
        register_world(&mut engine, "mars");

        // Establish relations with Earth
        engine.establish_relations("earth", 100).unwrap();
        assert_eq!(engine.get_world("earth").unwrap().diplomatic_status, DiplomaticStatus::Peace);

        // Build up relations and form trade pact
        engine.adjust_relation("earth", 20).unwrap();
        let trade_id = engine.propose_treaty("earth", CrossWorldTreatyType::TradePact, 110, Some(500), "free_trade".into()).unwrap();
        engine.accept_treaty(&trade_id, 115).unwrap();
        assert_eq!(engine.get_world("earth").unwrap().diplomatic_status, DiplomaticStatus::TradeAgreement);

        // Build alliance with Earth
        engine.adjust_relation("earth", 30).unwrap();
        let alliance_id = engine.propose_treaty("earth", CrossWorldTreatyType::MilitaryAlliance, 200, None, "mutual_defense".into()).unwrap();
        engine.accept_treaty(&alliance_id, 205).unwrap();
        assert_eq!(engine.get_world("earth").unwrap().diplomatic_status, DiplomaticStatus::Alliance);

        // Declare war on Mars
        engine.declare_war("mars", 300).unwrap();
        assert_eq!(engine.get_world("mars").unwrap().diplomatic_status, DiplomaticStatus::War);

        // Peace process with Mars
        let peace_id = engine.propose_peace("mars", 400).unwrap();
        engine.accept_peace("mars", 410).unwrap();
        assert_eq!(engine.get_world("mars").unwrap().diplomatic_status, DiplomaticStatus::Peace);

        // Sanctions on Mars
        engine.impose_sanctions("mars", "espionage".into(), 500).unwrap();
        assert_eq!(engine.get_world("mars").unwrap().diplomatic_status, DiplomaticStatus::ColdWar);
        engine.lift_sanctions("mars").unwrap();
        assert_eq!(engine.get_world("mars").unwrap().diplomatic_status, DiplomaticStatus::Peace);

        // Trade pact expiry
        let expired = engine.tick_expiry(615);
        assert_eq!(expired.len(), 1);
        assert_eq!(engine.get_treaty(&trade_id).unwrap().status, CrossWorldTreatyStatus::Expired);

        // Verify summary
        let summary = engine.summary();
        assert_eq!(summary.total_worlds, 2);
        assert_eq!(summary.active_treaties, 2); // alliance + peace treaty
        assert_eq!(summary.allied_with, vec!["earth"]);
        assert!(summary.at_war_with.is_empty());
    }
}
