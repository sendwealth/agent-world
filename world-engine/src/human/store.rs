use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::persistence::sqlite::SCHEMA_SQL;

// ── Oracle Types ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum OracleType {
    Guidance,
    Warning,
    Blessing,
    Curse,
}

impl OracleType {
    fn as_str(&self) -> &'static str {
        match self {
            OracleType::Guidance => "guidance",
            OracleType::Warning => "warning",
            OracleType::Blessing => "blessing",
            OracleType::Curse => "curse",
        }
    }

    fn from_str_lossy(s: &str) -> Self {
        match s {
            "warning" => OracleType::Warning,
            "blessing" => OracleType::Blessing,
            "curse" => OracleType::Curse,
            _ => OracleType::Guidance,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum OracleStatus {
    Pending,
    Delivered,
    Acknowledged,
    Expired,
}

impl OracleStatus {
    fn as_str(&self) -> &'static str {
        match self {
            OracleStatus::Pending => "pending",
            OracleStatus::Delivered => "delivered",
            OracleStatus::Acknowledged => "acknowledged",
            OracleStatus::Expired => "expired",
        }
    }

    fn from_str_lossy(s: &str) -> Self {
        match s {
            "delivered" => OracleStatus::Delivered,
            "acknowledged" => OracleStatus::Acknowledged,
            "expired" => OracleStatus::Expired,
            _ => OracleStatus::Pending,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Oracle {
    pub id: String,
    pub human_id: String,
    pub oracle_type: OracleType,
    pub target_agent_id: String,
    pub content: String,
    pub status: OracleStatus,
    pub agent_response: Option<String>,
    pub created_tick: u64,
    pub delivered_tick: Option<u64>,
}

// ── Bounty Types ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum BountyStatus {
    Open,
    InProgress,
    Completed,
    Expired,
    Cancelled,
}

impl BountyStatus {
    fn as_str(&self) -> &'static str {
        match self {
            BountyStatus::Open => "open",
            BountyStatus::InProgress => "in_progress",
            BountyStatus::Completed => "completed",
            BountyStatus::Expired => "expired",
            BountyStatus::Cancelled => "cancelled",
        }
    }

    fn from_str_lossy(s: &str) -> Self {
        match s {
            "in_progress" => BountyStatus::InProgress,
            "completed" => BountyStatus::Completed,
            "expired" => BountyStatus::Expired,
            "cancelled" => BountyStatus::Cancelled,
            _ => BountyStatus::Open,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bounty {
    pub id: String,
    pub human_id: String,
    pub title: String,
    pub description: String,
    pub reward: u64,
    pub target_agent_id: Option<String>,
    pub status: BountyStatus,
    pub claimant_agent_id: Option<String>,
    pub result: Option<String>,
    pub expires_tick: Option<u64>,
    pub created_tick: u64,
}

// ── Human Portfolio Types ─────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HumanHolding {
    pub agent_id: String,
    pub agent_name: String,
    pub invested: u64,
    pub current_value: u64,
    pub pnl: i64,
    pub pnl_percent: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HumanPortfolio {
    pub human_id: String,
    pub total_assets: u64,
    pub total_invested: u64,
    pub total_pnl: i64,
    pub holdings: Vec<HumanHolding>,
    pub history: Vec<PortfolioHistoryPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioHistoryPoint {
    pub tick: u64,
    pub value: u64,
}

// ── Human Influence Types ─────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HumanInfluenceEntry {
    pub human_id: String,
    pub display_name: String,
    pub total_influence: u64,
    pub oracle_count: usize,
    pub bounty_count: usize,
    pub agents_affected: usize,
    pub economic_impact: u64,
    pub political_impact: u64,
    pub cultural_impact: u64,
}

// ── Claimed Agent Types ──────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimedAgent {
    pub agent_id: String,
    pub agent_name: String,
    pub alive: bool,
    pub tokens: u64,
    pub money: u64,
    pub reputation: f64,
    pub skills: HashMap<String, u32>,
    pub age: u64,
}

// ── Intervention Types ────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum HumanInterventionType {
    DirectControl,
    Guidance,
    Observation,
    Voting,
}

impl HumanInterventionType {
    // TODO: Use when serialising intervention type for persistence/API responses.
    #[allow(dead_code)]
    fn as_str(&self) -> &'static str {
        match self {
            HumanInterventionType::DirectControl => "direct_control",
            HumanInterventionType::Guidance => "guidance",
            HumanInterventionType::Observation => "observation",
            HumanInterventionType::Voting => "voting",
        }
    }

    fn from_str_lossy(s: &str) -> Self {
        match s {
            "direct_control" => HumanInterventionType::DirectControl,
            "observation" => HumanInterventionType::Observation,
            "voting" => HumanInterventionType::Voting,
            _ => HumanInterventionType::Guidance,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HumanInterventionEvent {
    pub id: String,
    pub human_id: String,
    pub intervention_type: HumanInterventionType,
    pub target_agent_id: Option<String>,
    pub description: String,
    pub tick: u64,
    pub impact_score: f64,
}

// ── Request Types ─────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct SendOracleRequest {
    pub human_id: String,
    pub oracle_type: OracleType,
    pub target_agent_id: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateBountyRequest {
    pub human_id: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub reward: u64,
    pub target_agent_id: Option<String>,
    pub expires_tick: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct ClaimBountyRequest {
    pub agent_id: String,
}

#[derive(Debug, Deserialize)]
pub struct CompleteBountyRequest {
    pub result: String,
}

#[derive(Debug, Deserialize)]
pub struct OracleResponseRequest {
    pub agent_id: String,
    pub response: String,
}

#[derive(Debug, Deserialize)]
pub struct ClaimAgentRequest {
    pub human_id: String,
    pub agent_id: String,
}

#[derive(Debug, Deserialize)]
pub struct InvestRequest {
    pub human_id: String,
    pub agent_id: String,
    pub amount: u64,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct ListBountiesQuery {
    pub status: Option<String>,
    pub human_id: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct ListOraclesQuery {
    pub status: Option<String>,
    pub human_id: Option<String>,
    pub target_agent_id: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct InfluenceRankingsQuery {
    pub sort_by: Option<String>,
    pub limit: Option<usize>,
}

// ── Store ─────────────────────────────────────────────────────

pub type SharedHumanStore = Arc<Mutex<HumanParticipationStore>>;

/// SQLite-backed human participation store.
///
/// All data (oracles, bounties, portfolios, claimed agents, influence,
/// interventions) is persisted to SQLite. Restart-safe.
pub struct HumanParticipationStore {
    conn: std::sync::Mutex<Connection>,
    current_tick: u64,
}

impl HumanParticipationStore {
    /// Open (or create) the store at the given database path.
    /// Shares the schema with the main persistence module.
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        conn.execute_batch(SCHEMA_SQL)?;
        conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;")?;
        Ok(Self {
            conn: std::sync::Mutex::new(conn),
            current_tick: 0,
        })
    }

    /// Open an in-memory store (for testing).
    pub fn open_in_memory() -> anyhow::Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(SCHEMA_SQL)?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        Ok(Self {
            conn: std::sync::Mutex::new(conn),
            current_tick: 0,
        })
    }

    /// Create an in-memory store, panicking on failure (test convenience).
    pub fn new() -> Self {
        Self::open_in_memory().expect("in-memory SQLite should never fail")
    }
}

impl Default for HumanParticipationStore {
    fn default() -> Self {
        Self::new()
    }
}

impl HumanParticipationStore {

    pub fn set_tick(&mut self, tick: u64) {
        self.current_tick = tick;
    }

    fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap()
    }

    // ── Oracle operations ────────────────────────────────────

    pub fn send_oracle(&mut self, req: SendOracleRequest) -> Oracle {
        let oracle = Oracle {
            id: Uuid::new_v4().to_string(),
            human_id: req.human_id.clone(),
            oracle_type: req.oracle_type,
            target_agent_id: req.target_agent_id,
            content: req.content,
            status: OracleStatus::Pending,
            agent_response: None,
            created_tick: self.current_tick,
            delivered_tick: None,
        };

        let conn = self.conn();
        conn.execute(
            "INSERT INTO human_oracles (id, human_id, oracle_type, target_agent_id, content, status, created_tick) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                oracle.id,
                oracle.human_id,
                oracle.oracle_type.as_str(),
                oracle.target_agent_id,
                oracle.content,
                oracle.status.as_str(),
                oracle.created_tick as i64,
            ],
        )
        .expect("insert oracle should succeed");

        drop(conn);
        self.touch_influence(&req.human_id);
        oracle
    }

    pub fn list_oracles(&self, query: &ListOraclesQuery) -> Vec<Oracle> {
        let conn = self.conn();
        let mut sql = String::from("SELECT id, human_id, oracle_type, target_agent_id, content, status, agent_response, created_tick, delivered_tick FROM human_oracles WHERE 1=1");
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(ref status) = query.status {
            sql.push_str(" AND status = ?");
            param_values.push(Box::new(status.clone()));
        }
        if let Some(ref human_id) = query.human_id {
            sql.push_str(" AND human_id = ?");
            param_values.push(Box::new(human_id.clone()));
        }
        if let Some(ref target_agent_id) = query.target_agent_id {
            sql.push_str(" AND target_agent_id = ?");
            param_values.push(Box::new(target_agent_id.clone()));
        }
        sql.push_str(" ORDER BY created_tick DESC");

        let params_refs: Vec<&dyn rusqlite::types::ToSql> = param_values.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&sql).expect("prepare oracle list");
        let rows = stmt.query_map(params_refs.as_slice(), |row| {
            Ok(oracle_from_row(row))
        }).expect("query oracle list");

        rows.filter_map(|r| r.ok()).collect()
    }

    pub fn get_oracle(&self, id: &str) -> Option<Oracle> {
        let conn = self.conn();
        conn.query_row(
            "SELECT id, human_id, oracle_type, target_agent_id, content, status, agent_response, created_tick, delivered_tick \
             FROM human_oracles WHERE id = ?1",
            params![id],
            |row| Ok(oracle_from_row(row)),
        )
        .ok()
    }

    pub fn respond_to_oracle(&mut self, oracle_id: &str, agent_id: &str, response: &str) -> Option<Oracle> {
        let conn = self.conn();
        // First fetch the oracle and verify it's targeted at this agent
        let oracle: Oracle = conn.query_row(
            "SELECT id, human_id, oracle_type, target_agent_id, content, status, agent_response, created_tick, delivered_tick \
             FROM human_oracles WHERE id = ?1",
            params![oracle_id],
            |row| Ok(oracle_from_row(row)),
        ).ok()?;

        if oracle.target_agent_id != agent_id {
            return None;
        }
        if oracle.status != OracleStatus::Pending && oracle.status != OracleStatus::Delivered {
            return None;
        }

        conn.execute(
            "UPDATE human_oracles SET status = 'acknowledged', agent_response = ?1 WHERE id = ?2",
            params![response, oracle_id],
        )
        .ok()?;

        let updated = Oracle {
            status: OracleStatus::Acknowledged,
            agent_response: Some(response.to_string()),
            ..oracle
        };
        Some(updated)
    }

    // ── Bounty operations ────────────────────────────────────

    pub fn create_bounty(&mut self, req: CreateBountyRequest) -> Bounty {
        let bounty = Bounty {
            id: Uuid::new_v4().to_string(),
            human_id: req.human_id.clone(),
            title: req.title,
            description: req.description,
            reward: req.reward,
            target_agent_id: req.target_agent_id,
            status: BountyStatus::Open,
            claimant_agent_id: None,
            result: None,
            expires_tick: req.expires_tick,
            created_tick: self.current_tick,
        };

        let conn = self.conn();
        conn.execute(
            "INSERT INTO human_bounties (id, human_id, title, description, reward, target_agent_id, status, expires_tick, created_tick) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                bounty.id,
                bounty.human_id,
                bounty.title,
                bounty.description,
                bounty.reward as i64,
                bounty.target_agent_id,
                bounty.status.as_str(),
                bounty.expires_tick.map(|t| t as i64),
                bounty.created_tick as i64,
            ],
        )
        .expect("insert bounty should succeed");

        drop(conn);
        self.touch_influence(&req.human_id);
        bounty
    }

    pub fn list_bounties(&self, query: &ListBountiesQuery) -> Vec<Bounty> {
        let conn = self.conn();
        let mut sql = String::from("SELECT id, human_id, title, description, reward, target_agent_id, status, claimant_agent_id, result, expires_tick, created_tick FROM human_bounties WHERE 1=1");
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(ref status) = query.status {
            sql.push_str(" AND status = ?");
            param_values.push(Box::new(status.clone()));
        }
        if let Some(ref human_id) = query.human_id {
            sql.push_str(" AND human_id = ?");
            param_values.push(Box::new(human_id.clone()));
        }
        sql.push_str(" ORDER BY created_tick DESC");

        let params_refs: Vec<&dyn rusqlite::types::ToSql> = param_values.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&sql).expect("prepare bounty list");
        let rows = stmt.query_map(params_refs.as_slice(), |row| {
            Ok(bounty_from_row(row))
        }).expect("query bounty list");

        rows.filter_map(|r| r.ok()).collect()
    }

    pub fn get_bounty(&self, id: &str) -> Option<Bounty> {
        let conn = self.conn();
        conn.query_row(
            "SELECT id, human_id, title, description, reward, target_agent_id, status, claimant_agent_id, result, expires_tick, created_tick \
             FROM human_bounties WHERE id = ?1",
            params![id],
            |row| Ok(bounty_from_row(row)),
        )
        .ok()
    }

    pub fn claim_bounty(&mut self, bounty_id: &str, agent_id: &str) -> Option<Bounty> {
        let conn = self.conn();
        // Check status is open
        let status: String = conn.query_row(
            "SELECT status FROM human_bounties WHERE id = ?1",
            params![bounty_id],
            |row| row.get(0),
        ).ok()?;

        if status != "open" {
            return None;
        }

        conn.execute(
            "UPDATE human_bounties SET status = 'in_progress', claimant_agent_id = ?1 WHERE id = ?2",
            params![agent_id, bounty_id],
        )
        .ok()?;

        drop(conn);
        self.get_bounty(bounty_id)
    }

    pub fn complete_bounty(&mut self, bounty_id: &str, result: &str) -> Option<Bounty> {
        let conn = self.conn();
        let status: String = conn.query_row(
            "SELECT status FROM human_bounties WHERE id = ?1",
            params![bounty_id],
            |row| row.get(0),
        ).ok()?;

        if status != "in_progress" {
            return None;
        }

        conn.execute(
            "UPDATE human_bounties SET status = 'completed', result = ?1 WHERE id = ?2",
            params![result, bounty_id],
        )
        .ok()?;

        drop(conn);
        self.get_bounty(bounty_id)
    }

    pub fn cancel_bounty(&mut self, bounty_id: &str) -> Option<Bounty> {
        let conn = self.conn();
        let status: String = conn.query_row(
            "SELECT status FROM human_bounties WHERE id = ?1",
            params![bounty_id],
            |row| row.get(0),
        ).ok()?;

        if status == "completed" || status == "cancelled" {
            return None;
        }

        conn.execute(
            "UPDATE human_bounties SET status = 'cancelled' WHERE id = ?1",
            params![bounty_id],
        )
        .ok()?;

        drop(conn);
        self.get_bounty(bounty_id)
    }

    // ── Portfolio operations ──────────────────────────────────

    pub fn get_portfolio(&self, human_id: &str) -> Option<HumanPortfolio> {
        let conn = self.conn();

        // Check if portfolio row exists
        let exists: bool = conn.query_row(
            "SELECT COUNT(*) FROM human_portfolios WHERE human_id = ?1",
            params![human_id],
            |row| row.get::<_, i64>(0),
        ).unwrap_or(0) > 0;

        if !exists {
            return None;
        }

        let total_assets: i64 = conn.query_row(
            "SELECT total_assets FROM human_portfolios WHERE human_id = ?1",
            params![human_id],
            |row| row.get(0),
        ).unwrap_or(0);

        let total_invested: i64 = conn.query_row(
            "SELECT total_invested FROM human_portfolios WHERE human_id = ?1",
            params![human_id],
            |row| row.get(0),
        ).unwrap_or(0);

        let total_pnl: i64 = conn.query_row(
            "SELECT total_pnl FROM human_portfolios WHERE human_id = ?1",
            params![human_id],
            |row| row.get(0),
        ).unwrap_or(0);

        // Load holdings
        let mut holdings_stmt = conn.prepare(
            "SELECT agent_id, agent_name, invested, current_value, pnl, pnl_percent FROM human_holdings WHERE human_id = ?1",
        ).expect("prepare holdings");
        let holdings: Vec<HumanHolding> = holdings_stmt.query_map(params![human_id], |row| {
            Ok(HumanHolding {
                agent_id: row.get(0)?,
                agent_name: row.get(1)?,
                invested: row.get::<_, i64>(2)? as u64,
                current_value: row.get::<_, i64>(3)? as u64,
                pnl: row.get::<_, i64>(4)?,
                pnl_percent: row.get(5)?,
            })
        }).expect("query holdings").filter_map(|r| r.ok()).collect();

        // Load history
        let mut history_stmt = conn.prepare(
            "SELECT tick, value FROM human_portfolio_history WHERE human_id = ?1 ORDER BY tick",
        ).expect("prepare history");
        let history: Vec<PortfolioHistoryPoint> = history_stmt.query_map(params![human_id], |row| {
            Ok(PortfolioHistoryPoint {
                tick: row.get::<_, i64>(0)? as u64,
                value: row.get::<_, i64>(1)? as u64,
            })
        }).expect("query history").filter_map(|r| r.ok()).collect();

        Some(HumanPortfolio {
            human_id: human_id.to_string(),
            total_assets: total_assets as u64,
            total_invested: total_invested as u64,
            total_pnl,
            holdings,
            history,
        })
    }

    pub fn invest(
        &mut self,
        human_id: &str,
        agent_id: &str,
        agent_name: &str,
        amount: u64,
    ) -> HumanPortfolio {
        let conn = self.conn();

        // Upsert portfolio row
        conn.execute(
            "INSERT INTO human_portfolios (human_id, total_assets, total_invested, total_pnl) \
             VALUES (?1, 0, 0, 0) ON CONFLICT(human_id) DO NOTHING",
            params![human_id],
        ).expect("upsert portfolio");

        // Upsert holding
        conn.execute(
            "INSERT INTO human_holdings (human_id, agent_id, agent_name, invested, current_value, pnl, pnl_percent) \
             VALUES (?1, ?2, ?3, ?4, ?4, 0, 0.0) \
             ON CONFLICT(human_id, agent_id) DO UPDATE SET \
               invested = invested + ?4, \
               current_value = current_value + ?4",
            params![human_id, agent_id, agent_name, amount as i64],
        ).expect("upsert holding");

        // Update portfolio totals
        conn.execute(
            "UPDATE human_portfolios SET \
               total_invested = total_invested + ?1, \
               total_assets = total_assets + ?1, \
               total_pnl = total_assets + ?1 - total_invested - ?1 + (SELECT COALESCE(SUM(current_value), 0) FROM human_holdings WHERE human_id = ?2) - (total_invested + ?1) \
             WHERE human_id = ?2",
            params![amount as i64, human_id],
        ).expect("update portfolio totals");

        // Simpler approach: recalculate from holdings
        conn.execute(
            "UPDATE human_portfolios SET \
               total_assets = (SELECT COALESCE(SUM(current_value), 0) FROM human_holdings WHERE human_id = ?1), \
               total_invested = (SELECT COALESCE(SUM(invested), 0) FROM human_holdings WHERE human_id = ?1), \
               total_pnl = (SELECT COALESCE(SUM(current_value), 0) FROM human_holdings WHERE human_id = ?1) - (SELECT COALESCE(SUM(invested), 0) FROM human_holdings WHERE human_id = ?1) \
             WHERE human_id = ?1",
            params![human_id],
        ).expect("recalc portfolio");

        // Insert history point
        conn.execute(
            "INSERT INTO human_portfolio_history (human_id, tick, value) VALUES (?1, ?2, \
             (SELECT total_assets FROM human_portfolios WHERE human_id = ?1))",
            params![human_id, self.current_tick as i64],
        ).expect("insert portfolio history");

        drop(conn);
        self.get_portfolio(human_id).expect("portfolio must exist after invest")
    }

    // ── Claimed agent operations ──────────────────────────────

    #[allow(clippy::too_many_arguments)]
    pub fn claim_agent(
        &mut self,
        human_id: &str,
        agent_id: &str,
        agent_name: &str,
        tokens: u64,
        money: u64,
        reputation: f64,
        skills: HashMap<String, u32>,
        age: u64,
    ) -> ClaimedAgent {
        let skills_json = serde_json::to_string(&skills).expect("serialize skills");

        let conn = self.conn();
        conn.execute(
            "INSERT INTO human_claimed_agents (human_id, agent_id, agent_name, alive, tokens, money, reputation, skills_json, age) \
             VALUES (?1, ?2, ?3, 1, ?4, ?5, ?6, ?7, ?8) \
             ON CONFLICT(human_id, agent_id) DO UPDATE SET \
               agent_name = excluded.agent_name, \
               alive = excluded.alive, \
               tokens = excluded.tokens, \
               money = excluded.money, \
               reputation = excluded.reputation, \
               skills_json = excluded.skills_json, \
               age = excluded.age",
            params![
                human_id,
                agent_id,
                agent_name,
                tokens as i64,
                money as i64,
                reputation,
                skills_json,
                age as i64,
            ],
        )
        .expect("upsert claimed agent");

        ClaimedAgent {
            agent_id: agent_id.to_string(),
            agent_name: agent_name.to_string(),
            alive: true,
            tokens,
            money,
            reputation,
            skills,
            age,
        }
    }

    pub fn list_claimed_agents(&self, human_id: &str) -> Vec<ClaimedAgent> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT agent_id, agent_name, alive, tokens, money, reputation, skills_json, age \
             FROM human_claimed_agents WHERE human_id = ?1",
        ).expect("prepare claimed agents");

        let rows = stmt.query_map(params![human_id], |row| {
            let skills_json: String = row.get(6)?;
            let skills: HashMap<String, u32> = serde_json::from_str(&skills_json).unwrap_or_default();
            Ok(ClaimedAgent {
                agent_id: row.get(0)?,
                agent_name: row.get(1)?,
                alive: row.get::<_, i64>(2)? != 0,
                tokens: row.get::<_, i64>(3)? as u64,
                money: row.get::<_, i64>(4)? as u64,
                reputation: row.get(5)?,
                skills,
                age: row.get::<_, i64>(7)? as u64,
            })
        }).expect("query claimed agents");

        rows.filter_map(|r| r.ok()).collect()
    }

    // ── Influence rankings ───────────────────────────────────

    pub fn get_influence_rankings(&self, sort_by: &str, limit: usize) -> Vec<HumanInfluenceEntry> {
        let conn = self.conn();
        let order = match sort_by {
            "economic_impact" => "economic_impact DESC",
            "political_impact" => "political_impact DESC",
            "cultural_impact" => "cultural_impact DESC",
            _ => "total_influence DESC",
        };
        let sql = format!(
            "SELECT human_id, display_name, total_influence, oracle_count, bounty_count, agents_affected, economic_impact, political_impact, cultural_impact \
             FROM human_influence ORDER BY {} LIMIT ?1",
            order
        );
        let mut stmt = conn.prepare(&sql).expect("prepare influence rankings");
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok(HumanInfluenceEntry {
                human_id: row.get(0)?,
                display_name: row.get(1)?,
                total_influence: row.get::<_, i64>(2)? as u64,
                oracle_count: row.get::<_, i64>(3)? as usize,
                bounty_count: row.get::<_, i64>(4)? as usize,
                agents_affected: row.get::<_, i64>(5)? as usize,
                economic_impact: row.get::<_, i64>(6)? as u64,
                political_impact: row.get::<_, i64>(7)? as u64,
                cultural_impact: row.get::<_, i64>(8)? as u64,
            })
        }).expect("query influence rankings");

        rows.filter_map(|r| r.ok()).collect()
    }

    fn touch_influence(&mut self, human_id: &str) {
        let conn = self.conn();

        let oracle_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM human_oracles WHERE human_id = ?1",
            params![human_id],
            |row| row.get(0),
        ).unwrap_or(0);

        let bounty_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM human_bounties WHERE human_id = ?1",
            params![human_id],
            |row| row.get(0),
        ).unwrap_or(0);

        let oracle_impact = oracle_count * 10;
        let bounty_impact = bounty_count * 15;

        conn.execute(
            "INSERT INTO human_influence (human_id, display_name, total_influence, oracle_count, bounty_count, agents_affected, economic_impact, political_impact, cultural_impact) \
             VALUES (?1, ?2, ?3, ?4, ?5, 0, 0, 0, 0) \
             ON CONFLICT(human_id) DO UPDATE SET \
               oracle_count = ?4, \
               bounty_count = ?5, \
               total_influence = ?3 + economic_impact + political_impact + cultural_impact",
            params![
                human_id,
                format!("Human-{}", &human_id[..8.min(human_id.len())]),
                oracle_impact + bounty_impact,
                oracle_count,
                bounty_count,
            ],
        ).expect("upsert influence");
    }

    // ── Intervention events ──────────────────────────────────

    pub fn list_interventions(
        &self,
        human_id: Option<&str>,
        limit: usize,
    ) -> Vec<HumanInterventionEvent> {
        let conn = self.conn();

        let mut interventions = Vec::new();

        if let Some(hid) = human_id {
            let mut stmt = conn.prepare(
                "SELECT id, human_id, intervention_type, target_agent_id, description, tick, impact_score \
                 FROM human_interventions WHERE human_id = ?1 ORDER BY tick DESC LIMIT ?2"
            ).expect("prepare interventions");
            let rows = stmt.query_map(params![hid, limit as i64], |row| {
                Ok(intervention_from_row(row))
            }).expect("query interventions");
            interventions.extend(rows.filter_map(|r| r.ok()));
        } else {
            let mut stmt = conn.prepare(
                "SELECT id, human_id, intervention_type, target_agent_id, description, tick, impact_score \
                 FROM human_interventions ORDER BY tick DESC LIMIT ?1"
            ).expect("prepare interventions");
            let rows = stmt.query_map(params![limit as i64], |row| {
                Ok(intervention_from_row(row))
            }).expect("query interventions");
            interventions.extend(rows.filter_map(|r| r.ok()));
        }

        interventions
    }

    // ── Stats ────────────────────────────────────────────────

    pub fn get_stats(&self) -> HumanStats {
        let conn = self.conn();

        let total_oracles: i64 = conn.query_row(
            "SELECT COUNT(*) FROM human_oracles",
            [],
            |row| row.get(0),
        ).unwrap_or(0);

        let total_bounties: i64 = conn.query_row(
            "SELECT COUNT(*) FROM human_bounties",
            [],
            |row| row.get(0),
        ).unwrap_or(0);

        let total_investments: i64 = conn.query_row(
            "SELECT COUNT(*) FROM human_portfolios",
            [],
            |row| row.get(0),
        ).unwrap_or(0);

        // Count unique humans across all participation
        let active_humans: i64 = conn.query_row(
            "SELECT COUNT(DISTINCT human_id) FROM (\
               SELECT human_id FROM human_oracles \
               UNION SELECT human_id FROM human_bounties \
               UNION SELECT human_id FROM human_portfolios\
             )",
            [],
            |row| row.get(0),
        ).unwrap_or(0);

        // Intervention type distribution
        let mut type_counts: HashMap<String, usize> = HashMap::new();
        let mut stmt = conn.prepare(
            "SELECT intervention_type, COUNT(*) FROM human_interventions GROUP BY intervention_type",
        ).unwrap();
        let rows: Vec<(String, i64)> = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        }).unwrap().filter_map(|r| r.ok()).collect();
        for (itype, count) in rows {
            type_counts.insert(itype, count as usize);
        }

        HumanStats {
            active_humans: active_humans as usize,
            total_oracles: total_oracles as usize,
            total_bounties: total_bounties as usize,
            total_investments: total_investments as usize,
            intervention_type_distribution: type_counts,
        }
    }

    // ── Token recharge ────────────────────────────────────────

    /// Recharge tokens for a specific agent. Returns the recharge log ID.
    pub fn recharge_agent(&mut self, agent_id: &str, human_id: &str, amount: u64) -> String {
        let id = Uuid::new_v4().to_string();
        let conn = self.conn();
        conn.execute(
            "INSERT INTO token_recharge_log (id, agent_id, human_id, amount, tick) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, agent_id, human_id, amount as i64, self.current_tick as i64],
        ).expect("insert recharge log");
        id
    }

    /// Get recharge history for an agent.
    pub fn get_recharge_history(&self, agent_id: &str, limit: usize) -> Vec<RechargeEntry> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, agent_id, human_id, amount, tick FROM token_recharge_log WHERE agent_id = ?1 ORDER BY tick DESC LIMIT ?2",
        ).expect("prepare recharge history");
        let rows = stmt.query_map(params![agent_id, limit as i64], |row| {
            Ok(RechargeEntry {
                id: row.get(0)?,
                agent_id: row.get(1)?,
                human_id: row.get(2)?,
                amount: row.get::<_, i64>(3)? as u64,
                tick: row.get::<_, i64>(4)? as u64,
            })
        }).expect("query recharge history");
        rows.filter_map(|r| r.ok()).collect()
    }
}

// ── Row mapping helpers ───────────────────────────────────

fn oracle_from_row(row: &rusqlite::Row<'_>) -> Oracle {
    let status_str: String = row.get(5).unwrap_or_default();
    let agent_response: Option<String> = row.get(6).unwrap_or(None);
    let delivered_tick: Option<i64> = row.get(8).unwrap_or(None);

    Oracle {
        id: row.get(0).unwrap_or_default(),
        human_id: row.get(1).unwrap_or_default(),
        oracle_type: OracleType::from_str_lossy(&row.get::<_, String>(2).unwrap_or_default()),
        target_agent_id: row.get(3).unwrap_or_default(),
        content: row.get(4).unwrap_or_default(),
        status: OracleStatus::from_str_lossy(&status_str),
        agent_response,
        created_tick: row.get::<_, i64>(7).unwrap_or(0) as u64,
        delivered_tick: delivered_tick.map(|t| t as u64),
    }
}

fn bounty_from_row(row: &rusqlite::Row<'_>) -> Bounty {
    let status_str: String = row.get(6).unwrap_or_default();
    let expires_tick: Option<i64> = row.get(9).unwrap_or(None);

    Bounty {
        id: row.get(0).unwrap_or_default(),
        human_id: row.get(1).unwrap_or_default(),
        title: row.get(2).unwrap_or_default(),
        description: row.get(3).unwrap_or_default(),
        reward: row.get::<_, i64>(4).unwrap_or(0) as u64,
        target_agent_id: row.get(5).unwrap_or(None),
        status: BountyStatus::from_str_lossy(&status_str),
        claimant_agent_id: row.get(7).unwrap_or(None),
        result: row.get(8).unwrap_or(None),
        expires_tick: expires_tick.map(|t| t as u64),
        created_tick: row.get::<_, i64>(10).unwrap_or(0) as u64,
    }
}

fn intervention_from_row(row: &rusqlite::Row<'_>) -> HumanInterventionEvent {
    HumanInterventionEvent {
        id: row.get(0).unwrap_or_default(),
        human_id: row.get(1).unwrap_or_default(),
        intervention_type: HumanInterventionType::from_str_lossy(&row.get::<_, String>(2).unwrap_or_default()),
        target_agent_id: row.get(3).unwrap_or(None),
        description: row.get(4).unwrap_or_default(),
        tick: row.get::<_, i64>(5).unwrap_or(0) as u64,
        impact_score: row.get(6).unwrap_or(0.0),
    }
}

// ── Response types ──────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct HumanStats {
    pub active_humans: usize,
    pub total_oracles: usize,
    pub total_bounties: usize,
    pub total_investments: usize,
    pub intervention_type_distribution: HashMap<String, usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RechargeEntry {
    pub id: String,
    pub agent_id: String,
    pub human_id: String,
    pub amount: u64,
    pub tick: u64,
}

#[derive(Debug, Deserialize)]
pub struct RechargeRequest {
    pub amount: u64,
}
