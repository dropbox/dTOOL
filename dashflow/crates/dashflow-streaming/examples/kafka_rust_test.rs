//! Test kafka-rust library on macOS
//!
//! This example tests whether kafka-rust (pure Rust, no librdkafka)
//! can successfully consume from Kafka on macOS without NotImplemented errors.
//!
//! Usage:
//! ```bash
//! # Ensure Kafka is running
//! docker-compose -f docker-compose-kafka.yml up -d
//!
//! # Run test
//! cargo run --example kafka_rust_test --package dashflow-streaming
//! ```

use kafka::consumer::{Consumer, FetchOffset, GroupOffsetStorage};
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧪 Testing kafka-rust library on macOS");
    println!("   Library: kafka = 0.10 (pure Rust, no librdkafka dependency)");
    println!();

    // Configure consumer
    let hosts = vec!["localhost:9092".to_string()];
    let topic = "dashstream-events";
    let group = "kafka-rust-test";

    println!("📋 Configuration:");
    println!("   Brokers: {:?}", hosts);
    println!("   Topic: {}", topic);
    println!("   Group: {}", group);
    println!();

    // Create consumer
    println!("🔄 Creating kafka-rust consumer...");
    let consumer_result = Consumer::from_hosts(hosts)
        .with_topic(topic.to_owned())
        .with_group(group.to_owned())
        .with_fallback_offset(FetchOffset::Earliest)
        .with_offset_storage(Some(GroupOffsetStorage::Kafka))
        .create();

    match consumer_result {
        Ok(mut consumer) => {
            println!("✅ kafka-rust consumer created successfully!");
            println!();
            println!("🎧 Listening for messages (will timeout after 30 seconds if none)...");
            println!();

            let start = std::time::Instant::now();
            let timeout = Duration::from_secs(30);
            let mut message_count = 0;

            loop {
                if start.elapsed() > timeout {
                    println!();
                    if message_count == 0 {
                        println!("⏱️  No messages received in 30 seconds");
                        println!("   This is OK - topic might be empty");
                    }
                    break;
                }

                match consumer.poll() {
                    Ok(message_sets) => {
                        for ms in message_sets.iter() {
                            for message in ms.messages() {
                                message_count += 1;
                                println!(
                                    "📦 Message {}: {} bytes from partition {} offset {}",
                                    message_count,
                                    message.value.len(),
                                    ms.partition(),
                                    message.offset
                                );

                                if message_count >= 5 {
                                    println!();
                                    println!("✅ Successfully consumed 5 messages, stopping");
                                    break;
                                }
                            }
                            if message_count >= 5 {
                                break;
                            }
                        }
                        if message_count >= 5 {
                            break;
                        }

                        // Commit offsets
                        if let Err(e) = consumer.commit_consumed() {
                            println!("⚠️  Failed to commit offsets: {}", e);
                        }

                        // Brief pause between polls
                        std::thread::sleep(Duration::from_millis(100));
                    }
                    Err(e) => {
                        println!("❌ ERROR: Failed to poll: {}", e);
                        println!();
                        println!("📊 Summary:");
                        println!("   Status: FAILED");
                        println!("   Error: {}", e);
                        println!("   Messages consumed: {}", message_count);
                        return Err(Box::new(e));
                    }
                }
            }

            println!();
            println!("📊 Summary:");
            println!("   Status: SUCCESS");
            println!("   Messages consumed: {}", message_count);
            println!("   No NotImplemented errors!");
            println!();
            println!("✅ kafka-rust WORKS on macOS!");
            Ok(())
        }
        Err(e) => {
            println!("❌ ERROR: Failed to create consumer: {}", e);
            println!();
            println!("📊 Summary:");
            println!("   Status: FAILED");
            println!("   Error: {}", e);
            println!();
            println!("❌ kafka-rust FAILED on macOS");
            Err(Box::new(e))
        }
    }
}
