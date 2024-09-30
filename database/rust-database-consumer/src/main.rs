mod protos;
use protobuf::well_known_types::timestamp::Timestamp;
use protobuf::Message;
use protos::image::Image;
use rdkafka::consumer::{BaseConsumer, Consumer};
use rdkafka::message::Message as KafkaMessage;
use rdkafka::ClientConfig;
use rusqlite::{params, Connection, Result};
use std::env::args;
use std::thread::sleep;
use std::time::Duration;

fn create_consumer(bootstrap_server: &str, group_id: &str, topic: &str) -> BaseConsumer {
    let consumer: BaseConsumer = ClientConfig::new()
        .set("group.id", group_id)
        .set("bootstrap.servers", bootstrap_server)
        .create()
        .expect("Failed to create consumer");

    consumer
        .subscribe(&[topic])
        .expect("Failed to subscribe to topic");
    consumer
}

fn decode_image_data(encoded_data: &[u8]) -> protobuf::Result<Image> {
    let image_proto = Image::parse_from_bytes(encoded_data)?;
    Ok(image_proto)
}

fn connect_to_database() -> Result<Connection> {
    let conn = Connection::open("rust_images.db")?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS images (
            timestamp REAL PRIMARY KEY,
            orig_label INTEGER,
            inferred_label INTEGER,
            image_data BLOB
        )",
        [],
    )?;
    Ok(conn)
}

fn insert_initial_image_data(
    conn: &Connection,
    timestamp: &Timestamp,
    orig_label: &u8,
    image_data: Vec<u8>,
) -> Result<()> {
    let timestamp_real: f64 = timestamp.seconds as f64 + (timestamp.nanos as f64) * 1e-9;
    conn.execute(
        "INSERT INTO images (timestamp, orig_label, inferred_label, image_data)
         VALUES (?1, ?2, NULL, ?3)
         ON CONFLICT(timestamp) DO UPDATE SET
         orig_label = excluded.orig_label, image_data = excluded.image_data",
        params![timestamp_real, orig_label, image_data],
    )?;
    Ok(())
}

fn insert_inferred_label(
    conn: &Connection,
    timestamp: &Timestamp,
    inferred_label: &u8,
) -> Result<()> {
    let timestamp_real: f64 = timestamp.seconds as f64 + (timestamp.nanos as f64) * 1e-9;
    conn.execute(
        "INSERT INTO images (timestamp, orig_label, inferred_label, image_data)
         VALUES (?1, NULL, ?2, NULL)
         ON CONFLICT(timestamp) DO UPDATE SET
         inferred_label = excluded.inferred_label",
        params![timestamp_real, inferred_label],
    )?;
    Ok(())
}

fn main() {
    // Create Kafka consumers for both topics
    let bootstrap_server = &args().nth(1).unwrap_or("localhost:9092".to_string());
    let image_initial_consumer = create_consumer(bootstrap_server, "group1", "image_initial");
    let image_inference_consumer = create_consumer(bootstrap_server, "group2", "image_inference");

    // Connect to local SQLite database or create if doesn't exist
    match connect_to_database() {
        Ok(conn) => {
            // Main loop: poll both consumers and insert data into the database on payload received
            loop {
                // Poll the first consumer (image_initial)
                match image_initial_consumer.poll(Duration::from_millis(200)) {
                    Some(Ok(message)) => {
                        if let Some(payload) = message.payload() {
                            match decode_image_data(payload) {
                                Ok(decoded_image) => {
                                    // Insert the decoded image data into the database
                                    match insert_initial_image_data(
                                        &conn,
                                        &decoded_image.timestamp,
                                        &decoded_image.label[0],
                                        decoded_image.image_data,
                                    ) {
                                        Ok(_) => println!("Initial image data inserted."),
                                        Err(e) => {
                                            eprintln!("Failed to insert initial data: {:?}", e)
                                        }
                                    }
                                }
                                Err(e) => eprintln!("Failed to decode image data: {:?}", e),
                            }
                        }
                    }
                    Some(Err(e)) => eprintln!("Kafka error (image_initial): {:?}", e),
                    None => {}
                }

                // Poll the second consumer (image_inference)
                match image_inference_consumer.poll(Duration::from_millis(500)) {
                    Some(Ok(message)) => {
                        if let Some(payload) = message.payload() {
                            match decode_image_data(payload) {
                                Ok(decoded_image) => {
                                    let timestamp = &decoded_image.timestamp;
                                    let inferred_label = &decoded_image.label[0];

                                    // Insert the inferred label into the database
                                    match insert_inferred_label(&conn, timestamp, inferred_label) {
                                        Ok(_) => println!("Inferred label data inserted."),
                                        Err(e) => {
                                            eprintln!("Failed to insert inferred data: {:?}", e)
                                        }
                                    }
                                }
                                Err(e) => {
                                    eprintln!("Failed to decode inferred image data: {:?}", e)
                                }
                            }
                        }
                    }
                    Some(Err(e)) => eprintln!("Kafka error (image_inference): {:?}", e),
                    None => {}
                }

                // Pause briefly before the next poll cycle
                // sleep(Duration::from_secs(1));
            }
        }
        Err(e) => println!("Failed to connect to database: {:?}", e),
    }
}
