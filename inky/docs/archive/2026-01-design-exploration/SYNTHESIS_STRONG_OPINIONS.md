# Synthesis: Strong Opinions, Loosely Held

**Date:** 2026-01-01
**Key Insight:** "I already have a great freedom-first platform: no platform."

---

## The Realization

The AI critique said: *"Give me control, don't decide for me."*

The user's response: *"If you want total freedom, use no framework."*

**This is correct.** A framework without opinions is just a library. Rails beat Sinatra for most apps because opinions reduce decisions. React's "one-way data flow" is a constraint that makes code predictable.

**The question isn't whether to have opinions. It's which opinions to have.**

---

## Reconciliation: What's Actually Aligned

| Thesis | Critique | Resolution |
|--------|----------|------------|
| Streaming by default | Sometimes want replace/batch | **Opinion: Append-only output with named regions that can replace** |
| Content-aware rendering | I want control | **Opinion: Smart defaults, explicit override with same API** |
| Conversation model | Non-linear reality | **Opinion: Turn-based with first-class interruption** |
| Multi-agent | Premature complexity | **Opinion: Single-agent default, multi-agent as composition** |
| Progressive complexity | "Just works" is a lie | **Opinion: Same API at all levels, verbosity scales** |

The thesis and critique are closer than they appear. The gap is about **which opinions** and **how strongly to hold them**.

---

## The Strong Opinions

### Opinion 1: Output is Append-Only, Regions Can Replace

```rust
// Default: append to output (streaming AI text)
terminal.append("Processing your request...");
terminal.append(" done!");
// Result: "Processing your request... done!"

// Named region: can be replaced
let status = terminal.region("status");
status.set("Loading...");
status.set("50% complete");  // Replaces, doesn't append
status.set("Done!");

// Streaming within a region
let response = terminal.region("response");
response.stream(ai_output).await;  // Appends within region
```

**Why this opinion:** Most AI output is streaming text that appends. Status indicators need replacement. This handles both with a simple model.

**Escape hatch:** `terminal.clear()` for full reset.

---

### Opinion 2: Smart Defaults, Explicit Overrides, Same API

```rust
// Level 0: Smart default
terminal.show(Code::new("fn main() {}"));
// → Syntax highlighted (detected as Rust), scrollable

// Level 1: Explicit language (same API, more specific)
terminal.show(Code::new("fn main() {}").language("rust"));

// Level 2: Disable highlighting (same API, different value)
terminal.show(Code::new("fn main() {}").highlight(false));

// Level 3: Full control (same API, all options)
terminal.show(Code::new("fn main() {}")
    .language("rust")
    .highlight(true)
    .line_numbers(true)
    .theme(Theme::Monokai)
    .max_height(20));
```

**Why this opinion:** Defaults should be good. Overrides should be discoverable. Same API structure at all levels means no "level jumping."

**The key:** It's not `.basic()` vs `.advanced()`. It's the same method with optional parameters.

---

### Opinion 3: Turn-Based with First-Class Interruption

```rust
// Default: turn-based conversation
terminal.ai("I'll analyze the codebase...").await;
let response = terminal.human().await;

// But interruption is handled:
let output = terminal.ai_interruptible("I'll analyze...").await;
match output {
    Completed(text) => { /* AI finished */ }
    Interrupted { partial, reason } => {
        // partial = what AI said before interrupt
        // reason = why (user input, timeout, etc.)
        terminal.ai("I see you want to change direction...").await;
    }
}

// The opinion: interruption is NORMAL, not exceptional
```

**Why this opinion:** Real conversations have interruption. Treating it as an exception (try/catch) makes it awkward. Treating it as a variant (enum) makes it natural.

---

### Opinion 4: Single-Agent Default, Multi-Agent via Composition

```rust
// Default: one agent (simple)
let terminal = Terminal::new();
terminal.ai("Hello!").await;

// Multi-agent: compose terminals into workspace
let workspace = Workspace::new()
    .agent("coder", Terminal::new())
    .agent("reviewer", Terminal::new())
    .layout(Layout::SideBySide);  // Visual arrangement

// Each agent has its own terminal (region)
workspace.get("coder").ai("I'll write the code...").await;
workspace.get("reviewer").ai("I'll review...").await;
```

**Why this opinion:** Most apps need one agent. Multi-agent is composition of single agents, not a different paradigm. You don't learn a new API—you compose what you know.

---

### Opinion 5: History Is Managed, With Explicit Lifecycle

```rust
// Default: history is kept in memory
terminal.ai("First message").await;
terminal.ai("Second message").await;
terminal.history();  // → [Turn::Ai("First..."), Turn::Ai("Second...")]

// Persist to disk (opt-in)
let terminal = Terminal::new()
    .persist("~/.myapp/session.json");

// Limits (opt-in)
let terminal = Terminal::new()
    .max_history(100)  // Oldest dropped
    .max_memory(10_000_000);  // 10MB cap

// Branching (opt-in)
let branch = terminal.branch();  // Fork history
branch.ai("What if we tried...").await;
// Original terminal unchanged
```

**Why this opinion:** History management is crucial but varies by app. Defaults work (in-memory, no limits). Production apps configure limits and persistence.

---

### Opinion 6: Content Types Are Traits, Built-ins Are Canonical

```rust
// Built-in types (canonical implementations)
terminal.show(Code::new(src));     // Code with syntax highlighting
terminal.show(Diff::new(a, b));    // Diff view
terminal.show(Table::new(data));   // Data table
terminal.show(Progress::new(0.5)); // Progress bar

// Custom types implement trait
trait Content {
    fn render(&self, ctx: &mut RenderContext);
    fn fallback(&self) -> Text;  // For basic terminals
    fn accessible(&self) -> AccessibleContent;
}

struct MermaidDiagram(String);
impl Content for MermaidDiagram {
    fn render(&self, ctx: &mut RenderContext) {
        if ctx.supports_graphics() {
            ctx.draw_image(self.render_to_png());
        } else {
            ctx.draw_text(&self.to_ascii());
        }
    }
    fn fallback(&self) -> Text { Text::new(self.0.clone()) }
    fn accessible(&self) -> AccessibleContent { ... }
}

terminal.show(MermaidDiagram("graph TD; A-->B".into()));
```

**Why this opinion:** Common content types should be built-in with great defaults. But the set of content types is open—anyone can implement `Content`.

---

### Opinion 7: Accessibility Is Structural, Not Annotations

```rust
// NOT this (bolted on):
terminal.show(Code::new(src).aria_label("Source code"));

// THIS (structural):
terminal.show(Code::new(src));
// Framework KNOWS it's code, automatically:
// - Announces "code block" to screen reader
// - Provides navigation by line
// - Allows copy without line numbers

// Questions are inherently accessible:
terminal.ask("Continue?");
// Framework KNOWS it's a question, automatically:
// - Announces "question: Continue?"
// - Provides "yes" and "no" as options
// - Captures Enter as yes, Escape as no
```

**Why this opinion:** Accessibility from annotations is always incomplete. Accessibility from structure is always correct. The framework knows what things ARE, so it can make them accessible automatically.

---

### Opinion 8: Testing Is First-Class

```rust
// MockTerminal captures everything
let mut mock = MockTerminal::new();

// Run your code
my_ai_app(&mut mock).await;

// Assert on output
assert_eq!(mock.output(), vec![
    Output::Text("Hello!"),
    Output::Code { language: "rust", content: "fn main() {}" },
]);

// Simulate input
mock.queue_input("yes");
let answer = mock.ask("Continue?").await;
assert!(answer.is_yes());

// Snapshot testing
assert_snapshot!(mock.rendered());  // Visual snapshot
```

**Why this opinion:** If it's not testable, it's not production-ready. `MockTerminal` is a first-class citizen, not an afterthought.

---

## The Framework's Personality

Putting it all together, this framework is:

| Trait | Manifestation |
|-------|---------------|
| **Opinionated** | Smart defaults that work for 80% of cases |
| **Escape-hatched** | Override anything when you need to |
| **Consistent** | Same API shape at all complexity levels |
| **Composable** | Multi-agent = multiple single-agents composed |
| **Interruptible** | Non-linear reality is first-class |
| **Testable** | MockTerminal from day one |
| **Accessible** | Structural, not annotated |
| **Extensible** | Traits for content, not closed enums |

---

## What We're NOT Opinionated About

Some things should be left to the user:

| Decision | Left to User |
|----------|--------------|
| Which AI model | Framework doesn't care |
| Persistence format | JSON, SQLite, whatever |
| Visual theme | Your brand, your colors |
| Key bindings | Your muscle memory |
| Networking | Your stack, your auth |

**Strong opinions about the terminal. No opinions about your business.**

---

## The Litmus Test

A good framework opinion passes this test:

1. **Common case is delightful:** 80% of users get exactly what they need
2. **Override is obvious:** When you need different, you know how to ask
3. **Escape hatch exists:** For the 1%, raw access is available
4. **Composition works:** Building blocks combine predictably

If an opinion fails these tests, it's not a good opinion. Remove it or refine it.

---

## Final Positioning

```
┌─────────────────────────────────────────────────────────────┐
│                                                             │
│   "No platform" ←───────────────────────────→ "Our way"    │
│                                                             │
│        │                                          │         │
│        │                                          │         │
│     Total freedom                            Total control  │
│     Total decisions                          Total opinions │
│     Total responsibility                     Total coherence│
│                                                             │
│                         ┌───────┐                           │
│                         │ inky  │                           │
│                         └───────┘                           │
│                              │                              │
│                              ▼                              │
│                                                             │
│              Strong opinions about:                         │
│              • Terminal interaction patterns                │
│              • Content rendering                            │
│              • Accessibility                                │
│              • Testing                                      │
│                                                             │
│              No opinions about:                             │
│              • Your AI model                                │
│              • Your business logic                          │
│              • Your visual brand                            │
│              • Your infrastructure                          │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

**The value proposition:** We've thought hard about terminal UIs so you don't have to. Follow our opinions and get a great result. Override when your needs diverge. We're not the platform—we're the accelerator.
