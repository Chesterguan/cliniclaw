# ClinicClaw API server — multi-stage Rust build
FROM rust:1.83-slim AS builder

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/

# Build release binary
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
RUN cargo build --release --bin cliniclaw

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/cliniclaw /usr/local/bin/cliniclaw
COPY crates/cliniclaw-policy/policies/ /app/policies/

WORKDIR /app

ENV LISTEN_ADDR=0.0.0.0:3000
ENV CLINICLAW_MOCK=true
ENV DATABASE_URL=sqlite:cliniclaw.sqlite

EXPOSE 3000

CMD ["cliniclaw"]
