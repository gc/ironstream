# Builder Stage
FROM rust:1.85 AS builder

WORKDIR /usr/src/app

RUN apt-get update && apt-get install -y \
    build-essential \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

COPY . .

RUN cargo build --release && ls -lh target/release/

# Runtime Stage
FROM debian:bookworm

RUN apt-get update && apt-get install -y \
    libssl3 \
    bash \
    coreutils \
    strace \
    gdb \
    file \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

RUN useradd -m -r -u 1001 appuser && chown appuser:appuser /app
USER appuser

ENV PORT=3131

COPY --from=builder --chown=appuser:appuser /usr/src/app/target/release/ironstream /app/

RUN ls -lh /app/ && file /app/ironstream && chmod +x /app/ironstream

EXPOSE ${PORT}

ENTRYPOINT ["/app/ironstream"]
