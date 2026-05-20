/**
 * Data loading utilities.
 *
 * During development we use mock data (lib/mock-data.ts).
 * When the data-generation sub-task (SEN-239) finishes, JSON files will
 * appear in public/data/ and this module will read them instead.
 */

import type {
  DemoAgent,
  DemoOrganization,
  EmergenceEvent,
  MetricSeries,
  SandboxTemplate,
  WorldSnapshot,
} from "@/types/demo";
import { getMockData, SANDBOX_TEMPLATES } from "./mock-data";

// Set to true once real JSON data files are available in public/data/
const USE_REAL_DATA = false;

// ── Generic fetcher with fallback to mock ────────────────────────────

async function fetchJSON<T>(filename: string, mockFallback: () => T): Promise<T> {
  if (!USE_REAL_DATA) return mockFallback();
  try {
    const res = await fetch(`/data/${filename}`);
    if (!res.ok) throw new Error(`Failed to load ${filename}`);
    return (await res.json()) as T;
  } catch {
    return mockFallback();
  }
}

// ── Public API ──────────────────────────────────────────────────────

export async function loadAgents(): Promise<DemoAgent[]> {
  return fetchJSON("agents.json", () => getMockData().agents);
}

export async function loadEvents(): Promise<EmergenceEvent[]> {
  return fetchJSON("events.json", () => getMockData().events);
}

export async function loadSnapshots(): Promise<WorldSnapshot[]> {
  return fetchJSON("snapshots.json", () => getMockData().snapshots);
}

export async function loadMetrics(): Promise<Record<string, MetricSeries>> {
  return fetchJSON("metrics.json", () => getMockData().metrics);
}

export async function loadOrganizations(): Promise<DemoOrganization[]> {
  return fetchJSON("organizations.json", () => getMockData().organizations);
}

export function loadTemplates(): SandboxTemplate[] {
  return SANDBOX_TEMPLATES;
}
