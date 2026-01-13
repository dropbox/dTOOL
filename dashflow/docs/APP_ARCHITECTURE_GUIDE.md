# Building Applications on DashFlow

**Last Updated:** 2026-01-02 (Worker #2337 - Add missing Last Updated headers)

**Purpose:** How to structure apps for upgradability, portability, and modularity
**Audience:** Application developers using DashFlow framework

---

## The Problem

**Bad App Architecture:**
```rust
// main.rs (1000+ lines, everything mixed together)
use dashflow_openai::ChatOpenAI;
use dashflow::StateGraph;

fn main() {
    // Hardcoded model
    let llm = ChatOpenAI::new().with_model("gpt-4o");

    // Business logic mixed with framework calls
    let mut graph = StateGraph::new();
    graph.add_node_from_fn("research", |state| {
        // Hardcoded prompts
        // Direct API calls
        // Business logic embedded
    });

    // Can't test without real LLM
    // Can't swap providers
    // Can't upgrade framework easily
    // Everything is coupled
}
```

**When framework updates:**
- API changes break your app
- Can't upgrade without rewriting
- Tests break
- No migration path

---

## The Solution: Layered Architecture

### Layer 1: Framework Interface (Thin Adapter)

**Purpose:** Isolate your app from framework specifics

```rust
// src/framework/mod.rs

pub trait LLMProvider: Send + Sync {
    async fn generate(&self, prompt: &str) -> Result<String>;
}

pub trait Checkpointer: Send + Sync {
    async fn save(&self, id: &str, state: &AppState) -> Result<()>;
    async fn load(&self, id: &str) -> Result<Option<AppState>>;
}

pub trait GraphExecutor {
    async fn run(&self, input: AppInput) -> Result<AppOutput>;
}
```

**Benefits:**
- Your app depends on YOUR traits, not framework traits
- Framework changes don't break your app
- Can swap implementations
- Easy to test with mocks

---

### Layer 2: Framework Adapters

**Purpose:** Adapt framework to your interface

```rust
// src/framework/dashflow_adapter.rs

use crate::framework::{LLMProvider, Checkpointer};
use dashflow_openai::ChatOpenAI;
use dashflow_postgres_checkpointer::PostgresCheckpointer;

pub struct DashFlowLLM {
    inner: ChatOpenAI,
}

impl LLMProvider for DashFlowLLM {
    async fn generate(&self, prompt: &str) -> Result<String> {
        let messages = vec![Message::human(prompt)];
        let result = self.inner.generate(&messages, None).await?;
        Ok(result.generations[0].message.as_text().to_string())
    }
}

// When framework updates, ONLY adapters need changes
// Your app code stays the same!
```

---

### Layer 3: Domain Logic

**Purpose:** Your business logic, framework-agnostic

```rust
// src/domain/workflows.rs

use crate::framework::{LLMProvider, Checkpointer};

pub struct ResearchWorkflow {
    llm: Box<dyn LLMProvider>,
    checkpointer: Box<dyn Checkpointer>,
}

impl ResearchWorkflow {
    pub fn new(
        llm: impl LLMProvider + 'static,
        checkpointer: impl Checkpointer + 'static,
    ) -> Self {
        Self {
            llm: Box::new(llm),
            checkpointer: Box::new(checkpointer),
        }
    }

    pub async fn execute(&self, topic: &str) -> Result<Report> {
        // Your domain logic
        // No framework-specific code
        // Easy to test
        // Easy to understand
    }
}
```

---

### Layer 4: Configuration

**Purpose:** External configuration, not hardcoded

```toml
# config.toml

[llm]
provider = "openai"  # Can change to "anthropic" without code change
model = "gpt-4o-mini"
temperature = 0.7

[checkpointer]
type = "postgres"  # Can change to "redis" or "s3"
url = "postgresql://localhost/dashflow"

[workflow]
max_iterations = 10
timeout_seconds = 300
```

```rust
// src/config.rs

#[derive(Deserialize)]
pub struct AppConfig {
    pub llm: LLMConfig,
    pub checkpointer: CheckpointerConfig,
    pub workflow: WorkflowConfig,
}

impl AppConfig {
    pub fn from_file(path: &str) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Ok(toml::from_str(&content)?)
    }

    pub fn create_llm(&self) -> Box<dyn LLMProvider> {
        match self.llm.provider.as_str() {
            "openai" => Box::new(DashFlowLLM::openai(&self.llm)),
            "anthropic" => Box::new(DashFlowLLM::anthropic(&self.llm)),
            _ => panic!("Unknown provider"),
        }
    }
}
```

**Benefits:**
- Swap providers without code changes
- Different configs for dev/prod
- No secrets in code
- Easy deployment

---

## Complete App Structure

```
my-app/
├── Cargo.toml                    # App dependencies
├── config/
│   ├── dev.toml                  # Dev config
│   ├── prod.toml                 # Prod config
│   └── test.toml                 # Test config
├── src/
│   ├── main.rs                   # Entry point (minimal)
│   ├── framework/
│   │   ├── mod.rs                # Framework interface traits
│   │   ├── dashflow_adapter.rs  # DashFlow → Your traits
│   │   └── mock_adapter.rs       # Mocks for testing
│   ├── domain/
│   │   ├── mod.rs
│   │   ├── workflows.rs          # Business logic
│   │   ├── agents.rs             # Domain agents
│   │   └── state.rs              # Domain state types
│   ├── config.rs                 # Configuration
│   └── lib.rs                    # Library (testable!)
└── tests/
    ├── integration_tests.rs      # With mocks
    └── e2e_tests.rs              # With real framework (ignored)
```

---

## Example: Research App

### 1. Define Your Domain Types

```rust
// src/domain/state.rs

#[derive(Clone, Serialize, Deserialize)]
pub struct ResearchState {
    pub query: String,
    pub findings: Vec<Finding>,
    pub report: String,
    pub quality_score: f64,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Finding {
    pub source: String,
    pub content: String,
    pub relevance: f64,
}
```

**Not tied to framework - YOUR types**

---

### 2. Define Your Workflow Interface

```rust
// src/domain/workflows.rs

pub trait ResearchWorkflow {
    async fn research(&self, query: &str) -> Result<Vec<Finding>>;
    async fn write_report(&self, findings: &[Finding]) -> Result<String>;
    async fn critique(&self, report: &str) -> Result<f64>;
}
```

**YOUR interface, not framework's**

---

### 3. Implement with Framework

```rust
// src/framework/dashflow_research.rs

use dashflow::{StateGraph, END};
use crate::domain::{ResearchWorkflow, ResearchState};

pub struct DashFlowResearchWorkflow {
    graph: CompiledGraph<ResearchState>,
}

impl DashFlowResearchWorkflow {
    pub fn new(llm: impl ChatModel + 'static) -> Self {
        let mut graph = StateGraph::new();

        // Build graph using framework
        graph.add_node_from_fn("research", move |state| {
            let llm = llm.clone();
            Box::pin(async move {
                // Framework-specific code HERE
                // Isolated from domain logic
            })
        });

        graph.add_node_from_fn("write", ...);
        graph.add_node_from_fn("critique", ...);

        graph.set_entry_point("research");

        Self {
            graph: graph.compile().unwrap(),
        }
    }
}

impl ResearchWorkflow for DashFlowResearchWorkflow {
    async fn research(&self, query: &str) -> Result<Vec<Finding>> {
        let state = ResearchState {
            query: query.to_string(),
            ..Default::default()
        };

        let result = self.graph.invoke(state).await?;
        Ok(result.findings)
    }
}
```

**Framework code isolated in adapters**

---

### 4. Use in Main (Minimal)

```rust
// src/main.rs

use my_app::config::AppConfig;
use my_app::domain::ResearchWorkflow;
use my_app::framework::DashFlowResearchWorkflow;

#[tokio::main]
async fn main() -> Result<()> {
    // Load config
    let config = AppConfig::from_file("config/prod.toml")?;

    // Create workflow (via config)
    let workflow = config.create_research_workflow();

    // Use domain interface (not framework!)
    let findings = workflow.research("AI trends").await?;
    let report = workflow.write_report(&findings).await?;

    println!("{}", report);
    Ok(())
}
```

**Main.rs is 15 lines, not 1000!**

---

## Upgrading Framework Versions

### When DashFlow v1.7 comes out:

**Your code:**
```rust
// src/domain/workflows.rs
// UNCHANGED - doesn't depend on framework
```

**Update needed:**
```rust
// src/framework/dashflow_adapter.rs
// Update this ONE file to new API

// Before (v1.6):
let result = llm.generate(&messages, None).await?;

// After (v1.7 - hypothetical API change):
let result = llm.generate(&messages).with_timeout(30).await?;

// That's it! Rest of app unchanged.
```

**Steps:**
1. Update Cargo.toml: `dashflow-* = "1.11"`
2. Fix compiler errors in adapters/ only
3. Run tests
4. Deploy

**Your domain logic never changes!**

---

## Testing Strategy

### Unit Tests (No Framework)

```rust
// tests/domain_tests.rs

use my_app::domain::{ResearchWorkflow, Finding};
use my_app::framework::MockLLM;  // Your mock, not framework's

#[tokio::test]
async fn test_research_workflow() {
    let mock_llm = MockLLM::new(vec![
        "Finding 1: AI is growing",
        "Finding 2: LLMs are popular",
    ]);

    let workflow = create_workflow_with_mock(mock_llm);
    let findings = workflow.research("AI").await.unwrap();

    assert_eq!(findings.len(), 2);
    // Test YOUR logic, not framework
}
```

**Tests run fast, no external dependencies, no framework needed**

---

### Integration Tests (With Framework)

```rust
// tests/integration_tests.rs

#[tokio::test]
#[ignore]  // Needs API keys
async fn test_real_workflow() {
    let config = AppConfig::from_file("config/test.toml")?;
    let workflow = config.create_research_workflow();

    // Test with real framework
    let findings = workflow.research("Rust").await?;
    assert!(findings.len() > 0);
}
```

**Run occasionally to verify framework integration**

---

## Version Pinning Strategy

### In Your App's Cargo.toml

```toml
[dependencies]
# Pin MINOR version, allow PATCH updates
dashflow = "1.11"       # Will use 1.11.x (safe patches)
dashflow-openai = "1.11"

# NOT this (too loose):
# dashflow = "1"  # Could break on minor updates

# NOT this (too strict):
# dashflow = "1.11.0"  # Won't get security patches
```

**Rationale:**
- PATCH updates (1.6.0 → 1.6.1): Bug fixes, safe
- MINOR updates (1.6 → 1.7): New features, may break
- MAJOR updates (1.x → 2.x): Breaking changes, will break

**Upgrade process:**
- Test with new MINOR version in dev
- Update adapters if needed
- Verify tests pass
- Deploy to prod

---

## Modularity Patterns

### Pattern 1: Separate Crates

```
my-company/
├── my-app-domain/          # Domain logic (no framework deps)
│   └── Cargo.toml          # Only std, serde, etc.
├── my-app-framework/       # Framework adapters
│   └── Cargo.toml          # Depends on dashflow-*
└── my-app/                 # Main binary
    └── Cargo.toml          # Depends on domain + framework
```

**Benefits:**
- Domain crate has NO framework dependency
- Can test domain without framework
- Clear separation of concerns
- Framework isolated to adapter crate

---

### Pattern 2: Feature Flags

```toml
[features]
default = ["dashflow-framework"]
dashflow-framework = ["dashflow"]
alternative-framework = ["other-framework"]

[dependencies]
dashflow = { version = "1.11", optional = true }
other-framework = { version = "0.1", optional = true }
```

**Benefits:**
- Build without framework (for domain testing)
- Swap frameworks with feature flags
- Conditional compilation

---

### Pattern 3: Plugin Architecture

```rust
// src/plugins/mod.rs

pub trait WorkflowPlugin: Send + Sync {
    fn name(&self) -> &str;
    async fn execute(&self, state: &mut AppState) -> Result<()>;
}

// Users can add custom plugins without modifying core
```

---

## Interface Stability

### What You Should Depend On

**✅ STABLE (Safe to use):**
- Core traits: `ChatModel`, `Runnable`, `VectorStore`
- Message types: `Message`, `BaseMessage`
- Error types: `DashFlowError`, `Result`
- State trait: `GraphState`

**⚠️ MAY CHANGE:**
- Specific provider APIs (use via trait)
- Internal implementation details
- Helper functions
- Non-public APIs

**❌ NEVER DEPEND ON:**
- Internal modules (not pub)
- Implementation details
- Undocumented behavior
- Test utilities

---

## Portability Checklist

**Your app is portable if:**

- [ ] Domain logic in separate module/crate
- [ ] Framework accessed via thin adapters
- [ ] Configuration external (not hardcoded)
- [ ] Depends on stable traits, not concrete types
- [ ] Tests work with mocks (no framework needed)
- [ ] Can swap LLM providers via config
- [ ] Can swap checkpointers via config
- [ ] Zero `use dashflow_*` in domain code

**Example test:**
```rust
// Can you build domain without framework?
cargo build --package my-app-domain --no-default-features
// Should succeed!
```

---

## Framework Upgrade Guide

### Version 1.6 → 1.7 (Minor Update)

**Step 1:** Update Cargo.toml
```toml
dashflow = "1.7"  # Was "1.6"
```

**Step 2:** Check for API changes
```bash
cargo build
# Compiler errors show what changed
```

**Step 3:** Update adapters only
```rust
// src/framework/dashflow_adapter.rs
// Fix any API changes here
```

**Step 4:** Test
```bash
cargo test --workspace
```

**Step 5:** Deploy
```bash
cargo build --release
```

**If adapters are thin, this takes 30 minutes, not 30 hours!**

---

### Version 1.x → 2.0 (Major Update)

**Expect breaking changes:**
- Traits may have new methods
- Error types may change
- Some APIs removed

**Strategy:**
- Read CHANGELOG.md and migration guide
- Update adapters systematically
- May need to refactor adapters
- Domain logic should still be unchanged

**Your domain code protection pays off here!**

---

## Example: Production App Structure

### Domain Layer (Framework-Agnostic)

```rust
// src/domain/customer_service.rs

pub struct CustomerServiceWorkflow {
    // Use YOUR traits, not framework traits
    classifier: Box<dyn IntentClassifier>,
    router: Box<dyn Router>,
    specialists: HashMap<Intent, Box<dyn Agent>>,
}

impl CustomerServiceWorkflow {
    pub async fn handle_request(&self, request: CustomerRequest) -> Result<Response> {
        // 1. Classify intent
        let intent = self.classifier.classify(&request).await?;

        // 2. Route to specialist
        let specialist = self.router.route(&intent)?;

        // 3. Handle request
        let response = specialist.handle(&request).await?;

        Ok(response)
    }
}
```

**No `use dashflow_*` anywhere in domain!**

---

### Adapter Layer (Framework-Specific)

```rust
// src/adapters/dashflow_workflow.rs

pub struct DashFlowCustomerService {
    graph: CompiledGraph<CustomerServiceState>,
}

impl DashFlowCustomerService {
    pub fn new(config: &WorkflowConfig) -> Self {
        let mut graph = StateGraph::new();

        // Map YOUR domain to framework
        graph.add_node_from_fn("classify", |state| {
            // Adapter logic
        });

        graph.add_conditional_edges(...);

        Self {
            graph: graph.compile().unwrap(),
        }
    }
}

impl CustomerServiceWorkflow for DashFlowCustomerService {
    async fn handle_request(&self, request: CustomerRequest) -> Result<Response> {
        // Adapt domain → framework → domain
        let framework_state = self.to_framework_state(request);
        let result = self.graph.invoke(framework_state).await?;
        Ok(self.from_framework_state(result))
    }
}
```

**All framework code isolated here**

---

## Migration Example

### Your App (v1.6)

```rust
// src/adapters/dashflow_v1_6.rs

use dashflow_openai::ChatOpenAI;  // v1.6 API

impl LLMAdapter {
    async fn generate(&self, prompt: &str) -> Result<String> {
        let result = self.inner.generate(&messages, None).await?;
        //                                         ^^^^ v1.6 API
        Ok(result.text())
    }
}
```

### Framework Updates to v1.7

```rust
// src/adapters/dashflow_v1_7.rs

use dashflow_openai::ChatOpenAI;  // v1.7 API (hypothetical change)

impl LLMAdapter {
    async fn generate(&self, prompt: &str) -> Result<String> {
        let result = self.inner.generate(&messages).with_options(opts).await?;
        //                                         ^^^^^^^^^^^^^^^^^ v1.7 API
        Ok(result.text())
    }
}
```

**Your domain code:**
```rust
// src/domain/research.rs
// UNCHANGED - Still calls adapter.generate("prompt")
```

---

## Best Practices

### DO ✅

- **Depend on traits, not structs**
  ```rust
  fn process(llm: &dyn ChatModel)  // Good
  fn process(llm: &ChatOpenAI)     // Bad - coupled to provider
  ```

- **Configuration over hardcoding**
  ```rust
  let model = config.llm.model;    // Good
  let model = "gpt-4o";            // Bad - can't change
  ```

- **Domain-first design**
  ```rust
  struct MyWorkflow { ... }        // Good - YOUR type
  struct DashFlowWrapper { ... }  // Bad - framework-first
  ```

- **Adapter pattern**
  ```rust
  trait MyLLM { ... }              // Good - your interface
  impl MyLLM for DashFlowLLM      // Adapter
  ```

### DON'T ❌

- **Don't spread framework types through app**
  ```rust
  // Bad - DashFlow types in domain
  pub struct MyApp {
      graph: StateGraph<MyState>,  // Framework type leaked!
  }
  ```

- **Don't hardcode framework specifics**
  ```rust
  // Bad - hardcoded
  let llm = ChatOpenAI::new().with_model("gpt-4o");

  // Good - configured
  let llm = config.create_llm();
  ```

- **Don't test framework, test YOUR logic**
  ```rust
  // Bad - testing framework
  #[test]
  fn test_dashflow_works() {
      let graph = StateGraph::new();
      // ...testing framework, not your app
  }

  // Good - testing your logic
  #[test]
  fn test_research_classifies_correctly() {
      let workflow = create_with_mocks();
      // ...testing YOUR logic
  }
  ```

---

## Real-World Example: Customer Service Bot

### Structure

```rust
// Domain (framework-agnostic)
pub struct CustomerServiceBot {
    intent_classifier: Box<dyn IntentClassifier>,
    specialists: HashMap<Intent, Box<dyn Specialist>>,
}

// Framework adapter
pub struct DashFlowIntentClassifier {
    graph: CompiledGraph<ClassifierState>,
}

impl IntentClassifier for DashFlowIntentClassifier {
    async fn classify(&self, request: &str) -> Result<Intent> {
        // Use DashFlow here
    }
}

// Configuration
let config = load_config("prod.toml");
let bot = CustomerServiceBot::new(
    Box::new(DashFlowIntentClassifier::from_config(&config)),
    create_specialists(&config),
);

// Use
let response = bot.handle(customer_message).await?;
```

**When framework updates:**
- Update `DashFlowIntentClassifier` implementation
- `CustomerServiceBot` unchanged
- Tests unchanged
- Deploy

---

## Summary

**Key Principles:**

1. **Layer your app** (domain → adapter → framework)
2. **Depend on traits** (not concrete types)
3. **Configure, don't hardcode** (external config)
4. **Test domain with mocks** (fast, no framework)
5. **Isolate framework** (adapters only)

**Benefits:**

✅ Upgrade framework easily (update adapters)
✅ Swap providers (update config)
✅ Test without framework (mock adapters)
✅ Domain logic reusable (no framework coupling)
✅ Clear boundaries (easy to understand)
✅ Modular (each layer independent)

**Framework = Tool, not foundation**

**Your domain logic is the foundation.**

---

**Status:** Framework is powerful, now apps need to use it wisely.

**Author:** Andrew Yates © 2026
