use std::sync::Arc;
use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use uuid::Uuid;

// ── Oracle Types ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum OracleType {
    Guidance,
    Warning,
    Blessing,
    Curse,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum OracleStatus {
    Pending,
    Delivered,
    Acknowledged,
    Expired,
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

#[derive(Debug, Clone)]
pub struct HumanParticipationStore {
    oracles: Vec<Oracle>,
    bounties: Vec<Bounty>,
    portfolios: HashMap<String, HumanPortfolio>,
    claimed_agents: HashMap<String, Vec<ClaimedAgent>>,
    influence_entries: Vec<HumanInfluenceEntry>,
    interventions: Vec<HumanInterventionEvent>,
    current_tick: u64,
}

impl Default for HumanParticipationStore {
    fn default() -> Self {
        Self::new()
    }
}

impl HumanParticipationStore {
    pub fn new() -> Self {
        Self {
            oracles: Vec::new(),
            bounties: Vec::new(),
            portfolios: HashMap::new(),
            claimed_agents: HashMap::new(),
            influence_entries: Vec::new(),
            interventions: Vec::new(),
            current_tick: 0,
        }
    }

    pub fn set_tick(&mut self, tick: u64) {
        self.current_tick = tick;
    }

    // ── Oracle operations ────────────────────────────────────

    pub fn send_oracle(&mut self, req: SendOracleRequest) -> Oracle {
        let oracle = Oracle {
            id: Uuid::new_v4().to_string(),
            human_id: req.human_id,
            oracle_type: req.oracle_type,
            target_agent_id: req.target_agent_id,
            content: req.content,
            status: OracleStatus::Pending,
            agent_response: None,
            created_tick: self.current_tick,
            delivered_tick: None,
        };
        self.oracles.push(oracle.clone());
        // Update influence
        self.touch_influence(&oracle.human_id);
        oracle
    }

    pub fn list_oracles(&self, query: &ListOraclesQuery) -> Vec<&Oracle> {
        let mut result: Vec<&Oracle> = self.oracles.iter().collect();
        if let Some(ref status) = query.status {
            result.retain(|o| format!("{:?}", o.status).to_lowercase() == *status);
        }
        if let Some(ref human_id) = query.human_id {
            result.retain(|o| o.human_id == *human_id);
        }
        if let Some(ref target_agent_id) = query.target_agent_id {
            result.retain(|o| o.target_agent_id == *target_agent_id);
        }
        result
    }

    pub fn get_oracle(&self, id: &str) -> Option<&Oracle> {
        self.oracles.iter().find(|o| o.id == id)
    }

    // ── Bounty operations ────────────────────────────────────

    pub fn create_bounty(&mut self, req: CreateBountyRequest) -> Bounty {
        let bounty = Bounty {
            id: Uuid::new_v4().to_string(),
            human_id: req.human_id,
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
        self.bounties.push(bounty.clone());
        self.touch_influence(&bounty.human_id);
        bounty
    }

    pub fn list_bounties(&self, query: &ListBountiesQuery) -> Vec<&Bounty> {
        let mut result: Vec<&Bounty> = self.bounties.iter().collect();
        if let Some(ref status) = query.status {
            result.retain(|b| format!("{:?}", b.status).to_lowercase() == *status);
        }
        if let Some(ref human_id) = query.human_id {
            result.retain(|b| b.human_id == *human_id);
        }
        result
    }

    pub fn get_bounty(&self, id: &str) -> Option<&Bounty> {
        self.bounties.iter().find(|b| b.id == id)
    }

    pub fn claim_bounty(&mut self, bounty_id: &str, agent_id: &str) -> Option<Bounty> {
        let bounty = self.bounties.iter_mut().find(|b| b.id == bounty_id)?;
        if bounty.status != BountyStatus::Open {
            return None;
        }
        bounty.status = BountyStatus::InProgress;
        bounty.claimant_agent_id = Some(agent_id.to_string());
        Some(bounty.clone())
    }

    pub fn complete_bounty(&mut self, bounty_id: &str, result: &str) -> Option<Bounty> {
        let bounty = self.bounties.iter_mut().find(|b| b.id == bounty_id)?;
        if bounty.status != BountyStatus::InProgress {
            return None;
        }
        bounty.status = BountyStatus::Completed;
        bounty.result = Some(result.to_string());
        Some(bounty.clone())
    }

    pub fn cancel_bounty(&mut self, bounty_id: &str) -> Option<Bounty> {
        let bounty = self.bounties.iter_mut().find(|b| b.id == bounty_id)?;
        if bounty.status == BountyStatus::Completed || bounty.status == BountyStatus::Cancelled {
            return None;
        }
        bounty.status = BountyStatus::Cancelled;
        Some(bounty.clone())
    }

    // ── Portfolio operations ──────────────────────────────────

    pub fn get_portfolio(&self, human_id: &str) -> Option<&HumanPortfolio> {
        self.portfolios.get(human_id)
    }

    pub fn invest(&mut self, human_id: &str, agent_id: &str, agent_name: &str, amount: u64) -> HumanPortfolio {
        let portfolio = self.portfolios.entry(human_id.to_string()).or_insert_with(|| {
            HumanPortfolio {
                human_id: human_id.to_string(),
                total_assets: 0,
                total_invested: 0,
                total_pnl: 0,
                holdings: Vec::new(),
                history: Vec::new(),
            }
        });

        if let Some(holding) = portfolio.holdings.iter_mut().find(|h| h.agent_id == agent_id) {
            holding.invested += amount;
            holding.current_value += amount;
        } else {
            portfolio.holdings.push(HumanHolding {
                agent_id: agent_id.to_string(),
                agent_name: agent_name.to_string(),
                invested: amount,
                current_value: amount,
                pnl: 0,
                pnl_percent: 0.0,
            });
        }

        portfolio.total_invested += amount;
        portfolio.total_assets += amount;
        portfolio.total_pnl = portfolio.total_assets as i64 - portfolio.total_invested as i64;

        portfolio.history.push(PortfolioHistoryPoint {
            tick: self.current_tick,
            value: portfolio.total_assets,
        });

        portfolio.clone()
    }

    // ── Claimed agent operations ──────────────────────────────

    #[allow(clippy::too_many_arguments)]
    pub fn claim_agent(&mut self, human_id: &str, agent_id: &str, agent_name: &str, tokens: u64, money: u64, reputation: f64, skills: HashMap<String, u32>, age: u64) -> ClaimedAgent {
        let claimed = ClaimedAgent {
            agent_id: agent_id.to_string(),
            agent_name: agent_name.to_string(),
            alive: true,
            tokens,
            money,
            reputation,
            skills,
            age,
        };

        let agents = self.claimed_agents.entry(human_id.to_string()).or_default();

        // Remove existing claim on the same agent if any
        agents.retain(|a| a.agent_id != agent_id);
        agents.push(claimed.clone());

        claimed
    }

    pub fn list_claimed_agents(&self, human_id: &str) -> Vec<&ClaimedAgent> {
        self.claimed_agents.get(human_id).map(|v| v.iter().collect()).unwrap_or_default()
    }

    // ── Influence rankings ───────────────────────────────────

    pub fn get_influence_rankings(&self, sort_by: &str, limit: usize) -> Vec<&HumanInfluenceEntry> {
        let mut entries: Vec<&HumanInfluenceEntry> = self.influence_entries.iter().collect();
        match sort_by {
            "economic_impact" => entries.sort_by_key(|b| std::cmp::Reverse(b.economic_impact)),
            "political_impact" => entries.sort_by_key(|b| std::cmp::Reverse(b.political_impact)),
            "cultural_impact" => entries.sort_by_key(|b| std::cmp::Reverse(b.cultural_impact)),
            _ => entries.sort_by_key(|b| std::cmp::Reverse(b.total_influence)),
        }
        entries.truncate(limit);
        entries
    }

    fn touch_influence(&mut self, human_id: &str) {
        if let Some(entry) = self.influence_entries.iter_mut().find(|e| e.human_id == *human_id) {
            entry.oracle_count = self.oracles.iter().filter(|o| o.human_id == human_id).count();
            entry.bounty_count = self.bounties.iter().filter(|b| b.human_id == human_id).count();
            let oracle_impact: u64 = entry.oracle_count as u64 * 10;
            let bounty_impact: u64 = entry.bounty_count as u64 * 15;
            entry.total_influence = oracle_impact + bounty_impact + entry.economic_impact + entry.political_impact + entry.cultural_impact;
        } else {
            let oracle_count = self.oracles.iter().filter(|o| o.human_id == human_id).count();
            let bounty_count = self.bounties.iter().filter(|b| b.human_id == human_id).count();
            self.influence_entries.push(HumanInfluenceEntry {
                human_id: human_id.to_string(),
                display_name: format!("Human-{}", &human_id[..8.min(human_id.len())]),
                total_influence: oracle_count as u64 * 10 + bounty_count as u64 * 15,
                oracle_count,
                bounty_count,
                agents_affected: 0,
                economic_impact: 0,
                political_impact: 0,
                cultural_impact: 0,
            });
        }
    }

    // ── Intervention events ──────────────────────────────────

    pub fn list_interventions(&self, human_id: Option<&str>, limit: usize) -> Vec<&HumanInterventionEvent> {
        let mut result: Vec<&HumanInterventionEvent> = if let Some(hid) = human_id {
            self.interventions.iter().filter(|e| e.human_id == hid).collect()
        } else {
            self.interventions.iter().collect()
        };
        // Most recent first
        result.sort_by_key(|b| std::cmp::Reverse(b.tick));
        result.truncate(limit);
        result
    }

    // ── Stats ────────────────────────────────────────────────

    pub fn get_stats(&self) -> HumanStats {
        let unique_humans: std::collections::HashSet<&str> = self.oracles.iter()
            .map(|o| o.human_id.as_str())
            .chain(self.bounties.iter().map(|b| b.human_id.as_str()))
            .chain(self.portfolios.keys().map(|s| s.as_str()))
            .collect();

        let mut type_counts: HashMap<String, usize> = HashMap::new();
        for iv in &self.interventions {
            *type_counts.entry(format!("{:?}", iv.intervention_type).to_lowercase()).or_default() += 1;
        }

        HumanStats {
            active_humans: unique_humans.len(),
            total_oracles: self.oracles.len(),
            total_bounties: self.bounties.len(),
            total_investments: self.portfolios.len(),
            intervention_type_distribution: type_counts,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct HumanStats {
    pub active_humans: usize,
    pub total_oracles: usize,
    pub total_bounties: usize,
    pub total_investments: usize,
    pub intervention_type_distribution: HashMap<String, usize>,
}
