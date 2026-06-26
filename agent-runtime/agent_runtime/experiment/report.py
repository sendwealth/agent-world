"""Experiment report generator — Markdown, JSON, HTML, and PDF output.

Auto-generates experiment reports from ExperimentResult, and comparison
reports from A/B ComparisonReport.

PDF generation uses fpdf2 (lightweight, no external deps beyond Python).
If fpdf2 is not installed, PDF output falls back to HTML.
"""

from __future__ import annotations

import json
from dataclasses import dataclass, field
from typing import Any

# ---------------------------------------------------------------------------
# Result data model
# ---------------------------------------------------------------------------


@dataclass
class ExperimentResult:
    """Standardized result of a single experiment run.

    Captures everything needed to reproduce and analyze the experiment.
    """

    experiment_id: str
    config_snapshot: dict[str, Any]
    duration_ticks: int
    completed_ticks: int
    agent_count: int
    final_snapshot: dict[str, Any] = field(default_factory=dict)
    metrics_timeline: list[dict[str, Any]] = field(default_factory=list)
    emergence_events: list[dict[str, Any]] = field(default_factory=list)
    errors: list[str] = field(default_factory=list)
    started_at: str = ""
    finished_at: str = ""

    def to_dict(self) -> dict[str, Any]:
        """Serialize to a JSON-safe dict."""
        return {
            "experiment_id": self.experiment_id,
            "config_snapshot": self.config_snapshot,
            "duration_ticks": self.duration_ticks,
            "completed_ticks": self.completed_ticks,
            "agent_count": self.agent_count,
            "final_snapshot": self.final_snapshot,
            "metrics_timeline": self.metrics_timeline,
            "emergence_events": self.emergence_events,
            "errors": self.errors,
            "started_at": self.started_at,
            "finished_at": self.finished_at,
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> ExperimentResult:
        """Deserialize from a dict."""
        return cls(
            experiment_id=data.get("experiment_id", ""),
            config_snapshot=data.get("config_snapshot", {}),
            duration_ticks=data.get("duration_ticks", 0),
            completed_ticks=data.get("completed_ticks", 0),
            agent_count=data.get("agent_count", 0),
            final_snapshot=data.get("final_snapshot", {}),
            metrics_timeline=data.get("metrics_timeline", []),
            emergence_events=data.get("emergence_events", []),
            errors=data.get("errors", []),
            started_at=data.get("started_at", ""),
            finished_at=data.get("finished_at", ""),
        )


# ---------------------------------------------------------------------------
# Reporter
# ---------------------------------------------------------------------------


class ExperimentReporter:
    """Auto-generate experiment reports in multiple formats.

    Usage::

        reporter = ExperimentReporter()
        md = reporter.generate_report(result, format="markdown")
        pdf = reporter.generate_report(result, format="pdf")
    """

    def generate_report(
        self,
        result: ExperimentResult,
        format: str = "markdown",
    ) -> str | bytes:
        """Generate a single-experiment report.

        Args:
            result: The experiment result to report on.
            format: Output format — "markdown", "json", "html", or "pdf".

        Returns:
            Report as a string (or bytes for PDF).
        """
        if format == "json":
            return self._report_json(result)
        if format == "html":
            return self._report_html(result)
        if format == "pdf":
            return self._report_pdf(result)
        return self._report_markdown(result)

    def generate_ab_report(
        self,
        comparison: Any,
        format: str = "markdown",
    ) -> str | bytes:
        """Generate an A/B comparison report.

        Args:
            comparison: A ComparisonReport instance.
            format: Output format — "markdown", "json", "html", or "pdf".

        Returns:
            Comparison report as a string (or bytes for PDF).
        """
        if format == "json":
            return self._ab_report_json(comparison)
        if format == "html":
            return self._ab_report_html(comparison)
        if format == "pdf":
            return self._ab_report_pdf(comparison)
        return self._ab_report_markdown(comparison)

    # -------------------------------------------------------------------
    # Single-experiment formatters
    # -------------------------------------------------------------------

    def _report_markdown(self, result: ExperimentResult) -> str:
        """Generate a Markdown report."""
        lines: list[str] = []
        lines.append(f"# Experiment Report: {result.experiment_id}")
        lines.append("")
        lines.append("## Overview")
        lines.append("")
        lines.append(f"- **Started**: {result.started_at or 'N/A'}")
        lines.append(f"- **Finished**: {result.finished_at or 'N/A'}")
        lines.append(f"- **Duration**: {result.completed_ticks}/{result.duration_ticks} ticks")
        lines.append(f"- **Agent Count**: {result.agent_count}")
        lines.append(f"- **Errors**: {len(result.errors)}")
        lines.append("")

        # Metrics summary
        if result.metrics_timeline:
            lines.append("## Metrics Timeline")
            lines.append("")
            lines.append("| Tick | Metric |")
            lines.append("|------|--------|")
            for entry in result.metrics_timeline[-10:]:
                tick = entry.get("tick", "?")
                summary_parts = [
                    f"{k}={v:.4f}" if isinstance(v, float) else f"{k}={v}"
                    for k, v in entry.items()
                    if k != "tick"
                ]
                lines.append(f"| {tick} | {', '.join(summary_parts[:5])} |")
            lines.append("")

        # Emergence events
        if result.emergence_events:
            lines.append("## Emergence Events")
            lines.append("")
            for evt in result.emergence_events:
                tick = evt.get("tick", "?")
                event_type = evt.get("type", "unknown")
                desc = evt.get("description", "")
                lines.append(f"- **Tick {tick}** [{event_type}]: {desc}")
            lines.append("")

        # Errors
        if result.errors:
            lines.append("## Errors")
            lines.append("")
            for err in result.errors:
                lines.append(f"- {err}")
            lines.append("")

        return "\n".join(lines)

    def _report_json(self, result: ExperimentResult) -> str:
        """Generate a JSON report."""
        return json.dumps(result.to_dict(), indent=2, ensure_ascii=False)

    def _report_html(self, result: ExperimentResult) -> str:
        """Generate an HTML report."""
        md_content = self._report_markdown(result)
        escaped = (
            md_content.replace("&", "&amp;")
            .replace("<", "&lt;")
            .replace(">", "&gt;")
        )
        return f"""<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<title>Experiment Report: {result.experiment_id}</title>
<style>
body {{ font-family: -apple-system, sans-serif; max-width: 800px;
        margin: 2rem auto; padding: 0 1rem; }}
pre {{ background: #f5f5f5; padding: 1rem; overflow-x: auto; }}
table {{ border-collapse: collapse; width: 100%; }}
th, td {{ border: 1px solid #ddd; padding: 0.5rem; text-align: left; }}
</style>
</head>
<body>
<pre>{escaped}</pre>
</body>
</html>"""

    def _report_pdf(self, result: ExperimentResult) -> bytes:
        """Generate a PDF report using fpdf2.

        Falls back to HTML if fpdf2 is not installed.
        """
        try:
            from fpdf import FPDF
        except ImportError:
            # Fallback: return HTML with a note
            html = self._report_html(result)
            return html.encode("utf-8")

        pdf = FPDF()
        pdf.add_page()
        pdf.set_auto_page_break(auto=True, margin=15)

        # Title
        pdf.set_font("Helvetica", "B", 16)
        pdf.cell(0, 10, f"Experiment Report: {result.experiment_id}", new_x="LMARGIN", new_y="NEXT")
        pdf.ln(5)

        # Overview
        pdf.set_font("Helvetica", "B", 13)
        pdf.cell(0, 8, "Overview", new_x="LMARGIN", new_y="NEXT")
        pdf.set_font("Helvetica", "", 10)
        pdf.cell(0, 6, f"Started: {result.started_at or 'N/A'}", new_x="LMARGIN", new_y="NEXT")
        pdf.cell(0, 6, f"Finished: {result.finished_at or 'N/A'}", new_x="LMARGIN", new_y="NEXT")
        pdf.cell(
            0, 6,
            f"Duration: {result.completed_ticks}/{result.duration_ticks} ticks",
            new_x="LMARGIN", new_y="NEXT",
        )
        pdf.cell(0, 6, f"Agent Count: {result.agent_count}", new_x="LMARGIN", new_y="NEXT")
        pdf.cell(0, 6, f"Errors: {len(result.errors)}", new_x="LMARGIN", new_y="NEXT")
        pdf.ln(5)

        # Metrics table
        if result.metrics_timeline:
            pdf.set_font("Helvetica", "B", 13)
            pdf.cell(0, 8, "Metrics Timeline (last 10 snapshots)", new_x="LMARGIN", new_y="NEXT")
            pdf.ln(2)

            # Table header
            pdf.set_font("Helvetica", "B", 8)
            first_entry = result.metrics_timeline[0] if result.metrics_timeline else {}
            col_keys = [k for k in first_entry.keys() if k != "tick"]
            col_widths = [20] + [min(30, 170 // max(len(col_keys), 1))] * len(col_keys)

            pdf.cell(col_widths[0], 6, "Tick", border=1)
            for key in col_keys:
                pdf.cell(col_widths[1], 6, str(key)[:15], border=1)
            pdf.ln()

            # Table rows
            pdf.set_font("Helvetica", "", 8)
            for entry in result.metrics_timeline[-10:]:
                tick = entry.get("tick", "?")
                pdf.cell(col_widths[0], 6, str(tick), border=1)
                for key in col_keys:
                    val = entry.get(key, "")
                    fmt = f"{val:.4f}" if isinstance(val, float) else str(val)
                    pdf.cell(col_widths[1], 6, fmt, border=1)
                pdf.ln()
            pdf.ln(5)

        # Emergence events
        if result.emergence_events:
            pdf.set_font("Helvetica", "B", 13)
            pdf.cell(0, 8, "Emergence Events", new_x="LMARGIN", new_y="NEXT")
            pdf.set_font("Helvetica", "", 10)
            for evt in result.emergence_events:
                tick = evt.get("tick", "?")
                event_type = evt.get("type", "unknown")
                desc = evt.get("description", "")
                pdf.cell(0, 6, f"Tick {tick} [{event_type}]: {desc}", new_x="LMARGIN", new_y="NEXT")
            pdf.ln(5)

        # Errors
        if result.errors:
            pdf.set_font("Helvetica", "B", 13)
            pdf.cell(0, 8, "Errors", new_x="LMARGIN", new_y="NEXT")
            pdf.set_font("Helvetica", "", 10)
            for err in result.errors:
                pdf.cell(0, 6, f"- {err}", new_x="LMARGIN", new_y="NEXT")

        return bytes(pdf.output())

# -------------------------------------------------------------------
# Chart-aware rich report generation
# -------------------------------------------------------------------

    def generate_rich_report(
        self,
        result: ExperimentResult,
        format: str = "html",
    ) -> str:
        """Generate a rich report with embedded charts.

        Uses charts.py to produce matplotlib visualizations embedded
        as base64 PNG images in the HTML output.

        Args:
            result: The experiment result to report on.
            format: "html" (default) or "markdown".

        Returns:
            Report string with embedded chart images.
        """
        charts_b64: dict[str, str] = {}
        timeline = result.metrics_timeline
        snapshot = result.final_snapshot

        # Attempt chart generation (graceful no-op if matplotlib missing)
        try:
            from agent_runtime.experiment.charts import (
                gdp_trajectory,
                gini_coefficient,
                population_trend,
                skill_distribution,
                survival_pie,
            )

            if timeline:
                charts_b64["population"] = population_trend(timeline)
                charts_b64["gdp"] = gdp_trajectory(timeline)
                charts_b64["gini"] = gini_coefficient(timeline)

            if snapshot:
                skills = snapshot.get("skill_distribution", [])
                if skills:
                    charts_b64["skills"] = skill_distribution(skills)

                alive = snapshot.get("agents_alive", snapshot.get("alive_count", 0))
                total = result.agent_count or 1
                charts_b64["survival"] = survival_pie(alive, total - alive)

        except ImportError:
            pass

        if format == "markdown":
            return self._rich_report_markdown(result, charts_b64)
        return self._rich_report_html(result, charts_b64)

    def _rich_report_html(
        self,
        result: ExperimentResult,
        charts: dict[str, str],
    ) -> str:
        """Generate a rich HTML report with embedded chart images."""
        overview_html = self._section_overview_html(result)
        demographics_html = self._section_demographics_html(result, charts)
        economics_html = self._section_economics_html(result, charts)
        social_html = self._section_social_html(result)
        cultural_html = self._section_cultural_html(result)
        events_html = self._section_events_html(result)

        chart_images_html = ""
        for name, b64 in charts.items():
            chart_images_html += (
                f'<div class="chart-container">\n'
                f'  <h3>{name.replace("_", " ").title()}</h3>\n'
                f'  <img src="data:image/png;base64,{b64}" alt="{name} chart" />\n'
                f'</div>\n'
            )

        return f"""<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>Experiment Report: {result.experiment_id}</title>
<style>
  body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
         background: #0d1117; color: #c9d1d9; max-width: 1000px;
         margin: 2rem auto; padding: 0 1.5rem; }}
  h1 {{ color: #f0f6fc; font-size: 1.8rem; border-bottom: 2px solid #30363d;
       padding-bottom: 0.5rem; }}
  h2 {{ color: #f0f6fc; font-size: 1.3rem; margin-top: 2rem;
       border-bottom: 1px solid #30363d; padding-bottom: 0.4rem; }}
  h3 {{ color: #58a6ff; font-size: 1.05rem; }}
  .card {{ background: #161b22; border: 1px solid #30363d; border-radius: 8px;
           padding: 1.25rem; margin: 1rem 0; }}
  .grid {{ display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
           gap: 1rem; }}
  .metric {{ text-align: center; }}
  .metric .value {{ font-size: 1.8rem; font-weight: 700; color: #f0f6fc; }}
  .metric .label {{ font-size: 0.8rem; color: #8b949e; text-transform: uppercase; }}
  .chart-container {{ text-align: center; margin: 1.5rem 0; }}
  .chart-container img {{ max-width: 100%; border-radius: 6px; }}
  table {{ width: 100%; border-collapse: collapse; margin: 0.5rem 0; }}
  th {{ text-align: left; color: #8b949e; font-size: 0.75rem; text-transform: uppercase;
       padding: 0.5rem 0.75rem; border-bottom: 1px solid #30363d; }}
  td {{ padding: 0.5rem 0.75rem; border-bottom: 1px solid #21262d; font-size: 0.85rem; }}
  tr:hover {{ background: #161b22; }}
  .tag {{ display: inline-block; background: #1f6feb33; color: #58a6ff;
          border-radius: 4px; padding: 0.15rem 0.5rem; font-size: 0.75rem;
          margin: 0.1rem; }}
  .tag-warning {{ background: #d2992233; color: #d29922; }}
  .tag-danger {{ background: #f8514933; color: #f85149; }}
  .tag-success {{ background: #3fb95033; color: #3fb950; }}
  ul {{ padding-left: 1.2rem; }}
  li {{ margin: 0.3rem 0; }}
  .footer {{ color: #484f58; font-size: 0.75rem; margin-top: 3rem; text-align: center; }}
</style>
</head>
<body>
<h1>\U0001f4ca Experiment Report: {result.experiment_id}</h1>
{overview_html}
{demographics_html}
{economics_html}
{social_html}
{cultural_html}

<h2>\U0001f4c8 Charts</h2>
{chart_images_html}

{events_html}

<p class="footer">Agent World \u2014 Auto-generated report</p>
</body>
</html>"""

    def _rich_report_markdown(
        self,
        result: ExperimentResult,
        charts: dict[str, str],
    ) -> str:
        """Generate a rich Markdown report (charts as placeholders)."""
        lines: list[str] = []
        lines.append(f"# Experiment Report: {result.experiment_id}")
        lines.append("")

        lines.append("## Overview")
        lines.append("")
        lines.append(f"- **Started**: {result.started_at or 'N/A'}")
        lines.append(f"- **Finished**: {result.finished_at or 'N/A'}")
        lines.append(f"- **Duration**: {result.completed_ticks}/{result.duration_ticks} ticks")
        lines.append(f"- **Agent Count**: {result.agent_count}")
        lines.append(f"- **Errors**: {len(result.errors)}")
        lines.append("")

        snap = result.final_snapshot
        if snap:
            alive = snap.get("agents_alive", snap.get("alive_count", "N/A"))
            lines.append("## Demographics")
            lines.append("")
            lines.append(f"- **Alive**: {alive}")
            lines.append(f"- **Total**: {result.agent_count}")
            lines.append("")

        if result.metrics_timeline:
            lines.append("## Economic Indicators")
            lines.append("")
            lines.append("| Tick | GDP | Gini |")
            lines.append("|------|-----|------|")
            for entry in result.metrics_timeline[-20:]:
                tick = entry.get("tick", "?")
                gdp = entry.get("gdp", "N/A")
                gini = entry.get("gini", entry.get("gini_coefficient", "N/A"))
                gdp_str = f"{gdp:,.0f}" if isinstance(gdp, (int, float)) else str(gdp)
                gini_str = f"{gini:.4f}" if isinstance(gini, float) else str(gini)
                lines.append(f"| {tick} | {gdp_str} | {gini_str} |")
            lines.append("")

        if snap and snap.get("social_network"):
            lines.append("## Social Network")
            lines.append("")
            net = snap["social_network"]
            lines.append(f"- **Total Interactions**: {net.get('total_interactions', 'N/A')}")
            lines.append("")

        if snap and snap.get("cultural_metrics"):
            lines.append("## Cultural Emergence")
            lines.append("")
            for k, v in snap["cultural_metrics"].items():
                lines.append(f"- **{k}**: {v}")
            lines.append("")

        if charts:
            lines.append("## Charts")
            lines.append("")
            for name in charts:
                lines.append(f"*[{name.replace('_', ' ').title()} chart — view HTML report]*")
            lines.append("")

        if result.emergence_events:
            lines.append("## Emergence Events")
            lines.append("")
            for evt in result.emergence_events:
                tick = evt.get("tick", "?")
                etype = evt.get("type", "unknown")
                desc = evt.get("description", "")
                lines.append(f"- **Tick {tick}** [{etype}]: {desc}")
            lines.append("")

        return "\n".join(lines)

    def _section_overview_html(self, result: ExperimentResult) -> str:
        """Build the overview card section."""
        return (
            '<h2>Overview</h2>\n'
            '<div class="grid">\n'
            f'  <div class="card metric"><div class="value">{result.completed_ticks:,}</div>'
            f'<div class="label">Ticks Completed</div></div>\n'
            f'  <div class="card metric"><div class="value">{result.agent_count}</div>'
            f'<div class="label">Agents</div></div>\n'
            f'  <div class="card metric"><div class="value">{len(result.emergence_events)}</div>'
            f'<div class="label">Emergence Events</div></div>\n'
            f'  <div class="card metric"><div class="value">{len(result.errors)}</div>'
            f'<div class="label">Errors</div></div>\n'
            '</div>'
        )

    def _section_demographics_html(
        self, result: ExperimentResult, charts: dict[str, str]
    ) -> str:
        """Build the demographics section."""
        snap = result.final_snapshot
        if not snap:
            return ""
        alive = snap.get("agents_alive", snap.get("alive_count", 0))
        total = result.agent_count or 1
        rate = alive / total if total > 0 else 0
        rate_cls = (
            "tag-success" if rate >= 0.8
            else ("tag-warning" if rate >= 0.5 else "tag-danger")
        )
        return (
            '<h2>\U0001f465 Demographics</h2>\n'
            f'<div class="card"><p><strong>Survival Rate:</strong> '
            f'<span class="tag {rate_cls}">{rate:.1%}</span> '
            f'({alive}/{total} agents alive)</p></div>'
        )

    def _section_economics_html(
        self, result: ExperimentResult, charts: dict[str, str]
    ) -> str:
        """Build the economics section."""
        if not result.metrics_timeline:
            return ""
        first = result.metrics_timeline[0]
        last = result.metrics_timeline[-1]
        gdp_first = first.get("gdp", 0)
        gdp_last = last.get("gdp", 0)
        gdp_change = ((gdp_last - gdp_first) / gdp_first * 100) if gdp_first else 0
        gdp_cls = "tag-success" if gdp_change >= 0 else "tag-danger"
        gini_last = last.get("gini", last.get("gini_coefficient", 0))
        gini_cls = (
            "tag-success" if gini_last < 0.3
            else ("tag-warning" if gini_last < 0.6 else "tag-danger")
        )
        gini_label = "Low" if gini_last < 0.3 else "Moderate" if gini_last < 0.6 else "High"
        return (
            '<h2>\U0001f4b0 Economic Indicators</h2>\n'
            '<div class="grid">\n'
            f'  <div class="card metric"><div class="value">{gdp_last:,.0f}</div>'
            f'<div class="label">Final GDP</div>'
            f'<span class="tag {gdp_cls}">{gdp_change:+.1f}%</span></div>\n'
            f'  <div class="card metric"><div class="value">{gini_last:.4f}</div>'
            f'<div class="label">Gini Coefficient</div>'
            f'<span class="tag {gini_cls}">{gini_label} inequality</span></div>\n'
            '</div>'
        )

    def _section_social_html(self, result: ExperimentResult) -> str:
        """Build the social network section."""
        snap = result.final_snapshot
        if not snap or not snap.get("social_network"):
            return ""
        net = snap["social_network"]
        return (
            '<h2>\U0001f310 Social Network</h2>\n'
            f'<div class="card">'
            f'<p><strong>Total Interactions:</strong> {net.get("total_interactions", "N/A")}</p>'
            f'<p><strong>Unique Pairs:</strong> {net.get("unique_pairs", "N/A")}</p>'
            '</div>'
        )

    def _section_cultural_html(self, result: ExperimentResult) -> str:
        """Build the cultural emergence section."""
        snap = result.final_snapshot
        if not snap or not snap.get("cultural_metrics"):
            return ""
        cm = snap["cultural_metrics"]
        items = "".join(f"<li><strong>{k}</strong>: {v}</li>" for k, v in cm.items())
        return f'<h2>\U0001f3ad Cultural Emergence</h2>\n<div class="card"><ul>{items}</ul></div>'

    def _section_events_html(self, result: ExperimentResult) -> str:
        """Build the emergence events timeline section."""
        if not result.emergence_events:
            return ""
        rows = ""
        for evt in result.emergence_events[:30]:
            tick = evt.get("tick", "?")
            etype = evt.get("type", "unknown")
            desc = evt.get("description", "")
            rows += (
                f'<tr><td>{tick}</td>'
                f'<td><span class="tag">{etype}</span></td>'
                f'<td>{desc}</td></tr>'
            )
        return (
            '<h2>\u26a1 Emergence Events</h2>\n'
            '<table><thead><tr><th>Tick</th><th>Type</th><th>Description</th></tr></thead>'
            f'<tbody>{rows}</tbody></table>'
        )


    # -------------------------------------------------------------------
    # A/B comparison formatters
    # -------------------------------------------------------------------

    def _ab_report_markdown(self, comparison: Any) -> str:
        """Generate an A/B comparison Markdown report."""
        lines: list[str] = []
        lines.append("# A/B Comparison Report")
        lines.append("")
        lines.append("## Summary")
        lines.append("")
        lines.append(getattr(comparison, "summary", "No summary available."))
        lines.append("")

        metrics_diff = getattr(comparison, "metrics_diff", {})
        if metrics_diff:
            lines.append("## Metrics Differences")
            lines.append("")
            lines.append("| Metric | Difference | Effect Size |")
            lines.append("|--------|------------|-------------|")
            effect_sizes = getattr(comparison, "effect_sizes", {})
            for key, val in metrics_diff.items():
                formatted = f"{val:+.4f}" if isinstance(val, float) else str(val)
                es = f"{effect_sizes.get(key, 'N/A')}"
                lines.append(f"| {key} | {formatted} | {es} |")
            lines.append("")

        significance = getattr(comparison, "statistical_significance", {})
        if significance:
            lines.append("## Statistical Significance (Welch's t-test, p-values)")
            lines.append("")
            for key, pval in significance.items():
                marker = " ✅ significant" if isinstance(pval, float) and pval < 0.05 else ""
                lines.append(f"- **{key}**: {pval}{marker}")
            lines.append("")

        recommendation = getattr(comparison, "recommendation", "")
        if recommendation:
            lines.append("## Recommendation")
            lines.append("")
            lines.append(recommendation)
            lines.append("")

        return "\n".join(lines)

    def _ab_report_json(self, comparison: Any) -> str:
        """Generate an A/B comparison JSON report."""
        data = {
            "metrics_diff": getattr(comparison, "metrics_diff", {}),
            "statistical_significance": getattr(comparison, "statistical_significance", {}),
            "effect_sizes": getattr(comparison, "effect_sizes", {}),
            "test_results": getattr(comparison, "test_results", {}),
            "recommendation": getattr(comparison, "recommendation", ""),
            "summary": getattr(comparison, "summary", ""),
        }
        return json.dumps(data, indent=2, ensure_ascii=False)

    def _ab_report_html(self, comparison: Any) -> str:
        """Generate an A/B comparison HTML report."""
        md_content = self._ab_report_markdown(comparison)
        escaped = (
            md_content.replace("&", "&amp;")
            .replace("<", "&lt;")
            .replace(">", "&gt;")
        )
        return f"""<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<title>A/B Comparison Report</title>
<style>
body {{ font-family: -apple-system, sans-serif; max-width: 800px;
        margin: 2rem auto; padding: 0 1rem; }}
pre {{ background: #f5f5f5; padding: 1rem; overflow-x: auto; }}
table {{ border-collapse: collapse; width: 100%; }}
th, td {{ border: 1px solid #ddd; padding: 0.5rem; text-align: left; }}
</style>
</head>
<body>
<pre>{escaped}</pre>
</body>
</html>"""

    def _ab_report_pdf(self, comparison: Any) -> str | bytes:
        """Generate an A/B comparison PDF report."""
        try:
            from fpdf import FPDF
        except ImportError:
            return self._ab_report_html(comparison)

        pdf = FPDF()
        pdf.add_page()
        pdf.set_auto_page_break(auto=True, margin=15)

        # Title
        pdf.set_font("Helvetica", "B", 16)
        pdf.cell(0, 10, "A/B Comparison Report", new_x="LMARGIN", new_y="NEXT")
        pdf.ln(5)

        # Summary
        pdf.set_font("Helvetica", "B", 13)
        pdf.cell(0, 8, "Summary", new_x="LMARGIN", new_y="NEXT")
        pdf.set_font("Helvetica", "", 10)
        summary = getattr(comparison, "summary", "No summary available.")
        for line in summary.split("\n"):
            pdf.cell(0, 6, line, new_x="LMARGIN", new_y="NEXT")
        pdf.ln(5)

        # Metrics table
        metrics_diff = getattr(comparison, "metrics_diff", {})
        effect_sizes = getattr(comparison, "effect_sizes", {})
        significance = getattr(comparison, "statistical_significance", {})

        if metrics_diff:
            pdf.set_font("Helvetica", "B", 13)
            pdf.cell(0, 8, "Metrics Comparison", new_x="LMARGIN", new_y="NEXT")
            pdf.ln(2)

            pdf.set_font("Helvetica", "B", 9)
            pdf.cell(45, 6, "Metric", border=1)
            pdf.cell(30, 6, "Difference", border=1)
            pdf.cell(30, 6, "Effect Size", border=1)
            pdf.cell(30, 6, "p-value", border=1)
            pdf.cell(25, 6, "Significant", border=1)
            pdf.ln()

            pdf.set_font("Helvetica", "", 9)
            for key, val in metrics_diff.items():
                pval = significance.get(key, 1.0)
                sig = "Yes" if isinstance(pval, float) and pval < 0.05 else "No"
                es = effect_sizes.get(key, 0.0)
                pdf.cell(45, 6, str(key)[:20], border=1)
                pdf.cell(30, 6, f"{val:+.4f}" if isinstance(val, float) else str(val), border=1)
                pdf.cell(30, 6, f"{es:.4f}" if isinstance(es, float) else str(es), border=1)
                pdf.cell(30, 6, f"{pval:.4f}" if isinstance(pval, float) else str(pval), border=1)
                pdf.cell(25, 6, sig, border=1)
                pdf.ln()
            pdf.ln(5)

        # Recommendation
        recommendation = getattr(comparison, "recommendation", "")
        if recommendation:
            pdf.set_font("Helvetica", "B", 13)
            pdf.cell(0, 8, "Recommendation", new_x="LMARGIN", new_y="NEXT")
            pdf.set_font("Helvetica", "", 10)
            for line in recommendation.split("\n"):
                pdf.cell(0, 6, line, new_x="LMARGIN", new_y="NEXT")

        return bytes(pdf.output())
