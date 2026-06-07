//! Federation gRPC service implementations.
//!
//! Provides `FederationServiceImpl` which handles both WorldRegistryService
//! and MigrationService RPCs.

use std::sync::Arc;

use crate::federation::migration::{MigrationManager, MigrationPolicy};
use crate::federation::registry::WorldRegistry;

/// Federation gRPC service combining WorldRegistry + Migration RPCs.
///
/// Fields will be consumed by gRPC service method implementations once
/// the protobuf codegen layer is wired in.
#[derive(Clone)]
pub struct FederationServiceImpl {
    #[allow(dead_code)]
    world_registry: Arc<WorldRegistry>,
    #[allow(dead_code)]
    migration_manager: Arc<MigrationManager>,
}

impl FederationServiceImpl {
    pub fn new(
        world_registry: Arc<WorldRegistry>,
        migration_manager: Arc<MigrationManager>,
    ) -> Self {
        Self {
            world_registry,
            migration_manager,
        }
    }
}

// ── REST API Types ────────────────────────────────────────

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestMigrationSubmit {
    pub agent_id: String,
    pub source_world_id: String,
    pub target_world_id: String,
    pub name: String,
    pub phase: String,
    pub tokens: u64,
    pub money: u64,
    pub reputation: f64,
    pub skills: std::collections::HashMap<String, u64>,
    pub public_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestMigrationReview {
    pub migration_id: String,
    pub approved: bool,
    pub reviewer_world_id: String,
    #[serde(default)]
    pub rejection_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestWorldRegister {
    pub world_id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub host: String,
    pub grpc_port: u32,
    pub http_port: u32,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub max_agents: u32,
    #[serde(default)]
    pub labels: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestMigrationPolicy {
    pub enabled: bool,
    pub daily_quota: u32,
    pub weekly_quota: u32,
    pub min_reputation: f64,
    pub token_cost: u64,
    pub resource_tax_rate: f64,
    pub require_skill_certification: bool,
    pub blocked_skills: Vec<String>,
    pub cooldown_ticks: u32,
}

impl From<MigrationPolicy> for RestMigrationPolicy {
    fn from(p: MigrationPolicy) -> Self {
        Self {
            enabled: p.enabled,
            daily_quota: p.daily_quota,
            weekly_quota: p.weekly_quota,
            min_reputation: p.min_reputation,
            token_cost: p.token_cost,
            resource_tax_rate: p.resource_tax_rate,
            require_skill_certification: p.require_skill_certification,
            blocked_skills: p.blocked_skills,
            cooldown_ticks: p.cooldown_ticks,
        }
    }
}

impl From<RestMigrationPolicy> for MigrationPolicy {
    fn from(p: RestMigrationPolicy) -> Self {
        Self {
            enabled: p.enabled,
            daily_quota: p.daily_quota,
            weekly_quota: p.weekly_quota,
            min_reputation: p.min_reputation,
            token_cost: p.token_cost,
            resource_tax_rate: p.resource_tax_rate,
            require_skill_certification: p.require_skill_certification,
            blocked_skills: p.blocked_skills,
            cooldown_ticks: p.cooldown_ticks,
        }
    }
}
