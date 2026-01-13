//! Send Test Quality Metrics to Kafka
//!
//! This example sends a test Metrics message with scope="quality" to validate
//! the observability pipeline.
//!
//! Usage:
//!   cargo run -p dashflow-streaming --example send_test_metrics
//!
//! Prerequisites:
//!   docker-compose -f docker-compose-kafka.yml up -d

use dashflow_streaming::producer::DashStreamProducer;
use dashflow_streaming::{Header, MessageType, MetricValue, Metrics};
use std::collections::HashMap;
use std::time::Duration;

const BOOTSTRAP_SERVERS: &str = "localhost:9092";
const TOPIC: &str = "dashstream-quality";

fn create_quality_metrics(
    thread_id: &str,
    accuracy: f64,
    relevance: f64,
    completeness: f64,
    quality_score: f64,
    passed: bool,
) -> Metrics {
    let mut metrics_map: HashMap<String, MetricValue> = HashMap::new();

    metrics_map.insert(
        "accuracy".to_string(),
        MetricValue {
            value: Some(dashflow_streaming::metric_value::Value::FloatValue(
                accuracy,
            )),
            unit: "ratio".to_string(),
            r#type: dashflow_streaming::metric_value::MetricType::Gauge as i32,
        },
    );

    metrics_map.insert(
        "relevance".to_string(),
        MetricValue {
            value: Some(dashflow_streaming::metric_value::Value::FloatValue(
                relevance,
            )),
            unit: "ratio".to_string(),
            r#type: dashflow_streaming::metric_value::MetricType::Gauge as i32,
        },
    );

    metrics_map.insert(
        "completeness".to_string(),
        MetricValue {
            value: Some(dashflow_streaming::metric_value::Value::FloatValue(
                completeness,
            )),
            unit: "ratio".to_string(),
            r#type: dashflow_streaming::metric_value::MetricType::Gauge as i32,
        },
    );

    metrics_map.insert(
        "quality_score".to_string(),
        MetricValue {
            value: Some(dashflow_streaming::metric_value::Value::FloatValue(
                quality_score,
            )),
            unit: "ratio".to_string(),
            r#type: dashflow_streaming::metric_value::MetricType::Gauge as i32,
        },
    );

    metrics_map.insert(
        "passed".to_string(),
        MetricValue {
            value: Some(dashflow_streaming::metric_value::Value::BoolValue(passed)),
            unit: "boolean".to_string(),
            r#type: dashflow_streaming::metric_value::MetricType::Gauge as i32,
        },
    );

    let mut tags: HashMap<String, String> = HashMap::new();
    tags.insert("thread_id".to_string(), thread_id.to_string());
    tags.insert("tenant_id".to_string(), "test-tenant".to_string());

    Metrics {
        header: Some(Header {
            message_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
            timestamp_us: chrono::Utc::now().timestamp_micros(),
            tenant_id: "test-tenant".to_string(),
            thread_id: thread_id.to_string(),
            sequence: 1,
            r#type: MessageType::Metrics as i32,
            parent_id: vec![],
            compression: 0,
            schema_version: 1,
        }),
        scope: "quality".to_string(),
        scope_id: thread_id.to_string(),
        metrics: metrics_map,
        tags,
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧪 Sending Test Quality Metrics to Kafka");
    println!("=========================================\n");

    println!("Connecting to Kafka at {}...", BOOTSTRAP_SERVERS);
    let producer = DashStreamProducer::new(BOOTSTRAP_SERVERS, TOPIC).await?;
    println!("✅ Producer connected to topic: {}\n", TOPIC);

    // Send multiple test metrics
    // Each tuple: (thread_id, accuracy, relevance, completeness, quality_score, passed)
    let test_scores = [
        ("test-thread-001", 0.95, 0.87, 0.92, 0.91, true), // High quality, passed
        ("test-thread-002", 0.88, 0.91, 0.85, 0.88, true), // Good quality, passed
        ("test-thread-003", 0.72, 0.68, 0.79, 0.73, false), // Lower quality, failed
    ];

    for (thread_id, accuracy, relevance, completeness, quality_score, passed) in test_scores {
        println!(
            "📊 Sending metrics for {}: accuracy={}, relevance={}, completeness={}, quality_score={}, passed={}",
            thread_id, accuracy, relevance, completeness, quality_score, passed
        );

        let metrics = create_quality_metrics(
            thread_id,
            accuracy,
            relevance,
            completeness,
            quality_score,
            passed,
        );
        producer.send_metrics(metrics).await?;
        println!("   ✅ Sent");

        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    // Flush
    println!("\n🔄 Flushing messages...");
    producer.flush(Duration::from_secs(5)).await?;
    println!("✅ All test metrics sent successfully!");

    println!("\n📋 Next steps:");
    println!("   1. Check quality-monitor logs: docker logs dashstream-quality-monitor");
    println!("   2. Check Grafana: http://localhost:3000");
    println!("   3. Check Prometheus metrics:");
    println!("      - Accuracy: curl 'http://localhost:9090/api/v1/query?query=dashstream_quality_accuracy'");
    println!("      - Quality Score: curl 'http://localhost:9090/api/v1/query?query=dashstream_quality_monitor_quality_score'");
    println!("      - Passed: curl 'http://localhost:9090/api/v1/query?query=dashstream_quality_monitor_queries_passed_total'");

    Ok(())
}
