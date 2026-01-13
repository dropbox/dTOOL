# LLM-as-Judge Observability Validation Tests

## Overview

This directory contains LLM-as-judge integration tests that validate the observability stack using OpenAI GPT-4o-mini for visual inspection and functional verification.

## Test Scripts

### 1. `llm_validate_observability_ui.py`

Validates the React-based Observability UI (Issue #15):
- Checks if UI loads without errors
- Verifies WebSocket connection to server (localhost:3002)
- Confirms real-time DashFlow Streaming events are displayed
- Validates connection status indicator
- Inspects event stream rendering (timestamps, types, thread IDs)

**Usage:**
```bash
python3 scripts/llm_validate_observability_ui.py
```

**Prerequisites:**
- Observability UI dev server running at `localhost:5173`
- WebSocket server running at `localhost:3002`
- OPENAI_API_KEY environment variable set

### 2. `llm_validate_jaeger_traces.py`

Validates Jaeger distributed tracing (Issue #14):
- Checks if websocket-server service is registered
- Verifies traces are being generated
- Validates span data quality (operation names, tags, timing)
- Tests Jaeger UI accessibility and functionality

**Usage:**
```bash
python3 scripts/llm_validate_jaeger_traces.py
```

**Prerequisites:**
- Jaeger running at `localhost:16686`
- websocket-server generating traces
- OPENAI_API_KEY environment variable set

### 3. `comprehensive_observability_tests.py`

Runs all observability validation tests in sequence:
- Infrastructure metrics (Prometheus) - Issue #12
- Distributed tracing (Jaeger) - Issue #14
- Observability UI (React + WebSocket) - Issue #15

**Usage:**
```bash
python3 scripts/comprehensive_observability_tests.py
```

**Prerequisites:**
- All services running (Prometheus, Jaeger, WebSocket server, UI)
- OPENAI_API_KEY environment variable set

## Installation

1. Install Python dependencies:
```bash
pip install -r scripts/requirements-observability-tests.txt
```

2. Install Playwright browsers:
```bash
playwright install chromium
```

3. Set OpenAI API key:
```bash
export OPENAI_API_KEY="sk-proj-..."
# Or source the .env file in repo root
source .env
```

## Starting Required Services

### Option 1: Docker Compose (All Services)

```bash
# Start all services
docker-compose -f docker-compose.dashstream.yml up -d

# Check services are running
docker ps | grep dashstream
```

### Option 2: Individual Services

```bash
# 1. Start Kafka
docker-compose -f docker-compose-kafka.yml up -d

# 2. Start WebSocket server
cd crates/dashflow-observability
cargo run --bin websocket-server

# 3. Start Jaeger (if not started by docker-compose)
docker run -d --name jaeger \
  -p 16686:16686 \
  -p 4317:4317 \
  jaegertracing/all-in-one:latest

# 4. Start Prometheus
docker run -d --name prometheus \
  -p 9090:9090 \
  -v $(pwd)/prometheus.yml:/etc/prometheus/prometheus.yml \
  prom/prometheus

# 5. Start Observability UI
cd observability-ui
npm install
npm run dev  # Runs on localhost:5173
```

## Output Format

All test scripts return JSON to stdout:

```json
{
  "verdict": "PASS" | "FAIL",
  "confidence": 0-100,
  "reasoning": "Brief explanation of verdict",
  "screenshots": ["base64..."],  // Where applicable
  "timestamp": "2025-11-21T18:40:00.000Z",
  // ... test-specific fields
}
```

Exit codes:
- `0`: PASS
- `1`: FAIL

## Example Run

```bash
# Set API key
export OPENAI_API_KEY="sk-proj-..."

# Start all services
docker-compose -f docker-compose.dashstream.yml up -d

# Wait for services to be ready
sleep 10

# Run comprehensive test suite
python3 scripts/comprehensive_observability_tests.py

# Check exit code
echo "Exit code: $?"
```

## Troubleshooting

### Test fails with "Timeout"
- Ensure all required services are running
- Check service health: `docker ps` and `curl localhost:9090/-/healthy`
- Increase timeout in test script if services are slow to respond

### Test fails with "OPENAI_API_KEY not set"
- Export the environment variable: `export OPENAI_API_KEY="sk-..."`
- Or source the `.env` file: `source .env`

### Test fails with "Playwright not installed"
- Install Playwright: `pip install playwright`
- Install browsers: `playwright install chromium`

### Test fails with "Module not found"
- Install requirements: `pip install -r scripts/requirements-observability-tests.txt`

## Integration with CI/CD

These tests can be integrated into CI/CD pipelines.

> **Note:** DashFlow uses internal Dropbox CI, not GitHub Actions. The `.github/` directory does not exist in this repository. The workflow below is a template for teams using GitHub Actions.

```yaml
# .github/workflows/observability-validation.yml
name: Observability Validation

on: [push, pull_request]

jobs:
  validate:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions/setup-python@v4
        with:
          python-version: '3.11'

      - name: Install dependencies
        run: |
          pip install -r scripts/requirements-observability-tests.txt
          playwright install chromium

      - name: Start services
        run: docker-compose -f docker-compose.dashstream.yml up -d

      - name: Wait for services
        run: sleep 15

      - name: Run observability tests
        env:
          OPENAI_API_KEY: ${{ secrets.OPENAI_API_KEY }}
        run: python3 scripts/comprehensive_observability_tests.py
```

## Test Coverage

| Issue | Component | Test Script | Status |
|-------|-----------|-------------|--------|
| #12 | Infrastructure Metrics | `comprehensive_observability_tests.py` | ✅ Complete |
| #14 | Distributed Tracing | `llm_validate_jaeger_traces.py` | ✅ Complete |
| #15 | Observability UI | `llm_validate_observability_ui.py` | ✅ Complete |

## Next Steps

After these tests are validated:
1. Implement Issue #11: Consumer-side sequence validation
2. Implement Issue #13: Dead Letter Queue handling

See `[WORKER]_ISSUES_11-15_CORRECTED_STATUS.md` for details.
