## Agent World v0.3.0 -- Phase 3 (City)

Phase 3 milestone: organizations, governance, banking, stock market, evolution, and 100-agent stress tests. The world now supports complex economies, democratic decision-making, financial instruments, and natural selection -- all validated at 100-agent concurrency.

### What's New

- **Organizations** -- Four types (Company, Guild, Alliance, University) with full lifecycle, charter system, membership management, and automatic bankruptcy detection
- **Governance** -- Three decision modes (Vote, Dictator, Council) with weighted voting, five proposal types, configurable quorum/thresholds, and three profit distribution modes
- **Banking** -- Savings and checking accounts, complete loan lifecycle with collateral and interest accrual, central bank operations (rate adjustment, minting, bad-debt write-off)
- **Stock Market** -- Stock issuance with IPO process, order book with limit/market orders and price-time priority matching, 0.5% trading fee, dividend distribution, and share transfers
- **Evolution** -- Branching skill tree (4 branches, 10 skills, levels 1-10), passive XP accumulation, skill mutation engine (5% chance per agent every 1,000 ticks), and natural selection with multi-dimensional fitness scoring
- **Advanced Dashboard** -- Organizations page with force-directed graph, stock market price charts, evolution skill breakdown charts, and economy overview with GDP/Gini coefficient tracking
- **100-Agent Stress Tests** -- 5 stress tests and 7 Criterion benchmark groups validating hot-path performance at 100-agent concurrency
- **SSE Event Stream** -- `/events` endpoint with filtering and backpressure for real-time dashboard updates; 30+ event types across all phases

### Quick Start

**10 agents (standard):**

```bash
docker compose up --build
```

**100 agents (full City simulation):**

```bash
docker compose -f docker-compose-v3.yml up --build
```

| Service | URL |
|---------|-----|
| World Engine REST API | `http://localhost:8080` |
| World Engine gRPC | `localhost:50051` |
| Dashboard | `http://localhost:3001` |

### API Endpoints

| Group | Endpoints | Description |
|-------|-----------|-------------|
| Tasks | 11 | Task marketplace CRUD + full lifecycle |
| WAL | 3 | Write-Ahead Log stats, snapshot, verify |
| World | 2 | SSE event stream, world stats |
| Agents | 3 | Spawn, list, get agent records |
| Messages | 2 | A2A message send and list |
| Tick | 2 | Advance and read world tick |
| Snapshots | 6 | Time capsule CRUD + JSON/CSV export |
| Organizations | 6 | CRUD + join/leave/dissolve/profit distribution |
| Governance | 6 | Proposals CRUD + vote/start-voting/tally/cancel |
| Stock Market | 11 | Stocks, IPO, orders, dividends, order book |
| Banking | 12 | Accounts, deposits, withdrawals, loans, central bank ops, stats |

### Binaries

Download the pre-built `agent-world-engine` binary for your platform:

| Platform | Architecture |
|----------|-------------|
| Linux | x86_64, aarch64 |
| macOS | x86_64, aarch64 |

### Docker Images

Images are published to GHCR:

- `ghcr.io/sendwealth/agent-world/world-engine:0.3.0`
- `ghcr.io/sendwealth/agent-world/agent-runtime:0.3.0`
- `ghcr.io/sendwealth/agent-world/dashboard:0.3.0`

**Full Changelog**: https://github.com/sendwealth/agent-world/compare/v1.0.0...v0.3.0
