# Top 10 Software Design and Architecture Gaps

**Focus**: Technical design flaws, not process issues

---

## 1. ❌ **State Management: Mutable Coder with Shared References**

**The Flaw**:
```rust
pub struct Coder {
    current_conversation: Vec<Message>,
    history: Vec<Message>,
    tracked_files: HashSet<PathBuf>,
    model_router: ModelRouter,
    git_repo: Option<Repository>,
}

impl Coder {
    pub async fn send_message(&mut self, message: Message) -> Result<Response> {
        self.current_conversation.push(message);
        // ... model call ...
        self.current_conversation.push(response);
    }
}
```

**Problem**: `&mut self` everywhere means:
- Can't have multiple Coders simultaneously
- Can't share state across threads safely
- Async + mutable state = potential race conditions
- Tools can't safely read state while agent is thinking

**What's Missing**:
An **event-sourced** or **immutable state** design where:
```rust
pub struct CoderState {
    conversation: Arc<[Message]>,  // Immutable
    files: Arc<HashSet<PathBuf>>,
}

pub struct Coder {
    state: Arc<RwLock<CoderState>>,  // Shared, controlled mutation
}

// Tools can read state concurrently
impl Coder {
    pub async fn send_message(&self, message: Message) -> Result<Response> {
        // Create new state rather than mutate
        let new_state = {
            let current = self.state.read().await;
            current.with_message(message)
        };

        *self.state.write().await = new_state;
    }
}
```

**Why It Matters**:
- Parallel tool execution requires shared state
- TUI needs to read state while agent is working
- Multiple agents need independent state

---

## 2. ❌ **Error Handling: Lossy Error Types**

**The Flaw**:
```rust
pub enum Error {
    Config(String),
    Model(String),
    ToolExecution(String),
    Context(String),
    // ...
}
```

**Problems**:
- Errors contain only `String` - lost type information
- Can't programmatically handle specific errors
- Can't retry with different strategy
- Debugging requires parsing strings

**Example Failure**:
```rust
match coder.send_message(msg).await {
    Err(Error::Model(s)) => {
        // Is this a rate limit? Auth error? Network timeout?
        // Can't tell without parsing string
    }
}
```

**What's Missing**:
**Structured error types** with context:
```rust
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Model API error: {source}")]
    Model {
        provider: String,
        model: String,
        #[source]
        source: ModelError,
    },

    #[error("Tool execution failed: {tool}")]
    ToolExecution {
        tool: String,
        params: serde_json::Value,
        #[source]
        source: ToolError,
    },

    #[error("Context size {actual} exceeds limit {max}")]
    ContextTooLarge {
        actual: usize,
        max: usize,
        suggest_compression: bool,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum ModelError {
    #[error("Rate limited, retry after {retry_after:?}")]
    RateLimited { retry_after: Option<Duration> },

    #[error("Authentication failed")]
    AuthenticationFailed,

    #[error("Network timeout after {duration:?}")]
    Timeout { duration: Duration },
}
```

**Why It Matters**:
- Automatic retry logic for rate limits
- Better error messages to user
- Telemetry and debugging
- Programmatic error recovery

---

## 3. ❌ **Context Building: No Dependency Tracking**

**The Flaw**:
```rust
impl ContextManager {
    pub fn build_context(&self, target_tokens: usize) -> Result<String> {
        // Just concatenate files
        for (path, content) in self.tracked_files {
            context.push_str(&format!("File: {}\n{}", path.display(), content));
        }
    }
}
```

**Problems**:
- No understanding of file dependencies
- May include `user.rs` but not `user_repository.rs` that it imports
- No semantic ordering (definitions before usage)
- Truncates arbitrarily, might cut off critical context

**Example Failure**:
```rust
// Context includes:
impl UserService {
    fn create_user(&self, data: UserData) -> Result<User> {
        self.repo.create(data)  // What's self.repo? Not in context!
    }
}
// Missing: UserRepository definition
```

**What's Missing**:
**Dependency-aware context building**:
```rust
pub struct ContextGraph {
    files: HashMap<PathBuf, FileNode>,
}

pub struct FileNode {
    path: PathBuf,
    content: String,
    imports: Vec<PathBuf>,
    symbols_defined: HashSet<String>,
    symbols_used: HashSet<String>,
}

impl ContextGraph {
    pub fn build_context_for(&self, files: &[PathBuf], budget: usize) -> Result<String> {
        // 1. Start with requested files
        let mut to_include = files.to_vec();
        let mut included = HashSet::new();

        // 2. Recursively add dependencies
        while let Some(file) = to_include.pop() {
            if included.contains(&file) {
                continue;
            }

            let node = &self.files[&file];

            // Add imports/dependencies
            for import in &node.imports {
                if !included.contains(import) {
                    to_include.push(import.clone());
                }
            }

            included.insert(file);

            // Check budget
            if self.estimate_tokens(&included) > budget {
                // Prioritize: drop least important files
                break;
            }
        }

        // 3. Order by dependency (definitions first)
        let ordered = self.topological_sort(&included)?;

        Ok(ordered.iter()
            .map(|f| format!("File: {}\n{}", f.display(), self.files[f].content))
            .collect::<Vec<_>>()
            .join("\n\n"))
    }
}
```

**Why It Matters**:
- Claude can't understand code without its dependencies
- Random truncation breaks semantic understanding
- Multi-file edits fail without full context

---

## 4. ❌ **Model Provider: Synchronous Tool Calling**

**The Flaw**:
```rust
#[async_trait]
pub trait ModelProvider {
    async fn complete(&self, request: &CompletionRequest) -> Result<CompletionResponse>;
}

// Usage:
let response = provider.complete(&request).await?;

if let Some(tool_calls) = response.tool_calls {
    for call in tool_calls {
        let result = execute_tool(&call).await?;
    }

    // Make another API call with results
    let response2 = provider.complete(&request_with_results).await?;
}
```

**Problems**:
- Blocking: wait for full response before executing tools
- Sequential: tools execute one by one
- Multiple round-trips to API
- High latency for multi-tool workflows

**Example Failure**:
```
User: "Add tests for user.rs, auth.rs, and repo.rs"

Round 1: API call → returns tool calls [read_file("user.rs"), read_file("auth.rs"), read_file("repo.rs")]
  Execute read_file("user.rs")    - 100ms
  Execute read_file("auth.rs")    - 100ms
  Execute read_file("repo.rs")    - 100ms

Round 2: API call with results → returns more tool calls [write_file("user_test.rs"), ...]
  Execute write_file(...)         - 50ms

Total: 2+ seconds of sequential operations
```

**What's Missing**:
**Streaming with concurrent tool execution**:
```rust
pub trait ModelProvider {
    async fn stream(&self, request: &CompletionRequest) -> Result<ResponseStream>;
}

pub enum StreamEvent {
    TextChunk(String),
    ToolCallStart { id: String, name: String },
    ToolCallParams { id: String, params: serde_json::Value },
    ToolCallComplete { id: String },
    Done,
}

// Usage:
let mut stream = provider.stream(&request).await?;
let mut pending_tools = FuturesUnordered::new();

while let Some(event) = stream.next().await {
    match event {
        StreamEvent::ToolCallComplete { id } => {
            // Execute immediately, don't wait for stream end
            let tool_call = stream.get_tool_call(&id);
            let fut = execute_tool(tool_call);
            pending_tools.push(fut);
        }
        StreamEvent::TextChunk(text) => {
            print!("{}", text);  // Show to user immediately
        }
        _ => {}
    }
}

// Execute tools in parallel
let results: Vec<_> = pending_tools.collect().await;
```

**Why It Matters**:
- 10x latency reduction for multi-tool tasks
- Better UX (streaming text appears immediately)
- Parallel tool execution

---

## 5. ❌ **Tool Interface: No Cancellation or Progress**

**The Flaw**:
```rust
#[async_trait]
pub trait Tool {
    async fn execute(&self, params: serde_json::Value) -> Result<ToolOutput>;
}
```

**Problems**:
- No way to cancel long-running tools
- No progress reporting
- User has no visibility into what's happening
- Can't interrupt if tool goes wrong direction

**Example Failure**:
```rust
// Tool starts searching 1M files
let result = grep_tool.execute(params).await?;

// User realizes they meant different directory
// But can't cancel - must wait for completion
```

**What's Missing**:
**Cancellable tools with progress**:
```rust
pub struct ToolContext {
    cancel_token: CancellationToken,
    progress: ProgressReporter,
}

pub struct ProgressReporter {
    tx: mpsc::Sender<ProgressUpdate>,
}

pub enum ProgressUpdate {
    Started { tool: String },
    Progress { current: u64, total: Option<u64>, message: String },
    Completed { result: ToolOutput },
    Cancelled,
}

#[async_trait]
pub trait Tool {
    async fn execute(
        &self,
        params: serde_json::Value,
        ctx: ToolContext,
    ) -> Result<ToolOutput>;
}

// Implementation:
impl GrepTool {
    async fn execute(&self, params: serde_json::Value, ctx: ToolContext) -> Result<ToolOutput> {
        let pattern = params["pattern"].as_str()?;

        for (i, file) in files.iter().enumerate() {
            // Check cancellation
            if ctx.cancel_token.is_cancelled() {
                return Err(Error::Cancelled);
            }

            // Report progress
            ctx.progress.update(ProgressUpdate::Progress {
                current: i as u64,
                total: Some(files.len() as u64),
                message: format!("Searching {}", file.display()),
            })?;

            // ... actual work ...
        }
    }
}
```

**Why It Matters**:
- User can cancel if tool takes wrong direction
- Progress bar for long operations
- Better UX for expensive tools
- Timeout handling

---

## 6. ❌ **Configuration: No Hot Reload or Validation**

**The Flaw**:
```rust
impl Config {
    pub fn load() -> Result<Self> {
        let path = home_dir()?.join(".ay-coder/config.toml");
        let content = fs::read_to_string(&path)?;
        toml::from_str(&content)
    }
}

// Loaded once at startup, never changes
let config = Config::load()?;
```

**Problems**:
- Must restart to change config
- Invalid config only discovered at startup
- No schema validation
- No config versioning/migration

**Example Failure**:
```toml
[models]
default = "claude-sonnett-4.5"  # Typo - only discovered when used

[context]
max_tokens = 999999999  # Unreasonable - silently accepted
```

**What's Missing**:
**Validated, hot-reloadable configuration**:
```rust
pub struct ConfigManager {
    current: Arc<RwLock<Config>>,
    watcher: notify::RecommendedWatcher,
    validator: ConfigValidator,
}

pub struct ConfigValidator {
    schema: serde_json::Value,
}

impl ConfigValidator {
    pub fn validate(&self, config: &Config) -> Result<Vec<Warning>> {
        let mut warnings = Vec::new();

        // Validate model names
        if !KNOWN_MODELS.contains(&config.models.default) {
            warnings.push(Warning::UnknownModel {
                name: config.models.default.clone(),
                suggestion: did_you_mean(&config.models.default, &KNOWN_MODELS),
            });
        }

        // Validate token limits
        if config.context.max_tokens > 2_000_000 {
            warnings.push(Warning::UnreasonableValue {
                field: "context.max_tokens",
                value: config.context.max_tokens,
                recommended_max: 2_000_000,
            });
        }

        Ok(warnings)
    }
}

impl ConfigManager {
    pub async fn new() -> Result<Self> {
        let config = Self::load_and_validate()?;

        let (tx, mut rx) = mpsc::channel(1);

        let mut watcher = notify::recommended_watcher(move |res| {
            tx.blocking_send(res).ok();
        })?;

        watcher.watch(config_path(), RecursiveMode::NonRecursive)?;

        // Background task for hot reload
        tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                if let Ok(new_config) = Self::load_and_validate() {
                    *self.current.write().await = new_config;
                    println!("Config reloaded");
                }
            }
        });

        Ok(Self { current, watcher, validator })
    }
}
```

**Why It Matters**:
- Iterate on config without restarting
- Catch typos immediately
- Migrations when config format changes

---

## 7. ❌ **Edit Application: No Conflict Detection**

**The Flaw**:
```rust
impl Coder {
    pub async fn apply_edits(&mut self, edits: Vec<Edit>) -> Result<()> {
        for edit in edits {
            let content = fs::read_to_string(&edit.path)?;
            let new_content = apply_edit(&content, &edit)?;
            fs::write(&edit.path, new_content)?;
        }
    }
}
```

**Problems**:
- No detection of conflicting edits
- No detection of file changes between read and write
- No atomic multi-file edits
- Lost updates possible

**Example Failure**:
```
Time 0: Read user.rs (version A)
Time 1: User manually edits user.rs (now version B)
Time 2: Agent writes user.rs based on version A
Result: User's manual edits are lost
```

**What's Missing**:
**Three-way merge with conflict detection**:
```rust
pub struct EditApplication {
    workspace: PathBuf,
}

pub enum ApplyResult {
    Success { written: Vec<PathBuf> },
    Conflicts { conflicts: Vec<Conflict> },
    NeedsReview { diffs: Vec<FileDiff> },
}

pub struct Conflict {
    path: PathBuf,
    original: String,
    expected: String,
    actual: String,
}

impl EditApplication {
    pub fn apply_edits(&self, edits: Vec<Edit>) -> Result<ApplyResult> {
        let mut conflicts = Vec::new();
        let mut diffs = Vec::new();

        // Phase 1: Check for conflicts
        for edit in &edits {
            let current = fs::read_to_string(&edit.path)?;

            // Did file change since we read it?
            if current != edit.original_content {
                // Three-way merge
                let merged = merge3(
                    &edit.original_content,  // Base
                    &current,                // Theirs (user changes)
                    &edit.new_content,       // Ours (AI changes)
                )?;

                if merged.has_conflicts() {
                    conflicts.push(Conflict {
                        path: edit.path.clone(),
                        original: edit.original_content.clone(),
                        expected: edit.new_content.clone(),
                        actual: current,
                    });
                }
            }

            // Generate diff for review
            let diff = create_diff(&edit.original_content, &edit.new_content);
            diffs.push(FileDiff {
                path: edit.path.clone(),
                diff,
            });
        }

        if !conflicts.is_empty() {
            return Ok(ApplyResult::Conflicts { conflicts });
        }

        // Phase 2: Show diffs and ask for approval
        if !self.auto_apply {
            return Ok(ApplyResult::NeedsReview { diffs });
        }

        // Phase 3: Apply all edits atomically
        let mut written = Vec::new();
        for edit in edits {
            fs::write(&edit.path, &edit.new_content)?;
            written.push(edit.path);
        }

        Ok(ApplyResult::Success { written })
    }
}
```

**Why It Matters**:
- Don't lose user's manual edits
- Safe concurrent editing
- Atomic multi-file changes

---

## 8. ❌ **Conversation State: No Branching or Forking**

**The Flaw**:
```rust
pub struct Coder {
    current_conversation: Vec<Message>,  // Linear history
}
```

**Problems**:
- Can't explore multiple approaches
- Can't backtrack to earlier point
- Can't compare different solutions
- All-or-nothing conversation

**Example Failure**:
```
User: "Refactor this to use async"
Agent: [Makes changes]
User: "Actually, I want to try the synchronous approach too"
Agent: ❌ Can't go back to pre-refactor state
       ❌ Would have to manually undo
```

**What's Missing**:
**Tree-structured conversation with branches**:
```rust
pub struct ConversationTree {
    root: NodeId,
    nodes: HashMap<NodeId, ConversationNode>,
    current: NodeId,
}

pub struct ConversationNode {
    id: NodeId,
    parent: Option<NodeId>,
    message: Message,
    response: Option<Response>,
    children: Vec<NodeId>,  // Branches
    file_state: HashMap<PathBuf, FileVersion>,
}

impl ConversationTree {
    pub fn branch_from(&mut self, node_id: NodeId, message: Message) -> NodeId {
        let new_node = ConversationNode {
            id: NodeId::new(),
            parent: Some(node_id),
            message,
            response: None,
            children: Vec::new(),
            file_state: self.nodes[&node_id].file_state.clone(),
        };

        let new_id = new_node.id;
        self.nodes[&node_id].children.push(new_id);
        self.nodes.insert(new_id, new_node);
        new_id
    }

    pub fn switch_to_branch(&mut self, node_id: NodeId) -> Result<()> {
        // Restore file state from that branch
        let node = &self.nodes[&node_id];

        for (path, version) in &node.file_state {
            fs::write(path, &version.content)?;
        }

        self.current = node_id;
        Ok(())
    }

    pub fn compare_branches(&self, a: NodeId, b: NodeId) -> BranchDiff {
        let state_a = &self.nodes[&a].file_state;
        let state_b = &self.nodes[&b].file_state;

        BranchDiff::from_states(state_a, state_b)
    }
}
```

**Why It Matters**:
- Explore multiple solutions
- Easy backtracking
- A/B test different approaches
- Better for experimentation

---

## 9. ❌ **Memory Management: Unbounded Context Growth**

**The Flaw**:
```rust
pub struct Coder {
    current_conversation: Vec<Message>,  // Grows unbounded
    tracked_files: HashSet<PathBuf>,     // Grows unbounded
}
```

**Problems**:
- Memory leaks in long sessions
- Context eventually exceeds model limits
- No LRU eviction
- No compression strategy

**Example Failure**:
```rust
// After 1000 messages:
coder.current_conversation.len() == 1000;  // ~10MB
coder.tracked_files.len() == 500;          // ~50MB

// Context building takes 30+ seconds
// Most messages are old and irrelevant
```

**What's Missing**:
**Bounded context with smart eviction**:
```rust
pub struct BoundedContext {
    recent: VecDeque<Message>,           // Last N messages
    max_recent: usize,
    summarized: Option<String>,          // Compressed old messages
    files: LruCache<PathBuf, FileContent>,
}

impl BoundedContext {
    pub fn add_message(&mut self, msg: Message) {
        self.recent.push_back(msg);

        if self.recent.len() > self.max_recent {
            // Compress old messages
            let to_compress: Vec<_> = self.recent
                .drain(..self.max_recent / 2)
                .collect();

            let summary = summarize_messages(&to_compress);

            self.summarized = Some(match self.summarized.take() {
                Some(old) => format!("{}\n\n{}", old, summary),
                None => summary,
            });
        }
    }

    pub fn build_context(&self) -> String {
        let mut ctx = String::new();

        // Old compressed context
        if let Some(summary) = &self.summarized {
            ctx.push_str("Previous conversation:\n");
            ctx.push_str(summary);
            ctx.push_str("\n\n---\n\n");
        }

        // Recent messages (full detail)
        for msg in &self.recent {
            ctx.push_str(&format!("{}: {}\n\n", msg.role, msg.content));
        }

        ctx
    }
}
```

**Why It Matters**:
- Long sessions don't leak memory
- Context building stays fast
- Token limits never exceeded
- Graceful degradation

---

## 10. ❌ **Testing Strategy: No Hermetic Test Mode**

**The Flaw**:
```rust
#[tokio::test]
async fn test_coder() {
    let coder = Coder::new(None)?;  // Makes real API calls!

    let response = coder.send_message(Message::user("test")).await?;

    assert_eq!(response.content, "expected");  // Flaky: depends on API
}
```

**Problems**:
- Tests make real API calls (expensive, slow, flaky)
- No deterministic testing
- Can't test error conditions
- Integration tests are slow

**What's Missing**:
**Injectable dependencies with mock mode**:
```rust
pub trait ModelClient: Send + Sync {
    async fn complete(&self, request: &CompletionRequest) -> Result<CompletionResponse>;
}

pub struct AnthropicClient {
    api_key: String,
    client: reqwest::Client,
}

impl ModelClient for AnthropicClient {
    async fn complete(&self, request: &CompletionRequest) -> Result<CompletionResponse> {
        // Real API call
    }
}

pub struct MockModelClient {
    responses: Vec<CompletionResponse>,
    calls: Arc<Mutex<Vec<CompletionRequest>>>,
}

impl ModelClient for MockModelClient {
    async fn complete(&self, request: &CompletionRequest) -> Result<CompletionResponse> {
        self.calls.lock().unwrap().push(request.clone());
        Ok(self.responses.remove(0))
    }
}

pub struct Coder {
    model: Arc<dyn ModelClient>,  // Injected dependency
}

#[tokio::test]
async fn test_coder() {
    let mock = MockModelClient::new(vec![
        CompletionResponse {
            content: "I'll help you with that".into(),
            tool_calls: Some(vec![
                ToolCall { name: "read_file", params: json!({"path": "test.rs"}) }
            ]),
        },
    ]);

    let coder = Coder::with_client(Arc::new(mock.clone()));

    let response = coder.send_message(Message::user("test")).await?;

    // Deterministic assertions
    assert_eq!(response.content, "I'll help you with that");

    // Verify API was called correctly
    let calls = mock.calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].messages[0].content, "test");
}
```

**Why It Matters**:
- Fast, deterministic tests
- Can test error handling
- No API costs in CI
- Property-based testing possible

---

## Summary: Critical Design Issues

All 10 gaps are **actual software architecture problems**:

1. **State management**: Mutable state prevents concurrency
2. **Error types**: String errors lose information
3. **Context building**: No dependency tracking
4. **Model interface**: Synchronous, not streaming
5. **Tool interface**: No cancellation or progress
6. **Configuration**: No validation or hot reload
7. **Edit application**: No conflict detection
8. **Conversation state**: Linear, not tree-structured
9. **Memory management**: Unbounded growth
10. **Testing**: No dependency injection

Each would cause real problems in production. These aren't "nice to haves" - they're **fundamental design flaws** that will bite us.
