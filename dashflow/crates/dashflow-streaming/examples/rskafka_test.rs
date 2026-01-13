//! Test rskafka library on macOS
//!
//! This example tests whether rskafka (async/await native, pure Rust)
//! can successfully consume from Kafka on macOS without NotImplemented errors.
//!
//! Usage:
//! ```bash
//! # Ensure Kafka is running
//! docker-compose -f docker-compose-kafka.yml up -d
//!
//! # Run test
//! cargo run --example rskafka_test --package dashflow-streaming
//! ```

use rskafka::{client::partition::UnknownTopicHandling, client::ClientBuilder};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧪 Testing rskafka library on macOS");
    println!("   Library: rskafka = 0.5 (async/await native, maintained by InfluxData)");
    println!();

    // Configure consumer
    let brokers = vec!["localhost:9092".to_string()];
    let topic = "dashstream-events";

    println!("📋 Configuration:");
    println!("   Brokers: {:?}", brokers);
    println!("   Topic: {}", topic);
    println!();

    // Create client
    println!("🔄 Creating rskafka client...");
    let client = ClientBuilder::new(brokers).build().await?;

    println!("✅ rskafka client created successfully!");

    // Get partition client
    println!("🔄 Getting topic partition...");
    let partition_client = client
        .partition_client(topic, 0, UnknownTopicHandling::Error)
        .await?;

    println!("✅ Partition client created!");
    println!();
    println!("🎧 Fetching messages from beginning...");
    println!();

    let mut message_count = 0;
    let mut start_offset = 0;

    // Try to fetch messages in batches
    for _ in 0..10 {
        let (records, _high_water_mark) = partition_client
            .fetch_records(
                start_offset,
                1..100_000, // min/max bytes
                1000,       // max wait time in ms
            )
            .await?;

        if records.is_empty() {
            break;
        }

        for record in records {
            message_count += 1;
            println!(
                "📦 Message {}: {} bytes from offset {}",
                message_count,
                record.record.value.as_ref().map_or(0, |v| v.len()),
                start_offset
            );

            start_offset += 1;

            if message_count >= 5 {
                println!();
                println!("✅ Successfully fetched 5 messages, stopping");
                break;
            }
        }

        if message_count >= 5 {
            break;
        }
    }

    println!();
    if message_count == 0 {
        println!("⏱️  No messages in topic");
        println!("   This is OK - topic might be empty");
    }
    println!("📊 Summary:");
    println!("   Status: SUCCESS");
    println!("   Messages fetched: {}", message_count);
    println!("   No NotImplemented errors!");
    println!();
    println!("✅ rskafka WORKS on macOS!");
    Ok(())
}
