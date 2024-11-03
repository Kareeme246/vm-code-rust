FROM rust:1.82 as builder

WORKDIR /rust-database-consumer

# Copy manifests
COPY ./Cargo.lock ./Cargo.lock
COPY ./database/rust-database-consumer/Cargo.toml ./Cargo.toml

# Copy source code
COPY database/rust-database-consumer/src ./src
COPY database/rust-database-consumer/build.rs ./build.rs

RUN cargo build --release

# The final base image (Simply to reduce image size)
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y libsqlite3-0 && rm -rf /var/lib/apt/lists/*

# Copy into new image from the previous build
COPY --from=builder /rust-database-consumer/target/release/rust-database-consumer /usr/src/rust-database-consumer

# Run the binary
CMD ["/usr/src/rust-database-consumer", "kafka_server:9092"]
