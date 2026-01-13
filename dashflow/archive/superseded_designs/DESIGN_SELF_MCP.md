# Design: DashFlow Self-MCP Server

**Created:** 2025-12-12
**Status:** DESIGN
**Goal:** Make DashFlow self-documenting for AIs working ON it and WITH it

---

## The Two AI Personas

### 1. AI Developer (working ON DashFlow)
Questions they ask:
- "What modules exist in DashFlow?"
- "Where is distillation implemented?"
- "Is the train CLI wired up?"
- "What's the coding pattern for adding a new node type?"
- "What tests cover this module?"

### 2. AI Builder (working WITH DashFlow)
Questions they ask:
- "How do I create a StateGraph?"
- "What tools are available?"
- "Show me an example of checkpointing"
- "How do I add a custom node?"
- "What's the best way to handle errors?"

---

## Solution: Self-MCP Server

DashFlow exposes an MCP server that serves information about itself:

```
┌─────────────────────────────────────────────────────────────────┐
│                     AI Assistant (Claude)                        │
│                                                                  │
│  "What modules does DashFlow have?"                              │
│  "How do I use distillation?"                                    │
│  "Show me the StateGraph API"                                    │
└─────────────────────────────────────────────────────────────────┘
                              │
                              │ MCP Protocol
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                  DashFlow Self-MCP Server                        │
│                                                                  │
│  Tools:                                                          │
│  ├── list_modules()        → All modules with descriptions       │
│  ├── get_module(name)      → Detailed module info + examples     │
│  ├── search_api(query)     → Find APIs by keyword                │
│  ├── get_example(topic)    → Working code examples               │
│  ├── explain_pattern(name) → Coding patterns/conventions         │
│  ├── get_cli_mapping(cmd)  → CLI → library mapping               │
│  └── check_status(feature) → Implementation status               │
│                                                                  │
│  Resources:                                                      │
│  ├── dashflow://modules    → Module registry                     │
│  ├── dashflow://examples   → Example code                        │
│  ├── dashflow://patterns   → Coding patterns                     │
│  └── dashflow://status     → Implementation status               │
└─────────────────────────────────────────────────────────────────┘
                              │
                              │ Reads from
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    DashFlow Codebase                             │
│                                                                  │
│  - Source files with @dashflow-module markers                    │
│  - Doc comments                                                  │
│  - Examples directory                                            │
│  - Generated platform_registry                                   │
└─────────────────────────────────────────────────────────────────┘
```

---

## MCP Tools

### For AI Developers (working ON DashFlow)

#### `dashflow_list_modules`
```json
{
  "name": "dashflow_list_modules",
  "description": "List all modules in DashFlow with their categories and status",
  "parameters": {
    "category": { "type": "string", "optional": true },
    "status": { "type": "string", "optional": true }
  }
}
```

Response:
```json
{
  "modules": [
    {
      "name": "distillation",
      "path": "optimize/distillation",
      "category": "optimize",
      "status": "stable",
      "cli_command": "dashflow train distill",
      "description": "Teacher-student model distillation"
    }
  ]
}
```

#### `dashflow_get_implementation_status`
```json
{
  "name": "dashflow_get_implementation_status",
  "description": "Check if a feature is implemented, stubbed, or missing",
  "parameters": {
    "feature": { "type": "string", "required": true }
  }
}
```

Response:
```json
{
  "feature": "distillation",
  "library_status": "implemented",
  "cli_status": "stub",
  "cli_command": "dashflow train distill",
  "library_path": "optimize/distillation/mod.rs",
  "cli_path": "dashflow-cli/src/commands/train.rs:227",
  "action_needed": "Wire CLI to library implementation"
}
```

#### `dashflow_find_code`
```json
{
  "name": "dashflow_find_code",
  "description": "Find where something is implemented",
  "parameters": {
    "query": { "type": "string", "required": true }
  }
}
```

#### `dashflow_get_coding_pattern`
```json
{
  "name": "dashflow_get_coding_pattern",
  "description": "Get the coding pattern for a common task",
  "parameters": {
    "pattern": { "type": "string", "enum": [
      "add_new_node_type",
      "add_new_tool",
      "add_cli_command",
      "add_test",
      "error_handling",
      "async_pattern"
    ]}
  }
}
```

### For AI Builders (working WITH DashFlow)

#### `dashflow_create_graph_example`
```json
{
  "name": "dashflow_create_graph_example",
  "description": "Get a working example of creating a graph",
  "parameters": {
    "complexity": { "type": "string", "enum": ["simple", "medium", "advanced"] },
    "features": { "type": "array", "items": { "type": "string" } }
  }
}
```

#### `dashflow_api_reference`
```json
{
  "name": "dashflow_api_reference",
  "description": "Get API reference for a type or function",
  "parameters": {
    "name": { "type": "string", "required": true }
  }
}
```

Response:
```json
{
  "name": "StateGraph",
  "kind": "struct",
  "module": "dashflow::graph",
  "description": "A state graph that manages workflow execution",
  "methods": [
    {
      "name": "new",
      "signature": "pub fn new() -> Self",
      "description": "Create a new empty state graph"
    },
    {
      "name": "add_node",
      "signature": "pub fn add_node<F>(&mut self, name: &str, func: F)",
      "description": "Add a node to the graph"
    }
  ],
  "example": "let mut graph = StateGraph::new();\ngraph.add_node(\"start\", |state| { ... });"
}
```

#### `dashflow_explain_concept`
```json
{
  "name": "dashflow_explain_concept",
  "description": "Explain a DashFlow concept",
  "parameters": {
    "concept": { "type": "string", "enum": [
      "state_graph",
      "nodes_and_edges",
      "checkpointing",
      "streaming",
      "tools",
      "distillation"
    ]}
  }
}
```

---

## MCP Resources

### `dashflow://modules`
Full module registry as JSON

### `dashflow://modules/{name}`
Detailed info for a specific module

### `dashflow://examples`
List of all examples with descriptions

### `dashflow://examples/{name}`
Full source code for an example

### `dashflow://patterns`
List of coding patterns

### `dashflow://patterns/{name}`
Detailed pattern with template code

### `dashflow://api/{type}`
API reference for a type

---

## Implementation Architecture

```
crates/
├── dashflow-mcp-server/           <- NEW: Self-MCP server
│   ├── src/
│   │   ├── main.rs                <- MCP server entry point
│   │   ├── tools/
│   │   │   ├── modules.rs         <- list_modules, get_module
│   │   │   ├── search.rs          <- search_api, find_code
│   │   │   ├── examples.rs        <- get_example, create_graph_example
│   │   │   ├── patterns.rs        <- get_coding_pattern
│   │   │   └── status.rs          <- get_implementation_status
│   │   ├── resources/
│   │   │   ├── modules.rs         <- dashflow://modules
│   │   │   ├── examples.rs        <- dashflow://examples
│   │   │   └── api.rs             <- dashflow://api
│   │   └── data/
│   │       ├── registry.rs        <- Load from platform_registry
│   │       ├── examples.rs        <- Load from examples/
│   │       └── patterns.rs        <- Coding patterns database
│   └── Cargo.toml
```

---

## Data Sources

### 1. Platform Registry (generated)
- Module list, descriptions, CLI mappings
- Generated from @dashflow-module markers

### 2. Examples Directory
- Working code examples
- Each example has metadata (complexity, features)

### 3. Coding Patterns Database
- Hand-curated patterns for common tasks
- Template code with placeholders

### 4. Source Code (live)
- For search and find operations
- Parsed on-demand

### 5. Doc Comments
- API reference extracted from rustdoc
- Or parsed directly from source

---

## Usage

### Claude Code Integration
```bash
# Add to Claude Code MCP config
{
  "mcpServers": {
    "dashflow": {
      "command": "dashflow",
      "args": ["mcp-server"],
      "env": {
        "DASHFLOW_ROOT": "/path/to/dashflow"
      }
    }
  }
}
```

### Direct Usage
```bash
# Start MCP server
dashflow mcp-server --port 3100

# Or stdio mode for Claude
dashflow mcp-server --stdio
```

---

## Benefits

### For AI Developers
| Before | After |
|--------|-------|
| "Is distillation implemented?" → grep, assume | Query `get_implementation_status("distillation")` → exact answer |
| "Where is X?" → find, grep, guess | Query `find_code("X")` → exact path |
| "What's the pattern?" → look at existing code | Query `get_coding_pattern("add_node")` → template |

### For AI Builders
| Before | After |
|--------|-------|
| "How do I...?" → read docs, examples | Query `create_graph_example({features: ["tools"]})` → working code |
| "What's the API?" → cargo doc | Query `api_reference("StateGraph")` → structured response |
| "What can DashFlow do?" → browse docs | Query `list_modules()` → complete list |

---

## Integration with Introspection

This builds on the introspection design:

```
@dashflow-module markers
        │
        ▼
Platform Registry (generated)
        │
        ├──────────────────────┐
        ▼                      ▼
Runtime Query API        MCP Server
(PlatformRegistry)       (dashflow mcp-server)
        │                      │
        ▼                      ▼
Internal use             External AI use
```

---

## Phases

### Phase 1: Core MCP Server
- Basic tool implementations
- Module listing
- Example retrieval

### Phase 2: AI Developer Tools
- Implementation status checking
- Code finding
- Pattern retrieval

### Phase 3: AI Builder Tools
- Graph creation wizard
- API reference
- Concept explanations

### Phase 4: Integration
- Claude Code configuration
- Documentation
- Testing with real AI usage

---

## Success Metrics

| Metric | Target |
|--------|--------|
| AI can list all modules | ✓ |
| AI can check implementation status | ✓ |
| AI can find any code | ✓ |
| AI can get working examples | ✓ |
| AI can learn patterns | ✓ |
| Zero incorrect assumptions about codebase | ✓ |

---

## Open Questions

1. **Should this be a separate crate or part of dashflow-cli?**
   - Separate: cleaner, optional dependency
   - CLI subcommand: easier distribution

2. **How to keep patterns database updated?**
   - Manual curation
   - Extract from good examples automatically

3. **Should we generate rustdoc JSON or parse source directly?**
   - Rustdoc JSON: complete, but nightly-only
   - Source parsing: stable, but more work

4. **How detailed should API reference be?**
   - Just signatures?
   - Full doc comments?
   - Examples for every method?
