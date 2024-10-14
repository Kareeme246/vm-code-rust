#!/bin/bash

# Navigate to rust-database-consumer
cd database/rust-database-consumer
echo "Running checks for rust-database-consumer..."

cargo fmt -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo build --verbose
cargo test --verbose

# Navigate to rust-image-provider
cd ../../iot_device/rust-image-provider
echo "Running checks for rust-image-provider..."

cargo fmt -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo build --verbose
cargo test --verbose

# Navigate to rust-inference-consumer
cd ../../ml_model/rust-inference-consumer
echo "Running checks for rust-inference-consumer..."

cargo fmt -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo build --verbose
cargo test --verbose
