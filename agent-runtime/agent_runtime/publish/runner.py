"""High-level publish runner — orchestrates packaging + upload.

Called by the CLI ``publish`` subcommand::

    result = await publish_experiment(
        experiment_dir=Path("reports/benchmark"),
        backend="zenodo",
        sandbox=True,
    )
"""

from __future__ import annotations

import logging
from pathlib import Path

from agent_runtime.publish.backends import (
    BackendError,
    PublishBackend,
    PublishResult,
    TokenMissingError,
    get_backend,
)
from agent_runtime.publish.packaging import package_experiment

logger = logging.getLogger(__name__)


async def publish_experiment(
    experiment_dir: Path,
    *,
    backend: str = "zenodo",
    sandbox: bool = True,
    token: str | None = None,
    server_url: str | None = None,
    output_path: Path | None = None,
    title: str | None = None,
    creators: list[dict[str, str]] | None = None,
    description: str | None = None,
    package_only: bool = False,
) -> dict[str, object]:
    """Package and publish an experiment in one call.

    Args:
        experiment_dir: Directory (or report file) to publish.
        backend: Backend name (``"zenodo"`` or ``"dataverse"``).
        sandbox: Use the backend's sandbox/test mode.
        token: Optional explicit token (otherwise read from env).
        server_url: Optional explicit server URL (Dataverse only).
        output_path: Where to write the ZIP archive.
        title / creators / description: Optional dataset metadata overrides.
        package_only: If ``True``, only create the ZIP — skip upload.
            Useful for local inspection before publishing.

    Returns:
        Dict with ``package`` metadata and (when uploaded) ``result``.
    """
    package = package_experiment(
        experiment_dir,
        output_path=output_path,
        title=title,
        creators=creators,
        description=description,
    )

    if package_only:
        logger.info("package_only=True — skipping upload")
        return {
            "package": package.to_dict(),
            "result": None,
        }

    backend_obj: PublishBackend = get_backend(
        backend,
        sandbox=sandbox,
        token=token,
        server_url=server_url,
    )
    try:
        logger.info(
            "Publishing %s to %s (sandbox=%s)",
            package.archive_path,
            backend_obj.name,
            sandbox,
        )
        result: PublishResult = await backend_obj.upload(package)
        logger.info("Published: doi=%s url=%s", result.doi, result.record_url)
        return {
            "package": package.to_dict(),
            "result": {
                "doi": result.doi,
                "record_url": result.record_url,
                "backend": result.backend,
                "deposition_id": result.deposition_id,
            },
        }
    finally:
        await backend_obj.close()


__all__ = ["BackendError", "TokenMissingError", "publish_experiment"]
