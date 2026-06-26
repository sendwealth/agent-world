"""Agent pool — manage multiple agent subprocesses.

Provides ``AgentPool``, ``PoolAgentInfo``, and ``run_pool``.
"""

from __future__ import annotations

import asyncio
import json
import logging
import signal
import sys
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any

logger = logging.getLogger(__name__)


@dataclass
class PoolAgentInfo:
    """Tracks a single agent in the pool."""

    index: int
    name: str
    process: asyncio.subprocess.Process | None = None
    restarts: int = 0
    status: str = "pending"  # pending | running | crashed | stopped


class AgentPool:
    """Manages a pool of agent subprocesses with health monitoring."""

    def __init__(
        self,
        *,
        count: int = 1,
        config_dir: Path | None = None,
        max_restart: int = 3,
        health_interval: float = 10.0,
        api_port: int = 9090,
        spawn_args: list[str] | None = None,
    ) -> None:
        self._count = count
        self._config_dir = config_dir
        self._max_restart = max_restart
        self._health_interval = health_interval
        self._api_port = api_port
        self._spawn_args = spawn_args or []
        self._agents: list[PoolAgentInfo] = []
        self._shutdown: asyncio.Event | None = None
        self._api_server: asyncio.Server | None = None

    def _get_shutdown_event(self) -> asyncio.Event:
        if self._shutdown is None:
            self._shutdown = asyncio.Event()
        return self._shutdown

    async def run(self) -> dict[str, Any]:
        """Start all agents and monitor until shutdown."""
        start_time = time.monotonic()

        if self._config_dir is not None:
            self._agents = self._build_from_config_dir()
        else:
            self._agents = self._build_from_count()

        if not self._agents:
            logger.warning("No agents to start in pool")
            return {"agents": [], "duration_s": 0.0}

        logger.info(
            "Starting agent pool with %d agents",
            len(self._agents),
            extra={"event": "pool_start"},
        )

        for agent in self._agents:
            await self._start_agent(agent)

        # Start the Pool API server
        api_task = asyncio.create_task(self._start_api_server())

        try:
            while not self._get_shutdown_event().is_set():
                try:
                    await asyncio.wait_for(
                        self._get_shutdown_event().wait(), timeout=self._health_interval
                    )
                except TimeoutError:
                    pass
                await self._health_check()
                if all(a.status in ("stopped", "crashed") for a in self._agents):
                    logger.info("All agents finished — pool shutting down")
                    break
        except asyncio.CancelledError:
            pass
        finally:
            await self._stop_all()
            if self._api_server is not None:
                self._api_server.close()
                await self._api_server.wait_closed()
            api_task.cancel()
            try:
                await api_task
            except (asyncio.CancelledError, Exception):
                pass

        duration = time.monotonic() - start_time
        result = {
            "agents": [
                {
                    "name": a.name,
                    "status": a.status,
                    "restarts": a.restarts,
                }
                for a in self._agents
            ],
            "duration_s": round(duration, 2),
        }
        logger.info("Pool stopped: %s", result, extra={"event": "pool_stop"})
        return result

    def request_shutdown(self) -> None:
        """Signal the pool to shut down gracefully."""
        self._get_shutdown_event().set()

    def _build_from_count(self) -> list[PoolAgentInfo]:
        """Build agent list from --count with auto-naming (Agent-1..N)."""
        return [PoolAgentInfo(index=i, name=f"Agent-{i + 1}") for i in range(self._count)]

    def _build_from_config_dir(self) -> list[PoolAgentInfo]:
        """Build agent list from .toml files in --config-dir."""
        agents: list[PoolAgentInfo] = []
        if not self._config_dir or not self._config_dir.is_dir():
            logger.warning("Config directory not found: %s", self._config_dir)
            return agents
        for i, path in enumerate(sorted(self._config_dir.glob("*.toml"))):
            agents.append(PoolAgentInfo(index=i, name=path.stem))
        return agents

    async def _start_agent(self, agent: PoolAgentInfo) -> None:
        """Start a single agent subprocess."""
        cmd = [sys.executable, "-m", "agent_runtime", "spawn"]

        if self._config_dir is not None:
            config_file = self._config_dir / f"{agent.name}.toml"
            if config_file.exists():
                cmd.extend(["--config", str(config_file)])

        cmd.extend(["--name", agent.name])

        data_dir = Path("data") / agent.name
        data_dir.mkdir(parents=True, exist_ok=True)
        cmd.extend(["--data-dir", str(data_dir)])

        cmd.extend(self._spawn_args)

        try:
            process = await asyncio.create_subprocess_exec(
                *cmd,
                stdout=asyncio.subprocess.PIPE,
                stderr=asyncio.subprocess.PIPE,
            )
            agent.process = process
            agent.status = "running"
            logger.info(
                "Started agent %s (pid=%d)",
                agent.name,
                process.pid,
                extra={"agent": agent.name, "event": "pool_agent_started"},
            )
        except Exception:
            agent.status = "crashed"
            logger.exception(
                "Failed to start agent %s",
                agent.name,
                extra={"agent": agent.name, "event": "pool_agent_start_failed"},
            )

    async def _stop_agent(self, agent: PoolAgentInfo) -> None:
        """Stop a single agent subprocess gracefully."""
        if agent.process is None or agent.process.returncode is not None:
            return
        try:
            agent.process.terminate()
            try:
                await asyncio.wait_for(agent.process.wait(), timeout=5.0)
            except TimeoutError:
                agent.process.kill()
                await agent.process.wait()
            agent.status = "stopped"
            logger.info(
                "Stopped agent %s",
                agent.name,
                extra={"agent": agent.name, "event": "pool_agent_stopped"},
            )
        except Exception:
            logger.warning(
                "Error stopping agent %s",
                agent.name,
                extra={"agent": agent.name, "event": "pool_agent_stop_error"},
            )

    async def _health_check(self) -> None:
        """Check health and restart crashed agents."""
        for agent in self._agents:
            if agent.status != "running":
                continue
            if agent.process is None or agent.process.returncode is not None:
                if agent.restarts < self._max_restart:
                    agent.restarts += 1
                    logger.info(
                        "Restarting agent %s (attempt %d/%d)",
                        agent.name,
                        agent.restarts,
                        self._max_restart,
                    )
                    await self._start_agent(agent)
                else:
                    agent.status = "crashed"
                    logger.error(
                        "Agent %s exceeded max restarts (%d)",
                        agent.name,
                        self._max_restart,
                        extra={"agent": agent.name, "event": "pool_agent_max_restarts"},
                    )

    async def _stop_all(self) -> None:
        """Stop all agents in the pool."""
        for agent in self._agents:
            if agent.status in ("running",):
                await self._stop_agent(agent)

    async def _start_api_server(self) -> None:
        """Start the Pool API server for health/status."""
        # Minimal TCP server for pool health/status
        async def _handle_connection(
            reader: asyncio.StreamReader, writer: asyncio.StreamWriter
        ) -> None:
            try:
                await asyncio.wait_for(reader.readline(), timeout=5.0)
                agent_statuses = [
                    {"name": a.name, "status": a.status, "restarts": a.restarts}
                    for a in self._agents
                ]
                body = json.dumps({"agents": agent_statuses}, indent=2)
                response = (
                    "HTTP/1.1 200 OK\r\n"
                    "Content-Type: application/json\r\n"
                    f"Content-Length: {len(body)}\r\n"
                    "Connection: close\r\n"
                    "\r\n"
                    f"{body}"
                )
                writer.write(response.encode("ascii"))
                await writer.drain()
            except Exception:
                pass
            finally:
                writer.close()
                try:
                    await writer.wait_closed()
                except Exception:
                    pass

        try:
            self._api_server = await asyncio.start_server(
                _handle_connection,
                host="0.0.0.0",
                port=self._api_port,
            )
            logger.info(
                "Pool API server listening on 0.0.0.0:%d",
                self._api_port,
                extra={"event": "pool_api_started", "port": self._api_port},
            )
            try:
                if self._api_server is not None:
                    await self._api_server.serve_forever()
            except asyncio.CancelledError:
                pass
        except OSError:
            logger.warning("Pool API server: port %d unavailable, skipping", self._api_port)


def _build_pool_spawn_args(args: Any) -> list[str]:
    """Extract the shared spawn flags from parsed pool args into a CLI list."""
    parts: list[str] = []
    if getattr(args, "world_url", None):
        parts.extend(["--world-url", args.world_url])
    if getattr(args, "llm_provider", None):
        parts.extend(["--llm-provider", args.llm_provider])
    if getattr(args, "llm_model", None):
        parts.extend(["--llm-model", args.llm_model])
    if getattr(args, "llm_base_url", None):
        parts.extend(["--llm-base-url", args.llm_base_url])
    if getattr(args, "no_llm", False):
        parts.append("--no-llm")
    if getattr(args, "mock_llm", None):
        parts.extend(["--mock-llm", args.mock_llm])
    if getattr(args, "skills", None):
        parts.extend(["--skills", args.skills])
    if getattr(args, "traits", None):
        parts.extend(["--traits", *args.traits])
    if getattr(args, "tokens", None) is not None:
        parts.extend(["--tokens", str(args.tokens)])
    if getattr(args, "max_tokens", None) is not None:
        parts.extend(["--max-tokens", str(args.max_tokens)])
    if getattr(args, "max_ticks", None) is not None:
        parts.extend(["--max-ticks", str(args.max_ticks)])
    if getattr(args, "tick_interval", None) is not None:
        parts.extend(["--tick-interval", str(args.tick_interval)])
    if getattr(args, "health_port", None) is not None:
        parts.extend(["--health-port", str(args.health_port)])
    if getattr(args, "data_dir", None) is not None:
        parts.extend(["--data-dir", str(args.data_dir)])
    if getattr(args, "preset", None):
        parts.extend(["--preset", args.preset])
    return parts


async def run_pool(args: Any) -> dict[str, Any]:
    """Run an AgentPool from parsed CLI args."""
    spawn_args = _build_pool_spawn_args(args)

    pool = AgentPool(
        count=getattr(args, "count", 1),
        config_dir=getattr(args, "config_dir", None),
        max_restart=getattr(args, "max_restart", 3),
        health_interval=getattr(args, "health_interval", 10.0),
        api_port=getattr(args, "api_port", 9090),
        spawn_args=spawn_args,
    )

    # Graceful shutdown on SIGINT
    loop = asyncio.get_running_loop()

    def _signal_handler() -> None:
        logger.info("Pool received SIGINT — shutting down", extra={"event": "pool_shutdown_signal"})
        pool.request_shutdown()

    loop.add_signal_handler(signal.SIGINT, _signal_handler)

    try:
        result = await pool.run()
    finally:
        try:
            loop.remove_signal_handler(signal.SIGINT)
        except (ValueError, OSError):
            pass

    return result


def _run_publish(args: Any) -> None:
    """Execute the ``publish`` subcommand."""
    from agent_runtime.env_loader import load_dotenv
    from agent_runtime.publish import publish_experiment
    from agent_runtime.publish.backends import TokenMissingError

    if args.env_file is not None:
        load_dotenv(args.env_file)

    try:
        result = asyncio.run(
            publish_experiment(
                experiment_dir=args.experiment_dir,
                backend=args.backend,
                sandbox=not args.production,
                output_path=args.output,
                title=args.title,
                description=args.description,
                package_only=args.package_only,
            )
        )
    except TokenMissingError as exc:
        print(f"error: {exc}", file=sys.stderr)
        sys.exit(2)
    print(json.dumps(result, indent=2, default=str))
