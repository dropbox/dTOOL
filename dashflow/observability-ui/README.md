# DashFlow Observability UI

Real-time observability dashboard for DashFlow graph executions. Provides live visualization of graph state, time-travel debugging, and execution metrics.

## Features

- **Live Graph Visualization**: ReactFlow-based canvas showing graph topology with real-time node status
- **Time-Travel Debugging**: Scrub through execution history to inspect state at any point
- **Mermaid Export**: Export graph diagrams as Mermaid text for documentation
- **State Diff Viewer**: Compare state changes between execution steps
- **Execution Timeline**: Event-by-event timeline with timing information
- **Health Dashboard**: System health metrics and Kafka throughput monitoring

## Quick Start

```bash
# Install dependencies
npm install

# Start development server (port 5173)
npm run dev

# Build for production
npm run build
```

## Available Scripts

| Script | Description |
|--------|-------------|
| `npm run dev` | Start Vite development server with hot reload |
| `npm run build` | Build production bundle to `dist/` |
| `npm run preview` | Preview production build locally |
| `npm run typecheck` | Run TypeScript type checking |
| `npm run test` | Run typecheck + unit tests |
| `npm run test:unit` | Run unit tests only (324 tests) |
| `npm run test:e2e` | Run Playwright end-to-end tests |
| `npm run proto:gen` | Generate JSON schema from protobuf |
| `npm run proto:check` | Verify proto schema is up to date |

## Architecture

### Data Flow

```
┌─────────────────┐     ┌──────────────┐     ┌─────────────────┐
│ DashFlow Graph  │────>│    Kafka     │────>│ WebSocket Server│
│   (Backend)     │     │ (dashstream) │     │   (Port 3002)   │
└─────────────────┘     └──────────────┘     └────────┬────────┘
                                                      │
                                                      ▼
                                             ┌────────────────┐
                                             │ Observability  │
                                             │      UI        │
                                             │  (Port 5173)   │
                                             └────────────────┘
```

### Key Components

```
src/
├── App.tsx                      # Main dashboard component
├── components/
│   ├── GraphCanvas.tsx          # ReactFlow graph visualization
│   ├── GraphNode.tsx            # Custom node component
│   ├── MermaidView.tsx          # Mermaid text export view
│   ├── TimelineSlider.tsx       # Time-travel controls
│   ├── ExecutionTimeline.tsx    # Event timeline
│   ├── StateDiffViewer.tsx      # State diff visualization
│   └── StateViewer.tsx          # JSON state tree viewer
├── hooks/
│   └── useRunStateStore.ts      # Unified state management (time-travel + live)
├── proto/
│   ├── dashstream.ts            # Protobuf decoder
│   └── dashstream.schema.json   # Generated schema (proto:gen)
├── utils/
│   ├── jsonPatch.ts             # RFC 6902 JSON Patch implementation
│   └── mermaidRenderer.ts       # Graph to Mermaid conversion
├── types/
│   └── graph.ts                 # TypeScript type definitions
└── __tests__/
    ├── jsonPatch.test.ts        # 22 unit tests
    ├── mermaidRenderer.test.ts  # 33 unit tests
    └── ...                      # 20 additional test files (324 tests total)
```

### State Management

The UI uses a single unified state store:

**`useRunStateStore`** - Single source of truth for all state
- Tracks all runs by `thread_id`
- Maintains cursor `(thread_id, sequence)` for time-travel
- Reconstructs state at any point using JSON Patch (RFC 6902)
- Provides `getViewModel()` for unified graph rendering
- Handles deduplication, ordering, and out-of-order event handling
- Supports zstd compression and state hash verification

### WebSocket Protocol

The UI connects to `ws://localhost:3002/ws` (proxied via Vite in dev) and receives:

- **Event** - Graph lifecycle events (GraphStart, NodeStart, NodeEnd, etc.)
- **StateDiff** - JSON Patch operations for state changes
- **Metrics** - Quality scores and performance metrics

### Graph Tabs

1. **Overview** - System health, throughput charts, error rates
2. **Events** - Raw event stream with filtering
3. **Metrics** - Quality metrics and dashboards
4. **Graph** - Interactive graph visualization with Canvas/Mermaid toggle

## Development

### Prerequisites

- Node.js 20+
- DashFlow WebSocket server running on port 3002
- (Optional) Kafka and observability stack for full data flow

### Quick Demo (Recommended)

From the repo root:

```bash
./demo_observability.sh
```

### Running with Full Stack

```bash
# Terminal 1: Start observability stack
cd ../observability && docker-compose up -d

# Terminal 2: Start WebSocket server
cd ..
cargo run -p dashflow-observability --features websocket-server --bin websocket_server

# Terminal 3: Start UI
cd ../observability-ui
npm run dev

# Terminal 4: Run a DashFlow app with dashstream feature
cd ../examples/apps/librarian
cargo run --features dashstream
```

### Running Tests

```bash
# Type checking
npm run typecheck

# Unit tests (324 tests across 22 test files)
npm run test:unit

# E2E tests (requires running UI server)
npm run dev &
npm run test:e2e

# All tests
npm run test
```

### Updating Proto Schema

When `proto/dashstream.proto` changes:

```bash
npm run proto:gen    # Regenerate src/proto/dashstream.schema.json
npm run proto:check  # CI validation - fails if out of sync
```

## Configuration

The UI uses Vite's proxy for development:

| Path | Target | Description |
|------|--------|-------------|
| `/ws` | `ws://localhost:3002` | WebSocket events |
| `/health` | `http://localhost:3002` | Health endpoint |
| `/version` | `http://localhost:3002` | Version info |
| `/metrics` | `http://localhost:3002` | Prometheus metrics |

For production, configure a reverse proxy (nginx) to route these paths.

## Time-Travel Debugging

The time-travel feature (Phase 0.6) allows inspecting execution state at any point:

1. **Select a run** from the dropdown (by `thread_id`)
2. **Toggle "Live"** off to enable time-travel
3. **Use the slider** or step buttons to navigate
4. **View state** in the State panel at that point in time

State reconstruction uses:
- Initial state from `GraphStart` event
- JSON Patch operations from `StateDiff` events
- Periodic checkpoints for efficient seeking

## Troubleshooting

### "Waiting for graph execution..."
- Ensure WebSocket server is running (`cargo run -p dashflow-observability --features websocket-server --bin websocket_server`)
- Check browser console for WebSocket connection errors
- Verify Kafka has messages (`kafka-console-consumer --topic dashstream-quality`)

### Time-travel shows wrong state
- Initial state is emitted on `GraphStart` - check backend emits `initial_state_json`
- Verify JSON Patch operations are valid RFC 6902

### TypeScript errors
- Run `npm run typecheck` to see all errors
- Ensure `@types/react` and `@types/react-dom` are installed
