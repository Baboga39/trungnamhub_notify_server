# Multi-stage Dockerfile for trungnamhub_notify_server
FROM rust:1-slim-bookworm AS builder
WORKDIR /app

# Cache dependencies first
COPY Cargo.toml Cargo.lock* ./
RUN mkdir -p src && echo "fn main() {}" > src/main.rs && cargo build --release && rm -rf src

# Build actual application
COPY src ./src
RUN touch src/main.rs && cargo build --release

FROM debian:bookworm-slim AS runtime
WORKDIR /app
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/trungnamhub_notify_server /usr/local/bin/trungnamhub_notify_server

ENV PORT=5002
EXPOSE 5002
CMD ["trungnamhub_notify_server"]

