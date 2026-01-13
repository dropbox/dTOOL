# Testing the Observability Stack

Step-by-step guide for testing DashFlow's observability infrastructure.

## Prerequisites

- Docker and Docker Compose installed
- At least 8GB RAM available for containers
- Ports 2181, 3000, 3002, 3003, 9090, 9092, 9190, 16686 available

## Quick Start: E2E Stack Validation

> **Note (Dec 2025):** The `e2e_stack_validation.sh` script was removed during consolidation.
> Use the manual testing steps below to validate the stack.

The recommended way to test the stack:

```bash
# 1. Start the stack
docker-compose -f docker-compose.dashstream.yml up -d

# 2. Wait for services and verify health
docker-compose -f docker-compose.dashstream.yml ps
curl http://localhost:3002/health  # WebSocket server
curl http://localhost:9090/-/healthy  # Prometheus

# 3. Send test events
cargo run -p dashflow-streaming --bin quality_aggregator

# 4. Verify metrics in Prometheus
curl -s "http://localhost:9090/api/v1/query?query=dashstream_quality_monitor_quality_score" | jq
```

## Manual Testing Guide

### Step 1: Start the Full Stack

```bash
# Start all services
docker-compose -f docker-compose.dashstream.yml up -d

# Verify all containers are running
docker-compose -f docker-compose.dashstream.yml ps

# Expected services:
# - dashstream-zookeeper (port 2181)
# - dashstream-kafka (port 9092)
# - dashstream-quality-monitor (port 3003)
# - dashstream-websocket-server (port 3002)
# - dashstream-prometheus-exporter (port 9190)
# - dashstream-prometheus (port 9090)
# - dashstream-grafana (port 3000)
# - dashstream-jaeger (port 16686)
```

### Step 2: Verify Services Are Healthy

```bash
# Zookeeper
echo ruok | nc localhost 2181  # Should return "imok"

# Kafka (requires kafkacat/kcat)
kcat -b localhost:9092 -L | head -10

# Quality Monitor
curl http://localhost:3003/health

# WebSocket Server
curl http://localhost:3002/health

# Prometheus Exporter metrics
curl http://localhost:9190/metrics | head -20

# Prometheus
curl http://localhost:9090/-/healthy

# Grafana
curl http://localhost:3000/api/health
```

### Step 3: Send Test Events

```bash
# Option A: Run the quality aggregator (generates real events)
cargo run -p dashflow-streaming --bin quality_aggregator

# Option B: Manual Kafka message (requires kafkacat/kcat)
echo '{"type":"quality_score","value":0.85,"timestamp":"'"$(date -u +%Y-%m-%dT%H:%M:%SZ)"'"}' | \
  kcat -b localhost:9092 -t dashstream-quality -P
```

### Step 4: Verify Metrics in Prometheus

```bash
# Check that dashstream metrics exist
curl -s "http://localhost:9090/api/v1/label/__name__/values" | jq '.data[]' | grep dashstream

# Query specific metric
curl -s "http://localhost:9090/api/v1/query?query=dashstream_quality_monitor_quality_score" | jq

# Expected: Should show recent data points with values in 0.0-1.0 range
```

### Step 5: Verify Dashboard in Grafana

1. Open http://localhost:3000 in browser
2. Login with admin/admin
3. Navigate to Dashboards > DashFlow Quality Dashboard
4. Verify panels show data (not "No data"):
   - **Current Quality Score**: Should be 0.0-1.0
   - **Total Queries**: Should be incrementing
   - **Failure Rate**: Should be 0.0-1.0 percentage

```bash
# Programmatic check via Grafana API
# Note: datasource uid must match your Grafana datasource configuration
# Get datasource uid: curl -s -u admin:admin http://localhost:3000/api/datasources | jq '.[0].uid'
curl -s -u admin:admin \
  "http://localhost:3000/api/ds/query" \
  -H "Content-Type: application/json" \
  -d '{
    "queries": [{
      "refId": "A",
      "datasource": {"type": "prometheus", "uid": "prometheus"},
      "expr": "dashstream_quality_monitor_quality_score"
    }],
    "from": "now-5m",
    "to": "now"
  }' | jq '.results.A.frames[0].data.values'
```

## Common Problems and Solutions

### Problem: "No data" in Grafana panels

**Possible causes:**
1. Prometheus datasource not configured
2. No events generated yet
3. Metric names don't match queries

**Solutions:**
```bash
# Check datasource
curl -s -u admin:admin http://localhost:3000/api/datasources | jq '.[].name'

# Check Prometheus has data
curl -s "http://localhost:9090/api/v1/query?query=up" | jq '.data.result'

# Generate test events
cargo run -p dashflow-streaming --bin quality_aggregator
```

### Problem: Services won't start

**Possible causes:**
1. Ports already in use
2. Not enough memory
3. Previous containers not cleaned up

**Solutions:**
```bash
# Kill any process on conflicting port
lsof -ti :9092 | xargs kill -9

# Clean up previous containers
docker-compose -f docker-compose.dashstream.yml down -v
docker system prune -f

# Start fresh
docker-compose -f docker-compose.dashstream.yml up -d
```

### Problem: Prometheus targets showing "down"

**Possible causes:**
1. Service hasn't started yet
2. Wrong hostname in prometheus.yml
3. Service crashed

**Solutions:**
```bash
# Check target status
curl -s localhost:9090/api/v1/targets | jq '.data.activeTargets[] | {job: .labels.job, health: .health}'

# View service logs
docker-compose -f docker-compose.dashstream.yml logs websocket-server

# Restart unhealthy service
docker-compose -f docker-compose.dashstream.yml restart websocket-server
```

### Problem: Metrics exist but values are wrong

**Possible causes:**
1. PromQL query error (rate on gauge, missing labels)
2. Dashboard using wrong time range
3. Stale data from previous runs

**Solutions:**
```bash
# Verify raw metric value
curl -s "http://localhost:9090/api/v1/query?query=dashstream_quality_monitor_quality_score" | \
  jq '.data.result[0].value[1]'

# Run dashboard lint
python3 scripts/lint_grafana_dashboard.py grafana/dashboards/*.json
```

## Automated Tests

### Unit Tests
```bash
cargo test -p dashflow-observability
cargo test -p dashflow-prometheus-exporter
cargo test -p test-utils -- observability
```

### Integration Tests
```bash
# Requires docker stack running
cargo test -p test-utils --test observability_pipeline

# Playwright dashboard tests (requires npm install in test-utils)
cd test-utils && npm test
```

### CI Validation
The following checks run in CI:
- `scripts/lint_grafana_dashboard.py` - Validates dashboard JSON
- `cargo test -p test-utils --test observability_pipeline` - Integration tests

> **Note:** The `e2e_stack_validation.sh` script was removed during consolidation. Manual testing steps are documented above.

## Related Documentation

- [Prometheus Metrics Reference](../monitoring/PROMETHEUS_METRICS.md)
- [Alert Rules](../monitoring/alert_rules.yml)
- [Observability UI Architecture](../observability-ui/ARCHITECTURE.md)
- [Docker Compose Configuration](../docker-compose.dashstream.yml)
