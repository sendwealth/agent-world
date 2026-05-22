use std::collections::HashMap;
use serde::{Deserialize, Serialize};

/// A single phase within a tick trace
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseData {
    pub phase: String,
    pub input_data: serde_json::Value,
    pub output_data: serde_json::Value,
    pub duration_ms: f64,
    pub error: Option<String>,
}

/// Complete tick trace received from agent runtime
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TickTraceData {
    pub agent_id: String,
    pub tick: u64,
    pub phases: Vec<PhaseData>,
    pub started_at: String,
    pub finished_at: String,
    pub total_duration_ms: f64,
}

/// Lightweight summary for timeline views
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TickTraceSummary {
    pub agent_id: String,
    pub tick: u64,
    pub action: String,
    pub survival_mode: String,
    pub token_ratio: f64,
    pub duration_ms: f64,
    pub started_at: String,
    pub error: Option<String>,
}

impl TickTraceData {
    pub fn to_summary(&self) -> TickTraceSummary {
        let mut action = String::new();
        let mut survival_mode = String::new();
        let mut token_ratio = 0.0;
        let mut error = None;

        for p in &self.phases {
            match p.phase.as_str() {
                "act" => {
                    action = p.output_data.get("action_type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                }
                "survive" => {
                    survival_mode = p.output_data.get("mode")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                }
                "sense" => {
                    token_ratio = p.output_data.get("token_ratio")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0);
                }
                _ => {}
            }
            if p.error.is_some() && error.is_none() {
                error = p.error.clone();
            }
        }

        TickTraceSummary {
            agent_id: self.agent_id.clone(),
            tick: self.tick,
            action,
            survival_mode,
            token_ratio,
            duration_ms: self.total_duration_ms,
            started_at: self.started_at.clone(),
            error,
        }
    }
}

/// In-memory store for agent traces
pub struct TraceStore {
    traces: HashMap<String, Vec<TickTraceData>>,
}

impl Default for TraceStore {
    fn default() -> Self {
        Self::new()
    }
}

impl TraceStore {
    pub fn new() -> Self {
        Self {
            traces: HashMap::new(),
        }
    }

    pub fn save(&mut self, trace: TickTraceData) {
        let agent_id = trace.agent_id.clone();
        let entry = self.traces.entry(agent_id).or_default();
        // Insert in sorted order by tick
        let pos = entry.binary_search_by(|t| t.tick.cmp(&trace.tick));
        match pos {
            Ok(idx) => entry[idx] = trace, // update existing
            Err(idx) => entry.insert(idx, trace), // insert in order
        }
    }

    pub fn get_tick(&self, agent_id: &str, tick: u64) -> Option<&TickTraceData> {
        let traces = self.traces.get(agent_id)?;
        traces.binary_search_by(|t| t.tick.cmp(&tick)).ok().map(|i| &traces[i])
    }

    pub fn get_latest(&self, agent_id: &str) -> Option<&TickTraceData> {
        let traces = self.traces.get(agent_id)?;
        traces.last()
    }

    pub fn get_timeline(&self, agent_id: &str, limit: usize, offset: usize) -> Vec<TickTraceSummary> {
        let traces = self.traces.get(agent_id);
        match traces {
            Some(t) => t.iter()
                .rev()
                .skip(offset)
                .take(limit)
                .map(|t| t.to_summary())
                .collect(),
            None => Vec::new(),
        }
    }

    pub fn list_agents(&self) -> Vec<AgentTraceStats> {
        self.traces.iter().map(|(id, traces)| {
            AgentTraceStats {
                agent_id: id.clone(),
                total_ticks: traces.len(),
                latest_tick: traces.last().map(|t| t.tick).unwrap_or(0),
            }
        }).collect()
    }

    pub fn count_ticks(&self, agent_id: &str) -> usize {
        self.traces.get(agent_id).map(|t| t.len()).unwrap_or(0)
    }

    /// Returns all trace data, optionally filtered by agent IDs and tick range.
    pub fn get_all_traces(
        &self,
        agent_ids: Option<&[String]>,
        tick_range: Option<(u64, u64)>,
    ) -> Vec<&TickTraceData> {
        let mut result = Vec::new();
        for (id, traces) in &self.traces {
            if let Some(ids) = agent_ids {
                if !ids.contains(id) {
                    continue;
                }
            }
            for trace in traces {
                if let Some((from, to)) = tick_range {
                    if trace.tick < from || trace.tick > to {
                        continue;
                    }
                }
                result.push(trace);
            }
        }
        result
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTraceStats {
    pub agent_id: String,
    pub total_ticks: usize,
    pub latest_tick: u64,
}

// ---------------------------------------------------------------------------
// Dialect divergence data structures
// ---------------------------------------------------------------------------

/// A single dialect region detected from agent communication analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DialectRegionData {
    pub region_id: String,
    #[serde(default)]
    pub agent_ids: Vec<String>,
    #[serde(default)]
    pub signature_terms: Vec<String>,
    pub coherence: f64,
    pub isolation: f64,
}

/// A complete dialect divergence report submitted by the agent runtime.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DialectReportData {
    pub tick: u64,
    pub grouping_method: String,
    pub avg_inter_group_distance: f64,
    pub avg_intra_group_distance: f64,
    pub divergence_index: f64,
    /// Nested dict: group_a -> group_b -> distance
    #[serde(default)]
    pub distance_matrix: serde_json::Value,
    /// List of dialect regions
    #[serde(default)]
    pub regions: Vec<DialectRegionData>,
}

/// In-memory store for the latest dialect divergence report.
pub struct DialectStore {
    latest: Option<DialectReportData>,
    history: Vec<DialectReportData>,
}

impl Default for DialectStore {
    fn default() -> Self {
        Self::new()
    }
}

impl DialectStore {
    pub fn new() -> Self {
        Self {
            latest: None,
            history: Vec::new(),
        }
    }

    pub fn save(&mut self, report: DialectReportData) {
        self.latest = Some(report.clone());
        // Insert in sorted order by tick
        let pos = self.history.binary_search_by(|r| r.tick.cmp(&report.tick));
        match pos {
            Ok(idx) => self.history[idx] = report,
            Err(idx) => self.history.insert(idx, report),
        }
    }

    pub fn get_latest(&self) -> Option<&DialectReportData> {
        self.latest.as_ref()
    }

    pub fn get_by_tick(&self, tick: u64) -> Option<&DialectReportData> {
        self.history
            .binary_search_by(|r| r.tick.cmp(&tick))
            .ok()
            .map(|i| &self.history[i])
    }

    pub fn get_matrix(&self, tick: Option<u64>) -> serde_json::Value {
        let report = match tick {
            Some(t) => self.get_by_tick(t),
            None => self.get_latest(),
        };
        match report {
            Some(r) => serde_json::json!({
                "tick": r.tick,
                "method": "cosine",
                "distances": r.distance_matrix,
            }),
            None => serde_json::json!({
                "tick": 0,
                "method": "cosine",
                "distances": {},
            }),
        }
    }

    pub fn get_regions(&self, tick: Option<u64>) -> serde_json::Value {
        let report = match tick {
            Some(t) => self.get_by_tick(t),
            None => self.get_latest(),
        };
        match report {
            Some(r) => serde_json::json!({
                "tick": r.tick,
                "grouping_method": r.grouping_method,
                "regions": r.regions,
                "avg_inter_group_distance": r.avg_inter_group_distance,
                "avg_intra_group_distance": r.avg_intra_group_distance,
                "divergence_index": r.divergence_index,
            }),
            None => serde_json::json!({
                "tick": 0,
                "grouping_method": "region",
                "regions": [],
                "avg_inter_group_distance": 0.0,
                "avg_intra_group_distance": 0.0,
                "divergence_index": 0.0,
            }),
        }
    }

    pub fn get_timeline(&self, limit: usize, offset: usize) -> Vec<serde_json::Value> {
        self.history
            .iter()
            .rev()
            .skip(offset)
            .take(limit)
            .map(|r| {
                serde_json::json!({
                    "tick": r.tick,
                    "grouping_method": r.grouping_method,
                    "avg_inter_group_distance": r.avg_inter_group_distance,
                    "avg_intra_group_distance": r.avg_intra_group_distance,
                    "divergence_index": r.divergence_index,
                })
            })
            .collect()
    }
}
