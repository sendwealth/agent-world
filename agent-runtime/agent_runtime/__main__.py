"""Agent World — Agent Runtime CLI entry point."""

import argparse
import asyncio
import os
import sys


def main() -> None:
    parser = argparse.ArgumentParser(description="Agent World Runtime")
    sub = parser.add_subparsers(dest="command")

    spawn_parser = sub.add_parser("spawn", help="Spawn agents")
    spawn_parser.add_argument("--count", type=int, default=2, help="Number of agents to spawn")

    args = parser.parse_args()

    if args.command == "spawn":
        world_url = os.environ.get("WORLD_ENGINE_URL", "http://127.0.0.1:3000")
        count = args.count
        print(f"Spawning {count} agent(s) connecting to {world_url}")
        # TODO: implement actual agent spawning once the think loop is wired up
        for i in range(count):
            print(f"  Agent {i + 1}/{count}: initialized")
        print(f"All {count} agent(s) running.")
        # Keep the process alive
        try:
            asyncio.get_event_loop().run_forever()
        except KeyboardInterrupt:
            print("\nShutting down agents...")
    else:
        parser.print_help()
        sys.exit(1)


if __name__ == "__main__":
    main()
