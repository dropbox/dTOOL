# Thesis: The 2026 Terminal UI Framework

**Date:** 2026-01-01
**Status:** Working thesis, open to critique

---

## Core Thesis

The terminal in 2026 is not a command prompt. It is a **collaboration surface** where humans and AI agents work together. The UI framework should be designed for this reality.

---

## Foundational Principles

### 1. Conversation-Native, Not Form-Native

**Old model (2016):**
```rust
let name = input!("What's your name?").run()?;
let age = input!("How old are you?").run()?;
```

**New model (2026):**
```rust
// AI and human converse naturally
ai.say("I need some information to set up your project.").await;
let response = human.respond().await;
// AI parses natural language, asks follow-ups if needed
```

### 2. Streaming by Default

Everything streams. AI output arrives character by character. The framework handles:
- Incremental rendering
- Backpressure
- Interruption
- Progressive display

```rust
// Not this (blocking)
let result = compute_everything();
terminal.print(result);

// This (streaming)
let stream = ai.generate();
terminal.stream(stream).await;
```

### 3. Content-Aware Rendering

The framework understands content types and renders them appropriately:

```rust
ai.output(Content::Code { language: "rust", content: src });
// → Automatically syntax highlighted, scrollable, copyable

ai.output(Content::Diff { original, modified });
// → Automatically shows additions/deletions, line numbers

ai.output(Content::Data { format: "json", content: data });
// → Automatically formatted, collapsible, searchable
```

### 4. Progressive Complexity

```
Level 0: Just Works       ai.say("Hello")
Level 1: Typed Content    ai.show(Code { ... })
Level 2: Customized       ai.show(Code { ... }.theme(Dark))
Level 3: Custom Widget    ai.show(Custom(my_widget))
Level 4: Raw Terminal     terminal.write(escape_codes)
```

**80% of apps stay at Level 0-1.** Complexity is opt-in, not required.

### 5. Multi-Agent Ready

Multiple AI agents can collaborate in one terminal:

```rust
let workspace = Workspace::new()
    .agent("coder", CodingAgent::new())
    .agent("reviewer", ReviewAgent::new())
    .human();

// Framework handles:
// - Visual separation of agent outputs
// - Routing human input to correct agent
// - Agent-to-agent communication
```

### 6. Graceful Degradation

The framework detects terminal capabilities and adapts:

| Capability | Rich Terminal | Basic Terminal |
|------------|---------------|----------------|
| Images | Kitty protocol | ASCII art |
| Colors | 24-bit RGB | 16 colors |
| Unicode | Full | ASCII fallback |
| Mouse | Click regions | Keyboard only |

---

## Architecture

### Layer 1: Stream Core

```rust
pub struct Terminal {
    input: InputStream,    // Human input
    output: OutputStream,  // Display output
    capabilities: Caps,    // What this terminal supports
}
```

### Layer 2: Content Types

```rust
pub enum Content {
    Text(StyledText),
    Code(CodeBlock),
    Diff(DiffView),
    Data(DataView),
    Question(Question),
    Progress(Progress),
    Image(Image),
}
```

### Layer 3: Conversation

```rust
pub struct Conversation {
    terminal: Terminal,
    history: Vec<Turn>,
}

impl Conversation {
    async fn ai(&mut self, content: impl Stream<Item = Content>);
    async fn human(&mut self) -> Response;
    async fn ask(&mut self, question: Question) -> Answer;
}
```

### Layer 4: Workspace (Multi-Agent)

```rust
pub struct Workspace {
    terminal: Terminal,
    agents: HashMap<String, Agent>,
    human: HumanChannel,
}
```

---

## Key Design Decisions

### Why Streaming?
AI generates text token by token. Blocking until complete wastes time and feels unresponsive. Streaming is the natural model.

### Why Content Types?
The framework can make intelligent rendering decisions. Code gets highlighted. Diffs show changes. Data gets formatted. The AI doesn't have to specify HOW to render, just WHAT.

### Why Progressive Complexity?
Most apps are simple. Don't force everyone through configuration hell. Defaults should work. Escape hatches should exist for edge cases.

### Why Multi-Agent?
The future is multiple specialized AI agents collaborating. Building this into the architecture (not bolting it on) makes it natural.

---

## Example: Complete AI Coding Assistant

```rust
#[tokio::main]
async fn main() -> Result<()> {
    let workspace = Workspace::new()
        .agent("assistant", CodingAssistant::new())
        .human();

    loop {
        // Human says something
        let input = workspace.human().await;

        // AI responds (streams)
        let response = workspace.assistant.respond(&input);
        workspace.stream(response).await;

        // If AI needs approval
        if let Some(action) = workspace.assistant.pending_action() {
            let approved = workspace.ask("Apply these changes?").await;
            if approved {
                action.execute().await;
            }
        }
    }
}
```

---

## What This Framework Is NOT

- **Not a TUI framework for 2016** (forms, menus, widgets)
- **Not a React-for-terminals** (virtual DOM, components, hooks)
- **Not a game engine** (60fps rendering, sprites, physics)
- **Not a document renderer** (pagination, print layout)

It IS a **collaboration surface framework** for humans and AI.

---

## Success Criteria

1. **Simple things are simple:** `ai.say("Hello")` works with zero config
2. **AI content renders beautifully:** Code, diffs, data—automatically formatted
3. **Streaming feels natural:** No blocking, no buffering, instant feedback
4. **Multi-agent is easy:** Adding another agent is one line
5. **Escape hatches exist:** Can always drop to raw terminal
6. **Works everywhere:** SSH, tmux, basic terminals—with graceful degradation
