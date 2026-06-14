"""FastAPI REST API for the A/B Experiment Framework.

Provides endpoints for creating, running, and analyzing A/B experiments.

Endpoints (aligned with Rust backend ``world-engine/src/api_experiment.rs``):
    POST   /api/v2/experiments                  — Create a new A/B experiment
    GET    /api/v2/experiments                   — List all experiments
    GET    /api/v2/experiments/{id}              — Get experiment details
    POST   /api/v2/experiments/{id}/run          — Run an experiment
    GET    /api/v2/experiments/{id}/results      — Get results with statistical tests
    GET    /api/v2/experiments/{id}/report       — Get report (markdown/html/pdf)
    POST   /api/v2/experiments/{id}/compare      — Compare two experiment runs

Usage::

    from agent_runtime.experiment.rest_api import create_app

    app = create_app()
    # Run with: uvicorn agent_runtime.experiment.rest_api:create_app --factory

Or programmatically::

    import uvicorn
    from agent_runtime.experiment.rest_api import create_app

    app = create_app(world_engine_url="http://localhost:3000")
    uvicorn.run(app, host="0.0.0.0", port=8000)
"""

from __future__ import annotations

import uuid
from typing import Any

from fastapi import FastAPI, HTTPException, Query
from pydantic import BaseModel, Field

from agent_runtime.experiment.ab_framework import ABExperiment
from agent_runtime.experiment.dsl import (
    ExperimentDefinition,
    ExperimentGroup,
    ExperimentVariable,
    Hypothesis,
)
from agent_runtime.experiment.report import ExperimentReporter

# ---------------------------------------------------------------------------
# Pydantic models for API
# ---------------------------------------------------------------------------


class VariableModel(BaseModel):
    name: str
    default: Any = None
    type_hint: str = "str"
    description: str = ""


class GroupModel(BaseModel):
    name: str
    variables: dict[str, Any] = Field(default_factory=dict)
    description: str = ""
    agent_ratio: float | None = None


class HypothesisModel(BaseModel):
    null: str = ""
    alternative: str = ""
    direction: str = "two-sided"
    metric: str = ""
    alpha: float = 0.05


class CreateExperimentRequest(BaseModel):
    """Request body for creating an A/B experiment."""

    name: str
    description: str = ""
    variables: list[VariableModel] = Field(default_factory=list)
    groups: list[GroupModel]
    hypothesis: HypothesisModel | None = None
    agent_count: int = 50
    duration_ticks: int = 10000
    base_seed: int = 42
    world_config: dict[str, Any] = Field(default_factory=dict)
    llm_config: dict[str, Any] = Field(default_factory=dict)


class RunExperimentRequest(BaseModel):
    """Request body for running an experiment."""

    mode: str = "parallel"  # "parallel" or "sequential"


class CompareRequest(BaseModel):
    """Request body for comparing two runs."""

    run_id_a: str
    run_id_b: str
    alpha: float = 0.05


# ---------------------------------------------------------------------------
# In-memory store (for standalone mode)
# ---------------------------------------------------------------------------

_experiments: dict[str, dict[str, Any]] = {}


# ---------------------------------------------------------------------------
# App factory
# ---------------------------------------------------------------------------


def create_app(
    world_engine_url: str | None = None,
    title: str = "Agent World Experiment API",
) -> FastAPI:
    """Create the FastAPI application.

    Args:
        world_engine_url: Optional World Engine URL for live experiments.
        title: API title.

    Returns:
        Configured FastAPI application.
    """
    app = FastAPI(
        title=title,
        version="2.0.0",
        description="A/B Experiment Framework API for Agent World",
    )

    @app.post(
        "/api/v2/experiments",
        status_code=201,
        summary="Create a new A/B experiment",
    )
    async def create_experiment(req: CreateExperimentRequest) -> dict[str, Any]:
        """Create a new A/B experiment definition."""
        # Build DSL definition
        variables = [
            ExperimentVariable(
                name=v.name,
                default=v.default,
                type_hint=v.type_hint,
                description=v.description,
            )
            for v in req.variables
        ]
        groups = [
            ExperimentGroup(
                name=g.name,
                variables=g.variables,
                description=g.description,
                agent_ratio=g.agent_ratio,
            )
            for g in req.groups
        ]
        hypothesis = None
        if req.hypothesis:
            hypothesis = Hypothesis(
                null=req.hypothesis.null,
                alternative=req.hypothesis.alternative,
                direction=req.hypothesis.direction,
                metric=req.hypothesis.metric,
                alpha=req.hypothesis.alpha,
            )

        definition = ExperimentDefinition(
            name=req.name,
            description=req.description,
            variables=variables,
            groups=groups,
            hypothesis=hypothesis,
            agent_count=req.agent_count,
            duration_ticks=req.duration_ticks,
            base_seed=req.base_seed,
            world_config=req.world_config,
            llm_config=req.llm_config,
        )

        # Validate
        errors = definition.validate()
        if errors:
            raise HTTPException(status_code=400, detail={"errors": errors})

        # Generate configs for each group
        configs = definition.to_configs()

        # Store
        exp_id = str(uuid.uuid4())
        _experiments[exp_id] = {
            "id": exp_id,
            "definition": definition,
            "configs": configs,
            "status": "created",
            "results": {},
        }

        return {
            "experiment_id": exp_id,
            "name": req.name,
            "groups": [g.name for g in groups],
            "configs": {name: cfg.to_dict() for name, cfg in configs.items()},
        }

    @app.get(
        "/api/v2/experiments",
        summary="List all experiments",
    )
    async def list_experiments() -> list[dict[str, Any]]:
        """List all experiments."""
        return [
            {
                "id": exp["id"],
                "name": exp["definition"].name,
                "status": exp["status"],
                "groups": [g.name for g in exp["definition"].groups],
            }
            for exp in _experiments.values()
        ]

    @app.get(
        "/api/v2/experiments/{experiment_id}",
        summary="Get experiment details",
    )
    async def get_experiment(experiment_id: str) -> dict[str, Any]:
        """Get full experiment details."""
        if experiment_id not in _experiments:
            raise HTTPException(status_code=404, detail="Experiment not found")

        exp = _experiments[experiment_id]
        return {
            "id": exp["id"],
            "definition": exp["definition"].to_yaml(),
            "status": exp["status"],
            "configs": {name: cfg.to_dict() for name, cfg in exp["configs"].items()},
            "results": {
                name: result.to_dict()
                for name, result in exp["results"].items()
            },
        }

    @app.post(
        "/api/v2/experiments/{experiment_id}/run",
        summary="Run an experiment",
    )
    async def run_experiment(
        experiment_id: str,
        req: RunExperimentRequest,
    ) -> dict[str, Any]:
        """Run the experiment (parallel or sequential)."""
        if experiment_id not in _experiments:
            raise HTTPException(status_code=404, detail="Experiment not found")

        exp = _experiments[experiment_id]
        if exp["status"] == "running":
            raise HTTPException(status_code=409, detail="Experiment is already running")

        configs = list(exp["configs"].values())
        if len(configs) < 2:
            raise HTTPException(status_code=400, detail="Need at least 2 configs")

        exp["status"] = "running"

        ab = ABExperiment(
            configs[0],
            configs[1],
            seed_base=exp["definition"].base_seed,
            world_engine_url=world_engine_url,
        )

        if req.mode == "sequential":
            result = await ab.run_sequential()
        else:
            result = await ab.run_parallel()

        group_names = list(exp["configs"].keys())
        exp["results"][group_names[0]] = result.result_a
        exp["results"][group_names[1]] = result.result_b
        exp["status"] = "completed"

        return {
            "status": "completed",
            "result_a": result.result_a.to_dict(),
            "result_b": result.result_b.to_dict(),
            "comparison": {
                "metrics_diff": result.comparison.metrics_diff,
                "statistical_significance": result.comparison.statistical_significance,
                "effect_sizes": result.comparison.effect_sizes,
                "recommendation": result.comparison.recommendation,
            },
        }

    @app.get(
        "/api/v2/experiments/{experiment_id}/results",
        summary="Get experiment results with statistical tests",
    )
    async def get_results(
        experiment_id: str,
        alpha: float = Query(0.05, description="Significance level"),
    ) -> dict[str, Any]:
        """Get experiment results with statistical analysis."""
        if experiment_id not in _experiments:
            raise HTTPException(status_code=404, detail="Experiment not found")

        exp = _experiments[experiment_id]
        results = exp["results"]

        if len(results) < 2:
            raise HTTPException(
                status_code=400,
                detail="Experiment has not been run yet (need 2 result sets)",
            )

        result_list = list(results.values())
        ab = ABExperiment(result_list[0], result_list[1])
        comparison = ab.compare_results(result_list[0], result_list[1], alpha=alpha)

        return {
            "experiment_id": experiment_id,
            "results": {name: r.to_dict() for name, r in results.items()},
            "comparison": {
                "metrics_diff": comparison.metrics_diff,
                "statistical_significance": comparison.statistical_significance,
                "effect_sizes": comparison.effect_sizes,
                "test_results": comparison.test_results,
                "recommendation": comparison.recommendation,
                "summary": comparison.summary,
            },
        }

    @app.get(
        "/api/v2/experiments/{experiment_id}/report",
        summary="Get experiment report",
    )
    async def get_report(
        experiment_id: str,
        format: str = Query("markdown", description="Report format: markdown, json, html, pdf"),
    ) -> Any:
        """Get experiment report in specified format."""
        if experiment_id not in _experiments:
            raise HTTPException(status_code=404, detail="Experiment not found")

        exp = _experiments[experiment_id]
        results = exp["results"]

        if not results:
            raise HTTPException(status_code=400, detail="No results available yet")

        reporter = ExperimentReporter()

        # Single result report
        if len(results) == 1:
            result = list(results.values())[0]
            return reporter.generate_report(result, format=format)

        # A/B comparison report
        result_list = list(results.values())
        ab = ABExperiment(result_list[0], result_list[1])
        comparison = ab.compare_results(result_list[0], result_list[1])

        if format == "pdf":
            from fastapi.responses import Response
            content = reporter.generate_ab_report(comparison, format="pdf")
            if isinstance(content, bytes):
                return Response(
                    content=content,
                    media_type="application/pdf",
                    headers={
                        "Content-Disposition": (
                            f"attachment; filename=report-{experiment_id}.pdf"
                        )
                    },
                )
            return Response(content=content, media_type="text/html")

        report = reporter.generate_ab_report(comparison, format=format)

        if format == "json":
            import json as json_mod

            from fastapi.responses import JSONResponse

            return JSONResponse(
                content=json_mod.loads(report) if isinstance(report, str) else report
            )

        return {"report": report, "format": format}

    @app.post(
        "/api/v2/experiments/{experiment_id}/compare",
        summary="Compare two experiment runs",
    )
    async def compare_runs(
        experiment_id: str,
        req: CompareRequest,
    ) -> dict[str, Any]:
        """Compare two specific experiment runs with statistical tests."""
        if experiment_id not in _experiments:
            raise HTTPException(status_code=404, detail="Experiment not found")

        exp = _experiments[experiment_id]
        results = exp["results"]

        if req.run_id_a not in results or req.run_id_b not in results:
            raise HTTPException(
                status_code=400,
                detail=f"Run IDs not found. Available: {list(results.keys())}",
            )

        result_a = results[req.run_id_a]
        result_b = results[req.run_id_b]

        ab = ABExperiment(result_a, result_b)
        comparison = ab.compare_results(result_a, result_b, alpha=req.alpha)

        return {
            "comparison": {
                "metrics_diff": comparison.metrics_diff,
                "statistical_significance": comparison.statistical_significance,
                "effect_sizes": comparison.effect_sizes,
                "test_results": comparison.test_results,
                "recommendation": comparison.recommendation,
                "summary": comparison.summary,
            },
        }

    return app


# ---------------------------------------------------------------------------
# Convenience: default app instance
# ---------------------------------------------------------------------------


app = create_app()
