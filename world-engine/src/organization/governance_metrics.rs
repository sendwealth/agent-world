//! Governance metrics collection engine.
//!
//! Subscribes to the EventBus and aggregates governance-related metrics
//! (elections, taxation, diplomacy, organization health) without modifying
//! the core tick loop. Collection is fully asynchronous.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::world::event::{EventType, WorldEvent};
use crate::world::state::EventBus;

// ── Metrics data structures ────────────────────────────────────────────────

/// Governance event record for timeline queries.
#[derive(Debug, Clone)]
pub struct GovernanceEvent {
    pub event_type: EventType,
    pub org_id: Uuid,
    pub tick: u64,
    pub summary: String,
}

/// Per-organization metrics snapshot.
#[derive(Debug, Clone)]
pub struct OrgMetrics {
    pub org_id: Uuid,
    // Election metrics
    pub election_count: usize,
    pub avg_participation_rate: f64,
    pub avg_candidate_count: f64,
    pub avg_term_length_ticks: f64,
    // Tax metrics
    pub total_tax_collected: u64,
    pub tax_per_member: f64,
    pub tax_collection_count: usize,
    pub treasury_balance: i64,
    // Diplomacy metrics
    pub treaties_signed: usize,
    pub treaties_broken: usize,
    pub active_relations_count: usize,
    // Organization health
    pub member_count: usize,
    pub governance_stability_score: f64, // 0.0-1.0
}

/// World-wide governance summary across all organizations.
#[derive(Debug, Clone)]
pub struct WorldGovernanceSummary {
    pub total_orgs: usize,
    pub avg_stability: f64,
    pub total_tax_collected: u64,
    pub total_treaties: usize,
    pub election_activity_rate: f64,
}

// ── Internal per-org accumulator ───────────────────────────────────────────

/// Internal mutable state for tracking per-org governance metrics.
#[derive(Debug)]
struct OrgAccumulator {
    // Election tracking
    election_count: usize,
    total_candidates: usize,
    leader_changes: usize,
    term_lengths: Vec<u64>,

    // Tax tracking
    total_tax_collected: u64,
    tax_collection_count: usize,
    total_distributed: u64,

    // Diplomacy tracking
    treaties_signed: usize,
    treaties_broken: usize,
    relation_partners: std::collections::HashSet<String>,

    // Membership tracking
    member_count: usize,

    // Timeline
    timeline: Vec<GovernanceEvent>,
}

impl OrgAccumulator {
    fn new() -> Self {
        Self {
            election_count: 0,
            total_candidates: 0,
            leader_changes: 0,
            term_lengths: Vec::new(),
            total_tax_collected: 0,
            tax_collection_count: 0,
            total_distributed: 0,
            treaties_signed: 0,
            treaties_broken: 0,
            relation_partners: std::collections::HashSet::new(),
            member_count: 0,
            timeline: Vec::new(),
        }
    }
}

// ── Collector ──────────────────────────────────────────────────────────────

/// Asynchronous governance metrics collector.
///
/// Spawns a background tokio task that subscribes to the EventBus and
/// aggregates governance-related events into per-org metrics. Query methods
/// are synchronous and read from an `Arc<Mutex>` protected state.
pub struct GovernanceMetricsCollector {
    org_data: Arc<std::sync::Mutex<HashMap<Uuid, OrgAccumulator>>>,
    _handle: JoinHandle<()>,
}

impl GovernanceMetricsCollector {
    /// Create a new collector and spawn the background subscription task.
    pub fn new(event_bus: &EventBus) -> Self {
        let org_data: Arc<std::sync::Mutex<HashMap<Uuid, OrgAccumulator>>> =
            Arc::new(std::sync::Mutex::new(HashMap::new()));

        let filtered = event_bus.subscribe_filtered(
            vec![
                EventType::TaxCollected,
                EventType::TreasuryDistributed,
                EventType::LeadershipElectionStarted,
                EventType::LeadershipChanged,
                EventType::TreatyProposed,
                EventType::TreatySigned,
                EventType::TreatyBroken,
                EventType::RelationChanged,
                EventType::OrganizationMemberJoined,
                EventType::OrganizationMemberLeft,
                EventType::OrgMemberJoined,
                EventType::OrgMemberLeft,
            ],
            None,
        );

        let data = Arc::clone(&org_data);
        let handle = tokio::spawn(async move {
            collect_loop(filtered, data).await;
        });

        Self {
            org_data,
            _handle: handle,
        }
    }

    /// Get metrics snapshot for a specific organization.
    pub fn get_org_metrics(&self, org_id: Uuid) -> OrgMetrics {
        let data = self.org_data.lock().unwrap();
        let acc = data.get(&org_id);

        match acc {
            Some(a) => build_org_metrics(org_id, a),
            None => OrgMetrics {
                org_id,
                election_count: 0,
                avg_participation_rate: 0.0,
                avg_candidate_count: 0.0,
                avg_term_length_ticks: 0.0,
                total_tax_collected: 0,
                tax_per_member: 0.0,
                tax_collection_count: 0,
                treasury_balance: 0,
                treaties_signed: 0,
                treaties_broken: 0,
                active_relations_count: 0,
                member_count: 0,
                governance_stability_score: 0.0,
            },
        }
    }

    /// Get a world-wide governance summary across all tracked organizations.
    pub fn get_world_governance_summary(&self) -> WorldGovernanceSummary {
        let data = self.org_data.lock().unwrap();

        let total_orgs = data.len();
        if total_orgs == 0 {
            return WorldGovernanceSummary {
                total_orgs: 0,
                avg_stability: 0.0,
                total_tax_collected: 0,
                total_treaties: 0,
                election_activity_rate: 0.0,
            };
        }

        let mut total_stability = 0.0_f64;
        let mut total_tax: u64 = 0;
        let mut total_treaties: usize = 0;
        let mut orgs_with_elections: usize = 0;

        for (org_id, acc) in data.iter() {
            let metrics = build_org_metrics(*org_id, acc);
            total_stability += metrics.governance_stability_score;
            total_tax += metrics.total_tax_collected;
            total_treaties += metrics.treaties_signed;
            if metrics.election_count > 0 {
                orgs_with_elections += 1;
            }
        }

        WorldGovernanceSummary {
            total_orgs,
            avg_stability: total_stability / total_orgs as f64,
            total_tax_collected: total_tax,
            total_treaties,
            election_activity_rate: orgs_with_elections as f64 / total_orgs as f64,
        }
    }

    /// Query the governance event timeline for an organization.
    ///
    /// Optionally filter by event type and/or tick range `(start, end)`.
    pub fn get_timeline(
        &self,
        org_id: Uuid,
        event_type: Option<EventType>,
        range: (u64, u64),
    ) -> Vec<GovernanceEvent> {
        let data = self.org_data.lock().unwrap();
        match data.get(&org_id) {
            Some(acc) => acc
                .timeline
                .iter()
                .filter(|e| {
                    let type_match = event_type.map_or(true, |t| e.event_type == t);
                    let range_match = e.tick >= range.0 && e.tick <= range.1;
                    type_match && range_match
                })
                .cloned()
                .collect(),
            None => Vec::new(),
        }
    }
}

// ── Background collection loop ─────────────────────────────────────────────

async fn collect_loop(
    mut rx: crate::world::state::FilteredReceiver,
    data: Arc<std::sync::Mutex<HashMap<Uuid, OrgAccumulator>>>,
) {
    loop {
        match rx.recv().await {
            Ok(event) => process_event(&data, event),
            Err(broadcast::error::RecvError::Lagged(n)) => {
                tracing::warn!(
                    n,
                    "GovernanceMetricsCollector lagged, dropped {} events",
                    n
                );
            }
            Err(broadcast::error::RecvError::Closed) => break,
        }
    }
}

fn process_event(
    data: &Arc<std::sync::Mutex<HashMap<Uuid, OrgAccumulator>>>,
    event: WorldEvent,
) {
    match &event {
        WorldEvent::TaxCollected {
            org_id,
            tax_amount,
            tick,
            ..
        } => {
            let org_uuid = parse_org_id(org_id);
            let tax = *tax_amount;
            let t = *tick;
            let mut data = data.lock().unwrap();
            let acc = data.entry(org_uuid).or_insert_with(OrgAccumulator::new);
            acc.total_tax_collected += tax;
            acc.tax_collection_count += 1;
            acc.timeline.push(GovernanceEvent {
                event_type: EventType::TaxCollected,
                org_id: org_uuid,
                tick: t,
                summary: format!("Tax collected: {}", tax),
            });
        }

        WorldEvent::TreasuryDistributed {
            org_id,
            total_amount,
            tick,
            ..
        } => {
            let org_uuid = parse_org_id(org_id);
            let amount = *total_amount;
            let t = *tick;
            let mut data = data.lock().unwrap();
            let acc = data.entry(org_uuid).or_insert_with(OrgAccumulator::new);
            acc.total_distributed += amount;
            acc.timeline.push(GovernanceEvent {
                event_type: EventType::TreasuryDistributed,
                org_id: org_uuid,
                tick: t,
                summary: format!("Treasury distributed: {}", amount),
            });
        }

        WorldEvent::LeadershipElectionStarted {
            org_id,
            candidates,
            ..
        } => {
            let org_uuid = *org_id;
            let count = candidates.len();
            let mut data = data.lock().unwrap();
            let acc = data.entry(org_uuid).or_insert_with(OrgAccumulator::new);
            acc.election_count += 1;
            acc.total_candidates += count;
            acc.timeline.push(GovernanceEvent {
                event_type: EventType::LeadershipElectionStarted,
                org_id: org_uuid,
                tick: 0, // election started events don't carry tick
                summary: format!("Election started with {} candidates", count),
            });
        }

        WorldEvent::LeadershipChanged {
            org_id,
            old_leader_id,
            new_leader_id,
        } => {
            let org_uuid = *org_id;
            let mut data = data.lock().unwrap();
            let acc = data.entry(org_uuid).or_insert_with(OrgAccumulator::new);
            acc.leader_changes += 1;
            acc.timeline.push(GovernanceEvent {
                event_type: EventType::LeadershipChanged,
                org_id: org_uuid,
                tick: 0, // leadership changed events don't carry tick
                summary: format!(
                    "Leadership changed: {:?} -> {}",
                    old_leader_id, new_leader_id
                ),
            });
        }

        WorldEvent::TreatySigned {
            treaty_id,
            org_a,
            org_b,
        } => {
            let uuid_a = parse_org_id(org_a);
            let uuid_b = parse_org_id(org_b);
            let tid = treaty_id.clone();
            let mut data = data.lock().unwrap();

            let acc_a = data.entry(uuid_a).or_insert_with(OrgAccumulator::new);
            acc_a.treaties_signed += 1;
            acc_a.relation_partners.insert(org_b.clone());
            acc_a.timeline.push(GovernanceEvent {
                event_type: EventType::TreatySigned,
                org_id: uuid_a,
                tick: 0,
                summary: format!("Treaty signed: {} with {}", tid, org_b),
            });

            let acc_b = data.entry(uuid_b).or_insert_with(OrgAccumulator::new);
            acc_b.treaties_signed += 1;
            acc_b.relation_partners.insert(org_a.clone());
            acc_b.timeline.push(GovernanceEvent {
                event_type: EventType::TreatySigned,
                org_id: uuid_b,
                tick: 0,
                summary: format!("Treaty signed: {} with {}", tid, org_a),
            });
        }

        WorldEvent::TreatyBroken {
            treaty_id,
            breaker,
            reason,
        } => {
            let tid = treaty_id.clone();
            let brk = breaker.clone();
            let rsn = reason.clone();
            // We don't know the other org from this event alone,
            // so we record it against a UUID derived from the breaker.
            // The breaker field is an org_id string.
            let breaker_uuid = parse_org_id(&brk);
            let mut data = data.lock().unwrap();
            let acc = data.entry(breaker_uuid).or_insert_with(OrgAccumulator::new);
            acc.treaties_broken += 1;
            acc.timeline.push(GovernanceEvent {
                event_type: EventType::TreatyBroken,
                org_id: breaker_uuid,
                tick: 0,
                summary: format!("Treaty broken: {} reason: {}", tid, rsn),
            });
        }

        WorldEvent::TreatyProposed {
            treaty_id,
            org_a,
            org_b,
            treaty_type,
        } => {
            let uuid_a = parse_org_id(org_a);
            let uuid_b = parse_org_id(org_b);
            let tid = treaty_id.clone();
            let tt = treaty_type.clone();
            let mut data = data.lock().unwrap();

            let acc_a = data.entry(uuid_a).or_insert_with(OrgAccumulator::new);
            acc_a.timeline.push(GovernanceEvent {
                event_type: EventType::TreatyProposed,
                org_id: uuid_a,
                tick: 0,
                summary: format!("Treaty proposed: {} ({}) with {}", tid, tt, org_b),
            });

            let acc_b = data.entry(uuid_b).or_insert_with(OrgAccumulator::new);
            acc_b.timeline.push(GovernanceEvent {
                event_type: EventType::TreatyProposed,
                org_id: uuid_b,
                tick: 0,
                summary: format!("Treaty proposed: {} ({}) with {}", tid, tt, org_a),
            });
        }

        WorldEvent::RelationChanged {
            org_a,
            org_b,
            old_level,
            new_level,
        } => {
            let uuid_a = parse_org_id(org_a);
            let uuid_b = parse_org_id(org_b);
            let ol = *old_level;
            let nl = *new_level;
            let b_str = org_b.clone();
            let a_str = org_a.clone();
            let mut data = data.lock().unwrap();

            let acc_a = data.entry(uuid_a).or_insert_with(OrgAccumulator::new);
            acc_a.relation_partners.insert(org_b.clone());
            acc_a.timeline.push(GovernanceEvent {
                event_type: EventType::RelationChanged,
                org_id: uuid_a,
                tick: 0,
                summary: format!("Relation with {} changed: {} -> {}", b_str, ol, nl),
            });

            let acc_b = data.entry(uuid_b).or_insert_with(OrgAccumulator::new);
            acc_b.relation_partners.insert(org_a.clone());
            acc_b.timeline.push(GovernanceEvent {
                event_type: EventType::RelationChanged,
                org_id: uuid_b,
                tick: 0,
                summary: format!("Relation with {} changed: {} -> {}", a_str, ol, nl),
            });
        }

        WorldEvent::OrganizationMemberJoined { org_id, agent_id, .. } => {
            let org_uuid = *org_id;
            let aid = agent_id.clone();
            let mut data = data.lock().unwrap();
            let acc = data.entry(org_uuid).or_insert_with(OrgAccumulator::new);
            acc.member_count += 1;
            acc.timeline.push(GovernanceEvent {
                event_type: EventType::OrganizationMemberJoined,
                org_id: org_uuid,
                tick: 0,
                summary: format!("Member joined: {}", aid),
            });
        }

        WorldEvent::OrganizationMemberLeft { org_id, agent_id } => {
            let org_uuid = *org_id;
            let aid = agent_id.clone();
            let mut data = data.lock().unwrap();
            let acc = data.entry(org_uuid).or_insert_with(OrgAccumulator::new);
            acc.member_count = acc.member_count.saturating_sub(1);
            acc.timeline.push(GovernanceEvent {
                event_type: EventType::OrganizationMemberLeft,
                org_id: org_uuid,
                tick: 0,
                summary: format!("Member left: {}", aid),
            });
        }

        WorldEvent::OrgMemberJoined {
            org_id,
            agent_id,
            total_members,
            ..
        } => {
            let org_uuid = parse_org_id(org_id);
            let aid = agent_id.clone();
            let total = *total_members;
            let mut data = data.lock().unwrap();
            let acc = data.entry(org_uuid).or_insert_with(OrgAccumulator::new);
            acc.member_count = total;
            acc.timeline.push(GovernanceEvent {
                event_type: EventType::OrgMemberJoined,
                org_id: org_uuid,
                tick: 0,
                summary: format!("Member joined: {} (total: {})", aid, total),
            });
        }

        WorldEvent::OrgMemberLeft {
            org_id,
            agent_id,
            remaining_members,
        } => {
            let org_uuid = parse_org_id(org_id);
            let aid = agent_id.clone();
            let remaining = *remaining_members;
            let mut data = data.lock().unwrap();
            let acc = data.entry(org_uuid).or_insert_with(OrgAccumulator::new);
            acc.member_count = remaining;
            acc.timeline.push(GovernanceEvent {
                event_type: EventType::OrgMemberLeft,
                org_id: org_uuid,
                tick: 0,
                summary: format!("Member left: {} (remaining: {})", aid, remaining),
            });
        }

        _ => {} // Ignore other events
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────

/// Parse an org_id that may be a UUID string into a proper Uuid.
/// Falls back to a hash-based UUID for non-UUID strings to ensure
/// consistent lookups across events.
fn parse_org_id(id: &str) -> Uuid {
    Uuid::parse_str(id).unwrap_or_else(|_| {
        // Deterministic UUID from string bytes via simple hash
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        id.hash(&mut hasher);
        let hash = hasher.finish();
        Uuid::from_u64_pair(hash, hash.wrapping_add(1))
    })
}

/// Build an `OrgMetrics` snapshot from the internal accumulator.
fn build_org_metrics(org_id: Uuid, acc: &OrgAccumulator) -> OrgMetrics {
    let avg_candidate_count = if acc.election_count > 0 {
        acc.total_candidates as f64 / acc.election_count as f64
    } else {
        0.0
    };

    let avg_term_length = if acc.term_lengths.is_empty() {
        0.0
    } else {
        acc.term_lengths.iter().sum::<u64>() as f64 / acc.term_lengths.len() as f64
    };

    // Approximate participation: use leader_changes as a proxy for resolved elections.
    let avg_participation_rate = if acc.election_count > 0 {
        // We estimate participation from leader changes vs elections
        let resolution_rate = acc.leader_changes as f64 / acc.election_count as f64;
        resolution_rate.min(1.0)
    } else {
        0.0
    };

    let treasury_balance =
        acc.total_tax_collected as i64 - acc.total_distributed as i64;

    let tax_per_member = if acc.member_count > 0 {
        acc.total_tax_collected as f64 / acc.member_count as f64
    } else {
        0.0
    };

    // Governance stability score (0.0-1.0):
    // High stability = low treaty breaks relative to signings + steady membership
    let diplomacy_health = if acc.treaties_signed + acc.treaties_broken > 0 {
        let ratio = acc.treaties_signed as f64
            / (acc.treaties_signed + acc.treaties_broken) as f64;
        ratio
    } else {
        0.5 // neutral when no diplomacy data
    };

    // Leader turnover: very high turnover lowers stability
    let leadership_stability = if acc.leader_changes > 0 && acc.election_count > 0 {
        let turnover_ratio = acc.leader_changes as f64 / acc.election_count as f64;
        (1.0 - (turnover_ratio - 1.0).abs() * 0.2).clamp(0.0, 1.0)
    } else if acc.leader_changes == 0 {
        1.0 // no changes = stable
    } else {
        0.5
    };

    let governance_stability_score =
        (diplomacy_health * 0.5 + leadership_stability * 0.5).clamp(0.0, 1.0);

    OrgMetrics {
        org_id,
        election_count: acc.election_count,
        avg_participation_rate,
        avg_candidate_count,
        avg_term_length_ticks: avg_term_length,
        total_tax_collected: acc.total_tax_collected,
        tax_per_member,
        tax_collection_count: acc.tax_collection_count,
        treasury_balance,
        treaties_signed: acc.treaties_signed,
        treaties_broken: acc.treaties_broken,
        active_relations_count: acc.relation_partners.len(),
        member_count: acc.member_count,
        governance_stability_score,
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create an EventBus + collector and return both.
    /// Must be called within a tokio runtime context.
    fn setup() -> (EventBus, GovernanceMetricsCollector) {
        let bus = EventBus::new(256);
        let collector = GovernanceMetricsCollector::new(&bus);
        // Give the background task time to subscribe
        std::thread::sleep(std::time::Duration::from_millis(10));
        (bus, collector)
    }

    #[tokio::test]
    async fn test_tax_collection_metrics() {
        let (bus, collector) = setup();
        let org_id = Uuid::new_v4();

        bus.emit(WorldEvent::TaxCollected {
            org_id: org_id.to_string(),
            payer_id: "agent-1".to_string(),
            tax_kind: "IncomeTax".to_string(),
            rate: 0.1,
            gross_amount: 1000,
            tax_amount: 100,
            tick: 10,
        });
        bus.emit(WorldEvent::TaxCollected {
            org_id: org_id.to_string(),
            payer_id: "agent-2".to_string(),
            tax_kind: "WealthTax".to_string(),
            rate: 0.02,
            gross_amount: 5000,
            tax_amount: 100,
            tick: 20,
        });

        tokio::time::sleep(std::time::Duration::from_millis(20)).await;

        let metrics = collector.get_org_metrics(org_id);
        assert_eq!(metrics.total_tax_collected, 200);
        assert_eq!(metrics.tax_collection_count, 2);
        assert_eq!(metrics.org_id, org_id);
    }

    #[tokio::test]
    async fn test_election_and_leadership_metrics() {
        let (bus, collector) = setup();
        let org_id = Uuid::new_v4();

        bus.emit(WorldEvent::LeadershipElectionStarted {
            org_id,
            candidates: vec!["a".to_string(), "b".to_string(), "c".to_string()],
            voting_method: "SimpleMajority".to_string(),
        });
        bus.emit(WorldEvent::LeadershipChanged {
            org_id,
            old_leader_id: None,
            new_leader_id: "a".to_string(),
        });

        tokio::time::sleep(std::time::Duration::from_millis(20)).await;

        let metrics = collector.get_org_metrics(org_id);
        assert_eq!(metrics.election_count, 1);
        assert!(metrics.avg_candidate_count > 0.0);
        assert_eq!(metrics.avg_candidate_count, 3.0);
    }

    #[tokio::test]
    async fn test_diplomacy_metrics() {
        let (bus, collector) = setup();
        let org_a = Uuid::new_v4();
        let org_b = Uuid::new_v4();

        bus.emit(WorldEvent::TreatyProposed {
            treaty_id: "t-1".to_string(),
            org_a: org_a.to_string(),
            org_b: org_b.to_string(),
            treaty_type: "TradeAgreement".to_string(),
        });
        bus.emit(WorldEvent::TreatySigned {
            treaty_id: "t-1".to_string(),
            org_a: org_a.to_string(),
            org_b: org_b.to_string(),
        });
        bus.emit(WorldEvent::RelationChanged {
            org_a: org_a.to_string(),
            org_b: org_b.to_string(),
            old_level: 0,
            new_level: 2,
        });

        tokio::time::sleep(std::time::Duration::from_millis(20)).await;

        let metrics_a = collector.get_org_metrics(org_a);
        assert_eq!(metrics_a.treaties_signed, 1);
        assert_eq!(metrics_a.active_relations_count, 1);

        let metrics_b = collector.get_org_metrics(org_b);
        assert_eq!(metrics_b.treaties_signed, 1);
        assert_eq!(metrics_b.active_relations_count, 1);
    }

    #[tokio::test]
    async fn test_treaty_broken_and_stability() {
        let (bus, collector) = setup();
        let org_id = Uuid::new_v4();

        bus.emit(WorldEvent::TreatySigned {
            treaty_id: "t-1".to_string(),
            org_a: org_id.to_string(),
            org_b: Uuid::new_v4().to_string(),
        });
        bus.emit(WorldEvent::TreatyBroken {
            treaty_id: "t-1".to_string(),
            breaker: org_id.to_string(),
            reason: "violation".to_string(),
        });

        tokio::time::sleep(std::time::Duration::from_millis(20)).await;

        let metrics = collector.get_org_metrics(org_id);
        assert_eq!(metrics.treaties_signed, 1);
        assert_eq!(metrics.treaties_broken, 1);
        // Stability should be reduced due to broken treaty
        assert!(metrics.governance_stability_score < 1.0);
    }

    #[tokio::test]
    async fn test_member_tracking_and_tax_per_member() {
        let (bus, collector) = setup();
        let org_id = Uuid::new_v4();

        // Simulate members joining via OrganizationMemberJoined (Uuid variant)
        bus.emit(WorldEvent::OrganizationMemberJoined {
            org_id,
            agent_id: "a1".to_string(),
            role: "Member".to_string(),
        });
        bus.emit(WorldEvent::OrganizationMemberJoined {
            org_id,
            agent_id: "a2".to_string(),
            role: "Member".to_string(),
        });
        bus.emit(WorldEvent::OrganizationMemberJoined {
            org_id,
            agent_id: "a3".to_string(),
            role: "Member".to_string(),
        });

        // Collect some tax
        bus.emit(WorldEvent::TaxCollected {
            org_id: org_id.to_string(),
            payer_id: "a1".to_string(),
            tax_kind: "IncomeTax".to_string(),
            rate: 0.1,
            gross_amount: 300,
            tax_amount: 30,
            tick: 5,
        });

        tokio::time::sleep(std::time::Duration::from_millis(20)).await;

        let metrics = collector.get_org_metrics(org_id);
        assert_eq!(metrics.member_count, 3);
        assert_eq!(metrics.total_tax_collected, 30);
        assert!((metrics.tax_per_member - 10.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_world_summary_aggregation() {
        let (bus, collector) = setup();
        let org_a = Uuid::new_v4();
        let org_b = Uuid::new_v4();

        bus.emit(WorldEvent::TaxCollected {
            org_id: org_a.to_string(),
            payer_id: "p1".to_string(),
            tax_kind: "IncomeTax".to_string(),
            rate: 0.1,
            gross_amount: 100,
            tax_amount: 10,
            tick: 1,
        });
        bus.emit(WorldEvent::TaxCollected {
            org_id: org_b.to_string(),
            payer_id: "p2".to_string(),
            tax_kind: "IncomeTax".to_string(),
            rate: 0.1,
            gross_amount: 200,
            tax_amount: 20,
            tick: 2,
        });
        bus.emit(WorldEvent::LeadershipElectionStarted {
            org_id: org_a,
            candidates: vec!["c1".to_string()],
            voting_method: "SimpleMajority".to_string(),
        });
        bus.emit(WorldEvent::TreatySigned {
            treaty_id: "t-1".to_string(),
            org_a: org_a.to_string(),
            org_b: org_b.to_string(),
        });

        tokio::time::sleep(std::time::Duration::from_millis(20)).await;

        let summary = collector.get_world_governance_summary();
        assert_eq!(summary.total_orgs, 2);
        assert_eq!(summary.total_tax_collected, 30);
        // org_a signed a treaty, org_b signed a treaty, so total = 2
        assert_eq!(summary.total_treaties, 2);
        // Only org_a had an election
        assert!((summary.election_activity_rate - 0.5).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_timeline_query_with_filter() {
        let (bus, collector) = setup();
        let org_id = Uuid::new_v4();

        bus.emit(WorldEvent::TaxCollected {
            org_id: org_id.to_string(),
            payer_id: "p1".to_string(),
            tax_kind: "IncomeTax".to_string(),
            rate: 0.1,
            gross_amount: 100,
            tax_amount: 10,
            tick: 5,
        });
        bus.emit(WorldEvent::TaxCollected {
            org_id: org_id.to_string(),
            payer_id: "p2".to_string(),
            tax_kind: "IncomeTax".to_string(),
            rate: 0.1,
            gross_amount: 200,
            tax_amount: 20,
            tick: 15,
        });
        bus.emit(WorldEvent::LeadershipElectionStarted {
            org_id,
            candidates: vec!["c1".to_string()],
            voting_method: "SimpleMajority".to_string(),
        });

        tokio::time::sleep(std::time::Duration::from_millis(20)).await;

        // Query all events in range [0, 10]: tick 5 TaxCollected + tick 0 LeadershipElectionStarted
        let timeline = collector.get_timeline(org_id, None, (0, 10));
        assert_eq!(timeline.len(), 2);
        let tax_in_range: Vec<_> = timeline.iter().filter(|e| e.tick == 5).collect();
        assert_eq!(tax_in_range.len(), 1);

        // Query only TaxCollected events in full range
        let tax_events = collector.get_timeline(org_id, Some(EventType::TaxCollected), (0, 100));
        assert_eq!(tax_events.len(), 2);

        // Query LeadershipElectionStarted — tick is 0 since event doesn't carry tick
        let election_events =
            collector.get_timeline(org_id, Some(EventType::LeadershipElectionStarted), (0, 0));
        assert_eq!(election_events.len(), 1);
    }

    #[tokio::test]
    async fn test_org_member_events_with_string_org_id() {
        let (bus, collector) = setup();
        // Use a plain string org_id that gets namespace-derived to a Uuid
        let org_id_str = "org-test-123";
        let org_uuid = parse_org_id(org_id_str);

        bus.emit(WorldEvent::OrgMemberJoined {
            org_id: org_id_str.to_string(),
            agent_id: "a1".to_string(),
            agent_name: "Alice".to_string(),
            role: "Member".to_string(),
            total_members: 1,
        });
        bus.emit(WorldEvent::OrgMemberJoined {
            org_id: org_id_str.to_string(),
            agent_id: "a2".to_string(),
            agent_name: "Bob".to_string(),
            role: "Member".to_string(),
            total_members: 2,
        });
        bus.emit(WorldEvent::OrgMemberLeft {
            org_id: org_id_str.to_string(),
            agent_id: "a1".to_string(),
            remaining_members: 1,
        });

        tokio::time::sleep(std::time::Duration::from_millis(20)).await;

        let metrics = collector.get_org_metrics(org_uuid);
        assert_eq!(metrics.member_count, 1);
    }
}
