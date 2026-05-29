#!/usr/bin/env python3
"""
Emergence Experiment Runner — one-command script for Phase 4.1-C.

Spawns N LLM-driven agents connected to a World Engine via Docker Compose,
monitors tick progress / token health / survival rate in real time, then
automatically collects data and produces a structured emergence report.

Usage:
    # One-command start (10 agents, 60 min, qwen3:8b)
    python scripts/emergence_experiment.py --agents 10 --duration 60m --model qwen3:8b

    # Dry-run — validate config without starting containers
    python scripts/emergence_experiment.py --agents 10 --duration 60m --model qwen3:8b --dry-run

    # Custom Ollama parallelism
    python scripts/emergence_experiment.py --agents 10 --duration 30m --ollama-parallel 5

    # Use external LLM provider
    python scripts/emergence_experiment.py --agents 5 --duration 10m --llm-provider openai --llm-model gpt-4o-mini

Requirements:
    - Docker & Docker Compose v2 (`docker compose`)
    - Ollama running locally (or --llm-provider openai/anthropic/zhipu)
    - Python 3.11+
    - curl (for health checks)
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import signal
import subprocess
import sys
import time
from collections import defaultdict
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Dict, List, Optional


# ── Constants ────────────────────────────────────────────────────────

PROJECT_ROOT = Path(__file__).resolve().parent.parent
COMPOSE_FILE = PROJECT_ROOT / "docker-compose-emergence.yml"
CONFIG_AGENTS_DIR = PROJECT_ROOT / "config" / "agents"
LOGS_BASE_DIR = PROJECT_ROOT / "logs"
DEFAULT_DURATION_MINUTES = 60
DEFAULT_AGENT_COUNT = 10
DEFAULT_MODEL = "qwen3:8b"
DEFAULT_TICK_INTERVAL = 1.0
OLLAMA_DEFAULT_URL = "http://localhost:11434"


# ── Data classes ─────────────────────────────────────────────────────

@dataclass
class ExperimentConfig:
    """All parameters that define an emergence experiment."""
    agents: int = DEFAULT_AGENT_COUNT
    duration_minutes: int = DEFAULT_DURATION_MINUTES
    model: str = DEFAULT_MODEL
    llm_provider: str = "ollama"
    llm_base_url: Optional[str] = None
    ollama_parallel: int = 4
    tick_interval: float = DEFAULT_TICK_INTERVAL
    engine_port: int = 8080
    grpc_port: int = 50051
    dashboard_port: int = 3001
    ollama_port: int = 11434
    dry_run: bool = False
    output_dir: Optional[str] = None
    skip_dashboard: bool = False

    @property
    def duration_seconds(self) -> int:
        return self.duration_minutes * 60

    @property
    def experiment_id(self) -> str:
        return datetime.now(timezone.utc).strftime("experiment-%Y%m%d-%H%M%S")

    @property
    def log_dir(self) -> Path:
        base = Path(self.output_dir) if self.output_dir else LOGS_BASE_DIR
        return base / self.experiment_id


@dataclass
class AgentStatus:
    """Status snapshot of a single agent."""
    name: str
    service_name: str
    healthy: bool = False
    tick: int = 0
    uptime_s: float = 0.0
    status: str = "unknown"


@dataclass
class ExperimentMetrics:
    """Collected metrics from the experiment run."""
    experiment_id: str = ""
    config: Dict[str, Any] = field(default_factory=dict)
    start_time: str = ""
    end_time: str = ""
    wall_time_s: float = 0.0
    total_ticks: int = 0
    agents_alive: int = 0
    agents_dead: int = 0
    agent_details: List[Dict[str, Any]] = field(default_factory=list)
    llm_calls_estimated: int = 0
    error_rate: float = 0.0
    survival_rate: float = 0.0
    verdict: str = "NEED-MORE-DATA"
    verdict_reason: str = ""
    notes: List[str] = field(default_factory=list)


# ── Helpers ──────────────────────────────────────────────────────────

def log(msg: str) -> None:
    ts = datetime.now(timezone.utc).strftime("%H:%M:%S")
    print(f"  [{ts}] {msg}", flush=True)


def run_cmd(
    cmd: List[str],
    *,
    check: bool = True,
    capture: bool = True,
    cwd: Optional[Path] = None,
    env: Optional[Dict[str, str]] = None,
) -> subprocess.CompletedProcess:
    """Run a subprocess command with sensible defaults."""
    merged_env = {**os.environ, **(env or {})}
    return subprocess.run(
        cmd,
        check=check,
        capture_output=capture,
        text=True,
        cwd=cwd or PROJECT_ROOT,
        env=merged_env,
    )


def parse_duration(val: str) -> int:
    """Parse a duration string like '60m', '1h', '90m', '3600s' into minutes."""
    m = re.match(r"^(\d+)(m|h|s)?$", val.strip().lower())
    if not m:
        raise ValueError(f"Invalid duration: {val!r} (use e.g. 60m, 1h, 3600s)")
    num = int(m.group(1))
    unit = m.group(2) or "m"
    if unit == "h":
        return num * 60
    elif unit == "s":
        return max(1, num // 60)
    return num


def check_docker() -> bool:
    """Verify Docker and Docker Compose are available."""
    try:
        run_cmd(["docker", "--version"], check=True)
    except (FileNotFoundError, subprocess.CalledProcessError):
        log("ERROR: Docker is not installed or not in PATH.")
        log("Install Docker: https://docs.docker.com/get-docker/")
        return False

    try:
        run_cmd(["docker", "compose", "version"], check=True)
    except (FileNotFoundError, subprocess.CalledProcessError):
        log("ERROR: Docker Compose v2 is not available.")
        log("Install Docker Compose: https://docs.docker.com/compose/install/")
        return False

    return True


def check_ollama(config: ExperimentConfig) -> bool:
    """Check if Ollama is reachable and the model is available."""
    if config.llm_provider != "ollama":
        return True  # Not needed for other providers

    base_url = config.llm_base_url or OLLAMA_DEFAULT_URL
    # Strip trailing slash
    base_url = base_url.rstrip("/")

    log(f"Checking Ollama at {base_url} ...")
    try:
        result = run_cmd(["curl", "-sf", f"{base_url}/api/tags"], check=True)
        data = json.loads(result.stdout)
        models = [m.get("name", "") for m in data.get("models", [])]
        model_name = config.model
        # Check for exact or tagless match (e.g. "llama3" matches "llama3:latest")
        found = any(m == model_name or m.startswith(model_name + ":") for m in models)
        if found:
            log(f"  Model '{model_name}' is available.")
            return True
        else:
            log(f"  Model '{model_name}' not found. Available: {models}")
            log(f"  Pull it with: ollama pull {model_name}")
            return False
    except (subprocess.CalledProcessError, json.JSONDecodeError) as exc:
        log(f"ERROR: Cannot reach Ollama at {base_url}")
        log(f"  Make sure Ollama is running: https://ollama.com")
        log(f"  Start it with: ollama serve")
        if config.llm_base_url is None:
            log(f"  Or set --llm-base-url if Ollama runs on a different host/port.")
        return False


def get_available_agent_configs(count: int) -> List[Path]:
    """Get the first N agent TOML config files, sorted by number."""
    configs = sorted(CONFIG_AGENTS_DIR.glob("agent-*.toml"))
    if len(configs) < count:
        log(f"WARNING: Only {len(configs)} agent configs found, requested {count}.")
        count = len(configs)
    return configs[:count]


# ── Docker Compose Generation ────────────────────────────────────────

def generate_compose_file(config: ExperimentConfig) -> Path:
    """Generate docker-compose-emergence.yml from the experiment config."""
    agent_configs = get_available_agent_configs(config.agents)
    actual_count = len(agent_configs)

    if actual_count == 0:
        log("ERROR: No agent config files found in config/agents/")
        sys.exit(1)

    lines: List[str] = []
    lines.append("# ── Agent World — Emergence Experiment ─────────────────────────")
    lines.append("# Auto-generated by emergence_experiment.py")
    lines.append(f"# Agents: {actual_count}, Duration: {config.duration_minutes}m, Model: {config.model}")
    lines.append("# ────────────────────────────────────────────────────────────────")
    lines.append("")
    lines.append("x-agent-common: &agent-common")
    lines.append("  build:")
    lines.append("    context: ./agent-runtime")
    lines.append("  depends_on:")
    lines.append("    world-engine:")
    lines.append("      condition: service_healthy")
    lines.append("    ollama:")
    lines.append("      condition: service_healthy")
    lines.append("  volumes:")
    lines.append("    - ./config/agents:/app/agent-configs:ro")
    lines.append(f"    - experiment-logs:/app/logs")
    lines.append("  environment:")

    ollama_url = "http://ollama:11434" if config.llm_provider == "ollama" else (config.llm_base_url or "")
    lines.append(f"    - WORLD_ENGINE_URL=http://world-engine:{config.engine_port}")
    lines.append(f"    - LLM_PROVIDER={config.llm_provider}")
    lines.append(f"    - LLM_MODEL={config.model}")
    if ollama_url:
        lines.append(f"    - OLLAMA_BASE_URL={ollama_url}")
        lines.append(f"    - LLM_BASE_URL={ollama_url}")
    lines.append("    - HEALTH_PORT=9090")
    lines.append("  restart: unless-stopped")
    lines.append("  healthcheck:")
    lines.append("    test: [\"CMD\", \"python\", \"-c\", \"import urllib.request; urllib.request.urlopen('http://localhost:9090/health')\"]")
    lines.append("    interval: 15s")
    lines.append("    timeout: 5s")
    lines.append("    retries: 3")
    lines.append("    start_period: 30s")
    lines.append("  networks:")
    lines.append("    - agent-world")
    lines.append("")
    lines.append("services:")

    # World Engine
    lines.append("  # ── World Engine (Rust) ──────────────────────────────────────")
    lines.append("  world-engine:")
    lines.append("    build:")
    lines.append("      context: ./world-engine")
    lines.append("    container_name: world-engine")
    lines.append("    ports:")
    lines.append(f"      - \"{config.engine_port}:{config.engine_port}\"")
    lines.append(f"      - \"{config.grpc_port}:{config.grpc_port}\"")
    lines.append("    volumes:")
    lines.append("      - world-data:/app/data")
    lines.append("      - ./config:/app/config:ro")
    lines.append("    environment:")
    lines.append("      - HOST=0.0.0.0")
    lines.append(f"      - PORT={config.engine_port}")
    lines.append(f"      - GRPC_ADDR=0.0.0.0:{config.grpc_port}")
    lines.append("      - RUST_LOG=info")
    lines.append("      - GENESIS_PATH=config/genesis.yaml")
    lines.append("    restart: unless-stopped")
    lines.append("    healthcheck:")
    lines.append(f"      test: [\"CMD\", \"curl\", \"-f\", \"http://localhost:{config.engine_port}/tasks\"]")
    lines.append("      interval: 10s")
    lines.append("      timeout: 5s")
    lines.append("      retries: 5")
    lines.append("      start_period: 30s")
    lines.append("    networks:")
    lines.append("      - agent-world")
    lines.append("")

    # Ollama
    lines.append("  # ── Ollama (local LLM) ────────────────────────────────────────")
    lines.append("  ollama:")
    lines.append("    image: ollama/ollama:latest")
    lines.append("    container_name: ollama")
    lines.append("    ports:")
    lines.append(f"      - \"{config.ollama_port}:11434\"")
    lines.append("    volumes:")
    lines.append("      - ollama-data:/root/.ollama")
    lines.append("    environment:")
    lines.append(f"      - OLLAMA_NUM_PARALLEL={config.ollama_parallel}")
    lines.append("    restart: unless-stopped")
    lines.append("    healthcheck:")
    lines.append("      test: [\"CMD\", \"curl\", \"-f\", \"http://localhost:11434/api/tags\"]")
    lines.append("      interval: 30s")
    lines.append("      timeout: 10s")
    lines.append("      retries: 3")
    lines.append("      start_period: 30s")
    lines.append("    networks:")
    lines.append("      - agent-world")
    lines.append("")

    # Agent services
    lines.append("  # ── Agent Runtimes (Python + LLM) ──────────────────────────────")
    for cfg_path in agent_configs:
        basename = cfg_path.stem  # e.g. agent-01-alice
        parts = basename.split("-", 2)
        num = parts[1] if len(parts) >= 2 else "01"
        name = parts[2] if len(parts) >= 3 else f"agent-{num}"
        service_name = f"agent-{num}"

        lines.append(f"  {service_name}:")
        lines.append(f"    <<: *agent-common")
        lines.append(f"    container_name: agent-{name}")
        lines.append(f"    command:")
        lines.append(f"      - spawn")
        lines.append(f"      - --config")
        lines.append(f"      - /app/agent-configs/{basename}.toml")
        lines.append(f"      - --world-url")
        lines.append(f"      - http://world-engine:{config.engine_port}")
        lines.append(f"      - --tick-interval")
        lines.append(f"      - \"{config.tick_interval}\"")
        lines.append("")

    # Dashboard
    if not config.skip_dashboard:
        lines.append("  # ── Dashboard (Next.js) ──────────────────────────────────────")
        lines.append("  dashboard:")
        lines.append("    build:")
        lines.append("      context: ./dashboard")
        lines.append("      args:")
        lines.append(f"        WORLD_ENGINE_URL: http://world-engine:{config.engine_port}")
        lines.append("    container_name: dashboard")
        lines.append("    ports:")
        lines.append(f"      - \"{config.dashboard_port}:3000\"")
        lines.append("    depends_on:")
        lines.append("      world-engine:")
        lines.append("        condition: service_healthy")
        lines.append("    environment:")
        lines.append(f"      - WORLD_ENGINE_URL=http://world-engine:{config.engine_port}")
        lines.append("      - PORT=3000")
        lines.append("    restart: unless-stopped")
        lines.append("    healthcheck:")
        lines.append("      test: [\"CMD\", \"node\", \"-e\", \"const http = require('http'); http.get('http://localhost:3000/', (r) => { process.exit(r.statusCode < 500 ? 0 : 1); }).on('error', () => process.exit(1));\"]")
        lines.append("      interval: 15s")
        lines.append("      timeout: 5s")
        lines.append("      retries: 3")
        lines.append("      start_period: 30s")
        lines.append("    networks:")
        lines.append("      - agent-world")
        lines.append("")

    # Volumes & Networks
    lines.append("volumes:")
    lines.append("  world-data:")
    lines.append("  experiment-logs:")
    lines.append("  ollama-data:")
    lines.append("")
    lines.append("networks:")
    lines.append("  agent-world:")
    lines.append("    driver: bridge")
    lines.append("")

    content = "\n".join(lines)
    COMPOSE_FILE.write_text(content)
    return COMPOSE_FILE


# ── Monitoring ───────────────────────────────────────────────────────

def fetch_agent_health(config: ExperimentConfig, service_name: str, container_name: str) -> AgentStatus:
    """Fetch health status from an agent container."""
    status = AgentStatus(name=container_name.replace("agent-", ""), service_name=service_name)

    try:
        result = run_cmd(
            ["docker", "exec", container_name, "python", "-c",
             "import urllib.request,json; r=urllib.request.urlopen('http://localhost:9090/health'); print(r.read().decode())"],
            check=False, capture=True,
        )
        if result.returncode == 0 and result.stdout.strip():
            data = json.loads(result.stdout.strip())
            status.healthy = data.get("status") == "running"
            status.tick = data.get("tick", 0)
            status.uptime_s = data.get("uptime_s", 0.0)
            status.status = data.get("status", "unknown")
    except Exception:
        pass

    return status


def monitor_loop(config: ExperimentConfig, agent_info: List[tuple]) -> None:
    """Real-time monitoring loop until duration expires or user interrupts."""
    start = time.monotonic()
    remaining = config.duration_seconds
    report_interval = 60  # Print status every 60s
    next_report = start + report_interval

    log("=" * 60)
    log(f"  Experiment running: {config.agents} agents, {config.duration_minutes}m")
    log(f"  Press Ctrl+C to stop early and collect data")
    log("=" * 60)

    try:
        while True:
            now = time.monotonic()
            elapsed = now - start
            remaining = config.duration_seconds - elapsed

            if remaining <= 0:
                log("Duration expired — stopping experiment.")
                break

            # Periodic status report
            if now >= next_report:
                alive = 0
                total_ticks = 0
                for service_name, container_name in agent_info:
                    st = fetch_agent_health(config, service_name, container_name)
                    if st.healthy:
                        alive += 1
                        total_ticks += st.tick

                mins_left = remaining / 60
                log(f"  Status: {alive}/{config.agents} healthy | "
                    f"Total ticks: {total_ticks} | "
                    f"Remaining: {mins_left:.1f}m")
                next_report = now + report_interval

            time.sleep(5)

    except KeyboardInterrupt:
        log("Interrupted by user — collecting data before shutdown.")


# ── Data Collection ──────────────────────────────────────────────────

def collect_metrics(config: ExperimentConfig, agent_info: List[tuple]) -> ExperimentMetrics:
    """Collect metrics from all running containers."""
    metrics = ExperimentMetrics(
        experiment_id=config.experiment_id,
        config={
            "agents": config.agents,
            "duration_minutes": config.duration_minutes,
            "model": config.model,
            "llm_provider": config.llm_provider,
            "ollama_parallel": config.ollama_parallel,
            "tick_interval": config.tick_interval,
        },
        start_time=datetime.now(timezone.utc).isoformat(),
    )

    total_ticks = 0
    alive_count = 0
    dead_count = 0
    total_errors = 0

    for service_name, container_name in agent_info:
        agent_detail: Dict[str, Any] = {
            "service": service_name,
            "container": container_name,
        }

        # Health check
        st = fetch_agent_health(config, service_name, container_name)
        agent_detail["healthy"] = st.healthy
        agent_detail["tick"] = st.tick
        agent_detail["uptime_s"] = round(st.uptime_s, 1)
        agent_detail["status"] = st.status

        if st.healthy:
            alive_count += 1
        else:
            dead_count += 1

        total_ticks += st.tick

        # Docker logs — count errors
        try:
            result = run_cmd(
                ["docker", "logs", "--tail", "500", container_name],
                check=False, capture=True,
            )
            log_output = result.stdout + result.stderr
            error_count = log_output.count('"level":"ERROR"')
            error_count += log_output.count("ERROR:")
            agent_detail["errors"] = error_count
            total_errors += error_count

            # Count LLM calls from log lines containing LLM provider info
            llm_calls = log_output.count('"event":"llm_call"')
            llm_calls += log_output.count("LLM provider created")
            agent_detail["llm_calls_approx"] = llm_calls
        except Exception:
            agent_detail["errors"] = -1
            agent_detail["llm_calls_approx"] = 0

        metrics.agent_details.append(agent_detail)

    metrics.agents_alive = alive_count
    metrics.agents_dead = dead_count
    metrics.total_ticks = total_ticks
    metrics.error_rate = total_errors / max(1, total_ticks) if total_ticks > 0 else 0.0
    metrics.survival_rate = alive_count / max(1, config.agents)
    metrics.llm_calls_estimated = sum(
        d.get("llm_calls_approx", 0) for d in metrics.agent_details
    )
    metrics.end_time = datetime.now(timezone.utc).isoformat()

    # Verdict logic
    if metrics.survival_rate >= 0.8 and metrics.total_ticks > 1000 and metrics.error_rate < 0.05:
        metrics.verdict = "GO"
        metrics.verdict_reason = (
            f"High survival ({metrics.survival_rate:.0%}), "
            f"sufficient ticks ({metrics.total_ticks}), "
            f"low error rate ({metrics.error_rate:.2%}). "
            "Data quality sufficient for emergence analysis."
        )
    elif metrics.survival_rate < 0.3 or metrics.total_ticks < 100:
        metrics.verdict = "NO-GO"
        metrics.verdict_reason = (
            f"Low survival ({metrics.survival_rate:.0%}) or "
            f"insufficient ticks ({metrics.total_ticks}). "
            "System may not be stable enough for emergence observation."
        )
    else:
        metrics.verdict = "NEED-MORE-DATA"
        metrics.verdict_reason = (
            f"Survival {metrics.survival_rate:.0%}, ticks {metrics.total_ticks}. "
            "Results are inconclusive — recommend extending duration or adjusting parameters."
        )

    return metrics


# ── Report Generation ────────────────────────────────────────────────

def write_json_report(metrics: ExperimentMetrics, output_dir: Path) -> Path:
    """Write machine-readable JSON metrics report."""
    report_path = output_dir / "metrics.json"
    report_path.parent.mkdir(parents=True, exist_ok=True)

    data = {
        "experiment_id": metrics.experiment_id,
        "config": metrics.config,
        "timing": {
            "start": metrics.start_time,
            "end": metrics.end_time,
            "wall_time_s": round(metrics.wall_time_s, 2),
        },
        "summary": {
            "total_ticks": metrics.total_ticks,
            "agents_alive": metrics.agents_alive,
            "agents_dead": metrics.agents_dead,
            "survival_rate": round(metrics.survival_rate, 4),
            "error_rate": round(metrics.error_rate, 6),
            "llm_calls_estimated": metrics.llm_calls_estimated,
        },
        "agents": metrics.agent_details,
        "verdict": {
            "decision": metrics.verdict,
            "reason": metrics.verdict_reason,
        },
        "notes": metrics.notes,
    }

    report_path.write_text(json.dumps(data, indent=2, ensure_ascii=False))
    return report_path


def write_markdown_report(metrics: ExperimentMetrics, output_dir: Path) -> Path:
    """Write human-readable Markdown emergence observation report."""
    report_path = output_dir / "emergence-report.md"
    report_path.parent.mkdir(parents=True, exist_ok=True)

    lines: List[str] = []
    lines.append(f"# Emergence Observation Report")
    lines.append(f"")
    lines.append(f"**Experiment ID**: `{metrics.experiment_id}`  ")
    lines.append(f"**Date**: {metrics.start_time}  ")
    lines.append(f"**Verdict**: **{metrics.verdict}**  ")
    lines.append(f"")
    lines.append(f"---")
    lines.append(f"")
    lines.append(f"## 1. Experiment Configuration")
    lines.append(f"")
    lines.append(f"| Parameter | Value |")
    lines.append(f"|---|---|")
    for key, val in metrics.config.items():
        lines.append(f"| {key} | {val} |")
    lines.append(f"| wall_time | {metrics.wall_time_s:.0f}s ({metrics.wall_time_s / 60:.1f}m) |")
    lines.append(f"")
    lines.append(f"## 2. Running Statistics")
    lines.append(f"")
    lines.append(f"| Metric | Value |")
    lines.append(f"|---|---|")
    lines.append(f"| Total ticks | {metrics.total_ticks} |")
    lines.append(f"| Agents alive | {metrics.agents_alive} |")
    lines.append(f"| Agents dead | {metrics.agents_dead} |")
    lines.append(f"| Survival rate | {metrics.survival_rate:.1%} |")
    lines.append(f"| Error rate | {metrics.error_rate:.4%} |")
    lines.append(f"| LLM calls (est.) | {metrics.llm_calls_estimated} |")
    lines.append(f"| Avg ticks/agent | {metrics.total_ticks / max(1, metrics.agents_alive):.0f} |")
    lines.append(f"")
    lines.append(f"## 3. Per-Agent Details")
    lines.append(f"")
    lines.append(f"| Agent | Service | Healthy | Ticks | Uptime | Errors | LLM Calls |")
    lines.append(f"|---|---|---|---|---|---|---|")
    for d in metrics.agent_details:
        lines.append(
            f"| {d.get('container', '?')} | {d.get('service', '?')} "
            f"| {'Yes' if d.get('healthy') else 'No'} "
            f"| {d.get('tick', 0)} "
            f"| {d.get('uptime_s', 0):.0f}s "
            f"| {d.get('errors', 0)} "
            f"| {d.get('llm_calls_approx', 0)} |"
        )
    lines.append(f"")

    lines.append(f"## 4. Emergence Behavior Observations")
    lines.append(f"")
    lines.append(f"> **Note**: The following sections should be completed manually after")
    lines.append(f"> reviewing dashboard screenshots, interaction logs, and agent behavior")
    lines.append(f"> patterns. Use the data above as a starting point for analysis.")
    lines.append(f"")
    lines.append(f"### 4.1 Clustering Patterns")
    lines.append(f"")
    lines.append(f"- [ ] Did agents form spatial or social clusters?")
    lines.append(f"- [ ] Were there leader-follower dynamics?")
    lines.append(f"- [ ] Did agents specialize in specific roles?")
    lines.append(f"")
    lines.append(f"### 4.2 Cooperation Patterns")
    lines.append(f"")
    lines.append(f"- [ ] Did agents engage in reciprocal trading?")
    lines.append(f"- [ ] Were there instances of teaching/mentorship?")
    lines.append(f"- [ ] Did agents form organizations or alliances?")
    lines.append(f"- [ ] Were there collective problem-solving behaviors?")
    lines.append(f"")
    lines.append(f"### 4.3 Emergence Events")
    lines.append(f"")
    lines.append(f"- [ ] Unexpected behaviors not programmed explicitly")
    lines.append(f"- [ ] Communication pattern evolution")
    lines.append(f"- [ ] Economic bubbles, crashes, or equilibria")
    lines.append(f"- [ ] Cultural transmission or norm formation")
    lines.append(f"- [ ] Trust network formation or betrayal events")
    lines.append(f"")
    lines.append(f"### 4.4 Interaction Graph")
    lines.append(f"")
    lines.append(f"- Attach screenshots from the dashboard")
    lines.append(f"- Describe the communication topology")
    lines.append(f"- Note any phase transitions in behavior")
    lines.append(f"")
    lines.append(f"## 5. Verdict")
    lines.append(f"")
    lines.append(f"**Decision**: {metrics.verdict}")
    lines.append(f"")
    lines.append(f"**Reasoning**: {metrics.verdict_reason}")
    lines.append(f"")
    lines.append(f"| Criterion | Threshold | Actual | Pass? |")
    lines.append(f"|---|---|---|---|")
    lines.append(f"| Survival rate | >= 80% | {metrics.survival_rate:.1%} | {'Yes' if metrics.survival_rate >= 0.8 else 'No'} |")
    lines.append(f"| Min total ticks | > 1000 | {metrics.total_ticks} | {'Yes' if metrics.total_ticks > 1000 else 'No'} |")
    lines.append(f"| Error rate | < 5% | {metrics.error_rate:.2%} | {'Yes' if metrics.error_rate < 0.05 else 'No'} |")
    lines.append(f"")
    lines.append(f"## 6. Next Steps")
    lines.append(f"")
    if metrics.verdict == "GO":
        lines.append(f"- [ ] Analyze interaction graphs for emergent patterns")
        lines.append(f"- [ ] Extract and classify agent communication patterns")
        lines.append(f"- [ ] Map the economic network topology")
        lines.append(f"- [ ] Run follow-up experiments with varied parameters")
        lines.append(f"- [ ] Document emergent behaviors for Phase 4 design review")
    elif metrics.verdict == "NO-GO":
        lines.append(f"- [ ] Investigate root cause of low survival / insufficient ticks")
        lines.append(f"- [ ] Check Ollama performance (consider smaller model or more parallelism)")
        lines.append(f"- [ ] Adjust token economy parameters in genesis.yaml")
        lines.append(f"- [ ] Consider shorter tick intervals or more initial tokens")
    else:
        lines.append(f"- [ ] Extend experiment duration to collect more data")
        lines.append(f"- [ ] Monitor for patterns that emerge over longer time scales")
        lines.append(f"- [ ] Consider running multiple shorter experiments")
    lines.append(f"")
    lines.append(f"---")
    lines.append(f"*Generated by `scripts/emergence_experiment.py`*")

    report_path.write_text("\n".join(lines))
    return report_path


# ── Docker Compose Commands ──────────────────────────────────────────

def compose_up(config: ExperimentConfig) -> None:
    """Start all services via docker compose."""
    log("Starting Docker Compose services ...")
    run_cmd(
        ["docker", "compose", "-f", str(COMPOSE_FILE), "up", "-d", "--build"],
        check=True, cwd=PROJECT_ROOT,
    )
    log("Services started. Waiting for health checks ...")
    time.sleep(10)

    # Wait for world-engine to be healthy
    for attempt in range(30):
        try:
            result = run_cmd(
                ["curl", "-sf", f"http://localhost:{config.engine_port}/tasks"],
                check=True,
            )
            log(f"World Engine is healthy (attempt {attempt + 1}).")
            break
        except subprocess.CalledProcessError:
            if attempt < 29:
                time.sleep(5)
            else:
                log("WARNING: World Engine health check timed out.")

    # Pull model into Ollama container if using Ollama
    if config.llm_provider == "ollama":
        log(f"Pulling model '{config.model}' into Ollama container ...")
        try:
            run_cmd(
                ["docker", "exec", "ollama", "ollama", "pull", config.model],
                check=True,
            )
            log(f"Model '{config.model}' pulled successfully.")
        except subprocess.CalledProcessError:
            log(f"WARNING: Failed to pull model '{config.model}' — it may already be present.")


def compose_down(config: ExperimentConfig) -> None:
    """Stop and remove all containers, preserving logs."""
    log("Stopping Docker Compose services ...")
    run_cmd(
        ["docker", "compose", "-f", str(COMPOSE_FILE), "down", "--remove-orphans"],
        check=False, cwd=PROJECT_ROOT,
    )
    log("Services stopped.")


def get_container_names(config: ExperimentConfig) -> List[tuple]:
    """Get (service_name, container_name) for all agent services."""
    agent_configs = get_available_agent_configs(config.agents)
    result = []
    for cfg_path in agent_configs:
        basename = cfg_path.stem
        parts = basename.split("-", 2)
        num = parts[1] if len(parts) >= 2 else "01"
        name = parts[2] if len(parts) >= 3 else f"agent-{num}"
        service_name = f"agent-{num}"
        container_name = f"agent-{name}"
        result.append((service_name, container_name))
    return result


# ── Main ─────────────────────────────────────────────────────────────

def build_arg_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Agent World — Emergence Experiment Runner",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=(
            "Examples:\n"
            "  python scripts/emergence_experiment.py --agents 10 --duration 60m --model qwen3:8b\n"
            "  python scripts/emergence_experiment.py --agents 5 --duration 30m --dry-run\n"
            "  python scripts/emergence_experiment.py --agents 10 --duration 1h --ollama-parallel 8\n"
        ),
    )

    parser.add_argument(
        "--agents", type=int, default=DEFAULT_AGENT_COUNT,
        help=f"Number of agents to spawn (default: {DEFAULT_AGENT_COUNT})",
    )
    parser.add_argument(
        "--duration", type=str, default=f"{DEFAULT_DURATION_MINUTES}m",
        help="Experiment duration (e.g. 60m, 1h, 3600s). Default: 60m",
    )
    parser.add_argument(
        "--model", type=str, default=DEFAULT_MODEL,
        help=f"LLM model name (default: {DEFAULT_MODEL})",
    )
    parser.add_argument(
        "--llm-provider", type=str, default="ollama",
        choices=["ollama", "openai", "anthropic", "zhipu"],
        help="LLM provider (default: ollama)",
    )
    parser.add_argument(
        "--llm-base-url", type=str, default=None,
        help="LLM API base URL (auto-detected for Ollama)",
    )
    parser.add_argument(
        "--ollama-parallel", type=int, default=4,
        help="Ollama OLLAMA_NUM_PARALLEL setting (default: 4)",
    )
    parser.add_argument(
        "--tick-interval", type=float, default=DEFAULT_TICK_INTERVAL,
        help="Seconds between agent ticks (default: 1.0)",
    )
    parser.add_argument(
        "--engine-port", type=int, default=8080,
        help="World Engine REST API port (default: 8080)",
    )
    parser.add_argument(
        "--grpc-port", type=int, default=50051,
        help="World Engine gRPC port (default: 50051)",
    )
    parser.add_argument(
        "--dashboard-port", type=int, default=3001,
        help="Dashboard port (default: 3001)",
    )
    parser.add_argument(
        "--ollama-port", type=int, default=11434,
        help="Ollama port (default: 11434)",
    )
    parser.add_argument(
        "--output-dir", type=str, default=None,
        help="Output directory for logs and reports (default: ./logs/experiment-YYYYMMDD-HHMMSS/)",
    )
    parser.add_argument(
        "--skip-dashboard", action="store_true",
        help="Do not start the dashboard service",
    )
    parser.add_argument(
        "--dry-run", action="store_true",
        help="Validate configuration and generate compose file without starting containers",
    )

    return parser


def main() -> int:
    parser = build_arg_parser()
    args = parser.parse_args()

    # Parse duration
    try:
        duration_min = parse_duration(args.duration)
    except ValueError as exc:
        log(f"ERROR: {exc}")
        return 1

    # Build config
    config = ExperimentConfig(
        agents=args.agents,
        duration_minutes=duration_min,
        model=args.model,
        llm_provider=args.llm_provider,
        llm_base_url=args.llm_base_url,
        ollama_parallel=args.ollama_parallel,
        tick_interval=args.tick_interval,
        engine_port=args.engine_port,
        grpc_port=args.grpc_port,
        dashboard_port=args.dashboard_port,
        ollama_port=args.ollama_port,
        dry_run=args.dry_run,
        output_dir=args.output_dir,
        skip_dashboard=args.skip_dashboard,
    )

    print()
    print("=" * 60)
    print("  Agent World — Emergence Experiment")
    print("=" * 60)
    print(f"  Agents:        {config.agents}")
    print(f"  Duration:      {config.duration_minutes}m ({config.duration_seconds}s)")
    print(f"  Model:         {config.model}")
    print(f"  LLM Provider:  {config.llm_provider}")
    print(f"  Ollama Parallel: {config.ollama_parallel}")
    print(f"  Tick Interval: {config.tick_interval}s")
    print(f"  Output:        {config.log_dir}")
    print(f"  Dry Run:       {config.dry_run}")
    print("=" * 60)
    print()

    # ── Pre-flight checks ──

    log("Running pre-flight checks ...")

    if not check_docker():
        return 1

    if not check_ollama(config):
        return 1

    agent_configs = get_available_agent_configs(config.agents)
    if not agent_configs:
        log("ERROR: No agent config files found in config/agents/")
        return 1

    actual_count = len(agent_configs)
    if actual_count < config.agents:
        log(f"WARNING: Only {actual_count} agent configs available, adjusting from {config.agents}.")
        config.agents = actual_count

    log(f"Pre-flight passed: {config.agents} agents configured.")

    # ── Generate compose file ──

    log("Generating docker-compose-emergence.yml ...")
    compose_path = generate_compose_file(config)
    log(f"  Written to: {compose_path}")

    # ── Dry-run mode ──

    if config.dry_run:
        log("")
        log("DRY RUN — validating configuration:")
        log(f"  [OK] Docker available")
        if config.llm_provider == "ollama":
            log(f"  [OK] Ollama reachable with model '{config.model}'")
        log(f"  [OK] {config.agents} agent configs found")
        log(f"  [OK] docker-compose-emergence.yml generated")
        log(f"  [OK] Output directory: {config.log_dir}")

        # Validate compose file syntax
        try:
            run_cmd(
                ["docker", "compose", "-f", str(COMPOSE_FILE), "config", "--quiet"],
                check=True, cwd=PROJECT_ROOT,
            )
            log(f"  [OK] Docker Compose file syntax valid")
        except subprocess.CalledProcessError as exc:
            log(f"  [FAIL] Docker Compose file has errors: {exc.stderr}")
            return 1

        log("")
        log("Dry run complete — all checks passed. Run without --dry-run to start.")
        return 0

    # ── Create output directory ──

    config.log_dir.mkdir(parents=True, exist_ok=True)
    log(f"Logs will be saved to: {config.log_dir}")

    # ── Start services ──

    agent_info = get_container_names(config)

    try:
        compose_up(config)

        # ── Monitoring loop ──

        monitor_start = time.monotonic()
        monitor_loop(config, agent_info)
        monitor_end = time.monotonic()

        # ── Collect metrics ──

        log("Collecting metrics from all containers ...")
        metrics = collect_metrics(config, agent_info)
        metrics.wall_time_s = monitor_end - monitor_start

        # ── Save docker logs ──

        log("Saving container logs ...")
        logs_dir = config.log_dir / "docker-logs"
        logs_dir.mkdir(parents=True, exist_ok=True)

        for service_name, container_name in agent_info:
            try:
                result = run_cmd(
                    ["docker", "logs", container_name],
                    check=False, capture=True,
                )
                log_file = logs_dir / f"{container_name}.log"
                log_file.write_text(result.stdout + result.stderr)
            except Exception:
                pass

        # World engine logs
        try:
            result = run_cmd(
                ["docker", "logs", "world-engine"],
                check=False, capture=True,
            )
            (logs_dir / "world-engine.log").write_text(result.stdout + result.stderr)
        except Exception:
            pass

        # ── Write reports ──

        log("Generating reports ...")
        json_path = write_json_report(metrics, config.log_dir)
        md_path = write_markdown_report(metrics, config.log_dir)

        # Generate rich HTML report with charts via ExperimentReporter
        try:
            from agent_runtime.experiment.report import ExperimentReporter, ExperimentResult

            exp_result = ExperimentResult(
                experiment_id=metrics.experiment_id,
                config_snapshot=metrics.config,
                duration_ticks=config.duration_seconds,
                completed_ticks=metrics.total_ticks,
                agent_count=config.agents,
                final_snapshot={
                    "agents_alive": metrics.agents_alive,
                    "alive_count": metrics.agents_alive,
                    "skill_distribution": [],
                },
                metrics_timeline=[
                    {
                        "tick": i,
                        "gdp": 0,
                        "gini": 0,
                        "population": metrics.agents_alive,
                    }
                    for i in range(0, metrics.total_ticks, max(1, metrics.total_ticks // 20))
                ],
                emergence_events=[],
                errors=metrics.notes,
                started_at=metrics.start_time,
                finished_at=metrics.end_time,
            )

            reporter = ExperimentReporter()
            rich_html = reporter.generate_rich_report(exp_result, format="html")
            html_path = config.log_dir / "experiment-report.html"
            html_path.write_text(rich_html, encoding="utf-8")
            log(f"  Rich HTML report: {html_path}")
        except Exception as exc:
            log(f"  WARNING: Rich report generation failed: {exc}")

        log(f"  JSON report: {json_path}")
        log(f"  Markdown report: {md_path}")

        # ── Print summary ──

        print()
        print("=" * 60)
        print("  EXPERIMENT COMPLETE")
        print("=" * 60)
        print(f"  Duration:    {metrics.wall_time_s:.0f}s ({metrics.wall_time_s / 60:.1f}m)")
        print(f"  Total ticks: {metrics.total_ticks}")
        print(f"  Alive:       {metrics.agents_alive}/{config.agents}")
        print(f"  Survival:    {metrics.survival_rate:.1%}")
        print(f"  Error rate:  {metrics.error_rate:.4%}")
        print(f"  LLM calls:   {metrics.llm_calls_estimated} (estimated)")
        print(f"  Verdict:     {metrics.verdict}")
        print(f"  Reports:     {config.log_dir}")
        print("=" * 60)
        print()

        return 0

    except KeyboardInterrupt:
        log("Interrupted — performing emergency data collection ...")
        try:
            metrics = collect_metrics(config, agent_info)
            metrics.wall_time_s = time.monotonic() - monitor_start if 'monitor_start' in dir() else 0
            write_json_report(metrics, config.log_dir)
            write_markdown_report(metrics, config.log_dir)
            log(f"Partial reports saved to: {config.log_dir}")
        except Exception:
            log("WARNING: Could not collect metrics during emergency shutdown.")
        return 1

    finally:
        compose_down(config)


if __name__ == "__main__":
    sys.exit(main())
