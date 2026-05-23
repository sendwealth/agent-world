//! DSL parser — parse, validate, and convert Agent-proposed rules.
//!
//! # DSL Syntax
//!
//! ```yaml
//! rule:
//!   id: R-CUSTOM-001
//!   name: Trade Tax
//!   scope: global
//!   priority: 30
//!   category: tax
//!   trigger:
//!     event: on_trade
//!   conditions:
//!     - field: trade.amount
//!       operator: ">"
//!       value: 100
//!   actions:
//!     - type: tax
//!       params:
//!         rate: 0.05
//!         target: seller
//!   ttl_ticks: 1000
//!   cooldown_ticks: 10
//! ```

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::organization::rule_engine::{
    RuleCondition, RuleEffect, RuleType,
};

// ── Constants ──────────────────────────────────────────────

/// Built-in rule IDs that DSL rules may not override.
const RESERVED_IDS: &[&str] = &[
    "R001", "R002", "R003", "R004", "R005", "R006", "R007", "R008", "R009",
    "R010", "R011", "R012", "R013", "R014", "R015", "R016", "R017", "R018", "R019",
    "R020", "R021", "R022", "R023", "R024", "R025", "R026", "R027", "R028", "R029",
    "R030", "R031",
];

const VALID_TRIGGER_EVENTS: &[&str] = &[
    "on_tick",
    "on_trade",
    "on_attack",
    "on_agent_spawn",
    "on_agent_death",
    "on_transfer",
    "on_org_created",
    "on_org_join",
    "on_org_leave",
    "on_proposal_created",
    "on_vote_cast",
    "on_gather",
    "on_communicate",
    "on_loan",
    "on_stock_trade",
    "on_message",
    "on_contract",
    "on_reproduce",
    "on_death",
    "on_join_org",
    "on_leave_org",
    "on_resource_change",
];

const VALID_OPERATORS: &[&str] = &[
    "==", "!=", ">", "<", ">=", "<=",
    "contains", "not_contains", "in", "not_in",
];

const VALID_ACTION_TYPES: &[&str] = &[
    "tax", "penalty", "reward", "block",
    "modify_resource", "set_field", "send_message", "log_event",
    "transfer", "notify", "restrict", "set", "custom",
];

const VALID_SCOPES: &[&str] = &[
    "global", "organization", "region", "agent",
];

const VALID_CATEGORIES: &[&str] = &[
    "tax", "behavior", "trade", "diplomacy", "custom",
];

// ── DSL Types ──────────────────────────────────────────────

/// Scope of a DSL rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleScope {
    Global,
    Organization,
    Region,
    Agent,
}

impl std::fmt::Display for RuleScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RuleScope::Global => write!(f, "global"),
            RuleScope::Organization => write!(f, "organization"),
            RuleScope::Region => write!(f, "region"),
            RuleScope::Agent => write!(f, "agent"),
        }
    }
}

/// Trigger event type for a DSL rule.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct TriggerConfig {
    pub event: String,
}

/// A single condition in a DSL rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DslCondition {
    pub field: String,
    pub operator: String,
    pub value: Value,
}

/// A single action in a DSL rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DslAction {
    #[serde(rename = "type")]
    pub action_type: String,
    #[serde(default)]
    pub params: Value,
}

/// The top-level rule definition parsed from YAML/JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DslRule {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default = "default_scope")]
    pub scope: RuleScope,
    #[serde(default = "default_priority")]
    pub priority: u32,
    #[serde(default = "default_category")]
    pub category: String,
    pub trigger: TriggerConfig,
    #[serde(default)]
    pub conditions: Vec<DslCondition>,
    #[serde(default)]
    pub actions: Vec<DslAction>,
    #[serde(default)]
    pub org_id: Option<String>,
    #[serde(default)]
    pub ttl_ticks: Option<u64>,
    #[serde(default)]
    pub cooldown_ticks: Option<u64>,
}

fn default_scope() -> RuleScope { RuleScope::Global }
fn default_priority() -> u32 { 50 }
fn default_category() -> String { "custom".to_string() }

/// Wrapper for YAML input that may have a top-level `rule:` key.
#[derive(Debug, Deserialize)]
struct RuleWrapper {
    rule: DslRule,
}

/// Wrapper for multiple rules.
#[derive(Debug, Deserialize)]
struct RulesWrapper {
    rules: Vec<DslRule>,
}

// ── Parse Result ───────────────────────────────────────────

/// Result of parsing a DSL rule document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseResult {
    /// Whether the rule is syntactically and semantically valid.
    pub valid: bool,
    /// The parsed rule (present only when valid).
    pub rule: Option<DslRule>,
    /// Validation errors (blocking).
    pub errors: Vec<String>,
    /// Non-fatal warnings.
    pub warnings: Vec<String>,
}

impl ParseResult {
    fn ok(rule: DslRule, warnings: Vec<String>) -> Self {
        Self {
            valid: true,
            rule: Some(rule),
            errors: Vec::new(),
            warnings,
        }
    }

    fn err(errors: Vec<String>) -> Self {
        Self {
            valid: false,
            rule: None,
            errors,
            warnings: Vec::new(),
        }
    }
}

// ── Parsing ────────────────────────────────────────────────

/// Parse a single rule from a YAML string.
pub fn parse_yaml(input: &str) -> ParseResult {
    // Try wrapped format (`rule: ...`) first, then bare format.
    let rule = if let Ok(wrapped) = serde_yaml::from_str::<RuleWrapper>(input) {
        wrapped.rule
    } else if let Ok(bare) = serde_yaml::from_str::<DslRule>(input) {
        bare
    } else {
        return ParseResult::err(vec!["Invalid YAML: could not parse rule".to_string()]);
    };

    validate_and_build(rule)
}

/// Parse multiple rules from a YAML string.
pub fn parse_yaml_multi(input: &str) -> Vec<ParseResult> {
    if let Ok(wrapped) = serde_yaml::from_str::<RulesWrapper>(input) {
        wrapped.rules.into_iter().map(validate_and_build).collect()
    } else {
        vec![parse_yaml(input)]
    }
}

/// Parse a single rule from a JSON string.
pub fn parse_json(input: &str) -> ParseResult {
    // Try wrapped format first.
    let rule = if let Ok(wrapped) = serde_json::from_str::<RuleWrapper>(input) {
        wrapped.rule
    } else if let Ok(bare) = serde_json::from_str::<DslRule>(input) {
        bare
    } else {
        return ParseResult::err(vec!["Invalid JSON: could not parse rule".to_string()]);
    };

    validate_and_build(rule)
}

// ── Validation ─────────────────────────────────────────────

/// Validate a parsed rule and build a `ParseResult`.
fn validate_and_build(rule: DslRule) -> ParseResult {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    // Required fields
    if rule.id.trim().is_empty() {
        errors.push("rule.id is required".to_string());
    }
    if rule.name.trim().is_empty() {
        errors.push("rule.name is required".to_string());
    }
    if rule.trigger.event.trim().is_empty() {
        errors.push("rule.trigger.event is required".to_string());
    }

    // Reserved ID check
    if RESERVED_IDS.contains(&rule.id.as_str()) {
        errors.push(format!(
            "rule.id '{}' is reserved for built-in rules (R001-R031)",
            rule.id
        ));
    }

    // Trigger event whitelist
    if !rule.trigger.event.is_empty() && !VALID_TRIGGER_EVENTS.contains(&rule.trigger.event.as_str()) {
        errors.push(format!(
            "Invalid trigger event '{}'. Valid: {}",
            rule.trigger.event,
            VALID_TRIGGER_EVENTS.join(", ")
        ));
    }

    // Scope whitelist
    let scope_str = rule.scope.to_string();
    if !VALID_SCOPES.contains(&scope_str.as_str()) {
        errors.push(format!(
            "Invalid scope '{}'. Valid: {}",
            scope_str,
            VALID_SCOPES.join(", ")
        ));
    }

    // Category whitelist
    if !rule.category.is_empty() && !VALID_CATEGORIES.contains(&rule.category.as_str()) {
        errors.push(format!(
            "Invalid category '{}'. Valid: {}",
            rule.category,
            VALID_CATEGORIES.join(", ")
        ));
    }

    // Organization scope requires org_id
    if rule.scope == RuleScope::Organization && rule.org_id.is_none() {
        errors.push("Organization-scoped rules must specify org_id".to_string());
    }

    // Validate conditions
    for (i, cond) in rule.conditions.iter().enumerate() {
        if cond.field.trim().is_empty() {
            errors.push(format!("conditions[{}].field is empty", i));
        }
        if !VALID_OPERATORS.contains(&cond.operator.as_str()) {
            errors.push(format!(
                "conditions[{}] has invalid operator '{}'. Valid: {}",
                i, cond.operator, VALID_OPERATORS.join(", ")
            ));
        }
        // `in` / `not_in` operators require array value
        if (cond.operator == "in" || cond.operator == "not_in") && !cond.value.is_array() {
            errors.push(format!(
                "conditions[{}] uses '{}' operator but value is not an array",
                i, cond.operator
            ));
        }
    }

    // Validate actions
    for (i, action) in rule.actions.iter().enumerate() {
        if !VALID_ACTION_TYPES.contains(&action.action_type.as_str()) {
            errors.push(format!(
                "actions[{}] has invalid type '{}'. Valid: {}",
                i, action.action_type, VALID_ACTION_TYPES.join(", ")
            ));
        }
        // Per-action parameter validation
        validate_action_params(i, action, &mut errors, &mut warnings);
    }

    if rule.actions.is_empty() {
        warnings.push("Rule has no actions — it will have no effect when triggered".to_string());
    }

    if !errors.is_empty() {
        ParseResult {
            valid: false,
            rule: Some(rule),
            errors,
            warnings,
        }
    } else {
        ParseResult::ok(rule, warnings)
    }
}

/// Validate parameters for specific action types.
fn validate_action_params(
    index: usize,
    action: &DslAction,
    errors: &mut Vec<String>,
    warnings: &mut Vec<String>,
) {
    let params = &action.params;

    match action.action_type.as_str() {
        "tax" => {
            if let Some(rate) = params.get("rate").and_then(|v| v.as_f64()) {
                if rate < 0.0 || rate > 1.0 {
                    errors.push(format!(
                        "actions[{}]: tax.rate must be between 0.0 and 1.0, got {}",
                        index, rate
                    ));
                }
            } else if params.get("rate").is_none() {
                warnings.push(format!("actions[{}]: tax rule has no 'rate' parameter", index));
            }
            if params.get("target").is_none() {
                warnings.push(format!("actions[{}]: tax rule has no 'target' parameter", index));
            }
        }
        "penalty" | "reward" => {
            if params.get("amount").is_none() {
                warnings.push(format!(
                    "actions[{}]: {} has no 'amount' parameter",
                    index, action.action_type
                ));
            }
        }
        "transfer" => {
            if params.get("amount").is_none() {
                warnings.push(format!("actions[{}]: transfer has no 'amount' parameter", index));
            }
            if params.get("from").is_none() {
                warnings.push(format!("actions[{}]: transfer has no 'from' parameter", index));
            }
            if params.get("to").is_none() {
                warnings.push(format!("actions[{}]: transfer has no 'to' parameter", index));
            }
        }
        _ => {}
    }
}

// ── Conversion to SoftRule types ───────────────────────────

/// Convert DSL conditions to SoftRule `RuleCondition`s.
pub fn to_rule_conditions(dsl_conditions: &[DslCondition]) -> Vec<RuleCondition> {
    dsl_conditions
        .iter()
        .map(|c| RuleCondition {
            field: c.field.clone(),
            operator: c.operator.clone(),
            value: c.value.clone(),
        })
        .collect()
}

/// Convert DSL actions to SoftRule `RuleEffect`s.
pub fn to_rule_effects(dsl_actions: &[DslAction]) -> Vec<RuleEffect> {
    dsl_actions
        .iter()
        .map(|a| {
            let target = a.params.get("target")
                .and_then(|v| v.as_str())
                .unwrap_or("agent")
                .to_string();
            let action = match a.action_type.as_str() {
                "tax" | "penalty" => "subtract".to_string(),
                "reward" => "add".to_string(),
                "modify_resource" | "set_field" => "set".to_string(),
                "block" => "block_action".to_string(),
                _ => a.action_type.clone(),
            };
            let value = a.params.get("value").cloned()
                .or_else(|| a.params.get("amount").cloned())
                .or_else(|| a.params.get("rate").cloned())
                .unwrap_or(Value::Null);

            RuleEffect {
                target,
                action,
                value,
            }
        })
        .collect()
}

/// Map DSL category to SoftRule `RuleType`.
pub fn to_rule_type(category: &str) -> RuleType {
    match category {
        "tax" => RuleType::Tax,
        "behavior" => RuleType::Behavior,
        "trade" => RuleType::Trade,
        "diplomacy" => RuleType::Diplomacy,
        _ => RuleType::Custom,
    }
}

// ── Built-in Templates ─────────────────────────────────────

/// A rule template with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleTemplate {
    pub name: String,
    pub description: String,
    pub category: String,
    pub yaml: String,
}

/// Get all built-in rule templates.
pub fn builtin_templates() -> Vec<RuleTemplate> {
    vec![
        RuleTemplate {
            name: "trade_tax".to_string(),
            description: "Trade tax: apply a percentage tax on large trades".to_string(),
            category: "tax".to_string(),
            yaml: r#"rule:
  id: R-TRADE-TAX
  name: Trade Tax
  scope: global
  priority: 30
  category: tax
  trigger:
    event: on_trade
  conditions:
    - field: trade.amount
      operator: ">"
      value: 100
  actions:
    - type: tax
      params:
        rate: 0.05
        target: seller
"#.to_string(),
        },
        RuleTemplate {
            name: "warfare_law".to_string(),
            description: "Warfare law: protect young agents from attack".to_string(),
            category: "behavior".to_string(),
            yaml: r#"rule:
  id: R-WARFARE-LAW
  name: Warfare Law
  scope: global
  priority: 40
  category: behavior
  trigger:
    event: on_attack
  conditions:
    - field: target.age_ticks
      operator: "<"
      value: 100
  actions:
    - type: block
      params: {}
"#.to_string(),
        },
        RuleTemplate {
            name: "resource_protection".to_string(),
            description: "Resource protection: limit per-tick resource gathering".to_string(),
            category: "behavior".to_string(),
            yaml: r#"rule:
  id: R-RESOURCE-PROTECT
  name: Resource Protection
  scope: global
  priority: 35
  category: behavior
  trigger:
    event: on_gather
  conditions:
    - field: agent.resources_gathered_today
      operator: ">"
      value: 50
  actions:
    - type: penalty
      params:
        amount: 10
"#.to_string(),
        },
        RuleTemplate {
            name: "newbie_protection".to_string(),
            description: "Newbie protection: grant periodic bonus tokens to new agents".to_string(),
            category: "behavior".to_string(),
            yaml: r#"rule:
  id: R-NEWBIE-PROTECT
  name: Newbie Protection
  scope: global
  priority: 50
  category: behavior
  trigger:
    event: on_tick
  conditions:
    - field: agent.age_ticks
      operator: "<"
      value: 200
  actions:
    - type: reward
      params:
        amount: 5
"#.to_string(),
        },
        RuleTemplate {
            name: "anti_monopoly".to_string(),
            description: "Anti-monopoly: tax agents with >50% token share".to_string(),
            category: "tax".to_string(),
            yaml: r#"rule:
  id: R-ANTI-MONOPOLY
  name: Anti Monopoly
  scope: global
  priority: 30
  category: tax
  trigger:
    event: on_tick
  conditions:
    - field: agent.token_share
      operator: ">"
      value: 0.5
  actions:
    - type: tax
      params:
        rate: 0.1
        target: agent
"#.to_string(),
        },
        RuleTemplate {
            name: "diplomatic_sanction".to_string(),
            description: "Diplomatic sanction: block interactions with sanctioned organizations".to_string(),
            category: "diplomacy".to_string(),
            yaml: r#"rule:
  id: R-DIPLO-SANCTION
  name: Diplomatic Sanction
  scope: organization
  org_id: your-org-id
  priority: 40
  category: diplomacy
  trigger:
    event: on_trade
  conditions:
    - field: counterparty.org_id
      operator: "in"
      value: ["sanctioned-org-1", "sanctioned-org-2"]
  actions:
    - type: block
      params: {}
"#.to_string(),
        },
        RuleTemplate {
            name: "communication_filter".to_string(),
            description: "Communication filter: block spam messages".to_string(),
            category: "behavior".to_string(),
            yaml: r#"rule:
  id: R-COMM-FILTER
  name: Communication Filter
  scope: global
  priority: 45
  category: behavior
  trigger:
    event: on_communicate
  conditions:
    - field: message.length
      operator: ">"
      value: 500
  actions:
    - type: block
      params: {}
"#.to_string(),
        },
    ]
}

/// Get a template by name.
pub fn get_template(name: &str) -> Option<RuleTemplate> {
    builtin_templates().into_iter().find(|t| t.name == name)
}

// ── Serialization helpers ──────────────────────────────────

/// Serialize a DslRule to YAML.
pub fn to_yaml(rule: &DslRule) -> Result<String, String> {
    serde_yaml::to_string(rule).map_err(|e| format!("YAML serialization error: {}", e))
}

/// Serialize a DslRule to JSON.
pub fn to_json(rule: &DslRule) -> Result<String, String> {
    serde_json::to_string_pretty(rule).map_err(|e| format!("JSON serialization error: {}", e))
}

// ── Tests ──────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Parsing Tests ──────────────────────────────────────

    #[test]
    fn test_parse_yaml_wrapped() {
        let yaml = r#"
rule:
  id: R-CUSTOM-001
  name: Test Rule
  scope: global
  trigger:
    event: on_trade
  conditions:
    - field: trade.amount
      operator: ">"
      value: 100
  actions:
    - type: tax
      params:
        rate: 0.05
        target: seller
"#;
        let result = parse_yaml(yaml);
        assert!(result.valid, "Expected valid, got errors: {:?}", result.errors);
        let rule = result.rule.unwrap();
        assert_eq!(rule.id, "R-CUSTOM-001");
        assert_eq!(rule.name, "Test Rule");
        assert_eq!(rule.trigger.event, "on_trade");
        assert_eq!(rule.conditions.len(), 1);
        assert_eq!(rule.actions.len(), 1);
    }

    #[test]
    fn test_parse_yaml_bare() {
        let yaml = r#"
id: R-CUSTOM-002
name: Bare Rule
trigger:
  event: on_tick
actions:
  - type: log_event
    params: {}
"#;
        let result = parse_yaml(yaml);
        assert!(result.valid, "Expected valid, got errors: {:?}", result.errors);
        let rule = result.rule.unwrap();
        assert_eq!(rule.id, "R-CUSTOM-002");
    }

    #[test]
    fn test_parse_json_wrapped() {
        let json = r#"{"rule": {
            "id": "R-CUSTOM-003",
            "name": "JSON Rule",
            "trigger": {"event": "on_attack"},
            "conditions": [{"field": "agent.tokens", "operator": "<", "value": 10}],
            "actions": [{"type": "penalty", "params": {"amount": 5}}]
        }}"#;
        let result = parse_json(json);
        assert!(result.valid, "Expected valid, got errors: {:?}", result.errors);
        let rule = result.rule.unwrap();
        assert_eq!(rule.id, "R-CUSTOM-003");
        assert_eq!(rule.trigger.event, "on_attack");
    }

    #[test]
    fn test_parse_json_bare() {
        let json = r#"{
            "id": "R-CUSTOM-004",
            "name": "Bare JSON",
            "trigger": {"event": "on_tick"},
            "actions": [{"type": "log_event", "params": {}}]
        }"#;
        let result = parse_json(json);
        assert!(result.valid, "Expected valid, got errors: {:?}", result.errors);
    }

    #[test]
    fn test_parse_invalid_yaml() {
        let result = parse_yaml("not valid yaml @@@");
        assert!(!result.valid);
    }

    #[test]
    fn test_parse_invalid_json() {
        let result = parse_json("not valid json");
        assert!(!result.valid);
    }

    #[test]
    fn test_parse_multiple_rules() {
        let yaml = r#"
rules:
  - id: R-MULTI-001
    name: First
    trigger:
      event: on_tick
    actions:
      - type: log_event
        params: {}
  - id: R-MULTI-002
    name: Second
    trigger:
      event: on_trade
    actions:
      - type: log_event
        params: {}
"#;
        let results = parse_yaml_multi(yaml);
        assert_eq!(results.len(), 2);
        assert!(results[0].valid);
        assert!(results[1].valid);
    }

    // ── Validation Tests ───────────────────────────────────

    #[test]
    fn test_validation_missing_id() {
        let yaml = r#"
rule:
  name: No ID
  trigger:
    event: on_tick
  actions:
    - type: log_event
      params: {}
"#;
        let result = parse_yaml(yaml);
        assert!(!result.valid);
        assert!(result.errors.iter().any(|e| e.contains("rule.id is required")));
    }

    #[test]
    fn test_validation_missing_name() {
        let yaml = r#"
rule:
  id: R-TEST
  trigger:
    event: on_tick
  actions:
    - type: log_event
      params: {}
"#;
        let result = parse_yaml(yaml);
        assert!(!result.valid);
        assert!(result.errors.iter().any(|e| e.contains("rule.name is required")));
    }

    #[test]
    fn test_validation_reserved_id() {
        let yaml = r#"
rule:
  id: R011
  name: Override
  trigger:
    event: on_tick
  actions:
    - type: log_event
      params: {}
"#;
        let result = parse_yaml(yaml);
        assert!(!result.valid);
        assert!(result.errors.iter().any(|e| e.contains("reserved")));
    }

    #[test]
    fn test_validation_invalid_trigger() {
        let yaml = r#"
rule:
  id: R-CUSTOM-BAD
  name: Bad Trigger
  trigger:
    event: on_nonexistent
  actions:
    - type: log_event
      params: {}
"#;
        let result = parse_yaml(yaml);
        assert!(!result.valid);
        assert!(result.errors.iter().any(|e| e.contains("Invalid trigger event")));
    }

    #[test]
    fn test_validation_invalid_operator() {
        let yaml = r#"
rule:
  id: R-CUSTOM-BAD
  name: Bad Operator
  trigger:
    event: on_tick
  conditions:
    - field: agent.tokens
      operator: "invalid_op"
      value: 10
  actions:
    - type: log_event
      params: {}
"#;
        let result = parse_yaml(yaml);
        assert!(!result.valid);
        assert!(result.errors.iter().any(|e| e.contains("invalid operator")));
    }

    #[test]
    fn test_validation_invalid_action_type() {
        let yaml = r#"
rule:
  id: R-CUSTOM-BAD
  name: Bad Action
  trigger:
    event: on_tick
  actions:
    - type: nonexistent_action
      params: {}
"#;
        let result = parse_yaml(yaml);
        assert!(!result.valid);
        assert!(result.errors.iter().any(|e| e.contains("invalid type")));
    }

    #[test]
    fn test_validation_org_scope_no_org_id() {
        let yaml = r#"
rule:
  id: R-CUSTOM-ORG
  name: Org Rule
  scope: organization
  trigger:
    event: on_tick
  actions:
    - type: log_event
      params: {}
"#;
        let result = parse_yaml(yaml);
        assert!(!result.valid);
        assert!(result.errors.iter().any(|e| e.contains("org_id")));
    }

    #[test]
    fn test_validation_tax_rate_out_of_range() {
        let yaml = r#"
rule:
  id: R-CUSTOM-TAX
  name: Bad Tax
  trigger:
    event: on_trade
  actions:
    - type: tax
      params:
        rate: 1.5
"#;
        let result = parse_yaml(yaml);
        assert!(!result.valid);
        assert!(result.errors.iter().any(|e| e.contains("tax.rate must be between 0.0 and 1.0")));
    }

    #[test]
    fn test_validation_in_operator_not_array() {
        let yaml = r#"
rule:
  id: R-CUSTOM-IN
  name: In Test
  trigger:
    event: on_trade
  conditions:
    - field: agent.org
      operator: in
      value: "not-an-array"
  actions:
    - type: log_event
      params: {}
"#;
        let result = parse_yaml(yaml);
        assert!(!result.valid);
        assert!(result.errors.iter().any(|e| e.contains("not an array")));
    }

    #[test]
    fn test_warning_no_actions() {
        let yaml = r#"
rule:
  id: R-CUSTOM-NOACT
  name: No Actions
  trigger:
    event: on_tick
"#;
        let result = parse_yaml(yaml);
        assert!(result.valid);
        assert!(result.warnings.iter().any(|w| w.contains("no actions")));
    }

    // ── All trigger events parse ───────────────────────────

    #[test]
    fn test_all_trigger_events_parse() {
        for event in VALID_TRIGGER_EVENTS {
            let yaml = format!(r#"
rule:
  id: R-TRIGGER-TEST
  name: Trigger Test
  trigger:
    event: {}
  actions:
    - type: log_event
      params: {{}}
"#, event);
            let result = parse_yaml(&yaml);
            assert!(result.valid, "Trigger '{}' should be valid, errors: {:?}", event, result.errors);
        }
    }

    // ── All action types parse ─────────────────────────────

    #[test]
    fn test_all_action_types_parse() {
        for action_type in VALID_ACTION_TYPES {
            let yaml = format!(r#"
rule:
  id: R-ACTION-TEST
  name: Action Test
  trigger:
    event: on_tick
  actions:
    - type: {}
      params: {{}}
"#, action_type);
            let result = parse_yaml(&yaml);
            assert!(result.valid, "Action '{}' should be valid, errors: {:?}", action_type, result.errors);
        }
    }

    // ── Templates ──────────────────────────────────────────

    #[test]
    fn test_all_builtin_templates_validate() {
        for template in builtin_templates() {
            let result = parse_yaml(&template.yaml);
            assert!(
                result.valid,
                "Template '{}' should validate, errors: {:?}",
                template.name,
                result.errors
            );
        }
    }

    #[test]
    fn test_get_template() {
        assert!(get_template("trade_tax").is_some());
        assert!(get_template("nonexistent").is_none());
    }

    // ── Conversion to SoftRule types ───────────────────────

    #[test]
    fn test_to_rule_conditions() {
        let yaml = r#"
rule:
  id: R-CONV-001
  name: Conversion Test
  trigger:
    event: on_trade
  conditions:
    - field: trade.amount
      operator: ">"
      value: 100
    - field: agent.tokens
      operator: "<"
      value: 50
  actions:
    - type: tax
      params:
        rate: 0.05
        target: seller
"#;
        let result = parse_yaml(yaml);
        assert!(result.valid);
        let rule = result.rule.unwrap();
        let conditions = to_rule_conditions(&rule.conditions);
        assert_eq!(conditions.len(), 2);
        assert_eq!(conditions[0].field, "trade.amount");
        assert_eq!(conditions[0].operator, ">");
        assert_eq!(conditions[1].field, "agent.tokens");
    }

    #[test]
    fn test_to_rule_effects() {
        let yaml = r#"
rule:
  id: R-EFFECT-001
  name: Effect Test
  trigger:
    event: on_trade
  actions:
    - type: tax
      params:
        rate: 0.05
        target: seller
    - type: penalty
      params:
        amount: 10
        target: offender
"#;
        let result = parse_yaml(yaml);
        assert!(result.valid);
        let rule = result.rule.unwrap();
        let effects = to_rule_effects(&rule.actions);
        assert_eq!(effects.len(), 2);
        // tax → subtract
        assert_eq!(effects[0].action, "subtract");
        assert_eq!(effects[0].target, "seller");
        // penalty → subtract
        assert_eq!(effects[1].action, "subtract");
        assert_eq!(effects[1].target, "offender");
    }

    #[test]
    fn test_to_rule_type() {
        assert_eq!(to_rule_type("tax"), RuleType::Tax);
        assert_eq!(to_rule_type("behavior"), RuleType::Behavior);
        assert_eq!(to_rule_type("trade"), RuleType::Trade);
        assert_eq!(to_rule_type("diplomacy"), RuleType::Diplomacy);
        assert_eq!(to_rule_type("custom"), RuleType::Custom);
        assert_eq!(to_rule_type("unknown"), RuleType::Custom);
    }

    // ── Round-trip serialization ───────────────────────────

    #[test]
    fn test_yaml_roundtrip() {
        let yaml = r#"
rule:
  id: R-ROUNDTRIP
  name: Roundtrip Test
  scope: global
  priority: 30
  category: tax
  trigger:
    event: on_trade
  conditions:
    - field: trade.amount
      operator: ">"
      value: 100
  actions:
    - type: tax
      params:
        rate: 0.05
        target: seller
"#;
        let result = parse_yaml(yaml);
        assert!(result.valid);
        let rule = result.rule.unwrap();

        let serialized = to_yaml(&rule).unwrap();
        let result2 = parse_yaml(&serialized);
        assert!(result2.valid);
        let rule2 = result2.rule.unwrap();
        assert_eq!(rule2.id, "R-ROUNDTRIP");
        assert_eq!(rule2.trigger.event, "on_trade");
    }

    #[test]
    fn test_json_roundtrip() {
        let json = r#"{"rule": {
            "id": "R-JSON-ROUNDTRIP",
            "name": "JSON Roundtrip",
            "trigger": {"event": "on_tick"},
            "actions": [{"type": "log_event", "params": {}}]
        }}"#;
        let result = parse_json(json);
        assert!(result.valid);
        let rule = result.rule.unwrap();

        let serialized = to_json(&rule).unwrap();
        let result2 = parse_json(&serialized);
        assert!(result2.valid);
        let rule2 = result2.rule.unwrap();
        assert_eq!(rule2.id, "R-JSON-ROUNDTRIP");
    }
}
