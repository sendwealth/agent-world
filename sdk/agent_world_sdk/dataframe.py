"""DataFrame conversion utilities — optional pandas integration.

All functions gracefully degrade when pandas is not installed, raising
``ImportError`` with a helpful message.

Usage::

    from agent_world_sdk.dataframe import to_dataframe

    client = AgentWorldClient("http://localhost:3000")
    agents = client.agents.list()
    df = to_dataframe(agents)
"""

from __future__ import annotations

from typing import Any


def _require_pandas():
    """Import and return pandas, raising a clear error if not installed."""
    try:
        import pandas  # noqa: F811
        return pandas
    except ImportError as exc:
        raise ImportError(
            "pandas is required for DataFrame output.  "
            "Install it with:  pip install pandas"
        ) from exc


def to_dataframe(data: list[dict] | dict[str, Any], *, flatten: int = 1) -> Any:
    """Convert SDK response data to a ``pandas.DataFrame``.

    Args:
        data: A list of dicts (e.g. agent list) or a single dict (will be
              wrapped in a one-element list).
        flatten: How many levels of nested dicts to flatten into column
                 names using ``_`` as separator.  0 = no flattening.

    Returns:
        A ``pandas.DataFrame``.

    Raises:
        ImportError: If pandas is not installed.
    """
    pd = _require_pandas()

    if isinstance(data, dict):
        data = [data]

    if flatten > 0:
        data = [_flatten_dict(row, max_depth=flatten) for row in data]

    return pd.DataFrame(data)


def agents_dataframe(agents: list[dict]) -> Any:
    """Convenience: convert an agent list to a DataFrame with common columns.

    Extracts ``id``, ``name``, ``phase``, ``tokens``, ``money``, ``alive``,
    ``ticks_survived``, and ``generation`` into flat columns.  Nested fields
    like ``skills`` and ``organization`` are serialised as JSON strings.
    """
    pd = _require_pandas()
    import json

    rows = []
    for a in agents:
        row: dict[str, Any] = {
            "id": a.get("id"),
            "name": a.get("name"),
            "phase": a.get("phase"),
            "tokens": a.get("tokens"),
            "money": a.get("money"),
            "alive": a.get("alive"),
            "ticks_survived": a.get("ticks_survived"),
            "generation": a.get("generation"),
        }
        org = a.get("organization")
        if isinstance(org, dict):
            row["org_id"] = org.get("org_id")
            row["org_name"] = org.get("org_name")
            row["org_type"] = org.get("org_type")
            row["org_role"] = org.get("role")
        else:
            row["org_id"] = None
            row["org_name"] = None
            row["org_type"] = None
            row["org_role"] = None

        skills = a.get("skills")
        row["skills"] = json.dumps(skills) if isinstance(skills, dict) else skills
        row["reputation"] = a.get("reputation")
        rows.append(row)

    return pd.DataFrame(rows)


def history_dataframe(snapshots: list[dict]) -> Any:
    """Convenience: convert world history snapshots to a DataFrame.

    Flattens ``resource_distribution`` and other nested fields.
    """
    return to_dataframe(snapshots, flatten=2)


def network_dataframe(
    nodes: list[dict],
    edges: list[dict],
) -> tuple[Any, Any]:
    """Convenience: convert network graph data to DataFrames.

    Returns ``(nodes_df, edges_df)``.
    """
    pd = _require_pandas()
    nodes_df = pd.DataFrame(nodes)
    edges_df = pd.DataFrame(edges)
    return nodes_df, edges_df


def behavior_log_dataframe(entries: list[dict]) -> Any:
    """Convenience: convert behavior log entries to a DataFrame.

    Flattens the ``details`` dict into separate columns.
    """
    return to_dataframe(entries, flatten=1)


# -- Helpers ----------------------------------------------------------------

def _flatten_dict(d: dict, *, max_depth: int = 1, _depth: int = 0, _prefix: str = "") -> dict:
    """Flatten nested dicts into a single level using ``_`` separators."""
    out: dict[str, Any] = {}
    for key, value in d.items():
        full_key = f"{_prefix}{key}" if not _prefix else f"{_prefix}_{key}"
        if isinstance(value, dict) and _depth < max_depth:
            out.update(_flatten_dict(value, max_depth=max_depth, _depth=_depth + 1, _prefix=full_key))
        else:
            out[full_key] = value
    return out
