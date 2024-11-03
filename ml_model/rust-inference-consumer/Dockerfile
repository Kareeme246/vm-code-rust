# Use an x86_64 Rust image with QEMU for emulation on ARM
FROM --platform=linux/amd64 rust:1.82 as builder

# Rest of the Dockerfile setup
WORKDIR /rust-inference-consumer

RUN apt-get update && apt-get install -y wget unzip

# Download the x86-64 version of libtorch for compatibility
RUN wget https://download.pytorch.org/libtorch/cpu/libtorch-cxx11-abi-shared-with-deps-2.4.0%2Bcpu.zip -O libtorch.zip && \
    unzip libtorch.zip -d /usr/local && rm libtorch.zip

ENV LIBTORCH=/usr/local/libtorch
ENV LD_LIBRARY_PATH=${LIBTORCH}/lib:$LD_LIBRARY_PATH

# Copy manifests and source
COPY ./Cargo.lock ./Cargo.lock
COPY ./ml_model/rust-inference-consumer/Cargo.toml ./Cargo.toml
COPY ml_model/rust-inference-consumer/src ./src
COPY ml_model/rust-inference-consumer/build.rs ./build.rs

RUN cargo build --release

# Final stage
FROM --platform=linux/amd64 debian:bookworm-slim

RUN apt-get update && apt-get install -y libgomp1 && rm -rf /var/lib/apt/lists/*

COPY --from=builder /rust-inference-consumer/target/release/rust-inference-consumer /usr/src/rust-inference-consumer
COPY --from=builder /usr/local/libtorch /usr/local/libtorch

COPY ml_model/rust-inference-consumer/cifar100_model.pt /usr/src/cifar100_model.pt

ENV LD_LIBRARY_PATH=/usr/local/libtorch/lib:$LD_LIBRARY_PATH

# Setup an app user so the container doesn't run as the root user
RUN useradd app
USER app

# Set the working directory to /usr/src for easy data access
WORKDIR /usr/src

CMD ["/usr/src/rust-inference-consumer", "kafka_server:9092"]

