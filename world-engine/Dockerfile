# ── Build stage ──────────────────────────────────────────────
FROM rust:1.88-slim AS builder

RUN apt-get update && apt-get install -y build-essential protobuf-compiler pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY world-engine/Cargo.toml world-engine/Cargo.lock ./
COPY world-engine/build.rs build.rs
COPY world-engine/benches/ benches/
COPY protocol/ /protocol/
COPY world-engine/src/ src/

RUN cargo build --release --locked

# ── Runtime stage ────────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates curl && rm -rf /var/lib/apt/lists/*

RUN groupadd --system appuser && useradd --system --gid appuser appuser

COPY --from=builder /app/target/release/agent-world-engine /usr/local/bin/

ENV PORT=8080
EXPOSE 8080 50051
USER appuser

ENTRYPOINT ["agent-world-engine"]
