"""HTTP client wrapper with authentication, request handling, and error types."""

from __future__ import annotations

from typing import Any

import httpx


class APIError(Exception):
    """Raised when the API returns a non-2xx response."""

    def __init__(self, status_code: int, message: str) -> None:
        self.status_code = status_code
        self.message = message
        super().__init__(f"APIError({status_code}): {message}")


class AgentWorldClient:
    """Top-level client for the Agent World research API.

    Usage::

        client = AgentWorldClient("http://localhost:3000", api_key="my-key")
        state = client.world.state()
    """

    def __init__(
        self,
        base_url: str,
        *,
        api_key: str | None = None,
        timeout: float = 30.0,
    ) -> None:
        self._base_url = base_url.rstrip("/")
        self._api_key = api_key
        self._http = httpx.Client(
            base_url=self._base_url,
            timeout=timeout,
            headers=self._default_headers(),
        )

        # Lazy-initialised sub-clients.
        self._world: WorldModule | None = None
        self._agents: AgentsModule | None = None
        self._events: EventsModule | None = None
        self._experiments: ExperimentsModule | None = None
        self._export: ExportModule | None = None
        self._analyze: AnalyzeModule | None = None
        self._economic: EconomicModule | None = None
        self._social: SocialModule | None = None
        self._behavior: BehaviorModule | None = None

    # -- Sub-client properties --

    @property
    def world(self) -> WorldModule:
        if self._world is None:
            self._world = WorldModule(self)
        return self._world

    @property
    def agents(self) -> AgentsModule:
        if self._agents is None:
            self._agents = AgentsModule(self)
        return self._agents

    @property
    def events(self) -> EventsModule:
        if self._events is None:
            self._events = EventsModule(self)
        return self._events

    @property
    def experiments(self) -> ExperimentsModule:
        if self._experiments is None:
            self._experiments = ExperimentsModule(self)
        return self._experiments

    @property
    def export(self) -> ExportModule:
        if self._export is None:
            self._export = ExportModule(self)
        return self._export

    @property
    def analyze(self) -> AnalyzeModule:
        if self._analyze is None:
            self._analyze = AnalyzeModule()
        return self._analyze

    @property
    def economic(self) -> EconomicModule:
        """Economic analysis: Gini, wealth distribution, price trends, inflation."""
        if self._economic is None:
            self._economic = EconomicModule()
        return self._economic

    @property
    def social(self) -> SocialModule:
        """Social network analysis: centrality, communities, interaction patterns."""
        if self._social is None:
            self._social = SocialModule()
        return self._social

    @property
    def behavior(self) -> BehaviorModule:
        """Behavioral patterns: survival stats, activity profiles, strategy classification."""
        if self._behavior is None:
            self._behavior = BehaviorModule()
        return self._behavior

    # -- HTTP helpers --

    def _default_headers(self) -> dict[str, str]:
        headers: dict[str, str] = {
            "Accept": "application/json",
        }
        if self._api_key:
            headers["X-API-Key"] = self._api_key
        return headers

    def _get(self, path: str, *, params: dict[str, Any] | None = None) -> Any:
        resp = self._http.get(path, params=params)
        return self._handle_response(resp)

    def _post(self, path: str, *, json: Any = None) -> Any:
        resp = self._http.post(path, json=json)
        return self._handle_response(resp)

    def _get_raw(self, path: str, *, params: dict[str, Any] | None = None) -> httpx.Response:
        """Return the raw response (for streaming / non-JSON responses)."""
        return self._http.get(path, params=params)

    @staticmethod
    def _handle_response(resp: httpx.Response) -> Any:
        if resp.status_code >= 400:
            try:
                body = resp.json()
                message = body.get("error", resp.text)
            except Exception:
                message = resp.text
            raise APIError(resp.status_code, message)
        if resp.status_code == 204:
            return None
        return resp.json()

    def close(self) -> None:
        self._http.close()

    def __enter__(self) -> AgentWorldClient:
        return self

    def __exit__(self, *args: object) -> None:
        self.close()


# -- Sub-modules are imported at the bottom to avoid circular deps --
from agent_world_sdk.world import WorldModule, AgentsModule, EventsModule
from agent_world_sdk.experiments import ExperimentsModule
from agent_world_sdk.export import ExportModule
from agent_world_sdk.analyze import AnalyzeModule
from agent_world_sdk.economic import EconomicModule
from agent_world_sdk.social import SocialModule
from agent_world_sdk.behavior import BehaviorModule
