ARG CARGO_VERSION=1.88
ARG DEBIAN_VERSION=bookworm

FROM rust:${CARGO_VERSION}-bookworm AS chef
WORKDIR /app
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
    --bin pirouette

#####

FROM debian:${DEBIAN_VERSION}-slim AS runtime
WORKDIR /app
COPY --from=builder \
    /app/target/release/pirouette \
    /usr/local/bin/
ENTRYPOINT ["/usr/local/bin/pirouette"]
