# ── Build stage ──────────────────────────────────────────────
FROM node:20-slim AS builder

ARG WORLD_ENGINE_URL=http://127.0.0.1:8080
ENV WORLD_ENGINE_URL=${WORLD_ENGINE_URL}
# NEXT_PUBLIC_* vars are baked into client JS at build time
ARG NEXT_PUBLIC_API_URL=http://localhost:8080
ENV NEXT_PUBLIC_API_URL=${NEXT_PUBLIC_API_URL}

WORKDIR /app

COPY package.json package-lock.json ./
RUN npm ci

COPY . .
RUN npm run build

# ── Runtime stage ────────────────────────────────────────────
FROM node:20-slim

ARG WORLD_ENGINE_URL=http://127.0.0.1:8080
ENV WORLD_ENGINE_URL=${WORLD_ENGINE_URL}

WORKDIR /app

ENV NODE_ENV=production
ENV NEXT_TELEMETRY_DISABLED=1

RUN groupadd --system appuser && useradd --system --gid appuser appuser

COPY --from=builder /app/.next/standalone ./
COPY --from=builder /app/.next/static ./.next/static
COPY --from=builder /app/public ./public

USER appuser
ENV PORT=3000
EXPOSE 3000

CMD ["node", "server.js"]
