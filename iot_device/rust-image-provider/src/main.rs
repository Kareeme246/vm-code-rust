//! # Rust Image Provider
//!
//! This module is responsible for loading, encoding, and sending CIFAR-100 images via Kafka.
//! It reads raw image data, creates structured Protobuf messages, and transmits the data
//! to a Kafka topic for downstream consumers.
mod protos;
use protobuf::well_known_types::timestamp::Timestamp;
use protobuf::Message;
use protos::image::Image;
use rdkafka::producer::{BaseProducer, BaseRecord, Producer};
use rdkafka::ClientConfig;
use std::fs::File;
use std::io::{self, Read};
use std::thread::sleep;

// Constants for CIFAR-100
// const IMAGE_SIZE: usize = 3072; // 32x32 image with 3 channels (RGB)
const RECORD_SIZE: usize = 3074; // 1 byte for coarse label + 1 byte for fine label + 3072 bytes for image
const NUM_IMAGES: usize = 50_000; // 50,000 images in binary training file

/// Loads the entire CIFAR-100 dataset from a binary file and returns the contents as a `Vec<u8>`.
///
/// # Arguments
///
/// * `file_path` - Path to the CIFAR-100 binary file.
///
/// # Returns
///
/// A `Result` containing the file contents as a byte vector on success, or an error on failure.
fn load_cifar_datafile(file_path: &str) -> io::Result<Vec<u8>> {
    let mut file = File::open(file_path)?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;
    Ok(buffer)
}

/// Selects a random image and label from the CIFAR-100 data pool.
///
/// # Arguments
///
/// * `data` - CIFAR-100 dataset loaded as a byte array.
/// * `available_indices` - List of indices representing unprocessed images.
///
/// # Returns
///
/// A tuple containing the index of the selected image, its label, and the image data itself.
fn get_random_image(data: &[u8], available_indices: &[usize]) -> (usize, u8, Vec<u8>) {
    // Randomly select and remove an index from the available pool
    let removal_index = fastrand::usize(0..available_indices.len());
    let random_index = available_indices[removal_index];

    // Calculate the starting position of the record
    let start: usize = random_index * RECORD_SIZE;

    let _coarse_label: u8 = data[start]; // Coarse label (0-19)
    let fine_label: u8 = data[start + 1]; // Fine label (0-99)

    let image_data = data[start + 2..start + RECORD_SIZE].to_vec(); // The next 3072 bytes are the image

    (removal_index, fine_label, image_data)
}

/// Creates a Protobuf-encoded image message with metadata and image content.
///
/// # Arguments
///
/// * `label` - The label of the image.
/// * `image_data` - Byte vector of image data.
///
/// # Returns
///
/// A `Result` containing the encoded message as a byte vector on success, or an error on failure.
fn create_image_data(label: u8, image_data: Vec<u8>) -> protobuf::Result<Vec<u8>> {
    // Create a new instance of the `Image` struct
    let mut image_proto = Image::new();

    image_proto.timestamp = protobuf::MessageField::some(Timestamp::now());
    image_proto.original_label = vec![label];
    image_proto.image_data = image_data;

    // Serialize the data to binary format (protobuf wire format)
    let encoded_data = image_proto.write_to_bytes()?;

    Ok(encoded_data)
}

/// Decodes a Protobuf-encoded image message back into an `Image` structure.
///
/// # Arguments
///
/// * `encoded_data` - Byte slice containing Protobuf-encoded image data.
///
/// # Returns
///
/// A `Result` with the decoded `Image` struct on success, or an error on failure.
#[allow(dead_code)]
fn decode_image_data(encoded_data: &[u8]) -> protobuf::Result<Image> {
    // Parse the Protobuf-encoded data into an Image struct
    let image_proto = Image::parse_from_bytes(encoded_data)?;

    Ok(image_proto)
}

// https://www.arroyo.dev/blog/using-kafka-with-rust#what-is-kafka
/// Creates a Kafka producer with the specified bootstrap server.
///
/// # Arguments
///
/// * `bootstrap_server` - Kafka bootstrap server address.
///
/// # Returns
///
/// A configured `BaseProducer` ready to send messages.
fn create_producer(bootstrap_server: &str) -> BaseProducer {
    let producer: BaseProducer = ClientConfig::new()
        .set("bootstrap.servers", bootstrap_server)
        .set("queue.buffering.max.ms", "0")
        .create()
        .expect("Failed to create client");
    producer
}

fn main() -> io::Result<()> {
    // Load the CIFAR-100 binary file
    let file_path = "data/cifar-100-binary/train.bin";
    let data = load_cifar_datafile(file_path)?;

    // Get Kafka bootstrap server from command-line argument, environment variable, or default
    let bootstrap_server = &std::env::args()
        .nth(1)
        .or_else(|| std::env::var("KAFKA_BOOTSTRAP_SERVER").ok())
        .unwrap_or_else(|| "localhost:9092".to_string());
    let producer = create_producer(bootstrap_server);

    fastrand::seed(1);
    let mut available_indices: Vec<usize> = (0..NUM_IMAGES).collect(); // Initialize with all indices

    // Main loop: select random images, process them
    for _ in 0..100 {
        if available_indices.is_empty() {
            // Chosen 10,000 elements
            available_indices = (0..NUM_IMAGES).collect(); // Initialize with all indices again for
                                                           // next file
        }
        // Grab random image (data)
        let (idx, label, image_data) = get_random_image(&data, &available_indices);
        available_indices.swap_remove(idx);

        // Encode image via profobuf
        match create_image_data(label, image_data) {
            Ok(encoded_data) => {
                // Send to kafka broker // Send under topic "quickstart-events"
                // Create a record with the topic and payload
                // let record = BaseRecord::<(), _>::to("quickstart-events").payload(&encoded_data);
                let record = BaseRecord::<(), _>::to("image_initial").payload(&encoded_data);

                // Send the record
                match producer.send(record) {
                    Ok(_) => println!("Message sent successfully."),
                    Err(e) => eprintln!("Failed to send message: {:?}", e),
                }

                // Ensure all messages are sent before exiting
                let _ = producer.flush(std::time::Duration::from_secs(1));
                sleep(std::time::Duration::from_millis(500));

                // Access the decoded fields for logging
                // match decode_image_data(&encoded_data) {
                //     Ok(decoded_image) => {
                //         println!("Decoded timestamp: {:?}", decoded_image.timestamp);
                //         println!("Decoded label: {:?}", decoded_image.original_label);
                //         println!(
                //             "Decoded image data (first 10 bytes): {:?}",
                //             &decoded_image.image_data[..10]
                //         );
                //         println!();
                //     }
                //     Err(e) => {
                //         eprintln!("Failed to decode image data: {:?}", e);
                //     }
                // }
            }
            Err(e) => {
                eprintln!("Failed to encode image data: {:?}", e);
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_loading_data() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("cifar-100-binary");

        let mut file = File::create(&file_path).unwrap();

        // Write mock CIFAR-100 data
        let fake_data: Vec<u8> = vec![0; RECORD_SIZE * 10];
        io::Write::write_all(&mut file, &fake_data).unwrap();

        // Load data
        let result = load_cifar_datafile(file_path.to_str().unwrap());
        assert!(result.is_ok());
        let data = result.unwrap();
        assert_eq!(data.len(), RECORD_SIZE * 10);
    }

    #[test]
    fn test_get_random_image() {
        // Create mock data for CIFAR-100
        let data = vec![0u8; RECORD_SIZE * 10];
        let available_indices: Vec<usize> = (0..10).collect();

        let (removal_index, label, image_data) = get_random_image(&data, &available_indices);

        // Check if the returned index is within the range of available indices
        assert!(removal_index < available_indices.len());

        assert_eq!(label, 0);

        assert_eq!(image_data.len(), 3072);
    }

    #[test]
    fn test_create_image_data() {
        let image_data = vec![1u8; 3072];
        let label: u8 = 5;

        let result = create_image_data(label, image_data.clone());
        assert!(result.is_ok());
        let encoded_data = result.unwrap();

        let decoded_result = decode_image_data(&encoded_data);
        assert!(decoded_result.is_ok());
        let decoded_image = decoded_result.unwrap();

        // Validate decoded fields
        assert_eq!(decoded_image.original_label, vec![label]);
        assert_eq!(decoded_image.image_data, image_data);
    }

    #[test]
    fn test_kafka_producer() {
        const TOPIC: &str = "test_topic";
        let mock_cluster = rdkafka::mocking::MockCluster::new(1).unwrap();
        mock_cluster
            .create_topic(TOPIC, 32, 2)
            .expect("Failed to create topic");

        let producer: BaseProducer = create_producer(&mock_cluster.bootstrap_servers());

        let image_data = vec![1u8; 3072];
        let record = BaseRecord::<(), _>::to("test_topic").payload(&image_data);

        let send_result = producer.send(record);

        assert!(send_result.is_ok());
    }
}
