//! Auto Report Generation — `/api/v2/export/report`.
//!
//! Generates HTML reports from time capsule data, including:
//! - Population trend chart
//! - GDP trajectory
//! - Gini coefficient trend
//! - Key events timeline
//! - Agent summary statistics

use axum::{
    Router,
    extract::{Query, State},
    http::{StatusCode, header},
    response::IntoResponse,
    routing::get,
};
use serde::Deserialize;

use crate::api::AppState;

// ── HTML Escaping ─────────────────────────────────────────

/// Escape HTML special characters to prevent XSS.
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

// ── Query Types ───────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ReportQuery {
    /// Report format: "html". Defaults to "html".
    pub format: Option<String>,
    /// From tick (optional).
    pub from_tick: Option<u64>,
    /// To tick (optional).
    pub to_tick: Option<u64>,
    /// Report title.
    pub title: Option<String>,
}

// ── Router ────────────────────────────────────────────────

pub fn report_routes() -> Router<AppState> {
    Router::new().route("/api/v2/export/report", get(generate_report))
}

// ── Helpers ───────────────────────────────────────────────

/// Build an SVG sparkline from a series of values.
fn sparkline(values: &[f64], width: u32, height: u32, color: &str) -> String {
    if values.is_empty() {
        return format!(
            "<svg width=\"{}\" height=\"{}\" xmlns=\"http://www.w3.org/2000/svg\"></svg>",
            width, height
        );
    }

    let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let range = if max - min > 0.0 { max - min } else { 1.0 };

    let padding = 4.0;
    let w = width as f64 - padding * 2.0;
    let h = height as f64 - padding * 2.0;

    let points: Vec<String> = values
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let x = padding + (i as f64 / (values.len().max(1) - 1).max(1) as f64) * w;
            let y = padding + h - ((v - min) / range) * h;
            format!("{:.1},{:.1}", x, y)
        })
        .collect();

    let polyline = points.join(" ");

    format!(
        r#"<svg width="{}" height="{}" xmlns="http://www.w3.org/2000/svg">
  <polyline points="{}" fill="none" stroke="{}" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
</svg>"#,
        width, height, polyline, color
    )
}

/// Format a large number with K/M suffixes.
fn format_number(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

/// Generate an HTML report from snapshot data.
fn build_html_report(
    title: &str,
    snapshots: &[crate::time_capsule::WorldSnapshotData],
    current_tick: u64,
    agent_count: usize,
    alive_count: usize,
    total_tokens: u64,
    total_money: u64,
) -> String {
    // Escape the title to prevent XSS
    let title_escaped = html_escape(title);

    // Extract time series
    let population_series: Vec<f64> = snapshots.iter().map(|s| s.active_agents as f64).collect();
    let gdp_series: Vec<f64> = snapshots.iter().map(|s| s.gdp as f64).collect();
    let gini_series: Vec<f64> = snapshots.iter().map(|s| s.gini_coefficient).collect();

    // Key events
    let key_events: Vec<&crate::time_capsule::KeyEvent> = snapshots
        .iter()
        .flat_map(|s| s.key_events.iter())
        .collect();

    // Skill distribution
    let skills_html = if let Some(latest) = snapshots.last() {
        latest
            .skill_distribution_top5
            .iter()
            .map(|s| format!("<li>{} — {} agents (avg level: {:.1})</li>", html_escape(&s.skill_name), s.agent_count, s.avg_level))
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        "<li>No skill data available</li>".to_string()
    };

    // Trend indicators
    let population_trend = if population_series.len() >= 2 {
        let (Some(last), Some(first)) = (population_series.last(), population_series.first()) else {
            return "<p>No population data available</p>".to_string();
        };
        let delta = last - first;
        if delta > 0.0 {
            format!("<span style=\"color: #4CAF50;\">↑ +{:.0}</span>", delta)
        } else if delta < 0.0 {
            format!("<span style=\"color: #F44336;\">↓ {:.0}</span>", delta)
        } else {
            "<span style=\"color: #FF9800;\">→ stable</span>".to_string()
        }
    } else {
        String::new()
    };

    let gdp_trend = if gdp_series.len() >= 2 {
        let last = *gdp_series.last().unwrap_or(&0.0);
        let first = *gdp_series.first().unwrap_or(&0.0);
        if first > 0.0 {
            let pct = (last - first) / first * 100.0;
            if pct > 0.0 {
                format!("<span style=\"color: #4CAF50;\">↑ +{:.1}%</span>", pct)
            } else {
                format!("<span style=\"color: #F44336;\">↓ {:.1}%</span>", pct)
            }
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    let current_gini = gini_series.last().copied().unwrap_or(0.0);
    let gini_color = if current_gini < 0.3 {
        "#4CAF50"
    } else if current_gini < 0.5 {
        "#FF9800"
    } else {
        "#F44336"
    };
    let gini_display = format!("{:.4}", current_gini);

    // Events table
    let events_rows = key_events
        .iter()
        .take(20)
        .map(|e| {
            format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                e.tick,
                html_escape(&e.event_type),
                html_escape(e.agent_id.as_deref().unwrap_or("—")),
                html_escape(&e.description),
            )
        })
        .collect::<Vec<_>>()
        .join("\n              ");

    // Snapshot table
    let snapshot_rows = snapshots
        .iter()
        .rev()
        .take(50)
        .map(|s| {
            format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{:.4}</td></tr>",
                s.tick,
                s.total_population,
                s.active_agents,
                format_number(s.gdp),
                s.gini_coefficient,
            )
        })
        .collect::<Vec<_>>()
        .join("\n              ");

    format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>{title}</title>
  <style>
    * {{ margin: 0; padding: 0; box-sizing: border-box; }}
    body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; background: #0d1117; color: #c9d1d9; padding: 2rem; }}
    h1 {{ color: #f0f6fc; font-size: 1.8rem; margin-bottom: 0.5rem; }}
    h2 {{ color: #f0f6fc; font-size: 1.3rem; margin: 1.5rem 0 0.75rem; border-bottom: 1px solid #30363d; padding-bottom: 0.5rem; }}
    .subtitle {{ color: #8b949e; margin-bottom: 2rem; }}
    .grid {{ display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 1rem; margin-bottom: 2rem; }}
    .card {{ background: #161b22; border: 1px solid #30363d; border-radius: 8px; padding: 1.25rem; }}
    .card-label {{ color: #8b949e; font-size: 0.8rem; text-transform: uppercase; letter-spacing: 0.05em; }}
    .card-value {{ color: #f0f6fc; font-size: 1.6rem; font-weight: 600; margin: 0.25rem 0; }}
    .card-trend {{ font-size: 0.85rem; }}
    .sparkline {{ margin: 0.5rem 0; }}
    table {{ width: 100%; border-collapse: collapse; margin-top: 0.5rem; }}
    th {{ text-align: left; color: #8b949e; font-size: 0.75rem; text-transform: uppercase; padding: 0.5rem 0.75rem; border-bottom: 1px solid #30363d; }}
    td {{ padding: 0.5rem 0.75rem; border-bottom: 1px solid #21262d; font-size: 0.85rem; }}
    tr:hover {{ background: #161b22; }}
    .tag {{ display: inline-block; background: #1f6feb33; color: #58a6ff; border-radius: 4px; padding: 0.15rem 0.5rem; font-size: 0.75rem; }}
    ul {{ list-style: none; padding-left: 0; }}
    li {{ padding: 0.25rem 0; }}
    .footer {{ color: #484f58; font-size: 0.75rem; margin-top: 3rem; text-align: center; }}
  </style>
</head>
<body>
  <h1>📊 {title}</h1>
  <p class="subtitle">Generated at tick {current_tick} — {snapshot_count} snapshots captured</p>

  <h2>Key Metrics</h2>
  <div class="grid">
    <div class="card">
      <div class="card-label">Population</div>
      <div class="card-value">{alive_count} / {agent_count}</div>
      <div class="card-trend">{population_trend}</div>
      <div class="sparkline">{pop_sparkline}</div>
    </div>
    <div class="card">
      <div class="card-label">GDP (Total Tokens)</div>
      <div class="card-value">{gdp_display}</div>
      <div class="card-trend">{gdp_trend}</div>
      <div class="sparkline">{gdp_sparkline}</div>
    </div>
    <div class="card">
      <div class="card-label">Gini Coefficient</div>
      <div class="card-value" style="color: {gini_color}">{gini_display}</div>
      <div class="card-trend">{gini_interpretation}</div>
      <div class="sparkline">{gini_sparkline}</div>
    </div>
    <div class="card">
      <div class="card-label">Total Money</div>
      <div class="card-value">{money_display}</div>
    </div>
  </div>

  <h2>Top Skills</h2>
  <ul>
    {skills_html}
  </ul>

  <h2>Key Events Timeline</h2>
  <table>
    <thead>
      <tr><th>Tick</th><th>Type</th><th>Agent</th><th>Description</th></tr>
    </thead>
    <tbody>
      {events_rows}
    </tbody>
  </table>

  <h2>Snapshot History</h2>
  <table>
    <thead>
      <tr><th>Tick</th><th>Total Pop</th><th>Active</th><th>GDP</th><th>Gini</th></tr>
    </thead>
    <tbody>
      {snapshot_rows}
    </tbody>
  </table>

  <p class="footer">Agent World Engine v1.0 — Auto-generated report</p>
</body>
</html>"##,
        title = title_escaped,
        current_tick = current_tick,
        snapshot_count = snapshots.len(),
        alive_count = alive_count,
        agent_count = agent_count,
        pop_sparkline = sparkline(&population_series, 180, 40, "#58a6ff"),
        gdp_display = format_number(total_tokens),
        gdp_trend = gdp_trend,
        gdp_sparkline = sparkline(&gdp_series, 180, 40, "#3fb950"),
        gini_color = gini_color,
        gini_display = gini_display,
        gini_interpretation = if current_gini < 0.3 { "Low inequality" } else if current_gini < 0.5 { "Moderate inequality" } else { "High inequality" },
        gini_sparkline = sparkline(&gini_series, 180, 40, gini_color),
        money_display = format_number(total_money),
        skills_html = skills_html,
        events_rows = events_rows,
        snapshot_rows = snapshot_rows,
    )
}

// ── Handler ───────────────────────────────────────────────

/// `GET /api/v2/export/report` — generate a report from time capsule data.
///
/// Supports formats: html, json, markdown.
async fn generate_report(
    State(state): State<AppState>,
    Query(query): Query<ReportQuery>,
) -> impl IntoResponse {
    let fmt = query.format.as_deref().unwrap_or("html").to_lowercase();
    let title = query.title.as_deref().unwrap_or("Agent World Report");

    // Collect snapshot data
    let snapshots = if let Some(ref store) = state.snapshot_store {
        let store = store.lock().await;
        store.list(query.from_tick, query.to_tick, None).unwrap_or_default()
    } else {
        Vec::new()
    };

    // Current state
    let agents = state.agents.lock().await;
    let current_tick = *state.tick_rx.borrow();
    let agent_count = agents.len();
    let alive_count = agents.iter().filter(|a| a.alive).count();
    let total_tokens: u64 = agents.iter().map(|a| a.tokens).sum();
    let total_money: u64 = agents.iter().map(|a| a.money).sum();
    drop(agents);

    match fmt.as_str() {
        "html" => {
            let html = build_html_report(
                title,
                &snapshots,
                current_tick,
                agent_count,
                alive_count,
                total_tokens,
                total_money,
            );
            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
                html,
            )
                .into_response()
        }
        "json" => {
            let report = build_json_report(
                title,
                &snapshots,
                current_tick,
                agent_count,
                alive_count,
                total_tokens,
                total_money,
            );
            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, "application/json")],
                axum::Json(report),
            )
                .into_response()
        }
        "markdown" => {
            let md = build_markdown_report(
                title,
                &snapshots,
                current_tick,
                agent_count,
                alive_count,
                total_tokens,
                total_money,
            );
            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, "text/markdown; charset=utf-8")],
                md,
            )
                .into_response()
        }
        _ => (
            StatusCode::BAD_REQUEST,
            [(header::CONTENT_TYPE, "application/json")],
            axum::Json(serde_json::json!({
                "error": format!("Unsupported report format: {}. Use 'html', 'json', or 'markdown'.", fmt)
            })),
        )
            .into_response(),
    }
}

// ── JSON Report ──────────────────────────────────────────

/// Build a structured JSON report with trend analysis and emergent pattern detection.
fn build_json_report(
    title: &str,
    snapshots: &[crate::time_capsule::WorldSnapshotData],
    current_tick: u64,
    agent_count: usize,
    alive_count: usize,
    total_tokens: u64,
    total_money: u64,
) -> serde_json::Value {
    let trends = compute_trends(snapshots);
    let patterns = detect_emergent_patterns(snapshots);

    // Skill distribution from latest snapshot
    let skills: Vec<serde_json::Value> = snapshots
        .last()
        .map(|s| {
            s.skill_distribution_top5
                .iter()
                .map(|sk| {
                    serde_json::json!({
                        "skill_name": sk.skill_name,
                        "agent_count": sk.agent_count,
                        "avg_level": sk.avg_level,
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    // Key events
    let events: Vec<serde_json::Value> = snapshots
        .iter()
        .flat_map(|s| s.key_events.iter())
        .take(20)
        .map(|e| {
            serde_json::json!({
                "tick": e.tick,
                "event_type": e.event_type,
                "agent_id": e.agent_id,
                "description": e.description,
            })
        })
        .collect();

    // Snapshot history
    let history: Vec<serde_json::Value> = snapshots
        .iter()
        .rev()
        .take(50)
        .map(|s| {
            serde_json::json!({
                "tick": s.tick,
                "total_population": s.total_population,
                "active_agents": s.active_agents,
                "gdp": s.gdp,
                "gini_coefficient": s.gini_coefficient,
            })
        })
        .collect();

    serde_json::json!({
        "title": title,
        "generated_at_tick": current_tick,
        "snapshot_count": snapshots.len(),
        "summary": {
            "total_agents": agent_count,
            "alive_agents": alive_count,
            "total_tokens": total_tokens,
            "total_money": total_money,
        },
        "skills": skills,
        "events": events,
        "history": history,
        "trends": trends,
        "emergent_patterns": patterns,
    })
}

// ── Markdown Report ──────────────────────────────────────

/// Build a Markdown report with trend analysis and emergent pattern detection.
fn build_markdown_report(
    title: &str,
    snapshots: &[crate::time_capsule::WorldSnapshotData],
    current_tick: u64,
    agent_count: usize,
    alive_count: usize,
    total_tokens: u64,
    total_money: u64,
) -> String {
    let trends = compute_trends(snapshots);
    let patterns = detect_emergent_patterns(snapshots);

    let mut md = String::new();
    md.push_str(&format!("# {}\n\n", title));
    md.push_str(&format!("Generated at tick {} — {} snapshots captured\n\n", current_tick, snapshots.len()));

    md.push_str("## Summary\n\n");
    md.push_str(&format!("- **Agents**: {} alive / {} total\n", alive_count, agent_count));
    md.push_str(&format!("- **Total Tokens**: {}\n", format_number(total_tokens)));
    md.push_str(&format!("- **Total Money**: {}\n", format_number(total_money)));

    // Trends
    if !trends.is_empty() {
        md.push_str("\n## Trends\n\n");
        for t in &trends {
            md.push_str(&format!(
                "- **{}**: {} (delta: {:.1}%)\n",
                t["metric"].as_str().unwrap_or("unknown"),
                t["direction"].as_str().unwrap_or("stable"),
                t["delta_percent"].as_f64().unwrap_or(0.0),
            ));
        }
    }

    // Emergent Patterns
    if !patterns.is_empty() {
        md.push_str("\n## Emergent Patterns\n\n");
        for p in &patterns {
            let severity = p["severity"].as_str().unwrap_or("info");
            let icon = match severity {
                "critical" => "**[CRITICAL]**",
                "warning" => "**[WARNING]**",
                _ => "*[INFO]*",
            };
            md.push_str(&format!(
                "- {} {}: {}\n",
                icon,
                p["pattern"].as_str().unwrap_or("unknown"),
                p["description"].as_str().unwrap_or(""),
            ));
        }
    }

    // Top Skills
    if let Some(latest) = snapshots.last() {
        if !latest.skill_distribution_top5.is_empty() {
            md.push_str("\n## Top Skills\n\n");
            md.push_str("| Skill | Agents | Avg Level |\n");
            md.push_str("|-------|--------|----------|\n");
            for s in &latest.skill_distribution_top5 {
                md.push_str(&format!(
                    "| {} | {} | {:.1} |\n",
                    s.skill_name, s.agent_count, s.avg_level
                ));
            }
        }
    }

    // Snapshot History
    if !snapshots.is_empty() {
        md.push_str("\n## Snapshot History\n\n");
        md.push_str("| Tick | Population | Active | GDP | Gini |\n");
        md.push_str("|------|-----------|--------|-----|------|\n");
        for s in snapshots.iter().rev().take(50) {
            md.push_str(&format!(
                "| {} | {} | {} | {} | {:.4} |\n",
                s.tick, s.total_population, s.active_agents,
                format_number(s.gdp), s.gini_coefficient,
            ));
        }
    }

    md.push_str("\n---\n*Agent World Engine v1.0 — Auto-generated report*\n");
    md
}

// ── Trend Analysis ───────────────────────────────────────

/// Compute trend analysis for key metrics.
fn compute_trends(snapshots: &[crate::time_capsule::WorldSnapshotData]) -> Vec<serde_json::Value> {
    let mut trends = Vec::new();

    if snapshots.len() < 2 {
        return trends;
    }

    let (Some(first), Some(last)) = (snapshots.first(), snapshots.last()) else {
        return vec![serde_json::json!({"error": "no snapshot data available"})];
    };

    let metrics: Vec<(&str, f64, f64)> = vec![
        ("population", first.active_agents as f64, last.active_agents as f64),
        ("gdp", first.gdp as f64, last.gdp as f64),
        ("gini_coefficient", first.gini_coefficient, last.gini_coefficient),
    ];

    for (name, first_val, last_val) in metrics {
        let delta = last_val - first_val;
        let delta_percent = if first_val != 0.0 {
            (delta / first_val) * 100.0
        } else {
            0.0
        };
        let direction = if delta > 0.0 {
            "increasing"
        } else if delta < 0.0 {
            "decreasing"
        } else {
            "stable"
        };

        trends.push(serde_json::json!({
            "metric": name,
            "first_value": first_val,
            "last_value": last_val,
            "delta": delta,
            "delta_percent": delta_percent,
            "direction": direction,
        }));
    }

    trends
}

// ── Emergent Pattern Detection ───────────────────────────

/// Detect emergent patterns in world state data.
fn detect_emergent_patterns(snapshots: &[crate::time_capsule::WorldSnapshotData]) -> Vec<serde_json::Value> {
    let mut patterns = Vec::new();

    if snapshots.is_empty() {
        return patterns;
    }

    let Some(latest) = snapshots.last() else {
        return vec![serde_json::json!({"error": "no snapshot data available"})];
    };

    // 1. Wealth inequality spike
    if latest.gini_coefficient > 0.8 {
        patterns.push(serde_json::json!({
            "pattern": "wealth_inequality_spike",
            "severity": "critical",
            "description": format!("Extreme wealth inequality detected (Gini = {:.4}). Resources are highly concentrated.", latest.gini_coefficient),
            "value": latest.gini_coefficient,
        }));
    } else if latest.gini_coefficient > 0.6 {
        patterns.push(serde_json::json!({
            "pattern": "wealth_inequality_warning",
            "severity": "warning",
            "description": format!("High wealth inequality (Gini = {:.4}). Consider redistributive policies.", latest.gini_coefficient),
            "value": latest.gini_coefficient,
        }));
    }

    // 2. Population collapse
    if snapshots.len() >= 3 {
        let recent_count = snapshots.len().min(5);
        let recent: Vec<_> = snapshots.iter().rev().take(recent_count).collect();
        if let (Some(earliest), Some(newest)) = (recent.last(), recent.first()) {
            if earliest.active_agents > 0 {
                let decline_pct =
                    ((earliest.active_agents - newest.active_agents) as f64 / earliest.active_agents as f64) * 100.0;
                if decline_pct > 50.0 {
                    patterns.push(serde_json::json!({
                        "pattern": "population_collapse",
                        "severity": "critical",
                        "description": format!("Population declined {:.1}% in recent snapshots ({} → {} agents).", decline_pct, earliest.active_agents, newest.active_agents),
                        "decline_percent": decline_pct,
                    }));
                } else if decline_pct > 25.0 {
                    patterns.push(serde_json::json!({
                        "pattern": "population_decline",
                        "severity": "warning",
                        "description": format!("Significant population decline ({:.1}%) in recent snapshots.", decline_pct),
                        "decline_percent": decline_pct,
                    }));
                }
            }
        }
    }

    // 3. Skill homogenization
    if !latest.skill_distribution_top5.is_empty() && latest.active_agents > 0 {
        let top_skill_share = latest.skill_distribution_top5[0].agent_count as f64
            / latest.active_agents as f64;
        if top_skill_share > 0.8 {
            patterns.push(serde_json::json!({
                "pattern": "skill_homogenization",
                "severity": "info",
                "description": format!(
                    "Over 80% of agents share the same skill '{}' ({:.0}% of population).",
                    latest.skill_distribution_top5[0].skill_name,
                    top_skill_share * 100.0
                ),
                "skill_name": latest.skill_distribution_top5[0].skill_name,
                "share": top_skill_share,
            }));
        }
    }

    // 4. Economic boom/recession
    if snapshots.len() >= 4 {
        let mid = snapshots.len() / 2;
        let first_half_gdp: u64 = snapshots[..mid].iter().map(|s| s.gdp).sum();
        let second_half_gdp: u64 = snapshots[mid..].iter().map(|s| s.gdp).sum();
        let first_half_avg = first_half_gdp / mid.max(1) as u64;
        let second_half_avg = second_half_gdp / (snapshots.len() - mid).max(1) as u64;
        if first_half_avg > 0 {
            let change_pct =
                ((second_half_avg as f64 - first_half_avg as f64) / first_half_avg as f64) * 100.0;
            if change_pct > 50.0 {
                patterns.push(serde_json::json!({
                    "pattern": "economic_boom",
                    "severity": "info",
                    "description": format!("GDP increased {:.1}% between first and second half of observation period.", change_pct),
                    "change_percent": change_pct,
                }));
            } else if change_pct < -50.0 {
                patterns.push(serde_json::json!({
                    "pattern": "economic_recession",
                    "severity": "warning",
                    "description": format!("GDP decreased {:.1}% between first and second half of observation period.", change_pct),
                    "change_percent": change_pct,
                }));
            }
        }
    }

    patterns
}

// ── Tests ──────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sparkline_empty() {
        let svg = sparkline(&[], 100, 30, "#fff");
        assert!(svg.contains("<svg"));
        assert!(svg.contains("</svg>"));
    }

    #[test]
    fn sparkline_single_value() {
        let svg = sparkline(&[5.0], 100, 30, "#fff");
        assert!(svg.contains("<svg"));
        assert!(svg.contains("polyline"));
    }

    #[test]
    fn sparkline_multiple_values() {
        let svg = sparkline(&[1.0, 5.0, 3.0, 8.0, 2.0], 200, 40, "#3fb950");
        assert!(svg.contains("stroke=\"#3fb950\""));
        assert!(svg.contains("polyline"));
    }

    #[test]
    fn format_number_small() {
        assert_eq!(format_number(42), "42");
    }

    #[test]
    fn format_number_thousands() {
        assert_eq!(format_number(1500), "1.5K");
    }

    #[test]
    fn format_number_millions() {
        assert_eq!(format_number(2_500_000), "2.5M");
    }

    #[test]
    fn build_report_with_snapshots() {
        use crate::time_capsule::{WorldSnapshotData, SkillCount, KeyEvent};

        let snapshots = vec![
            WorldSnapshotData {
                tick: 100,
                timestamp: 1700000000,
                total_population: 10,
                active_agents: 8,
                gdp: 5000,
                gini_coefficient: 0.25,
                skill_distribution_top5: vec![SkillCount {
                    skill_name: "mining".into(),
                    agent_count: 5,
                    avg_level: 3.2,
                }],
                key_events: vec![KeyEvent {
                    tick: 90,
                    event_type: "agent_died".into(),
                    agent_id: Some("a1".into()),
                    description: "Agent died: TokenDepleted".into(),
                }],
            },
            WorldSnapshotData {
                tick: 200,
                timestamp: 1700000100,
                total_population: 10,
                active_agents: 7,
                gdp: 6000,
                gini_coefficient: 0.30,
                // Latest snapshot also has skills so they appear in the report
                skill_distribution_top5: vec![SkillCount {
                    skill_name: "mining".into(),
                    agent_count: 5,
                    avg_level: 3.2,
                }, SkillCount {
                    skill_name: "trading".into(),
                    agent_count: 3,
                    avg_level: 2.5,
                }],
                key_events: vec![],
            },
        ];

        let html = build_html_report("Test Report", &snapshots, 200, 10, 7, 6000, 3000);
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("Test Report"));
        assert!(html.contains("7 / 10"));
        assert!(html.contains("mining"));
        assert!(html.contains("agent_died"));
        assert!(html.contains("0.3000"));
    }

    #[test]
    fn build_report_empty_snapshots() {
        let html = build_html_report("Empty Report", &[], 0, 0, 0, 0, 0);
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("Empty Report"));
    }
}
