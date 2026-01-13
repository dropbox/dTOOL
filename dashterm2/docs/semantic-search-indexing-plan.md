# Progressive Semantic Search Indexing

A resource-conscious strategy for semantic search that starts fast and improves continuously without consuming system resources.

## Design Philosophy

1. **Never block the user** - all indexing happens incrementally or in background
2. **Good enough immediately** - use fast heuristics that work from doc #1
3. **Improve continuously** - every operation leaves the index slightly better
4. **Respect resources** - small work chunks, bounded CPU/memory usage
5. **No big rebuilds** - quality converges asymptotically through many small updates

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                      SEMANTIC SEARCH SYSTEM                     │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐         │
│  │ File Watcher│───▶│  Embedder   │───▶│ Lazy Index  │         │
│  │ (debounced) │    │ (background)│    │ (adaptive)  │         │
│  └─────────────┘    └─────────────┘    └──────┬──────┘         │
│                                               │                 │
│                                               ▼                 │
│                                        ┌─────────────┐         │
│                                        │   Search    │         │
│                                        │   + Improve │         │
│                                        └─────────────┘         │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

## Multi-Vector Search (XTR-WARP Style)

This system uses **multi-vector retrieval** where each document has multiple embeddings (one per token). Scoring uses **MaxSim**:

```
score(query, doc) = mean(max(similarity(q_token, d_token) for d_token in doc) for q_token in query)
```

This provides better semantic matching than single-vector approaches but requires more sophisticated indexing.

## Progressive Clustering Strategy

### Phase Transitions

```
Time 0:        LSH assignments (instant, ~70% quality)
                    ↓ automatic transition at 1K docs
Time 1 min:    Online k-means (fast, ~85% quality)
                    ↓ continuous improvement
Time 10 min:   Refined clusters (~95% quality)
                    ↓ asymptotic convergence
Time 1 hour+:  Near-optimal (~98% quality)
```

### Phase 1: Locality-Sensitive Hashing (Instant Start)

Random hyperplanes provide instant "clustering" with no training:

```rust
struct LSHIndex {
    hyperplanes: Tensor,  // [num_bits, embedding_dim]
    num_bits: usize,
}

impl LSHIndex {
    fn new(embedding_dim: usize, num_bits: usize) -> Self {
        // Random unit vectors as hyperplanes
        let hyperplanes = Tensor::randn(0.0, 1.0, (num_bits, embedding_dim), &Device::Cpu)
            .l2_normalize();
        Self { hyperplanes, num_bits }
    }

    fn hash(&self, embedding: &Tensor) -> u32 {
        // Which side of each hyperplane? -> bit vector -> bucket ID
        let dots = embedding.matmul(&self.hyperplanes.t());
        let bits: Vec<bool> = dots.gt(0.0).to_vec1();

        let mut bucket = 0u32;
        for (i, b) in bits.iter().enumerate() {
            if *b { bucket |= 1 << i; }
        }
        bucket
    }
}
```

**Characteristics:**
- O(d) assignment, no iteration needed
- Works immediately with zero warmup
- ~70% quality compared to optimal k-means
- Transitions to learned clusters as data accumulates

### Phase 2: Online K-Means (Continuous Improvement)

Centers update incrementally with each new document:

```rust
struct OnlineClusters {
    centers: Vec<Tensor>,
    counts: Vec<usize>,
}

impl OnlineClusters {
    fn add(&mut self, embedding: &Tensor) -> usize {
        let cluster = self.nearest(embedding);

        // Incremental mean update (Welford's algorithm)
        self.counts[cluster] += 1;
        let n = self.counts[cluster] as f32;
        self.centers[cluster] = (
            &self.centers[cluster] * ((n - 1.0) / n) +
            embedding * (1.0 / n)
        ).l2_normalize();

        cluster
    }

    fn nearest(&self, embedding: &Tensor) -> usize {
        self.centers
            .iter()
            .enumerate()
            .map(|(i, c)| (i, embedding.dot(c)))
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .unwrap().0
    }
}
```

**Quality progression:**
```
After 100 docs:   ~90%
After 1000 docs:  ~97%
After 10000 docs: ~99%
```

## The Lazy Index

Core data structure that improves incrementally:

```rust
struct LazyIndex {
    // Clustering
    centers: Vec<Tensor>,
    counts: Vec<usize>,

    // Storage: buckets of (doc_id, quantized_residual)
    buckets: Vec<Vec<(u32, Vec<u8>)>>,

    // For reservoir sampling (eventual optimization)
    reservoir: Vec<Tensor>,
    reservoir_size: usize,
    total_seen: usize,

    // Resource management
    budget: ResourceBudget,
}

struct ResourceBudget {
    max_work_per_add_ms: u64,     // e.g., 1ms
    max_work_per_search_ms: u64,  // e.g., 5ms
    max_idle_work_ms: u64,        // e.g., 50ms
    max_memory_mb: usize,         // e.g., 500MB
}
```

### Core Operations

#### Add Document

```rust
impl LazyIndex {
    fn add(&mut self, doc_id: u32, embedding: &Tensor) {
        // 1. Find nearest cluster - O(k)
        let cluster = self.nearest(embedding);

        // 2. Compute and store residual
        let residual = embedding - &self.centers[cluster];
        let quantized = residual.compand().quantize(4).to_bytes();
        self.buckets[cluster].push((doc_id, quantized));
        self.counts[cluster] += 1;

        // 3. Reservoir sample for future optimization
        self.reservoir_sample(embedding);

        // 4. Tiny incremental improvement
        self.improve_center(cluster, embedding);

        // 5. Maybe structural change (rare, amortized)
        self.maybe_split_or_merge(cluster);
    }

    fn improve_center(&mut self, cluster: usize, embedding: &Tensor) {
        // Incremental mean update - O(d)
        let n = self.counts[cluster] as f32;
        self.centers[cluster] = (
            &self.centers[cluster] * ((n - 1.0) / n) +
            embedding * (1.0 / n)
        ).l2_normalize();
    }

    fn reservoir_sample(&mut self, embedding: &Tensor) {
        self.total_seen += 1;

        if self.reservoir.len() < self.reservoir_size {
            self.reservoir.push(embedding.clone());
        } else {
            // Reservoir sampling: random replacement
            let j = rand::thread_rng().gen_range(0..self.total_seen);
            if j < self.reservoir_size {
                self.reservoir[j] = embedding.clone();
            }
        }
    }

    fn maybe_split_or_merge(&mut self, cluster: usize) {
        // Split large clusters (probabilistic to amortize cost)
        if self.counts[cluster] > 10000 && rand::random::<f32>() < 0.01 {
            self.split_cluster(cluster);
        }

        // Merge small clusters (even more rare)
        if self.counts[cluster] < 50 && rand::random::<f32>() < 0.001 {
            self.merge_smallest_clusters();
        }
    }
}
```

#### Search with Piggyback Improvement

```rust
impl LazyIndex {
    fn search(&mut self, query: &Tensor, top_k: usize) -> Vec<(f32, u32)> {
        // 1. Find top-k clusters to search
        let cluster_scores = query.matmul(&self.centers_matrix().t());
        let top_clusters = cluster_scores.topk(32);

        // 2. Search selected buckets
        let mut candidates = vec![];
        for cluster in top_clusters {
            let center = &self.centers[cluster];
            for (doc_id, residual) in &self.buckets[cluster] {
                let reconstructed = center + dequantize(residual);
                let score = maxsim(query, &reconstructed);
                candidates.push((score, *doc_id));
            }
        }

        // 3. Piggyback: improve one random cluster
        self.improve_random_cluster();

        // 4. Return top-k
        candidates.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        candidates.truncate(top_k);
        candidates
    }

    fn improve_random_cluster(&mut self) {
        let cluster = rand::random::<usize>() % self.centers.len();

        // Recompute center from sample of bucket contents
        let sample: Vec<_> = self.buckets[cluster]
            .iter()
            .choose_multiple(&mut rand::thread_rng(), 100);

        if sample.len() >= 10 {
            let center = &self.centers[cluster];
            let reconstructed: Vec<Tensor> = sample
                .iter()
                .map(|(_, r)| center + dequantize(r))
                .collect();

            let new_center = reconstructed
                .iter()
                .fold(Tensor::zeros_like(center), |a, b| a + b)
                .l2_normalize();

            // Blend: don't jump too fast
            self.centers[cluster] = (
                &self.centers[cluster] * 0.9 + &new_center * 0.1
            ).l2_normalize();
        }
    }
}
```

### Cluster Management

#### Splitting Large Clusters

```rust
impl LazyIndex {
    fn split_cluster(&mut self, cluster: usize) {
        // Generate split direction (random hyperplane through center)
        let direction = Tensor::randn_like(&self.centers[cluster]).l2_normalize();

        // Create new cluster with perturbed center
        let new_center = (&self.centers[cluster] + &direction * 0.1).l2_normalize();
        let new_cluster = self.centers.len();

        self.centers.push(new_center);
        self.counts.push(0);
        self.buckets.push(vec![]);

        // Lazy reassignment: docs migrate on next access/update
        // Don't do expensive bulk reassignment now
    }
}
```

#### Merging Small Clusters

```rust
impl LazyIndex {
    fn merge_smallest_clusters(&mut self) {
        // Find two smallest adjacent clusters
        let (c1, c2) = self.find_smallest_pair();

        // Merge c2 into c1
        let total = (self.counts[c1] + self.counts[c2]) as f32;
        let w1 = self.counts[c1] as f32 / total;
        let w2 = self.counts[c2] as f32 / total;

        self.centers[c1] = (
            &self.centers[c1] * w1 + &self.centers[c2] * w2
        ).l2_normalize();

        // Move docs from c2 to c1
        let docs = std::mem::take(&mut self.buckets[c2]);
        self.buckets[c1].extend(docs);
        self.counts[c1] += self.counts[c2];

        // Mark c2 as empty (or remove and reindex)
        self.counts[c2] = 0;
    }
}
```

## Background Worker

```rust
struct IndexWorker {
    index: Arc<RwLock<LazyIndex>>,
    embedder: Embedder,
    rx: Receiver<IndexJob>,
}

enum IndexJob {
    Embed { doc_id: u32, path: PathBuf, content: String },
    ImproveCluster { cluster: usize },
    DeepOptimize,
}

impl IndexWorker {
    fn run(&mut self) {
        let mut last_activity = Instant::now();

        loop {
            match self.rx.recv_timeout(Duration::from_secs(1)) {
                Ok(IndexJob::Embed { doc_id, path, content }) => {
                    // Generate embedding
                    let embedding = self.embedder.embed(&content);

                    // Add to index (includes incremental improvement)
                    self.index.write().unwrap().add(doc_id, &embedding);

                    last_activity = Instant::now();
                }

                Ok(IndexJob::ImproveCluster { cluster }) => {
                    self.index.write().unwrap().improve_cluster(cluster);
                }

                Ok(IndexJob::DeepOptimize) => {
                    self.deep_optimize();
                }

                Err(RecvTimeoutError::Timeout) => {
                    // Idle time - do background improvement
                    let idle_duration = last_activity.elapsed();

                    if idle_duration > Duration::from_secs(300) {
                        // Very idle: deeper optimization
                        self.medium_optimize();
                    } else if idle_duration > Duration::from_secs(30) {
                        // Somewhat idle: improve worst cluster
                        self.improve_worst_cluster();
                    }
                }

                Err(RecvTimeoutError::Disconnected) => break,
            }
        }
    }

    fn improve_worst_cluster(&mut self) {
        let mut index = self.index.write().unwrap();

        // Find cluster with worst health
        let worst = (0..index.centers.len())
            .max_by_key(|&c| OrderedFloat(index.cluster_health(c)))
            .unwrap();

        index.improve_cluster(worst);
    }

    fn medium_optimize(&mut self) {
        let mut index = self.index.write().unwrap();

        // Improve several clusters
        for _ in 0..10 {
            index.improve_random_cluster();
        }

        // Check for structural issues
        index.rebalance_if_needed();
    }

    fn deep_optimize(&mut self) {
        let mut index = self.index.write().unwrap();

        // Use reservoir to recompute better centers
        if index.reservoir.len() >= index.centers.len() * 10 {
            let k = index.centers.len();
            let data = Tensor::stack(&index.reservoir, 0);

            // Mini-batch k-means on reservoir (not full rebuild)
            for _ in 0..3 {
                let assignments = data.matmul(&index.centers_matrix().t()).argmax(-1);

                for c in 0..k {
                    let mask = assignments.eq(c as u32);
                    let cluster_data = data.masked_select(&mask);
                    if cluster_data.len() > 0 {
                        let new_center = cluster_data.mean(0).l2_normalize();
                        index.centers[c] = (
                            &index.centers[c] * 0.7 + &new_center * 0.3
                        ).l2_normalize();
                    }
                }
            }
        }
    }
}
```

## File Watcher Integration

```rust
fn watch_directory(path: &Path, tx: Sender<IndexJob>) -> Result<()> {
    let (watcher_tx, watcher_rx) = channel();
    let mut watcher = notify::recommended_watcher(watcher_tx)?;
    watcher.watch(path, RecursiveMode::Recursive)?;

    // Debounce map: path -> last change time
    let mut debounce: HashMap<PathBuf, Instant> = HashMap::new();
    let debounce_ms = 500;

    loop {
        // Collect file system events
        match watcher_rx.recv_timeout(Duration::from_millis(100)) {
            Ok(Ok(Event { kind, paths, .. })) => {
                match kind {
                    EventKind::Modify(_) | EventKind::Create(_) => {
                        for path in paths {
                            if should_index(&path) {
                                debounce.insert(path, Instant::now());
                            }
                        }
                    }
                    EventKind::Remove(_) => {
                        for path in paths {
                            // Mark for removal from index
                            tx.send(IndexJob::Remove { path });
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }

        // Process debounced changes
        let now = Instant::now();
        let ready: Vec<PathBuf> = debounce
            .iter()
            .filter(|(_, t)| now.duration_since(**t).as_millis() > debounce_ms)
            .map(|(p, _)| p.clone())
            .collect();

        for path in ready {
            debounce.remove(&path);

            if let Ok(content) = fs::read_to_string(&path) {
                let doc_id = path_to_doc_id(&path);
                tx.send(IndexJob::Embed { doc_id, path, content })?;
            }
        }
    }
}

fn should_index(path: &Path) -> bool {
    // Index code files, skip binaries/generated
    let dominated_extensions = ["rs", "ts", "js", "py", "go", "java", "c", "cpp", "h", "md"];

    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| allowed_extensions.contains(&e))
        .unwrap_or(false)
    && !path.components().any(|c| {
        let s = c.as_os_str().to_str().unwrap_or("");
        s == "node_modules" || s == "target" || s == ".git" || s == "build"
    })
}
```

## Cluster Health Metrics

```rust
impl LazyIndex {
    /// Returns 0.0 (perfect) to 1.0 (random)
    fn cluster_health(&self, cluster: usize) -> f32 {
        if self.buckets[cluster].len() < 10 {
            return 0.5; // Not enough data to judge
        }

        // Sample bucket contents
        let sample: Vec<_> = self.buckets[cluster]
            .iter()
            .choose_multiple(&mut rand::thread_rng(), 50);

        // Compute average distance from center
        let center = &self.centers[cluster];
        let avg_dist: f32 = sample
            .iter()
            .map(|(_, residual)| {
                let reconstructed = center + dequantize(residual);
                1.0 - reconstructed.dot(center)  // cosine distance
            })
            .sum::<f32>() / sample.len() as f32;

        avg_dist
    }

    fn overall_health(&self) -> f32 {
        let total: f32 = self.centers
            .iter()
            .enumerate()
            .map(|(i, _)| self.cluster_health(i) * self.counts[i] as f32)
            .sum();

        total / self.total_seen as f32
    }

    fn needs_work(&self) -> bool {
        self.overall_health() > 0.2
    }

    fn needs_structural_change(&self) -> bool {
        // Check for imbalanced clusters
        let max_count = self.counts.iter().max().unwrap_or(&0);
        let min_count = self.counts.iter().filter(|&&c| c > 0).min().unwrap_or(&0);

        *max_count > *min_count * 100  // 100x imbalance
    }
}
```

## Resource Budgets

```rust
struct ResourceBudget {
    // Per-operation limits
    max_work_per_add_ms: u64,
    max_work_per_search_ms: u64,

    // Background work limits
    max_idle_work_ms: u64,
    max_deep_work_ms: u64,

    // Memory limits
    max_memory_mb: usize,
    max_reservoir_size: usize,

    // Thresholds
    idle_threshold_secs: u64,
    deep_idle_threshold_secs: u64,
}

impl Default for ResourceBudget {
    fn default() -> Self {
        Self {
            max_work_per_add_ms: 1,
            max_work_per_search_ms: 5,
            max_idle_work_ms: 50,
            max_deep_work_ms: 500,
            max_memory_mb: 500,
            max_reservoir_size: 10000,
            idle_threshold_secs: 30,
            deep_idle_threshold_secs: 300,
        }
    }
}
```

## Performance Characteristics

| Operation | Time | CPU | Quality Impact |
|-----------|------|-----|----------------|
| Add document | <1ms | O(k) | +0.01% |
| Search | 10-50ms | O(k×b) | - |
| Search + improve | +1-5ms | +O(d) | +0.1% |
| Idle tick | ~50ms | O(d×100) | +0.5% |
| Deep optimize | ~500ms | O(n×k×3) | +2% |

Where:
- k = number of clusters
- b = average bucket size
- d = embedding dimension (128)
- n = reservoir size

## Quality vs Time Trade-off

```
Quality ≈ 0.70 + 0.28 × (1 - e^(-docs_seen / 1000))

Time 0:           70% (LSH baseline)
After 100 docs:   88%
After 1000 docs:  95%
After 10000 docs: 98%
Asymptote:        ~98% (limited by clustering, not iteration)
```

## Integration Points

### For Coding Agent (Dasher)

```rust
// On file save
file_watcher.on_change(|path| {
    indexer.queue(IndexJob::Embed { path, content });
});

// Before sending to LLM
let context = index.search(&user_query, top_k: 10);
let relevant_files = context.iter().map(|(_, doc_id)| get_file(doc_id));
prompt.add_context(relevant_files);
```

### For Terminal (DashTerm)

```rust
// On command execution
terminal.on_command(|cmd, output| {
    let doc = format!("$ {}\n{}", cmd, output);
    indexer.queue(IndexJob::Embed { doc_id: cmd_id, content: doc });
});

// On history search
let results = index.search(&query, top_k: 20);
show_history_matches(results);
```

## Future Optimizations

### Algorithmic Improvements
1. **Product Quantization** - Better compression than scalar 4-bit
2. **HNSW for navigation** - Faster cluster selection for very large k
3. **Learned routing** - Train a small network to predict best clusters
4. **Tiered storage** - Hot/warm/cold with different update strategies
5. **Distributed** - Shard by cluster for multi-core search

### Runtime Backends (from [xtr-warp](https://github.com/jlscheerer/xtr-warp))

The original Stanford/Google XTR-WARP implementation supports multiple inference backends that could be ported:

| Backend | Platform | Benefit |
|---------|----------|---------|
| **ONNX Runtime** | Cross-platform | Portable, optimized inference |
| **OpenVINO** | Intel CPUs | 2-3x faster on Intel hardware |
| **CoreML** | macOS/iOS | Native Apple Silicon optimization |
| **CUDA** | NVIDIA GPUs | Fast indexing on Linux servers |
| **TensorRT** | NVIDIA GPUs | Optimized inference |

**Implementation path:**
```rust
enum InferenceBackend {
    Candle,      // Current: pure Rust, Metal on macOS
    Onnx,        // Future: ort crate, cross-platform
    CoreML,      // Future: Native macOS, better than Metal for some ops
    OpenVino,    // Future: Intel optimization
    Cuda,        // Future: Linux GPU support via candle-cuda or ort
}

impl Embedder {
    fn new(backend: InferenceBackend) -> Self {
        match backend {
            InferenceBackend::Candle => Self::new_candle(),
            InferenceBackend::Onnx => Self::new_onnx(),
            // ...
        }
    }
}
```

**Priority:**
1. ONNX Runtime - Best cross-platform story, single export works everywhere
2. CUDA (via ONNX) - Enables fast indexing on Linux GPU servers
3. CoreML - Better macOS integration than raw Metal
4. OpenVINO - Only if targeting Intel-heavy deployments

### Code-Specific Model Fine-tuning

The base XTR model is trained on general text. For better code search:

1. **Fine-tune on CodeSearchNet** - Query → code retrieval pairs
2. **Add code-specific tokens** - `[FUNC]`, `[CLASS]`, `[VAR]`
3. **Language-aware tokenization** - Preserve identifier boundaries
4. **Multi-language support** - Separate adapters per language

See: [CodeSearchNet dataset](https://github.com/github/CodeSearchNet)

### References

- Original XTR-WARP: https://github.com/jlscheerer/xtr-warp
- XTR Paper (NeurIPS 2023): https://arxiv.org/abs/2304.01982
- WARP Paper (SIGIR 2025): https://arxiv.org/abs/2501.17788
- ColBERTv2: https://arxiv.org/abs/2112.01488

---

# Part 2: Batteries-Included Integration

## The `sg` Command (Super Grep)

A semantic grep that understands meaning, not just text:

```bash
# Traditional grep - exact matches only
$ grep "handle auth" -r .
(nothing)

# Super grep - understands intent
$ sg "handle auth"
src/auth/login.ts:42      validateCredentials(user, password)
src/middleware/session.ts:18  checkSessionToken(req)
src/api/oauth.ts:7        initiateOAuthFlow(provider)
```

### Usage

```bash
# Basic semantic search in current project
$ sg "error handling"

# Search specific directory
$ sg "database connection" --in ~/code/myapp

# Search with file type filter
$ sg "parse json" --type ts

# Hybrid mode: semantic + ripgrep
$ sg "auth" --hybrid

# Show index status
$ sg --status

# Force index a directory
$ sg --add ~/code/newproject

# Control the daemon
$ sg --daemon status
$ sg --daemon stop
$ sg --daemon restart
```

## Architecture: Daemon + DashTerm Integration

```
┌─────────────────────────────────────────────────────────────────────┐
│                         USER'S MACHINE                              │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐              │
│  │  DashTerm    │  │  DashTerm    │  │  DashTerm    │   Terminals  │
│  │  Instance 1  │  │  Instance 2  │  │  Instance 3  │              │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘              │
│         │                 │                 │                       │
│         │    Unix Socket / IPC              │                       │
│         └─────────────────┼─────────────────┘                       │
│                           ▼                                         │
│              ┌────────────────────────┐                             │
│              │    dashterm-indexer    │  ← Single daemon            │
│              │    (background daemon) │                             │
│              ├────────────────────────┤                             │
│              │  • File watcher        │                             │
│              │  • Embedding engine    │                             │
│              │  • Index manager       │                             │
│              │  • Search server       │                             │
│              └───────────┬────────────┘                             │
│                          │                                          │
│                          ▼                                          │
│              ┌────────────────────────┐                             │
│              │  ~/.local/share/       │                             │
│              │    dashterm/semantic/  │  ← Storage                  │
│              │    ├── index.sqlite    │                             │
│              │    ├── model.bin       │                             │
│              │    └── daemon.sock     │                             │
│              └────────────────────────┘                             │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### Why a Daemon?

| Concern | In-Process | Daemon |
|---------|------------|--------|
| Multiple terminals | Each has own indexer (wasteful) | Shared (efficient) |
| Terminal closes | Indexing stops | Continues |
| Resource control | Per-process | System-wide |
| Index consistency | Risk of conflicts | Single writer |
| Model loading | Each loads 100MB | Loaded once |
| Initial index | Restarts if terminal closes | Runs to completion |

### Daemon Lifecycle

```rust
// dashterm-indexer daemon

fn main() {
    // Check if already running
    if let Some(pid) = read_pidfile() {
        if process_exists(pid) {
            eprintln!("Daemon already running (pid {})", pid);
            exit(1);
        }
    }

    // Daemonize
    daemonize();
    write_pidfile();

    // Set up socket
    let socket = UnixListener::bind(SOCKET_PATH)?;

    // Initialize
    let indexer = SemanticIndexer::new()?;

    // Main loop
    loop {
        select! {
            // Handle client connections
            client = socket.accept() => {
                handle_client(client, &indexer);
            }

            // Handle file system events
            event = indexer.file_watcher.next() => {
                indexer.handle_fs_event(event);
            }

            // Periodic tasks
            _ = interval(Duration::from_secs(60)) => {
                indexer.do_background_work();
                indexer.gc_if_needed();
            }

            // Shutdown signal
            _ = shutdown_signal() => {
                indexer.shutdown();
                break;
            }
        }
    }
}
```

### DashTerm Integration

```rust
// In DashTerm startup

impl DashTerm {
    fn init_semantic_search(&mut self) -> Result<()> {
        // Ensure daemon is running
        if !daemon_is_running() {
            self.start_daemon()?;
        }

        // Connect to daemon
        self.indexer_client = IndexerClient::connect(SOCKET_PATH)?;

        // Register current directory for indexing
        let cwd = std::env::current_dir()?;
        self.indexer_client.watch(&cwd)?;

        Ok(())
    }

    fn start_daemon(&self) -> Result<()> {
        // Spawn daemon process
        Command::new("dashterm-indexer")
            .arg("--daemon")
            .spawn()?;

        // Wait for socket to be ready
        for _ in 0..50 {
            if Path::new(SOCKET_PATH).exists() {
                return Ok(());
            }
            thread::sleep(Duration::from_millis(100));
        }

        Err(anyhow!("Daemon failed to start"))
    }

    fn on_directory_change(&mut self, new_cwd: &Path) {
        // Tell daemon we moved
        self.indexer_client.watch(new_cwd);
    }
}
```

### IPC Protocol

Simple JSON-RPC over Unix socket:

```rust
// Request types
enum Request {
    // Indexing
    Watch { path: PathBuf },
    Unwatch { path: PathBuf },
    ForceIndex { path: PathBuf },

    // Search
    Search { query: String, options: SearchOptions },

    // Status
    Status,
    ProjectStatus { path: PathBuf },

    // Control
    Pause,
    Resume,
    Shutdown,
}

// Response types
enum Response {
    Ok,
    SearchResults(Vec<SearchResult>),
    Status(DaemonStatus),
    Error(String),
}

// Wire format
// Request:  { "id": 1, "method": "search", "params": { "query": "auth", ... } }
// Response: { "id": 1, "result": { "hits": [...] } }
```

### Client Library

```rust
// Used by DashTerm, `sg` CLI, and agents

pub struct IndexerClient {
    socket: UnixStream,
    next_id: AtomicU64,
}

impl IndexerClient {
    pub fn connect(path: &Path) -> Result<Self> {
        let socket = UnixStream::connect(path)?;
        Ok(Self { socket, next_id: AtomicU64::new(1) })
    }

    pub fn search(&self, query: &str, options: SearchOptions) -> Result<Vec<SearchResult>> {
        let req = Request::Search {
            query: query.to_string(),
            options
        };
        self.call(req)
    }

    pub fn watch(&self, path: &Path) -> Result<()> {
        self.call(Request::Watch { path: path.to_path_buf() })
    }

    pub fn status(&self) -> Result<DaemonStatus> {
        self.call(Request::Status)
    }
}
```

## Smart Directory Detection

### Auto-Discovery

```rust
impl SemanticIndexer {
    fn discover_projects(&self) -> Vec<ProjectRoot> {
        let mut roots = vec![];

        // 1. From shell history (where user actually works)
        if let Ok(history) = self.load_shell_history() {
            for entry in history.iter().rev().take(5000) {
                if let Some(cwd) = &entry.cwd {
                    if let Some(root) = find_project_root(cwd) {
                        roots.push(ProjectRoot {
                            path: root,
                            reason: DiscoveryReason::ShellHistory,
                            last_accessed: entry.timestamp,
                        });
                    }
                }
            }
        }

        // 2. Common code directories
        let code_dirs = ["~/code", "~/projects", "~/src", "~/work", "~/dev", "~/repos"];
        for dir in code_dirs {
            if let Ok(expanded) = shellexpand::tilde(dir).into_owned() {
                self.scan_for_projects(&expanded, 2, &mut roots);
            }
        }

        // 3. Dedupe and sort by recency
        roots.sort_by_key(|r| std::cmp::Reverse(r.last_accessed));
        roots.dedup_by_key(|r| r.path.clone());

        roots
    }

    fn scan_for_projects(&self, dir: &Path, max_depth: usize, roots: &mut Vec<ProjectRoot>) {
        let markers = [
            ".git", "package.json", "Cargo.toml", "go.mod",
            "pyproject.toml", "Makefile", "pom.xml", "build.gradle"
        ];

        for entry in WalkDir::new(dir).max_depth(max_depth) {
            if let Ok(entry) = entry {
                for marker in &markers {
                    if entry.path().join(marker).exists() {
                        roots.push(ProjectRoot {
                            path: entry.path().to_path_buf(),
                            reason: DiscoveryReason::ProjectMarker(marker.to_string()),
                            last_accessed: entry.metadata()?.modified()?,
                        });
                        break;
                    }
                }
            }
        }
    }
}

fn find_project_root(path: &Path) -> Option<PathBuf> {
    let markers = [".git", "package.json", "Cargo.toml", "go.mod", "pyproject.toml"];

    let mut current = path.to_path_buf();
    while let Some(parent) = current.parent() {
        for marker in &markers {
            if current.join(marker).exists() {
                return Some(current);
            }
        }
        current = parent.to_path_buf();
    }
    None
}
```

### Priority-Based Indexing

```rust
impl SemanticIndexer {
    fn index_project(&mut self, root: &Path) {
        // Priority order for indexing
        let phases = [
            // Phase 1: Entry points and docs (index first, most valuable)
            vec!["README*", "readme*", "*.md", "docs/**/*.md"],
            vec!["src/main.*", "src/lib.*", "src/index.*", "index.*", "main.*"],
            vec!["src/app.*", "app.*", "src/mod.*"],

            // Phase 2: Source code
            vec!["src/**/*", "lib/**/*", "app/**/*", "pkg/**/*"],

            // Phase 3: Tests and config
            vec!["test/**/*", "tests/**/*", "spec/**/*", "__tests__/**/*"],
            vec!["*.toml", "*.json", "*.yaml", "*.yml"],

            // Phase 4: Everything else
            vec!["**/*"],
        ];

        for (priority, patterns) in phases.iter().enumerate() {
            for pattern in patterns {
                for path in glob(root, pattern) {
                    if self.should_index(&path) {
                        self.queue_file(path, priority);
                    }
                }
            }
        }
    }

    fn should_index(&self, path: &Path) -> bool {
        // Skip patterns
        let skip = ["node_modules", "target", ".git", "vendor", "dist",
                    "build", "__pycache__", ".venv", "venv"];

        if path.components().any(|c| skip.contains(&c.as_os_str().to_str().unwrap_or(""))) {
            return false;
        }

        // File size limit
        if let Ok(meta) = path.metadata() {
            if meta.len() > 100 * 1024 {  // 100KB
                return false;
            }
        }

        // Extension filter
        let allowed = ["rs", "ts", "tsx", "js", "jsx", "py", "go", "java",
                       "c", "cpp", "h", "hpp", "rb", "swift", "kt", "md", "txt"];

        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| allowed.contains(&e))
            .unwrap_or(false)
    }
}
```

## Resource Management

### Consumption Targets

| State | CPU | GPU | When |
|-------|-----|-----|------|
| Idle | 0% | 0% | No pending work |
| Background (user active) | 1-2% | 5% | User typing/working |
| Background (user idle 30s) | 10% | 20% | User paused |
| Background (user away 5m) | 50% | 50% | User AFK |
| Search query | Burst | Burst | <100ms then done |

### Throttling Implementation

```rust
struct ResourceThrottler {
    last_user_activity: Instant,
    embedding_budget: TokenBucket,
}

impl ResourceThrottler {
    fn get_current_limits(&self) -> ResourceLimits {
        let idle_duration = self.last_user_activity.elapsed();

        match idle_duration {
            d if d < Duration::from_secs(5) => ResourceLimits {
                // User very active - minimal work
                files_per_second: 0.5,
                cpu_percent: 2,
                gpu_percent: 5,
            },
            d if d < Duration::from_secs(30) => ResourceLimits {
                // User somewhat active
                files_per_second: 2.0,
                cpu_percent: 5,
                gpu_percent: 10,
            },
            d if d < Duration::from_secs(300) => ResourceLimits {
                // User idle
                files_per_second: 10.0,
                cpu_percent: 20,
                gpu_percent: 30,
            },
            _ => ResourceLimits {
                // User away
                files_per_second: 50.0,
                cpu_percent: 50,
                gpu_percent: 50,
            },
        }
    }

    fn on_user_activity(&mut self) {
        self.last_user_activity = Instant::now();
    }
}
```

### Storage Limits

```rust
struct StorageManager {
    base_path: PathBuf,
    max_total_mb: usize,
    max_per_project_mb: usize,
}

impl StorageManager {
    fn enforce_limits(&mut self, indexer: &mut SemanticIndexer) {
        let total = self.total_size_mb();

        if total > self.max_total_mb {
            // Evict least-recently-used projects
            let mut projects: Vec<_> = indexer.projects.iter().collect();
            projects.sort_by_key(|p| p.last_accessed);

            for project in projects {
                if self.total_size_mb() < self.max_total_mb * 80 / 100 {
                    break;
                }
                indexer.evict_project(&project.path);
            }
        }
    }

    fn total_size_mb(&self) -> usize {
        WalkDir::new(&self.base_path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter_map(|e| e.metadata().ok())
            .map(|m| m.len() as usize)
            .sum::<usize>() / (1024 * 1024)
    }
}
```

## Storage Layout

```
~/.local/share/dashterm/semantic/
├── daemon.pid                    # Daemon process ID
├── daemon.sock                   # Unix socket for IPC
├── config.toml                   # User config (optional)
├── model/
│   └── xtr-quantized.bin         # Model weights (~100MB)
├── index/
│   ├── main.sqlite               # Main index database
│   └── embeddings/               # Large blob storage (optional)
│       ├── project_abc123.bin
│       └── project_def456.bin
└── cache/
    └── query_cache.sqlite        # Recent query cache
```

### SQLite Schema

```sql
-- Projects being tracked
CREATE TABLE project (
    id INTEGER PRIMARY KEY,
    path TEXT UNIQUE NOT NULL,
    name TEXT,
    discovered_via TEXT,          -- 'shell_history', 'manual', 'scan'
    first_indexed_at INTEGER,
    last_accessed_at INTEGER,
    total_files INTEGER,
    index_quality REAL            -- 0.0 to 1.0
);

-- Indexed files
CREATE TABLE file (
    id INTEGER PRIMARY KEY,
    project_id INTEGER REFERENCES project(id),
    path TEXT NOT NULL,
    hash TEXT,                    -- content hash for change detection
    indexed_at INTEGER,
    size_bytes INTEGER,
    UNIQUE(project_id, path)
);

-- Embeddings (or stored in separate .bin files for large projects)
CREATE TABLE embedding (
    id INTEGER PRIMARY KEY,
    file_id INTEGER REFERENCES file(id),
    chunk_start INTEGER,          -- byte offset
    chunk_end INTEGER,
    embedding BLOB                -- quantized embedding
);

-- Cluster index
CREATE TABLE cluster (
    id INTEGER PRIMARY KEY,
    project_id INTEGER REFERENCES project(id),
    generation INTEGER,
    center BLOB
);

CREATE TABLE bucket (
    cluster_id INTEGER REFERENCES cluster(id),
    file_id INTEGER,
    residual BLOB
);

-- Indexes
CREATE INDEX idx_file_project ON file(project_id);
CREATE INDEX idx_file_path ON file(path);
CREATE INDEX idx_embedding_file ON embedding(file_id);
CREATE INDEX idx_bucket_cluster ON bucket(cluster_id);
```

## Agent Integration

```rust
// Tool definition for coding agents

pub fn semantic_search_tool() -> Tool {
    Tool::new("semantic_search")
        .description(
            "Search code semantically by meaning, not just keywords. \
             Use this to find code related to a concept even if it uses \
             different terminology. More powerful than grep for understanding \
             'where is X handled' or 'code related to Y'."
        )
        .param("query", ParamType::String, "What to search for (natural language)")
        .param("scope", ParamType::String, "Directory path or 'all' for everything")
        .param("limit", ParamType::Int, "Max results (default 10)")
        .handler(|params, ctx| {
            let client = ctx.get::<IndexerClient>()?;

            let results = client.search(
                &params.query,
                SearchOptions {
                    root: params.scope.as_deref(),
                    top_k: params.limit.unwrap_or(10),
                    hybrid: true,
                }
            )?;

            // Format for agent consumption
            let mut output = String::new();
            for (i, hit) in results.iter().enumerate() {
                writeln!(&mut output, "{}. {} ({}% match)",
                    i + 1,
                    hit.path.display(),
                    (hit.score * 100.0) as i32
                )?;
                writeln!(&mut output, "   Line {}: {}", hit.line, hit.snippet.trim())?;
            }

            Ok(output)
        })
}
```

## Status & Monitoring

```bash
$ sg --status

╭─────────────────────────────────────────────────────────────────╮
│                    Semantic Search Status                       │
╰─────────────────────────────────────────────────────────────────╯

Daemon:     Running (pid 12345, uptime 2d 4h)
Socket:     ~/.local/share/dashterm/semantic/daemon.sock
Storage:    847 MB / 2048 MB (41%)
Model:      Loaded (XTR-base, 198 MB RAM)

Indexed Projects:
  ● ~/code/dashterm2      2,847 files   142 MB   active      98%
  ● ~/code/webapp         1,203 files    61 MB   3h ago      95%
  ○ ~/code/scripts          89 files     4 MB   2d ago      91%
  ○ ~/code/old-project     412 files    21 MB   14d ago     87%

Activity:
  Indexing:    Idle
  Queue:       0 files
  Last file:   src/main.rs (4 min ago)

Resources (last hour):
  CPU:  avg 1.8%, peak 18%
  GPU:  avg 2.1%, peak 31%
  I/O:  12 MB read, 3 MB written

$ sg --status --json  # For programmatic access
```

## Configuration

```toml
# ~/.config/dashterm/semantic.toml
# All settings are optional - sensible defaults are built in

[daemon]
# Socket location (default: auto)
socket = "~/.local/share/dashterm/semantic/daemon.sock"

# Start on login (default: false, DashTerm starts it on-demand)
autostart = false

[resources]
# CPU limits
max_cpu_active = 5        # While user is active (%)
max_cpu_idle = 50         # While user is away (%)

# Storage limits
max_total_mb = 2048       # Total index storage
max_per_project_mb = 500  # Per-project limit

# Memory
max_ram_mb = 512          # RAM limit for daemon

[indexing]
# File filters
max_file_size_kb = 100
skip_dirs = ["node_modules", "target", ".git", "vendor", "dist", "build"]
extensions = ["rs", "ts", "js", "py", "go", "java", "c", "cpp", "h", "md"]

# Timing
idle_threshold_secs = 30       # When to speed up indexing
stale_project_days = 30        # When to evict unused projects

[projects]
# Always index these
always = ["~/code/important-project"]

# Never index these
never = ["~/code/vendor", "~/code/archive"]

# Auto-discover from these locations
discover_paths = ["~/code", "~/projects", "~/src"]
discover_depth = 2
```

## Summary

| Component | What | Where |
|-----------|------|-------|
| **dashterm-indexer** | Background daemon | Single process, started by DashTerm |
| **IndexerClient** | IPC library | Used by DashTerm, `sg`, agents |
| **sg** | CLI command | Shell alias or binary |
| **index.sqlite** | Storage | `~/.local/share/dashterm/semantic/` |
| **model.bin** | XTR weights | Loaded once by daemon |

The user experience:
1. Install DashTerm → daemon starts automatically
2. `cd ~/code/myproject` → project gets indexed in background
3. `sg "handle auth"` → instant semantic search results
4. Agent uses semantic search tool → finds relevant code automatically
5. User never configures anything → it just works

## Summary

The key principles:

1. **Start with LSH** - Instant, good-enough clustering from document #1
2. **Improve on every operation** - Add updates centers, search improves random cluster
3. **Use idle time wisely** - Background worker does deeper optimization when user is idle
4. **Never block** - All heavy work is async, all sync work is O(1) or O(k)
5. **Bound resources** - Explicit budgets for CPU, memory, and time per operation
6. **Measure health** - Track cluster quality, only do work when needed
7. **Converge asymptotically** - Quality improves forever, never needs full rebuild
