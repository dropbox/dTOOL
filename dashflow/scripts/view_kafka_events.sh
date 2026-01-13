#!/bin/bash
set -euo pipefail
# View DashFlow Streaming events from Kafka
# © 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

echo "=== Viewing DashFlow Streaming Events from Kafka ==="
echo "Press Ctrl+C to stop"
echo ""

docker exec kafka kafka-console-consumer \
  --bootstrap-server localhost:9092 \
  --topic dashstream_events \
  --from-beginning \
  --property print.key=true \
  --property print.timestamp=true

# Note: Events are protobuf-encoded (binary)
# To decode, would need protobuf deserializer
# For JSON, apps could log to a file-based logger instead
