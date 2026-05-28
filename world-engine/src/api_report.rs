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
    let mut html = String::new();

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
            .map(|s| format!("<li>{} — {} agents (avg level: {:.1})</li>", s.skill_name, s.agent_count, s.avg_level))
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        "<li>No skill data available</li>".to_string()
    };

    // Trend indicators
    let population_trend = if population_series.len() >= 2 {
        let last = population_series.last().unwrap();
        let first = population_series.first().unwrap();
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
                e.event_type,
                e.agent_id.as_deref().unwrap_or("—"),
                e.description,
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

    html.push_str(&format!(
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
        title = title,
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
    ));

    html
}

// ── Handler ───────────────────────────────────────────────

/// `GET /api/v2/export/report` — generate an HTML report from time capsule data.
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
        _ => (
            StatusCode::BAD_REQUEST,
            [(header::CONTENT_TYPE, "application/json")],
            axum::Json(serde_json::json!({
                "error": format!("Unsupported report format: {}. Use 'html'.", fmt)
            })),
        )
            .into_response(),
    }
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
