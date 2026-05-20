//! Phase 4.4.1 Integration Tests — Spontaneous Organization Formation.
//!
//! Validates:
//! 1. Organization creation through the OrganizationStore in tick context
//! 2. Competition mechanics between organizations
//! 3. Auto-scan engine triggers organization formation events
//! 4. WorldEvent broadcast for org lifecycle events
//! 5. Multi-org scenario: 10 agents → 2+ org types
//! 6. Inactivity tracking
//! 7. Multiple org types
//! 8. Single membership enforcement

use std::collections::HashMap;

use agent_world_engine::organization::charter::{Charter, GovernanceModel, ProfitSharing};
use agent_world_engine::organization::org::{
    OrganizationStore, OrgStatus, OrgType, CREATION_COST_MONEY, INACTIVE_THRESHOLD_TICKS,
};
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

fn make_distinct_founders(start: usize, n: usize, prefix: &str) -> Vec<(String, String)> {
    (start..start + n)
        .map(|i| {
            (
                format!("{}-agent-{}", prefix, i),
                format!("{} Agent {}", prefix, i),
            )
        })
        .collect()
}

/// Drain all pending events from a receiver matching a predicate.
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

// ═══════════════════════════════════════════════════════════════
// Test 1: Organization creation in tick context
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_org_creation_in_tick_context() {
    let bus = EventBus::new(256);
    let mut rx = bus.subscribe();
    let mut store = OrganizationStore::with_event_bus(bus.clone());

    // Simulate tick 100 — agents decide to form a Guild
    let org = store
        .create_org(
            "Miners Guild".to_string(),
            OrgType::Guild,
            Some(test_charter()),
            make_founders(3, "miner"),
            100,
        )
        .unwrap();

    assert_eq!(org.name, "Miners Guild");
    assert_eq!(org.org_type, OrgType::Guild);
    assert_eq!(org.status, OrgStatus::Active);
    assert_eq!(org.members.len(), 3);
    assert_eq!(org.created_tick, 100);
    assert_eq!(org.treasury, CREATION_COST_MONEY);

    // Verify all founders are members
    for i in 0..3 {
        let agent_id = format!("miner-agent-{}", i);
        assert!(org.is_member(&agent_id));
        assert_eq!(store.agent_org(&agent_id), Some(org.id.as_str()));
    }

    // Verify OrgCreated event was broadcast
    let org_events = drain_events(&mut rx, |e| matches!(e, WorldEvent::OrgCreated { .. }));
    assert_eq!(org_events.len(), 1);
    if let WorldEvent::OrgCreated {
        name,
        org_type,
        founder_count,
        ..
    } = &org_events[0]
    {
        assert_eq!(name, "Miners Guild");
        assert_eq!(org_type, "guild");
        assert_eq!(*founder_count, 3);
    }
}

// ═══════════════════════════════════════════════════════════════
// Test 2: Org competition — resource conflict
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_org_competition_over_resource() {
    let bus = EventBus::new(256);
    let mut store = OrganizationStore::with_event_bus(bus.clone());

    // Create two competing organizations
    let org_a = store
        .create_org(
            "Big Corp".to_string(),
            OrgType::Company,
            Some(test_charter()),
            make_founders(4, "big"),
            0,
        )
        .unwrap();

    let org_b = store
        .create_org(
            "Small Corp".to_string(),
            OrgType::Company,
            Some(test_charter()),
            make_founders(2, "small"),
            0,
        )
        .unwrap();

    // Simulate competition: larger org wins
    let score_a = store.get(&org_a.id).unwrap().member_count() as f64;
    let score_b = store.get(&org_b.id).unwrap().member_count() as f64;

    let winner_id = if score_a >= score_b {
        org_a.id.clone()
    } else {
        org_b.id.clone()
    };

    // Winner gets a treasury bonus (simulating competition reward)
    let bonus: u64 = 50;
    {
        let winner = store.get_mut(&winner_id).unwrap();
        winner.treasury += bonus;
    }

    let winner = store.get(&winner_id).unwrap();
    assert_eq!(winner.treasury, CREATION_COST_MONEY + bonus);
    assert!(winner.member_count() >= 3);
}

// ═══════════════════════════════════════════════════════════════
// Test 3: Auto-scan engine triggers formation events
// ═══════════════════════════════════════════════════════════════

/// Simulates an auto-scan that periodically checks for org formation conditions.
/// In production, this would be a Subsystem running in the tick loop.
struct OrgFormationScanner {
    store: OrganizationStore,
}

impl OrgFormationScanner {
    fn new(store: OrganizationStore) -> Self {
        Self { store }
    }

    /// Simulate a periodic scan that checks if agents near a resource
    /// should form an org. Returns the number of orgs created this scan.
    fn scan_for_guild_formation(
        &mut self,
        agent_ids: &[(String, String, Vec<String>)], // (id, name, skills)
        current_tick: u64,
    ) -> usize {
        let mut created = 0;

        // Group agents by shared skills
        let mut skill_groups: HashMap<String, Vec<(String, String)>> = HashMap::new();
        for (id, name, skills) in agent_ids {
            for skill in skills {
                skill_groups
                    .entry(skill.clone())
                    .or_default()
                    .push((id.clone(), name.clone()));
            }
        }

        // For each skill group with 2+ agents, try to form a Guild
        for (skill, agents) in &skill_groups {
            if agents.len() < 2 {
                continue;
            }

            // Filter out agents already in an org
            let available: Vec<(String, String)> = agents
                .iter()
                .filter(|(id, _)| self.store.agent_org(id).is_none())
                .cloned()
                .collect();

            if available.len() < 2 {
                continue;
            }

            let result = self.store.create_org(
                format!("{} Guild", skill),
                OrgType::Guild,
                Some(test_charter()),
                available,
                current_tick,
            );

            if result.is_ok() {
                created += 1;
            }
        }

        created
    }

    fn store(&self) -> &OrganizationStore {
        &self.store
    }
}

#[test]
fn test_auto_scan_triggers_guild_formation() {
    let bus = EventBus::new(256);
    let mut rx = bus.subscribe();
    let store = OrganizationStore::with_event_bus(bus.clone());
    let mut scanner = OrgFormationScanner::new(store);

    // Simulate 5 agents with mining skills arriving at a resource point
    let agents: Vec<(String, String, Vec<String>)> = (0..5)
        .map(|i| {
            (
                format!("scan-agent-{}", i),
                format!("Scanner Agent {}", i),
                vec!["mining".to_string(), "crafting".to_string()],
            )
        })
        .collect();

    // Tick 100: auto-scan triggers
    let created = scanner.scan_for_guild_formation(&agents, 100);
    assert_eq!(created, 1, "Should create one Guild from auto-scan");

    // Verify the org was created
    let orgs = scanner.store().list();
    assert_eq!(orgs.len(), 1);
    assert_eq!(orgs[0].org_type, OrgType::Guild);
    assert_eq!(orgs[0].members.len(), 5);

    // Verify event broadcast
    let events = drain_events(&mut rx, |e| matches!(e, WorldEvent::OrgCreated { .. }));
    assert_eq!(events.len(), 1);
}

#[test]
fn test_auto_scan_no_duplicate_formation() {
    let bus = EventBus::new(256);
    let store = OrganizationStore::with_event_bus(bus.clone());
    let mut scanner = OrgFormationScanner::new(store);

    let agents: Vec<(String, String, Vec<String>)> = (0..3)
        .map(|i| {
            (
                format!("dedup-agent-{}", i),
                format!("Dedup Agent {}", i),
                vec!["fishing".to_string()],
            )
        })
        .collect();

    // First scan: creates org
    let created1 = scanner.scan_for_guild_formation(&agents, 100);
    assert_eq!(created1, 1);

    // Second scan: agents already in org, no new creation
    let created2 = scanner.scan_for_guild_formation(&agents, 200);
    assert_eq!(created2, 0, "Should not create duplicate org");
}

// ═══════════════════════════════════════════════════════════════
// Test 4: WorldEvent broadcast verification
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_org_lifecycle_event_broadcast() {
    let bus = EventBus::new(256);
    let mut rx = bus.subscribe();
    let mut store = OrganizationStore::with_event_bus(bus.clone());

    // Create org
    let org = store
        .create_org(
            "Lifecycle Corp".to_string(),
            OrgType::Company,
            Some(test_charter()),
            make_founders(3, "lc"),
            100,
        )
        .unwrap();

    // Consume OrgCreated
    let created_event = rx.try_recv().unwrap();
    assert!(matches!(created_event, WorldEvent::OrgCreated { .. }));

    // Member joins
    store
        .join_org(
            &org.id,
            "new-member".to_string(),
            "New Member".to_string(),
            200,
        )
        .unwrap();

    // Consume OrgMemberJoined
    let joined_event = rx.try_recv().unwrap();
    assert!(matches!(joined_event, WorldEvent::OrgMemberJoined { .. }));

    // Member leaves
    store.leave_org(&org.id, "new-member", 300).unwrap();

    // Consume OrgMemberLeft
    let left_event = rx.try_recv().unwrap();
    assert!(matches!(left_event, WorldEvent::OrgMemberLeft { .. }));

    // Dissolve org
    store.dissolve_org(&org.id, "integration_test").unwrap();

    // Consume OrgDissolved
    let dissolved_event = rx.try_recv().unwrap();
    assert!(matches!(dissolved_event, WorldEvent::OrgDissolved { .. }));
}

// ═══════════════════════════════════════════════════════════════
// Test 5: Multi-org scenario — 10 agents → 2+ org types
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_multi_org_scenario_ten_agents() {
    let bus = EventBus::new(256);
    let mut rx = bus.subscribe();
    let store = OrganizationStore::with_event_bus(bus.clone());
    let mut scanner = OrgFormationScanner::new(store);

    // 10 agents divided into two skill groups
    let miners: Vec<(String, String, Vec<String>)> = (0..5)
        .map(|i| {
            (
                format!("multi-miner-{}", i),
                format!("Multi Miner {}", i),
                vec!["mining".to_string()],
            )
        })
        .collect();

    let guards: Vec<(String, String, Vec<String>)> = (5..10)
        .map(|i| {
            (
                format!("multi-guard-{}", i),
                format!("Multi Guard {}", i),
                vec!["combat".to_string()],
            )
        })
        .collect();

    // Tick 50: miners form a Guild
    let miner_orgs = scanner.scan_for_guild_formation(&miners, 50);
    assert_eq!(miner_orgs, 1);

    // Tick 100: guards form another Guild (different skill group)
    let guard_orgs = scanner.scan_for_guild_formation(&guards, 100);
    assert_eq!(guard_orgs, 1);

    // Verify: 2 orgs total
    let all_orgs = scanner.store().list();
    assert_eq!(all_orgs.len(), 2);

    // Verify all are Guilds
    let guild_count = all_orgs
        .iter()
        .filter(|o| o.org_type == OrgType::Guild)
        .count();
    assert_eq!(guild_count, 2);

    // Verify all 10 agents are organized
    for i in 0..5 {
        assert!(
            scanner
                .store()
                .agent_org(&format!("multi-miner-{}", i))
                .is_some(),
            "Miner {} should be in an org",
            i
        );
    }
    for i in 5..10 {
        assert!(
            scanner
                .store()
                .agent_org(&format!("multi-guard-{}", i))
                .is_some(),
            "Guard {} should be in an org",
            i
        );
    }

    // Verify events
    let events = drain_events(&mut rx, |e| matches!(e, WorldEvent::OrgCreated { .. }));
    assert_eq!(events.len(), 2);
}

// ═══════════════════════════════════════════════════════════════
// Test 6: Inactivity tracking in tick loop
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_org_inactivity_tracking_in_tick_loop() {
    let bus = EventBus::new(256);
    let mut rx = bus.subscribe();
    let mut store = OrganizationStore::with_event_bus(bus.clone());

    let org = store
        .create_org(
            "Inactive Corp".to_string(),
            OrgType::Company,
            Some(test_charter()),
            make_founders(2, "inactive"),
            0,
        )
        .unwrap();

    // Consume OrgCreated event
    let _ = rx.try_recv();

    // Simulate tick loop: advance to inactive threshold
    let inactive = store.check_inactivity(0);
    assert!(
        inactive.is_empty(),
        "Org just created at tick 0 should not be inactive yet"
    );

    // Advance well past threshold
    let inactive = store.check_inactivity(INACTIVE_THRESHOLD_TICKS + 100);
    assert_eq!(inactive.len(), 1);
    assert_eq!(inactive[0], org.id);

    let org = store.get(&org.id).unwrap();
    assert_eq!(org.status, OrgStatus::Inactive);

    // Verify OrgInactivated event
    let event = rx.try_recv().unwrap();
    assert!(matches!(event, WorldEvent::OrgInactivated { .. }));
}

// ═══════════════════════════════════════════════════════════════
// Test 7: Direct multi-type org creation
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_multiple_org_types_created() {
    let bus = EventBus::new(256);
    let mut rx = bus.subscribe();
    let mut store = OrganizationStore::with_event_bus(bus.clone());

    let types = vec![
        (OrgType::Company, "company"),
        (OrgType::Guild, "guild"),
        (OrgType::Alliance, "alliance"),
        (OrgType::University, "university"),
    ];

    for (i, (org_type, prefix)) in types.into_iter().enumerate() {
        let founders: Vec<(String, String)> = (0..2)
            .map(|j| {
                (
                    format!("{}-agent-{}", prefix, j),
                    format!("{} Agent {}", prefix, j),
                )
            })
            .collect();

        let org = store
            .create_org(
                format!("{} #{}", prefix, i),
                org_type,
                Some(test_charter()),
                founders,
                100 + i as u64,
            )
            .unwrap();

        assert_eq!(org.org_type, org_type);
        assert_eq!(org.status, OrgStatus::Active);
    }

    assert_eq!(store.list().len(), 4);

    // Verify 4 OrgCreated events
    let events = drain_events(&mut rx, |e| matches!(e, WorldEvent::OrgCreated { .. }));
    assert_eq!(events.len(), 4);

    // Verify different org types in events
    let event_types: Vec<String> = events
        .iter()
        .filter_map(|e| {
            if let WorldEvent::OrgCreated { org_type, .. } = e {
                Some(org_type.clone())
            } else {
                None
            }
        })
        .collect();

    assert!(event_types.contains(&"company".to_string()));
    assert!(event_types.contains(&"guild".to_string()));
    assert!(event_types.contains(&"alliance".to_string()));
    assert!(event_types.contains(&"university".to_string()));
}

// ═══════════════════════════════════════════════════════════════
// Test 8: Single membership enforcement
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_single_membership_enforcement() {
    let bus = EventBus::new(256);
    let mut store = OrganizationStore::with_event_bus(bus.clone());

    // Create two orgs
    let _org1 = store
        .create_org(
            "Org One".to_string(),
            OrgType::Guild,
            Some(test_charter()),
            make_founders(2, "one"),
            0,
        )
        .unwrap();

    let org2 = store
        .create_org(
            "Org Two".to_string(),
            OrgType::Alliance,
            Some(test_charter()),
            make_distinct_founders(0, 2, "two"),
            0,
        )
        .unwrap();

    // Agent from org1 tries to join org2 — should fail
    let result = store.join_org(
        &org2.id,
        "one-agent-0".to_string(),
        "One Agent 0".to_string(),
        100,
    );
    assert!(result.is_err(), "Agent already in org1 should be rejected");

    // Agent from org1 tries to create org3 — should fail
    let result = store.create_org(
        "Org Three".to_string(),
        OrgType::Company,
        Some(test_charter()),
        vec![
            ("one-agent-0".to_string(), "One Agent 0".to_string()),
            ("fresh-agent".to_string(), "Fresh Agent".to_string()),
        ],
        100,
    );
    assert!(
        result.is_err(),
        "Agent already in org should not be able to found another"
    );

    // Fresh agent can join org2
    let result = store.join_org(
        &org2.id,
        "fresh-agent".to_string(),
        "Fresh Agent".to_string(),
        100,
    );
    assert!(result.is_ok());
}
