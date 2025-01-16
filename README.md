# VM-CODE-RUST

## Rust Image Processing and Inference Pipeline

![Screenshot 2025-01-15 at 10 25 53 PM](https://github.com/user-attachments/assets/d24e5af2-4c57-4138-b02a-7918c59f6068)

### Overview

This project provides a complete pipeline for loading, processing, and transmitting image data from the CIFAR-100 dataset through Kafka to perform inference and store results in a database. The pipeline consists of three main Rust modules:

- **Image Provider**: Loads and encodes CIFAR-100 images, sending them via Kafka for downstream processing.
- **Inference Consumer**: Receives image data, applies a pre-trained machine learning model for inference, and sends inferred results.
- **Database Consumer**: Receives initial and inferred data, storing it in a database for future analysis.

This setup utilizes Protobuf for structured data serialization, Kafka for message streaming, and SQLite for persistent storage.

### Features

- **End-to-End Pipeline**: From loading images to storing inference results.
- **Efficient Serialization**: Protobuf is used for lightweight and structured message formatting.
- **Scalable Messaging**: Kafka provides high-throughput data streaming across modules.
- **Database Integration**: Results are saved in SQLite for later retrieval and analysis.

### Structure
This project is structured as a rust workspace with three modules. The entire setup is run through docker-compose.

---

## Documentation

To generate documentation for each module, use the following commands:

```bash
cargo doc -p rust-image-provider --open
cargo doc -p rust-inference-consumer --open
cargo doc -p rust-database-consumer --open
```

## Installation/How to run:

- Prerequisites: Docker

1. Clone the repository:

```bash
git clone https://github.com/Kareeme246/vm-code-rust.git
```

2. Download and extract the data file used by the image provider

Run the following command in the root directory of the project
```bash
mkdir -p iot_device/rust-image-provider/data && curl -sL https://www.cs.toronto.edu/~kriz/cifar-100-binary.tar.gz | tar -xz -C iot_device/rust-image-provider/data
```
3. Build & Start all services:

```bash
docker compose up
```
