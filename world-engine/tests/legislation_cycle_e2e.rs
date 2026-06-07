//! End-to-end integration test for the full self-legislation cycle.

use serde_json::json;

use agent_world_engine::organization::governance::{DecisionMode, GovernanceSystem};
use agent_world_engine::organization::leadership::{LeadershipEngine, VotingMethod};
use agent_world_engine::organization::legislation_cycle::{
    CandidateRule, CycleStatus, LegislationCycleConfig, LegislationCycleEngine,
};
use agent_world_engine::organization::rule_engine::{
    RuleCondition, RuleEffect, RuleStatus, RuleType,
};

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

fn make_behavior_rule(proposer: &str, title: &str) -> CandidateRule {
    CandidateRule {
        proposer_id: proposer.to_string(),
        title: title.to_string(),
        description: format!("{} behavioral rule", title),
        rule_type: RuleType::Behavior,
        conditions: vec![RuleCondition {
            field: "world.tick".to_string(),
            operator: ">=".to_string(),
            value: json!(0),
        }],
        effects: vec![RuleEffect {
            target: "agent.behavior_modifier".to_string(),
            action: "set".to_string(),
            value: json!("cooperative"),
        }],
        expires_tick: None,
    }
}

fn setup_system() -> (LegislationCycleEngine, GovernanceSystem, LeadershipEngine) {
    let config = LegislationCycleConfig {
        min_proposals: 1,
        quorum: 2,
        auto_trigger: true,
        election_method: VotingMethod::SimpleMajority,
        election_interval_ticks: 50,
        min_members_for_auto_trigger: 3,
        allow_repeal_proposals: true,
    };
    (LegislationCycleEngine::new(config), GovernanceSystem::new(), LeadershipEngine::new())
}

#[test]
fn test_full_legislation_cycle_with_rule_execution() {
    let (mut engine, mut governance, mut leadership) = setup_system();
    let org_id = governance.create_org("Legislation Guild".to_string(), "founder".to_string(), DecisionMode::Vote, 0).unwrap();
    governance.join_org(org_id, "member1".to_string(), 1).unwrap();
    governance.join_org(org_id, "member2".to_string(), 1).unwrap();
    governance.join_org(org_id, "member3".to_string(), 1).unwrap();
    governance.join_org(org_id, "member4".to_string(), 1).unwrap();

    let members = vec!["founder".to_string(), "member1".to_string(), "member2".to_string(), "member3".to_string(), "member4".to_string()];

    // Phase 1: Start cycle and run election
    engine.start_cycle(org_id, members.clone(), 10, "regular governance review").unwrap();
    leadership.initiate_election(org_id, members.clone(), VotingMethod::SimpleMajority, 10).unwrap();
    leadership.cast_vote(org_id, "founder".to_string(), vec!["founder".to_string()]).unwrap();
    leadership.cast_vote(org_id, "member1".to_string(), vec!["founder".to_string()]).unwrap();
    leadership.cast_vote(org_id, "member2".to_string(), vec!["founder".to_string()]).unwrap();
    leadership.cast_vote(org_id, "member3".to_string(), vec!["member1".to_string()]).unwrap();
    leadership.cast_vote(org_id, "member4".to_string(), vec!["member1".to_string()]).unwrap();
    leadership.resolve_election(org_id).unwrap();
    engine.resolve_election(&leadership, org_id).unwrap();

    let record = engine.get_cycle(org_id).unwrap();
    assert_eq!(record.status, CycleStatus::CollectingProposals);
    assert_eq!(record.leader_id, Some("founder".to_string()));

    // Phase 2: Leader submits candidate rules
    engine.submit_candidate_rule(org_id, make_candidate_rule("founder", "Wealth Tax")).unwrap();
    engine.submit_candidate_rule(org_id, make_behavior_rule("founder", "Cooperation Mandate")).unwrap();
    assert_eq!(engine.get_candidate_rules(org_id).unwrap().len(), 2);

    // Phase 3: Start voting
    engine.start_voting_phase(&mut governance, org_id, 20).unwrap();
    assert_eq!(engine.get_cycle_status(org_id), CycleStatus::VotingOpen);

    // Phase 4: Members vote
    engine.cast_vote(&mut governance, org_id, "founder".to_string(), true, 21).unwrap();
    engine.cast_vote(&mut governance, org_id, "member1".to_string(), true, 22).unwrap();
    engine.cast_vote(&mut governance, org_id, "member2".to_string(), true, 23).unwrap();
    engine.cast_vote(&mut governance, org_id, "member3".to_string(), true, 24).unwrap();
    engine.cast_vote(&mut governance, org_id, "member4".to_string(), false, 25).unwrap();

    // Phase 5: Tally and enact
    let enacted = engine.tally_and_enact(&mut governance, org_id, 30).unwrap();
    assert_eq!(enacted.len(), 2);
    assert_eq!(engine.get_cycle_status(org_id), CycleStatus::Enacted);

    // Phase 6: Verify rules are active
    assert_eq!(governance.active_rules.active_rule_count(), 2);
    for rule_id in &enacted {
        let rule = governance.active_rules.get_rule(rule_id).unwrap();
        assert_eq!(rule.status, RuleStatus::Active);
        assert_eq!(rule.org_id, org_id.to_string());
    }

    // Phase 7: Verify rules produce observable effects
    let context = json!({"agent": {"resources": 500, "tax_bonus": 0.0, "behavior_modifier": "neutral"}, "world": {"tick": 30}});
    let effects = governance.active_rules.evaluate_rules_for_org(&org_id.to_string(), &context);
    assert_eq!(effects.len(), 2);
    assert!(effects.iter().any(|e| e.target == "agent.tax_bonus"));
    assert!(effects.iter().any(|e| e.target == "agent.behavior_modifier"));

    // Phase 8: Feedback loop
    let summary = engine.evaluate_cycle_effects(&governance.active_rules, org_id);
    assert_eq!(summary.total_enacted, 2);
    assert_eq!(summary.still_active, 2);
}

#[test]
fn test_auto_trigger_legislation_cycle() {
    let (mut engine, mut governance, mut leadership) = setup_system();
    let org_id = governance.create_org("Auto Guild".to_string(), "founder".to_string(), DecisionMode::Vote, 0).unwrap();
    governance.join_org(org_id, "member1".to_string(), 1).unwrap();
    governance.join_org(org_id, "member2".to_string(), 1).unwrap();

    assert!(engine.tick_org(org_id, 40, 5, vec!["founder".to_string(), "member1".to_string()]).is_none());
    assert!(engine.tick_org(org_id, 50, 5, vec!["founder".to_string(), "member1".to_string(), "member2".to_string()]).is_some());

    leadership.initiate_election(org_id, vec!["founder".to_string(), "member1".to_string(), "member2".to_string()], VotingMethod::SimpleMajority, 50).unwrap();
    leadership.cast_vote(org_id, "founder".to_string(), vec!["founder".to_string()]).unwrap();
    leadership.cast_vote(org_id, "member1".to_string(), vec!["founder".to_string()]).unwrap();
    leadership.cast_vote(org_id, "member2".to_string(), vec!["member1".to_string()]).unwrap();
    leadership.resolve_election(org_id).unwrap();
    engine.resolve_election(&leadership, org_id).unwrap();
    engine.submit_candidate_rule(org_id, make_candidate_rule("founder", "Auto Tax")).unwrap();
    engine.start_voting_phase(&mut governance, org_id, 55).unwrap();
    engine.cast_vote(&mut governance, org_id, "founder".to_string(), true, 56).unwrap();
    engine.cast_vote(&mut governance, org_id, "member1".to_string(), true, 57).unwrap();
    let enacted = engine.tally_and_enact(&mut governance, org_id, 60).unwrap();
    assert_eq!(enacted.len(), 1);
    assert!(engine.should_auto_trigger(org_id, 109, 5).is_none());
    assert!(engine.should_auto_trigger(org_id, 110, 5).is_some());
}

#[test]
fn test_legislation_cycle_repeal_existing_rule() {
    let (mut engine, mut governance, mut leadership) = setup_system();
    let org_id = governance.create_org("Repeal Corp".to_string(), "founder".to_string(), DecisionMode::Vote, 0).unwrap();
    governance.join_org(org_id, "member1".to_string(), 1).unwrap();
    governance.join_org(org_id, "member2".to_string(), 1).unwrap();

    let (_, enacted) = engine.run_full_cycle(
        &mut governance, &mut leadership, org_id,
        vec!["founder".to_string(), "member1".to_string(), "member2".to_string()],
        &[("founder".to_string(), true), ("member1".to_string(), true)],
        vec![make_candidate_rule("leader", "Original Tax")],
        10, "first legislation",
    ).unwrap();
    assert_eq!(enacted.len(), 1);
    let rule_id = &enacted[0];
    assert_eq!(governance.active_rules.active_rule_count(), 1);

    let ctx = json!({"agent": {"resources": 500, "tax_bonus": 0.0}});
    assert!(!governance.active_rules.evaluate_rules(&ctx).is_empty());

    engine.start_cycle_with_leader(org_id, "founder".to_string(), 50, "repeal").unwrap();
    engine.submit_repeal_proposal(org_id, "founder".to_string(), rule_id.clone(), "economic damage".to_string()).unwrap();
    engine.start_voting_phase(&mut governance, org_id, 55).unwrap();
    engine.cast_vote(&mut governance, org_id, "founder".to_string(), true, 56).unwrap();
    engine.cast_vote(&mut governance, org_id, "member1".to_string(), true, 57).unwrap();
    let repeal_enacted = engine.tally_and_enact(&mut governance, org_id, 60).unwrap();
    assert!(!repeal_enacted.is_empty());

    let repealed = engine.process_repeal_effects(&mut governance.active_rules, org_id, 60);
    assert_eq!(repealed.len(), 1);
    assert_eq!(governance.active_rules.get_rule(rule_id).unwrap().status, RuleStatus::Repealed);
}

#[test]
fn test_multi_org_legislation_with_event_triggers() {
    let (mut engine, mut governance, mut leadership) = setup_system();
    let org_a = governance.create_org("Org Alpha".to_string(), "a_founder".to_string(), DecisionMode::Vote, 0).unwrap();
    governance.join_org(org_a, "a_member1".to_string(), 1).unwrap();
    governance.join_org(org_a, "a_member2".to_string(), 1).unwrap();
    let org_b = governance.create_org("Org Beta".to_string(), "b_founder".to_string(), DecisionMode::Vote, 0).unwrap();
    governance.join_org(org_b, "b_member1".to_string(), 1).unwrap();
    governance.join_org(org_b, "b_member2".to_string(), 1).unwrap();

    engine.trigger_from_event(org_a, "economic crisis", 30, vec!["a_founder".to_string(), "a_member1".to_string()]).unwrap();
    assert!(engine.get_cycle(org_a).unwrap().trigger_reason.contains("event-trigger"));

    leadership.initiate_election(org_a, vec!["a_founder".to_string(), "a_member1".to_string()], VotingMethod::SimpleMajority, 30).unwrap();
    leadership.cast_vote(org_a, "a_founder".to_string(), vec!["a_founder".to_string()]).unwrap();
    leadership.cast_vote(org_a, "a_member1".to_string(), vec!["a_founder".to_string()]).unwrap();
    leadership.resolve_election(org_a).unwrap();
    engine.resolve_election(&leadership, org_a).unwrap();
    engine.submit_candidate_rule(org_a, make_candidate_rule("a_founder", "Crisis Tax")).unwrap();
    engine.start_voting_phase(&mut governance, org_a, 35).unwrap();
    engine.cast_vote(&mut governance, org_a, "a_founder".to_string(), true, 36).unwrap();
    engine.cast_vote(&mut governance, org_a, "a_member1".to_string(), true, 37).unwrap();
    assert_eq!(engine.tally_and_enact(&mut governance, org_a, 40).unwrap().len(), 1);

    let triggered = engine.tick_auto_trigger(50, &[(org_a, 3), (org_b, 3)]);
    assert_eq!(triggered.len(), 1);
    assert_eq!(triggered[0].0, org_b);

    leadership.initiate_election(org_b, vec!["b_founder".to_string(), "b_member1".to_string(), "b_member2".to_string()], VotingMethod::SimpleMajority, 50).unwrap();
    leadership.cast_vote(org_b, "b_founder".to_string(), vec!["b_founder".to_string()]).unwrap();
    leadership.cast_vote(org_b, "b_member1".to_string(), vec!["b_founder".to_string()]).unwrap();
    leadership.cast_vote(org_b, "b_member2".to_string(), vec!["b_founder".to_string()]).unwrap();
    leadership.resolve_election(org_b).unwrap();
    engine.resolve_election(&leadership, org_b).unwrap();
    engine.submit_candidate_rule(org_b, make_behavior_rule("b_founder", "Trade Alliance")).unwrap();
    engine.start_voting_phase(&mut governance, org_b, 55).unwrap();
    engine.cast_vote(&mut governance, org_b, "b_founder".to_string(), true, 56).unwrap();
    engine.cast_vote(&mut governance, org_b, "b_member1".to_string(), true, 57).unwrap();
    engine.cast_vote(&mut governance, org_b, "b_member2".to_string(), true, 58).unwrap();
    assert_eq!(engine.tally_and_enact(&mut governance, org_b, 60).unwrap().len(), 1);

    assert_eq!(governance.active_rules.active_rule_count(), 2);
    assert_eq!(governance.active_rules.active_rules_for_org(&org_a.to_string()).len(), 1);
    assert_eq!(governance.active_rules.active_rules_for_org(&org_b.to_string()).len(), 1);

    let ctx = json!({"agent": {"resources": 500, "tax_bonus": 0.0, "behavior_modifier": "neutral"}, "world": {"tick": 60}});
    let eff_a = governance.active_rules.evaluate_rules_for_org(&org_a.to_string(), &ctx);
    let eff_b = governance.active_rules.evaluate_rules_for_org(&org_b.to_string(), &ctx);
    assert_eq!(eff_a.len(), 1);
    assert_eq!(eff_a[0].target, "agent.tax_bonus");
    assert_eq!(eff_b.len(), 1);
    assert_eq!(eff_b[0].target, "agent.behavior_modifier");
}

#[test]
fn test_rejected_legislation_and_feedback() {
    let (mut engine, mut governance, mut leadership) = setup_system();
    let org_id = governance.create_org("Reject Guild".to_string(), "founder".to_string(), DecisionMode::Vote, 0).unwrap();
    governance.join_org(org_id, "member1".to_string(), 1).unwrap();
    governance.join_org(org_id, "member2".to_string(), 1).unwrap();

    let (_, enacted) = engine.run_full_cycle(
        &mut governance, &mut leadership, org_id,
        vec!["founder".to_string(), "member1".to_string(), "member2".to_string()],
        &[("founder".to_string(), false), ("member1".to_string(), false), ("member2".to_string(), true)],
        vec![make_candidate_rule("leader", "Unpopular Tax")],
        10, "rejected test",
    ).unwrap();
    assert!(enacted.is_empty());
    assert_eq!(engine.get_cycle_status(org_id), CycleStatus::Rejected);
    assert_eq!(governance.active_rules.active_rule_count(), 0);
    let summary = engine.evaluate_cycle_effects(&governance.active_rules, org_id);
    assert_eq!(summary.total_enacted, 0);
}

#[test]
fn test_legislation_rule_expiry() {
    let (mut engine, mut governance, mut leadership) = setup_system();
    let org_id = governance.create_org("Expiry Corp".to_string(), "founder".to_string(), DecisionMode::Vote, 0).unwrap();
    governance.join_org(org_id, "member1".to_string(), 1).unwrap();
    governance.join_org(org_id, "member2".to_string(), 1).unwrap();

    let expiring_rule = CandidateRule {
        proposer_id: "leader".to_string(),
        title: "Temporary Tax".to_string(),
        description: "Expires after 50 ticks".to_string(),
        rule_type: RuleType::Tax,
        conditions: vec![RuleCondition { field: "agent.resources".to_string(), operator: ">".to_string(), value: json!(100) }],
        effects: vec![RuleEffect { target: "agent.temp_tax".to_string(), action: "set".to_string(), value: json!(0.05) }],
        expires_tick: Some(80),
    };

    let (_, enacted) = engine.run_full_cycle(
        &mut governance, &mut leadership, org_id,
        vec!["founder".to_string(), "member1".to_string(), "member2".to_string()],
        &[("founder".to_string(), true), ("member1".to_string(), true)],
        vec![expiring_rule],
        10, "temporary rule test",
    ).unwrap();
    assert_eq!(enacted.len(), 1);
    assert_eq!(governance.active_rules.active_rule_count(), 1);

    governance.active_rules.expire_rules(79);
    assert_eq!(governance.active_rules.active_rule_count(), 1);
    assert!(!governance.active_rules.evaluate_rules(&json!({"agent": {"resources": 500}})).is_empty());

    let expired = governance.active_rules.expire_rules(80);
    assert_eq!(expired.len(), 1);
    assert_eq!(governance.active_rules.active_rule_count(), 0);

    let summary = engine.evaluate_cycle_effects(&governance.active_rules, org_id);
    assert_eq!(summary.total_enacted, 1);
    assert_eq!(summary.still_active, 0);
    // Expired rules get status Repealed in the engine, counted as repealed
    assert_eq!(summary.repealed, 1);
}
