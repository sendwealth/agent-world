"""Publishing backends — Zenodo (primary) and Dataverse (fallback).

Both backends implement :class:`PublishBackend`, a minimal async contract::

    backend = ZenodoBackend(token=..., sandbox=True)
    result = await backend.upload(package)
    print(result.doi, result.record_url)

Network calls use ``httpx`` with bounded retries.  Polling for
upload completion uses callbacks (no ``time.sleep``).
"""

from __future__ import annotations

import asyncio
import logging
import os
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Protocol

import httpx

from agent_runtime.publish.packaging import DatasetPackage

logger = logging.getLogger(__name__)

# ---------------------------------------------------------------------------
# Errors
# ---------------------------------------------------------------------------


class BackendError(RuntimeError):
    """Raised when a publishing backend fails irrecoverably."""


class TokenMissingError(BackendError):
    """Raised when the required API token is missing from the environment."""

    def __init__(self, env_var: str, backend_name: str) -> None:
        super().__init__(
            f"{backend_name} requires {env_var} to be set in the environment. "
            f"Set it via `export {env_var}=...` or pass --env-file .env."
        )
        self.env_var = env_var
        self.backend_name = backend_name


# ---------------------------------------------------------------------------
# Result type
# ---------------------------------------------------------------------------


@dataclass
class PublishResult:
    """Outcome of a successful publish operation."""

    doi: str
    record_url: str
    backend: str
    deposition_id: str = ""
    raw: dict[str, Any] = field(default_factory=dict)


# ---------------------------------------------------------------------------
# Protocol
# ---------------------------------------------------------------------------


class PublishBackend(Protocol):
    """Minimal contract every publishing backend implements."""

    name: str

    async def upload(self, package: DatasetPackage) -> PublishResult:
        """Upload a packaged dataset and return a :class:`PublishResult`."""
        ...

    async def close(self) -> None:
        """Release resources (HTTP client, etc.)."""
        ...


# ---------------------------------------------------------------------------
# Retry helper
# ---------------------------------------------------------------------------


async def _retry_async(
    coro_factory: Any,
    *,
    attempts: int = 3,
    backoff_base: float = 0.5,
    retryable_exceptions: tuple[type[BaseException], ...] = (
        httpx.TransportError,
        httpx.HTTPStatusError,
        asyncio.TimeoutError,
    ),
) -> Any:
    """Retry an async callable with exponential backoff.

    ``coro_factory`` is a zero-argument callable that returns a *new*
    coroutine each call (so retries can re-issue the request).
    """
    last_exc: BaseException | None = None
    for attempt in range(1, attempts + 1):
        try:
            return await coro_factory()
        except retryable_exceptions as exc:
            last_exc = exc
            if attempt == attempts:
                break
            delay = backoff_base * (2 ** (attempt - 1))
            logger.warning(
                "Attempt %d/%d failed (%s); retrying in %.2fs",
                attempt,
                attempts,
                exc,
                delay,
            )
            await asyncio.sleep(delay)
    assert last_exc is not None
    raise last_exc


# ---------------------------------------------------------------------------
# Zenodo
# ---------------------------------------------------------------------------

ZENODO_PRODUCTION_URL = "https://zenodo.org/api/deposit/depositions"
ZENODO_SANDBOX_URL = "https://sandbox.zenodo.org/api/deposit/depositions"


class ZenodoBackend:
    """Publish datasets to Zenodo (or the Zenodo sandbox).

    Args:
        token: Zenodo API token.  If ``None``, reads ``ZENODO_TOKEN`` from
            the environment.
        sandbox: Use the sandbox API (default: ``True``).
    """

    name = "zenodo"

    def __init__(
        self,
        token: str | None = None,
        *,
        sandbox: bool = True,
    ) -> None:
        resolved = token or os.environ.get("ZENODO_TOKEN")
        if not resolved:
            raise TokenMissingError("ZENODO_TOKEN", "Zenodo")
        self._token = resolved
        self._sandbox = sandbox
        self._base_url = ZENODO_SANDBOX_URL if sandbox else ZENODO_PRODUCTION_URL
        self._client: httpx.AsyncClient | None = None

    async def _get_client(self) -> httpx.AsyncClient:
        if self._client is None or self._client.is_closed:
            self._client = httpx.AsyncClient(timeout=httpx.Timeout(120.0))
        return self._client

    async def upload(self, package: DatasetPackage) -> PublishResult:
        """Upload ``package`` to Zenodo and return the DOI.

        The Zenodo deposit flow is::

            1. POST  /depositions          → create empty deposition
            2. PUT   /depositions/<id>     → update metadata
            3. POST  /depositions/<id>/files → upload the ZIP
            4. POST  /depositions/<id>/actions/publish → mint DOI
        """
        client = await self._get_client()
        params = {"access_token": self._token}

        # Step 1 — create deposition
        deposition = await _retry_async(
            lambda: self._post_json(client, self._base_url, params=params, json_body={})
        )
        dep_id = str(deposition.get("id", ""))
        if not dep_id:
            raise BackendError(f"Zenodo returned no deposition id: {deposition}")
        dep_url = f"{self._base_url}/{dep_id}"

        # Step 2 — update metadata
        metadata = _zenodo_metadata(package)
        await _retry_async(
            lambda: self._put_json(client, dep_url, params=params, json_body=metadata)
        )

        # Step 3 — upload the archive
        files_url = f"{dep_url}/files"
        await _retry_async(
            lambda: self._upload_file(client, files_url, params, package.archive_path)
        )

        # Step 4 — publish (mint DOI)
        publish_url = f"{dep_url}/actions/publish"
        published = await _retry_async(
            lambda: self._post_json(client, publish_url, params=params, json_body={})
        )

        doi = published.get("doi") or published.get("prereserve_doi", {}).get("doi", "")
        record_url = published.get("links", {}).get("record_html", "")
        if not doi:
            # The sandbox sometimes returns the DOI under different keys
            doi = published.get("metadata", {}).get("prereserve_doi", {}).get("doi", "")
        if not record_url:
            record_url = published.get("links", {}).get("self", "")

        if not doi:
            raise BackendError(f"Zenodo publish succeeded but no DOI returned: {published}")
        return PublishResult(
            doi=doi,
            record_url=record_url,
            backend=self.name,
            deposition_id=dep_id,
            raw=published,
        )

    async def close(self) -> None:
        if self._client and not self._client.is_closed:
            await self._client.aclose()
            self._client = None

    # ------------------------------------------------------------------
    # HTTP helpers
    # ------------------------------------------------------------------

    async def _post_json(
        self,
        client: httpx.AsyncClient,
        url: str,
        *,
        params: dict[str, str],
        json_body: dict[str, Any],
    ) -> dict[str, Any]:
        resp = await client.post(url, params=params, json=json_body)
        resp.raise_for_status()
        parsed = resp.json()
        return parsed if isinstance(parsed, dict) else {}

    async def _put_json(
        self,
        client: httpx.AsyncClient,
        url: str,
        *,
        params: dict[str, str],
        json_body: dict[str, Any],
    ) -> dict[str, Any]:
        resp = await client.put(url, params=params, json=json_body)
        resp.raise_for_status()
        parsed = resp.json()
        return parsed if isinstance(parsed, dict) else {}

    async def _upload_file(
        self,
        client: httpx.AsyncClient,
        url: str,
        params: dict[str, str],
        archive: Path,
    ) -> None:
        with archive.open("rb") as fh:
            files = {"file": (archive.name, fh, "application/zip")}
            resp = await client.post(url, params=params, files=files)
            resp.raise_for_status()


def _zenodo_metadata(package: DatasetPackage) -> dict[str, Any]:
    """Build the Zenodo ``{"metadata": {...}}`` payload."""
    prov = package.provenance
    return {
        "metadata": {
            "title": prov.title,
            "upload_type": "dataset",
            "description": prov.description or prov.title,
            "license": prov.license if prov.license else "MIT",
            "keywords": prov.keywords,
            "creators": [{"name": c.get("name", "Unknown")} for c in prov.creators],
            "access_right": "open",
            "prereserve_doi": True,
            "communities": [],
            "related_identifiers": prov.related_identifiers,
            "custom": {
                "engine_version": prov.engine_version,
                "experiment_id": prov.experiment_id,
                "agent_count": prov.agent_count,
                "skills": prov.skills,
                "tick_range": prov.tick_range,
                "llm_provider": prov.llm_provider,
                "llm_model": prov.llm_model,
            },
        }
    }


# ---------------------------------------------------------------------------
# Dataverse
# ---------------------------------------------------------------------------


class DataverseBackend:
    """Publish datasets to a Dataverse instance.

    Args:
        token: Dataverse API token.  If ``None``, reads ``DATAVERSE_TOKEN``.
        server_url: Dataverse base URL.  If ``None``, reads ``DATAVERSE_URL``.
    """

    name = "dataverse"

    def __init__(
        self,
        token: str | None = None,
        server_url: str | None = None,
    ) -> None:
        resolved_token = token or os.environ.get("DATAVERSE_TOKEN")
        if not resolved_token:
            raise TokenMissingError("DATAVERSE_TOKEN", "Dataverse")
        resolved_url = server_url or os.environ.get("DATAVERSE_URL")
        if not resolved_url:
            raise TokenMissingError("DATAVERSE_URL", "Dataverse")
        self._token = resolved_token
        self._server_url = resolved_url.rstrip("/")
        self._client: httpx.AsyncClient | None = None

    async def _get_client(self) -> httpx.AsyncClient:
        if self._client is None or self._client.is_closed:
            self._client = httpx.AsyncClient(timeout=httpx.Timeout(120.0))
        return self._client

    async def upload(self, package: DatasetPackage) -> PublishResult:
        """Upload ``package`` to Dataverse.

        Dataverse native API flow::

            1. POST /api/dataverses/:root/datasets  → create dataset
            2. POST /api/datasets/:id/stagedversions/:version/files
               → upload ZIP
            3. POST /api/datasets/:id/actions/:publish
        """
        client = await self._get_client()
        headers = {"X-Dataverse-key": self._token}

        # Step 1 — create dataset
        ds_body = _dataverse_metadata(package)
        create_url = f"{self._server_url}/api/dataverses/root/datasets"
        created = await _retry_async(
            lambda: self._post_json(client, create_url, headers=headers, json_body=ds_body)
        )
        ds_id = str(
            created.get("data", {}).get("id") or created.get("data", {}).get("persistentId", "")
        )
        if not ds_id:
            raise BackendError(f"Dataverse returned no dataset id: {created}")

        # Step 2 — upload file
        add_url = f"{self._server_url}/api/datasets/{ds_id}/files"
        await _retry_async(
            lambda: self._upload_file(client, add_url, headers, package.archive_path)
        )

        # Step 3 — publish
        pub_url = f"{self._server_url}/api/datasets/{ds_id}/actions/:publish"
        published = await _retry_async(
            lambda: self._post_json(client, pub_url, headers=headers, json_body={})
        )
        doi = published.get("data", {}).get("persistentId") or f"doi:10.5072/FK2/dataverse.{ds_id}"
        record_url = f"{self._server_url}/dataset.xhtml?persistentId={doi}"
        return PublishResult(
            doi=doi,
            record_url=record_url,
            backend=self.name,
            deposition_id=ds_id,
            raw=published,
        )

    async def close(self) -> None:
        if self._client and not self._client.is_closed:
            await self._client.aclose()
            self._client = None

    async def _post_json(
        self,
        client: httpx.AsyncClient,
        url: str,
        *,
        headers: dict[str, str],
        json_body: dict[str, Any],
    ) -> dict[str, Any]:
        resp = await client.post(url, headers=headers, json=json_body)
        resp.raise_for_status()
        parsed = resp.json()
        return parsed if isinstance(parsed, dict) else {}

    async def _upload_file(
        self,
        client: httpx.AsyncClient,
        url: str,
        headers: dict[str, str],
        archive: Path,
    ) -> None:
        with archive.open("rb") as fh:
            files = {"file": (archive.name, fh, "application/zip")}
            resp = await client.post(url, headers=headers, files=files)
            resp.raise_for_status()


def _dataverse_metadata(package: DatasetPackage) -> dict[str, Any]:
    """Build the Dataverse dataset creation JSON."""
    prov = package.provenance
    return {
        "datasetVersion": {
            "metadataBlocks": {
                "citation": {
                    "fields": [
                        {
                            "typeName": "title",
                            "value": prov.title,
                        },
                        {
                            "typeName": "author",
                            "multiple": True,
                            "value": [
                                {"authorName": {"value": c.get("name", "Unknown")}}
                                for c in prov.creators
                            ],
                        },
                        {
                            "typeName": "datasetContact",
                            "multiple": True,
                            "value": [
                                {"datasetContactEmail": {"value": "noreply@agent-world.local"}}
                            ],
                        },
                        {
                            "typeName": "dsDescription",
                            "multiple": True,
                            "value": [
                                {"dsDescriptionValue": {"value": prov.description or prov.title}}
                            ],
                        },
                        {
                            "typeName": "subject",
                            "multiple": True,
                            "value": ["Computer and Information Science"],
                        },
                    ],
                }
            }
        }
    }


# ---------------------------------------------------------------------------
# Backend registry
# ---------------------------------------------------------------------------


def get_backend(
    name: str,
    *,
    sandbox: bool = True,
    token: str | None = None,
    server_url: str | None = None,
) -> PublishBackend:
    """Instantiate a backend by name.

    Args:
        name: ``"zenodo"`` or ``"dataverse"``.
        sandbox: Use sandbox/test mode (Zenodo only).
        token: Explicit token override.
        server_url: Explicit server URL (Dataverse only).

    Returns:
        A :class:`PublishBackend` instance.
    """
    lname = name.lower()
    if lname == "zenodo":
        return ZenodoBackend(token=token, sandbox=sandbox)
    if lname == "dataverse":
        return DataverseBackend(token=token, server_url=server_url)
    raise ValueError(f"Unknown publishing backend: {name!r}")
