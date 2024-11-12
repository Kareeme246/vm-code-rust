//! # Rust Database Consumer
//!
//! This module receives image data and inferred labels from Kafka and stores them in an SQLite database.
//! It supports both initial data insertion and updates with inference labels.
mod protos;
use protobuf::well_known_types::timestamp::Timestamp;
use protobuf::Message;
use protos::image::Image;
use rdkafka::consumer::{BaseConsumer, Consumer};
use rdkafka::message::Message as KafkaMessage;
use rdkafka::ClientConfig;
use rusqlite::{params, Connection, Result};
use std::time::Duration;

/// Creates a Kafka consumer for subscribing to a specific topic.
///
/// # Arguments
///
/// * `bootstrap_server` - Kafka bootstrap server address.
/// * `group_id` - Consumer group ID for Kafka.
/// * `topic` - Kafka topic to subscribe to.
///
/// # Returns
///
/// A configured `BaseConsumer` subscribed to the specified topic.
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

/// Decodes a Protobuf-encoded image message received from Kafka.
///
/// # Arguments
///
/// * `encoded_data` - Byte slice containing Protobuf-encoded image data.
///
/// # Returns
///
/// A `Result` containing the decoded `Image` structure.
fn decode_image_data(encoded_data: &[u8]) -> protobuf::Result<Image> {
    let image_proto = Image::parse_from_bytes(encoded_data)?;
    Ok(image_proto)
}

/// Connects to an SQLite database and creates an `images` table if it doesn't exist.
///
/// # Returns
///
/// A `Result` containing the database connection on success, or an error on failure.
fn connect_to_database(db_path: &str) -> Result<Connection> {
    let conn = Connection::open(db_path)?;
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

/// Inserts initial image data (without inference label) into the database.
///
/// # Arguments
///
/// * `conn` - Database connection.
/// * `timestamp` - Timestamp of the image.
/// * `orig_label` - Original label of the image.
/// * `image_data` - Byte vector of the image data.
///
/// # Returns
///
/// A `Result` indicating success or failure.
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

/// Inserts or updates an inferred label in the database for an existing image entry.
///
/// # Arguments
///
/// * `conn` - Database connection.
/// * `timestamp` - Timestamp of the image.
/// * `inferred_label` - Inferred label of the image.
///
/// # Returns
///
/// A `Result` indicating success or failure.
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
    // Get Kafka bootstrap server from command-line argument, environment variable, or default
    let bootstrap_server = &std::env::args()
        .nth(1)
        .or_else(|| std::env::var("KAFKA_BOOTSTRAP_SERVER").ok())
        .unwrap_or_else(|| "localhost:9092".to_string());
    let image_initial_consumer = create_consumer(bootstrap_server, "db_group", "image_initial");
    let image_inference_consumer = create_consumer(bootstrap_server, "db_group", "image_inference");

    let db_path = "rust_images.db";
    match connect_to_database(db_path) {
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
                                        &decoded_image.original_label[0],
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
                                    // Insert the inferred label into the database
                                    match insert_inferred_label(
                                        &conn,
                                        &decoded_image.timestamp,
                                        &decoded_image.inferred_label[0],
                                    ) {
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

#[cfg(test)]

mod tests {
    use super::*;
    use protobuf::well_known_types::timestamp::Timestamp;
    use protos::image::Image;
    use rusqlite::Connection;
    use std::fs;

    fn setup_database() -> Connection {
        let conn = Connection::open_in_memory().expect("Failed to open database");
        conn.execute(
            "CREATE TABLE images (
                timestamp REAL PRIMARY KEY,
                orig_label INTEGER,
                inferred_label INTEGER,
                image_data BLOB
            )",
            [],
        )
        .expect("Failed to create table");
        conn
    }

    #[test]
    fn test_insert_initial_image_data() {
        let conn = setup_database();
        let timestamp = Timestamp {
            seconds: 1_000_000,
            nanos: 0,
            ..Default::default()
        };
        let orig_label = 5;
        let image_data = vec![1, 2, 3, 4, 5];

        let result = insert_initial_image_data(&conn, &timestamp, &orig_label, image_data.clone());
        assert!(result.is_ok());

        let mut stmt = conn
            .prepare("SELECT * FROM images WHERE timestamp = ?1")
            .unwrap();
        let mut rows = stmt.query(params![1_000_000.0]).unwrap();
        if let Some(row) = rows.next().unwrap() {
            let retrieved_label: u8 = row.get(1).unwrap();
            let retrieved_data: Vec<u8> = row.get(3).unwrap();
            assert_eq!(retrieved_label, orig_label);
            assert_eq!(retrieved_data, image_data);
        } else {
            panic!("No data found");
        }
    }

    #[test]
    fn test_insert_inferred_label() {
        let conn = setup_database();
        let timestamp = Timestamp {
            seconds: 1_000_000,
            nanos: 0,
            ..Default::default()
        };
        let orig_label = 5;
        let inferred_label = 9;
        let image_data = vec![1, 2, 3, 4, 5];

        // First, insert initial image data
        insert_initial_image_data(&conn, &timestamp, &orig_label, image_data).unwrap();

        // Then, insert inferred label
        let result = insert_inferred_label(&conn, &timestamp, &inferred_label);
        assert!(result.is_ok());

        let mut stmt = conn
            .prepare("SELECT * FROM images WHERE timestamp = ?1")
            .unwrap();
        let mut rows = stmt.query(params![1_000_000.0]).unwrap();
        if let Some(row) = rows.next().unwrap() {
            let retrieved_inferred_label: u8 = row.get(2).unwrap();
            assert_eq!(retrieved_inferred_label, inferred_label);
        } else {
            panic!("No data found");
        }
    }

    #[test]
    fn test_decode_image_data() {
        let mut image = Image::new();
        image.timestamp = protobuf::MessageField::some(Timestamp {
            seconds: 1_000_000,
            nanos: 0,
            ..Default::default()
        });
        image.original_label = vec![5];
        image.image_data = vec![1, 2, 3, 4, 5];

        let encoded_data = image.write_to_bytes().unwrap();
        let decoded_image = decode_image_data(&encoded_data).unwrap();

        assert_eq!(decoded_image.timestamp.seconds, 1_000_000);
        assert_eq!(decoded_image.original_label[0], 5);
        assert_eq!(decoded_image.image_data, &[1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_connect_to_database() {
        let db_path = "test_rust_images.db";
        let conn = connect_to_database(db_path).expect("Failed to connect to database");

        // Set up the table if it doesn't already exist
        conn.execute(
            "CREATE TABLE IF NOT EXISTS images (id INTEGER PRIMARY KEY, data BLOB NOT NULL)",
            [],
        )
        .expect("Failed to create images table");

        // Check if we can select from the images table
        let result = conn.execute("SELECT 1 FROM images LIMIT 1", []);
        assert!(result.is_ok(), "Query failed: {:?}", result.err());

        // Clean up the test database file
        fs::remove_file(db_path)
            .unwrap_or_else(|_| panic!("Failed to delete test database file: {}", db_path));
    }
}
