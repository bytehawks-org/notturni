# syntax=docker/dockerfile:1
#
# Backend Rust: un'unica immagine con `noct-api`, `noct-worker` e `noct-ctl` (il comando decide il
# ruolo). Agnostica rispetto all'architettura: la stessa Dockerfile produce x86_64 e ARM64
# (`docker buildx build --platform linux/amd64,linux/arm64`), nessun tag di base specifico.

ARG RUST_VERSION=1.97

# --- ricetta delle dipendenze (cache dei layer: cambia solo se cambiano i Cargo.toml) ---------------
FROM docker.io/lukemathwalker/cargo-chef:latest-rust-${RUST_VERSION}-slim-trixie AS chef
WORKDIR /src

FROM chef AS planner
COPY Cargo.toml Cargo.lock rustfmt.toml ./
COPY crates ./crates
COPY modules ./modules
COPY bins ./bins
RUN cargo chef prepare --recipe-path recipe.json

# --- compilazione ------------------------------------------------------------------------------------
FROM chef AS builder
# cmake e un compilatore C servono a aws-lc-sys (TLS di rustls)
RUN apt-get update && apt-get install --no-install-recommends -y build-essential cmake pkg-config \
    && rm -rf /var/lib/apt/lists/*
COPY --from=planner /src/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY Cargo.toml Cargo.lock rustfmt.toml ./
COPY crates ./crates
COPY modules ./modules
COPY bins ./bins
RUN cargo build --release --locked -p noct-api -p noct-worker -p noct-ctl

# --- immagine finale ---------------------------------------------------------------------------------
FROM docker.io/library/debian:trixie-slim AS runtime
# postgresql-client: `pg_dump` del backup infrastrutturale (supporta server con versione <= alla sua:
# quella di Debian è più recente di postgres:16). ca-certificates: TLS verso SMTP, S3 e provider SSO.
RUN apt-get update \
    && apt-get install --no-install-recommends -y ca-certificates postgresql-client tini \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --create-home --shell /usr/sbin/nologin appuser
COPY --from=builder /src/target/release/noct-api /src/target/release/noct-worker /src/target/release/noct-ctl /usr/local/bin/
# storage locale (installazioni `solo`): volume scrivibile dall'utente applicativo
RUN mkdir -p /data/storage && chown -R appuser:appuser /data
ENV NOCT_BIND_ADDR=0.0.0.0:8000 \
    NOCT_LOCAL_STORAGE_BASE_PATH=/data/storage
USER appuser
WORKDIR /home/appuser
EXPOSE 8000
ENTRYPOINT ["/usr/bin/tini", "--"]
CMD ["noct-api"]
