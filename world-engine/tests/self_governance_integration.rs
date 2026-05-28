//! Phase 4.4.2 Integration Tests — Self-Governance (Treasury / Leadership / Diplomacy).
//!
//! Validates end-to-end integration of the three governance subsystems within
//! a simulated tick loop:
//! 1. 3 organizations are created via OrganizationStore
//! 2. A 200+ tick simulation exercises treasury collection, leadership elections,
//!    and diplomacy treaty lifecycle
//! 3. Verifies that all expected WorldEvent variants are emitted

use std::sync::Arc;

use uuid::Uuid;

use agent_world_engine::organization::charter::{Charter, GovernanceModel, ProfitSharing};
use agent_world_engine::organization::diplomacy::{
    DiplomacyEngine, RelationLevel, TreatyStatus, TreatyType,
};
use agent_world_engine::organization::leadership::{
    ElectionStatus, LeadershipEngine, VotingMethod,
};
use agent_world_engine::organization::org::{OrgType, OrganizationStore};
use agent_world_engine::organization::treasury::{TaxKind, Treasury};
use agent_world_engine::world::event::WorldEvent;
use agent_world_engine::world::state::EventBus;

// ═══════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════

fn test_charter() -> Charter {
    Charter {
        purpose: "Integration test org".to_string(),
        governance: GovernanceModel::Vote,
        profit_sharing: ProfitSharing::Equal,
        membership_fee: 0,
    }
}

fn make_founders(n: usize, prefix: &str) -> Vec<(String, String)> {
    (0..n)
        .map(|i| {
            (
                format!("{}-agent-{}", prefix, i),
                format!("{} Agent {}", prefix, i),
            )
        })
        .collect()
}

/// Drain all pending events matching a predicate.
fn drain_events(
    rx: &mut tokio::sync::broadcast::Receiver<WorldEvent>,
    predicate: impl Fn(&WorldEvent) -> bool,
) -> Vec<WorldEvent> {
    let mut collected = Vec::new();
    loop {
        match rx.try_recv() {
            Ok(event) if predicate(&event) => collected.push(event),
            Ok(_) => {}
            Err(_) => break,
        }
    }
    collected
}

/// Collect ALL pending events into a Vec, preserving them for multi-pass analysis.
fn collect_all_events(rx: &mut tokio::sync::broadcast::Receiver<WorldEvent>) -> Vec<WorldEvent> {
    let mut collected = Vec::new();
    while let Ok(event) = rx.try_recv() {
        collected.push(event);
    }
    collected
}

// ═══════════════════════════════════════════════════════════════
// Test 1: Treasury integration — collect tax, distribute, verify events
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_treasury_collect_and_distribute_with_events() {
    let bus = EventBus::new(256);
    let mut rx = bus.subscribe();
    let mut store = OrganizationStore::with_event_bus(bus.clone());

    // Create org with 4 members
    let org = store
        .create_org(
            "Treasury Corp".to_string(),
            OrgType::Company,
            Some(test_charter()),
            make_founders(4, "tax"),
            10,
        )
        .unwrap();

    // Drain OrgCreated
    let _ = rx.try_recv();

    let mut treasury = Treasury::with_event_bus(org.id.clone(), bus.clone());

    // Collect income tax from each member at tick 50
    for i in 0..4 {
        let member_id = format!("tax-agent-{}", i);
        treasury
            .collect_tax(
                store.get_mut(&org.id).unwrap(),
                &member_id,
                TaxKind::IncomeTax,
                1000,
                50,
            )
            .unwrap();
    }

    // Verify TaxCollected events (4)
    let tax_events = drain_events(&mut rx, |e| matches!(e, WorldEvent::TaxCollected { .. }));
    assert_eq!(tax_events.len(), 4, "Expected 4 TaxCollected events");

    // Check treasury balance: 100 (creation) + 4 * 100 (tax) = 500
    let org_ref = store.get(&org.id).unwrap();
    assert_eq!(org_ref.treasury, 500);

    // Distribute 400 equally at tick 100
    treasury
        .distribute(store.get_mut(&org.id).unwrap(), 400, 100, None, None)
        .unwrap();

    // Verify TreasuryDistributed event
    let dist_events = drain_events(&mut rx, |e| {
        matches!(e, WorldEvent::TreasuryDistributed { .. })
    });
    assert_eq!(dist_events.len(), 1, "Expected 1 TreasuryDistributed event");

    // Remaining: 500 - 400 = 100
    let org_ref = store.get(&org.id).unwrap();
    assert_eq!(org_ref.treasury, 100);
}

// ═══════════════════════════════════════════════════════════════
// Test 2: Leadership election lifecycle with event bus
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_leadership_election_lifecycle_with_events() {
    let bus = EventBus::new(256);
    let mut rx = bus.subscribe();
    let mut store = OrganizationStore::with_event_bus(bus.clone());

    let org = store
        .create_org(
            "Election Guild".to_string(),
            OrgType::Guild,
            Some(test_charter()),
            make_founders(5, "vote"),
            10,
        )
        .unwrap();

    // Drain OrgCreated
    let _ = rx.try_recv();

    let org_uuid: Uuid = org.id.parse().expect("valid uuid");
    let mut leadership = LeadershipEngine::with_event_bus(bus.clone());

    // Initiate election at tick 50
    let election_id = leadership
        .initiate_election(
            org_uuid,
            vec![
                "vote-agent-0".to_string(),
                "vote-agent-1".to_string(),
                "vote-agent-2".to_string(),
            ],
            VotingMethod::SimpleMajority,
            50,
        )
        .unwrap();

    // Verify LeadershipElectionStarted event
    let started_events = drain_events(&mut rx, |e| {
        matches!(e, WorldEvent::LeadershipElectionStarted { .. })
    });
    assert_eq!(started_events.len(), 1);

    // Cast votes: 3 for agent-0, 2 for agent-1
    leadership
        .cast_vote(
            org_uuid,
            "vote-agent-3".to_string(),
            vec!["vote-agent-0".to_string()],
        )
        .unwrap();
    leadership
        .cast_vote(
            org_uuid,
            "vote-agent-4".to_string(),
            vec!["vote-agent-0".to_string()],
        )
        .unwrap();
    leadership
        .cast_vote(
            org_uuid,
            "vote-agent-0".to_string(),
            vec!["vote-agent-0".to_string()],
        )
        .unwrap();
    leadership
        .cast_vote(
            org_uuid,
            "vote-agent-1".to_string(),
            vec!["vote-agent-1".to_string()],
        )
        .unwrap();
    leadership
        .cast_vote(
            org_uuid,
            "vote-agent-2".to_string(),
            vec!["vote-agent-1".to_string()],
        )
        .unwrap();

    // Resolve: agent-0 wins (3 out of 5)
    let winner = leadership.resolve_election(org_uuid).unwrap();
    assert_eq!(winner, Some("vote-agent-0".to_string()));
    assert_eq!(leadership.get_leader(org_uuid), Some("vote-agent-0"));

    // Verify LeadershipChanged event
    let changed_events = drain_events(&mut rx, |e| {
        matches!(e, WorldEvent::LeadershipChanged { .. })
    });
    assert_eq!(changed_events.len(), 1);
    if let WorldEvent::LeadershipChanged {
        new_leader_id,
        old_leader_id,
        ..
    } = &changed_events[0]
    {
        assert_eq!(new_leader_id, "vote-agent-0");
        assert_eq!(*old_leader_id, None);
    }

    // Verify election is resolved
    let election = leadership.get_election(election_id).unwrap();
    assert_eq!(election.status, ElectionStatus::Resolved);
}

// ═══════════════════════════════════════════════════════════════
// Test 3: Diplomacy — propose, sign, break treaty with events
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_diplomacy_treaty_lifecycle_with_events() {
    let bus = EventBus::new(256);
    let mut rx = bus.subscribe();
    let mut store = OrganizationStore::with_event_bus(bus.clone());

    let org_a = store
        .create_org(
            "Alpha Alliance".to_string(),
            OrgType::Alliance,
            Some(test_charter()),
            make_founders(3, "alpha"),
            10,
        )
        .unwrap();

    let org_b = store
        .create_org(
            "Beta Guild".to_string(),
            OrgType::Guild,
            Some(test_charter()),
            make_founders(3, "beta"),
            10,
        )
        .unwrap();

    // Drain OrgCreated events
    let _ = rx.try_recv();
    let _ = rx.try_recv();

    let mut diplomacy = DiplomacyEngine::with_event_bus(bus.clone());

    // Set initial positive relation
    diplomacy.set_relation(&org_a.id, &org_b.id, RelationLevel::WARM);
    // Drain RelationChanged from set_relation
    let _ = rx.try_recv();

    // Propose trade agreement at tick 100
    let treaty_id = diplomacy
        .propose_treaty(&org_a.id, &org_b.id, TreatyType::TradeAgreement, 100, None)
        .unwrap();

    // Verify TreatyProposed event
    let proposed_events = drain_events(&mut rx, |e| matches!(e, WorldEvent::TreatyProposed { .. }));
    assert_eq!(proposed_events.len(), 1);
    if let WorldEvent::TreatyProposed {
        org_a: a,
        org_b: b,
        treaty_type,
        ..
    } = &proposed_events[0]
    {
        assert_eq!(treaty_type, "trade_agreement");
        assert!(a == &org_a.id || b == &org_a.id);
    }

    // Sign treaty at tick 105
    diplomacy.sign_treaty(&treaty_id, &org_b.id, 105).unwrap();

    // Verify TreatySigned event
    let signed_events = drain_events(&mut rx, |e| matches!(e, WorldEvent::TreatySigned { .. }));
    assert_eq!(signed_events.len(), 1);

    // Verify treaty is active
    let treaty = diplomacy.get_treaty(&treaty_id).unwrap();
    assert_eq!(treaty.status, TreatyStatus::Active);

    // Verify relation improved (WARM + 1 from signing)
    let relation = diplomacy.get_relation(&org_a.id, &org_b.id);
    assert!(relation.0 > RelationLevel::WARM);

    // Break treaty at tick 150
    diplomacy
        .break_treaty(&treaty_id, &org_a.id, "dispute over resources", 150)
        .unwrap();

    // Verify TreatyBroken event
    let broken_events = drain_events(&mut rx, |e| matches!(e, WorldEvent::TreatyBroken { .. }));
    assert_eq!(broken_events.len(), 1);
    if let WorldEvent::TreatyBroken {
        breaker, reason, ..
    } = &broken_events[0]
    {
        assert_eq!(breaker, &org_a.id);
        assert_eq!(reason, "dispute over resources");
    }

    // Verify relation degraded after breaking
    let relation = diplomacy.get_relation(&org_a.id, &org_b.id);
    assert!(relation.0 < RelationLevel::WARM);
}

// ═══════════════════════════════════════════════════════════════
// Test 4: Succession — leader leaves, new leader elected
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_leadership_succession_on_departure() {
    let bus = EventBus::new(256);
    let mut rx = bus.subscribe();
    let mut store = OrganizationStore::with_event_bus(bus.clone());

    let org = store
        .create_org(
            "Succession Corp".to_string(),
            OrgType::Company,
            Some(test_charter()),
            make_founders(4, "succ"),
            10,
        )
        .unwrap();

    let _ = rx.try_recv(); // OrgCreated

    let org_uuid: Uuid = org.id.parse().expect("valid uuid");
    let mut leadership = LeadershipEngine::with_event_bus(bus.clone());

    // First election: agent-0 wins
    leadership
        .initiate_election(
            org_uuid,
            vec!["succ-agent-0".to_string(), "succ-agent-1".to_string()],
            VotingMethod::SimpleMajority,
            50,
        )
        .unwrap();
    let _ = rx.try_recv(); // LeadershipElectionStarted

    leadership
        .cast_vote(
            org_uuid,
            "succ-agent-0".to_string(),
            vec!["succ-agent-0".to_string()],
        )
        .unwrap();
    leadership
        .cast_vote(
            org_uuid,
            "succ-agent-1".to_string(),
            vec!["succ-agent-0".to_string()],
        )
        .unwrap();
    leadership
        .cast_vote(
            org_uuid,
            "succ-agent-2".to_string(),
            vec!["succ-agent-0".to_string()],
        )
        .unwrap();
    leadership
        .cast_vote(
            org_uuid,
            "succ-agent-3".to_string(),
            vec!["succ-agent-1".to_string()],
        )
        .unwrap();

    let winner = leadership.resolve_election(org_uuid).unwrap();
    assert_eq!(winner, Some("succ-agent-0".to_string()));
    let _ = rx.try_recv(); // LeadershipChanged

    // Leader departs — succession with single remaining member
    let new_leader = leadership
        .handle_succession(
            org_uuid,
            "succ-agent-0".to_string(),
            vec!["succ-agent-3".to_string()],
            VotingMethod::SimpleMajority,
            100,
        )
        .unwrap();

    assert_eq!(new_leader, Some("succ-agent-3".to_string()));
    assert_eq!(leadership.get_leader(org_uuid), Some("succ-agent-3"));

    // Verify LeadershipChanged event for succession
    let changed = drain_events(&mut rx, |e| {
        matches!(e, WorldEvent::LeadershipChanged { .. })
    });
    assert_eq!(changed.len(), 1);
    if let WorldEvent::LeadershipChanged {
        old_leader_id,
        new_leader_id,
        ..
    } = &changed[0]
    {
        assert_eq!(*old_leader_id, Some("succ-agent-0".to_string()));
        assert_eq!(new_leader_id, "succ-agent-3");
    }
}

// ═══════════════════════════════════════════════════════════════
// Test 5: Multi-org 200+ tick simulation — tax, election, diplomacy
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_three_orgs_200_tick_simulation() {
    let bus = EventBus::new(4096);
    let mut rx = bus.subscribe();
    let mut store = OrganizationStore::with_event_bus(bus.clone());

    // ── Phase 1: Create 3 organizations ─────────────────────
    let org_a = store
        .create_org(
            "Miners Guild".to_string(),
            OrgType::Guild,
            Some(test_charter()),
            make_founders(5, "miner"),
            10,
        )
        .unwrap();

    let org_b = store
        .create_org(
            "Traders Company".to_string(),
            OrgType::Company,
            Some(test_charter()),
            make_founders(4, "trader"),
            10,
        )
        .unwrap();

    let org_c = store
        .create_org(
            "Defenders Alliance".to_string(),
            OrgType::Alliance,
            Some(test_charter()),
            make_founders(3, "def"),
            10,
        )
        .unwrap();

    // Drain OrgCreated events
    drain_events(&mut rx, |e| matches!(e, WorldEvent::OrgCreated { .. }));

    let uuid_a: Uuid = org_a.id.parse().unwrap();
    let uuid_b: Uuid = org_b.id.parse().unwrap();

    // ── Initialize governance subsystems ─────────────────────

    let mut treasury_a = Treasury::with_event_bus(org_a.id.clone(), bus.clone());
    let mut treasury_b = Treasury::with_event_bus(org_b.id.clone(), bus.clone());
    let mut treasury_c = Treasury::with_event_bus(org_c.id.clone(), bus.clone());

    let mut leadership = LeadershipEngine::with_shared_event_bus(Arc::new(bus.clone()));
    let mut diplomacy = DiplomacyEngine::with_shared_event_bus(Arc::new(bus.clone()));

    // Set up diplomatic relations: A↔B warm, A↔C warm, B↔C friendly
    diplomacy.set_relation(&org_a.id, &org_b.id, RelationLevel::WARM);
    diplomacy.set_relation(&org_a.id, &org_c.id, RelationLevel::WARM);
    diplomacy.set_relation(&org_b.id, &org_c.id, RelationLevel::FRIENDLY);

    // Drain initial RelationChanged events
    drain_events(&mut rx, |e| matches!(e, WorldEvent::RelationChanged { .. }));

    // ── Phase 2: Run 250 ticks ──────────────────────────────

    let tax_interval: u64 = 20; // Collect tax every 20 ticks
    let election_tick: u64 = 30; // Hold elections at tick 30
    let diplomacy_tick_start: u64 = 50; // Start diplomacy at tick 50

    for tick in 11..=260 {
        // ── Tax Collection ───────────────────────────────
        if tick > 0 && (tick - 11) % tax_interval == 0 {
            // Each member pays income tax
            for i in 0..5 {
                let member = format!("miner-agent-{}", i);
                let _ = treasury_a.collect_tax(
                    store.get_mut(&org_a.id).unwrap(),
                    &member,
                    TaxKind::IncomeTax,
                    500,
                    tick,
                );
            }
            for i in 0..4 {
                let member = format!("trader-agent-{}", i);
                let _ = treasury_b.collect_tax(
                    store.get_mut(&org_b.id).unwrap(),
                    &member,
                    TaxKind::IncomeTax,
                    800,
                    tick,
                );
            }
            for i in 0..3 {
                let member = format!("def-agent-{}", i);
                let _ = treasury_c.collect_tax(
                    store.get_mut(&org_c.id).unwrap(),
                    &member,
                    TaxKind::IncomeTax,
                    300,
                    tick,
                );
            }
        }

        // ── Distribution every 60 ticks ─────────────────
        if tick > 11 && (tick - 11) % 60 == 0 {
            let org_a_ref = store.get(&org_a.id).unwrap();
            if org_a_ref.treasury > 50 {
                let amt = org_a_ref.treasury / 2;
                let _ =
                    treasury_a.distribute(store.get_mut(&org_a.id).unwrap(), amt, tick, None, None);
            }
        }

        // ── Leadership Election at tick 30 ──────────────
        if tick == election_tick {
            // Org A: election with simple majority
            let _ = leadership.initiate_election(
                uuid_a,
                vec!["miner-agent-0".to_string(), "miner-agent-1".to_string()],
                VotingMethod::SimpleMajority,
                tick,
            );

            // Org B: election with ranked choice
            let _ = leadership.initiate_election(
                uuid_b,
                vec![
                    "trader-agent-0".to_string(),
                    "trader-agent-1".to_string(),
                    "trader-agent-2".to_string(),
                ],
                VotingMethod::RankedChoice,
                tick,
            );
        }

        // ── Cast votes for elections ────────────────────
        if tick == election_tick + 1 {
            // Org A: 3 votes for agent-0, 2 for agent-1
            let _ = leadership.cast_vote(
                uuid_a,
                "miner-agent-0".to_string(),
                vec!["miner-agent-0".to_string()],
            );
            let _ = leadership.cast_vote(
                uuid_a,
                "miner-agent-1".to_string(),
                vec!["miner-agent-1".to_string()],
            );
            let _ = leadership.cast_vote(
                uuid_a,
                "miner-agent-2".to_string(),
                vec!["miner-agent-0".to_string()],
            );
            let _ = leadership.cast_vote(
                uuid_a,
                "miner-agent-3".to_string(),
                vec!["miner-agent-0".to_string()],
            );
            let _ = leadership.cast_vote(
                uuid_a,
                "miner-agent-4".to_string(),
                vec!["miner-agent-1".to_string()],
            );

            // Org B: ranked choice voting
            let _ = leadership.cast_vote(
                uuid_b,
                "trader-agent-0".to_string(),
                vec!["trader-agent-0".to_string(), "trader-agent-1".to_string()],
            );
            let _ = leadership.cast_vote(
                uuid_b,
                "trader-agent-1".to_string(),
                vec!["trader-agent-1".to_string(), "trader-agent-0".to_string()],
            );
            let _ = leadership.cast_vote(
                uuid_b,
                "trader-agent-2".to_string(),
                vec!["trader-agent-2".to_string(), "trader-agent-0".to_string()],
            );
            let _ = leadership.cast_vote(
                uuid_b,
                "trader-agent-3".to_string(),
                vec!["trader-agent-0".to_string(), "trader-agent-2".to_string()],
            );
        }

        // ── Resolve elections at tick 32 ────────────────
        if tick == election_tick + 2 {
            let winner_a = leadership.resolve_election(uuid_a).unwrap();
            assert_eq!(winner_a, Some("miner-agent-0".to_string()));

            let winner_b = leadership.resolve_election(uuid_b).unwrap();
            assert_eq!(winner_b, Some("trader-agent-0".to_string()));
        }

        // ── Diplomacy: propose treaties starting at tick 50 ──
        if tick == diplomacy_tick_start {
            // A↔B trade agreement
            let _ = diplomacy.propose_treaty(
                &org_a.id,
                &org_b.id,
                TreatyType::TradeAgreement,
                tick,
                None,
            );

            // B↔C non-aggression
            let _ = diplomacy.propose_treaty(
                &org_b.id,
                &org_c.id,
                TreatyType::NonAggression,
                tick,
                Some(100),
            );

            // A↔C mutual defense (requires WARM=1, which we have)
            let _ = diplomacy.propose_treaty(
                &org_a.id,
                &org_c.id,
                TreatyType::MutualDefense,
                tick,
                None,
            );
        }

        // ── Sign treaties at tick 55 ────────────────────
        if tick == diplomacy_tick_start + 5 {
            // Sign A↔B trade agreement
            let _ = diplomacy.sign_treaty("treaty-1", &org_b.id, tick);
            // Sign B↔C non-aggression
            let _ = diplomacy.sign_treaty("treaty-2", &org_c.id, tick);
            // Sign A↔C mutual defense
            let _ = diplomacy.sign_treaty("treaty-3", &org_c.id, tick);
        }

        // ── Treaty expiry check every tick ──────────────
        let _ = diplomacy.tick_expiry(tick);
    }

    // ── Phase 3: Verify all event types appeared ────────────

    // Collect all remaining events for multi-pass analysis
    let all_events = collect_all_events(&mut rx);

    let tax_events = all_events
        .iter()
        .filter(|e| matches!(e, WorldEvent::TaxCollected { .. }))
        .count();
    assert!(tax_events > 0, "Expected TaxCollected events, got 0");

    let dist_events = all_events
        .iter()
        .filter(|e| matches!(e, WorldEvent::TreasuryDistributed { .. }))
        .count();
    assert!(
        dist_events > 0,
        "Expected TreasuryDistributed events, got 0"
    );

    let election_started = all_events
        .iter()
        .filter(|e| matches!(e, WorldEvent::LeadershipElectionStarted { .. }))
        .count();
    assert!(
        election_started > 0,
        "Expected LeadershipElectionStarted events, got 0"
    );

    let leadership_changed = all_events
        .iter()
        .filter(|e| matches!(e, WorldEvent::LeadershipChanged { .. }))
        .count();
    assert!(
        leadership_changed > 0,
        "Expected LeadershipChanged events, got 0"
    );

    let treaty_proposed = all_events
        .iter()
        .filter(|e| matches!(e, WorldEvent::TreatyProposed { .. }))
        .count();
    assert!(treaty_proposed > 0, "Expected TreatyProposed events, got 0");

    let treaty_signed = all_events
        .iter()
        .filter(|e| matches!(e, WorldEvent::TreatySigned { .. }))
        .count();
    assert!(treaty_signed > 0, "Expected TreatySigned events, got 0");

    // Verify final state

    // Org A has a leader
    assert_eq!(leadership.get_leader(uuid_a), Some("miner-agent-0"));
    // Org B has a leader
    assert_eq!(leadership.get_leader(uuid_b), Some("trader-agent-0"));

    // Orgs have positive treasury from tax collection
    let org_a_ref = store.get(&org_a.id).unwrap();
    assert!(
        org_a_ref.treasury > 0,
        "Org A should have treasury from taxes"
    );

    let org_b_ref = store.get(&org_b.id).unwrap();
    assert!(
        org_b_ref.treasury > 0,
        "Org B should have treasury from taxes"
    );

    // Diplomacy: B↔C non-aggression should have expired (signed at tick 55, duration 100, expired at tick 155)
    let treaty_bc = diplomacy.get_treaty("treaty-2").unwrap();
    assert_eq!(treaty_bc.status, TreatyStatus::Expired);

    // A↔B trade and A↔C defense should still be active (no duration)
    let treaty_ab = diplomacy.get_treaty("treaty-1").unwrap();
    assert_eq!(treaty_ab.status, TreatyStatus::Active);
    let treaty_ac = diplomacy.get_treaty("treaty-3").unwrap();
    assert_eq!(treaty_ac.status, TreatyStatus::Active);
}

// ═══════════════════════════════════════════════════════════════
// Test 6: Cross-system interaction — tax collection after election
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_tax_collection_after_leadership_change() {
    let bus = EventBus::new(256);
    let mut rx = bus.subscribe();
    let mut store = OrganizationStore::with_event_bus(bus.clone());

    let org = store
        .create_org(
            "TaxGuild".to_string(),
            OrgType::Guild,
            Some(test_charter()),
            make_founders(3, "tg"),
            0,
        )
        .unwrap();

    let _ = rx.try_recv(); // OrgCreated

    let org_uuid: Uuid = org.id.parse().unwrap();
    let mut treasury = Treasury::with_event_bus(org.id.clone(), bus.clone());
    let mut leadership = LeadershipEngine::with_shared_event_bus(Arc::new(bus.clone()));

    // Collect tax before election
    treasury
        .collect_tax(
            store.get_mut(&org.id).unwrap(),
            "tg-agent-0",
            TaxKind::IncomeTax,
            1000,
            10,
        )
        .unwrap();
    let _ = drain_events(&mut rx, |e| matches!(e, WorldEvent::TaxCollected { .. }));

    // Run election
    leadership
        .initiate_election(
            org_uuid,
            vec!["tg-agent-0".to_string(), "tg-agent-1".to_string()],
            VotingMethod::SimpleMajority,
            20,
        )
        .unwrap();
    let _ = rx.try_recv(); // LeadershipElectionStarted

    leadership
        .cast_vote(
            org_uuid,
            "tg-agent-0".to_string(),
            vec!["tg-agent-0".to_string()],
        )
        .unwrap();
    leadership
        .cast_vote(
            org_uuid,
            "tg-agent-1".to_string(),
            vec!["tg-agent-0".to_string()],
        )
        .unwrap();
    leadership
        .cast_vote(
            org_uuid,
            "tg-agent-2".to_string(),
            vec!["tg-agent-1".to_string()],
        )
        .unwrap();
    leadership.resolve_election(org_uuid).unwrap();
    let _ = rx.try_recv(); // LeadershipChanged

    // Collect tax after election — still works
    treasury
        .collect_tax(
            store.get_mut(&org.id).unwrap(),
            "tg-agent-1",
            TaxKind::IncomeTax,
            2000,
            30,
        )
        .unwrap();

    let tax_events = drain_events(&mut rx, |e| matches!(e, WorldEvent::TaxCollected { .. }));
    assert_eq!(
        tax_events.len(),
        1,
        "Tax should still be collected after election"
    );

    // Verify total: 100 (creation) + 100 (first tax) + 200 (second tax) = 400
    assert_eq!(store.get(&org.id).unwrap().treasury, 400);
}

// ═══════════════════════════════════════════════════════════════
// Test 7: Diplomacy between orgs with tax interactions
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_diplomacy_and_tax_interaction() {
    let bus = EventBus::new(256);
    let mut rx = bus.subscribe();
    let mut store = OrganizationStore::with_event_bus(bus.clone());

    let org_a = store
        .create_org(
            "TradeCo A".to_string(),
            OrgType::Company,
            Some(test_charter()),
            make_founders(3, "ta"),
            0,
        )
        .unwrap();
    let org_b = store
        .create_org(
            "TradeCo B".to_string(),
            OrgType::Company,
            Some(test_charter()),
            make_founders(3, "tb"),
            0,
        )
        .unwrap();

    drain_events(&mut rx, |e| matches!(e, WorldEvent::OrgCreated { .. }));

    let mut treasury_a = Treasury::with_event_bus(org_a.id.clone(), bus.clone());
    let mut diplomacy = DiplomacyEngine::with_shared_event_bus(Arc::new(bus.clone()));

    // Collect taxes to build treasury
    for i in 0..3 {
        treasury_a
            .collect_tax(
                store.get_mut(&org_a.id).unwrap(),
                &format!("ta-agent-{}", i),
                TaxKind::IncomeTax,
                1000,
                10,
            )
            .unwrap();
    }
    drain_events(&mut rx, |e| matches!(e, WorldEvent::TaxCollected { .. }));

    // Set warm relations and propose/sign trade agreement
    diplomacy.set_relation(&org_a.id, &org_b.id, RelationLevel::WARM);
    drain_events(&mut rx, |_| true);

    let treaty_id = diplomacy
        .propose_treaty(&org_a.id, &org_b.id, TreatyType::TradeAgreement, 50, None)
        .unwrap();
    diplomacy.sign_treaty(&treaty_id, &org_b.id, 55).unwrap();

    // Collect trade tax (simulating trade between orgs)
    treasury_a
        .collect_tax(
            store.get_mut(&org_a.id).unwrap(),
            "ta-agent-0",
            TaxKind::TradeTax,
            500,
            60,
        )
        .unwrap();

    let trade_events: Vec<_> = drain_events(
        &mut rx,
        |e| matches!(e, WorldEvent::TaxCollected { tax_kind, .. } if tax_kind == "trade_tax"),
    );
    assert_eq!(trade_events.len(), 1, "Trade tax should be collected");

    // Verify final balance: 100 + 3*100 (income) + 25 (trade) = 425
    let balance = store.get(&org_a.id).unwrap().treasury;
    assert_eq!(balance, 425);
}
