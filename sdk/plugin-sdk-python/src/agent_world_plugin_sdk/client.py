"""Agent World Plugin SDK — HTTP client for the Plugin API.

Provides a :class:`PluginClient` for interacting with the Agent World
engine's plugin management endpoints over HTTP.
"""
from __future__ import annotations

import json
import urllib.request
import urllib.error
import urllib.parse
from typing import Any, Dict, List, Optional


class PluginClientError(Exception):
    """Raised when the plugin API returns an error response."""

    def __init__(self, status_code: int, message: str):
        self.status_code = status_code
        self.message = message
        super().__init__(f"HTTP {status_code}: {message}")


class PluginClient:
    """HTTP client for the Agent World plugin management API.

    Args:
        base_url: Base URL of the Agent World engine
            (e.g. ``"http://localhost:8080"``).
        api_key: Optional API key for authentication.
        timeout: Request timeout in seconds (default: 30).
    """

    def __init__(
        self,
        base_url: str = "http://localhost:8080",
        api_key: Optional[str] = None,
        timeout: int = 30,
    ):
        self.base_url = base_url.rstrip("/")
        self.api_key = api_key
        self.timeout = timeout

    # ── Plugin Management ─────────────────────────────────────────────

    def register(self, plugin_id: str, config: Optional[Dict[str, str]] = None) -> Dict[str, Any]:
        """Register a plugin with the engine.

        Args:
            plugin_id: Plugin identifier (e.g. ``"author/plugin-name"``).
            config: Optional plugin configuration key-value pairs.

        Returns:
            Engine response dict with registration status.
        """
        payload: Dict[str, Any] = {"plugin_id": plugin_id}
        if config:
            payload["config"] = config
        return self._request("POST", "/api/v1/plugins", json_body=payload)

    def list_plugins(self) -> List[Dict[str, Any]]:
        """List all registered plugins.

        Returns:
            List of plugin metadata dicts.
        """
        result = self._request("GET", "/api/v1/plugins")
        return result if isinstance(result, list) else result.get("plugins", [])

    def enable(self, plugin_id: str) -> Dict[str, Any]:
        """Enable a registered plugin.

        Args:
            plugin_id: Plugin identifier to enable.

        Returns:
            Engine response dict.
        """
        return self._request("POST", f"/api/v1/plugins/{plugin_id}/enable")

    def disable(self, plugin_id: str) -> Dict[str, Any]:
        """Disable a registered plugin.

        Args:
            plugin_id: Plugin identifier to disable.

        Returns:
            Engine response dict.
        """
        return self._request("POST", f"/api/v1/plugins/{plugin_id}/disable")

    def unregister(self, plugin_id: str) -> Dict[str, Any]:
        """Unregister a plugin from the engine.

        Args:
            plugin_id: Plugin identifier to unregister.

        Returns:
            Engine response dict.
        """
        return self._request("DELETE", f"/api/v1/plugins/{plugin_id}")

    def get_plugin(self, plugin_id: str) -> Dict[str, Any]:
        """Get details for a specific plugin.

        Args:
            plugin_id: Plugin identifier to look up.

        Returns:
            Plugin metadata dict.
        """
        return self._request("GET", f"/api/v1/plugins/{plugin_id}")

    # ── WASM Loading ──────────────────────────────────────────────────

    def load_wasm(self, plugin_id: str, wasm_path: str) -> Dict[str, Any]:
        """Load a WASM module for a plugin.

        Args:
            plugin_id: Plugin identifier to associate with the module.
            wasm_path: Filesystem path to the WASM module (on the server).

        Returns:
            Engine response dict with load status.
        """
        payload = {"plugin_id": plugin_id, "wasm_path": wasm_path}
        return self._request("POST", "/api/v1/plugins/wasm/load", json_body=payload)

    def load_wasm_bytes(self, plugin_id: str, wasm_bytes: bytes) -> Dict[str, Any]:
        """Load a WASM module from raw bytes.

        Args:
            plugin_id: Plugin identifier to associate with the module.
            wasm_bytes: Raw WASM binary content.

        Returns:
            Engine response dict with load status.
        """
        import base64

        payload = {
            "plugin_id": plugin_id,
            "wasm_base64": base64.b64encode(wasm_bytes).decode("ascii"),
        }
        return self._request("POST", "/api/v1/plugins/wasm/load", json_body=payload)

    # ── Execution ─────────────────────────────────────────────────────

    def execute(
        self,
        plugin_id: str,
        skill_id: str,
        agent_id: str,
        params: Optional[Dict[str, str]] = None,
    ) -> Dict[str, Any]:
        """Execute a plugin skill for an agent.

        Args:
            plugin_id: Plugin that provides the skill.
            skill_id: Skill to execute.
            agent_id: Agent requesting execution.
            params: Optional skill-specific parameters.

        Returns:
            ActionResult dict from the plugin.
        """
        payload: Dict[str, Any] = {
            "plugin_id": plugin_id,
            "skill_id": skill_id,
            "agent_id": agent_id,
        }
        if params:
            payload["params"] = params
        return self._request("POST", "/api/v1/plugins/execute", json_body=payload)

    # ── Health ────────────────────────────────────────────────────────

    def health(self) -> Dict[str, Any]:
        """Check engine health.

        Returns:
            Health status dict.
        """
        return self._request("GET", "/api/v1/health")

    # ── Internal ──────────────────────────────────────────────────────

    def _request(
        self,
        method: str,
        path: str,
        json_body: Optional[Dict[str, Any]] = None,
    ) -> Any:
        url = f"{self.base_url}{path}"
        headers: Dict[str, str] = {"Accept": "application/json"}

        if self.api_key:
            headers["Authorization"] = f"Bearer {self.api_key}"

        body: Optional[bytes] = None
        if json_body is not None:
            body = json.dumps(json_body).encode("utf-8")
            headers["Content-Type"] = "application/json"

        req = urllib.request.Request(
            url, data=body, headers=headers, method=method
        )

        try:
            with urllib.request.urlopen(req, timeout=self.timeout) as resp:
                raw = resp.read().decode("utf-8")
                if raw:
                    return json.loads(raw)
                return {}
        except urllib.error.HTTPError as exc:
            message = ""
            try:
                message = exc.read().decode("utf-8")
            except Exception:
                message = str(exc)
            raise PluginClientError(exc.code, message) from exc
        except urllib.error.URLError as exc:
            raise PluginClientError(0, f"Connection error: {exc.reason}") from exc
