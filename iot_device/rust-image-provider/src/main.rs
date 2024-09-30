mod protos;
use protobuf::well_known_types::timestamp::Timestamp;
use protobuf::Message;
use protos::image::Image;
use rdkafka::producer::{BaseProducer, BaseRecord, Producer};
use rdkafka::ClientConfig;
use std::env::args;
use std::fs::File;
use std::io::{self, Read};
use std::thread::sleep;

// Constants for CIFAR-100
const IMAGE_SIZE: usize = 3072; // 32x32 image with 3 channels (RGB)
const RECORD_SIZE: usize = 3073; // 1 byte for label + 3072 bytes for image
const NUM_IMAGES: usize = 10_000; // 10,000 images per binary file
const TOTAL_IMAGES: usize = 60_000; // 10,000 images per binary file * 5 train and 1 test file

// Function to load the CIFAR-100 binary file
fn load_cifar_datafile(file_path: &str) -> io::Result<Vec<u8>> {
    let mut file = File::open(file_path)?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;
    Ok(buffer)
}

// Function to get a random image and label from the CIFAR-100 data
fn get_random_image(data: &[u8], available_indices: &[usize]) -> (usize, u8, Vec<u8>) {
    // Randomly select and remove an index from the available pool
    let removal_index = fastrand::usize(0..available_indices.len());
    let random_index = available_indices[removal_index];

    // Calculate the starting position of the record
    let start: usize = random_index * RECORD_SIZE;
    let label: u8 = data[start]; // The label is the first byte
    let image_data = data[start + 1..start + RECORD_SIZE].to_vec(); // The next 3072 bytes are the image

    (removal_index, label, image_data)
}

// Assuming the generated struct is available
fn create_image_data(label: u8, image_data: Vec<u8>) -> protobuf::Result<Vec<u8>> {
    // Create a new instance of the `Image` struct
    let mut image_proto = Image::new();

    image_proto.timestamp = protobuf::MessageField::some(Timestamp::now());
    image_proto.label = vec![label];
    image_proto.image_data = image_data;

    // Serialize the data to binary format (protobuf wire format)
    let encoded_data = image_proto.write_to_bytes()?;

    Ok(encoded_data)
}

fn decode_image_data(encoded_data: &[u8]) -> protobuf::Result<Image> {
    // Parse the Protobuf-encoded data into an Image struct
    let image_proto = Image::parse_from_bytes(encoded_data)?;

    Ok(image_proto)
}

// https://www.arroyo.dev/blog/using-kafka-with-rust#what-is-kafka
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
    let file_path = "data/cifar-10-batches-bin/data_batch_1.bin"; // Change this based on the batch you're loading
    let data = load_cifar_datafile(file_path)?;

    // Creates a producer, reading the bootstrap server from the first command-line argument or defaulting to localhost:9092
    let producer = create_producer(&args().nth(1).unwrap_or("localhost:9092".to_string()));

    fastrand::seed(1);
    let mut available_indices: Vec<usize> = (0..NUM_IMAGES).collect(); // Initialize with all indices

    // Main loop: select random images, process them
    for _ in 0..5 {
        if available_indices.is_empty() {
            // Chosen 10,000 elements
            available_indices = (0..NUM_IMAGES).collect(); // Initialize with all indices again for
                                                           // next file
        }
        // Grab random image (data)
        let (idx, label, image_data) = get_random_image(&data, &available_indices);
        available_indices.swap_remove(idx);

        // Image processing (placeholder)
        // Reshape the flat image (prep for blur)

        // Blur Image

        // Flatten image (prep for serialization)

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
                sleep(std::time::Duration::from_millis(100));

                // Access the decoded fields for logging
                match decode_image_data(&encoded_data) {
                    Ok(decoded_image) => {
                        println!("Decoded timestamp: {:?}", decoded_image.timestamp);
                        println!("Decoded label: {:?}", decoded_image.label);
                        println!(
                            "Decoded image data (first 10 bytes): {:?}",
                            &decoded_image.image_data[..10]
                        );
                        println!();
                    }
                    Err(e) => {
                        eprintln!("Failed to decode image data: {:?}", e);
                    }
                }
            }
            Err(e) => {
                eprintln!("Failed to encode image data: {:?}", e);
            }
        }
    }

    Ok(())
}
