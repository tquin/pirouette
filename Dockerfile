ARG CARGO_VERSION=1.88
ARG DEBIAN_VERSION=bookworm
ARG ALPINE_VERSION=3.22

FROM rust:${CARGO_VERSION}-bookworm AS chef
WORKDIR /app
RUN rustup target add x86_64-unknown-linux-musl
RUN cargo install cargo-chef 

#####

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

#####

# Build dependencies
FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# Build application
COPY . .
RUN cargo build \
    --release \
    --target x86_64-unknown-linux-musl \
    --bin pirouette

#####

# FROM debian:${DEBIAN_VERSION}-slim AS runtime
FROM alpine:${ALPINE_VERSION} AS runtime
WORKDIR /app
COPY --from=builder \
    /app/target/x86_64-unknown-linux-musl/release/pirouette \
    /usr/local/bin/
ENTRYPOINT ["/usr/local/bin/pirouette"]
