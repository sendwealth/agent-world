# ── Build stage ──────────────────────────────────────────────
FROM rust:1.85-slim AS builder

RUN apt-get update && apt-get install -y protobuf-compiler && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
# Cache dependencies by creating a dummy main.rs
RUN mkdir src && echo "fn main() {}" > src/main.rs && cargo build --release --locked && rm -rf src

COPY src/ src/
RUN touch src/main.rs && cargo build --release --locked

# ── Runtime stage ────────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates curl && rm -rf /var/lib/apt/lists/*

RUN groupadd --system appuser && useradd --system --gid appuser appuser

COPY --from=builder /app/target/release/agent-world-engine /usr/local/bin/

ENV PORT=8080
EXPOSE 8080
USER appuser

ENTRYPOINT ["agent-world-engine"]
