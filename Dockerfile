# --- Build Stage ---
FROM rust:1.85-alpine AS builder

RUN apk add --no-cache musl-dev pkgconfig openssl-dev

WORKDIR /app

# Copy dependency manifests
COPY Cargo.toml Cargo.lock ./

# Create dummy source to build and cache dependencies
RUN mkdir -p src && echo "pub fn dummy() {}" > src/lib.rs && echo "fn main() {}" > src/main.rs
RUN cargo build --release
RUN rm -rf src

# Copy real source code
COPY src ./src
COPY config ./config

# Build final binary
RUN touch src/main.rs src/lib.rs && cargo build --release

# --- Runtime Stage ---
FROM alpine:3.21

RUN apk add --no-cache ca-certificates tzdata

WORKDIR /app

COPY --from=builder /app/target/release/rust-cache-server /app/rust-cache-gateway
COPY --from=builder /app/config /app/config

EXPOSE 8080

ENV RUST_LOG=info

ENTRYPOINT ["/app/rust-cache-gateway"]
