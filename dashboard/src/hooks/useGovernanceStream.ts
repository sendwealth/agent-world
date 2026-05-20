"use client";

import { useEffect, useState, useCallback } from "react";
import type {
  WorldGovernanceSummary,
  OrgMetrics,
  GovernanceTimelineEvent,
} from "@/types/world";
import type { WorldEvent, Organization } from "@/types/world";
import { fetchJSON } from "@/lib/api";
import { useSSEContext } from "@/components/SSEProvider";

const GOVERNANCE_EVENT_TYPES = new Set([
  "tax_collected",
  "treasury_distributed",
  "leadership_election_started",
  "leadership_changed",
  "treaty_proposed",
  "treaty_signed",
  "treaty_broken",
  "relation_changed",
]);

function isGovernanceEvent(event: WorldEvent): boolean {
  return GOVERNANCE_EVENT_TYPES.has(event.type);
}

export function useGovernanceSummary() {
  const [summary, setSummary] = useState<WorldGovernanceSummary | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const sse = useSSEContext();

  const loadSummary = useCallback(async () => {
    try {
      const data = await fetchJSON<WorldGovernanceSummary>(
        "/api/v1/governance/summary"
      );
      setSummary(data);
      setError(null);
    } catch {
      setError("无法加载治理概览数据");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    (async () => {
      await loadSummary();
    })();
    const interval = setInterval(loadSummary, 10000);
    return () => clearInterval(interval);
  }, [loadSummary]);

  useEffect(() => {
    function onEvent(event: WorldEvent) {
      if (isGovernanceEvent(event)) {
        loadSummary();
      }
    }
    const unsubscribe = sse.subscribe(onEvent);
    return unsubscribe;
  }, [sse, loadSummary]);

  return { summary, loading, error };
}

/**
 * Fetches all orgs via /api/v1/orgs, then fetches governance metrics
 * for each via /api/v1/governance/comparison. Returns enriched org
 * metrics with name populated from the org list.
 */
export function useGovernanceOverview() {
  const [orgMetrics, setOrgMetrics] = useState<
    (OrgMetrics & { org_name: string })[]
  >([]);
  const [summary, setSummary] = useState<WorldGovernanceSummary | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const sse = useSSEContext();

  const loadData = useCallback(async () => {
    try {
      const [orgs, govSummary] = await Promise.all([
        fetchJSON<Organization[]>("/api/v1/orgs"),
        fetchJSON<WorldGovernanceSummary>("/api/v1/governance/summary"),
      ]);

      setSummary(govSummary);

      const activeOrgs = orgs.filter((o) => o.status === "active");
      if (activeOrgs.length === 0) {
        setOrgMetrics([]);
        setError(null);
        return;
      }

      const orgIds = activeOrgs.map((o) => o.id);
      try {
        const metrics = await fetchJSON<OrgMetrics[]>(
          `/api/v1/governance/comparison?org_ids=${orgIds.join(",")}`
        );
        // Merge org names from the org list
        const orgNameMap = new Map(activeOrgs.map((o) => [o.id, o.name]));
        const enriched = metrics.map((m) => ({
          ...m,
          org_name: orgNameMap.get(m.org_id) ?? m.org_id,
        }));
        setOrgMetrics(enriched);
      } catch {
        // comparison endpoint may 404 if no metrics yet
        setOrgMetrics([]);
      }
      setError(null);
    } catch {
      setError("无法加载治理数据");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    (async () => {
      await loadData();
    })();
    const interval = setInterval(loadData, 10000);
    return () => clearInterval(interval);
  }, [loadData]);

  useEffect(() => {
    function onEvent(event: WorldEvent) {
      if (isGovernanceEvent(event)) {
        loadData();
      }
    }
    const unsubscribe = sse.subscribe(onEvent);
    return unsubscribe;
  }, [sse, loadData]);

  return { orgMetrics, summary, loading, error };
}

export function useGovernanceOrg(orgId: string) {
  const [metrics, setMetrics] = useState<OrgMetrics | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const sse = useSSEContext();

  const loadMetrics = useCallback(async () => {
    try {
      const data = await fetchJSON<OrgMetrics>(
        `/api/v1/governance/orgs/${orgId}`
      );
      setMetrics(data);
      setError(null);
    } catch {
      setError("无法加载组织治理数据");
    } finally {
      setLoading(false);
    }
  }, [orgId]);

  useEffect(() => {
    (async () => {
      await loadMetrics();
    })();
    const interval = setInterval(loadMetrics, 10000);
    return () => clearInterval(interval);
  }, [loadMetrics]);

  useEffect(() => {
    function onEvent(event: WorldEvent) {
      if (isGovernanceEvent(event)) {
        loadMetrics();
      }
    }
    const unsubscribe = sse.subscribe(onEvent);
    return unsubscribe;
  }, [sse, loadMetrics]);

  return { metrics, loading, error };
}

export function useGovernanceTimeline(
  orgId?: string,
  eventType?: string
) {
  const [events, setEvents] = useState<GovernanceTimelineEvent[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const sse = useSSEContext();

  const loadTimeline = useCallback(async () => {
    try {
      const params = new URLSearchParams();
      if (eventType) params.set("event_type", eventType);
      const query = params.toString();
      const path = orgId
        ? `/api/v1/governance/orgs/${orgId}/timeline${query ? `?${query}` : ""}`
        : `/api/v1/governance/timeline${query ? `?${query}` : ""}`;
      const data = await fetchJSON<GovernanceTimelineEvent[]>(path);
      setEvents(data);
      setError(null);
    } catch {
      setError("无法加载治理时间线");
    } finally {
      setLoading(false);
    }
  }, [orgId, eventType]);

  useEffect(() => {
    (async () => {
      await loadTimeline();
    })();
    const interval = setInterval(loadTimeline, 10000);
    return () => clearInterval(interval);
  }, [loadTimeline]);

  useEffect(() => {
    function onEvent(event: WorldEvent) {
      if (isGovernanceEvent(event)) {
        loadTimeline();
      }
    }
    const unsubscribe = sse.subscribe(onEvent);
    return unsubscribe;
  }, [sse, loadTimeline]);

  return { events, loading, error };
}

export function useGovernanceComparison(orgIds?: string[]) {
  const [orgs, setOrgs] = useState<OrgMetrics[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const sse = useSSEContext();

  const loadComparison = useCallback(async () => {
    try {
      const params = new URLSearchParams();
      if (orgIds && orgIds.length > 0) {
        params.set("org_ids", orgIds.join(","));
      }
      const query = params.toString();
      const data = await fetchJSON<OrgMetrics[]>(
        `/api/v1/governance/comparison${query ? `?${query}` : ""}`
      );
      setOrgs(data);
      setError(null);
    } catch {
      setError("无法加载治理对比数据");
    } finally {
      setLoading(false);
    }
  }, [orgIds]);

  useEffect(() => {
    (async () => {
      await loadComparison();
    })();
    const interval = setInterval(loadComparison, 10000);
    return () => clearInterval(interval);
  }, [loadComparison]);

  useEffect(() => {
    function onEvent(event: WorldEvent) {
      if (isGovernanceEvent(event)) {
        loadComparison();
      }
    }
    const unsubscribe = sse.subscribe(onEvent);
    return unsubscribe;
  }, [sse, loadComparison]);

  return { orgs, loading, error };
}
