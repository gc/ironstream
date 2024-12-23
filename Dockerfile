FROM rust:1.73 AS builder

WORKDIR /usr/src/app

COPY . .

RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

ENV PORT=3131

COPY --from=builder /usr/src/app/target/release/ironstream /app/

EXPOSE ${PORT}

ENTRYPOINT ["/app/ironstream"]