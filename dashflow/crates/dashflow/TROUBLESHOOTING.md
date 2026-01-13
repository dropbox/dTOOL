# DashFlow Troubleshooting Guide

Comprehensive guide to diagnosing and solving common issues in DashFlow Rust workflows.

---

## Table of Contents

1. [State Serialization Issues](#1-state-serialization-issues)
2. [Checkpoint Problems](#2-checkpoint-problems)
3. [Performance Problems](#3-performance-problems)
4. [Memory Issues](#4-memory-issues)
5. [Debugging Techniques](#5-debugging-techniques)
6. [Common Pitfalls](#6-common-pitfalls)

---

## 1. State Serialization Issues

### Problem: Serialization Fails During Checkpointing

**Symptoms:**
```
Error: Failed to serialize state
  caused by: the trait `serde::Serialize` is not implemented for `MyType`
```

**Diagnosis:**

Check which fields fail serialization:
```rust
// Test individual fields
#[test]
fn test_state_serialization() {
    let state = MyState {
        field1: "test".to_string(),
        field2: vec![1, 2, 3],
        connection: Arc::new(Mutex::new(Connection::new())),  // This fails!
    };

    // Test full state
    let json = serde_json::to_string(&state);
    assert!(json.is_ok(), "Serialization failed: {:?}", json.err());
}
```

**Common Causes:**

1. **Non-serializable types** (connections, file handles, raw pointers)
2. **Generic types without serde bounds**
3. **Recursive types without size limit**
4. **Private fields in external crates**

**Solutions:**

**Solution 1: Skip non-serializable fields**
```rust
use serde::{Serialize, Deserialize};
use std::sync::{Arc, Mutex};

#[derive(Clone, Serialize, Deserialize)]
struct MyState {
    // Serializable fields
    data: HashMap<String, String>,
    counter: i32,

    // Skip non-serializable fields
    #[serde(skip)]
    connection: Arc<Mutex<Connection>>,

    #[serde(skip, default = "default_cache")]
    cache: Arc<Cache>,
}

fn default_cache() -> Arc<Cache> {
    Arc::new(Cache::new())
}
```

**Solution 2: Add serde bounds for generics**
```rust
use serde::{Serialize, Deserialize};

#[derive(Clone, Serialize, Deserialize)]
#[serde(bound = "T: Serialize + for<'de> Deserialize<'de>")]
struct GenericState<T> {
    data: Vec<T>,
    metadata: HashMap<String, String>,
}
```

**Solution 3: Use dynamic types for flexibility**
```rust
use serde_json::Value;

#[derive(Clone, Serialize, Deserialize)]
struct FlexibleState {
    // Type-safe fields
    user_id: String,

    // Dynamic fields (any JSON-serializable data)
    dynamic_data: HashMap<String, Value>,
}
```

**Solution 4: Custom serialization**
```rust
use serde::{Serialize, Deserialize, Serializer, Deserializer};

#[derive(Clone)]
struct MyState {
    connection_string: String,
    #[serde(skip)]
    connection: Connection,
}

impl Serialize for MyState {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Only serialize connection string, not connection itself
        #[derive(Serialize)]
        struct Helper<'a> {
            connection_string: &'a str,
        }

        Helper {
            connection_string: &self.connection_string,
        }
        .serialize(serializer)
    }
}
```

**Testing Strategy:**

Test serialization before runtime:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_roundtrip() {
        let original = MyState::new();

        // Test JSON serialization
        let json = serde_json::to_string(&original).expect("Serialize failed");
        let deserialized: MyState = serde_json::from_str(&json).expect("Deserialize failed");

        assert_eq!(original.data, deserialized.data);

        // Test bincode serialization (used by FileCheckpointer)
        let bytes = bincode::serialize(&original).expect("Bincode serialize failed");
        let deserialized: MyState = bincode::deserialize(&bytes).expect("Bincode deserialize failed");

        assert_eq!(original.data, deserialized.data);
    }
}
```

---

### Problem: Deserialization Version Mismatch

**Symptoms:**
```
Error: Failed to deserialize checkpoint
  caused by: missing field `new_field` at line 10
```

**Diagnosis:**

This occurs when state schema changes between serialization and deserialization (e.g., adding/removing fields).

**Solutions:**

**Solution 1: Use serde defaults**
```rust
#[derive(Clone, Serialize, Deserialize)]
struct MyState {
    // Required fields
    user_id: String,

    // Optional field with default (backward compatible)
    #[serde(default)]
    new_feature_flag: bool,

    // Optional field with custom default
    #[serde(default = "default_timeout")]
    timeout_ms: u64,
}

fn default_timeout() -> u64 {
    5000  // 5 second default
}
```

**Solution 2: Schema versioning**
```rust
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "version")]
enum VersionedState {
    #[serde(rename = "1")]
    V1(StateV1),
    #[serde(rename = "2")]
    V2(StateV2),
}

impl VersionedState {
    fn migrate_to_latest(self) -> StateV2 {
        match self {
            VersionedState::V1(v1) => v1.into(),
            VersionedState::V2(v2) => v2,
        }
    }
}
```

**Solution 3: Graceful degradation**
```rust
#[derive(Clone, Serialize, Deserialize)]
struct MyState {
    user_id: String,

    // Ignore unknown fields during deserialization
    #[serde(flatten)]
    extra: HashMap<String, serde_json::Value>,
}
```

---

## 2. Checkpoint Problems

### Problem: Cannot Save Checkpoint

**Symptoms:**
```
Error: Failed to save checkpoint
  caused by: Permission denied (os error 13)
```

**Diagnosis:**

Check file system permissions and disk space:
```bash
# Check directory permissions
ls -ld ./checkpoints

# Check disk space
df -h .

# Check if directory exists
test -d ./checkpoints && echo "Exists" || echo "Not found"

# Try creating test file
touch ./checkpoints/test.txt && rm ./checkpoints/test.txt
```

**Common Causes:**

1. **Directory doesn't exist**
2. **Insufficient permissions**
3. **Disk full**
4. **Read-only filesystem**

**Solutions:**

**Solution 1: Ensure directory exists**
```rust
use std::fs;

let checkpoint_dir = "./checkpoints";

// Create directory if it doesn't exist
fs::create_dir_all(checkpoint_dir)?;

let checkpointer = FileCheckpointer::new(checkpoint_dir)?;
```

**Solution 2: Handle permission errors gracefully**
```rust
use dashflow::checkpointer::{FileCheckpointer, MemoryCheckpointer};

fn create_checkpointer(path: &str) -> Result<Box<dyn Checkpointer<MyState>>> {
    match FileCheckpointer::new(path) {
        Ok(cp) => Ok(Box::new(cp)),
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            eprintln!("Warning: Cannot write to {}, using memory checkpointer", path);
            Ok(Box::new(MemoryCheckpointer::new()))
        }
        Err(e) => Err(e.into()),
    }
}
```

**Solution 3: Check disk space before saving**
```rust
use std::fs;

fn check_disk_space(path: &str) -> Result<u64> {
    let metadata = fs::metadata(path)?;
    // Note: This is a simplified check; for production use `statvfs` crate
    Ok(metadata.len())
}
```

---

### Problem: Cannot Resume from Checkpoint

**Symptoms:**
```
Error: Checkpoint not found for thread_id: session-123
```

**Diagnosis:**

Check if checkpoint exists and thread_id matches:
```bash
# List checkpoint files
ls -lh ./checkpoints/

# Check checkpoint content (JSON format)
cat ./checkpoints/session-123.json | jq .

# Check checkpoint content (bincode format)
# (requires custom tool or hexdump)
hexdump -C ./checkpoints/session-123.bin | head -20
```

**Common Causes:**

1. **Wrong thread_id** (typo or inconsistent ID)
2. **Checkpoint deleted or moved**
3. **Different checkpointer instance** (memory vs file)
4. **Checkpoint corruption**

**Solutions:**

**Solution 1: Use consistent thread_id**
```rust
// Save checkpoint
let thread_id = "user-456-session-789".to_string();
let app = graph.compile()?
    .with_checkpointer(checkpointer.clone())
    .with_thread_id(thread_id.clone());

let result = app.invoke(state).await?;

// Later: resume with SAME thread_id
let app2 = graph.compile()?
    .with_checkpointer(checkpointer)
    .with_thread_id(thread_id);  // Must match!

let resumed = app2.resume().await?;
```

**Solution 2: List available checkpoints**
```rust
use dashflow::checkpointer::Checkpointer;

async fn list_checkpoints(checkpointer: &FileCheckpointer) -> Result<Vec<String>> {
    // List all checkpoint files in directory
    let dir = checkpointer.directory();
    let mut thread_ids = Vec::new();

    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().map(|e| e == "bin").unwrap_or(false) {
            if let Some(stem) = path.file_stem() {
                thread_ids.push(stem.to_string_lossy().to_string());
            }
        }
    }

    Ok(thread_ids)
}

// Usage
let available = list_checkpoints(&checkpointer).await?;
println!("Available checkpoints: {:?}", available);
```

**Solution 3: Handle missing checkpoints gracefully**
```rust
async fn resume_or_start_new(
    graph: &StateGraph<MyState>,
    checkpointer: FileCheckpointer,
    thread_id: String,
    initial_state: MyState,
) -> Result<MyState> {
    let app = graph.compile()?
        .with_checkpointer(checkpointer.clone())
        .with_thread_id(thread_id.clone());

    match checkpointer.load(&thread_id.clone().into()).await {
        Ok(checkpoint) => {
            println!("Resuming from checkpoint");
            app.resume().await
        }
        Err(_) => {
            println!("No checkpoint found, starting new execution");
            app.invoke(initial_state).await
        }
    }
}
```

---

### Problem: Checkpoint Corruption

**Symptoms:**
```
Error: Failed to deserialize checkpoint
  caused by: unexpected end of file
```

**Diagnosis:**

Check checkpoint file integrity:
```bash
# Check file size (should be > 0)
ls -lh ./checkpoints/session-123.bin

# Check if file is truncated
tail -c 100 ./checkpoints/session-123.bin | hexdump -C

# Verify bincode format (should start with valid bincode header)
head -c 100 ./checkpoints/session-123.bin | hexdump -C
```

**Common Causes:**

1. **Incomplete write** (process killed during save)
2. **Disk full during write**
3. **Filesystem errors**
4. **Concurrent writes** (race condition)

**Solutions:**

**Solution 1: Atomic writes with backup**
```rust
use std::fs;
use std::path::Path;

async fn save_checkpoint_safe(
    checkpointer: &FileCheckpointer,
    thread_id: &str,
    state: &MyState,
) -> Result<()> {
    let checkpoint_path = format!("{}/{}.bin", checkpointer.directory(), thread_id);
    let temp_path = format!("{}.tmp", checkpoint_path);
    let backup_path = format!("{}.bak", checkpoint_path);

    // Write to temporary file
    let bytes = bincode::serialize(state)?;
    fs::write(&temp_path, &bytes)?;

    // Backup existing checkpoint
    if Path::new(&checkpoint_path).exists() {
        fs::copy(&checkpoint_path, &backup_path)?;
    }

    // Atomic rename
    fs::rename(&temp_path, &checkpoint_path)?;

    Ok(())
}
```

**Solution 2: Checkpoint verification**
```rust
async fn verify_checkpoint(
    checkpointer: &FileCheckpointer,
    thread_id: &str,
) -> Result<bool> {
    match checkpointer.load(&thread_id.to_string().into()).await {
        Ok(_) => Ok(true),
        Err(e) => {
            eprintln!("Checkpoint verification failed: {}", e);
            Ok(false)
        }
    }
}

// Usage: verify after save
checkpointer.save(&thread_id.into(), &checkpoint).await?;
if !verify_checkpoint(&checkpointer, &thread_id).await? {
    eprintln!("Warning: Checkpoint may be corrupted");
}
```

**Solution 3: Restore from backup**
```bash
# Manual recovery: restore from backup
cp ./checkpoints/session-123.bin.bak ./checkpoints/session-123.bin
```

---

## 3. Performance Problems

### Problem: Slow Graph Execution

**Symptoms:**
- Execution takes 10x longer than expected
- CPU usage low during execution
- Benchmarks show high latency

**Diagnosis:**

**Step 1: Profile with event callbacks**
```rust
use dashflow::callbacks::PrintCallback;
use std::time::Instant;

// Create timing callback
struct TimingCallback;

impl EventCallback<MyState> for TimingCallback {
    fn on_event(&self, event: &GraphEvent<MyState>) {
        match event {
            GraphEvent::NodeStart { node_name } => {
                println!("[{}] Node started: {}", Instant::now(), node_name);
            }
            GraphEvent::NodeEnd { node_name, duration_ms } => {
                println!("[{}] Node completed: {} ({} ms)",
                    Instant::now(), node_name, duration_ms);
            }
            _ => {}
        }
    }
}

// Use in graph
let app = graph.compile()?.with_callback(TimingCallback);
let result = app.invoke(state).await?;
```

**Step 2: Benchmark individual operations**
```bash
# Benchmark full workflow
cargo bench --package dashflow -- my_workflow

# Benchmark specific operations
cargo bench --package dashflow -- state_clone
cargo bench --package dashflow -- checkpoint
```

**Step 3: Profile with flamegraph**
```bash
# Install flamegraph
cargo install flamegraph

# Profile execution (requires root on Linux)
cargo flamegraph --example my_workflow

# Open flamegraph.svg in browser
```

**Common Causes:**

1. **Large state cloning** (state cloned on every node transition)
2. **Slow checkpointing** (file I/O dominates)
3. **Inefficient node implementations** (blocking operations)
4. **Debug build** (10-100x slower than release)

**Solutions:**

**Solution 1: Reduce state size**
```rust
// Before: Large state with unnecessary data
#[derive(Clone, Serialize, Deserialize)]
struct SlowState {
    messages: Vec<String>,  // Grows unbounded!
    full_history: Vec<Event>,  // 10,000+ items
    large_payload: Vec<u8>,  // 10 MB vector
}

// After: Optimized state
#[derive(Clone, Serialize, Deserialize)]
struct FastState {
    // Keep only essential data in state
    current_message: String,

    // Use Arc for large shared data (not cloned)
    #[serde(skip, default)]
    large_payload: Arc<Vec<u8>>,

    // Limit history size
    recent_history: VecDeque<Event>,  // Last 100 items only
}

impl FastState {
    fn add_to_history(&mut self, event: Event) {
        self.recent_history.push_back(event);
        if self.recent_history.len() > 100 {
            self.recent_history.pop_front();
        }
    }
}
```

**Solution 2: Use MemoryCheckpointer for non-persistent workflows**
```rust
// Before: File checkpointer (slow I/O)
let checkpointer = FileCheckpointer::new("./checkpoints")?;

// After: Memory checkpointer (100x faster)
let checkpointer = MemoryCheckpointer::new();

let app = graph.compile()?
    .with_checkpointer(checkpointer);
```

**Solution 3: Reduce checkpoint frequency**
```rust
// Before: Checkpoint after every node
graph.add_node_from_fn("process", |state| {
    Box::pin(async move {
        // Work...
        Ok(state)  // Checkpoint saved here
    })
});

// After: Checkpoint only at logical boundaries
graph.add_node_from_fn("process_batch", |state| {
    Box::pin(async move {
        // Process 100 items without checkpointing
        for item in &state.items {
            process_item(item);
        }
        Ok(state)  // Checkpoint saved once per batch
    })
});
```

**Solution 4: Use release builds**
```bash
# NEVER benchmark debug builds
cargo build --example my_workflow

# ALWAYS use release builds for performance testing
cargo build --release --example my_workflow
./target/release/examples/my_workflow

# Benchmarks use release by default
cargo bench --package dashflow
```

**Solution 5: Optimize async nodes**
```rust
// Before: Blocking operation in async node
graph.add_node_from_fn("slow_node", |state| {
    Box::pin(async move {
        // This blocks the tokio runtime!
        std::thread::sleep(Duration::from_secs(1));
        Ok(state)
    })
});

// After: Use tokio::time for async delays
graph.add_node_from_fn("fast_node", |state| {
    Box::pin(async move {
        // This yields to tokio runtime
        tokio::time::sleep(Duration::from_secs(1)).await;
        Ok(state)
    })
});

// Better: Use spawn_blocking for CPU-intensive work
graph.add_node_from_fn("cpu_intensive", |state| {
    Box::pin(async move {
        let result = tokio::task::spawn_blocking(move || {
            // Heavy CPU work here
            expensive_computation()
        }).await?;

        Ok(state.with_result(result))
    })
});
```

---

### Problem: High Node Execution Overhead

**Symptoms:**
- Flamegraph shows time in framework code, not node logic
- Many fast nodes (<1 μs each)
- Parallel execution slower than sequential

**Diagnosis:**

```bash
# Benchmark framework overhead
cargo bench --package dashflow -- overhead
```

**Common Causes:**

1. **Many trivial nodes** (framework overhead > node work)
2. **Excessive parallel spawning** (spawn overhead > benefit)
3. **Unnecessary checkpointing** (checkpoint time > node time)

**Solutions:**

**Solution 1: Merge small nodes**
```rust
// Before: Many trivial nodes
graph.add_node_from_fn("validate", validate);
graph.add_node_from_fn("lowercase", lowercase);
graph.add_node_from_fn("trim", trim);
graph.add_node_from_fn("dedupe", dedupe);
graph.add_edge("validate", "lowercase");
graph.add_edge("lowercase", "trim");
graph.add_edge("trim", "dedupe");

// After: Single preprocessing node
graph.add_node_from_fn("preprocess", |state| {
    Box::pin(async move {
        // All operations in one node
        let text = validate(&state.text)?;
        let text = text.to_lowercase();
        let text = text.trim().to_string();
        let text = dedupe(&text);

        Ok(state.with_text(text))
    })
});
```

**Solution 2: Use sequential edges for fast nodes**
```rust
// Before: Parallel execution with spawn overhead
graph.add_parallel_edge("start", vec!["fast1", "fast2", "fast3"]);
// Each node takes <5 μs, spawn takes 10 μs → 3x slower!

// After: Sequential execution (no spawn overhead)
graph.add_edge("start", "fast1");
graph.add_edge("fast1", "fast2");
graph.add_edge("fast2", "fast3");
// Total: 15 μs vs 30 μs+ for parallel
```

**Guideline:** Use parallel edges only when:
- 3+ nodes to parallelize
- Each node takes >5 μs
- Nodes are truly independent

---

### Problem: Memory Leaks or High Memory Usage

**Symptoms:**
- Memory usage grows continuously
- Process RSS increases over time
- OOM (out of memory) errors

**Diagnosis:**

**Step 1: Profile with valgrind (Linux)**
```bash
# Build release binary
cargo build --release --example my_workflow

# Profile with massif (heap profiler)
valgrind --tool=massif ./target/release/examples/my_workflow

# Analyze results
ms_print massif.out.<pid> | less
```

**Step 2: Check state size growth**
```rust
use std::mem::size_of_val;

graph.add_node_from_fn("monitor", |state| {
    Box::pin(async move {
        let size = size_of_val(&state);
        println!("State size: {} bytes", size);

        // Also check heap allocations
        let json = serde_json::to_string(&state)?;
        println!("Serialized size: {} bytes", json.len());

        Ok(state)
    })
});
```

**Step 3: Check checkpoint accumulation**
```bash
# List checkpoint files and sizes
ls -lh ./checkpoints/

# Total checkpoint directory size
du -sh ./checkpoints/
```

**Common Causes:**

1. **State accumulation** (state grows unbounded)
2. **Checkpoint accumulation** (old checkpoints never deleted)
3. **Event callback closures** (captured data not freed)
4. **Large Arc clones** (Arc itself is cloned many times)

**Solutions:**

**Solution 1: Limit state growth**
```rust
use std::collections::VecDeque;

#[derive(Clone, Serialize, Deserialize)]
struct BoundedState {
    // Use VecDeque with max size
    history: VecDeque<Event>,  // Max 100 items

    // Clear logs periodically
    #[serde(skip)]
    debug_logs: Arc<Mutex<Vec<String>>>,
}

impl BoundedState {
    fn add_event(&mut self, event: Event) {
        self.history.push_back(event);

        // Enforce limit
        while self.history.len() > 100 {
            self.history.pop_front();
        }
    }

    fn clear_logs(&mut self) {
        if let Ok(mut logs) = self.debug_logs.lock() {
            logs.clear();
        }
    }
}
```

**Solution 2: Clean up old checkpoints**
```rust
use std::time::{SystemTime, Duration};
use std::fs;

async fn cleanup_old_checkpoints(
    checkpointer: &FileCheckpointer,
    max_age: Duration,
) -> Result<usize> {
    let dir = checkpointer.directory();
    let now = SystemTime::now();
    let mut deleted = 0;

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let metadata = entry.metadata()?;
        let modified = metadata.modified()?;

        if let Ok(age) = now.duration_since(modified) {
            if age > max_age {
                fs::remove_file(entry.path())?;
                deleted += 1;
            }
        }
    }

    Ok(deleted)
}

// Usage: clean up checkpoints older than 24 hours
let deleted = cleanup_old_checkpoints(&checkpointer, Duration::from_secs(86400)).await?;
println!("Deleted {} old checkpoints", deleted);
```

**Solution 3: Use Arc sparingly**
```rust
// Bad: Arc cloned many times (Arc overhead accumulates)
#[derive(Clone)]
struct State {
    data1: Arc<Data>,
    data2: Arc<Data>,
    data3: Arc<Data>,
    data4: Arc<Data>,
    data5: Arc<Data>,
    // 100+ Arc fields... each clone increments refcount 100+ times!
}

// Good: Single Arc for entire shared data
#[derive(Clone)]
struct State {
    // Only clone Arc once per state clone
    shared: Arc<SharedData>,

    // Keep frequently mutated data outside Arc
    counter: i32,
    status: String,
}

struct SharedData {
    data1: Data,
    data2: Data,
    data3: Data,
    data4: Data,
    data5: Data,
}
```

---

## 4. Memory Issues

### Problem: Memory Usage Higher Than Expected

**Symptoms:**
- Process RSS 10x larger than state size
- Memory doesn't decrease after workflow completes
- Gradual memory growth over multiple runs

**Diagnosis:**

**Step 1: Measure baseline memory**
```bash
# Check process memory before and after
ps aux | grep my_workflow

# Or use /proc (Linux)
cat /proc/<pid>/status | grep -E "VmRSS|VmSize"
```

**Step 2: Compare state size vs process size**
```rust
use std::mem::size_of_val;

let state_size = size_of_val(&state);
println!("State size: {} bytes ({:.2} MB)",
    state_size, state_size as f64 / 1024.0 / 1024.0);

// Compare to process RSS
// Expected: RSS should be <10x state size
```

**Step 3: Check for memory leaks with valgrind**
```bash
# Build with debug symbols
cargo build --release --example my_workflow

# Run with leak check
valgrind --leak-check=full ./target/release/examples/my_workflow

# Look for "definitely lost" or "possibly lost" blocks
```

**Common Causes:**

1. **Large checkpoints kept in memory** (FileCheckpointer caches)
2. **Tokio runtime overhead** (task allocations)
3. **Debug symbols and allocator overhead**
4. **Fragmentation** (many small allocations)

**Solutions:**

**Solution 1: Use streaming for large workflows**
```rust
// Before: Keep all results in memory
let results = app.invoke(state).await?;

// After: Stream results to disk/database
let mut stream = app.stream(state, StreamMode::Values).await?;
while let Some(event) = stream.next().await {
    // Process event immediately, don't accumulate
    process_and_store(event).await?;
}
```

**Solution 2: Disable checkpoint caching**
```rust
// FileCheckpointer caches recent checkpoints
// For low-memory environments, use MemoryCheckpointer or implement custom checkpointer
let checkpointer = MemoryCheckpointer::new();
```

**Solution 3: Use jemalloc allocator (reduces fragmentation)**
```rust
// In Cargo.toml
[dependencies]
jemallocator = "0.5"

// In main.rs
#[global_allocator]
static GLOBAL: jemallocator::Jemalloc = jemallocator::Jemalloc;
```

---

### Problem: Out of Memory (OOM) Errors

**Symptoms:**
```
Error: Cannot allocate memory
Process killed (signal 9)
```

**Diagnosis:**

Check memory limits and usage:
```bash
# Check system memory
free -h

# Check process limits
ulimit -a

# Check OOM killer logs (Linux)
dmesg | grep -i "out of memory"
```

**Common Causes:**

1. **Unbounded state growth**
2. **Memory leak in node implementation**
3. **Too many parallel branches** (each branch clones state)
4. **Insufficient system memory**

**Solutions:**

**Solution 1: Limit parallel fanout**
```rust
// Before: Unbounded parallelism
graph.add_parallel_edge("start", vec!["worker1", "worker2", /* ... */, "worker100"]);
// Each worker clones state → 100 state copies in memory!

// After: Batch parallelism
graph.add_node_from_fn("batch_process", |state| {
    Box::pin(async move {
        // Process in batches of 10
        for chunk in state.items.chunks(10) {
            let futures: Vec<_> = chunk.iter()
                .map(|item| process_item(item))
                .collect();

            tokio::try_join_all(futures).await?;
        }

        Ok(state)
    })
});
```

**Solution 2: Use external storage for large data**
```rust
use std::fs::File;
use std::io::Write;

#[derive(Clone, Serialize, Deserialize)]
struct ExternalState {
    // Small metadata in state
    result_file: String,
    item_count: usize,

    // Large data stored externally (not in state)
    #[serde(skip)]
    results: Vec<u8>,  // Not serialized
}

impl ExternalState {
    async fn save_results(&self) -> Result<()> {
        let mut file = File::create(&self.result_file)?;
        file.write_all(&self.results)?;
        Ok(())
    }

    async fn load_results(&mut self) -> Result<()> {
        self.results = std::fs::read(&self.result_file)?;
        Ok(())
    }
}
```

**Solution 3: Monitor memory and fail gracefully**
```rust
use sysinfo::{System, SystemExt};

graph.add_node_from_fn("check_memory", |state| {
    Box::pin(async move {
        let mut sys = System::new_all();
        sys.refresh_memory();

        let used_mb = sys.used_memory() / 1024 / 1024;
        let total_mb = sys.total_memory() / 1024 / 1024;
        let usage_percent = (used_mb * 100) / total_mb;

        if usage_percent > 90 {
            return Err(Error::ResourceLimit(
                format!("Memory usage too high: {}%", usage_percent)
            ));
        }

        Ok(state)
    })
});
```

---

## 5. Debugging Techniques

### Technique 1: Enable Logging

**Step 1: Add env_logger**
```rust
// In main.rs or test
use env_logger;

#[tokio::main]
async fn main() {
    // Initialize logger
    env_logger::init();

    // Your code here
    let result = app.invoke(state).await?;
}
```

**Step 2: Add log statements in nodes**
```rust
use log::{info, debug, warn, error};

graph.add_node_from_fn("process", |state| {
    Box::pin(async move {
        debug!("Processing state: {:?}", state);

        match expensive_operation(&state).await {
            Ok(result) => {
                info!("Operation succeeded: {}", result);
                Ok(state.with_result(result))
            }
            Err(e) => {
                error!("Operation failed: {}", e);
                Err(e.into())
            }
        }
    })
});
```

**Step 3: Run with log level**
```bash
# Show all logs
RUST_LOG=debug cargo run --example my_workflow

# Show only errors and warnings
RUST_LOG=warn cargo run --example my_workflow

# Show logs for specific module
RUST_LOG=dashflow=debug cargo run --example my_workflow
```

---

### Technique 2: Use Event Callbacks

**Built-in callback: PrintCallback**
```rust
use dashflow::callbacks::PrintCallback;

let app = graph.compile()?
    .with_callback(PrintCallback);

let result = app.invoke(state).await?;
// Prints: Node started: node_name
// Prints: Node completed: node_name (123 ms)
```

**Custom callback for detailed debugging**
```rust
use dashflow::callbacks::{EventCallback, GraphEvent};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct DebugCallback {
    events: Arc<Mutex<Vec<String>>>,
}

impl EventCallback<MyState> for DebugCallback {
    fn on_event(&self, event: &GraphEvent<MyState>) {
        let mut events = self.events.lock().unwrap();

        match event {
            GraphEvent::NodeStart { node_name } => {
                events.push(format!("START: {}", node_name));
            }
            GraphEvent::NodeEnd { node_name, result, duration_ms } => {
                events.push(format!("END: {} ({} ms) - {:?}",
                    node_name, duration_ms, result.is_ok()));
            }
            GraphEvent::EdgeEval { from, to, condition } => {
                events.push(format!("EDGE: {} -> {} ({})", from, to, condition));
            }
            GraphEvent::CheckpointSaved { thread_id } => {
                events.push(format!("CHECKPOINT SAVED: {}", thread_id));
            }
            GraphEvent::Error { node_name, error } => {
                events.push(format!("ERROR in {}: {}", node_name, error));
            }
        }
    }
}

// Usage
let callback = DebugCallback::default();
let app = graph.compile()?.with_callback(callback.clone());
let result = app.invoke(state).await?;

// Print execution trace
let events = callback.events.lock().unwrap();
for event in events.iter() {
    println!("{}", event);
}
```

---

### Technique 3: Stream Events for Real-Time Debugging

```rust
use dashflow::StreamMode;

let mut stream = app.stream(state, StreamMode::Debug).await?;

while let Some(event) = stream.next().await {
    println!("[DEBUG] {:?}", event);

    // Breakpoint here for step-through debugging
    if event.node_name == "critical_node" {
        println!("State before critical_node: {:?}", event.state);
    }
}
```

---

### Technique 4: Inspect Checkpoints Manually

**For JSON checkpoints:**
```bash
# Pretty-print checkpoint
cat ./checkpoints/session-123.json | jq .

# Extract specific field
cat ./checkpoints/session-123.json | jq '.state.messages'

# Find checkpoints with specific data
grep -r "error" ./checkpoints/*.json
```

**For bincode checkpoints:**
```rust
// Create inspection tool
use dashflow::checkpointer::FileCheckpointer;

#[tokio::main]
async fn main() -> Result<()> {
    let checkpointer = FileCheckpointer::new("./checkpoints")?;
    let checkpoint = checkpointer.load(&"session-123".into()).await?;

    println!("Checkpoint state: {:#?}", checkpoint.state);
    println!("Metadata: {:#?}", checkpoint.metadata);

    Ok(())
}
```

---

### Technique 5: Unit Test Nodes Before Graph Integration

**Test nodes in isolation:**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_process_node() {
        let state = MyState {
            data: "test input".to_string(),
            counter: 0,
        };

        // Call node function directly
        let result = process_node(state).await;

        assert!(result.is_ok());
        let new_state = result.unwrap();
        assert_eq!(new_state.data, "PROCESSED: test input");
        assert_eq!(new_state.counter, 1);
    }

    #[tokio::test]
    async fn test_process_node_error_handling() {
        let state = MyState {
            data: "".to_string(),  // Invalid input
            counter: 0,
        };

        let result = process_node(state).await;
        assert!(result.is_err());
    }
}
```

**Test conditional edges:**
```rust
#[test]
fn test_routing_condition() {
    let state = MyState { score: 0.8 };

    let route = router_condition(&state);
    assert_eq!(route, "high_score_path");

    let state2 = MyState { score: 0.3 };
    let route2 = router_condition(&state2);
    assert_eq!(route2, "low_score_path");
}
```

---

### Technique 6: Use Rust Debugger (lldb/gdb)

```bash
# Build with debug symbols
cargo build --example my_workflow

# Run with debugger
rust-lldb ./target/debug/examples/my_workflow

# Set breakpoint
(lldb) breakpoint set --name my_node_function
(lldb) breakpoint set --file src/main.rs --line 42

# Run
(lldb) run

# Inspect variables
(lldb) print state
(lldb) print state.data

# Step through
(lldb) step
(lldb) next
(lldb) continue
```

---

## 6. Common Pitfalls

### Pitfall 1: Forgetting to Set Entry Point

**Symptom:**
```
Error: Entry point not set
```

**Solution:**
```rust
// WRONG: Missing set_entry_point
let graph = StateGraph::new();
graph.add_node_from_fn("start", start_fn);
let app = graph.compile()?;  // ERROR!

// CORRECT: Always set entry point before compile
let graph = StateGraph::new();
graph.add_node_from_fn("start", start_fn);
graph.set_entry_point("start");  // Required!
let app = graph.compile()?;  // OK
```

---

### Pitfall 2: Using Wrong END Syntax

**Symptom:**
```
Error: Invalid edge target: __end__
```

**Solution:**
```rust
use dashflow::END;

// WRONG: String literal
graph.add_edge("final_node", "__end__");  // ERROR!

// CORRECT: Use END constant
graph.add_edge("final_node", END);  // OK
```

---

### Pitfall 3: State Not Implementing Required Traits

**Symptom:**
```
Error: the trait `GraphState` is not implemented for `MyState`
```

**Solution:**
```rust
use dashflow::GraphState;
use serde::{Serialize, Deserialize};

// WRONG: Missing traits
struct MyState {
    data: String,
}

// CORRECT: Implement all required traits
#[derive(Clone, Serialize, Deserialize)]
struct MyState {
    data: String,
}

impl GraphState for MyState {}
```

---

### Pitfall 4: Blocking Operations in Async Nodes

**Symptom:**
- Poor parallel performance
- Tokio runtime warnings

**Solution:**
```rust
// WRONG: Blocking in async
graph.add_node_from_fn("slow", |state| {
    Box::pin(async move {
        std::thread::sleep(Duration::from_secs(1));  // Blocks runtime!
        Ok(state)
    })
});

// CORRECT: Use async sleep
graph.add_node_from_fn("fast", |state| {
    Box::pin(async move {
        tokio::time::sleep(Duration::from_secs(1)).await;  // Yields to runtime
        Ok(state)
    })
});

// CORRECT: Use spawn_blocking for CPU work
graph.add_node_from_fn("cpu_work", |state| {
    Box::pin(async move {
        let result = tokio::task::spawn_blocking(|| {
            expensive_computation()  // OK to block here
        }).await?;

        Ok(state.with_result(result))
    })
});
```

---

### Pitfall 5: Mutable State References

**Symptom:**
```
Error: cannot borrow `state` as mutable more than once at a time
```

**Solution:**
```rust
// WRONG: Trying to mutate state (state is consumed)
graph.add_node_from_fn("bad", |state| {
    Box::pin(async move {
        state.data.push_str("more");  // Error: state moved
        Ok(state)
    })
});

// CORRECT: Clone and modify
graph.add_node_from_fn("good", |state| {
    Box::pin(async move {
        let mut new_state = state.clone();
        new_state.data.push_str("more");
        Ok(new_state)
    })
});

// BETTER: Use builder pattern
graph.add_node_from_fn("better", |state| {
    Box::pin(async move {
        Ok(state.with_data(format!("{}more", state.data)))
    })
});
```

---

### Pitfall 6: Conditional Edge Returns Wrong Type

**Symptom:**
```
Error: expected `String`, found `&str`
```

**Solution:**
```rust
// WRONG: Returns &str (invalid)
graph.add_conditional_edge("router", |state| {
    if state.score > 0.5 {
        "high"  // Error: &str
    } else {
        "low"
    }
});

// CORRECT: Returns String
graph.add_conditional_edge("router", |state| {
    if state.score > 0.5 {
        "high".to_string()  // OK: String
    } else {
        "low".to_string()
    }
});
```

---

### Pitfall 7: Forgetting async/await

**Symptom:**
```
Error: `impl Future` cannot be sent between threads safely
```

**Solution:**
```rust
// WRONG: Missing .await
graph.add_node_from_fn("bad", |state| {
    Box::pin(async move {
        let result = async_function(state);  // Returns Future, not value!
        Ok(result)  // ERROR!
    })
});

// CORRECT: Add .await
graph.add_node_from_fn("good", |state| {
    Box::pin(async move {
        let result = async_function(state).await?;  // OK: awaited
        Ok(result)
    })
});
```

---

### Pitfall 8: Infinite Loops in Graphs

**Symptom:**
- Execution never completes
- Memory grows continuously
- No error messages

**Solution:**
```rust
// WRONG: Unconditional loop
graph.add_edge("node1", "node2");
graph.add_edge("node2", "node1");  // Infinite loop!

// CORRECT: Add loop exit condition
graph.add_conditional_edge("node1", |state| {
    if state.iterations >= 10 {
        END.to_string()  // Exit after 10 iterations
    } else {
        "node2".to_string()  // Continue loop
    }
});
```

---

### Pitfall 9: Thread ID Mismatch on Resume

**Symptom:**
```
Error: Checkpoint not found
```

**Solution:**
```rust
// WRONG: Different thread IDs
let app1 = graph.compile()?.with_thread_id("session-123");
app1.invoke(state).await?;

let app2 = graph.compile()?.with_thread_id("session-456");  // Different!
app2.resume().await?;  // ERROR: checkpoint not found

// CORRECT: Use same thread ID
let thread_id = "session-123".to_string();
let app1 = graph.compile()?.with_thread_id(thread_id.clone());
app1.invoke(state).await?;

let app2 = graph.compile()?.with_thread_id(thread_id);  // Same!
app2.resume().await?;  // OK
```

---

### Pitfall 10: Debug Build Performance Testing

**Symptom:**
- Execution 10-100x slower than expected
- Benchmarks show terrible performance

**Solution:**
```bash
# WRONG: Testing debug builds
cargo build --example my_workflow
./target/debug/examples/my_workflow  # 10-100x slower!

# CORRECT: Always use release builds for performance
cargo build --release --example my_workflow
./target/release/examples/my_workflow  # Full performance

# Benchmarks automatically use release
cargo bench --package dashflow
```

---

## Additional Resources

- **README.md**: Overview and quick start
- **TUTORIAL.md**: Beginner to advanced guide with examples
- **PERFORMANCE.md**: Benchmarks, optimization techniques, and profiling
- **ARCHITECTURE.md**: System design and implementation details
- **Examples**: `crates/dashflow/examples/` - 69 production-quality examples
- **GitHub Issues**: https://github.com/dropbox/dTOOL/dashflow/issues

---

**Last Updated:** 2026-01-05
**Version:** 1.11.3
**Maintainer:** DashFlow Rust Team
