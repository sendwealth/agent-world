"""Tests for the dataset publishing flow (Phase 5.3 / SEN-721).

Covers:
- Provenance collection from various report shapes
- Dataset packaging (ZIP + manifest + checksums)
- Backend token-missing error handling (no real network calls)
- Zenodo / Dataverse upload with mocked httpx
- ``publish_experiment`` runner end-to-end with a mocked backend
"""

from __future__ import annotations

import json
import zipfile
from pathlib import Path
from unittest.mock import AsyncMock, MagicMock

import pytest

from agent_runtime.publish import (
    TokenMissingError,
    ZenodoBackend,
    collect_provenance,
    package_experiment,
    publish_experiment,
)
from agent_runtime.publish.backends import DataverseBackend

# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------


@pytest.fixture
def sample_report(tmp_path: Path) -> Path:
    """Create a minimal experiment directory resembling Phase 5.1 output."""
    exp_dir = tmp_path / "experiment"
    exp_dir.mkdir()
    report = {
        "experiment_id": "exp-001",
        "scenario": "park-benchmark",
        "config": {
            "agents": 25,
            "ticks": 200,
            "seed": 42,
            "llm": {"provider": "ollama", "model": "qwen2"},
        },
        "generated_at": "2026-06-15T00:12:45Z",
        "diffusion": {
            "final_coverage": 1.0,
            "interpretation": "Coverage 100%",
        },
        "network": {
            "density": 0.60,
            "interpretation": "dense",
        },
    }
    (exp_dir / "reference.json").write_text(json.dumps(report), encoding="utf-8")
    # A dummy artefact
    (exp_dir / "events.csv").write_text("tick,event\n0,start\n200,end\n", encoding="utf-8")
    # A subdirectory with a snapshot
    snap_dir = exp_dir / "snapshots"
    snap_dir.mkdir()
    (snap_dir / "tick_100.json").write_text(
        json.dumps({"tick": 100, "alive": 23}), encoding="utf-8"
    )
    return exp_dir


# ---------------------------------------------------------------------------
# Provenance tests
# ---------------------------------------------------------------------------


class TestProvenance:
    def test_collect_from_reference_json(self, sample_report: Path) -> None:
        prov = collect_provenance(sample_report)
        assert prov.experiment_id == "exp-001"
        assert prov.agent_count == 25
        assert prov.llm_provider == "ollama"
        assert prov.llm_model == "qwen2"
        assert prov.benchmark_metrics  # not empty
        assert "diffusion" in prov.benchmark_metrics
        # The "interpretation" key should be stripped
        assert "interpretation" not in prov.benchmark_metrics["diffusion"]
        assert prov.tick_range == {"start": 0, "end": 200}
        assert prov.title  # non-empty default

    def test_collect_from_single_file(self, sample_report: Path) -> None:
        prov = collect_provenance(sample_report / "reference.json")
        assert prov.experiment_id == "exp-001"

    def test_collect_overrides(self, sample_report: Path) -> None:
        prov = collect_provenance(
            sample_report,
            title="My Title",
            creators=[{"name": "Jane Doe"}],
            description="Custom desc",
        )
        assert prov.title == "My Title"
        assert prov.creators == [{"name": "Jane Doe"}]
        assert prov.description == "Custom desc"

    def test_collect_keywords(self, sample_report: Path) -> None:
        prov = collect_provenance(sample_report)
        assert "agent-world" in prov.keywords
        assert "emergence-benchmark" in prov.keywords

    def test_to_json_roundtrip(self, sample_report: Path) -> None:
        prov = collect_provenance(sample_report)
        data = json.loads(prov.to_json())
        assert data["experiment_id"] == "exp-001"
        assert isinstance(data["benchmark_metrics"], dict)

    def test_empty_dir_does_not_crash(self, tmp_path: Path) -> None:
        empty = tmp_path / "empty"
        empty.mkdir()
        prov = collect_provenance(empty)
        assert prov.experiment_id  # falls back to dir name
        assert prov.agent_count == 0


# ---------------------------------------------------------------------------
# Packaging tests
# ---------------------------------------------------------------------------


class TestPackaging:
    def test_package_creates_zip(self, sample_report: Path) -> None:
        pkg = package_experiment(sample_report)
        assert pkg.archive_path.exists()
        assert pkg.archive_path.suffix == ".zip"
        assert pkg.file_count >= 1  # at least reference.json + events.csv

    def test_zip_contains_manifest(self, sample_report: Path) -> None:
        pkg = package_experiment(sample_report)
        with zipfile.ZipFile(pkg.archive_path) as zf:
            names = zf.namelist()
        assert "manifest.json" in names
        assert "reference.json" in names
        assert "events.csv" in names
        assert "snapshots/tick_100.json" in names

    def test_manifest_has_provenance(self, sample_report: Path) -> None:
        pkg = package_experiment(sample_report)
        with zipfile.ZipFile(pkg.archive_path) as zf:
            manifest = json.loads(zf.read("manifest.json"))
        assert manifest["schema_version"] == "agent-world-dataset/v1"
        assert "provenance" in manifest
        assert manifest["provenance"]["experiment_id"] == "exp-001"
        assert manifest["file_count"] >= 1

    def test_package_sha256(self, sample_report: Path) -> None:
        pkg = package_experiment(sample_report)
        assert len(pkg.sha256) == 64  # hex digest length

    def test_custom_output_path(self, sample_report: Path, tmp_path: Path) -> None:
        out = tmp_path / "custom" / "ds.zip"
        pkg = package_experiment(sample_report, output_path=out)
        assert pkg.archive_path == out
        assert out.exists()

    def test_nonexistent_dir_raises(self, tmp_path: Path) -> None:
        with pytest.raises(FileNotFoundError):
            package_experiment(tmp_path / "does-not-exist")


# ---------------------------------------------------------------------------
# Backend tests (no real network)
# ---------------------------------------------------------------------------


class TestZenodoBackend:
    def test_token_missing_raises(self, monkeypatch: pytest.MonkeyPatch) -> None:
        monkeypatch.delenv("ZENODO_TOKEN", raising=False)
        with pytest.raises(TokenMissingError) as exc_info:
            ZenodoBackend()
        assert exc_info.value.env_var == "ZENODO_TOKEN"

    def test_explicit_token_accepted(self) -> None:
        backend = ZenodoBackend(token="test-token", sandbox=True)
        assert backend.name == "zenodo"
        assert backend._token == "test-token"
        assert "sandbox" in backend._base_url

    def test_production_url(self) -> None:
        backend = ZenodoBackend(token="t", sandbox=False)
        assert "sandbox" not in backend._base_url

    @pytest.mark.asyncio
    async def test_upload_success(
        self, sample_report: Path, monkeypatch: pytest.MonkeyPatch
    ) -> None:
        """Full upload flow with mocked httpx."""
        monkeypatch.setenv("ZENODO_TOKEN", "fake-token")
        backend = ZenodoBackend(sandbox=True)

        pkg = package_experiment(sample_report)

        # Mock the AsyncClient methods in order
        create_resp = MagicMock()
        create_resp.status_code = 201
        create_resp.json.return_value = {"id": 12345, "links": {}}
        create_resp.raise_for_status = MagicMock()

        put_resp = MagicMock()
        put_resp.status_code = 200
        put_resp.json.return_value = {}
        put_resp.raise_for_status = MagicMock()

        upload_resp = MagicMock()
        upload_resp.status_code = 201
        upload_resp.raise_for_status = MagicMock()

        pub_resp = MagicMock()
        pub_resp.status_code = 202
        pub_resp.json.return_value = {
            "doi": "10.5072/sandbox.abc123",
            "links": {"record_html": "https://sandbox.zenodo.org/record/12345"},
        }
        pub_resp.raise_for_status = MagicMock()

        mock_client = AsyncMock()
        mock_client.post.side_effect = [
            create_resp,
            upload_resp,
            pub_resp,
        ]
        mock_client.put = AsyncMock(return_value=put_resp)
        mock_client.is_closed = False
        backend._client = mock_client  # type: ignore[attr-defined]

        result = await backend.upload(pkg)
        assert result.doi == "10.5072/sandbox.abc123"
        assert "sandbox.zenodo.org" in result.record_url
        assert result.backend == "zenodo"
        await backend.close()


class TestDataverseBackend:
    def test_token_missing_raises(self, monkeypatch: pytest.MonkeyPatch) -> None:
        monkeypatch.delenv("DATAVERSE_TOKEN", raising=False)
        monkeypatch.delenv("DATAVERSE_URL", raising=False)
        with pytest.raises(TokenMissingError):
            DataverseBackend()

    def test_url_missing_raises(self, monkeypatch: pytest.MonkeyPatch) -> None:
        monkeypatch.setenv("DATAVERSE_TOKEN", "t")
        monkeypatch.delenv("DATAVERSE_URL", raising=False)
        with pytest.raises(TokenMissingError) as exc_info:
            DataverseBackend()
        assert exc_info.value.env_var == "DATAVERSE_URL"

    def test_explicit_config_accepted(self) -> None:
        backend = DataverseBackend(token="t", server_url="https://demo.dataverse.org")
        assert backend.name == "dataverse"
        assert backend._server_url == "https://demo.dataverse.org"


# ---------------------------------------------------------------------------
# Runner test
# ---------------------------------------------------------------------------


class TestPublishRunner:
    @pytest.mark.asyncio
    async def test_package_only_skips_upload(self, sample_report: Path) -> None:
        """``package_only=True`` should produce a package without uploading."""
        result = await publish_experiment(sample_report, package_only=True)
        assert result["result"] is None
        assert result["package"]["file_count"] >= 1

    @pytest.mark.asyncio
    async def test_token_missing_exits_cleanly(
        self,
        sample_report: Path,
        monkeypatch: pytest.MonkeyPatch,
    ) -> None:
        """When token is missing, publish_experiment should raise
        ``TokenMissingError`` (the CLI catches it and prints a friendly
        message)."""
        monkeypatch.delenv("ZENODO_TOKEN", raising=False)
        with pytest.raises(TokenMissingError):
            await publish_experiment(sample_report, backend="zenodo")


# ---------------------------------------------------------------------------
# CLI parser test
# ---------------------------------------------------------------------------


class TestCLIParser:
    def test_publish_subcommand_exists(self) -> None:
        from agent_runtime.__main__ import build_parser

        parser = build_parser()
        args = parser.parse_args(["publish", "reports/benchmark", "--backend", "zenodo"])
        assert args.command == "publish"
        assert args.backend == "zenodo"
        assert args.experiment_dir == Path("reports/benchmark")

    def test_publish_defaults_to_sandbox(self) -> None:
        from agent_runtime.__main__ import build_parser

        parser = build_parser()
        args = parser.parse_args(["publish", "x"])
        # production flag defaults to False → sandbox mode
        assert args.production is False

    def test_publish_package_only_flag(self) -> None:
        from agent_runtime.__main__ import build_parser

        parser = build_parser()
        args = parser.parse_args(["publish", "x", "--package-only"])
        assert args.package_only is True
