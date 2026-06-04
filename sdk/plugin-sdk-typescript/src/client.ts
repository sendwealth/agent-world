/**
 * Agent World Plugin SDK — HTTP Client for the Plugin API
 *
 * Provides a `PluginClient` class for interacting with the Agent World
 * engine's plugin management endpoints over HTTP (fetch-based).
 */

import type {
  PluginInfo,
  PluginListResponse,
  PluginActionResponse,
  RegisterPluginRequest,
  LoadWasmRequest,
  ExecuteRequest,
  ActionResult,
} from "./types.js";

// ─── Error ───────────────────────────────────────────────────────────────

/** Raised when the plugin API returns an error response. */
export class PluginClientError extends Error {
  /** HTTP status code (0 for network errors). */
  readonly statusCode: number;

  constructor(statusCode: number, message: string) {
    super(`HTTP ${statusCode}: ${message}`);
    this.name = "PluginClientError";
    this.statusCode = statusCode;
  }
}

// ─── Client ──────────────────────────────────────────────────────────────

/** Configuration options for {@link PluginClient}. */
export interface PluginClientOptions {
  /** Base URL of the Agent World engine (e.g. `"http://localhost:8080"`). */
  baseUrl?: string;
  /** Optional API key for authentication (Bearer token). */
  apiKey?: string;
  /** Request timeout in milliseconds (default: 30_000). */
  timeout?: number;
}

/**
 * HTTP client for the Agent World plugin management API.
 *
 * Uses the standard `fetch` API (available in Node ≥ 18 and modern browsers).
 *
 * @example
 * ```typescript
 * const client = new PluginClient({ baseUrl: "http://localhost:8080" });
 *
 * // Register a plugin
 * const reg = await client.register({
 *   id: "me/my-plugin",
 *   name: "My Plugin",
 *   version: "0.1.0",
 *   description: "A test plugin",
 *   author: "Me",
 * });
 *
 * // List all plugins
 * const list = await client.list();
 * console.log(`Active: ${list.active} / ${list.total}`);
 *
 * // Enable it
 * await client.enable("me/my-plugin");
 * ```
 */
export class PluginClient {
  private readonly baseUrl: string;
  private readonly apiKey?: string;
  private readonly timeout: number;

  constructor(options: PluginClientOptions = {}) {
    this.baseUrl = (options.baseUrl ?? "http://localhost:8080").replace(
      /\/+$/,
      "",
    );
    this.apiKey = options.apiKey;
    this.timeout = options.timeout ?? 30_000;
  }

  // ─── Plugin Management ─────────────────────────────────────────────

  /**
   * Register a new plugin with the engine.
   *
   * @param req - Plugin registration details.
   * @returns Registration confirmation from the engine.
   */
  async register(req: RegisterPluginRequest): Promise<PluginActionResponse> {
    return this.request<PluginActionResponse>(
      "POST",
      "/api/v1/plugins/register",
      req,
    );
  }

  /**
   * List all registered plugins.
   *
   * @returns Object containing the plugin list and counts.
   */
  async list(): Promise<PluginListResponse> {
    return this.request<PluginListResponse>("GET", "/api/v1/plugins");
  }

  /**
   * Get details for a specific plugin.
   *
   * @param id - Plugin identifier.
   * @returns Plugin metadata.
   */
  async get(id: string): Promise<PluginInfo> {
    return this.request<PluginInfo>("GET", `/api/v1/plugins/${encodeURIComponent(id)}`);
  }

  /**
   * Enable a registered plugin.
   *
   * @param id - Plugin identifier to enable.
   * @returns Action response confirming enable.
   */
  async enable(id: string): Promise<PluginActionResponse> {
    return this.request<PluginActionResponse>(
      "POST",
      `/api/v1/plugins/${encodeURIComponent(id)}/enable`,
    );
  }

  /**
   * Disable an active plugin.
   *
   * @param id - Plugin identifier to disable.
   * @returns Action response confirming disable.
   */
  async disable(id: string): Promise<PluginActionResponse> {
    return this.request<PluginActionResponse>(
      "POST",
      `/api/v1/plugins/${encodeURIComponent(id)}/disable`,
    );
  }

  // ─── WASM Sandbox ──────────────────────────────────────────────────

  /**
   * Load a WASM module into the sandbox for a registered plugin.
   *
   * @param req - Request containing the plugin ID and base64-encoded WASM binary.
   * @returns Load confirmation from the engine.
   */
  async loadWasm(req: LoadWasmRequest): Promise<PluginActionResponse> {
    return this.request<PluginActionResponse>(
      "POST",
      "/api/v1/plugins/sandbox/load",
      req,
    );
  }

  /**
   * Load a WASM module from a raw byte array.
   *
   * @param pluginId - Plugin identifier to associate with the module.
   * @param wasmBytes - Raw WASM binary content.
   * @returns Load confirmation from the engine.
   */
  async loadWasmBytes(
    pluginId: string,
    wasmBytes: Uint8Array,
  ): Promise<PluginActionResponse> {
    // Encode Uint8Array to base64 in a way that works in both Node and browsers
    const base64 =
      typeof Buffer !== "undefined"
        ? Buffer.from(wasmBytes).toString("base64")
        : btoa(String.fromCharCode(...wasmBytes));

    return this.loadWasm({ plugin_id: pluginId, wasm_base64: base64 });
  }

  // ─── Execution ─────────────────────────────────────────────────────

  /**
   * Execute a plugin skill for an agent.
   *
   * @param req - Execution request with plugin, skill, agent, and optional params.
   * @returns Action result from the plugin execution.
   */
  async execute(req: ExecuteRequest): Promise<ActionResult> {
    return this.request<ActionResult>(
      "POST",
      `/api/v1/plugins/sandbox/${encodeURIComponent(req.plugin_id)}/execute`,
      req,
    );
  }

  // ─── Health ────────────────────────────────────────────────────────

  /**
   * Check engine health.
   *
   * @returns Health status object.
   */
  async health(): Promise<Record<string, unknown>> {
    return this.request<Record<string, unknown>>("GET", "/api/v1/health");
  }

  // ─── Internal ──────────────────────────────────────────────────────

  /**
   * Perform an HTTP request against the engine API.
   */
  private async request<T>(
    method: string,
    path: string,
    body?: unknown,
  ): Promise<T> {
    const url = `${this.baseUrl}${path}`;
    const headers: Record<string, string> = {
      Accept: "application/json",
    };

    if (this.apiKey) {
      headers["Authorization"] = `Bearer ${this.apiKey}`;
    }

    let requestBody: string | undefined;
    if (body !== undefined) {
      requestBody = JSON.stringify(body);
      headers["Content-Type"] = "application/json";
    }

    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), this.timeout);

    try {
      const response = await fetch(url, {
        method,
        headers,
        body: requestBody,
        signal: controller.signal,
      });

      if (!response.ok) {
        const text = await response.text().catch(() => "");
        throw new PluginClientError(response.status, text);
      }

      const text = await response.text();
      if (!text) {
        return {} as T;
      }
      return JSON.parse(text) as T;
    } catch (err) {
      if (err instanceof PluginClientError) throw err;
      if (err instanceof DOMException && err.name === "AbortError") {
        throw new PluginClientError(0, `Request timed out after ${this.timeout}ms`);
      }
      if (err instanceof TypeError) {
        throw new PluginClientError(0, `Connection error: ${(err as Error).message}`);
      }
      throw new PluginClientError(0, `Unexpected error: ${(err as Error).message}`);
    } finally {
      clearTimeout(timer);
    }
  }
}
