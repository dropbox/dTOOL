# AI Platform Awareness - DashFlow Self-Knowledge

**Version:** 1.0
**Date:** 2025-12-09
**Status:** COMPLETE - Phase 6 Implemented (N=259-265)
**Focus:** AIs understanding DashFlow platform and their own app architecture
**User Clarification:** "AIs using DashFlow need to know what functions of DashFlow exist as the platform, and how the app is built and how it works that is built in DashFlow."
**Priority:** P1 (critical for AI self-awareness)
**Implemented:** N=259-265 (229 platform tests)
**Note:** See ROADMAP_UNIFIED.md Phase 6 for completion details.

---

## Clear Understanding

**AI agents built with DashFlow need to understand:**

### 1. The Platform (DashFlow itself)
- What APIs does DashFlow provide?
- What features are available?
- What can I use to build my functionality?
- **"What is the toolbox I'm built with?"**

### 2. Their Own Application
- How is MY app structured?
- Which DashFlow features does MY app use?
- What's MY architecture?
- **"How am I built?"**

### 3. How They Work
- What happens when I execute?
- What's MY execution flow?
- Why do I make decisions?
- **"How do I work?"**

---

## Phase 1: DashFlow Platform Discovery (P1 - 12-15 hours)

### 1.1 Platform API Registry (6-8 hours)

**Purpose:** AI knows "what can DashFlow do?"

**Implementation:**
```rust
pub struct PlatformRegistry {
    pub modules: Vec<ModuleInfo>,
    pub features: Vec<FeatureInfo>,
    pub crates: Vec<CrateInfo>,
}

pub struct ModuleInfo {
    pub name: String,
    pub description: String,
    pub apis: Vec<ApiInfo>,
}

pub struct ApiInfo {
    pub function: String,
    pub description: String,
    pub signature: String,
    pub example: String,
}

impl DashFlow {
    pub fn platform_capabilities() -> PlatformRegistry {
        // Return all DashFlow features
    }
}
```

**Generated from source:**
```rust
// Auto-generate from documentation
// Parse lib.rs, collect all pub fn, pub struct
// Extract doc comments
// Create structured catalog
```

**Use case:**
```rust
// AI asks: "What can DashFlow do?"
let platform = DashFlow::platform_capabilities();

// AI sees:
// "StateGraph: Build graph-based workflows"
// "DashStreamCallback: Streaming telemetry"
// "dashflow-context: Token management"
// etc.

// AI asks: "How do I create a graph?"
let api = platform.find_api("StateGraph::new");
// Returns: signature, description, example code
```

---

### 1.2 Feature Catalog (4-5 hours)

**Purpose:** AI knows "what features does this platform have?"

**Implementation:**
```rust
pub enum DashFlowFeature {
    // Core
    GraphOrchestration { description: String },
    Checkpointing { backends: Vec<String> },
    Streaming { backends: Vec<String> },

    // Optimization
    PromptOptimization { algorithms: Vec<String> },
    ABTesting,
    QualityGates,

    // Tools & Integrations
    LLMProviders { supported: Vec<String> },
    VectorStores { supported: Vec<String> },
    Tools { available: Vec<String> },
}

impl DashFlow {
    pub fn features() -> Vec<DashFlowFeature> {
        // Enumerate all features
    }

    pub fn has_feature(&self, feature: &str) -> bool {
        // Check if feature available
    }
}
```

**Use case:**
```rust
// AI asks: "Can I use OpenAI?"
if DashFlow::has_feature("dashflow-openai") {
    // Yes, use it
}

// AI asks: "What LLM providers are available?"
let llm_feature = DashFlow::features()
    .find(|f| matches!(f, DashFlowFeature::LLMProviders { .. }));
// Returns: ["OpenAI", "Anthropic", "Bedrock", ...]
```

---

### 1.3 Documentation Querying (2-3 hours)

**Purpose:** AI can read DashFlow docs

**Implementation:**
```rust
pub struct DocumentationQuery {
    query: String,
}

impl DocumentationQuery {
    pub fn search(&self, query: &str) -> Vec<DocResult> {
        // Search embedded documentation
    }

    pub fn get_example(&self, topic: &str) -> Option<String> {
        // Get code example for topic
    }

    pub fn get_api_docs(&self, function: &str) -> Option<ApiDocs> {
        // Get function documentation
    }
}
```

**Use case:**
```rust
// AI asks: "How do I add a node?"
let docs = DocumentationQuery::new();
let results = docs.search("add node");
// Returns relevant doc sections + examples
```

---

## Phase 2: Application Self-Discovery (P1 - 10-12 hours)

### 2.1 App Architecture Analysis (6-8 hours)

**Purpose:** AI understands "how am I built?"

**Implementation:**
```rust
pub struct AppArchitecture {
    pub graph_structure: GraphManifest,
    pub dashflow_features_used: Vec<String>,
    pub custom_code: Vec<CodeModule>,
    pub dependencies: Vec<Dependency>,
}

impl CompiledGraph {
    pub fn analyze_architecture(&self) -> AppArchitecture {
        AppArchitecture {
            // What DashFlow features does this app use?
            dashflow_features_used: vec![
                "StateGraph (core orchestration)",
                "dashflow-openai (LLM provider)",
                "dashflow-context (token management)",
                "DashStreamCallback (telemetry)",
            ],

            // What custom code exists?
            custom_code: vec![
                CodeModule {
                    name: "reasoning_node",
                    file: "src/nodes/reasoning.rs",
                    lines: 245,
                    dashflow_apis_used: vec!["Message", "ChatModel"],
                },
                CodeModule {
                    name: "tool_execution",
                    file: "src/nodes/tools.rs",
                    lines: 180,
                    dashflow_apis_used: vec!["Tool", "SafeShellTool"],
                },
            ],

            graph_structure: self.manifest(),
            dependencies: parse_cargo_toml(),
        }
    }
}
```

**Use case:**
```rust
// AI asks: "What DashFlow features am I using?"
let arch = graph.analyze_architecture();

for feature in arch.dashflow_features_used {
    println!("Using: {}", feature);
}

// AI sees:
// "Using: StateGraph (core orchestration)"
// "Using: dashflow-openai (LLM provider)"
// "Using: dashflow-context (token management)"

// AI asks: "Where is my custom code?"
for module in arch.custom_code {
    println!("Custom module: {} ({} lines in {})",
        module.name, module.lines, module.file
    );
}
```

---

### 2.2 Dependency Analysis (4-5 hours)

**Purpose:** AI knows its dependency stack

**Implementation:**
```rust
pub struct DependencyAnalysis {
    pub dashflow_version: String,
    pub dashflow_crates: Vec<CrateDependency>,
    pub external_crates: Vec<CrateDependency>,
}

pub struct CrateDependency {
    pub name: String,
    pub version: String,
    pub purpose: String,  // Why is this needed?
    pub apis_used: Vec<String>,
}

impl AppArchitecture {
    pub fn dependency_analysis(&self) -> DependencyAnalysis {
        // Parse Cargo.toml + Cargo.lock
        // Identify DashFlow vs external
        // Map APIs to dependencies
    }
}
```

**Use case:**
```rust
// AI asks: "What version of DashFlow am I using?"
let deps = arch.dependency_analysis();
println!("DashFlow: v{}", deps.dashflow_version);

// AI asks: "Why do I depend on tokio?"
let tokio = deps.external_crates.find(|c| c.name == "tokio");
println!("Purpose: {}", tokio.purpose);
// "Async runtime - used by DashFlow core"
```

---

## Phase 3: Execution Understanding (P1 - 8-10 hours)

### 3.1 Execution Flow Documentation (4-5 hours)

**Purpose:** AI explains "how do I work?"

**Implementation:**
```rust
pub struct ExecutionFlow {
    pub graph_id: String,
    pub flow_description: String,
    pub decision_points: Vec<DecisionPoint>,
    pub loop_structures: Vec<LoopStructure>,
}

pub struct DecisionPoint {
    pub node: String,
    pub condition: String,
    pub paths: Vec<String>,
    pub explanation: String,
}

impl CompiledGraph {
    pub fn explain_execution_flow(&self) -> ExecutionFlow {
        // Analyze graph structure
        // Generate natural language explanation
    }
}
```

**Use case:**
```rust
// AI asks: "How do I work?"
let flow = graph.explain_execution_flow();

println!("{}", flow.flow_description);
// Output:
// "I am a coding agent that:
//  1. Receives user input (user_input node)
//  2. Reasons about the task (reasoning node using GPT-4)
//  3. If I need to call tools:
//     - Decides which tools (tool_selection node)
//     - Executes them safely (tool_execution node with sandbox)
//     - Analyzes results (result_analysis node)
//     - Loops back to reasoning if needed
//  4. Provides final response (output node)
//
//  Decision points:
//  - reasoning → tool_selection if has_tool_calls()
//  - reasoning → output if no tools needed
//  - result_analysis → reasoning if should_continue()
//  - result_analysis → output if done"

// AI understands its own execution flow!
```

---

### 3.2 Node Purpose Explanation (4-5 hours)

**Purpose:** AI knows "what does each node do?"

**Implementation:**
```rust
pub struct NodePurpose {
    pub node_name: String,
    pub purpose: String,
    pub inputs: Vec<String>,  // State fields read
    pub outputs: Vec<String>, // State fields written
    pub apis_used: Vec<String>, // DashFlow APIs called
    pub external_calls: Vec<String>, // External services
}

impl CompiledGraph {
    pub fn explain_node(&self, node_name: &str) -> NodePurpose {
        // Analyze node implementation
        // Extract purpose from docs
    }

    pub fn explain_all_nodes(&self) -> HashMap<String, NodePurpose> {
        // Explain every node
    }
}
```

**Use case:**
```rust
// AI asks: "What does my 'reasoning' node do?"
let purpose = graph.explain_node("reasoning");

println!("{}", purpose.purpose);
// "Calls GPT-4 to analyze the user's request and decide on tool usage"

println!("Inputs: {:?}", purpose.inputs);
// ["messages", "context", "available_tools"]

println!("Outputs: {:?}", purpose.outputs);
// ["pending_tool_calls", "reasoning_trace", "token_usage"]

println!("External calls: {:?}", purpose.external_calls);
// ["OpenAI API (GPT-4)"]

// AI fully understands what each part of itself does!
```

---

## Integration: Complete Self-Awareness API

```rust
pub struct AIPlatformAwareness {
    // Platform knowledge
    pub platform: PlatformRegistry,
    pub features: Vec<DashFlowFeature>,
    pub documentation: DocumentationQuery,

    // App knowledge
    pub architecture: AppArchitecture,
    pub dependencies: DependencyAnalysis,

    // Execution knowledge
    pub flow: ExecutionFlow,
    pub node_purposes: HashMap<String, NodePurpose>,
}

impl AIPlatformAwareness {
    pub fn introspect(&self) -> SelfKnowledge {
        SelfKnowledge {
            // Platform
            platform_version: self.platform.version,
            available_features: self.features.len(),

            // App
            dashflow_features_used: self.architecture.dashflow_features_used,
            custom_code_modules: self.architecture.custom_code.len(),

            // Execution
            graph_structure: self.flow.graph_id,
            node_count: self.node_purposes.len(),
            decision_points: self.flow.decision_points.len(),
        }
    }

    pub fn answer(&self, question: &str) -> String {
        match question {
            "what is dashflow?" => {
                format!("DashFlow v{} is a graph-based agent orchestration framework.
                        Features: {:?}",
                        self.platform.version,
                        self.features.iter().map(|f| f.name()).collect::<Vec<_>>())
            }
            "how am i built?" => {
                format!("I use these DashFlow features: {:?}.
                        I have {} custom nodes in {} files.",
                        self.architecture.dashflow_features_used,
                        self.node_purposes.len(),
                        self.architecture.custom_code.len())
            }
            "how do i work?" => {
                self.flow.flow_description.clone()
            }
            _ => self.documentation.search(question)
                .first()
                .map(|r| r.content.clone())
                .unwrap_or("Unknown".to_string())
        }
    }
}
```

---

## Implementation Plan

### Phase 1: Platform Discovery (12-15h)
- N=250-252: API registry, feature catalog, documentation querying

### Phase 2: App Self-Discovery (10-12h)
- N=253-254: Architecture analysis, dependency analysis

### Phase 3: Execution Understanding (8-10h)
- N=255-256: Flow explanation, node purpose

### Integration (5-8h)
- N=257: Combined awareness API

**Total:** 35-45 hours

---

## Example Conversations

### AI Understanding Platform:

```
AI: What is DashFlow?
System: DashFlow v1.11.3 is a graph-based agent orchestration framework.
        Features: StateGraph, Checkpointing, Streaming, Optimization,
        100+ integrations

AI: Can I use vector stores?
System: Yes. Available: Chroma, Qdrant, Pinecone, Postgres, Redis,
        12 more...

AI: How do I create a graph?
System: [Returns code example from docs]
```

### AI Understanding Itself:

```
AI: How am I built?
System: You use these DashFlow features:
        - StateGraph (core orchestration)
        - dashflow-openai (GPT-4 provider)
        - dashflow-context (token management)
        - dashflow-git-tool (repository access)
        - SafeShellTool (command execution)

        You have 5 custom nodes:
        - user_input (src/nodes/input.rs, 120 lines)
        - reasoning (src/nodes/reasoning.rs, 245 lines)
        - tool_selection (src/nodes/tools.rs, 180 lines)
        - tool_execution (src/nodes/execute.rs, 156 lines)
        - output (src/nodes/output.rs, 98 lines)

AI: How do I work?
System: [Returns execution flow explanation]
        You are a coding agent that:
        1. Receives user requests
        2. Reasons about them (using GPT-4)
        3. Calls tools if needed (with safety checks)
        4. Provides responses

AI: What does my 'reasoning' node do?
System: Calls GPT-4 to analyze requests and decide on tool usage.
        Inputs: messages, context, available_tools
        Outputs: pending_tool_calls, reasoning_trace
        External: OpenAI API (GPT-4)

AI: Why do I depend on dashflow-context?
System: Token management - you use it to:
        - Count tokens in conversations
        - Truncate context to fit model limits
        - Track token budget
```

---

## Storage & Generation

### Auto-Generate from Code:
```rust
// Parse at compile time or runtime
pub fn generate_platform_registry() -> PlatformRegistry {
    // Use syn to parse Rust code
    // Extract pub items with doc comments
    // Build structured registry
}
```

### Embed in Binary:
```rust
// Include at compile time
pub const PLATFORM_DOCS: &str = include_str!("../PLATFORM_API.json");

// Or generate dynamically
pub fn platform_registry() -> &'static PlatformRegistry {
    // Lazy static with parsed registry
}
```

---

## Success Criteria

**After implementation:**

- [x] AI can list all DashFlow APIs (PlatformRegistry)
- [x] AI can query "what can DashFlow do?" (platform_capabilities())
- [x] AI knows which DashFlow features it uses (FeatureInfo)
- [x] AI can explain its own architecture (AppArchitecture)
- [x] AI understands its execution flow (ExecutionFlow)
- [x] AI knows purpose of each node (NodePurpose)
- [x] AI can answer "how am I built?" (analyze_architecture())
- [x] AI can answer "how do I work?" (explain_execution_flow())

---

## Priority

**This is CRITICAL for AI self-awareness:**

Without this, AI agents:
- ❌ Don't know what platform features are available
- ❌ Don't understand their own architecture
- ❌ Can't explain how they work
- ❌ Can't effectively self-improve

With this, AI agents:
- ✅ Understand the platform they're built on
- ✅ Know their own structure and capabilities
- ✅ Can explain their execution flow
- ✅ Can make informed decisions about changes

---

## Timeline

**Current:** Finish stability (10-20h)

**Then:** AI Platform Awareness (35-45h) ← THIS ROADMAP

**Then:** AI Introspection (70-90h) - runtime behavior

**Then:** Graph Versioning (45-60h) - evolution tracking

**Total:** 160-215 hours for complete AI self-awareness

---

## Progress Tracking

| Phase | Component | Status | Tests | Commit |
|-------|-----------|--------|-------|--------|
| 1.1 | Platform API Registry | Complete | 26 | #259 |
| 1.2 | Feature Catalog | Complete | 37 | #260 |
| 1.3 | Documentation Querying | Complete | 31 | #261 |
| 2.1 | App Architecture Analysis | Complete | 22 | #262 |
| 2.2 | Dependency Analysis | Complete | 45 | #263 |
| 3.1 | Execution Flow Documentation | Complete | 34 | #264 |
| 3.2 | Node Purpose Explanation | Complete | 34 | #265 |

**Total Tests:** 229 platform awareness tests passing

**All phases complete!** This roadmap is fully implemented.

---

**This roadmap makes DashFlow AIs truly self-aware of both the platform and themselves.**
