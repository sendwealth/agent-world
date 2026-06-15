"""Dataset publishing flow — ``python -m agent_runtime publish``.

Packages an A/B experiment's artefacts (report JSON, event logs, agent
snapshots, benchmark metrics) into a citable dataset with full provenance
and uploads it to Zenodo (primary) or Dataverse (fallback), returning a DOI.

Public API::

    from agent_runtime.publish import publish_experiment, Provenance

    result = await publish_experiment(
        experiment_dir=Path("reports/benchmark"),
        backend="zenodo",
    )
    print(result.doi)

The flow is intentionally backend-agnostic: each backend is a small adapter
implementing :class:`PublishBackend`.  Provenance metadata is collected
once, reused across backends, and embedded in the dataset archive.
"""

from agent_runtime.publish.backends import (
    BackendError,
    DataverseBackend,
    PublishBackend,
    PublishResult,
    TokenMissingError,
    ZenodoBackend,
)
from agent_runtime.publish.packaging import DatasetPackage, package_experiment
from agent_runtime.publish.provenance import Provenance, collect_provenance
from agent_runtime.publish.runner import publish_experiment

__all__ = [
    # runner
    "publish_experiment",
    # provenance
    "Provenance",
    "collect_provenance",
    # packaging
    "DatasetPackage",
    "package_experiment",
    # backends
    "BackendError",
    "DataverseBackend",
    "PublishBackend",
    "PublishResult",
    "TokenMissingError",
    "ZenodoBackend",
]
