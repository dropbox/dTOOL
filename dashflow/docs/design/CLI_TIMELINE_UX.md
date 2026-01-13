# CLI Timeline UX Design Spec

**M-38:** Unify `watch`/`replay`/`visualize` UX around a shared "graph timeline" model.

**Author:** Worker #2120
**Date:** 2025-12-29
**Status:** Phase 2 Complete
**Last Updated:** 2025-12-30 (Worker #2155 - Fixed stale file paths)

## Implementation Status

| Phase | Status | Commit | Notes |
|-------|--------|--------|-------|
| Phase 1 | ✅ Complete | #2120 | Design spec created |
| Phase 2 | ✅ Complete | #2127 | `dashflow timeline` command implemented |
| Phase 3 | Pending | - | Deprecation notices added, wait for 2 releases |
| Phase 4 | Pending | - | Web timeline UI extension |

---

## 1. Current State

DashFlow has three CLI commands for observing graph executions:

### 1.1 `dashflow watch`

**Purpose:** Live TUI visualization of graph execution from Kafka.

```
dashflow watch [OPTIONS]

Options:
  -b, --bootstrap-servers <BOOTSTRAP_SERVERS>  [env: KAFKA_BROKERS=] [default: localhost:9092]
  -t, --topic <TOPIC>                          [env: KAFKA_TOPIC=] [default: dashstream-events]
      --thread <THREAD>                        Filter by thread ID
  -f, --from-beginning                         Start from beginning of topic
  -r, --refresh-ms <REFRESH_MS>                Refresh rate [default: 100]
```

**Characteristics:**
- TUI-based (terminal)
- Real-time streaming from Kafka
- Filter by thread ID
- No time-travel (live only)

### 1.2 `dashflow replay`

**Purpose:** Time-travel debugging from checkpoints.

```
dashflow replay [OPTIONS] --thread-id <THREAD_ID>

Options:
  -b, --bootstrap-servers <BOOTSTRAP_SERVERS>  [env: KAFKA_BROKERS=] [default: localhost:9092]
  -t, --topic <TOPIC>                          [env: KAFKA_TOPIC=] [default: dashstream-events]
      --thread-id <THREAD_ID>                  Thread ID to replay (REQUIRED)
      --from-timestamp <FROM_TIMESTAMP>        Start from timestamp (RFC3339 or Unix micros)
      --to-timestamp <TO_TIMESTAMP>            Stop at timestamp
      --from-checkpoint <FROM_CHECKPOINT>      Start from checkpoint ID
      --speed <SPEED>                          Playback speed (0 = instant) [default: 0]
      --events <EVENTS>                        Filter event types
      --pause-on-error                         Pause on errors
```

**Characteristics:**
- Historical playback
- Requires thread ID
- Time range filtering
- Playback speed control
- Checkpoint-based seeking

### 1.3 `dashflow visualize`

**Purpose:** Interactive web UI for graph visualization.

```
dashflow visualize <COMMAND>

Commands:
  view    View Mermaid file in web UI
  export  Export standalone HTML
  serve   Start visualization server
```

Subcommand options:
- `view <INPUT>`: Path to .mmd or JSON file, `--open`, `--port`
- `serve`: `--port`, `--public`
- `export`: (produces static HTML)

**Characteristics:**
- Web-based (browser)
- File-based input (not Kafka)
- Static graph structure visualization
- No live streaming

---

## 2. Inconsistencies Identified

### 2.1 Data Source Inconsistency

| Command | Data Source | Time Model |
|---------|-------------|------------|
| watch | Kafka (live) | Real-time only |
| replay | Kafka (historical) | Time-travel |
| visualize | File (.mmd, JSON) | Static snapshot |

**Issue:** No unified data source abstraction. Users must know which command to use for which scenario.

### 2.2 Flag Inconsistencies

| Flag | watch | replay | visualize |
|------|-------|--------|-----------|
| `--thread` | `--thread` | `--thread-id` | N/A |
| `--bootstrap-servers` | `-b` short | `-b` short | N/A |
| `--port` | N/A | N/A | `-p` short |
| Time filtering | `--from-beginning` | `--from-timestamp`, `--to-timestamp` | N/A |

**Issues:**
- `--thread` vs `--thread-id` (inconsistent naming)
- Different concepts for time filtering
- visualize doesn't support streaming at all

### 2.3 Output Mode Inconsistency

| Command | Output | Interactivity |
|---------|--------|---------------|
| watch | TUI | Keyboard controls |
| replay | TUI | Playback controls |
| visualize | Web browser | Mouse/click |

**Issue:** No way to use web UI for live streaming, or TUI for file visualization.

### 2.4 Conceptual Gap: "Timeline" vs "Snapshot"

- `watch` and `replay` operate on a **timeline** (sequence of events over time)
- `visualize` operates on a **snapshot** (static graph structure)

There's no command that combines both: showing graph structure with execution timeline overlay.

---

## 3. Proposed Unified Model: Graph Timeline

### 3.1 Core Concept

A **Graph Timeline** is a unified data model representing:
1. **Graph Structure:** Nodes, edges, entry/exit points
2. **Execution Events:** Node starts, completions, state changes, errors
3. **Time Cursor:** Current position in the timeline (live = latest, replay = historical)

### 3.2 Unified Command Structure

```
dashflow timeline <SUBCOMMAND>

Subcommands:
  live      Watch live execution (current: watch)
  replay    Replay historical execution (current: replay)
  view      View static graph (current: visualize view)
  export    Export visualization (current: visualize export)
```

### 3.3 Shared Flags

All timeline subcommands share these flags where applicable:

```
Common flags:
  --thread <THREAD_ID>        Filter by thread ID
  --output <MODE>             Output mode: tui, web, json [default: tui for live/replay, web for view]

Data source flags (live/replay only):
  -b, --brokers <SERVERS>     Kafka brokers [env: KAFKA_BROKERS=] [default: localhost:9092]
  -t, --topic <TOPIC>         Kafka topic [env: KAFKA_TOPIC=] [default: dashstream-events]

Time flags (replay only):
  --from <TIMESTAMP>          Start time (RFC3339 or Unix micros or checkpoint ID)
  --to <TIMESTAMP>            End time
  --speed <MULTIPLIER>        Playback speed [default: 0 = instant]

Server flags (web output only):
  -p, --port <PORT>           Server port [default: 8765]
  --open                      Open browser automatically
```

### 3.4 Example Usage

```bash
# Live monitoring in TUI (equivalent to current: dashflow watch)
dashflow timeline live --thread my-agent

# Live monitoring in web UI (new capability!)
dashflow timeline live --thread my-agent --output web --open

# Replay in TUI (equivalent to current: dashflow replay)
dashflow timeline replay --thread my-agent --from 2025-01-01T00:00:00Z

# Replay in web UI (new capability!)
dashflow timeline replay --thread my-agent --from checkpoint-123 --output web

# View static graph (equivalent to current: dashflow visualize view)
dashflow timeline view graph.mmd --open

# Export static visualization (equivalent to current: dashflow visualize export)
dashflow timeline export graph.mmd -o visualization.html
```

---

## 4. Migration Path

### 4.1 Phase 1: Aliases (Non-Breaking)

Keep existing commands as aliases to new unified commands:

```
dashflow watch  → dashflow timeline live
dashflow replay → dashflow timeline replay
dashflow visualize view  → dashflow timeline view
dashflow visualize serve → dashflow timeline serve
```

Add deprecation notices to old commands.

### 4.2 Phase 2: Flag Harmonization

Update flags for consistency:
- `--thread-id` → `--thread` (replay command)
- Add `--output` flag to live/replay for web mode

### 4.3 Phase 3: Deprecation

After 2 releases:
- Mark old commands as deprecated in help text
- Remove from documentation
- Keep functional for backwards compatibility

### 4.4 Phase 4: Web Timeline UI

Extend `observability-ui` to support:
- Live streaming mode (connect to websocket)
- Replay mode (fetch historical data)
- Unified timeline scrubber for both modes

---

## 5. Implementation Notes

### 5.1 Shared Components

```rust
// Common data model for all timeline commands
pub struct GraphTimeline {
    pub graph: GraphStructure,
    pub events: Vec<ExecutionEvent>,
    pub cursor: TimelineCursor,
}

pub enum TimelineCursor {
    Live,                    // Follow latest events
    Historical(Timestamp),   // Fixed point in time
    Replay { start: Timestamp, speed: f64 }, // Playback mode
}

pub enum OutputMode {
    Tui,
    Web { port: u16, open: bool },
    Json,
}
```

### 5.2 Data Source Abstraction

```rust
pub trait TimelineSource: Send {
    async fn events(&self) -> impl Stream<Item = ExecutionEvent>;
    async fn graph_at(&self, cursor: &TimelineCursor) -> GraphStructure;
}

// Implementations
struct KafkaSource { brokers: String, topic: String }
struct FileSource { path: PathBuf }
struct WalSource { storage_dir: PathBuf }  // Future: local WAL files
```

### 5.3 Rendering Abstraction

```rust
pub trait TimelineRenderer {
    async fn render(&mut self, timeline: &GraphTimeline) -> Result<()>;
    fn supports_streaming(&self) -> bool;
}

// Implementations
struct TuiRenderer { ... }
struct WebRenderer { server: WebServer }
struct JsonRenderer { output: Box<dyn Write> }
```

---

## 6. Open Questions

1. **Should `visualize export` support timeline data?**
   - Currently exports static graph only
   - Could export timeline as interactive HTML with replay controls

2. **Should we support WAL files as a data source?**
   - Current: Kafka only for live/replay
   - Future: Could read from local `.dashflow/wal/` files without Kafka

3. **What about non-Kafka streaming?**
   - Current: Only Kafka
   - Future: WebSocket, file tailing, etc.

4. **How to handle very long timelines?**
   - Memory limits for web UI
   - Pagination or windowing for TUI

---

## 7. Success Criteria

M-38 Phase 2 is complete when:

- [x] New `dashflow timeline` command structure is implemented (#2127)
- [x] Old commands work as aliases with deprecation notices (#2127)
- [x] `--thread` flag is consistent across all subcommands (#2127: added `--thread` alias to replay)
- [ ] `--output web` works for live and replay modes (Phase 4: Web timeline UI)
- [x] Documentation updated to reflect unified model (#2127: CLI help updated)
- [ ] Migration guide available for existing users (can be added when Phase 3 deprecation happens)

---

## Appendix: Current Command Locations

| Command | CLI Definition | Implementation |
|---------|---------------|----------------|
| watch | `crates/dashflow-cli/src/commands/watch.rs` | TUI rendering |
| replay | `crates/dashflow-cli/src/commands/replay.rs` | Playback logic |
| visualize | `crates/dashflow-cli/src/commands/visualize.rs` | Web server |
| timeline | `crates/dashflow-cli/src/commands/timeline.rs` | Unified timeline command |
