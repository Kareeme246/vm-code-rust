FROM rust:1.82 as builder

WORKDIR /rust-image-provider

# Copy manifests
COPY ./Cargo.lock ./Cargo.lock
COPY ./iot_device/rust-image-provider/Cargo.toml ./Cargo.toml

# Copy source code
COPY iot_device/rust-image-provider/src ./src
COPY iot_device/rust-image-provider/build.rs ./build.rs


RUN cargo build --release

# The final base image (Simply to reduce image size)
FROM debian:bookworm-slim

# Copy into new image from the builder
COPY --from=builder /rust-image-provider/target/release/rust-image-provider /usr/src/rust-image-provider

# Copy Cifar100 data
COPY iot_device/rust-image-provider/data /usr/src/data

# Setup an app user so the container doesn't run as the root user
RUN useradd app
USER app

# Set the working directory to /usr/src for easy data access
WORKDIR /usr/src

# Run the binary
CMD ["/usr/src/rust-image-provider", "kafka_server:9092"]
