"""Dataset packaging — bundle an experiment directory into a ZIP archive
with an embedded provenance manifest.

The archive layout is intentionally flat and self-describing::

    dataset.zip
    ├── manifest.json         # provenance + file listing + checksums
    ├── report.json           # original experiment report (if present)
    ├── reference.json        # benchmark reference (if present)
    ├── events.csv            # event log (if present)
    ├── snapshots/            # agent state snapshots (if present)
    └── ...

Anyone who downloads the ZIP can read ``manifest.json`` to understand
exactly what the dataset contains and how it was produced.
"""

from __future__ import annotations

import hashlib
import json
import logging
import zipfile
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from agent_runtime.publish.provenance import Provenance, collect_provenance

logger = logging.getLogger(__name__)

# File globs that are always included when present
_INCLUDE_GLOBS: tuple[str, ...] = (
    "report.json",
    "reference.json",
    "reference.md",
    "events.csv",
    "events.json",
    "config.yaml",
    "config.toml",
    "world.json",
    "skills.json",
)

# Directories that are included wholesale when present
_INCLUDE_DIRS: tuple[str, ...] = (
    "snapshots",
    "exports",
    "charts",
    "logs",
)

# Hard size cap per single artefact (50 MB) — protects against accidental
# inclusion of huge binary blobs; provenance should still capture metadata.
_MAX_SINGLE_FILE_BYTES = 50 * 1024 * 1024

# Hard cap on total directory payload (200 MB).
_MAX_TOTAL_BYTES = 200 * 1024 * 1024


@dataclass
class DatasetPackage:
    """A packaged dataset ready for upload."""

    archive_path: Path
    provenance: Provenance
    file_count: int = 0
    total_bytes: int = 0
    sha256: str = ""
    files: list[dict[str, Any]] = field(default_factory=list)

    def to_dict(self) -> dict[str, Any]:
        """Serialise to a JSON-safe dict."""
        return {
            "archive_path": str(self.archive_path),
            "file_count": self.file_count,
            "total_bytes": self.total_bytes,
            "sha256": self.sha256,
            "files": self.files,
            "provenance": self.provenance.to_dict(),
        }


def package_experiment(
    experiment_path: Path,
    *,
    output_path: Path | None = None,
    title: str | None = None,
    creators: list[dict[str, str]] | None = None,
    description: str | None = None,
) -> DatasetPackage:
    """Package an experiment directory into a ZIP archive with manifest.

    Args:
        experiment_path: Directory (or report file) to package.
        output_path: Where to write the ZIP.  Defaults to
            ``<experiment_dir>/dataset.zip``.
        title: Optional dataset title (passed to provenance collector).
        creators: Optional creators list.
        description: Optional description.

    Returns:
        :class:`DatasetPackage` describing the archive.
    """
    if experiment_path.is_file():
        experiment_dir = experiment_path.parent
    else:
        experiment_dir = experiment_path

    if not experiment_dir.exists():
        raise FileNotFoundError(f"Experiment directory not found: {experiment_dir}")

    provenance = collect_provenance(
        experiment_path,
        title=title,
        creators=creators,
        description=description,
    )

    out = output_path or (experiment_dir / "dataset.zip")
    out.parent.mkdir(parents=True, exist_ok=True)

    file_entries: list[dict[str, Any]] = []
    total_bytes = 0

    with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED) as zf:
        # 1. Known single files
        for pattern in _INCLUDE_GLOBS:
            for path in experiment_dir.glob(pattern):
                if path.is_file():
                    entry = _add_file(zf, path, arcname=path.name)
                    if entry is not None:
                        file_entries.append(entry["meta"])
                        total_bytes += entry["size"]
                        if total_bytes > _MAX_TOTAL_BYTES:
                            raise ValueError(f"Dataset payload exceeds {_MAX_TOTAL_BYTES} bytes")

        # 2. Known subdirectories
        for dirname in _INCLUDE_DIRS:
            sub = experiment_dir / dirname
            if not sub.is_dir():
                continue
            for path in sorted(sub.rglob("*")):
                if path.is_file():
                    arc = path.relative_to(experiment_dir)
                    entry = _add_file(zf, path, arcname=str(arc))
                    if entry is not None:
                        file_entries.append(entry["meta"])
                        total_bytes += entry["size"]
                        if total_bytes > _MAX_TOTAL_BYTES:
                            raise ValueError(f"Dataset payload exceeds {_MAX_TOTAL_BYTES} bytes")

        # 3. Manifest — written last but appears as first entry on read
        manifest = {
            "schema_version": "agent-world-dataset/v1",
            "provenance": provenance.to_dict(),
            "files": file_entries,
            "total_bytes": total_bytes,
            "file_count": len(file_entries),
        }
        zf.writestr(
            "manifest.json",
            json.dumps(manifest, indent=2, ensure_ascii=False, default=str),
        )

    sha = _sha256_file(out)
    package = DatasetPackage(
        archive_path=out,
        provenance=provenance,
        file_count=len(file_entries),
        total_bytes=total_bytes,
        sha256=sha,
        files=file_entries,
    )
    logger.info(
        "Packaged dataset: %s (%d files, %d bytes, sha256=%s)",
        out,
        package.file_count,
        package.total_bytes,
        sha[:12],
    )
    return package


# ---------------------------------------------------------------------------
# Internal helpers
# ---------------------------------------------------------------------------


def _add_file(
    zf: zipfile.ZipFile,
    path: Path,
    *,
    arcname: str,
) -> dict[str, Any] | None:
    """Add a single file to the archive, returning metadata.

    Returns ``None`` if the file was skipped (too large or unreadable).
    """
    try:
        size = path.stat().st_size
    except OSError:
        logger.warning("Cannot stat %s — skipping", path)
        return None
    if size > _MAX_SINGLE_FILE_BYTES:
        logger.warning("Skipping %s (%d bytes > %d cap)", path, size, _MAX_SINGLE_FILE_BYTES)
        return None
    try:
        zf.write(path, arcname=arcname)
    except OSError as exc:
        logger.warning("Failed to add %s to archive: %s", path, exc)
        return None
    sha = _sha256_file(path)
    return {
        "size": size,
        "meta": {
            "path": arcname,
            "size": size,
            "sha256": sha,
        },
    }


def _sha256_file(path: Path) -> str:
    """Compute the SHA-256 hex digest of a file."""
    h = hashlib.sha256()
    try:
        with path.open("rb") as fh:
            for chunk in iter(lambda: fh.read(65536), b""):
                h.update(chunk)
    except OSError:
        return ""
    return h.hexdigest()
