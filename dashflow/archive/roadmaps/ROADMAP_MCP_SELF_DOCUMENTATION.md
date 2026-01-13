# MCP Self-Documentation Protocol - P0 Priority

**Version:** 1.0
**Date:** 2025-12-09
**Status:** COMPLETE - Phase 9 Implemented (N=297-308)
**User Requirement:** "apps built using DashFlow need a --help and --help-more CLI option... Maybe this can be some kind of MCP interface?... MCP service for itself"
**Priority:** P0 (CRITICAL for AI-to-AI understanding)
**Implemented:** N=297-308 (64 mcp_self_doc tests)

---

## Vision

**DashFlow apps expose themselves via MCP (Model Context Protocol):**
- AI agents can query "what are you?" and get structured responses
- Progressive disclosure: tl;dr → detailed → implementation-level
- Standardized format that all DashFlow apps follow
- Works for AI agents talking to DashFlow apps
- Works for DashFlow apps talking to themselves

**Key insight:** Every DashFlow app is an MCP server that describes itself.

---

## MCP Self-Documentation Protocol

### Standard Endpoints

Every DashFlow app exposes these MCP endpoints:

```json
{
  "protocol": "dashflow-self-doc/1.0",
  "endpoints": {
    "/about": "High-level description (tl;dr)",
    "/capabilities": "What can this app do?",
    "/architecture": "How is it built? (ok-a-little-more)",
    "/implementation": "Code pointers and internals (detailed)",
    "/nodes": "All nodes with versions and code locations",
    "/tools": "Available tools and their implementations",
    "/introspect": "Query interface for arbitrary questions"
  }
}
```

---

## Phase 1: CLI Integration (P0 - 10-12 hours)

### 1.1 --help and --help-more Flags (6-8 hours)

**Implementation:**
```rust
// Auto-generated help system
impl CompiledGraph<S> {
    pub fn generate_help(&self, level: HelpLevel) -> String {
        match level {
            HelpLevel::Brief => self.help_tldr(),
            HelpLevel::More => self.help_detailed(),
            HelpLevel::Full => self.help_implementation(),
        }
    }

    fn help_tldr(&self) -> String {
        format!(
            "{} v{}\n\n\
             {}\n\n\
             CAPABILITIES:\n\
             {}\n\n\
             USAGE:\n\
             {} [OPTIONS]\n\n\
             Use --help-more for detailed information",
            self.manifest().name,
            self.manifest().version,
            self.manifest().description,
            self.list_capabilities_brief(),
            self.manifest().binary_name,
        )
    }

    fn help_detailed(&self) -> String {
        format!(
            "{}

ARCHITECTURE:
This app is built with DashFlow and consists of:
- {} nodes (processing units)
- {} edges (connections)
- {} tools available

NODES:
{}

DASHFLOW FEATURES USED:
{}

EXECUTION FLOW:
{}

Use --help-implementation for code locations and versions",
            self.help_tldr(),
            self.manifest().nodes.len(),
            self.manifest().edges.len(),
            self.capabilities().tools.len(),
            self.list_nodes_with_descriptions(),
            self.list_dashflow_features_used(),
            self.explain_execution_flow(),
        )
    }

    fn help_implementation(&self) -> String {
        format!(
            "{}

IMPLEMENTATION DETAILS:

NODE VERSIONS:
{}

CODE LOCATIONS:
{}

DASHFLOW VERSION:
- dashflow: {}
- Features: {}

INTERNAL APIS:
{}

DEPENDENCIES:
{}",
            self.help_detailed(),
            self.list_node_versions(),
            self.list_code_locations(),
            env!("CARGO_PKG_VERSION"),
            self.list_cargo_features(),
            self.list_internal_apis(),
            self.list_dependencies(),
        )
    }
}

// In main.rs of every DashFlow app:
#[tokio::main]
async fn main() -> Result<()> {
    let args = std::env::args().collect::<Vec<_>>();

    if args.contains(&"--help".to_string()) {
        println!("{}", compiled.generate_help(HelpLevel::Brief));
        return Ok(());
    }

    if args.contains(&"--help-more".to_string()) {
        println!("{}", compiled.generate_help(HelpLevel::More));
        return Ok(());
    }

    if args.contains(&"--help-implementation".to_string()) {
        println!("{}", compiled.generate_help(HelpLevel::Full));
        return Ok(());
    }

    // Normal execution...
}
```

---

### 1.2 Example Output

**$ ./my_agent --help** (tl;dr)
```
Coding Agent v1.0.0

An AI coding assistant that can read, write, and modify code files
using git-aware context and safe shell execution.

CAPABILITIES:
- Read and analyze code files
- Generate code modifications
- Execute shell commands (sandboxed)
- Git operations (status, diff, commit)
- Context-aware suggestions

USAGE:
my_agent [OPTIONS]

Options:
  --query <text>     Natural language request
  --help-more        Show architecture and implementation details
```

**$ ./my_agent --help-more** (ok-a-little-more)
```
[... tl;dr content ...]

ARCHITECTURE:
This app is built with DashFlow and consists of:
- 5 nodes (user_input, reasoning, tool_selection, tool_execution, output)
- 8 edges (including 2 conditional branches)
- 6 tools available (read_file, write_file, shell, git_status, git_diff, git_commit)

NODES:
- user_input: Parse user request and initialize state
  • Reads: raw_input
  • Writes: messages, turn_id
  • Tools: none
  • Description: Converts natural language to structured request

- reasoning: LLM-based decision making (GPT-4)
  • Reads: messages, context, available_tools
  • Writes: pending_tool_calls, reasoning_trace
  • Tools: all 6 tools
  • Description: Analyzes request and decides which tools to use

- tool_execution: Execute selected tools with safety checks
  • Reads: pending_tool_calls, sandbox_mode
  • Writes: tool_results, execution_errors
  • Tools: executes selected tools
  • Description: Runs tools in sandboxed environment with approval

[... more nodes ...]

DASHFLOW FEATURES USED:
- StateGraph (core orchestration)
- dashflow-openai (GPT-4 provider)
- dashflow-context (token management)
- dashflow-git-tool (repository operations)
- dashflow-shell-tool (sandboxed execution)
- DashStreamCallback (telemetry)
- InMemoryCheckpointer (state persistence)

EXECUTION FLOW:
1. User provides input
2. Reasoning node calls GPT-4 to analyze
3. If tools needed: tool_selection → tool_execution → reasoning (loop)
4. If no tools: reasoning → output (direct response)

Use --help-implementation for code locations and versions
```

**$ ./my_agent --help-implementation** (full details)
```
[... previous content ...]

IMPLEMENTATION DETAILS:

NODE VERSIONS:
- user_input: v1.0.0 (hash: a1b2c3)
- reasoning: v1.2.3 (hash: d4e5f6)
- tool_execution: v1.1.0 (hash: g7h8i9)

CODE LOCATIONS:
- user_input: src/nodes/input.rs:45-120
- reasoning: src/nodes/reasoning.rs:23-245
- tool_execution: src/nodes/execute.rs:78-234

DASHFLOW VERSION:
- dashflow: 1.11.3
- Features: streaming, introspection, optimization

INTERNAL APIS:
- Message: dashflow::core::messages::Message
- ChatModel: dashflow::core::language_models::ChatModel
- Tool: dashflow::core::tools::Tool
- StateGraph: dashflow::StateGraph
- CompiledGraph: dashflow::CompiledGraph

DEPENDENCIES (DashFlow crates):
- dashflow-openai: 1.0.0 (OpenAI API client)
- dashflow-context: 1.0.0 (token counting)
- dashflow-git-tool: 1.0.0 (git operations)
- dashflow-shell-tool: 1.0.0 (sandboxed shell)

DEPENDENCIES (External):
- tokio: 1.40 (async runtime)
- serde: 1.0 (serialization)
- clap: 4.5 (CLI parsing)
```

---

## Phase 2: MCP Server Implementation (P0 - 8-10 hours)

### 2.1 DashFlow MCP Server (5-7 hours)

**Purpose:** Every DashFlow app is an MCP server

**Implementation:**
```rust
pub struct DashFlowMcpServer {
    compiled: CompiledGraph<S>,
    port: u16,
}

impl DashFlowMcpServer {
    /// Start MCP server for introspection
    pub async fn start(&self) -> Result<()> {
        let app = Router::new()
            .route("/mcp/about", get(self.handle_about()))
            .route("/mcp/capabilities", get(self.handle_capabilities()))
            .route("/mcp/architecture", get(self.handle_architecture()))
            .route("/mcp/implementation", get(self.handle_implementation()))
            .route("/mcp/nodes", get(self.handle_nodes()))
            .route("/mcp/tools", get(self.handle_tools()))
            .route("/mcp/introspect", post(self.handle_query()));

        axum::Server::bind(&format!("127.0.0.1:{}", self.port).parse()?)
            .serve(app.into_make_service())
            .await?;

        Ok(())
    }
}

// MCP Response Format (JSON)
#[derive(Serialize)]
pub struct McpAboutResponse {
    pub name: String,
    pub version: String,
    pub description: String,
    pub capabilities: Vec<String>,
    pub dashflow_version: String,
}

#[derive(Serialize)]
pub struct McpArchitectureResponse {
    pub nodes: HashMap<String, NodeInfo>,
    pub edges: HashMap<String, Vec<EdgeInfo>>,
    pub dashflow_features: Vec<String>,
    pub execution_flow: String,
}

#[derive(Serialize)]
pub struct McpImplementationResponse {
    pub node_versions: HashMap<String, String>,
    pub code_locations: HashMap<String, CodeLocation>,
    pub dependencies: Vec<Dependency>,
    pub internal_apis: Vec<ApiReference>,
}

#[derive(Serialize)]
pub struct CodeLocation {
    pub file: String,
    pub line_start: usize,
    pub line_end: usize,
    pub github_url: Option<String>,  // If available
}
```

---

### 2.2 Query Interface (3-4 hours)

**Purpose:** AI agents can ask arbitrary questions

**Implementation:**
```rust
#[derive(Deserialize)]
pub struct McpQuery {
    pub question: String,
}

#[derive(Serialize)]
pub struct McpQueryResponse {
    pub answer: String,
    pub sources: Vec<String>,  // References
    pub confidence: f64,
}

impl DashFlowMcpServer {
    async fn handle_query(&self, query: McpQuery) -> Result<McpQueryResponse> {
        // Parse question and route to appropriate handler
        let answer = match query.question.as_str() {
            q if q.contains("what") && q.contains("node") => {
                self.answer_node_question(q)
            }
            q if q.contains("how") && q.contains("work") => {
                self.answer_execution_question(q)
            }
            q if q.contains("why") => {
                self.answer_decision_question(q)
            }
            q if q.contains("code") || q.contains("implementation") => {
                self.answer_implementation_question(q)
            }
            _ => {
                self.search_all_knowledge(q)
            }
        };

        Ok(answer)
    }
}

// Example queries:
// "what does the reasoning node do?"
// "how does tool execution work?"
// "why did I choose this path?"
// "show me the code for node X"
// "what version of dashflow-openai am I using?"
```

---

## Phase 3: Progressive Disclosure Roadmap (P0 - 5-7 hours)

### 3.1 Three-Level Roadmap (3-4 hours)

**Auto-generated from graph structure:**

```rust
pub struct ProgressiveRoadmap {
    pub tldr: String,
    pub ok_a_little_more: String,
    pub detailed: String,
}

impl CompiledGraph {
    pub fn generate_roadmap(&self) -> ProgressiveRoadmap {
        ProgressiveRoadmap {
            // Level 1: tl;dr (30 seconds to read)
            tldr: format!(
                "I am a {} agent.\n\
                 I have {} capabilities.\n\
                 I use {} DashFlow features.\n\
                 I can: {}",
                self.manifest().category,
                self.capabilities().tools.len(),
                self.architecture().dashflow_features_used.len(),
                self.list_capabilities_oneline(),
            ),

            // Level 2: ok-a-little-more (2-3 minutes to read)
            ok_a_little_more: format!(
                "{}

STRUCTURE:
I am built as a graph with {} nodes:
{}

I make decisions at {} points:
{}

I use these DashFlow features:
{}

My typical execution:
{}",
                self.tldr,
                self.manifest().nodes.len(),
                self.list_nodes_brief(),
                self.count_decision_points(),
                self.list_decision_logic(),
                self.list_features_with_purpose(),
                self.describe_typical_flow(),
            ),

            // Level 3: detailed (10-15 minutes to read)
            detailed: format!(
                "{}

DETAILED IMPLEMENTATION:

Node Implementations:
{}

DashFlow Feature Usage:
{}

Code Organization:
{}

Dependencies:
{}

Execution Patterns:
{}

Internal APIs:
{}",
                self.ok_a_little_more,
                self.describe_all_nodes_detailed(),
                self.explain_feature_usage_detailed(),
                self.show_code_organization(),
                self.list_dependencies_detailed(),
                self.explain_execution_patterns(),
                self.document_internal_apis(),
            ),
        }
    }
}
```

---

### 3.2 MCP Tools for Self-Documentation (2-3 hours)

**Expose as MCP tools:**

```json
{
  "tools": [
    {
      "name": "app_about",
      "description": "Get high-level description of this DashFlow app",
      "parameters": {
        "level": {
          "type": "string",
          "enum": ["tldr", "more", "detailed"],
          "default": "tldr"
        }
      }
    },
    {
      "name": "app_inspect_node",
      "description": "Get details about a specific node",
      "parameters": {
        "node_name": {"type": "string"},
        "include_code": {"type": "boolean", "default": false}
      }
    },
    {
      "name": "app_show_code",
      "description": "Show actual code for a component",
      "parameters": {
        "component": {"type": "string"},
        "context_lines": {"type": "integer", "default": 10}
      }
    },
    {
      "name": "app_explain_decision",
      "description": "Explain why app made a specific decision",
      "parameters": {
        "execution_id": {"type": "string"},
        "decision_point": {"type": "string"}
      }
    }
  ]
}
```

---

## Phase 4: Standardized Format (P0 - 10-12 hours)

### 4.1 DashFlow App Manifest (JSON Schema) (6-8 hours)

**Standard format all apps follow:**

```json
{
  "$schema": "https://dashflow.dev/schemas/app-manifest/1.0.json",
  "name": "coding_agent",
  "version": "1.0.0",
  "description": "AI coding assistant with git and shell tools",
  "dashflow_version": "1.11.3",

  "capabilities": [
    {
      "id": "code_reading",
      "name": "Code Analysis",
      "description": "Read and understand code files",
      "tools": ["read_file", "list_directory"],
      "nodes": ["file_reader", "code_analyzer"]
    },
    {
      "id": "code_modification",
      "name": "Code Generation",
      "description": "Generate and modify code",
      "tools": ["write_file", "apply_patch"],
      "nodes": ["code_generator", "patch_applier"]
    }
  ],

  "graph_structure": {
    "nodes": [
      {
        "name": "reasoning",
        "type": "llm",
        "version": "1.2.3",
        "code_location": {
          "file": "src/nodes/reasoning.rs",
          "line_start": 23,
          "line_end": 245,
          "github_url": "https://github.com/org/repo/blob/main/src/nodes/reasoning.rs#L23-L245"
        },
        "dashflow_apis": [
          "dashflow::core::messages::Message",
          "dashflow::core::language_models::ChatModel"
        ],
        "purpose": "LLM-based reasoning and tool selection",
        "inputs": ["messages", "context", "available_tools"],
        "outputs": ["pending_tool_calls", "reasoning_trace"]
      }
    ],
    "edges": [
      {
        "from": "reasoning",
        "to": "tool_selection",
        "condition": "has_tool_calls()",
        "type": "conditional"
      }
    ]
  },

  "dashflow_features": [
    {
      "feature": "dashflow-openai",
      "version": "1.0.0",
      "purpose": "GPT-4 LLM provider",
      "used_in": ["reasoning"]
    },
    {
      "feature": "dashflow-context",
      "version": "1.0.0",
      "purpose": "Token counting and context management",
      "used_in": ["reasoning", "output"]
    }
  ],

  "introspection_endpoints": {
    "mcp_port": 8080,
    "endpoints": [
      "/mcp/about",
      "/mcp/capabilities",
      "/mcp/architecture",
      "/mcp/implementation"
    ]
  }
}
```

---

### 4.2 Auto-Generate Manifest (4-5 hours)

**Generate on compile:**

```rust
impl StateGraph<S> {
    pub fn compile(self) -> Result<CompiledGraph<S>> {
        // ... existing compilation ...

        // AUTO-GENERATE MANIFEST
        let manifest = self.generate_manifest()?;

        // AUTO-GENERATE ROADMAP
        let roadmap = self.generate_progressive_roadmap(&manifest)?;

        // Save manifest.json
        if cfg!(debug_assertions) {
            std::fs::write("target/app_manifest.json",
                serde_json::to_string_pretty(&manifest)?)?;
        }

        Ok(CompiledGraph {
            manifest,
            roadmap,
            ...
        })
    }
}
```

---

## Integration with MCP Ecosystem

### AI-to-AI Communication

**Scenario:** Claude using a DashFlow app

```
Claude: What can you do?
DashFlow App (via MCP): I can read/write code, execute shell commands,
                         and perform git operations.

Claude: Show me your architecture
DashFlow App: [Returns JSON manifest with full structure]

Claude: Where is your reasoning code?
DashFlow App: src/nodes/reasoning.rs:23-245
                Uses: dashflow-openai for GPT-4

Claude: Why did you choose tool_execution instead of output?
DashFlow App: State had 3 pending_tool_calls. Condition has_tool_calls()
              evaluated to true. This matches decision pattern "tools_needed".
```

---

## Use Cases

### 1. AI Agent Inspecting Another AI

```rust
// Agent A queries Agent B via MCP
let client = McpClient::connect("http://agent-b:8080")?;

let about = client.call("app_about", json!({"level": "tldr"})).await?;
println!("Agent B: {}", about);

let nodes = client.call("app_list_nodes", json!({})).await?;
println!("Agent B has {} nodes", nodes.len());
```

### 2. DashFlow App Inspecting Itself

```rust
// Agent introspects itself during execution
async fn reasoning_node(state: State, ctx: &ExecutionContext) -> Result<State> {
    // AI knows about itself
    let my_capabilities = ctx.manifest.capabilities();
    let my_tools = ctx.manifest.available_tools();

    // AI checks its implementation
    let my_code = ctx.manifest.get_node_code("reasoning")?;
    println!("I am implemented in: {}", my_code.file);

    // AI makes informed decisions
    if state.complexity > 0.8 && my_tools.contains("decompose") {
        // "I know I have a decompose tool, I'll use it for complex tasks"
    }

    Ok(state)
}
```

### 3. Human Inspecting DashFlow App

```bash
# Quick overview
$ my_agent --help

# More detail
$ my_agent --help-more

# Full implementation
$ my_agent --help-implementation

# MCP queries
$ curl http://localhost:8080/mcp/about
$ curl http://localhost:8080/mcp/architecture
$ curl -X POST http://localhost:8080/mcp/introspect \
  -d '{"question": "what does reasoning node do?"}'
```

---

## Implementation Priority

**Worker N=295-300: MCP Self-Documentation (25-35 hours)**

- N=295-296: CLI --help integration (10-12h)
- N=297: MCP server implementation (5-7h)
- N=298: Progressive roadmap generation (3-4h)
- N=299: JSON manifest schema (4-5h)
- N=300: MCP query interface (3-4h)

---

## Success Criteria

After implementation:

- [ ] Every DashFlow app has --help, --help-more, --help-implementation
- [ ] Help output shows nodes, tools, DashFlow features, code locations
- [ ] Apps expose MCP server on localhost
- [ ] Standard JSON manifest auto-generated
- [ ] AI agents can query other DashFlow apps via MCP
- [ ] DashFlow apps can introspect themselves via MCP
- [ ] Progressive disclosure (tl;dr → detailed)
- [ ] Code pointers work (file:line references)
- [ ] Works with all DashFlow features

---

## Benefits

**For AI Agents:**
- Understand other AI agents' capabilities
- Know how they're implemented
- Query for specific details
- Standard protocol (MCP)

**For Developers:**
- Auto-documented apps
- No manual documentation needed
- Always up-to-date
- Standard format

**For Operations:**
- Service discovery
- Capability mapping
- Version tracking
- Dependency auditing

---

**This makes DashFlow apps self-documenting via MCP - AI agents can understand each other through a standard protocol.**
