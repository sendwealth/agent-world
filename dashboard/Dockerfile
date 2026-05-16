# ── Build stage ──────────────────────────────────────────────
FROM node:20-slim AS builder

ARG WORLD_ENGINE_URL=http://127.0.0.1:3000
ENV WORLD_ENGINE_URL=${WORLD_ENGINE_URL}

WORKDIR /app

COPY package.json package-lock.json ./
RUN npm ci

COPY . .
RUN npm run build

# ── Runtime stage ────────────────────────────────────────────
FROM node:20-slim

ARG WORLD_ENGINE_URL=http://127.0.0.1:3000
ENV WORLD_ENGINE_URL=${WORLD_ENGINE_URL}

WORKDIR /app

ENV NODE_ENV=production
ENV NEXT_TELEMETRY_DISABLED=1

RUN groupadd --system appuser && useradd --system --gid appuser appuser

COPY --from=builder /app/.next/standalone ./
COPY --from=builder /app/.next/static ./.next/static
COPY --from=builder /app/public ./public

USER appuser
EXPOSE 3001

CMD ["node", "server.js"]
