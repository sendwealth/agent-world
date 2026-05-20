//! Soft rule engine — Agent-proposed rules evaluated declaratively.
//!
//! Agents can propose rules with conditions and effects. When a rule's conditions
//! match the current world state, its effects are applied. The engine supports
//! six comparison operators (`>`, `<`, `==`, `>=`, `<=`, `contains`) and four
//! effect actions (`add`, `subtract`, `multiply`, `set`, `block_action`).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::world::event::WorldEvent;
use crate::world::state::EventBus;
use std::sync::Arc;

// ── Rule Types ─────────────────────────────────────────────

/// Category of a soft rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleType {
    /// Tax rate adjustment (e.g. extra 10% tax on outsiders).
    Tax,
    /// Behavioral constraint (e.g. no gathering in a zone).
    Behavior,
    /// Trade rule (e.g. minimum trade price).
    Trade,
    /// Diplomatic rule (e.g. interaction limits with non-allies).
    Diplomacy,
    /// LLM-generated custom rule.
    Custom,
}

impl std::fmt::Display for RuleType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RuleType::Tax => write!(f, "tax"),
            RuleType::Behavior => write!(f, "behavior"),
            RuleType::Trade => write!(f, "trade"),
            RuleType::Diplomacy => write!(f, "diplomacy"),
            RuleType::Custom => write!(f, "custom"),
        }
    }
}

// ── Rule Status ────────────────────────────────────────────

/// Lifecycle status of a soft rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleStatus {
    /// Proposed but not yet voted on / activated.
    Proposed,
    /// Active and evaluated every tick.
    Active,
    /// Temporarily suspended.
    Suspended,
    /// Permanently repealed.
    Repealed,
}

impl std::fmt::Display for RuleStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RuleStatus::Proposed => write!(f, "proposed"),
            RuleStatus::Active => write!(f, "active"),
            RuleStatus::Suspended => write!(f, "suspended"),
            RuleStatus::Repealed => write!(f, "repealed"),
        }
    }
}

// ── Condition & Effect ─────────────────────────────────────

/// A trigger condition for a rule.
///
/// `field` is a dot-path into the world state context (e.g. `"agent.tokens"`,
/// `"world.tick"`, `"org.treasury"`). The engine resolves the path against a
/// `serde_json::Value` context object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleCondition {
    /// Dot-path into the context (e.g. `"agent.hunger"`).
    pub field: String,
    /// Comparison operator: `>`, `<`, `==`, `>=`, `<=`, `contains`.
    pub operator: String,
    /// The value to compare against.
    pub value: Value,
}

/// An effect applied when a rule's conditions are met.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleEffect {
    /// Dot-path to the target field (e.g. `"agent.token"`).
    pub target: String,
    /// Action: `add`, `subtract`, `multiply`, `set`, `block_action`.
    pub action: String,
    /// Value used by the action.
    pub value: Value,
}

// ── SoftRule ───────────────────────────────────────────────

/// An agent-proposed soft rule with conditions and effects.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoftRule {
    pub id: String,
    pub proposer_id: String,
    pub org_id: String,
    pub title: String,
    pub description: String,
    pub rule_type: RuleType,
    pub conditions: Vec<RuleCondition>,
    pub effects: Vec<RuleEffect>,
    pub status: RuleStatus,
    pub created_tick: u64,
    pub votes_for: u32,
    pub votes_against: u32,
    /// Optional tick at which the rule automatically expires.
    pub expires_tick: Option<u64>,
}

impl SoftRule {
    /// Check whether all conditions match the given context.
    pub fn matches(&self, context: &Value) -> bool {
        self.conditions.iter().all(|c| evaluate_condition(c, context))
    }
}

// ── Rule Engine Error ──────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleEngineError {
    NotFound(String),
    AlreadyActive(String),
    NotProposed(String),
    AlreadyVoted { rule_id: String, voter_id: String },
    Expired(String),
    Repealed(String),
}

impl std::fmt::Display for RuleEngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RuleEngineError::NotFound(id) => write!(f, "rule not found: {}", id),
            RuleEngineError::AlreadyActive(id) => write!(f, "rule is already active: {}", id),
            RuleEngineError::NotProposed(id) => write!(f, "rule is not in proposed state: {}", id),
            RuleEngineError::AlreadyVoted { rule_id, voter_id } => {
                write!(f, "agent {} already voted on rule {}", voter_id, rule_id)
            }
            RuleEngineError::Expired(id) => write!(f, "rule has expired: {}", id),
            RuleEngineError::Repealed(id) => write!(f, "rule has been repealed: {}", id),
        }
    }
}

impl std::error::Error for RuleEngineError {}

// ── Vote Record ────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RuleVote {
    voter_id: String,
    in_favor: bool,
}

// ── Rule Engine ────────────────────────────────────────────

/// Core engine that stores, evaluates, and applies soft rules.
pub struct RuleEngine {
    rules: HashMap<String, SoftRule>,
    votes: HashMap<String, Vec<RuleVote>>,
    event_bus: Option<Arc<EventBus>>,
}

impl RuleEngine {
    /// Create a new empty rule engine.
    pub fn new() -> Self {
        Self {
            rules: HashMap::new(),
            votes: HashMap::new(),
            event_bus: None,
        }
    }

    /// Create with an event bus for broadcasting rule events.
    pub fn with_event_bus(event_bus: Arc<EventBus>) -> Self {
        Self {
            rules: HashMap::new(),
            votes: HashMap::new(),
            event_bus: Some(event_bus),
        }
    }

    // ── Rule Proposal ──────────────────────────────────────

    /// Propose a new soft rule. Returns the rule ID.
    pub fn propose_rule(
        &mut self,
        proposer_id: String,
        org_id: String,
        title: String,
        description: String,
        rule_type: RuleType,
        conditions: Vec<RuleCondition>,
        effects: Vec<RuleEffect>,
        created_tick: u64,
        expires_tick: Option<u64>,
    ) -> String {
        let id = Uuid::new_v4().to_string();
        let rule = SoftRule {
            id: id.clone(),
            proposer_id,
            org_id,
            title,
            description,
            rule_type,
            conditions,
            effects,
            status: RuleStatus::Proposed,
            created_tick,
            votes_for: 0,
            votes_against: 0,
            expires_tick,
        };
        self.rules.insert(id.clone(), rule);
        self.votes.insert(id.clone(), Vec::new());
        id
    }

    // ── Voting ─────────────────────────────────────────────

    /// Cast a vote on a proposed rule.
    pub fn vote_on_rule(
        &mut self,
        rule_id: &str,
        voter_id: String,
        support: bool,
    ) -> Result<(), RuleEngineError> {
        let rule = self.rules.get(rule_id)
            .ok_or_else(|| RuleEngineError::NotFound(rule_id.to_string()))?;

        if rule.status != RuleStatus::Proposed {
            return Err(RuleEngineError::NotProposed(rule_id.to_string()));
        }

        // Check for duplicate vote
        if let Some(votes) = self.votes.get(rule_id) {
            if votes.iter().any(|v| v.voter_id == voter_id) {
                return Err(RuleEngineError::AlreadyVoted {
                    rule_id: rule_id.to_string(),
                    voter_id,
                });
            }
        }

        let vote = RuleVote {
            voter_id: voter_id.clone(),
            in_favor: support,
        };
        self.votes.get_mut(rule_id).unwrap().push(vote);

        let rule = self.rules.get_mut(rule_id).unwrap();
        if support {
            rule.votes_for += 1;
        } else {
            rule.votes_against += 1;
        }

        Ok(())
    }

    /// Activate a proposed rule (called after governance vote passes).
    pub fn activate_rule(&mut self, rule_id: &str) -> Result<(), RuleEngineError> {
        let rule = self.rules.get_mut(rule_id)
            .ok_or_else(|| RuleEngineError::NotFound(rule_id.to_string()))?;

        if rule.status == RuleStatus::Active {
            return Err(RuleEngineError::AlreadyActive(rule_id.to_string()));
        }
        if rule.status == RuleStatus::Repealed {
            return Err(RuleEngineError::Repealed(rule_id.to_string()));
        }

        rule.status = RuleStatus::Active;
        Ok(())
    }

    // ── Evaluation ─────────────────────────────────────────

    /// Evaluate all active rules against the given context and return matching effects.
    ///
    /// `context` is a JSON object representing the current world state snapshot,
    /// e.g. `{"agent": {"tokens": 100, "hunger": 5}, "world": {"tick": 42}, ...}`.
    pub fn evaluate_rules(&self, context: &Value) -> Vec<RuleEffect> {
        let mut effects = Vec::new();
        for rule in self.rules.values() {
            if rule.status != RuleStatus::Active {
                continue;
            }
            if rule.matches(context) {
                effects.extend(rule.effects.clone());
            }
        }
        effects
    }

    /// Evaluate rules scoped to a specific organization.
    pub fn evaluate_rules_for_org(&self, org_id: &str, context: &Value) -> Vec<RuleEffect> {
        let mut effects = Vec::new();
        for rule in self.rules.values() {
            if rule.status != RuleStatus::Active {
                continue;
            }
            if rule.org_id != org_id {
                continue;
            }
            if rule.matches(context) {
                effects.extend(rule.effects.clone());
            }
        }
        effects
    }

    /// Get all active rules for an organization.
    pub fn active_rules_for_org(&self, org_id: &str) -> Vec<&SoftRule> {
        self.rules.values()
            .filter(|r| r.status == RuleStatus::Active && r.org_id == org_id)
            .collect()
    }

    // ── Repeal / Suspend / Expire ──────────────────────────

    /// Repeal a rule permanently.
    pub fn repeal_rule(&mut self, rule_id: &str, _repeal_tick: u64) -> Result<(), RuleEngineError> {
        let rule = self.rules.get_mut(rule_id)
            .ok_or_else(|| RuleEngineError::NotFound(rule_id.to_string()))?;
        rule.status = RuleStatus::Repealed;
        Ok(())
    }

    /// Suspend a rule temporarily.
    pub fn suspend_rule(&mut self, rule_id: &str) -> Result<(), RuleEngineError> {
        let rule = self.rules.get_mut(rule_id)
            .ok_or_else(|| RuleEngineError::NotFound(rule_id.to_string()))?;
        if rule.status == RuleStatus::Active {
            rule.status = RuleStatus::Suspended;
        }
        Ok(())
    }

    /// Resume a suspended rule.
    pub fn resume_rule(&mut self, rule_id: &str) -> Result<(), RuleEngineError> {
        let rule = self.rules.get_mut(rule_id)
            .ok_or_else(|| RuleEngineError::NotFound(rule_id.to_string()))?;
        if rule.status == RuleStatus::Suspended {
            rule.status = RuleStatus::Active;
        }
        Ok(())
    }

    /// Expire rules whose `expires_tick` has passed.
    pub fn expire_rules(&mut self, current_tick: u64) -> Vec<String> {
        let mut expired = Vec::new();
        for rule in self.rules.values_mut() {
            if rule.status == RuleStatus::Active {
                if let Some(expires) = rule.expires_tick {
                    if current_tick >= expires {
                        rule.status = RuleStatus::Repealed;
                        expired.push(rule.id.clone());
                    }
                }
            }
        }
        expired
    }

    // ── Query ──────────────────────────────────────────────

    pub fn get_rule(&self, rule_id: &str) -> Option<&SoftRule> {
        self.rules.get(rule_id)
    }

    pub fn list_rules(&self) -> Vec<&SoftRule> {
        self.rules.values().collect()
    }

    pub fn list_rules_for_org(&self, org_id: &str) -> Vec<&SoftRule> {
        self.rules.values().filter(|r| r.org_id == org_id).collect()
    }

    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }

    pub fn active_rule_count(&self) -> usize {
        self.rules.values().filter(|r| r.status == RuleStatus::Active).count()
    }

    // ── Helpers ────────────────────────────────────────────

    fn emit(&self, _event: WorldEvent) {
        if let Some(ref bus) = self.event_bus {
            bus.emit(_event);
        }
    }
}

impl Default for RuleEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ── Condition Evaluation ───────────────────────────────────

/// Resolve a dot-path (e.g. `"agent.hunger"`) against a JSON context.
fn resolve_path<'a>(path: &str, context: &'a Value) -> Option<&'a Value> {
    let mut current = context;
    for part in path.split('.') {
        match current {
            Value::Object(map) => {
                current = map.get(part)?;
            }
            _ => return None,
        }
    }
    Some(current)
}

/// Evaluate a single condition against the context.
fn evaluate_condition(condition: &RuleCondition, context: &Value) -> bool {
    let actual = match resolve_path(&condition.field, context) {
        Some(v) => v,
        None => return false,
    };

    match condition.operator.as_str() {
        ">" => compare_ordered(actual, &condition.value, |a, b| a > b),
        "<" => compare_ordered(actual, &condition.value, |a, b| a < b),
        "==" => actual == &condition.value,
        ">=" => compare_ordered(actual, &condition.value, |a, b| a >= b),
        "<=" => compare_ordered(actual, &condition.value, |a, b| a <= b),
        "contains" => {
            match (actual, &condition.value) {
                (Value::String(s), Value::String(needle)) => s.contains(needle.as_str()),
                (Value::Array(arr), _) => arr.contains(&condition.value),
                _ => false,
            }
        }
        _ => false,
    }
}

/// Compare two values as ordered (numbers). Falls back to false on type mismatch.
fn compare_ordered<F>(a: &Value, b: &Value, cmp: F) -> bool
where
    F: Fn(f64, f64) -> bool,
{
    let a_num = value_to_f64(a);
    let b_num = value_to_f64(b);
    match (a_num, b_num) {
        (Some(av), Some(bv)) => cmp(av, bv),
        _ => false,
    }
}

/// Convert a JSON value to f64 if possible.
fn value_to_f64(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.parse::<f64>().ok(),
        _ => None,
    }
}

// ── Apply Effect ───────────────────────────────────────────

/// Apply a single rule effect to a mutable context.
///
/// Returns `true` if the effect was applied successfully.
pub fn apply_effect(effect: &RuleEffect, context: &mut Value) -> bool {
    // Resolve the parent path and the final key
    let parts: Vec<&str> = effect.target.rsplitn(2, '.').collect();
    let (key, parent_path) = if parts.len() == 2 {
        (parts[0], parts[1])
    } else {
        (parts[0], "")
    };

    let parent = if parent_path.is_empty() {
        Some(context)
    } else {
        resolve_path_mut(parent_path, context)
    };

    let parent = match parent {
        Some(p) => p,
        None => return false,
    };

    match effect.action.as_str() {
        "set" => {
            if let Some(map) = parent.as_object_mut() {
                map.insert(key.to_string(), effect.value.clone());
                true
            } else {
                false
            }
        }
        "add" => {
            if let Some(map) = parent.as_object_mut() {
                if let Some(existing) = map.get(key) {
                    let new_val = add_values(existing, &effect.value);
                    map.insert(key.to_string(), new_val);
                    true
                } else {
                    map.insert(key.to_string(), effect.value.clone());
                    true
                }
            } else {
                false
            }
        }
        "subtract" => {
            if let Some(map) = parent.as_object_mut() {
                if let Some(existing) = map.get(key) {
                    let neg_value = negate_value(&effect.value);
                    let new_val = add_values(existing, &neg_value);
                    map.insert(key.to_string(), new_val);
                    true
                } else {
                    false
                }
            } else {
                false
            }
        }
        "multiply" => {
            if let Some(map) = parent.as_object_mut() {
                if let Some(existing) = map.get(key) {
                    let new_val = multiply_values(existing, &effect.value);
                    map.insert(key.to_string(), new_val);
                    true
                } else {
                    false
                }
            } else {
                false
            }
        }
        "block_action" => {
            // Mark the action as blocked in context
            if let Some(map) = parent.as_object_mut() {
                map.insert(
                    key.to_string(),
                    Value::String(format!("blocked:{}", effect.value)),
                );
                true
            } else {
                false
            }
        }
        _ => false,
    }
}

/// Mutable version of resolve_path.
fn resolve_path_mut<'a>(path: &str, context: &'a mut Value) -> Option<&'a mut Value> {
    let mut current = context;
    for part in path.split('.') {
        match current {
            Value::Object(map) => {
                current = map.get_mut(part)?;
            }
            _ => return None,
        }
    }
    Some(current)
}

fn add_values(a: &Value, b: &Value) -> Value {
    match (a, b) {
        (Value::Number(an), Value::Number(bn)) => {
            if let (Some(ai), Some(bi)) = (an.as_i64(), bn.as_i64()) {
                Value::Number(serde_json::Number::from(ai + bi))
            } else if let (Some(af), Some(bf)) = (an.as_f64(), bn.as_f64()) {
                serde_json::Number::from_f64(af + bf)
                    .map(Value::Number)
                    .unwrap_or(Value::Null)
            } else {
                Value::Null
            }
        }
        _ => Value::Null,
    }
}

fn negate_value(v: &Value) -> Value {
    match v {
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Number(serde_json::Number::from(-i))
            } else if let Some(f) = n.as_f64() {
                serde_json::Number::from_f64(-f)
                    .map(Value::Number)
                    .unwrap_or(Value::Null)
            } else {
                Value::Null
            }
        }
        _ => Value::Null,
    }
}

fn multiply_values(a: &Value, b: &Value) -> Value {
    let af = value_to_f64(a);
    let bf = value_to_f64(b);
    match (af, bf) {
        (Some(av), Some(bv)) => {
            serde_json::Number::from_f64(av * bv)
                .map(Value::Number)
                .unwrap_or(Value::Null)
        }
        _ => Value::Null,
    }
}

// ── Tests ──────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ── Condition Evaluation ───────────────────────────────

    #[test]
    fn test_evaluate_condition_greater_than() {
        let ctx = json!({ "agent": { "tokens": 100 } });
        let cond = RuleCondition {
            field: "agent.tokens".to_string(),
            operator: ">".to_string(),
            value: json!(50),
        };
        assert!(evaluate_condition(&cond, &ctx));

        let cond_fail = RuleCondition {
            field: "agent.tokens".to_string(),
            operator: ">".to_string(),
            value: json!(200),
        };
        assert!(!evaluate_condition(&cond_fail, &ctx));
    }

    #[test]
    fn test_evaluate_condition_less_than() {
        let ctx = json!({ "agent": { "tokens": 100 } });
        let cond = RuleCondition {
            field: "agent.tokens".to_string(),
            operator: "<".to_string(),
            value: json!(200),
        };
        assert!(evaluate_condition(&cond, &ctx));
    }

    #[test]
    fn test_evaluate_condition_equal() {
        let ctx = json!({ "world": { "tick": 42 } });
        let cond = RuleCondition {
            field: "world.tick".to_string(),
            operator: "==".to_string(),
            value: json!(42),
        };
        assert!(evaluate_condition(&cond, &ctx));
    }

    #[test]
    fn test_evaluate_condition_greater_equal() {
        let ctx = json!({ "agent": { "tokens": 100 } });
        let cond_ge = RuleCondition {
            field: "agent.tokens".to_string(),
            operator: ">=".to_string(),
            value: json!(100),
        };
        assert!(evaluate_condition(&cond_ge, &ctx));

        let cond_gt = RuleCondition {
            field: "agent.tokens".to_string(),
            operator: ">=".to_string(),
            value: json!(99),
        };
        assert!(evaluate_condition(&cond_gt, &ctx));
    }

    #[test]
    fn test_evaluate_condition_less_equal() {
        let ctx = json!({ "agent": { "tokens": 100 } });
        let cond = RuleCondition {
            field: "agent.tokens".to_string(),
            operator: "<=".to_string(),
            value: json!(100),
        };
        assert!(evaluate_condition(&cond, &ctx));
    }

    #[test]
    fn test_evaluate_condition_contains_string() {
        let ctx = json!({ "agent": { "name": "Alice the Builder" } });
        let cond = RuleCondition {
            field: "agent.name".to_string(),
            operator: "contains".to_string(),
            value: json!("Builder"),
        };
        assert!(evaluate_condition(&cond, &ctx));
    }

    #[test]
    fn test_evaluate_condition_contains_array() {
        let ctx = json!({ "agent": { "tags": ["builder", "trader"] } });
        let cond = RuleCondition {
            field: "agent.tags".to_string(),
            operator: "contains".to_string(),
            value: json!("trader"),
        };
        assert!(evaluate_condition(&cond, &ctx));
    }

    #[test]
    fn test_evaluate_condition_missing_field() {
        let ctx = json!({ "agent": { "tokens": 100 } });
        let cond = RuleCondition {
            field: "agent.hunger".to_string(),
            operator: ">".to_string(),
            value: json!(50),
        };
        assert!(!evaluate_condition(&cond, &ctx));
    }

    // ── Rule Proposal & Voting ─────────────────────────────

    #[test]
    fn test_propose_and_vote_rule() {
        let mut engine = RuleEngine::new();
        let rule_id = engine.propose_rule(
            "agent-1".to_string(),
            "org-1".to_string(),
            "Tax on outsiders".to_string(),
            "Charge 10% extra tax".to_string(),
            RuleType::Tax,
            vec![RuleCondition {
                field: "agent.is_outsider".to_string(),
                operator: "==".to_string(),
                value: json!(true),
            }],
            vec![RuleEffect {
                target: "agent.tax_bonus".to_string(),
                action: "set".to_string(),
                value: json!(0.1),
            }],
            100,
            None,
        );

        assert!(!rule_id.is_empty());
        let rule = engine.get_rule(&rule_id).unwrap();
        assert_eq!(rule.status, RuleStatus::Proposed);
        assert_eq!(rule.votes_for, 0);

        // Vote
        engine.vote_on_rule(&rule_id, "agent-2".to_string(), true).unwrap();
        engine.vote_on_rule(&rule_id, "agent-3".to_string(), false).unwrap();

        let rule = engine.get_rule(&rule_id).unwrap();
        assert_eq!(rule.votes_for, 1);
        assert_eq!(rule.votes_against, 1);

        // Duplicate vote fails
        let result = engine.vote_on_rule(&rule_id, "agent-2".to_string(), true);
        assert!(result.is_err());
    }

    #[test]
    fn test_activate_rule() {
        let mut engine = RuleEngine::new();
        let rule_id = engine.propose_rule(
            "agent-1".to_string(),
            "org-1".to_string(),
            "Test rule".to_string(),
            "Desc".to_string(),
            RuleType::Behavior,
            vec![],
            vec![],
            10,
            None,
        );

        engine.activate_rule(&rule_id).unwrap();
        let rule = engine.get_rule(&rule_id).unwrap();
        assert_eq!(rule.status, RuleStatus::Active);

        // Double activate fails
        let result = engine.activate_rule(&rule_id);
        assert!(result.is_err());
    }

    // ── Rule Evaluation ────────────────────────────────────

    #[test]
    fn test_rule_evaluation_matching() {
        let mut engine = RuleEngine::new();
        let rule_id = engine.propose_rule(
            "agent-1".to_string(),
            "org-1".to_string(),
            "Low token tax".to_string(),
            "Add tax when tokens < 50".to_string(),
            RuleType::Tax,
            vec![RuleCondition {
                field: "agent.tokens".to_string(),
                operator: "<".to_string(),
                value: json!(50),
            }],
            vec![RuleEffect {
                target: "agent.tokens".to_string(),
                action: "subtract".to_string(),
                value: json!(5),
            }],
            100,
            None,
        );
        engine.activate_rule(&rule_id).unwrap();

        // Context where agent has low tokens — should match
        let ctx = json!({ "agent": { "tokens": 30 } });
        let effects = engine.evaluate_rules(&ctx);
        assert_eq!(effects.len(), 1);
        assert_eq!(effects[0].action, "subtract");
    }

    #[test]
    fn test_rule_evaluation_not_matching() {
        let mut engine = RuleEngine::new();
        let rule_id = engine.propose_rule(
            "agent-1".to_string(),
            "org-1".to_string(),
            "Low token tax".to_string(),
            "Add tax when tokens < 50".to_string(),
            RuleType::Tax,
            vec![RuleCondition {
                field: "agent.tokens".to_string(),
                operator: "<".to_string(),
                value: json!(50),
            }],
            vec![RuleEffect {
                target: "agent.tokens".to_string(),
                action: "subtract".to_string(),
                value: json!(5),
            }],
            100,
            None,
        );
        engine.activate_rule(&rule_id).unwrap();

        // Context where agent has plenty of tokens — should NOT match
        let ctx = json!({ "agent": { "tokens": 200 } });
        let effects = engine.evaluate_rules(&ctx);
        assert!(effects.is_empty());
    }

    // ── Expiration ─────────────────────────────────────────

    #[test]
    fn test_rule_expiration() {
        let mut engine = RuleEngine::new();
        let rule_id = engine.propose_rule(
            "agent-1".to_string(),
            "org-1".to_string(),
            "Temporary rule".to_string(),
            "Expires at tick 500".to_string(),
            RuleType::Behavior,
            vec![],
            vec![],
            100,
            Some(500),
        );
        engine.activate_rule(&rule_id).unwrap();

        // Before expiration
        let expired = engine.expire_rules(499);
        assert!(expired.is_empty());
        assert_eq!(engine.get_rule(&rule_id).unwrap().status, RuleStatus::Active);

        // At expiration tick
        let expired = engine.expire_rules(500);
        assert_eq!(expired.len(), 1);
        assert_eq!(expired[0], rule_id);
        assert_eq!(engine.get_rule(&rule_id).unwrap().status, RuleStatus::Repealed);
    }

    // ── Repeal ─────────────────────────────────────────────

    #[test]
    fn test_rule_repeal() {
        let mut engine = RuleEngine::new();
        let rule_id = engine.propose_rule(
            "agent-1".to_string(),
            "org-1".to_string(),
            "Bad rule".to_string(),
            "Repeal this".to_string(),
            RuleType::Tax,
            vec![],
            vec![],
            10,
            None,
        );
        engine.activate_rule(&rule_id).unwrap();

        engine.repeal_rule(&rule_id, 200).unwrap();
        assert_eq!(engine.get_rule(&rule_id).unwrap().status, RuleStatus::Repealed);

        // Repealed rule should not evaluate
        let ctx = json!({});
        let effects = engine.evaluate_rules(&ctx);
        assert!(effects.is_empty());
    }

    // ── Apply Effects ──────────────────────────────────────

    #[test]
    fn test_apply_effect_set() {
        let mut ctx = json!({ "agent": { "tokens": 100 } });
        let effect = RuleEffect {
            target: "agent.tokens".to_string(),
            action: "set".to_string(),
            value: json!(50),
        };
        assert!(apply_effect(&effect, &mut ctx));
        assert_eq!(ctx["agent"]["tokens"], json!(50));
    }

    #[test]
    fn test_apply_effect_add() {
        let mut ctx = json!({ "agent": { "tokens": 100 } });
        let effect = RuleEffect {
            target: "agent.tokens".to_string(),
            action: "add".to_string(),
            value: json!(25),
        };
        assert!(apply_effect(&effect, &mut ctx));
        assert_eq!(ctx["agent"]["tokens"], json!(125));
    }

    #[test]
    fn test_apply_effect_subtract() {
        let mut ctx = json!({ "agent": { "tokens": 100 } });
        let effect = RuleEffect {
            target: "agent.tokens".to_string(),
            action: "subtract".to_string(),
            value: json!(30),
        };
        assert!(apply_effect(&effect, &mut ctx));
        assert_eq!(ctx["agent"]["tokens"], json!(70));
    }

    #[test]
    fn test_apply_effect_multiply() {
        let mut ctx = json!({ "agent": { "tokens": 100 } });
        let effect = RuleEffect {
            target: "agent.tokens".to_string(),
            action: "multiply".to_string(),
            value: json!(1.5),
        };
        assert!(apply_effect(&effect, &mut ctx));
        assert_eq!(ctx["agent"]["tokens"], json!(150.0));
    }

    // ── Multiple Rules ─────────────────────────────────────

    #[test]
    fn test_multiple_active_rules() {
        let mut engine = RuleEngine::new();

        let r1 = engine.propose_rule(
            "a1".to_string(),
            "org-1".to_string(),
            "Rule 1".to_string(),
            "Desc 1".to_string(),
            RuleType::Tax,
            vec![RuleCondition {
                field: "agent.tokens".to_string(),
                operator: ">".to_string(),
                value: json!(0),
            }],
            vec![RuleEffect {
                target: "agent.tax".to_string(),
                action: "set".to_string(),
                value: json!("rule1_applied"),
            }],
            1,
            None,
        );
        engine.activate_rule(&r1).unwrap();

        let r2 = engine.propose_rule(
            "a2".to_string(),
            "org-1".to_string(),
            "Rule 2".to_string(),
            "Desc 2".to_string(),
            RuleType::Behavior,
            vec![RuleCondition {
                field: "agent.tokens".to_string(),
                operator: ">".to_string(),
                value: json!(0),
            }],
            vec![RuleEffect {
                target: "agent.behavior".to_string(),
                action: "set".to_string(),
                value: json!("rule2_applied"),
            }],
            1,
            None,
        );
        engine.activate_rule(&r2).unwrap();

        let ctx = json!({ "agent": { "tokens": 100 } });
        let effects = engine.evaluate_rules(&ctx);
        assert_eq!(effects.len(), 2);
    }

    // ── Org-scoped Evaluation ──────────────────────────────

    #[test]
    fn test_evaluate_rules_for_org() {
        let mut engine = RuleEngine::new();
        let r1 = engine.propose_rule(
            "a1".to_string(),
            "org-1".to_string(),
            "Org 1 rule".to_string(),
            "Desc".to_string(),
            RuleType::Tax,
            vec![RuleCondition {
                field: "agent.tokens".to_string(),
                operator: ">".to_string(),
                value: json!(0),
            }],
            vec![RuleEffect {
                target: "agent.bonus".to_string(),
                action: "set".to_string(),
                value: json!("org1_bonus"),
            }],
            1,
            None,
        );
        engine.activate_rule(&r1).unwrap();

        let r2 = engine.propose_rule(
            "a2".to_string(),
            "org-2".to_string(),
            "Org 2 rule".to_string(),
            "Desc".to_string(),
            RuleType::Tax,
            vec![RuleCondition {
                field: "agent.tokens".to_string(),
                operator: ">".to_string(),
                value: json!(0),
            }],
            vec![RuleEffect {
                target: "agent.bonus".to_string(),
                action: "set".to_string(),
                value: json!("org2_bonus"),
            }],
            1,
            None,
        );
        engine.activate_rule(&r2).unwrap();

        let ctx = json!({ "agent": { "tokens": 100 } });
        let effects = engine.evaluate_rules_for_org("org-1", &ctx);
        assert_eq!(effects.len(), 1);
        assert_eq!(effects[0].value, json!("org1_bonus"));
    }

    // ── Suspend / Resume ───────────────────────────────────

    #[test]
    fn test_suspend_and_resume() {
        let mut engine = RuleEngine::new();
        let rule_id = engine.propose_rule(
            "a1".to_string(),
            "org-1".to_string(),
            "Test".to_string(),
            "Desc".to_string(),
            RuleType::Tax,
            vec![RuleCondition {
                field: "agent.tokens".to_string(),
                operator: ">".to_string(),
                value: json!(0),
            }],
            vec![RuleEffect {
                target: "agent.x".to_string(),
                action: "set".to_string(),
                value: json!(1),
            }],
            1,
            None,
        );
        engine.activate_rule(&rule_id).unwrap();

        // Suspend
        engine.suspend_rule(&rule_id).unwrap();
        let ctx = json!({ "agent": { "tokens": 100 } });
        assert!(engine.evaluate_rules(&ctx).is_empty());

        // Resume
        engine.resume_rule(&rule_id).unwrap();
        assert_eq!(engine.evaluate_rules(&ctx).len(), 1);
    }
}
