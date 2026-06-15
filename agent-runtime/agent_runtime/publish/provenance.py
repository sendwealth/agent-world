"""Provenance collector — turns an experiment directory into a structured
metadata record that travels with the published dataset.

The provenance schema is intentionally JSON-serialisable and aligned with the
DataCite 4.x fields Zenodo/Dataverse expect, plus Agent World-specific fields
(engine version, LLM provider/model, benchmark metrics, tick range).
"""

from __future__ import annotations

import json
import logging
from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import Any

logger = logging.getLogger(__name__)


@dataclass
class Provenance:
    """Structured provenance metadata for a published dataset.

    Every field is optional — the collector fills what it can find; the
    publishing flow only hard-requires ``title`` and ``creators`` which
    always have sensible defaults.
    """

    title: str = ""
    description: str = ""
    creators: list[dict[str, str]] = field(default_factory=list)
    keywords: list[str] = field(default_factory=list)
    license: str = "MIT"
    related_identifiers: list[dict[str, str]] = field(default_factory=list)
    # Agent World-specific
    engine_version: str = ""
    experiment_id: str = ""
    agent_count: int = 0
    skills: list[str] = field(default_factory=list)
    tick_range: dict[str, int] = field(default_factory=dict)
    llm_provider: str = ""
    llm_model: str = ""
    benchmark_metrics: dict[str, Any] = field(default_factory=dict)
    config_snapshot: dict[str, Any] = field(default_factory=dict)
    generated_at: str = ""

    def to_dict(self) -> dict[str, Any]:
        """Serialise to a JSON-safe dict (suitable for DataCite/Zenodo)."""
        return asdict(self)

    def to_json(self, *, indent: int = 2) -> str:
        """Serialise to a JSON string."""
        return json.dumps(self.to_dict(), indent=indent, ensure_ascii=False, default=str)


def _read_json(path: Path) -> dict[str, Any]:
    """Read a JSON file, returning ``{}`` on failure."""
    try:
        with path.open("r", encoding="utf-8") as fh:
            data = json.load(fh)
        if isinstance(data, dict):
            return data
    except (OSError, json.JSONDecodeError):
        logger.debug("Failed to read JSON from %s", path, exc_info=True)
    return {}


def _read_version_file(repo_root: Path) -> str:
    """Read the engine version from ``VERSION`` at the repo root.

    Falls back to "unknown" if not found.  The function is defensive so
    that publishing from a bare experiment directory (no repo context)
    still works.
    """
    candidates = [repo_root / "VERSION", repo_root.parent / "VERSION"]
    for candidate in candidates:
        try:
            return candidate.read_text(encoding="utf-8").strip()
        except OSError:
            continue
    return "unknown"


def collect_provenance(
    experiment_path: Path,
    *,
    repo_root: Path | None = None,
    title: str | None = None,
    creators: list[dict[str, str]] | None = None,
    description: str | None = None,
) -> Provenance:
    """Collect provenance metadata from an experiment directory.

    Args:
        experiment_path: Path to either a directory containing experiment
            artefacts or a ``report.json`` / ``reference.json`` file.
        repo_root: Optional repository root for reading ``VERSION``.
            If ``None``, the function walks up from ``experiment_path``.
        title: Optional dataset title override.
        creators: Optional creators list (``[{"name": "..."}]``).
        description: Optional description override.

    Returns:
        A populated :class:`Provenance`.
    """
    if experiment_path.is_file():
        experiment_dir = experiment_path.parent
        primary_file = experiment_path
    else:
        experiment_dir = experiment_path
        primary_file = experiment_dir / "report.json"
        if not primary_file.exists():
            primary_file = experiment_dir / "reference.json"

    root = repo_root or _find_repo_root(experiment_dir)
    engine_version = _read_version_file(root)

    primary = _read_json(primary_file)

    # Extract common fields from various known report shapes
    experiment_id = primary.get("experiment_id") or primary.get("scenario") or experiment_dir.name
    config = primary.get("config") or primary.get("config_snapshot") or {}
    agent_count = (
        config.get("agents")
        if isinstance(config.get("agents"), int)
        else primary.get("agent_count", 0)
    )
    if not isinstance(agent_count, int):
        agent_count = 0

    skills = _extract_skills(primary, experiment_dir)
    tick_range = _extract_tick_range(primary, config)
    llm_provider, llm_model = _extract_llm(config, primary)
    benchmark_metrics = _extract_benchmark_metrics(primary, experiment_dir)

    default_title = f"Agent World experiment: {experiment_id}"
    resolved_title = title or primary.get("title") or default_title
    resolved_creators = creators or [{"name": "Agent World"}]
    resolved_description = (
        description
        or primary.get("summary")
        or f"Reproducible artefacts for experiment {experiment_id}."
    )

    keywords = ["agent-world", "multi-agent-simulation", "emergence"]
    if benchmark_metrics:
        keywords.append("emergence-benchmark")

    return Provenance(
        title=resolved_title,
        description=str(resolved_description),
        creators=list(resolved_creators),
        keywords=keywords,
        license="MIT",
        related_identifiers=[],
        engine_version=engine_version,
        experiment_id=str(experiment_id),
        agent_count=agent_count,
        skills=list(skills),
        tick_range=tick_range,
        llm_provider=llm_provider,
        llm_model=llm_model,
        benchmark_metrics=benchmark_metrics,
        config_snapshot=config if isinstance(config, dict) else {},
        generated_at=primary.get("generated_at", ""),
    )


# ---------------------------------------------------------------------------
# Internal extractors
# ---------------------------------------------------------------------------


def _find_repo_root(start: Path) -> Path:
    """Walk up from ``start`` looking for a ``VERSION`` file."""
    for candidate in [start, *start.parents]:
        if (candidate / "VERSION").exists():
            return candidate
    return start


def _extract_skills(primary: dict[str, Any], experiment_dir: Path) -> list[str]:
    """Pull skill names from the report or skill-distribution file."""
    snapshot = primary.get("final_snapshot") or {}
    if isinstance(snapshot, dict):
        dist = snapshot.get("skill_distribution")
        if isinstance(dist, list) and dist:
            return [str(item.get("skill", item.get("name", ""))) for item in dist]
    # Fallback: look for a skills.json sidecar
    skills_file = experiment_dir / "skills.json"
    if skills_file.exists():
        data = _read_json(skills_file)
        if isinstance(data, list):
            return [str(item) for item in data]
        if isinstance(data, dict) and "skills" in data:
            return [str(s) for s in data["skills"]]
    return []


def _extract_tick_range(primary: dict[str, Any], config: dict[str, Any]) -> dict[str, int]:
    """Extract ``{"start": 0, "end": N}`` from the report/config."""
    end_tick = (
        primary.get("completed_ticks")
        or primary.get("duration_ticks")
        or config.get("ticks")
        or config.get("duration_ticks")
        or 0
    )
    try:
        end_tick_int = int(end_tick)
    except (TypeError, ValueError):
        end_tick_int = 0
    return {"start": 0, "end": end_tick_int}


def _extract_llm(config: dict[str, Any], primary: dict[str, Any]) -> tuple[str, str]:
    """Return ``(provider, model)`` from config or report."""
    llm_cfg = config.get("llm") if isinstance(config.get("llm"), dict) else None
    provider = ""
    model = ""
    if llm_cfg:
        provider = str(llm_cfg.get("provider", ""))
        model = str(llm_cfg.get("model", ""))
    if not provider:
        provider = str(primary.get("llm_provider", ""))
    if not model:
        model = str(primary.get("llm_model", ""))
    return provider, model


def _extract_benchmark_metrics(primary: dict[str, Any], experiment_dir: Path) -> dict[str, Any]:
    """Extract structured benchmark metrics from the report.

    The Phase 5.1 reference report has top-level metric objects
    (``diffusion``, ``network``, ``specialization``, ``inequality``,
    ``organization``, ``culture``).  We keep each as a sub-dict, dropping
    the human-readable ``interpretation`` text for brevity.
    """
    metric_keys = (
        "diffusion",
        "network",
        "specialization",
        "inequality",
        "organization",
        "culture",
        "linguistic",
    )
    out: dict[str, Any] = {}
    for key in metric_keys:
        val = primary.get(key)
        if isinstance(val, dict):
            out[key] = {k: v for k, v in val.items() if k != "interpretation"}
    if not out:
        # Look for a sibling benchmark JSON
        for candidate in experiment_dir.glob("*benchmark*.json"):
            data = _read_json(candidate)
            for key in metric_keys:
                val = data.get(key)
                if isinstance(val, dict):
                    out[key] = {k: v for k, v in val.items() if k != "interpretation"}
            if out:
                break
    return out
