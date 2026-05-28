//! Phase 4.4.2 E2E Integration Tests — Self-Governance (Treasury / Leadership / Diplomacy).
//!
//! Validates end-to-end interaction across all Phase 4.4.2 subsystems:
//! 1. Create 3 organizations of different types (Company, Guild, Alliance)
//! 2. Run 200+ ticks with treasury, leadership, and diplomacy operations
//! 3. Assert at least 1 election event (LeadershipChanged)
//! 4. Assert at least 1 tax event (TaxCollected / TreasuryDistributed)
//! 5. Assert at least 1 diplomacy event (TreatyProposed / TreatySigned)
//! 6. Verify organization relationships change during simulation

use std::collections::HashMap;
use std::sync::Arc;

use uuid::Uuid;

use agent_world_engine::organization::charter::{Charter, GovernanceModel, ProfitSharing};
use agent_world_engine::organization::diplomacy::{
    DiplomacyEngine, RelationLevel, TreatyStatus, TreatyType,
};
use agent_world_engine::organization::leadership::{LeadershipEngine, VotingMethod};
use agent_world_engine::organization::org::{OrgType, OrganizationStore};
use agent_world_engine::organization::treasury::{TaxKind, Treasury};
use agent_world_engine::world::event::WorldEvent;
use agent_world_engine::world::state::EventBus;

// ═══════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════

fn test_charter() -> Charter {
    Charter {
        purpose: "E2E integration test org".to_string(),
        governance: GovernanceModel::Vote,
        profit_sharing: ProfitSharing::Equal,
        membership_fee: 0,
    }
}

fn make_founders(start: usize, n: usize, prefix: &str) -> Vec<(String, String)> {
    (start..start + n)
        .map(|i| {
            (
                format!("{}-agent-{}", prefix, i),
                format!("{} Agent {}", prefix, i),
            )
        })
        .collect()
}

/// Drain all pending events matching a predicate from a broadcast receiver.
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

/// Count events of each category for final assertions.
#[derive(Default)]
struct EventCounts {
    elections_started: usize,
    leadership_changed: usize,
    tax_collected: usize,
    treasury_distributed: usize,
    treaty_proposed: usize,
    treaty_signed: usize,
    treaty_broken: usize,
    relation_changed: usize,
}

impl EventCounts {
    fn from_events(events: &[WorldEvent]) -> Self {
        let mut c = Self::default();
        for e in events {
            match e {
                WorldEvent::LeadershipElectionStarted { .. } => c.elections_started += 1,
                WorldEvent::LeadershipChanged { .. } => c.leadership_changed += 1,
                WorldEvent::TaxCollected { .. } => c.tax_collected += 1,
                WorldEvent::TreasuryDistributed { .. } => c.treasury_distributed += 1,
                WorldEvent::TreatyProposed { .. } => c.treaty_proposed += 1,
                WorldEvent::TreatySigned { .. } => c.treaty_signed += 1,
                WorldEvent::TreatyBroken { .. } => c.treaty_broken += 1,
                WorldEvent::RelationChanged { .. } => c.relation_changed += 1,
                _ => {}
            }
        }
        c
    }
}

// ═══════════════════════════════════════════════════════════════
// Test 1: 3 Orgs × 200+ Ticks — Full E2E Simulation
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_three_orgs_200_ticks_e2e() {
    // ── Setup shared event bus ──
    let bus = Arc::new(EventBus::new(4096));
    let mut rx = bus.subscribe();

    let mut store = OrganizationStore::with_event_bus((*bus).clone());
    let mut leadership = LeadershipEngine::with_shared_event_bus(bus.clone());
    let mut diplomacy = DiplomacyEngine::with_shared_event_bus(bus.clone());

    // ── Create 3 organizations of different types ──

    let company = store
        .create_org(
            "MegaCorp".to_string(),
            OrgType::Company,
            Some(test_charter()),
            make_founders(0, 5, "company"),
            0,
        )
        .expect("company creation should succeed");

    let guild = store
        .create_org(
            "Miners United".to_string(),
            OrgType::Guild,
            Some(test_charter()),
            make_founders(10, 4, "guild"),
            0,
        )
        .expect("guild creation should succeed");

    let alliance = store
        .create_org(
            "Northern Alliance".to_string(),
            OrgType::Alliance,
            Some(test_charter()),
            make_founders(20, 3, "alliance"),
            0,
        )
        .expect("alliance creation should succeed");

    let org_ids: Vec<String> = vec![company.id.clone(), guild.id.clone(), alliance.id.clone()];

    // Drain org-creation events
    let _ = drain_events(&mut rx, |e| matches!(e, WorldEvent::OrgCreated { .. }));

    // ── Initialize treasuries ──
    let mut treasuries: HashMap<String, Treasury> = HashMap::new();
    for org_id in &org_ids {
        treasuries.insert(
            org_id.clone(),
            Treasury::with_event_bus(org_id.clone(), (*bus).clone()),
        );
    }

    // ── Set initial diplomatic relations (warm between guild & alliance) ──
    diplomacy.set_relation(&guild.id, &alliance.id, RelationLevel::WARM);
    diplomacy.set_relation(&company.id, &guild.id, RelationLevel::NEUTRAL);
    diplomacy.set_relation(&company.id, &alliance.id, RelationLevel::COLD);

    // Drain any RelationChanged events from initial setup
    let _ = drain_events(&mut rx, |e| matches!(e, WorldEvent::RelationChanged { .. }));

    // ── Run 250 ticks ──
    let total_ticks: u64 = 250;

    for tick in 1..=total_ticks {
        // --- Treasury: collect taxes every 10 ticks ---
        if tick % 10 == 0 {
            for org_id in &org_ids {
                let org = store.get(org_id).unwrap();
                let member_ids: Vec<String> =
                    org.members.iter().map(|m| m.agent_id.clone()).collect();
                // Collect income tax from first member
                if let Some(payer) = member_ids.first() {
                    let treasury = treasuries.get_mut(org_id).unwrap();
                    let org = store.get_mut(org_id).unwrap();
                    let _ = treasury.collect_tax(org, payer, TaxKind::IncomeTax, 100, tick);
                }
            }
        }

        // --- Treasury: distribute every 50 ticks ---
        if tick % 50 == 0 {
            for org_id in &org_ids {
                let org = store.get(org_id).unwrap();
                let balance = org.treasury;
                if balance > 20 {
                    let distribute_amount = balance / 2;
                    let treasury = treasuries.get_mut(org_id).unwrap();
                    let org = store.get_mut(org_id).unwrap();
                    let _ = treasury.distribute(org, distribute_amount, tick, None, None);
                }
            }
        }

        // --- Leadership: election at tick 30, 120, 200 ---
        if tick == 30 || tick == 120 || tick == 200 {
            // Run election for the company org
            let org_uuid: Uuid = company.id.parse().expect("valid UUID");
            let member_ids: Vec<String> = store
                .get(&company.id)
                .unwrap()
                .members
                .iter()
                .take(3)
                .map(|m| m.agent_id.clone())
                .collect();

            if let Ok(_election_id) = leadership.initiate_election(
                org_uuid,
                member_ids.clone(),
                VotingMethod::SimpleMajority,
                tick,
            ) {
                // All members vote for the first candidate (guarantees a majority)
                for voter_id in &member_ids {
                    let _ = leadership.cast_vote(
                        org_uuid,
                        voter_id.clone(),
                        vec![member_ids[0].clone()],
                    );
                }
                let _ = leadership.resolve_election(org_uuid);
            }
        }

        // --- Diplomacy: propose & sign treaty at tick 60 ---
        if tick == 60 {
            // Guild & Alliance sign a trade agreement (they have warm relations)
            let _ = diplomacy.propose_treaty(
                &guild.id,
                &alliance.id,
                TreatyType::TradeAgreement,
                tick,
                None,
            );
        }

        if tick == 65 {
            // Sign the proposed treaty — clone id to release borrow
            let treaty_id = diplomacy
                .get_treaties_for_org(&guild.id)
                .first()
                .map(|t| t.id.clone());
            if let Some(tid) = treaty_id {
                let _ = diplomacy.sign_treaty(&tid, &alliance.id, tick);
            }
        }

        // --- Diplomacy: propose non-aggression at tick 100 ---
        if tick == 100 {
            // Improve company-guild relations first so we can propose
            diplomacy.adjust_relation(&company.id, &guild.id, 2);
            let _ = diplomacy.propose_treaty(
                &company.id,
                &guild.id,
                TreatyType::NonAggression,
                tick,
                Some(100),
            );
        }

        if tick == 105 {
            // Clone id to release borrow before mutable call
            let treaty_id = diplomacy
                .get_treaties_for_org(&company.id)
                .iter()
                .find(|t| t.status == TreatyStatus::Proposed)
                .map(|t| t.id.clone());
            if let Some(tid) = treaty_id {
                let _ = diplomacy.sign_treaty(&tid, &guild.id, tick);
            }
        }

        // --- Leadership: succession event at tick 150 ---
        // Use only 2 candidates to ensure a majority result in succession
        if tick == 150 {
            let org_uuid: Uuid = company.id.parse().expect("valid UUID");
            let current_leader = leadership.get_leader(org_uuid).map(|s| s.to_string());
            if let Some(leader) = current_leader {
                let remaining: Vec<String> = store
                    .get(&company.id)
                    .unwrap()
                    .members
                    .iter()
                    .filter(|m| m.agent_id != leader)
                    .map(|m| m.agent_id.clone())
                    .collect();
                // Use only 2 candidates to avoid tie in auto-succession voting
                let candidates: Vec<String> = remaining.into_iter().take(2).collect();
                let _ = leadership.handle_succession(
                    org_uuid,
                    leader,
                    candidates,
                    VotingMethod::SimpleMajority,
                    tick,
                );
            }
        }

        // --- Diplomacy: break treaty at tick 180 ---
        if tick == 180 {
            // Clone id to release borrow before mutable call
            let treaty_id = diplomacy
                .get_treaties_for_org(&company.id)
                .iter()
                .find(|t| t.status == TreatyStatus::Active)
                .map(|t| t.id.clone());
            if let Some(tid) = treaty_id {
                let _ = diplomacy.break_treaty(&tid, &company.id, "betrayal", tick);
            }
        }

        // --- Diplomacy: adjust relations at tick 220 ---
        if tick == 220 {
            diplomacy.adjust_relation(&guild.id, &alliance.id, 1);
            diplomacy.adjust_relation(&company.id, &alliance.id, -1);
        }
    }

    // ── Collect all Phase 4.4.2 events ──
    let all_events: Vec<WorldEvent> = drain_events(&mut rx, |_| true);
    let counts = EventCounts::from_events(&all_events);

    // ── Assertions ──

    // At least 1 election event
    assert!(
        counts.elections_started >= 1,
        "Expected at least 1 LeadershipElectionStarted, got {}",
        counts.elections_started,
    );

    // At least 1 leadership change
    assert!(
        counts.leadership_changed >= 1,
        "Expected at least 1 LeadershipChanged, got {}",
        counts.leadership_changed,
    );

    // At least 1 tax collection event
    assert!(
        counts.tax_collected >= 1,
        "Expected at least 1 TaxCollected, got {}",
        counts.tax_collected,
    );

    // At least 1 treasury distribution event
    assert!(
        counts.treasury_distributed >= 1,
        "Expected at least 1 TreasuryDistributed, got {}",
        counts.treasury_distributed,
    );

    // At least 1 treaty proposed
    assert!(
        counts.treaty_proposed >= 1,
        "Expected at least 1 TreatyProposed, got {}",
        counts.treaty_proposed,
    );

    // At least 1 treaty signed
    assert!(
        counts.treaty_signed >= 1,
        "Expected at least 1 TreatySigned, got {}",
        counts.treaty_signed,
    );

    // Organization relationships changed during simulation
    assert!(
        counts.relation_changed >= 1,
        "Expected at least 1 RelationChanged, got {}",
        counts.relation_changed,
    );

    // ── Verify final state ──

    // All orgs should still be active
    for org_id in &org_ids {
        let org = store.get(org_id).unwrap();
        assert!(
            org.treasury > 0 || !org.members.is_empty(),
            "Org {} should have positive treasury or members after simulation",
            org_id,
        );
    }

    // Leader should be set for the company
    let company_uuid: Uuid = company.id.parse().unwrap();
    assert!(
        leadership.get_leader(company_uuid).is_some(),
        "Company should have a leader after elections",
    );

    // Guild-Alliance should have positive relations (warm + treaty bonus)
    let guild_alliance_relation = diplomacy.get_relation(&guild.id, &alliance.id);
    assert!(
        guild_alliance_relation.is_positive(),
        "Guild-Alliance relation should be positive after trade agreement, got {}",
        guild_alliance_relation.0,
    );
}

// ═══════════════════════════════════════════════════════════════
// Test 2: Treasury-only integration — tax → distribute cycle
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_treasury_collect_and_distribute_cycle() {
    let bus = EventBus::new(256);
    let mut rx = bus.subscribe();
    let mut store = OrganizationStore::with_event_bus(bus.clone());
    let mut treasury = Treasury::with_event_bus("test-org".to_string(), bus.clone());

    let org = store
        .create_org(
            "TaxCorp".to_string(),
            OrgType::Company,
            Some(test_charter()),
            make_founders(0, 3, "tax"),
            0,
        )
        .unwrap();

    // Drain OrgCreated
    let _ = rx.try_recv();

    // Collect taxes from each member 5 times
    for tick in 1..=5 {
        for member in &org.members {
            let org = store.get_mut(&org.id).unwrap();
            let _ = treasury.collect_tax(org, &member.agent_id, TaxKind::IncomeTax, 200, tick);
        }
    }

    // 3 members × 5 ticks = 15 tax collections
    let tax_events = drain_events(&mut rx, |e| matches!(e, WorldEvent::TaxCollected { .. }));
    assert_eq!(tax_events.len(), 15);

    // Treasury should have accumulated — clone values to release borrow
    let (org_id_clone, balance) = {
        let org = store.get(&org.id).unwrap();
        (org.id.clone(), org.treasury)
    };
    assert!(
        balance > 0,
        "Treasury balance should be positive: {}",
        balance
    );

    // Distribute half
    let dist_amount = balance / 2;
    let org = store.get_mut(&org_id_clone).unwrap();
    let result = treasury.distribute(org, dist_amount, 100, None, None);
    assert!(result.is_ok(), "Distribution should succeed");

    let dist_events = drain_events(&mut rx, |e| {
        matches!(e, WorldEvent::TreasuryDistributed { .. })
    });
    assert_eq!(dist_events.len(), 1);

    // Verify allocation count matches member count
    if let WorldEvent::TreasuryDistributed { allocations, .. } = &dist_events[0] {
        assert_eq!(allocations.len(), 3, "Should allocate to 3 members");
    }
}

// ═══════════════════════════════════════════════════════════════
// Test 3: Leadership election flow — nominate → vote → resolve
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_leadership_election_full_flow() {
    let bus = EventBus::new(256);
    let mut rx = bus.subscribe();
    let mut leadership = LeadershipEngine::with_event_bus(bus.clone());

    let org_id = Uuid::new_v4();
    let candidates = vec![
        "candidate-a".to_string(),
        "candidate-b".to_string(),
        "candidate-c".to_string(),
    ];

    // Initiate election
    let _election_id = leadership
        .initiate_election(org_id, candidates.clone(), VotingMethod::SimpleMajority, 10)
        .unwrap();

    let started = drain_events(&mut rx, |e| {
        matches!(e, WorldEvent::LeadershipElectionStarted { .. })
    });
    assert_eq!(started.len(), 1);

    // Cast votes: 3 voters, candidate-a gets 2 votes, candidate-b gets 1
    leadership
        .cast_vote(
            org_id,
            "voter-1".to_string(),
            vec!["candidate-a".to_string()],
        )
        .unwrap();
    leadership
        .cast_vote(
            org_id,
            "voter-2".to_string(),
            vec!["candidate-a".to_string()],
        )
        .unwrap();
    leadership
        .cast_vote(
            org_id,
            "voter-3".to_string(),
            vec!["candidate-b".to_string()],
        )
        .unwrap();

    // Resolve
    let winner = leadership.resolve_election(org_id).unwrap();
    assert_eq!(winner, Some("candidate-a".to_string()));

    let changed = drain_events(&mut rx, |e| {
        matches!(e, WorldEvent::LeadershipChanged { .. })
    });
    assert_eq!(changed.len(), 1);
    if let WorldEvent::LeadershipChanged {
        new_leader_id,
        old_leader_id,
        ..
    } = &changed[0]
    {
        assert_eq!(new_leader_id, "candidate-a");
        assert!(
            old_leader_id.is_none(),
            "First election should have no old leader"
        );
    }

    // Verify leader is tracked
    assert_eq!(leadership.get_leader(org_id), Some("candidate-a"));
}

// ═══════════════════════════════════════════════════════════════
// Test 4: Diplomacy flow — propose → sign → break
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_diplomacy_treaty_lifecycle() {
    let bus = EventBus::new(256);
    let mut rx = bus.subscribe();
    let mut diplomacy = DiplomacyEngine::with_event_bus(bus.clone());

    // Set warm relations to allow treaty proposal
    diplomacy.set_relation("org-a", "org-b", RelationLevel::WARM);
    let _ = drain_events(&mut rx, |e| matches!(e, WorldEvent::RelationChanged { .. }));

    // Propose trade agreement
    let treaty_id = diplomacy
        .propose_treaty("org-a", "org-b", TreatyType::TradeAgreement, 10, None)
        .unwrap();

    let proposed = drain_events(&mut rx, |e| matches!(e, WorldEvent::TreatyProposed { .. }));
    assert_eq!(proposed.len(), 1);

    // Sign it
    diplomacy.sign_treaty(&treaty_id, "org-b", 15).unwrap();

    let signed = drain_events(&mut rx, |e| matches!(e, WorldEvent::TreatySigned { .. }));
    assert_eq!(signed.len(), 1);

    // Relations should have improved (warm +1 = friendly)
    let relation = diplomacy.get_relation("org-a", "org-b");
    assert_eq!(relation.0, RelationLevel::FRIENDLY);

    // Break the treaty
    diplomacy
        .break_treaty(&treaty_id, "org-a", "testing", 50)
        .unwrap();

    let broken = drain_events(&mut rx, |e| matches!(e, WorldEvent::TreatyBroken { .. }));
    assert_eq!(broken.len(), 1);
}

// ═══════════════════════════════════════════════════════════════
// Test 5: Cross-module — leadership change affects diplomacy
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_leadership_change_triggers_diplomacy_shift() {
    let bus = Arc::new(EventBus::new(512));
    let mut rx = bus.subscribe();

    let mut store = OrganizationStore::with_event_bus((*bus).clone());
    let mut leadership = LeadershipEngine::with_shared_event_bus(bus.clone());
    let mut diplomacy = DiplomacyEngine::with_shared_event_bus(bus.clone());

    // Create two orgs
    let org_a = store
        .create_org(
            "Alpha Corp".to_string(),
            OrgType::Company,
            Some(test_charter()),
            make_founders(0, 4, "alpha"),
            0,
        )
        .unwrap();

    let org_b = store
        .create_org(
            "Beta Guild".to_string(),
            OrgType::Guild,
            Some(test_charter()),
            make_founders(10, 3, "beta"),
            0,
        )
        .unwrap();

    // Drain org creation events
    let _ = drain_events(&mut rx, |_| true);

    // Set initial relations
    diplomacy.set_relation(&org_a.id, &org_b.id, RelationLevel::NEUTRAL);

    // Drain RelationChanged
    let _ = drain_events(&mut rx, |_| true);

    // Run election in org_a
    let org_a_uuid: Uuid = org_a.id.parse().unwrap();
    let candidates: Vec<String> = org_a
        .members
        .iter()
        .take(3)
        .map(|m| m.agent_id.clone())
        .collect();
    let _election_id = leadership
        .initiate_election(
            org_a_uuid,
            candidates.clone(),
            VotingMethod::SimpleMajority,
            10,
        )
        .unwrap();

    // All vote for candidate 0
    for c in &candidates {
        let _ = leadership.cast_vote(org_a_uuid, c.clone(), vec![candidates[0].clone()]);
    }
    let winner = leadership.resolve_election(org_a_uuid).unwrap();
    assert!(winner.is_some());

    // New leader changes diplomatic stance — improve relations with org_b
    diplomacy.adjust_relation(&org_a.id, &org_b.id, 2);

    // Now propose a treaty
    let treaty_id = diplomacy
        .propose_treaty(&org_a.id, &org_b.id, TreatyType::NonAggression, 20, None)
        .unwrap();

    diplomacy.sign_treaty(&treaty_id, &org_b.id, 25).unwrap();

    // Collect all events
    let all_events = drain_events(&mut rx, |_| true);

    // Verify we got the full chain: election → leader change → relation change → treaty → sign
    let counts = EventCounts::from_events(&all_events);
    assert!(
        counts.leadership_changed >= 1,
        "Should have leadership change"
    );
    assert!(counts.relation_changed >= 1, "Should have relation change");
    assert!(counts.treaty_proposed >= 1, "Should have treaty proposed");
    assert!(counts.treaty_signed >= 1, "Should have treaty signed");

    // Final relation should be positive
    let final_relation = diplomacy.get_relation(&org_a.id, &org_b.id);
    assert!(
        final_relation.is_positive(),
        "Relations should be positive after diplomacy"
    );
}
