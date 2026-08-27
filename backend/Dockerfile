# ===== Stage 1: Build =====
FROM rust:1.93-slim-bookworm AS builder

RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY migration/ migration/

# Cache dependencies (dummy build for all bin targets)
# Cargo.toml declares 3 [[bin]]: ironix_pay, seed_blacklist, encrypt_secrets
# Additional bins auto-discovered from src/bin/: export_bindings
RUN mkdir -p src/bin && \
    echo "fn main() {}" > src/main.rs && \
    echo "fn main() {}" > src/bin/seed_blacklist.rs && \
    echo "fn main() {}" > src/bin/encrypt_secrets.rs && \
    echo "fn main() {}" > src/bin/export_bindings.rs && \
    cargo build --release 2>/dev/null || true && \
    rm -rf src

# Build actual source
COPY src/ src/
COPY docs/ docs/
RUN touch src/main.rs && cargo build --release

# ===== Stage 2: Runtime =====
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates libssl3 curl && \
    rm -rf /var/lib/apt/lists/* && \
    groupadd -r ironix && useradd -r -g ironix ironix

WORKDIR /app
COPY --from=builder /app/target/release/ironix_pay .
COPY --from=builder /app/target/release/seed_blacklist .
COPY --from=builder /app/target/release/encrypt_secrets .

USER ironix
EXPOSE 3000

HEALTHCHECK --interval=30s --timeout=5s --retries=3 \
    CMD curl -f http://localhost:3000/health || exit 1

ENTRYPOINT ["./ironix_pay"]
