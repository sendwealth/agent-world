const http = require("http");
const httpProxy = require("http-proxy");

const WORLD_ENGINE_URL =
  process.env.WORLD_ENGINE_URL || "http://127.0.0.1:8080";
const PORT = parseInt(process.env.PORT || "3000", 10);

// Create a proxy instance
const proxy = httpProxy.createProxyServer({
  target: WORLD_ENGINE_URL,
  changeOrigin: true,
});

// Import the original Next.js standalone server
// We'll wrap it to intercept /api/ requests before they reach Next.js
const nextServer = require("./server.js");

// Actually, we can't easily wrap the Next.js server.
// Instead, let's just patch the proxy into the existing server.
// The simplest approach: run a small proxy in front of Next.js.

console.log(`[proxy] Engine URL: ${WORLD_ENGINE_URL}`);
console.log(`[proxy] Listening on :${PORT + 10000}`);
