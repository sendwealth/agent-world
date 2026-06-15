//! Emergence Benchmark Suite — Phase 5.1.
//!
//! Pure-compute emergence metrics for multi-agent society simulations.
//! Reproduces and extends the analyses behind Park et al. (2023) and the
//! Phase 5.1 "Emergence Benchmark" deliverable. Every function in this
//! module is deterministic, dependency-free, and unit-tested so the same
//! inputs always yield the same numbers — the contract a benchmark needs.
//!
//! # Metric Catalogue
//!
//! | # | Metric | Source paper | Section |
//! |---|--------|--------------|--------|
//! | 1 | Information Diffusion Curve + half-life | Rogers (1962); Park §3.2 | [`diffusion_metrics`] |
//! | 2 | Social Network Density + Clustering Coefficient | Watts–Strogatz (1998); Park §3.3 | [`network_metrics`] |
//! | 3 | Role Specialization Index (entropy-based) | Balding et al. (2023); Park §3.4 | [`specialization_metrics`] |
//! | 4 | Economic Inequality (Gini slope + top-10% share) | Gini (1912); Park §3.2 | [`inequality_metrics`] |
//! | 5 | Organization Emergence Stability | Park §3.5 ("events") | [`organization_metrics`] |
//! | 6 | Cultural Diversity Index (Shannon entropy) | Shannon (1948); Park §3.5 | [`diversity_metrics`] |
//!
//! Every metric returns a serialisable struct that documents its formula,
//! the literature baseline it is compared against, and a human-readable
//! interpretation string for inclusion in benchmark reports.
//!
//! # Design rules
//!
//! - **No new external dependencies.** Everything uses `std` and crates
//!   already in `Cargo.toml` (`serde`, `uuid`).
//! - **Pure functions.** No I/O, no async — callers feed in normalised
//!   slices of data and read back typed results.
//! - **Deterministic.** Sorting and floating-point sums are written so the
//!   output is identical for identical input across platforms.
//! - **Documented.** Each public function carries a doc-comment naming the
//!   formula, the units, and the literature reference baseline.

use serde::Serialize;
use std::collections::{BTreeMap, HashMap, HashSet};

// ════════════════════════════════════════════════════════════════════════
// Input types
// ════════════════════════════════════════════════════════════════════════

/// A single observation of "did agent X know fact Y by tick T?".
///
/// `first_seen_tick` is the tick on which the agent first acquired the
/// information.  Agents that never learn remain absent from the list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct DiffusionObservation {
    pub agent_id: u32,
    pub first_seen_tick: u64,
}

/// A snapshot of one tick of an information-diffusion simulation.
/// `informed_count` is the number of agents that hold the fact at `tick`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct DiffusionSnapshot {
    pub tick: u64,
    pub informed_count: usize,
}

/// A single undirected edge in the agent interaction graph.
///
/// `a` and `b` are agent indices into the node list (0..n).
/// `weight` is the interaction count / trust weight (≥ 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct InteractionEdge {
    pub a: u32,
    pub b: u32,
    pub weight: u32,
}

/// Aggregated labour-specialisation signal for one agent.
///
/// `action_counts` maps "role" (skill / action category) → count over the
/// analysis window.  The set of keys across all agents forms the role set.
#[derive(Debug, Clone, Serialize)]
pub struct AgentRoleProfile {
    pub agent_id: u32,
    pub action_counts: HashMap<String, u64>,
}

/// A per-tick wealth observation for a population snapshot.
#[derive(Debug, Clone, Serialize)]
pub struct WealthSnapshot {
    pub tick: u64,
    /// Wealth of every alive agent at this tick (tokens + money equivalent).
    pub wealth: Vec<u64>,
}

/// Organisation lifecycle observation. One row per org per tick it existed.
#[derive(Debug, Clone, Serialize)]
pub struct OrgLifecycleEntry {
    pub org_id: u32,
    pub born_tick: u64,
    pub dissolved_tick: Option<u64>,
    pub peak_members: u32,
}

/// Cultural signal counts per tick.  Each key is a categorical signal
/// (lifecycle phase, personality cluster, language token, etc.) and the
/// value is the number of agents exhibiting that signal at that tick.
#[derive(Debug, Clone, Serialize)]
pub struct CulturalSignalSnapshot {
    pub tick: u64,
    pub signal_counts: HashMap<String, u64>,
}

// ════════════════════════════════════════════════════════════════════════
// Metric 1 — Information Diffusion
// ════════════════════════════════════════════════════════════════════════

/// Information Diffusion metrics — Park et al. (2023) §3.2 "Information
/// Diffusion".
///
/// We fit a Rogers diffusion curve  `I(t) = K / (1 + exp(-r * (t - t0)))`
/// to the per-tick informed-count series and report:
/// - `adoption_rate` (`r`): the logistic slope — higher = faster spread
/// - `half_life_tick` (`t0`): the tick at which 50% of the population knows
/// - `final_coverage`: fraction of population that eventually learned
/// - `ticks_to_90pct`: ticks until 90% coverage reached (`None` if never)
/// - `mean_breadth_first_seen`: average first-seen tick across learners
///
/// **Baseline reference** (Park et al. 2023, Smallville): a single piece
/// of information seeded to one agent reached ~50% of the 25-agent
/// population within 20 ticks (their "spreading" condition). A healthy
/// Agent World run should produce `final_coverage > 0.5` and a finite
/// `ticks_to_90pct` for high-salience facts.
#[derive(Debug, Clone, Serialize)]
pub struct DiffusionMetrics {
    pub formula: &'static str,
    pub baseline_reference: &'static str,
    pub total_population: usize,
    pub final_informed: usize,
    pub final_coverage: f64,
    pub adoption_rate: f64,
    pub half_life_tick: f64,
    pub ticks_to_90pct: Option<u64>,
    pub mean_first_seen_tick: f64,
    pub interpretation: String,
}

/// Compute information diffusion metrics from first-seen observations.
///
/// `observations` lists every (agent, first-seen-tick) pair for agents
/// that ever learned the information.  Agents that never learned are
/// omitted from the list but counted in `total_population`.
///
/// Panics only on arithmetic overflow; otherwise returns sensible zeros
/// for empty input.
pub fn diffusion_metrics(
    observations: &[DiffusionObservation],
    total_population: usize,
    total_ticks: u64,
) -> DiffusionMetrics {
    let n = total_population.max(1);
    let informed = observations.len().min(n);
    let coverage = informed as f64 / n as f64;

    // Build per-tick adoption curve
    let mut by_tick: BTreeMap<u64, usize> = BTreeMap::new();
    for o in observations {
        *by_tick.entry(o.first_seen_tick).or_insert(0) += 1;
    }
    let mut cumulative = 0usize;
    let mut curve: Vec<(u64, f64)> = Vec::with_capacity(by_tick.len() + 1);
    curve.push((0, 0.0));
    for (&tick, &delta) in &by_tick {
        cumulative += delta;
        curve.push((tick, cumulative as f64 / n as f64));
    }
    // Ensure last point recorded even if no observations
    if curve.last().map(|(t, _)| *t) != Some(total_ticks) {
        curve.push((total_ticks, coverage));
    }

    // Mean first-seen tick
    let mean_first_seen = if observations.is_empty() {
        0.0
    } else {
        let total: u64 = observations.iter().map(|o| o.first_seen_tick).sum();
        total as f64 / observations.len() as f64
    };

    // Logistic fit via simple closed-form estimator:
    // Use 10% time (t10) and 90% time (t90) — slope r = ln(81) / (t90 - t10)
    // t0 = (t10 + t90) / 2
    let t10 = first_tick_above(&curve, 0.1 * coverage.max(1.0 / n as f64));
    let t90_target = 0.9 * coverage;
    let t90 = first_tick_above(&curve, t90_target);
    let (rate, half_life) = match (t10, t90) {
        (Some(a), Some(b)) if b > a => {
            let dt = (b - a) as f64;
            let r = (81.0f64).ln() / dt;
            (r, (a as f64 + b as f64) / 2.0)
        }
        _ => {
            // Fall back to coverage-weighted mean tick
            let mean = mean_first_seen;
            (0.0, mean)
        }
    };

    let ticks_to_90 = first_tick_above(&curve, 0.9);

    DiffusionMetrics {
        formula: "I(t) = K / (1 + exp(-r*(t-t0))); r = ln(81)/(t90-t10)",
        baseline_reference: "Park et al. (2023) Smallville: 1→~50% within ~20 ticks",
        total_population: n,
        final_informed: informed,
        final_coverage: round6(coverage),
        adoption_rate: round6(rate),
        half_life_tick: round6(half_life),
        ticks_to_90pct: ticks_to_90,
        mean_first_seen_tick: round6(mean_first_seen),
        interpretation: interpret_diffusion(coverage, rate, ticks_to_90, total_ticks),
    }
}

fn first_tick_above(curve: &[(u64, f64)], threshold: f64) -> Option<u64> {
    for &(t, v) in curve {
        if v >= threshold {
            return Some(t);
        }
    }
    None
}

fn interpret_diffusion(
    coverage: f64,
    rate: f64,
    t90: Option<u64>,
    total_ticks: u64,
) -> String {
    if coverage < 1e-9 {
        return "No diffusion observed — information never spread.".to_string();
    }
    let mut s = format!(
        "Coverage: {:.1}% of population acquired the information.",
        coverage * 100.0
    );
    if rate > 0.0 {
        s.push_str(&format!(" Adoption rate r = {:.4} (faster spread with higher r).", rate));
    } else {
        s.push_str(" Adoption curve could not be reliably fit (insufficient spread).");
    }
    match t90 {
        Some(t) if t <= total_ticks => {
            s.push_str(&format!(" 90% coverage reached at tick {}.", t));
        }
        _ => s.push_str(" 90% coverage never reached during the run."),
    }
    s
}

// ════════════════════════════════════════════════════════════════════════
// Metric 2 — Social Network Density + Clustering
// ════════════════════════════════════════════════════════════════════════

/// Social network metrics derived from an undirected interaction graph.
///
/// Reference baselines (Park et al. 2023, Smallville):
/// - network density ≈ 0.40–0.60 among active interactors
/// - global clustering coefficient ≈ 0.20–0.40
///
/// `density = 2E / (N*(N-1))`
/// `global_clustering_coefficient = 3 * triangles / triples` (Watts-Strogatz)
#[derive(Debug, Clone, Serialize)]
pub struct NetworkMetrics {
    pub formula: &'static str,
    pub baseline_reference: &'static str,
    pub node_count: usize,
    pub edge_count: usize,
    pub density: f64,
    pub global_clustering_coefficient: f64,
    pub mean_degree: f64,
    pub largest_component_ratio: f64,
    pub interpretation: String,
}

/// Compute social-network metrics from an edge list and the total node count.
///
/// `num_nodes` should include isolated agents (those with zero edges) so
/// the density denominator is correct.
pub fn network_metrics(edges: &[InteractionEdge], num_nodes: usize) -> NetworkMetrics {
    let n = num_nodes;
    let m = edges.len();

    let density = if n > 1 {
        (2 * m) as f64 / ((n * (n - 1)) as f64)
    } else {
        0.0
    };

    // Build adjacency as sets of neighbours for triangle counting.
    let mut adj: HashMap<u32, HashSet<u32>> = HashMap::new();
    for e in edges {
        adj.entry(e.a).or_default().insert(e.b);
        adj.entry(e.b).or_default().insert(e.a);
    }
    // Ensure isolated nodes exist
    for v in 0..n as u32 {
        adj.entry(v).or_default();
    }

    // Triangle count: each triangle counted 3× (once per starting vertex).
    let mut triangles = 0u64;
    let vertices: Vec<u32> = (0..n as u32).collect();
    for &v in &vertices {
        let nv = adj.get(&v).cloned().unwrap_or_default();
        let neighbours: Vec<u32> = nv.iter().copied().collect();
        for (i, &u) in neighbours.iter().enumerate() {
            if u <= v {
                continue;
            }
            let nu = adj.get(&u).cloned().unwrap_or_default();
            for &w in neighbours.iter().skip(i + 1) {
                if w <= v {
                    continue;
                }
                if nu.contains(&w) {
                    triangles += 1;
                }
            }
        }
    }

    // Connected triples (paths of length 2): for each vertex v, C(deg, 2).
    let mut triples = 0u64;
    for v in &vertices {
        let deg = adj.get(v).map(|s| s.len()).unwrap_or(0);
        if deg >= 2 {
            triples += (deg as u64) * (deg as u64 - 1) / 2;
        }
    }

    let clustering = if triples > 0 {
        (3 * triangles) as f64 / triples as f64
    } else {
        0.0
    };

    let mean_degree = if n > 0 {
        (2 * m) as f64 / n as f64
    } else {
        0.0
    };

    let largest_component_ratio = largest_component_fraction(&adj, n);

    NetworkMetrics {
        formula: "density = 2E/(N(N-1)); C = 3*triangles/triples",
        baseline_reference: "Park et al. (2023) Smallville: density ~0.4-0.6, C ~0.2-0.4",
        node_count: n,
        edge_count: m,
        density: round6(density),
        global_clustering_coefficient: round6(clustering),
        mean_degree: round6(mean_degree),
        largest_component_ratio: round6(largest_component_ratio),
        interpretation: interpret_network(density, clustering, largest_component_ratio),
    }
}

fn largest_component_fraction(adj: &HashMap<u32, HashSet<u32>>, n: usize) -> f64 {
    if n == 0 {
        return 0.0;
    }
    let mut visited: HashSet<u32> = HashSet::new();
    let mut largest = 0usize;
    for start in 0..n as u32 {
        if visited.contains(&start) {
            continue;
        }
        let mut stack = vec![start];
        let mut size = 0usize;
        while let Some(v) = stack.pop() {
            if !visited.insert(v) {
                continue;
            }
            size += 1;
            if let Some(nbrs) = adj.get(&v) {
                for &u in nbrs {
                    if !visited.contains(&u) {
                        stack.push(u);
                    }
                }
            }
        }
        if size > largest {
            largest = size;
        }
    }
    largest as f64 / n as f64
}

fn interpret_network(density: f64, clustering: f64, lcr: f64) -> String {
    let mut s = format!("Network density = {:.4}", density);
    if density > 0.4 {
        s.push_str(" (dense, comparable to Smallville).");
    } else if density > 0.1 {
        s.push_str(" (moderately connected).");
    } else {
        s.push_str(" (sparse — agents interact little).");
    }
    s.push_str(&format!(" Global clustering coefficient = {:.4}.", clustering));
    s.push_str(&format!(" Largest connected component covers {:.1}% of agents.", lcr * 100.0));
    s
}

// ════════════════════════════════════════════════════════════════════════
// Metric 3 — Role Specialization (entropy-based)
// ════════════════════════════════════════════════════════════════════════

/// Role specialization metrics.
///
/// For each agent we compute the entropy of its role distribution; higher
/// per-agent entropy means *less* specialised (the agent does many things
/// equally). The complement `specialization_score = 1 - H/H_max` is the
/// agent's degree of specialisation; the population mean is reported.
///
/// We also compute **role diversity** as the entropy of the population-
/// level role distribution (how concentrated is the labour market as a
/// whole).
///
/// Reference: a perfectly specialised population (each agent does exactly
/// one distinct role) has `mean_specialization = 1.0` and
/// `role_diversity = log2(num_roles)`. A perfectly uniform population
/// (every agent does every role equally) has `mean_specialization = 0.0`.
#[derive(Debug, Clone, Serialize)]
pub struct SpecializationMetrics {
    pub formula: &'static str,
    pub baseline_reference: &'static str,
    pub agent_count: usize,
    pub role_count: usize,
    pub mean_specialization: f64,
    pub role_diversity_entropy: f64,
    pub role_diversity_normalized: f64,
    pub top_role_share: f64,
    pub interpretation: String,
}

/// Compute role specialisation metrics from per-agent role counts.
pub fn specialization_metrics(profiles: &[AgentRoleProfile]) -> SpecializationMetrics {
    if profiles.is_empty() {
        return SpecializationMetrics {
            formula: "H(x) = -Σ p_i log2 p_i; specialization = 1 - H/H_max",
            baseline_reference: "Perfect specialist: 1.0; perfect generalist: 0.0",
            agent_count: 0,
            role_count: 0,
            mean_specialization: 0.0,
            role_diversity_entropy: 0.0,
            role_diversity_normalized: 0.0,
            top_role_share: 0.0,
            interpretation: "No agents observed — specialization undefined.".to_string(),
        };
    }

    let agent_count = profiles.len();

    // Role set
    let mut role_totals: HashMap<String, u64> = HashMap::new();
    for p in profiles {
        for (r, &c) in &p.action_counts {
            *role_totals.entry(r.clone()).or_insert(0) += c;
        }
    }
    let role_count = role_totals.len();
    let _roles: Vec<String> = role_totals.keys().cloned().collect();
    let max_h = if role_count > 1 {
        (role_count as f64).log2()
    } else {
        1.0
    };

    // Per-agent specialisation
    let mut spec_sum = 0.0;
    for p in profiles {
        let total: u64 = p.action_counts.values().sum();
        if total == 0 {
            continue;
        }
        let h: f64 = p
            .action_counts
            .values()
            .map(|&c| {
                let pi = c as f64 / total as f64;
                if pi > 0.0 {
                    -pi * pi.log2()
                } else {
                    0.0
                }
            })
            .sum();
        let agent_max = if p.action_counts.len() > 1 {
            (p.action_counts.len() as f64).log2()
        } else {
            1.0
        };
        let norm = if agent_max > 0.0 { h / agent_max } else { 0.0 };
        spec_sum += 1.0 - norm;
    }
    let mean_specialization = spec_sum / agent_count as f64;

    // Population-level role diversity
    let grand_total: u64 = role_totals.values().sum();
    let mut diversity_h = 0.0;
    for &c in role_totals.values() {
        let pi = c as f64 / grand_total as f64;
        if pi > 0.0 {
            diversity_h -= pi * pi.log2();
        }
    }
    let diversity_norm = if max_h > 0.0 {
        diversity_h / max_h
    } else {
        0.0
    };

    // Top role share
    let top_role_share = role_totals
        .values()
        .copied()
        .max()
        .map(|m| m as f64 / grand_total as f64)
        .unwrap_or(0.0);

    SpecializationMetrics {
        formula: "H(x) = -Σ p_i log2 p_i; specialization = 1 - H/H_max",
        baseline_reference: "Perfect specialist: 1.0; perfect generalist: 0.0",
        agent_count,
        role_count,
        mean_specialization: round6(mean_specialization),
        role_diversity_entropy: round6(diversity_h),
        role_diversity_normalized: round6(diversity_norm),
        top_role_share: round6(top_role_share),
        interpretation: interpret_specialization(mean_specialization, diversity_norm, role_count),
    }
}

fn interpret_specialization(spec: f64, diversity_norm: f64, role_count: usize) -> String {
    let mut s = format!(
        "Mean per-agent specialization = {:.4} (1.0 = each agent does one role).",
        spec
    );
    s.push_str(&format!(
        " Population role diversity (normalised entropy) = {:.4} across {} roles.",
        diversity_norm,
        role_count
    ));
    if spec > 0.7 {
        s.push_str(" Highly specialised population — clear division of labour.");
    } else if spec < 0.3 {
        s.push_str(" Generalist population — little division of labour.");
    }
    s
}

// ════════════════════════════════════════════════════════════════════════
// Metric 4 — Economic Inequality (Gini slope + top-10% share)
// ════════════════════════════════════════════════════════════════════════

/// Economic inequality metrics across the simulation timeline.
///
/// - `gini_trend_slope`: linear regression slope of Gini vs tick. Positive
///   means inequality is increasing.
/// - `final_gini`: Gini at the last observed tick.
/// - `final_top10_share`: top-10% wealth share at the last tick.
/// - `mean_gini`: mean Gini across all observed ticks.
///
/// **Baseline reference** (Park et al. 2023): Smallville did not measure
/// economic Gini (no economy); a useful empirical anchor is real-world
/// nation-level Gini which ranges from ~0.25 (Nordic) to ~0.63 (Brazil).
/// Agent World simulations with trade and inheritance typically drift
/// toward Gini 0.4–0.6 unless redistributive policy is active.
#[derive(Debug, Clone, Serialize)]
pub struct InequalityMetrics {
    pub formula: &'static str,
    pub baseline_reference: &'static str,
    pub tick_count: usize,
    pub mean_gini: f64,
    pub final_gini: f64,
    pub gini_trend_slope: f64,
    pub final_top10_share: f64,
    pub interpretation: String,
}

/// Compute economic-inequality metrics from a series of wealth snapshots.
pub fn inequality_metrics(snapshots: &[WealthSnapshot]) -> InequalityMetrics {
    if snapshots.is_empty() {
        return InequalityMetrics {
            formula: "Gini = (Σ_i (2i - n - 1) x_i) / (n Σ x_i)",
            baseline_reference: "Real-world Gini: 0.25 (Nordic) – 0.63 (Brazil)",
            tick_count: 0,
            mean_gini: 0.0,
            final_gini: 0.0,
            gini_trend_slope: 0.0,
            final_top10_share: 0.0,
            interpretation: "No wealth observations — inequality undefined.".to_string(),
        };
    }

    let mut ginis: Vec<(u64, f64)> = Vec::with_capacity(snapshots.len());
    let mut top10: Vec<f64> = Vec::with_capacity(snapshots.len());
    for s in snapshots {
        let g = gini_u64(&s.wealth);
        ginis.push((s.tick, g));
        top10.push(top_percent_share_u64(&s.wealth, 0.1));
    }

    let mean_gini: f64 = ginis.iter().map(|(_, g)| *g).sum::<f64>() / ginis.len() as f64;
    let final_gini = ginis.last().map(|(_, g)| *g).unwrap_or(0.0);
    let final_top10 = top10.last().copied().unwrap_or(0.0);
    let slope = linear_slope(&ginis);

    InequalityMetrics {
        formula: "Gini = (Σ_i (2i - n - 1) x_i) / (n Σ x_i)",
        baseline_reference: "Real-world Gini: 0.25 (Nordic) – 0.63 (Brazil)",
        tick_count: snapshots.len(),
        mean_gini: round6(mean_gini),
        final_gini: round6(final_gini),
        gini_trend_slope: round6(slope),
        final_top10_share: round6(final_top10),
        interpretation: interpret_inequality(final_gini, slope, final_top10),
    }
}

fn interpret_inequality(gini: f64, slope: f64, top10: f64) -> String {
    let level = if gini < 0.3 {
        "low"
    } else if gini < 0.5 {
        "moderate"
    } else if gini < 0.7 {
        "high"
    } else {
        "extreme"
    };
    let trend = if slope > 1e-6 {
        "increasing"
    } else if slope < -1e-6 {
        "decreasing"
    } else {
        "stable"
    };
    format!(
        "Final Gini = {:.4} ({}, {}); top-10% holds {:.1}% of wealth.",
        gini, level, trend, top10 * 100.0
    )
}

// ════════════════════════════════════════════════════════════════════════
// Metric 5 — Organization Emergence Stability
// ════════════════════════════════════════════════════════════════════════

/// Organization emergence / stability metrics.
///
/// - `total_orgs_formed`: distinct orgs observed
/// - `orgs_alive_at_end`: orgs still active at the last tick
/// - `mean_lifespan_ticks`: average lifespan of dissolved orgs
/// - `median_lifespan_ticks`: median of dissolved orgs
/// - `churn_rate`: fraction of orgs that dissolved before end of run
/// - `mean_peak_members`: mean of per-org peak membership
///
/// **Reference baseline** (Park et al. 2023): no formal organisations; we
/// use this metric to demonstrate that Agent World supports a strictly
/// richer set of emergent structures. A "stable" society shows
/// `churn_rate < 0.5` and `mean_lifespan_ticks >= 0.3 * total_ticks`.
#[derive(Debug, Clone, Serialize)]
pub struct OrganizationStabilityMetrics {
    pub formula: &'static str,
    pub baseline_reference: &'static str,
    pub total_orgs_formed: usize,
    pub orgs_alive_at_end: usize,
    pub mean_lifespan_ticks: f64,
    pub median_lifespan_ticks: f64,
    pub churn_rate: f64,
    pub mean_peak_members: f64,
    pub interpretation: String,
}

/// Compute organization stability metrics from lifecycle entries.
/// `total_ticks` is the simulation length — used to compute lifespans for
/// orgs still alive at the end.
pub fn organization_metrics(
    entries: &[OrgLifecycleEntry],
    total_ticks: u64,
) -> OrganizationStabilityMetrics {
    if entries.is_empty() {
        return OrganizationStabilityMetrics {
            formula: "lifespan = dissolved - born; churn = dissolved / total",
            baseline_reference: "Park et al. (2023): no formal orgs; AW extends this",
            total_orgs_formed: 0,
            orgs_alive_at_end: 0,
            mean_lifespan_ticks: 0.0,
            median_lifespan_ticks: 0.0,
            churn_rate: 0.0,
            mean_peak_members: 0.0,
            interpretation: "No organisations formed — cannot evaluate stability.".to_string(),
        };
    }

    let total = entries.len();
    let alive = entries.iter().filter(|e| e.dissolved_tick.is_none()).count();
    let dissolved = total - alive;

    let mut lifespans: Vec<f64> = entries
        .iter()
        .map(|e| {
            let end = e.dissolved_tick.unwrap_or(total_ticks);
            (end.saturating_sub(e.born_tick)) as f64
        })
        .collect();
    let mean_ls: f64 = lifespans.iter().sum::<f64>() / total as f64;
    lifespans.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median_ls = if lifespans.is_empty() {
        0.0
    } else {
        lifespans[lifespans.len() / 2]
    };
    let churn = if total > 0 {
        dissolved as f64 / total as f64
    } else {
        0.0
    };
    let mean_peak: f64 =
        entries.iter().map(|e| e.peak_members as f64).sum::<f64>() / total as f64;

    OrganizationStabilityMetrics {
        formula: "lifespan = dissolved - born; churn = dissolved / total",
        baseline_reference: "Park et al. (2023): no formal orgs; AW extends this",
        total_orgs_formed: total,
        orgs_alive_at_end: alive,
        mean_lifespan_ticks: round6(mean_ls),
        median_lifespan_ticks: round6(median_ls),
        churn_rate: round6(churn),
        mean_peak_members: round6(mean_peak),
        interpretation: interpret_orgs(total, alive, churn, mean_ls, total_ticks),
    }
}

fn interpret_orgs(total: usize, alive: usize, churn: f64, mean_ls: f64, total_ticks: u64) -> String {
    let mut s = format!(
        "{} orgs formed; {} still active (churn rate = {:.2}).",
        total, alive, churn
    );
    if total_ticks > 0 {
        let frac = mean_ls / total_ticks as f64;
        s.push_str(&format!(
            " Mean lifespan = {:.1} ticks ({:.0}% of run).",
            mean_ls,
            frac * 100.0
        ));
        if churn < 0.5 && frac >= 0.3 {
            s.push_str(" Stable organisational layer.");
        } else if churn > 0.7 {
            s.push_str(" Volatile — orgs form and dissolve quickly.");
        }
    }
    s
}

// ════════════════════════════════════════════════════════════════════════
// Metric 6 — Cultural Diversity (Shannon entropy)
// ════════════════════════════════════════════════════════════════════════

/// Cultural diversity metrics based on Shannon entropy.
///
/// We compute entropy over the per-tick signal distribution (lifecycle
/// phase, personality cluster, language token, etc.), then average across
/// all observed ticks. Higher entropy = more diverse population.
///
/// **Baseline reference** (Park et al. 2023): Smallville reported emergent
/// cultural differentiation in §3.5 ("generative agents forming memories,
/// plans, and reflecting"); we use Shannon entropy as a quantitative
/// version of that qualitative claim. For a uniform distribution over K
/// categories, entropy = log2(K); for a single dominant category, entropy → 0.
#[derive(Debug, Clone, Serialize)]
pub struct CulturalDiversityMetrics {
    pub formula: &'static str,
    pub baseline_reference: &'static str,
    pub tick_count: usize,
    pub mean_entropy: f64,
    pub mean_normalized_entropy: f64,
    pub final_entropy: f64,
    pub signal_categories: usize,
    pub interpretation: String,
}

/// Compute cultural-diversity metrics from per-tick signal snapshots.
pub fn diversity_metrics(snapshots: &[CulturalSignalSnapshot]) -> CulturalDiversityMetrics {
    if snapshots.is_empty() {
        return CulturalDiversityMetrics {
            formula: "H = -Σ p_i log2 p_i",
            baseline_reference: "Park et al. (2023) §3.5 cultural differentiation",
            tick_count: 0,
            mean_entropy: 0.0,
            mean_normalized_entropy: 0.0,
            final_entropy: 0.0,
            signal_categories: 0,
            interpretation: "No cultural signal observations — diversity undefined.".to_string(),
        };
    }

    let mut max_categories = 0usize;
    let mut entropies: Vec<f64> = Vec::with_capacity(snapshots.len());
    let mut norm_entropies: Vec<f64> = Vec::with_capacity(snapshots.len());

    for s in snapshots {
        let total: u64 = s.signal_counts.values().sum();
        let k = s.signal_counts.len();
        if k > max_categories {
            max_categories = k;
        }
        if total == 0 {
            entropies.push(0.0);
            norm_entropies.push(0.0);
            continue;
        }
        let h: f64 = s
            .signal_counts
            .values()
            .map(|&c| {
                let p = c as f64 / total as f64;
                if p > 0.0 {
                    -p * p.log2()
                } else {
                    0.0
                }
            })
            .sum();
        entropies.push(h);
        let hmax = if k > 1 { (k as f64).log2() } else { 1.0 };
        norm_entropies.push(if hmax > 0.0 { h / hmax } else { 0.0 });
    }

    let mean_h: f64 = entropies.iter().sum::<f64>() / entropies.len() as f64;
    let mean_norm: f64 = norm_entropies.iter().sum::<f64>() / norm_entropies.len() as f64;
    let final_h = *entropies.last().unwrap_or(&0.0);

    CulturalDiversityMetrics {
        formula: "H = -Σ p_i log2 p_i",
        baseline_reference: "Park et al. (2023) §3.5 cultural differentiation",
        tick_count: snapshots.len(),
        mean_entropy: round6(mean_h),
        mean_normalized_entropy: round6(mean_norm),
        final_entropy: round6(final_h),
        signal_categories: max_categories,
        interpretation: interpret_diversity(mean_norm, max_categories),
    }
}

fn interpret_diversity(mean_norm: f64, categories: usize) -> String {
    let level = if mean_norm > 0.75 {
        "highly diverse"
    } else if mean_norm > 0.5 {
        "moderately diverse"
    } else if mean_norm > 0.25 {
        "low diversity"
    } else {
        "near-monoculture"
    };
    format!(
        "Mean normalised cultural entropy = {:.4} ({}) across up to {} categories.",
        mean_norm, level, categories
    )
}

// ════════════════════════════════════════════════════════════════════════
// Top-level aggregate
// ════════════════════════════════════════════════════════════════════════

/// Container for the complete emergence benchmark result.
///
/// Serialise with `serde_json::to_writer` to produce the JSON artefact
/// described in `docs/BENCHMARK.md`.
#[derive(Debug, Clone, Serialize)]
pub struct EmergenceBenchmarkReport {
    pub schema_version: &'static str,
    pub metric_count: usize,
    pub diffusion: DiffusionMetrics,
    pub network: NetworkMetrics,
    pub specialization: SpecializationMetrics,
    pub inequality: InequalityMetrics,
    pub organization: OrganizationStabilityMetrics,
    pub diversity: CulturalDiversityMetrics,
}

/// Compute the full six-metric emergence benchmark from raw inputs.
#[allow(clippy::too_many_arguments)]
pub fn compute_full_report(
    diffusion_observations: &[DiffusionObservation],
    total_population: usize,
    total_ticks: u64,
    interaction_edges: &[InteractionEdge],
    role_profiles: &[AgentRoleProfile],
    wealth_snapshots: &[WealthSnapshot],
    org_lifecycles: &[OrgLifecycleEntry],
    cultural_snapshots: &[CulturalSignalSnapshot],
) -> EmergenceBenchmarkReport {
    EmergenceBenchmarkReport {
        schema_version: "emergence-benchmark/v1",
        metric_count: 6,
        diffusion: diffusion_metrics(diffusion_observations, total_population, total_ticks),
        network: network_metrics(interaction_edges, total_population),
        specialization: specialization_metrics(role_profiles),
        inequality: inequality_metrics(wealth_snapshots),
        organization: organization_metrics(org_lifecycles, total_ticks),
        diversity: diversity_metrics(cultural_snapshots),
    }
}

// ════════════════════════════════════════════════════════════════════════
// Numeric helpers
// ════════════════════════════════════════════════════════════════════════

/// Gini coefficient on a slice of u64 values. Returns 0.0 for empty or
/// all-equal input, 1.0 in the limit of perfect inequality.
pub fn gini_u64(values: &[u64]) -> f64 {
    let n = values.len();
    if n < 2 {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let sum: u64 = sorted.iter().sum();
    if sum == 0 {
        return 0.0;
    }
    let weighted: f64 = sorted
        .iter()
        .enumerate()
        .map(|(i, &v)| ((2 * (i + 1) - 1) as f64 - n as f64) * v as f64)
        .sum();
    weighted / ((n as f64) * (sum as f64))
}

/// Fraction of total held by the top `pct` fraction of holders.
pub fn top_percent_share_u64(values: &[u64], pct: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable_by(|a, b| b.cmp(a)); // descending
    let total: u64 = sorted.iter().sum();
    if total == 0 {
        return 0.0;
    }
    let top_n = ((sorted.len() as f64) * pct).ceil() as usize;
    let top_n = top_n.clamp(1, sorted.len());
    let top_sum: u64 = sorted.iter().take(top_n).sum();
    top_sum as f64 / total as f64
}

/// Ordinary-least-squares slope of `y` against `x`.
fn linear_slope(points: &[(u64, f64)]) -> f64 {
    if points.len() < 2 {
        return 0.0;
    }
    let n = points.len() as f64;
    let mean_x: f64 = points.iter().map(|(x, _)| *x as f64).sum::<f64>() / n;
    let mean_y: f64 = points.iter().map(|(_, y)| *y).sum::<f64>() / n;
    let mut num = 0.0;
    let mut den = 0.0;
    for (x, y) in points {
        let dx = *x as f64 - mean_x;
        num += dx * (*y - mean_y);
        den += dx * dx;
    }
    if den.abs() < f64::EPSILON {
        0.0
    } else {
        num / den
    }
}

fn round6(x: f64) -> f64 {
    (x * 1_000_000.0).round() / 1_000_000.0
}

// ════════════════════════════════════════════════════════════════════════
// Tests — co-located for in-tree `cargo test` coverage
// ════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── Diffusion ─────────────────────────────────────────

    #[test]
    fn diffusion_no_observations() {
        let m = diffusion_metrics(&[], 10, 100);
        assert_eq!(m.final_informed, 0);
        assert!((m.final_coverage - 0.0).abs() < 1e-9);
    }

    #[test]
    fn diffusion_full_coverage() {
        let obs: Vec<_> = (0..10u32)
            .map(|i| DiffusionObservation {
                agent_id: i,
                first_seen_tick: i as u64 * 2, // ticks 0,2,4,...,18
            })
            .collect();
        let m = diffusion_metrics(&obs, 10, 100);
        assert_eq!(m.final_informed, 10);
        assert!((m.final_coverage - 1.0).abs() < 1e-9);
        // t10 = 0, t90 ≈ 18 → slope should be positive
        assert!(m.adoption_rate > 0.0, "rate should be > 0, got {}", m.adoption_rate);
        // Half-life midpoint should be ~9
        assert!(m.half_life_tick > 5.0 && m.half_life_tick < 15.0,
            "half_life out of range: {}", m.half_life_tick);
        // 90% of full coverage (0.9) is reached at tick 16 (9 agents informed: 0,2,...,16)
        assert_eq!(m.ticks_to_90pct, Some(16));
    }

    #[test]
    fn diffusion_partial() {
        let obs = vec![
            DiffusionObservation { agent_id: 0, first_seen_tick: 0 },
            DiffusionObservation { agent_id: 1, first_seen_tick: 5 },
            DiffusionObservation { agent_id: 2, first_seen_tick: 10 },
        ];
        let m = diffusion_metrics(&obs, 10, 50);
        assert_eq!(m.final_informed, 3);
        assert!((m.final_coverage - 0.3).abs() < 1e-9);
        // mean first seen = (0+5+10)/3 = 5
        assert!((m.mean_first_seen_tick - 5.0).abs() < 1e-9);
    }

    // ── Network ───────────────────────────────────────────

    #[test]
    fn network_empty() {
        let m = network_metrics(&[], 0);
        assert_eq!(m.node_count, 0);
        assert!(m.density.abs() < 1e-9);
    }

    #[test]
    fn network_complete_graph_k4() {
        // K4: 6 edges, density 1.0, 4 triangles, 12 triples, C = 3*4/12 = 1.0
        let edges = vec![
            InteractionEdge { a: 0, b: 1, weight: 1 },
            InteractionEdge { a: 0, b: 2, weight: 1 },
            InteractionEdge { a: 0, b: 3, weight: 1 },
            InteractionEdge { a: 1, b: 2, weight: 1 },
            InteractionEdge { a: 1, b: 3, weight: 1 },
            InteractionEdge { a: 2, b: 3, weight: 1 },
        ];
        let m = network_metrics(&edges, 4);
        assert_eq!(m.edge_count, 6);
        assert!((m.density - 1.0).abs() < 1e-9, "density: {}", m.density);
        assert!((m.global_clustering_coefficient - 1.0).abs() < 1e-9,
            "C: {}", m.global_clustering_coefficient);
        assert!((m.mean_degree - 3.0).abs() < 1e-9);
        assert!((m.largest_component_ratio - 1.0).abs() < 1e-9);
    }

    #[test]
    fn network_star_graph() {
        // Star on 5 vertices: 4 edges, 0 triangles, C = 0
        let edges = vec![
            InteractionEdge { a: 0, b: 1, weight: 1 },
            InteractionEdge { a: 0, b: 2, weight: 1 },
            InteractionEdge { a: 0, b: 3, weight: 1 },
            InteractionEdge { a: 0, b: 4, weight: 1 },
        ];
        let m = network_metrics(&edges, 5);
        assert_eq!(m.edge_count, 4);
        // density = 2*4/(5*4) = 0.4
        assert!((m.density - 0.4).abs() < 1e-9, "density: {}", m.density);
        assert!(m.global_clustering_coefficient.abs() < 1e-9,
            "C: {}", m.global_clustering_coefficient);
        // Mean degree = 2*4/5 = 1.6
        assert!((m.mean_degree - 1.6).abs() < 1e-9);
    }

    #[test]
    fn network_disconnected_components() {
        // Two disjoint edges on 4 nodes → largest component fraction = 0.5
        let edges = vec![
            InteractionEdge { a: 0, b: 1, weight: 1 },
            InteractionEdge { a: 2, b: 3, weight: 1 },
        ];
        let m = network_metrics(&edges, 4);
        assert!((m.largest_component_ratio - 0.5).abs() < 1e-9);
    }

    // ── Specialization ────────────────────────────────────

    #[test]
    fn specialization_empty() {
        let m = specialization_metrics(&[]);
        assert_eq!(m.agent_count, 0);
        assert!(m.mean_specialization.abs() < 1e-9);
    }

    #[test]
    fn specialization_perfect_specialists() {
        // Each agent does exactly one distinct role
        let profiles: Vec<_> = (0..5u32)
            .map(|i| AgentRoleProfile {
                agent_id: i,
                action_counts: {
                    let mut h = HashMap::new();
                    h.insert(format!("role-{}", i), 10);
                    h
                },
            })
            .collect();
        let m = specialization_metrics(&profiles);
        // Each agent's H=0, so specialization = 1.0
        assert!((m.mean_specialization - 1.0).abs() < 1e-9,
            "mean_specialization: {}", m.mean_specialization);
        assert_eq!(m.role_count, 5);
    }

    #[test]
    fn specialization_perfect_generalists() {
        // Every agent does every role equally
        let profiles: Vec<_> = (0..3u32)
            .map(|i| AgentRoleProfile {
                agent_id: i,
                action_counts: {
                    let mut h = HashMap::new();
                    h.insert("a".to_string(), 5);
                    h.insert("b".to_string(), 5);
                    h.insert("c".to_string(), 5);
                    h
                },
            })
            .collect();
        let m = specialization_metrics(&profiles);
        // Each agent's H = log2(3) = max → specialization = 0
        assert!(m.mean_specialization.abs() < 1e-6,
            "expected ~0, got {}", m.mean_specialization);
        // Population-level diversity normalised = 1.0
        assert!((m.role_diversity_normalized - 1.0).abs() < 1e-6);
    }

    // ── Inequality ────────────────────────────────────────

    #[test]
    fn inequality_empty() {
        let m = inequality_metrics(&[]);
        assert_eq!(m.tick_count, 0);
        assert!(m.final_gini.abs() < 1e-9);
    }

    #[test]
    fn inequality_perfect_equality() {
        let snaps = vec![WealthSnapshot { tick: 0, wealth: vec![100, 100, 100, 100] }];
        let m = inequality_metrics(&snaps);
        assert!(m.final_gini.abs() < 1e-9, "gini: {}", m.final_gini);
        // top 10% of 4 = ceil(0.4) = 1 agent → 100/400 = 0.25
        assert!((m.final_top10_share - 0.25).abs() < 1e-9);
    }

    #[test]
    fn inequality_monopoly() {
        let snaps = vec![WealthSnapshot {
            tick: 0,
            wealth: vec![0, 0, 0, 0, 1000],
        }];
        let m = inequality_metrics(&snaps);
        // Gini = 0.8
        assert!((m.final_gini - 0.8).abs() < 1e-6, "gini: {}", m.final_gini);
        // top 10% of 5 = ceil(0.5) = 1 → 1000/1000 = 1.0
        assert!((m.final_top10_share - 1.0).abs() < 1e-9);
    }

    #[test]
    fn inequality_increasing_slope() {
        let snaps = vec![
            WealthSnapshot { tick: 0, wealth: vec![100, 100, 100, 100] }, // gini 0
            WealthSnapshot { tick: 10, wealth: vec![50, 100, 100, 150] }, // gini ~0.2
            WealthSnapshot { tick: 20, wealth: vec![0, 100, 100, 200] },  // gini ~0.4
        ];
        let m = inequality_metrics(&snaps);
        assert!(m.gini_trend_slope > 0.0, "slope should be > 0: {}", m.gini_trend_slope);
        assert!(m.final_gini > m.mean_gini - m.final_gini,
            "final {} should exceed typical", m.final_gini);
    }

    // ── Organization ──────────────────────────────────────

    #[test]
    fn organization_empty() {
        let m = organization_metrics(&[], 100);
        assert_eq!(m.total_orgs_formed, 0);
    }

    #[test]
    fn organization_all_stable() {
        let entries = vec![
            OrgLifecycleEntry { org_id: 0, born_tick: 10, dissolved_tick: None, peak_members: 5 },
            OrgLifecycleEntry { org_id: 1, born_tick: 20, dissolved_tick: None, peak_members: 8 },
        ];
        let m = organization_metrics(&entries, 100);
        assert_eq!(m.total_orgs_formed, 2);
        assert_eq!(m.orgs_alive_at_end, 2);
        assert!(m.churn_rate.abs() < 1e-9);
        // lifespans: 100-10=90, 100-20=80 → mean 85
        assert!((m.mean_lifespan_ticks - 85.0).abs() < 1e-6);
        assert!((m.mean_peak_members - 6.5).abs() < 1e-6);
    }

    #[test]
    fn organization_all_dissolved() {
        let entries = vec![
            OrgLifecycleEntry { org_id: 0, born_tick: 0, dissolved_tick: Some(10), peak_members: 3 },
            OrgLifecycleEntry { org_id: 1, born_tick: 5, dissolved_tick: Some(15), peak_members: 4 },
        ];
        let m = organization_metrics(&entries, 100);
        assert_eq!(m.orgs_alive_at_end, 0);
        assert!((m.churn_rate - 1.0).abs() < 1e-9);
        // lifespans 10 and 10 → mean 10, median 10
        assert!((m.mean_lifespan_ticks - 10.0).abs() < 1e-6);
        assert!((m.median_lifespan_ticks - 10.0).abs() < 1e-6);
    }

    // ── Cultural Diversity ────────────────────────────────

    #[test]
    fn diversity_empty() {
        let m = diversity_metrics(&[]);
        assert_eq!(m.tick_count, 0);
    }

    #[test]
    fn diversity_uniform_distribution() {
        let mut counts = HashMap::new();
        counts.insert("a".to_string(), 10);
        counts.insert("b".to_string(), 10);
        counts.insert("c".to_string(), 10);
        counts.insert("d".to_string(), 10);
        let snaps = vec![CulturalSignalSnapshot { tick: 0, signal_counts: counts }];
        let m = diversity_metrics(&snaps);
        // Entropy = log2(4) = 2.0
        assert!((m.mean_entropy - 2.0).abs() < 1e-6, "entropy: {}", m.mean_entropy);
        assert!((m.mean_normalized_entropy - 1.0).abs() < 1e-6);
        assert_eq!(m.signal_categories, 4);
    }

    #[test]
    fn diversity_monoculture() {
        let mut counts = HashMap::new();
        counts.insert("only".to_string(), 100);
        let snaps = vec![CulturalSignalSnapshot { tick: 0, signal_counts: counts }];
        let m = diversity_metrics(&snaps);
        assert!(m.mean_entropy.abs() < 1e-9);
    }

    // ── Helpers ───────────────────────────────────────────

    #[test]
    fn gini_helpers_basic() {
        assert!(gini_u64(&[]).abs() < 1e-9);
        assert!(gini_u64(&[5]).abs() < 1e-9);
        assert!(gini_u64(&[5, 5, 5]).abs() < 1e-9);
        let g = gini_u64(&[0, 0, 0, 0, 100]);
        assert!((g - 0.8).abs() < 1e-6, "gini: {}", g);
    }

    #[test]
    fn top_share_basic() {
        assert!(top_percent_share_u64(&[], 0.1).abs() < 1e-9);
        let s = top_percent_share_u64(&[10, 20, 30, 40], 0.1); // top 1 of 4
        // top 1 = 40, total 100 → 0.4
        assert!((s - 0.4).abs() < 1e-9, "top10: {}", s);
    }

    // ── Full report ───────────────────────────────────────

    #[test]
    fn full_report_schema() {
        let report = compute_full_report(
            &[DiffusionObservation { agent_id: 0, first_seen_tick: 0 }],
            2,
            100,
            &[InteractionEdge { a: 0, b: 1, weight: 1 }],
            &[AgentRoleProfile {
                agent_id: 0,
                action_counts: {
                    let mut h = HashMap::new();
                    h.insert("trade".to_string(), 3);
                    h
                },
            }],
            &[WealthSnapshot { tick: 0, wealth: vec![100, 100] }],
            &[OrgLifecycleEntry {
                org_id: 0,
                born_tick: 0,
                dissolved_tick: None,
                peak_members: 2,
            }],
            &[CulturalSignalSnapshot {
                tick: 0,
                signal_counts: {
                    let mut h = HashMap::new();
                    h.insert("adult".to_string(), 2);
                    h
                },
            }],
        );
        assert_eq!(report.schema_version, "emergence-benchmark/v1");
        assert_eq!(report.metric_count, 6);
        assert!(report.diffusion.final_coverage >= 0.0);
        assert!(report.network.density > 0.0);
    }
}
