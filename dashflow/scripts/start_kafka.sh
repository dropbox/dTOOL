#!/bin/bash
# Start local Kafka for DashFlow Streaming testing
# © 2025 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

set -euo pipefail

# Navigate to repo root (script is in scripts/)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

echo "=== Starting Local Kafka for DashFlow Streaming ==="

if docker ps | grep -q kafka; then
    echo "✅ Kafka already running"
else
    echo "Starting Kafka and Zookeeper..."
    docker-compose -f docker-compose-kafka.yml up -d

    echo "Waiting for Kafka to be ready (30 seconds)..."
    sleep 30
fi

# Verify Kafka is reachable
echo "Verifying Kafka connection..."
if nc -zv localhost 9092 2>&1 | grep -q succeeded; then
    echo "✅ Kafka ready at localhost:9092"
else
    echo "❌ Kafka not reachable on localhost:9092"
    echo "   Check: docker-compose -f docker-compose-kafka.yml ps"
    exit 1
fi

# Check if topic exists, create if not
echo "Ensuring dashstream_events topic exists..."
docker exec kafka kafka-topics --bootstrap-server localhost:9092 --list | grep -q dashstream_events || \
docker exec kafka kafka-topics --bootstrap-server localhost:9092 --create --topic dashstream_events --partitions 1 --replication-factor 1

echo ""
echo "✅ Kafka setup complete!"
echo ""
echo "Next steps:"
echo "  1. Run tests: cargo test dashstream -- --ignored"
echo "  2. Run apps with DashFlow Streaming enabled"
echo "  3. View events: ./scripts/view_kafka_events.sh"
echo "  4. Stop: ./scripts/stop_kafka.sh"
