FROM rust:1.86-bookworm AS chef
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

FROM debian:bookworm-slim AS runtime
WORKDIR /app
COPY --from=builder /app/target/release/pirouette /usr/local/bin
ENTRYPOINT ["/usr/local/bin/pirouette"]
