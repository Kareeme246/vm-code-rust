mod protos;
use protobuf::Message;
use protos::image::Image;
use rdkafka::message::Message as KafkaMessage;
use rdkafka::producer::Producer;
use rdkafka::{
    consumer::{BaseConsumer, Consumer},
    producer::{BaseProducer, BaseRecord},
    ClientConfig,
};
use tch::{CModule, Kind, Tensor};

// Preprocess the image data received into a Tensor
fn preprocess_image(image_data: Vec<u8>) -> Tensor {
    // Convert the image data to a tensor and normalize it
    let img_tensor = Tensor::from_slice(&image_data)
        .to_kind(tch::Kind::Float)
        .divide_scalar_(255.0)
        .view([3, 32, 32]); // CIFAR images are 32x32 with 3 color channels

    // Normalize using CIFAR-100 mean and std
    let mean = Tensor::from_slice(&[0.5071, 0.4867, 0.4408])
        .to_kind(Kind::Float)
        .view([3, 1, 1]);
    let std = Tensor::from_slice(&[0.2675, 0.2565, 0.2761])
        .to_kind(Kind::Float)
        .view([3, 1, 1]);

    let normalized_tensor = (img_tensor - &mean) / &std;

    normalized_tensor.unsqueeze(0) // Shape: [1, 3, 32, 32]
}

// Function to create the Protobuf message with inferred label
fn create_inferred_image(original_image: Image, inferred_label: u8) -> protobuf::Result<Vec<u8>> {
    let mut image_proto = Image::new();

    // Copy the timestamp, original label, and image data from the received image
    image_proto.timestamp = original_image.timestamp.clone();
    image_proto.original_label = original_image.original_label.clone();
    image_proto.image_data = original_image.image_data.clone();

    // Add the inferred label from the model's prediction
    image_proto.inferred_label = vec![inferred_label];

    // Serialize the data to binary format (protobuf wire format)
    image_proto.write_to_bytes()
}

// Decode the Protobuf image data received from Kafka
fn decode_image_data(encoded_data: &[u8]) -> protobuf::Result<Image> {
    Image::parse_from_bytes(encoded_data)
}

// Kafka Consumer setup
fn create_consumer(bootstrap_server: &str, group_id: &str, topic: &str) -> BaseConsumer {
    let consumer: BaseConsumer = ClientConfig::new()
        .set("group.id", group_id)
        .set("bootstrap.servers", bootstrap_server)
        .set("auto.offset.reset", "latest")
        .create()
        .expect("Failed to create consumer");

    consumer
        .subscribe(&[topic])
        .expect("Failed to subscribe to topic");
    consumer
}

// Kafka Producer setup
fn create_producer(bootstrap_server: &str) -> BaseProducer {
    ClientConfig::new()
        .set("bootstrap.servers", bootstrap_server)
        .set("queue.buffering.max.ms", "0")
        .create()
        .expect("Failed to create client")
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load the TorchScript model // Retry on error
    let model = loop {
        match CModule::load("cifar100_model.pt") {
            Ok(m) => {
                println!("Model loaded successfully.");
                break m;
            }
            Err(e) => {
                eprintln!("Failed to load model: {:?}. Retrying in 5 seconds...", e);
                std::thread::sleep(std::time::Duration::from_secs(3));
            }
        }
    };

    // Create Kafka consumer and producer
    let bootstrap_server = &std::env::args()
        .nth(1)
        .unwrap_or("localhost:9092".to_string());
    let image_initial_consumer = create_consumer(bootstrap_server, "ml_group", "image_initial");
    let inference_producer = create_producer(bootstrap_server);

    // Main loop for Kafka consumer
    loop {
        match image_initial_consumer.poll(std::time::Duration::from_millis(200)) {
            Some(Ok(message)) => {
                if let Some(payload) = message.payload() {
                    match decode_image_data(payload) {
                        Ok(decoded_image) => {
                            // Preprocess the image from the received payload
                            let input_tensor = preprocess_image(decoded_image.image_data.clone());

                            // Run inference on the model // Retry on error
                            let output = loop {
                                match model.forward_ts(&[input_tensor.shallow_clone()]) {
                                    Ok(out) => break out,
                                    Err(e) => {
                                        eprintln!("Error during model inference: {:?}. Retrying in 1 second...", e);
                                        std::thread::sleep(std::time::Duration::from_secs(1));
                                    }
                                }
                            };
                            let predicted_class = output.argmax(1, false);
                            let predicted_label = predicted_class.int64_value(&[0]) as u8;

                            // Create a new Protobuf message with inferred label
                            match create_inferred_image(decoded_image, predicted_label) {
                                Ok(encoded_message) => {
                                    // Send the inferred image to the Kafka topic
                                    let record = BaseRecord::to("image_inference")
                                        .payload(&encoded_message)
                                        .key("inference");

                                    match inference_producer.send(record) {
                                        Ok(_) => println!("Inferred message sent successfully."),
                                        Err(e) => {
                                            eprintln!("Failed to send inferred message: {:?}", e)
                                        }
                                    }

                                    // Ensure all messages are sent before exiting
                                    let _ =
                                        inference_producer.flush(std::time::Duration::from_secs(1));
                                    std::thread::sleep(std::time::Duration::from_millis(100));
                                }
                                Err(e) => eprintln!("Failed to create inferred image: {:?}", e),
                            }
                        }
                        Err(e) => eprintln!("Failed to decode image data: {:?}", e),
                    }
                }
            }
            Some(Err(e)) => eprintln!("Kafka error (image_initial): {:?}", e),
            None => {}
        }

        // Pause briefly before the next poll cycle
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
}
