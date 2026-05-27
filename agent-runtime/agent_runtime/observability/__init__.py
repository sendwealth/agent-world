"""Observability module — OpenTelemetry tracing + Prometheus metrics for agent-runtime.

Provides:
- ``setup_telemetry`` — one-call init that wires up OTLP tracing and Prometheus metrics.
- ``Metrics`` — convenience wrapper exposing counters/gauges/histograms.
- ``trace_think_loop`` — decorator/context-manager for instrumenting the think-loop phases.

Usage::

    from agent_runtime.observability import setup_telemetry, metrics, trace_phase

    setup_telemetry(service_name="agent-alice", otlp_endpoint="http://otel-collector:4317")

    with trace_phase("perceive"):
        perception = await perceive(...)

    metrics.think_duration.observe(0.42)
"""

from __future__ import annotations

import logging
import os
import time
from contextlib import contextmanager
from dataclasses import dataclass
from typing import Any, Generator

logger = logging.getLogger(__name__)

# ---------------------------------------------------------------------------
# Lazy imports — OTel is optional at runtime
# ---------------------------------------------------------------------------

_tracer: Any = None
_meter: Any = None
_initialized: bool = False


# ---------------------------------------------------------------------------
# Prometheus-compatible metrics (pure-Python, no dependency on prometheus_client)
# ---------------------------------------------------------------------------

@dataclass
class _Counter:
    """Simple monotonically-increasing counter."""
    name: str
    value: int = 0

    def inc(self, n: int = 1) -> None:
        self.value += n


@dataclass
class _Gauge:
    """Simple gauge that can go up and down."""
    name: str
    value: float = 0.0

    def set(self, v: float) -> None:
        self.value = v

    def inc(self, n: float = 1.0) -> None:
        self.value += n

    def dec(self, n: float = 1.0) -> None:
        self.value -= n


@dataclass
class _Histogram:
    """Simple histogram backed by a list of observations."""
    name: str
    _observations: list[float] | None = None

    def __post_init__(self) -> None:
        if self._observations is None:
            self._observations = []

    def observe(self, v: float) -> None:
        self._observations.append(v)  # type: ignore[union-attr]

    @property
    def count(self) -> int:
        return len(self._observations or [])  # type: ignore[arg-type]

    @property
    def sum(self) -> float:
        return sum(self._observations or [])  # type: ignore[arg-type]


class Metrics:
    """Container for agent-runtime Prometheus metrics.

    Instances are created by ``setup_telemetry`` and exposed as the module-level
    ``metrics`` singleton.
    """

    def __init__(self) -> None:
        # Counters
        self.think_ticks = _Counter("agent_think_ticks_total")
        self.llm_calls = _Counter("agent_llm_calls_total")
        self.llm_tokens_used = _Counter("agent_llm_tokens_used_total")
        self.messages_sent = _Counter("agent_messages_sent_total")
        self.messages_received = _Counter("agent_messages_received_total")
        self.tasks_completed = _Counter("agent_tasks_completed_total")
        self.tasks_claimed = _Counter("agent_tasks_claimed_total")
        self.errors_total = _Counter("agent_errors_total")
        self.survival_actions = _Counter("agent_survival_actions_total")

        # Gauges
        self.tokens_balance = _Gauge("agent_tokens_balance")
        self.money_balance = _Gauge("agent_money_balance")
        self.health = _Gauge("agent_health")
        self.memory_size_bytes = _Gauge("agent_memory_size_bytes")

        # Histograms
        self.think_duration = _Histogram("agent_think_duration_seconds")
        self.perceive_duration = _Histogram("agent_perceive_duration_seconds")
        self.decide_duration = _Histogram("agent_decide_duration_seconds")
        self.act_duration = _Histogram("agent_act_duration_seconds")
        self.llm_latency = _Histogram("agent_llm_latency_seconds")

    def snapshot(self) -> dict[str, Any]:
        """Return a dict of all metric values for Prometheus exposition."""
        return {
            self.think_ticks.name: self.think_ticks.value,
            self.llm_calls.name: self.llm_calls.value,
            self.llm_tokens_used.name: self.llm_tokens_used.value,
            self.messages_sent.name: self.messages_sent.value,
            self.messages_received.name: self.messages_received.value,
            self.tasks_completed.name: self.tasks_completed.value,
            self.tasks_claimed.name: self.tasks_claimed.value,
            self.errors_total.name: self.errors_total.value,
            self.survival_actions.name: self.survival_actions.value,
            self.tokens_balance.name: self.tokens_balance.value,
            self.money_balance.name: self.money_balance.value,
            self.health.name: self.health.value,
            self.memory_size_bytes.name: self.memory_size_bytes.value,
            f"{self.think_duration.name}_count": self.think_duration.count,
            f"{self.think_duration.name}_sum": self.think_duration.sum,
            f"{self.perceive_duration.name}_count": self.perceive_duration.count,
            f"{self.perceive_duration.name}_sum": self.perceive_duration.sum,
            f"{self.decide_duration.name}_count": self.decide_duration.count,
            f"{self.decide_duration.name}_sum": self.decide_duration.sum,
            f"{self.act_duration.name}_count": self.act_duration.count,
            f"{self.act_duration.name}_sum": self.act_duration.sum,
            f"{self.llm_latency.name}_count": self.llm_latency.count,
            f"{self.llm_latency.name}_sum": self.llm_latency.sum,
        }

    def render_prometheus(self) -> str:
        """Render metrics in Prometheus text exposition format."""
        lines: list[str] = []
        snap = self.snapshot()
        for name, value in snap.items():
            if isinstance(value, int):
                lines.append(f"# TYPE {name} counter")
                lines.append(f"{name} {value}")
            else:
                lines.append(f"# TYPE {name} gauge")
                lines.append(f"{name} {value}")
        return "\n".join(lines) + "\n"


# Module-level singleton — safe to import before ``setup_telemetry``.
metrics = Metrics()


# ---------------------------------------------------------------------------
# Setup
# ---------------------------------------------------------------------------

def setup_telemetry(
    service_name: str = "agent-runtime",
    otlp_endpoint: str | None = None,
    enable_tracing: bool = True,
    enable_metrics: bool = True,
) -> None:
    """Initialise OpenTelemetry tracing and metrics.

    This is a no-op when OTel packages are not installed so the agent-runtime
    works without them in dev/test mode.

    Args:
        service_name: Logical service name for traces and metrics.
        otlp_endpoint: OTLP gRPC endpoint (e.g. ``http://otel-collector:4317``).
            Falls back to ``OTEL_EXPORTER_OTLP_ENDPOINT`` env var.
        enable_tracing: Whether to set up tracing.
        enable_metrics: Whether to set up metrics.
    """
    global _tracer, _meter, _initialized

    if _initialized:
        return

    endpoint = otlp_endpoint or os.environ.get("OTEL_EXPORTER_OTLP_ENDPOINT")

    if not endpoint:
        logger.info(
            "Observability: no OTLP endpoint configured — running with built-in metrics only"
        )
        _initialized = True
        return

    try:
        from opentelemetry import metrics as otel_metrics
        from opentelemetry import trace  # type: ignore[import-untyped]
        from opentelemetry.exporter.otlp.proto.grpc.trace_exporter import (  # type: ignore[import-untyped]
            OTLPSpanExporter,
        )
        from opentelemetry.sdk.resources import Resource  # type: ignore[import-untyped]
        from opentelemetry.sdk.trace import TracerProvider  # type: ignore[import-untyped]
        from opentelemetry.sdk.trace.export import (
            BatchSpanProcessor,  # type: ignore[import-untyped]
        )

        resource = Resource.create({"service.name": service_name})

        if enable_tracing:
            provider = TracerProvider(resource=resource)
            processor = BatchSpanProcessor(OTLPSpanExporter(endpoint=endpoint))
            provider.add_span_processor(processor)
            trace.set_tracer_provider(provider)
            _tracer = trace.get_tracer(service_name)
            logger.info("Observability: OTLP tracing enabled → %s", endpoint)

        if enable_metrics:
            try:
                from opentelemetry.exporter.otlp.proto.grpc.metric_exporter import (  # type: ignore[import-untyped]
                    OTLPMetricExporter,
                )
                from opentelemetry.sdk.metrics import MeterProvider  # type: ignore[import-untyped]

                _metric_reader = OTLPMetricExporter(endpoint=endpoint)
                meter_provider = MeterProvider(
                    resource=resource,
                    metric_readers=[],  # OTLP push is via periodic reader
                )
                otel_metrics.set_meter_provider(meter_provider)
                _meter = otel_metrics.get_meter(service_name)
                logger.info("Observability: OTLP metrics enabled → %s", endpoint)
            except Exception as exc:
                logger.warning("Observability: OTLP metrics setup failed: %s", exc)

    except ImportError:
        logger.info(
            "Observability: opentelemetry packages not installed — "
            "using built-in metrics only"
        )
    except Exception as exc:
        logger.warning("Observability: setup failed: %s", exc)

    _initialized = True


# ---------------------------------------------------------------------------
# Tracing helpers
# ---------------------------------------------------------------------------

@contextmanager
def trace_phase(phase_name: str, agent_id: str | None = None) -> Generator[None, None, None]:
    """Context manager that creates an OTel span for a think-loop phase.

    Falls back to simple timing when OTel is not available.

    Usage::

        with trace_phase("perceive", agent_id="alice"):
            perception = await perceive(state, tick)
    """
    if _tracer is not None:
        try:
            from opentelemetry import trace  # type: ignore[import-untyped]

            attributes: dict[str, str] = {"phase": phase_name}
            if agent_id:
                attributes["agent.id"] = agent_id

            with _tracer.start_as_current_span(
                f"think_loop.{phase_name}", attributes=attributes
            ) as span:
                start = time.monotonic()
                try:
                    yield
                except Exception as exc:
                    span.set_status(
                        trace.StatusCode.ERROR,
                        description=str(exc),
                    )
                    raise
                finally:
                    elapsed = time.monotonic() - start
                    span.set_attribute("duration_ms", elapsed * 1000)
        except Exception:
            # OTel span creation failed — fall through to timing-only path
            start = time.monotonic()
            yield
            elapsed = time.monotonic() - start
    else:
        start = time.monotonic()
        yield
        elapsed = time.monotonic() - start

    # Always record in local histogram
    _histogram_for_phase(phase_name).observe(
        elapsed if 'elapsed' in dir() else time.monotonic() - start
    )


def _histogram_for_phase(phase: str) -> _Histogram:
    """Map phase names to Metrics histogram fields."""
    mapping = {
        "perceive": metrics.perceive_duration,
        "decide": metrics.decide_duration,
        "act": metrics.act_duration,
        "survive": metrics.think_duration,
        "reflect": metrics.think_duration,
        "think": metrics.think_duration,
    }
    return mapping.get(phase, metrics.think_duration)


# ---------------------------------------------------------------------------
# Structured logging helpers
# ---------------------------------------------------------------------------

def log_tick(tick: int, agent_id: str, tokens: int, health: float, phase: str) -> None:
    """Emit a structured log line for a tick event."""
    logger.info(
        "tick=%d agent=%s tokens=%d health=%.1f phase=%s",
        tick, agent_id, tokens, health, phase,
    )
    metrics.think_ticks.inc()


def log_error(tick: int, agent_id: str, phase: str, error: str) -> None:
    """Emit a structured error log."""
    logger.error(
        "tick=%d agent=%s phase=%s error=%s",
        tick, agent_id, phase, error,
    )
    metrics.errors_total.inc()


def log_llm_call(
    agent_id: str,
    model: str,
    prompt_tokens: int,
    completion_tokens: int,
    latency_s: float,
) -> None:
    """Record an LLM API call."""
    metrics.llm_calls.inc()
    metrics.llm_tokens_used.inc(prompt_tokens + completion_tokens)
    metrics.llm_latency.observe(latency_s)
    logger.debug(
        "agent=%s model=%s prompt_tokens=%d completion_tokens=%d latency=%.3fs",
        agent_id, model, prompt_tokens, completion_tokens, latency_s,
    )
