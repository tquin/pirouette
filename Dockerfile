ARG CARGO_VERSION=1.88

FROM rust:${CARGO_VERSION}-bookworm AS chef
RUN cargo install cargo-chef 
WORKDIR /app

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
RUN cargo build --release --bin pirouette

#####

FROM gcr.io/distroless/cc-debian12 AS runtime
WORKDIR /app
COPY --from=builder /app/target/release/pirouette /bin/
ENTRYPOINT ["/bin/pirouette"]
