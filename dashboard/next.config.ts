import type { NextConfig } from "next";

const WORLD_ENGINE_URL =
  process.env.WORLD_ENGINE_URL ?? "http://127.0.0.1:3000";

const nextConfig: NextConfig = {
  async rewrites() {
    return [
      {
        source: "/api/v1/:path*",
        destination: `${WORLD_ENGINE_URL}/:path*`,
      },
    ];
  },
};

export default nextConfig;
