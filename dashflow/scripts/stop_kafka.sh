#!/bin/bash
set -euo pipefail  # Exit on error, undefined var, pipe fail
# Stop local Kafka
# © 2025 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

# Navigate to repo root (script is in scripts/)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

echo "=== Stopping Kafka ==="

docker-compose -f docker-compose-kafka.yml down

echo "✅ Kafka stopped"
echo ""
echo "To preserve data: Volumes remain (zookeeper-data, kafka-data)"
echo "To remove all data: docker-compose -f docker-compose-kafka.yml down -v"
