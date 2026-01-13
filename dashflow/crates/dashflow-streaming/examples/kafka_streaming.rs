// Kafka Streaming Example
// Author: Andrew Yates (ayates@dropbox.com) © 2025 Dropbox

//! Example demonstrating DashStream Kafka producer and consumer
//!
//! # Prerequisites
//!
//! Start Kafka:
//! ```bash
//! docker-compose -f docker-compose-kafka.yml up -d
//! ```
//!
//! # Run Producer
//! ```bash
//! cargo run --example kafka_streaming -- producer
//! ```
//!
//! # Run Consumer (in separate terminal)
//! ```bash
//! cargo run --example kafka_streaming -- consumer
//! ```

use dashflow_streaming::{
    consumer::DashStreamConsumer,
    kafka::{create_topic, dev_config},
    producer::DashStreamProducer,
    AttributeValue, Event, EventType, Header, MessageType, TokenChunk,
};
use std::env;
use std::time::Duration;

const BOOTSTRAP_SERVERS: &str = "localhost:9092";
const TOPIC: &str = "dashstream-demo";

fn create_graph_start_event(thread_id: &str, sequence: u64) -> Event {
    Event {
        header: Some(Header {
            message_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
            timestamp_us: chrono::Utc::now().timestamp_micros(),
            tenant_id: "demo-tenant".to_string(),
            thread_id: thread_id.to_string(),
            sequence,
            r#type: MessageType::Event as i32,
            parent_id: vec![],
            compression: 0,
            schema_version: 1,
        }),
        event_type: EventType::GraphStart as i32,
        node_id: "".to_string(),
        attributes: std::collections::HashMap::from([(
            "graph_name".to_string(),
            AttributeValue {
                value: Some(dashflow_streaming::attribute_value::Value::StringValue(
                    "demo_graph".to_string(),
                )),
            },
        )]),
        duration_us: 0,
        llm_request_id: "".to_string(),
    }
}

fn create_node_start_event(thread_id: &str, sequence: u64, node_id: &str) -> Event {
    Event {
        header: Some(Header {
            message_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
            timestamp_us: chrono::Utc::now().timestamp_micros(),
            tenant_id: "demo-tenant".to_string(),
            thread_id: thread_id.to_string(),
            sequence,
            r#type: MessageType::Event as i32,
            parent_id: vec![],
            compression: 0,
            schema_version: 1,
        }),
        event_type: EventType::NodeStart as i32,
        node_id: node_id.to_string(),
        attributes: Default::default(),
        duration_us: 0,
        llm_request_id: "".to_string(),
    }
}

fn create_node_end_event(thread_id: &str, sequence: u64, node_id: &str, duration_us: i64) -> Event {
    Event {
        header: Some(Header {
            message_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
            timestamp_us: chrono::Utc::now().timestamp_micros(),
            tenant_id: "demo-tenant".to_string(),
            thread_id: thread_id.to_string(),
            sequence,
            r#type: MessageType::Event as i32,
            parent_id: vec![],
            compression: 0,
            schema_version: 1,
        }),
        event_type: EventType::NodeEnd as i32,
        node_id: node_id.to_string(),
        attributes: Default::default(),
        duration_us,
        llm_request_id: "".to_string(),
    }
}

fn create_graph_end_event(thread_id: &str, sequence: u64, duration_us: i64) -> Event {
    Event {
        header: Some(Header {
            message_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
            timestamp_us: chrono::Utc::now().timestamp_micros(),
            tenant_id: "demo-tenant".to_string(),
            thread_id: thread_id.to_string(),
            sequence,
            r#type: MessageType::Event as i32,
            parent_id: vec![],
            compression: 0,
            schema_version: 1,
        }),
        event_type: EventType::GraphEnd as i32,
        node_id: "".to_string(),
        attributes: Default::default(),
        duration_us,
        llm_request_id: "".to_string(),
    }
}

fn create_token_chunk(thread_id: &str, sequence: u64, text: &str) -> TokenChunk {
    TokenChunk {
        header: Some(Header {
            message_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
            timestamp_us: chrono::Utc::now().timestamp_micros(),
            tenant_id: "demo-tenant".to_string(),
            thread_id: thread_id.to_string(),
            sequence,
            r#type: MessageType::TokenChunk as i32,
            parent_id: vec![],
            compression: 0,
            schema_version: 1,
        }),
        request_id: "".to_string(),
        text: text.to_string(),
        token_ids: vec![],
        logprobs: vec![],
        chunk_index: 0,
        is_final: false,
        finish_reason: 0,
        model: "".to_string(),
        stats: None,
    }
}

async fn run_producer() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Starting DashStream Producer");
    println!("================================");

    // Create topic if it doesn't exist
    println!("Creating topic '{}'...", TOPIC);
    create_topic(BOOTSTRAP_SERVERS, TOPIC, dev_config()).await?;
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Create producer
    println!("Connecting to Kafka at {}...", BOOTSTRAP_SERVERS);
    let producer = DashStreamProducer::new(BOOTSTRAP_SERVERS, TOPIC).await?;
    println!("✅ Producer connected");

    // Simulate a graph execution
    let thread_id = "demo-session-123";
    let mut sequence = 0;

    println!("\n📊 Simulating graph execution...");

    // Graph start
    println!("  → Graph start");
    producer
        .send_event(create_graph_start_event(thread_id, sequence))
        .await?;
    sequence += 1;
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Node 1: Input
    println!("  → Node start: input");
    producer
        .send_event(create_node_start_event(thread_id, sequence, "input"))
        .await?;
    sequence += 1;
    tokio::time::sleep(Duration::from_millis(200)).await;

    producer
        .send_event(create_node_end_event(thread_id, sequence, "input", 200_000))
        .await?;
    sequence += 1;

    // Node 2: LLM
    println!("  → Node start: llm");
    producer
        .send_event(create_node_start_event(thread_id, sequence, "llm"))
        .await?;
    sequence += 1;

    // Stream tokens
    let tokens = [
        "Hello",
        " world",
        "!",
        " This",
        " is",
        " a",
        " streaming",
        " response",
        ".",
    ];
    for token in tokens {
        producer
            .send_token_chunk(create_token_chunk(thread_id, sequence, token))
            .await?;
        sequence += 1;
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    producer
        .send_event(create_node_end_event(thread_id, sequence, "llm", 1_500_000))
        .await?;
    sequence += 1;

    // Node 3: Output
    println!("  → Node start: output");
    producer
        .send_event(create_node_start_event(thread_id, sequence, "output"))
        .await?;
    sequence += 1;
    tokio::time::sleep(Duration::from_millis(100)).await;

    producer
        .send_event(create_node_end_event(
            thread_id, sequence, "output", 100_000,
        ))
        .await?;
    sequence += 1;

    // Graph end
    println!("  → Graph end");
    producer
        .send_event(create_graph_end_event(thread_id, sequence, 2_000_000))
        .await?;

    // Flush
    println!("\n🔄 Flushing messages...");
    producer.flush(Duration::from_secs(5)).await?;
    println!("✅ All messages sent successfully");

    Ok(())
}

async fn run_consumer() -> Result<(), Box<dyn std::error::Error>> {
    println!("🎧 Starting DashStream Consumer");
    println!("================================");

    // Create consumer
    println!("Connecting to Kafka at {}...", BOOTSTRAP_SERVERS);
    let mut consumer =
        DashStreamConsumer::new(BOOTSTRAP_SERVERS, TOPIC, "demo-consumer-group").await?;
    println!("✅ Consumer connected");
    println!("📡 Listening for messages on topic '{}'...\n", TOPIC);

    // Consume messages
    let mut count = 0;
    loop {
        match consumer.next_timeout(Duration::from_secs(30)).await {
            Some(Ok(msg)) => {
                count += 1;
                match msg.message {
                    Some(dashflow_streaming::dash_stream_message::Message::Event(event)) => {
                        let event_type =
                            EventType::try_from(event.event_type).unwrap_or(EventType::GraphStart);
                        let header = event.header.unwrap_or_default();
                        println!(
                            "[{}] Event: {:?} | node: {} | thread: {} | seq: {}",
                            count, event_type, event.node_id, header.thread_id, header.sequence
                        );
                    }
                    Some(dashflow_streaming::dash_stream_message::Message::TokenChunk(chunk)) => {
                        print!("{}", chunk.text);
                        std::io::Write::flush(&mut std::io::stdout())?;
                    }
                    Some(dashflow_streaming::dash_stream_message::Message::StateDiff(diff)) => {
                        let header = diff.header.unwrap_or_default();
                        println!(
                            "[{}] StateDiff | ops: {} | thread: {}",
                            count,
                            diff.operations.len(),
                            header.thread_id
                        );
                    }
                    _ => {
                        println!("[{}] Other message type", count);
                    }
                }
            }
            Some(Err(e)) => {
                eprintln!("❌ Error decoding message: {}", e);
            }
            None => {
                println!("\n⏱️  Timeout - no messages received in 30s");
                break;
            }
        }
    }

    println!("\n✅ Consumer finished ({} messages)", count);
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: {} [producer|consumer]", args[0]);
        eprintln!();
        eprintln!("Start Kafka first:");
        eprintln!("  docker-compose -f docker-compose-kafka.yml up -d");
        eprintln!();
        eprintln!("Then run producer:");
        eprintln!("  cargo run --example kafka_streaming -- producer");
        eprintln!();
        eprintln!("In another terminal, run consumer:");
        eprintln!("  cargo run --example kafka_streaming -- consumer");
        std::process::exit(1);
    }

    match args[1].as_str() {
        "producer" => run_producer().await?,
        "consumer" => run_consumer().await?,
        _ => {
            eprintln!("Unknown command: {}", args[1]);
            eprintln!("Use 'producer' or 'consumer'");
            std::process::exit(1);
        }
    }

    Ok(())
}
