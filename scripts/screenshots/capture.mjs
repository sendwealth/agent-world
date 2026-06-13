#!/usr/bin/env node
/**
 * Dashboard Screenshot Automation Tool
 * =====================================
 *
 * Captures full-page screenshots of every major Dashboard route at 1920×1080
 * using Playwright. Designed for open-source promotional material (README,
 * docs site, social media).
 *
 * Usage
 * -----
 *   node capture.mjs                       # capture all routes
 *   node capture.mjs --only world,agents   # capture a subset (comma-sep slugs)
 *   node capture.mjs --out ./docs/screenshots
 *   DASHBOARD_URL=http://localhost:3001 node capture.mjs
 *
 * Prerequisites
 * -------------
 *   1. World engine + Dashboard running (e.g. `make dev` or `make run-dashboard`).
 *   2. Playwright Chromium installed: `npm run install-browsers` (in this dir).
 *
 * Exit codes
 * ----------
 *   0  all screenshots captured
 *   1  one or more screenshots failed
 *   2  dashboard unreachable
 */

import { chromium } from "playwright";
import { mkdir, readdir, rm } from "node:fs/promises";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

// ─── Config ─────────────────────────────────────────────────────────────

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(__dirname, "..", "..");

const DASHBOARD_URL =
  process.env.DASHBOARD_URL ?? "http://localhost:3000";

const DEFAULT_OUT_DIR = resolve(ROOT, "docs", "screenshots");

// Viewport: 1920×1080 as required by the issue.
const VIEWPORT = { width: 1920, height: 1080 };

// Per-route capture timeout (ms).
const NAV_TIMEOUT = 30_000;
const STABILISE = 1_500; // extra wait after load for animations / SSE frames

// ─── Route catalogue ────────────────────────────────────────────────────
//
// Each entry maps a numbered filename to a dashboard path plus a "ready"
// strategy. Ready strategies:
//   selector  — wait for a CSS selector to appear (strong signal).
//   networkidle — wait for network to settle (fallback for data-light pages).
//   text      — wait for visible text fragment.
//
// The slug doubles as the `--only` filter key.

const ROUTES = [
  {
    slug: "world-overview",
    file: "01-world-overview.png",
    path: "/",
    ready: { selector: "h1" },
  },
  {
    slug: "agents",
    file: "02-agents.png",
    path: "/agents",
    ready: { selector: "h1" },
  },
  {
    slug: "agent-detail",
    file: "03-agent-detail.png",
    path: "/agents",
    ready: { selector: "h1" },
    dynamic: true, // needs a real agent id — resolved at runtime
  },
  {
    slug: "organizations",
    file: "04-organizations.png",
    path: "/organizations",
    ready: { selector: "h1", fallback: "networkidle" },
  },
  {
    slug: "stocks",
    file: "05-stocks.png",
    path: "/stocks",
    ready: { selector: "h1", fallback: "networkidle" },
  },
  {
    slug: "evolution",
    file: "06-evolution.png",
    path: "/evolution",
    ready: { selector: "h1", fallback: "networkidle" },
  },
  {
    slug: "economy",
    file: "07-economy.png",
    path: "/economy",
    ready: { selector: "h1", fallback: "networkidle" },
  },
  {
    slug: "governance",
    file: "08-governance.png",
    path: "/governance",
    ready: { selector: "h1", fallback: "networkidle" },
  },
  {
    slug: "timeline",
    file: "09-timeline.png",
    path: "/timeline",
    ready: { selector: "h1", fallback: "networkidle" },
  },
  {
    slug: "tasks",
    file: "10-tasks.png",
    path: "/tasks",
    ready: { selector: "h1", fallback: "networkidle" },
  },
  {
    slug: "traces",
    file: "11-traces.png",
    path: "/traces",
    ready: { selector: "h1", fallback: "networkidle" },
  },
  {
    slug: "marketplace",
    file: "12-marketplace.png",
    path: "/marketplace",
    ready: { selector: "h1", fallback: "networkidle" },
  },
  {
    slug: "feed",
    file: "13-feed.png",
    path: "/feed",
    ready: { selector: "h1", fallback: "networkidle" },
  },
  {
    slug: "briefing",
    file: "14-briefing.png",
    path: "/briefing",
    ready: { selector: "h1", fallback: "networkidle" },
  },
  {
    slug: "diplomacy",
    file: "15-diplomacy.png",
    path: "/diplomacy",
    ready: { selector: "h1", fallback: "networkidle" },
  },
];

// ─── CLI parsing ────────────────────────────────────────────────────────

function parseArgs(argv) {
  const args = { only: null, out: DEFAULT_OUT_DIR, clean: false };
  for (let i = 2; i < argv.length; i++) {
    const a = argv[i];
    if (a === "--only") args.only = argv[++i]?.split(",").map((s) => s.trim());
    else if (a === "--out") args.out = argv[++i];
    else if (a === "--clean") args.clean = true;
    else if (a === "-h" || a === "--help") {
      printHelp();
      process.exit(0);
    }
  }
  return args;
}

function printHelp() {
  console.log(`
Dashboard Screenshot Tool

Usage:
  node capture.mjs [options]

Options:
  --only <slugs>   Comma-separated route slugs to capture (default: all)
  --out <dir>      Output directory (default: docs/screenshots/)
  --clean          Delete existing *.png in output dir before capturing
  -h, --help       Show this help

Environment:
  DASHBOARD_URL    Dashboard base URL (default: http://localhost:3000)
                   Use http://localhost:3001 when running via Docker Compose.
`);
}

// ─── Helpers ────────────────────────────────────────────────────────────

const RED = "\x1b[31m";
const GREEN = "\x1b[32m";
const YELLOW = "\x1b[33m";
const CYAN = "\x1b[36m";
const DIM = "\x1b[2m";
const RESET = "\x1b[0m";

function ok(msg) {
  console.log(`${GREEN}✓${RESET} ${msg}`);
}
function warn(msg) {
  console.log(`${YELLOW}⚠${RESET} ${msg}`);
}
function fail(msg) {
  console.error(`${RED}✗${RESET} ${msg}`);
}
function info(msg) {
  console.log(`${CYAN}→${RESET} ${msg}`);
}

/**
 * Check that the dashboard is reachable before starting captures.
 * Exits with code 2 if not reachable.
 */
async function assertDashboardReachable(baseUrl) {
  const url = baseUrl.replace(/\/$/, "") + "/api/health";
  info(`Checking dashboard health at ${url} …`);
  try {
    const res = await fetch(url, { signal: AbortSignal.timeout(5_000) });
    if (!res.ok) {
      fail(`Health endpoint returned HTTP ${res.status}.`);
      process.exit(2);
    }
    ok(`Dashboard reachable at ${baseUrl}`);
  } catch (err) {
    fail(
      `Cannot reach dashboard at ${baseUrl}.\n` +
        `  ${DIM}${err.message}${RESET}\n` +
        `  Make sure the dashboard is running (e.g. ${CYAN}make run-dashboard${RESET}).\n` +
        `  If using Docker Compose, set ${CYAN}DASHBOARD_URL=http://localhost:3001${RESET}.`
    );
    process.exit(2);
  }
}

/**
 * Resolve a dynamic route that needs a real resource id (e.g. agent id).
 * Fetches the list endpoint and picks the first id.
 */
async function resolveDynamicPath(route, baseUrl) {
  if (route.slug === "agent-detail") {
    // Try /api/v1/agents first; fall back to scraping an <a> from /agents.
    try {
      const res = await fetch(`${baseUrl}/api/v1/agents`);
      if (res.ok) {
        const body = await res.json();
        const list = body?.agents ?? body?.data ?? body;
        if (Array.isArray(list) && list.length > 0) {
          const id = list[0]?.id ?? list[0]?.agent_id;
          if (id) return `/agents/${id}`;
        }
      }
    } catch {
      /* fall through to DOM scrape */
    }
    // Fallback: scrape the first agent link from the DOM.
    // Handled in captureRoute via page evaluation.
    return null;
  }
  return route.path;
}

/**
 * Wait for a page to be "ready" using the route's strategy.
 */
async function waitReady(page, route) {
  const strategy = route.ready;
  const waiters = [];

  if (strategy.selector) {
    waiters.push(
      page
        .waitForSelector(strategy.selector, { timeout: 15_000 })
        .catch(() => {
          if (!strategy.fallback) {
            throw new Error(
              `Selector "${strategy.selector}" not found within 15s`
            );
          }
        })
    );
  }

  if (strategy.fallback === "networkidle" || !strategy.selector) {
    waiters.push(
      page.waitForLoadState("networkidle", { timeout: 15_000 }).catch(() => {})
    );
  }

  await Promise.all(waiters);
  // Extra stabilisation for animations / SSE-driven re-renders.
  await page.waitForTimeout(STABILISE);
}

/**
 * Capture a single route.
 */
async function captureRoute(browser, route, baseUrl, outDir) {
  const url = baseUrl.replace(/\/$/, "") + route.path;
  info(`Capturing ${route.slug} (${route.file}) ← ${url}`);

  const page = await browser.newPage({
    viewport: VIEWPORT,
    deviceScaleFactor: 1,
  });
  page.setDefaultNavigationTimeout(NAV_TIMEOUT);

  try {
    await page.goto(url, { waitUntil: "domcontentloaded" });

    // Resolve dynamic routes after initial load.
    if (route.dynamic) {
      const resolved = await resolveDynamicPath(route, baseUrl);
      if (resolved) {
        await page.goto(baseUrl.replace(/\/$/, "") + resolved, {
          waitUntil: "domcontentloaded",
        });
        route = { ...route, path: resolved };
      } else {
        // Try scraping first agent link from the DOM.
        const href = await page
          .evaluate(() => {
            const a = document.querySelector('a[href^="/agents/"]');
            return a?.getAttribute("href") ?? null;
          })
          .catch(() => null);
        if (href && href !== "/agents") {
          await page.goto(baseUrl.replace(/\/$/, "") + href, {
            waitUntil: "domcontentloaded",
          });
        } else {
          warn(
            `No agent id found for ${route.slug} — capturing the /agents list as fallback.`
          );
        }
      }
    }

    await waitReady(page, route);

    const outPath = resolve(outDir, route.file);
    await page.screenshot({
      path: outPath,
      fullPage: false, // viewport-only: 1920×1080, matching the issue spec
    });
    ok(`Saved ${route.file}`);
    return true;
  } catch (err) {
    fail(`Failed to capture ${route.slug}: ${err.message}`);
    // Save a last-resort screenshot so we can debug.
    try {
      await page.screenshot({ path: resolve(outDir, route.file) });
      warn(`Saved (best-effort) ${route.file}`);
    } catch {
      /* nothing more we can do */
    }
    return false;
  } finally {
    await page.close();
  }
}

// ─── Main ───────────────────────────────────────────────────────────────

async function main() {
  const args = parseArgs(process.argv);

  const selectedRoutes = args.only
    ? ROUTES.filter((r) => args.only.includes(r.slug))
    : ROUTES;

  if (selectedRoutes.length === 0) {
    fail(`No routes matched --only=${args.only?.join(",")}`);
    fail(`Available slugs: ${ROUTES.map((r) => r.slug).join(", ")}`);
    process.exit(1);
  }

  // Ensure output dir exists.
  await mkdir(args.out, { recursive: true });

  // Optionally clean existing PNGs.
  if (args.clean) {
    info(`Cleaning existing PNGs in ${args.out} …`);
    try {
      const entries = await readdir(args.out);
      for (const f of entries) {
        if (f.endsWith(".png")) await rm(resolve(args.out, f));
      }
    } catch {
      /* dir may not exist yet */
    }
  }

  await assertDashboardReachable(DASHBOARD_URL);

  info(
    `Capturing ${selectedRoutes.length} route(s) at ${VIEWPORT.width}×${VIEWPORT.height} …`
  );

  const browser = await chromium.launch({ headless: true });
  let success = 0;
  let failed = 0;

  try {
    for (const route of selectedRoutes) {
      const ok_flag = await captureRoute(
        browser,
        route,
        DASHBOARD_URL,
        args.out
      );
      if (ok_flag) success++;
      else failed++;
    }
  } finally {
    await browser.close();
  }

  console.log();
  if (failed === 0) {
    ok(
      `All ${success} screenshot(s) saved to ${args.out} (${success}/${success})`
    );
    process.exit(0);
  } else {
    fail(
      `${failed} screenshot(s) failed, ${success} succeeded. ` +
        `See errors above. Output dir: ${args.out}`
    );
    process.exit(1);
  }
}

main().catch((err) => {
  fail(`Unhandled error: ${err.message}`);
  console.error(err.stack);
  process.exit(1);
});
