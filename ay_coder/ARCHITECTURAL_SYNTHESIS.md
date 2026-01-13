# AY Coder Architectural Synthesis
## Combining Best Patterns from 5 Leading Open-Source AI Coding Systems

**Date**: November 19, 2025
**Purpose**: Synthesize architectural patterns from Codex, Aider, Continue.dev, OpenHands, and Plandex into actionable design for AY Coder (Rust)

---

## Executive Summary

After analyzing 5 major open-source AI coding systems (751M of source code), we've identified the core architectural patterns that will form the foundation of AY Coder. This document synthesizes findings into **concrete, implementable Rust patterns**.

### Framework Overview

| Framework | Size | Language | Key Strength | Our Priority |
|-----------|------|----------|--------------|--------------|
| **Codex** | 64M | Rust 96% | Native Rust patterns, MCP, sandboxing | 🔥 HIGH - Direct reference |
| **Aider** | 141M | Python 80% | CLI pair programming, Git, edit strategies | 🔥 HIGH - CLI patterns |
| **Continue.dev** | 451M | TypeScript 83% | CLI+TUI+IDE unified architecture | 🔥 HIGH - Hybrid design |
| **OpenHands** | 35M | Python 77% | Multi-agent orchestration, event-driven | 🔥 MEDIUM - Enterprise patterns |
| **Plandex** | 60M | Go 93% | 2M token context handling | 🔥 MEDIUM - Large context |

---

## Part 1: Core Architectural Decisions

### 1.1 Foundation Architecture

**Decision**: Library-first Rust workspace with multiple interfaces

**Pattern from**: Codex (Rust workspace), Continue.dev (core + interfaces)

```rust
// AY Coder structure
ay_coder/
├── crates/
│   ├── core/              # Library-first design (Codex pattern)
│   │   ├── agent/         # ReAct loop
│   │   ├── models/        # Multi-model abstraction
│   │   ├── tools/         # MCP + built-in tools
│   │   ├── context/       # Context management
│   │   ├── sandbox/       # Platform-specific sandboxing
│   │   └── config/        # Configuration
│   │
│   ├── cli/               # CLI interface (Aider pattern)
│   ├── tui/               # TUI interface (Continue.dev pattern)
│   └── mcp-server/        # MCP server (Codex pattern)
```

**Rationale**:
- Codex proves Rust workspace scales well
- Continue.dev shows how to share core across interfaces
- Aider demonstrates mature CLI design
- Separating core library enables multiple frontends

---

### 1.2 Coder/Agent Abstraction

**Pattern from**: Aider (Python) → Translated to Rust

**Aider's Pattern**:
```python
class Coder:
    def __init__(self, main_model, io):
        self.cur_messages = []     # Current conversation
        self.done_messages = []    # Completed history
        self.abs_fnames = set()    # Tracked files

    def send(self, messages, model=None, functions=None):
        # Handle model interaction, streaming, token tracking
        pass

    def apply_updates(self):
        # Get edits, validate, apply, optional auto-commit
        pass
```

**AY Coder Translation**:
```rust
pub struct Coder {
    current_conversation: Vec<Message>,
    history: Vec<Message>,
    tracked_files: HashSet<PathBuf>,
    model_router: ModelRouter,
    git_repo: Option<Repository>,
}

impl Coder {
    pub async fn send_message(
        &mut self,
        message: Message,
        tools: Option<Vec<Tool>>,
    ) -> Result<Response> {
        // Streaming, token tracking, cost calculation
        let response = self.model_router
            .complete_stream(&self.current_conversation, tools)
            .await?;

        self.current_conversation.push(message);
        self.current_conversation.push(response.clone());

        Ok(response)
    }

    pub async fn apply_edits(
        &mut self,
        edits: Vec<Edit>,
    ) -> Result<Vec<PathBuf>> {
        // Validate, apply, track changes
        let edited_files = self.apply_and_validate(edits)?;

        // Auto-commit if configured
        if self.config.auto_commit {
            self.auto_commit(&edited_files)?;
        }

        Ok(edited_files)
    }
}
```

**Key Improvements**:
- Rust's ownership prevents stale file references
- `async/await` for non-blocking model calls
- Type safety for edits and messages
- Zero-cost abstractions

---

### 1.3 Multi-Model Router

**Pattern from**: Aider (Python multi-LLM), OpenHands (LLM Registry)

**OpenHands Pattern**:
```python
class LLMRegistry:
    _registry: dict[str, type[LLM]] = {}

    @classmethod
    def register(cls, name: str, llm_cls: type[LLM]):
        cls._registry[name] = llm_cls

    @classmethod
    def get_llm(cls, name: str, config: LLMConfig):
        llm_cls = cls._registry[name]
        return llm_cls(config)
```

**AY Coder Translation**:
```rust
use async_trait::async_trait;

#[async_trait]
pub trait ModelProvider: Send + Sync {
    async fn complete(&self, request: &CompletionRequest) -> Result<CompletionResponse>;
    async fn stream(&self, request: &CompletionRequest) -> Result<CompletionStream>;
    fn capabilities(&self) -> ModelCapabilities;
    fn name(&self) -> &str;
}

pub struct ModelRouter {
    providers: HashMap<String, Arc<dyn ModelProvider>>,
    default: String,
    routing_strategy: RoutingStrategy,
}

impl ModelRouter {
    pub async fn complete_stream(
        &self,
        messages: &[Message],
        tools: Option<Vec<Tool>>,
    ) -> Result<CompletionStream> {
        // Select provider based on strategy
        let provider = self.select_provider(messages)?;

        let request = CompletionRequest {
            messages: messages.to_vec(),
            tools,
            max_tokens: Some(4096),
            stream: true,
        };

        provider.stream(&request).await
    }

    fn select_provider(&self, messages: &[Message]) -> Result<&Arc<dyn ModelProvider>> {
        match self.routing_strategy {
            RoutingStrategy::Explicit(ref name) => {
                self.providers.get(name)
                    .ok_or(Error::ProviderNotFound(name.clone()))
            }
            RoutingStrategy::TaskBased => {
                // Analyze task complexity and select optimal model
                let complexity = analyze_complexity(messages);
                self.providers.get(&self.select_by_complexity(complexity))
                    .ok_or(Error::NoSuitableProvider)
            }
            // ... other strategies
        }
    }
}

// Concrete implementations
pub struct AnthropicProvider {
    client: reqwest::Client,
    api_key: String,
}

#[async_trait]
impl ModelProvider for AnthropicProvider {
    async fn complete(&self, request: &CompletionRequest) -> Result<CompletionResponse> {
        let response = self.client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .json(&request)
            .send()
            .await?
            .json()
            .await?;

        Ok(response)
    }

    fn capabilities(&self) -> ModelCapabilities {
        ModelCapabilities {
            max_context_length: 200_000,
            supports_function_calling: true,
            supports_streaming: true,
            supports_vision: true,
        }
    }

    fn name(&self) -> &str {
        "anthropic"
    }
}
```

---

### 1.4 Context Management (Large Context)

**Pattern from**: Plandex (Go - 2M tokens), Continue.dev (TypeScript)

**Plandex Pattern**:
```go
func AutoLoadContextFiles(ctx context.Context, files []string) (string, error) {
    var totalSize int64

    for _, file := range files {
        if totalSize > shared.MaxTotalContextSize {
            log.Println("Skipping file - max context body size exceeded")
            break
        }

        content := readFile(file)
        totalSize += len(content)
    }

    if res.TotalTokens > res.MaxTokens {
        overage := res.TotalTokens - res.MaxTokens
        return "", fmt.Errorf("exceeded token limit by %d tokens", overage)
    }
}
```

**AY Coder Translation**:
```rust
pub struct ContextManager {
    workspace: Workspace,
    tracked_files: LruCache<PathBuf, FileContent>,
    conversation: Vec<Message>,
    instructions: InstructionHierarchy,
    max_tokens: usize,
}

impl ContextManager {
    pub fn build_context(&self, target_tokens: usize) -> Result<String> {
        let mut context = String::new();
        let mut token_count = 0;

        // Layer 1: System prompt (always included)
        let system = self.system_prompt();
        token_count += estimate_tokens(&system);
        context.push_str(&system);

        // Layer 2: Hierarchical instructions (AGENTS.md files)
        let instructions = self.instructions.compile();
        token_count += estimate_tokens(&instructions);
        context.push_str(&instructions);

        // Layer 3: Workspace structure (summary)
        let workspace_summary = self.workspace.summarize();
        token_count += estimate_tokens(&workspace_summary);
        context.push_str(&workspace_summary);

        // Layer 4: Relevant files (prioritized by recency and relevance)
        for (path, content) in self.prioritize_files() {
            let file_tokens = estimate_tokens(&content.text);

            if token_count + file_tokens > target_tokens * 60 / 100 {
                break; // Reserve 40% for conversation
            }

            context.push_str(&format!("\nFile: {}\n{}\n", path.display(), content.text));
            token_count += file_tokens;
        }

        // Layer 5: Conversation (with compression if needed)
        let remaining_budget = target_tokens.saturating_sub(token_count);
        let conversation = self.build_conversation_context(remaining_budget)?;
        context.push_str(&conversation);

        // Validate total
        let final_count = estimate_tokens(&context);
        if final_count > target_tokens {
            return Err(Error::ContextTooLarge {
                actual: final_count,
                max: target_tokens,
            });
        }

        Ok(context)
    }

    fn build_conversation_context(&self, budget: usize) -> Result<String> {
        let mut messages = self.conversation.clone();
        let mut total_tokens = 0;

        // Keep last N messages as-is
        let keep_recent = 10;
        let recent = &messages[messages.len().saturating_sub(keep_recent)..];

        for msg in recent.iter().rev() {
            total_tokens += estimate_tokens(&msg.content);
        }

        // Summarize older messages if we exceed budget
        if total_tokens > budget && messages.len() > keep_recent {
            let to_summarize = &messages[..messages.len() - keep_recent];
            let summary = self.summarize_messages(to_summarize)?;

            let mut result = format!("Previous conversation summary:\n{}\n\n", summary);

            for msg in recent {
                result.push_str(&format!("{}: {}\n\n", msg.role, msg.content));
            }

            Ok(result)
        } else {
            // All messages fit
            Ok(messages.iter()
                .map(|m| format!("{}: {}\n\n", m.role, m.content))
                .collect())
        }
    }

    fn prioritize_files(&self) -> Vec<(PathBuf, &FileContent)> {
        // LRU cache automatically prioritizes recently accessed files
        self.tracked_files
            .iter()
            .map(|(path, content)| (path.clone(), content))
            .collect()
    }
}

// Token estimation (simple heuristic, can be improved)
fn estimate_tokens(text: &str) -> usize {
    // Rough estimate: 1 token ≈ 4 characters
    text.len() / 4
}
```

**Key Features**:
- Plandex's strict token limit enforcement
- Continue.dev's layered context building
- LRU cache for file prioritization
- Budget allocation (60% files, 40% conversation)
- Automatic compression when needed

---

### 1.5 Git Integration

**Pattern from**: Aider (Python - best-in-class Git)

**Aider Pattern**:
```python
def auto_commit(self, edited_files):
    # Stage files
    for fname in edited_files:
        self.repo.repo.git.add(fname)

    # Generate commit message
    commit_message = self.generate_commit_message(edited_files)

    # Commit
    self.repo.commit(message=commit_message)
```

**AY Coder Translation**:
```rust
use git2::{Repository, Signature, IndexAddOption};

pub struct GitIntegration {
    repo: Repository,
    auto_commit: bool,
}

impl GitIntegration {
    pub fn auto_commit(&self, edited_files: &[PathBuf]) -> Result<()> {
        if !self.auto_commit {
            return Ok(());
        }

        // Stage files
        let mut index = self.repo.index()?;
        for file in edited_files {
            index.add_path(file)?;
        }
        index.write()?;

        // Generate commit message
        let message = self.generate_commit_message(edited_files)?;

        // Create commit
        let tree_id = index.write_tree()?;
        let tree = self.repo.find_tree(tree_id)?;
        let parent = self.repo.head()?.peel_to_commit()?;

        let sig = Signature::now("AY Coder", "noreply@ay-coder.dev")?;

        self.repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            &message,
            &tree,
            &[&parent],
        )?;

        Ok(())
    }

    fn generate_commit_message(&self, files: &[PathBuf]) -> Result<String> {
        // Simple strategy: list files changed
        let files_list = files.iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(", ");

        Ok(format!(
            "AI-assisted edit: {}\n\n\
             Files changed:\n{}\n\n\
             🤖 Generated with AY Coder",
            files_list,
            files.iter()
                .map(|p| format!("  - {}", p.display()))
                .collect::<Vec<_>>()
                .join("\n")
        ))
    }

    pub fn track_changes(&self) -> Result<Vec<PathBuf>> {
        let mut changed = Vec::new();
        let statuses = self.repo.statuses(None)?;

        for entry in statuses.iter() {
            if let Some(path) = entry.path() {
                changed.push(PathBuf::from(path));
            }
        }

        Ok(changed)
    }
}
```

---

### 1.6 Tool System with MCP

**Pattern from**: Codex (Rust MCP), OpenHands (Plugin architecture)

**OpenHands Plugin Pattern**:
```python
ALL_PLUGINS = {
    'jupyter': JupyterPlugin,
    'agent_skills': AgentSkillsPlugin,
    'vscode': VSCodePlugin,
}

class Plugin:
    def initialize(self, runtime):
        pass
```

**AY Coder Translation**:
```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameter_schema(&self) -> serde_json::Value;
    async fn execute(&self, params: serde_json::Value) -> Result<ToolOutput>;
    fn requires_permission(&self) -> PermissionLevel;
    fn supports_parallel_execution(&self) -> bool {
        true
    }
}

pub struct ToolRegistry {
    builtin: HashMap<String, Arc<dyn Tool>>,
    mcp_clients: Vec<McpClient>,
}

impl ToolRegistry {
    pub async fn execute_tool(
        &self,
        name: &str,
        params: serde_json::Value,
    ) -> Result<ToolOutput> {
        // Try built-in first
        if let Some(tool) = self.builtin.get(name) {
            return tool.execute(params).await;
        }

        // Try MCP servers
        for client in &self.mcp_clients {
            if client.has_tool(name) {
                return client.call_tool(name, params).await;
            }
        }

        Err(Error::ToolNotFound(name.to_string()))
    }

    pub async fn execute_parallel(
        &self,
        calls: Vec<ToolCall>,
    ) -> Result<Vec<ToolOutput>> {
        let futures: Vec<_> = calls.into_iter()
            .map(|call| self.execute_tool(&call.name, call.params))
            .collect();

        futures::future::try_join_all(futures).await
    }
}

// Built-in tool example
pub struct ReadFileTool;

#[async_trait]
impl Tool for ReadFileTool {
    fn name(&self) -> &str {
        "read_file"
    }

    fn description(&self) -> &str {
        "Read the contents of a file"
    }

    fn parameter_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file"
                }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, params: serde_json::Value) -> Result<ToolOutput> {
        let path: PathBuf = serde_json::from_value(params["path"].clone())?;
        let content = tokio::fs::read_to_string(&path).await?;

        Ok(ToolOutput {
            success: true,
            data: json!({ "content": content }),
            error: None,
        })
    }

    fn requires_permission(&self) -> PermissionLevel {
        PermissionLevel::None // Read-only
    }
}
```

---

### 1.7 Multi-Agent Orchestration

**Pattern from**: OpenHands (Python delegation)

**OpenHands Pattern**:
```python
async def start_delegate(self, action: AgentDelegateAction) -> None:
    delegate_agent = agent_cls(config=agent_config)
    self.delegate = AgentController(
        agent=delegate_agent,
        event_stream=self.event_stream,
        state=derived_state_from_parent
    )
```

**AY Coder Translation** (Future enhancement):
```rust
pub struct AgentController {
    agent: Box<dyn Agent>,
    state: AgentState,
    delegate: Option<Box<AgentController>>,
}

impl AgentController {
    pub async fn delegate_task(
        &mut self,
        task: Task,
        agent_type: AgentType,
    ) -> Result<TaskResult> {
        // Create sub-agent
        let sub_agent = AgentFactory::create(agent_type, &self.state)?;

        let mut sub_controller = AgentController {
            agent: sub_agent,
            state: self.state.derive_for_subtask(&task),
            delegate: None,
        };

        // Execute delegated task
        let result = sub_controller.execute(task).await?;

        // Merge results back into parent state
        self.state.merge_subtask_result(&result);

        Ok(result)
    }
}
```

---

## Part 2: CLI + TUI Architecture

### 2.1 Unified Core with Multiple Interfaces

**Pattern from**: Continue.dev (TypeScript)

**Structure**:
```
ay_coder/
├── crates/
│   ├── core/          # Shared business logic
│   ├── cli/           # CLI commands
│   └── tui/           # Terminal UI
```

**CLI Implementation**:
```rust
// crates/cli/src/main.rs
use ay_coder_core::Coder;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "ay-coder")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start interactive coding session
    Run {
        /// Initial prompt
        prompt: Option<String>,

        /// Model to use
        #[arg(short, long)]
        model: Option<String>,
    },

    /// Execute single command
    Exec {
        /// Command to execute
        prompt: String,
    },

    /// Initialize configuration
    Init,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Run { prompt, model } => {
            let mut coder = Coder::new(model)?;

            if let Some(p) = prompt {
                // Non-interactive: execute and exit
                coder.send_message(Message::user(p)).await?;
            } else {
                // Interactive: REPL loop
                run_interactive(coder).await?;
            }
        }
        Commands::Exec { prompt } => {
            let mut coder = Coder::new(None)?;
            coder.send_message(Message::user(prompt)).await?;
        }
        Commands::Init => {
            Config::initialize_default()?;
        }
    }

    Ok(())
}

async fn run_interactive(mut coder: Coder) -> Result<()> {
    use rustyline::Editor;

    let mut rl = Editor::<()>::new()?;

    loop {
        let readline = rl.readline("ay> ");
        match readline {
            Ok(line) => {
                rl.add_history_entry(&line);

                if line.trim() == "exit" {
                    break;
                }

                let response = coder.send_message(Message::user(line)).await?;
                println!("{}", response.content);
            }
            Err(_) => break,
        }
    }

    Ok(())
}
```

**TUI Implementation**:
```rust
// crates/tui/src/main.rs
use ratatui::{
    backend::CrosstermBackend,
    Terminal,
};

#[tokio::main]
async fn main() -> Result<()> {
    let mut terminal = Terminal::new(CrosstermBackend::new(std::io::stdout()))?;

    let mut app = App::new()?;

    loop {
        terminal.draw(|f| ui(f, &app))?;

        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Char('q') => break,
                KeyCode::Enter => {
                    app.send_message().await?;
                }
                KeyCode::Char(c) => {
                    app.input.push(c);
                }
                KeyCode::Backspace => {
                    app.input.pop();
                }
                _ => {}
            }
        }
    }

    Ok(())
}
```

---

## Part 3: Key Implementation Priorities

### Phase 1: Core Foundation (Weeks 1-4)
```rust
// Implement these first
ay_coder_core::models::ModelRouter
ay_coder_core::tools::ToolRegistry
ay_coder_core::context::ContextManager
ay_coder_core::config::Config
```

### Phase 2: Agent Loop (Weeks 5-6)
```rust
ay_coder_core::agent::Coder
ay_coder_core::agent::ReActLoop
```

### Phase 3: Git + CLI (Weeks 7-8)
```rust
ay_coder_core::git::GitIntegration
ay_coder_cli::main
```

### Phase 4: Sandboxing (Weeks 9-10)
```rust
ay_coder_core::sandbox::SandboxManager
// Platform-specific implementations
```

### Phase 5: TUI + MCP (Weeks 11-12)
```rust
ay_coder_tui::main
ay_coder_mcp_server::main
```

---

## Part 4: Key Rust Patterns Learned

### From Codex (Rust)
✅ Cargo workspace structure
✅ Async trait patterns
✅ MCP protocol implementation
✅ Platform-specific conditional compilation

### From Aider (Python → Rust)
✅ Coder base class → Trait
✅ Multi-LLM support → Trait objects
✅ Git integration → git2 crate
✅ Edit strategies → Enum-based pattern

### From Continue.dev (TypeScript → Rust)
✅ Core + interfaces separation
✅ Context layering
✅ Event streams → async channels

### From OpenHands (Python → Rust)
✅ Plugin registry → HashMap + Arc<dyn Trait>
✅ Delegation → Recursive struct
✅ Event-driven → tokio::sync::mpsc

### From Plandex (Go → Rust)
✅ Concurrent file loading → tokio::spawn
✅ Token limit enforcement → Runtime checks
✅ Channel-based error handling → Result

---

## Conclusion

**AY Coder will be unique by combining**:
- Codex's Rust performance + MCP
- Aider's Git integration + editing strategies
- Continue.dev's unified CLI/TUI architecture
- OpenHands' multi-agent capabilities
- Plandex's large context handling

**Next Steps**:
1. Implement ModelRouter with trait-based providers
2. Build ContextManager with token-aware layering
3. Create Coder with conversation management
4. Add Git integration with auto-commit
5. Implement CLI with interactive mode
6. Add TUI with ratatui

All patterns are now **concrete and implementable** in Rust. 🚀
