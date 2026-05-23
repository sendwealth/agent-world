//! Rule Engine DSL — YAML/JSON rule description language for Agent-proposed rules.
//!
//! Agents (LLM-driven) generate rules in YAML or JSON format following a
//! trigger / condition / action structure. This module parses, validates,
//! and converts those rules into the existing `SoftRule` / `RuleEngine`
//! types so they can enter the legislation lifecycle (propose → vote → activate).

mod parser;

pub use parser::*;
