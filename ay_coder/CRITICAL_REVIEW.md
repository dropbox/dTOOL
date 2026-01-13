# Critical Review: Top 10 Problems and Gaps in AY Coder Design

**Date**: November 19, 2025
**Reviewer**: Self (skeptical mode)
**Context**: User has been "most successful with Claude Code" - what am I missing?

---

## Preamble: The Fundamental Problem

**I don't actually know how Claude Code works internally.**

The GitHub repo (github.com/anthropics/claude-code) is:
- ❌ **Proprietary** - "All rights reserved"
- ❌ **Only has plugins/scripts** - NOT the core implementation
- ❌ **Closed source** - Can't see actual architecture

So my entire architectural synthesis is based on:
- ✅ Aider (open source Python)
- ✅ Continue.dev (open source TypeScript)
- ✅ OpenAI Codex (open source Rust)
- ✅ OpenHands (open source Python)
- ✅ Plandex (open source Go)

But **NOT** the one system the user says actually works well for them.

---

## TOP 10 PROBLEMS AND GAPS

### 1. ❌ **Over-Engineering: Library-First is Developer-First, Not User-First**

**The Problem**:
```rust
// My proposed design
ay_coder/
├── crates/
│   ├── core/              # Library abstraction
│   ├── cli/               # CLI interface
│   ├── tui/               # TUI interface
│   └── mcp-server/        # MCP server
```

**Why This Is Wrong**:
- Claude Code is **monolithic and focused** - you just run `claude code` and it works
- I'm building a "flexible framework" when users want a "tool that just works"
- The user hasn't asked for a library, SDK, or MCP server
- I'm solving problems that don't exist yet

**Evidence**:
- Aider's success comes from **simplicity**: one Python script, no fancy architecture
- Claude Code likely has similar focus: deep integration, not broad flexibility

**The Gap**:
I'm building for extensibility and multiple interfaces when I should build for:
1. One thing that works really well
2. Fast iteration
3. User feedback loops

**Fix**:
Start with a single focused CLI, not a workspace with 4 crates.

---

### 2. ❌ **Multi-Model is Premature: Quality Over Quantity**

**The Problem**:
```rust
pub struct ModelRouter {
    providers: HashMap<String, Arc<dyn ModelProvider>>,
    routing_strategy: RoutingStrategy,
}
```

**Why This Is Wrong**:
- Claude Code uses **one model** (Claude) and it works extremely well
- I'm proposing support for Claude, GPT, Gemini, local models, auto-routing, etc.
- This adds massive complexity for questionable value
- **Good integration with one model > mediocre integration with many**

**Evidence**:
- User says they've been "most successful with Claude Code"
- Not "I wish Claude Code supported more models"
- The success comes from deep Claude integration, not model variety

**The Gap**:
I'm adding complexity (trait objects, async trait, provider abstraction, routing strategies) when I should:
1. Pick ONE model (Claude Sonnet 4)
2. Integrate it deeply and well
3. Add other models ONLY if users ask

**Cost of Multi-Model**:
- More code to maintain
- More bugs
- Slower iteration
- Each provider needs testing
- Configuration complexity
- User choice paralysis

**Fix**:
Start with direct Anthropic API integration. Add providers only when proven necessary.

---

### 3. ❌ **Context Management: Solving a Problem I Don't Understand**

**The Problem**:
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
        // 300 lines of complex logic
    }
}
```

**Why This Is Wrong**:
- I don't know what context problems users actually face
- I'm implementing "2M token support" from Plandex, but:
  - Does the user need 2M tokens?
  - What's the real problem being solved?
  - How does Claude Code handle context?

**The Gap**:
I'm building for a hypothetical problem ("what if we need to load 500 files?") when real users might have different needs:
- "I just want to edit this one file"
- "Help me understand this function"
- "Add tests to this module"

**Evidence**:
Claude Code probably keeps context management simple and lets Claude's 200k context do the heavy lifting.

**Fix**:
Start naive:
1. Include only files explicitly mentioned
2. Include recent conversation
3. Let Claude request more context if needed
4. **Iterate based on real usage**, not hypothetical edge cases

---

### 4. ❌ **Rust: Wrong Language for Iteration Speed**

**The Problem**:
I'm proposing Rust for a tool that needs rapid iteration and user feedback.

**Why This Is Wrong**:
- **OpenAI Codex in Rust**: Yes, but OpenAI has resources and mature product
- **Aider in Python**: 38.5k stars, one developer, fast iteration
- **Claude Code**: Likely TypeScript (Node.js based) for fast development

**Rust Costs**:
- ❌ Longer compile times (even with incremental)
- ❌ Steeper learning curve for contributors
- ❌ More time fighting the borrow checker than solving user problems
- ❌ Harder to prototype and experiment
- ❌ Async + trait objects + error handling complexity

**Rust Benefits**:
- ✅ Performance (but this tool is I/O bound, not CPU bound)
- ✅ Safety (but Python works fine for Aider)
- ✅ Binary distribution (but `npm install` or `pip install` work fine)

**The Gap**:
I chose Rust because it's cool and I want to learn it, not because it's the right tool for the job.

**Honest Assessment**:
The user needs a tool that:
1. Works today
2. Can be fixed quickly
3. Can incorporate feedback fast

Rust optimizes for:
1. Performance at scale
2. Safety in production
3. Long-term maintenance

We're at phase 0, not production scale.

**Fix**:
Consider TypeScript/Node.js or Python for v1. Port to Rust later if performance becomes an issue.

---

### 5. ❌ **Missing the Core Workflow: What Does "Successful" Mean?**

**The Problem**:
I've designed architecture without understanding the actual user workflow.

**Questions I Haven't Answered**:
- How does the user actually start a session?
- What does a typical interaction look like?
- When does the user feel "successful"?
- What frustrates users about current tools?
- Why does Claude Code work better than others?

**My Design Assumes**:
```rust
// Hypothetical workflow I'm designing for
ay-coder
> User types prompt
> Agent thinks, uses tools, edits files
> User accepts/rejects changes
> Auto-commit to git
```

**But What If Real Workflow Is**:
```
// What actually happens with Claude Code?
$ claude code
> Describe what you want
> Claude asks clarifying questions
> Claude shows a plan
> User approves
> Claude makes changes iteratively
> User reviews each change
> Claude explains what it did
> User continues conversation
```

**The Gap**:
I'm building infrastructure without understanding the user journey.

**Fix**:
1. Use Claude Code for a week
2. Document actual workflows
3. Identify pain points
4. Design to solve THOSE problems

---

### 6. ❌ **Git Auto-Commit: Assuming What Users Want**

**The Problem**:
```rust
pub fn auto_commit(&self, edited_files: &[PathBuf]) -> Result<()> {
    if !self.auto_commit {
        return Ok(());
    }
    // ... create commit ...
}
```

**Why This Is Wrong**:
I'm copying Aider's auto-commit feature without understanding:
- Do users actually want this?
- When is it helpful vs annoying?
- How does Claude Code handle this?
- What control do users want?

**Alternative Designs**:
1. **Never auto-commit**: User commits manually (full control)
2. **Always auto-commit**: Aider's approach (convenience)
3. **Ask every time**: Claude Code's approval flow
4. **Smart commit**: Only commit when logical unit is complete

**The Gap**:
I don't know which approach users prefer because I haven't used any of these tools in real projects.

**Fix**:
Make this configurable and test with real users. Default to safe (no auto-commit).

---

### 7. ❌ **Tool System: MCP is Premature Optimization**

**The Problem**:
```rust
pub struct ToolRegistry {
    builtin: HashMap<String, Arc<dyn Tool>>,
    mcp_clients: Vec<McpClient>,
}
```

**Why This Is Wrong**:
- MCP (Model Context Protocol) is cool but adds complexity
- I need maybe 5 tools: read_file, write_file, run_command, search, git
- Building a plugin system before having any plugins
- OpenAI Codex has MCP but it's a mature product

**The Gap**:
I'm solving tomorrow's problems (extensibility) instead of today's problems (does it work?).

**Cost of MCP**:
- JSON-RPC protocol
- Subprocess management
- Tool discovery
- Error handling across process boundaries
- Security/sandboxing

**What I Actually Need**:
```rust
enum Tool {
    ReadFile { path: PathBuf },
    WriteFile { path: PathBuf, content: String },
    RunCommand { command: String },
    Search { pattern: String },
}
```

**Fix**:
Start with 5 built-in tools as an enum. Add MCP only when users ask for external integrations.

---

### 8. ❌ **Sandboxing: Security Theater vs Usability**

**The Problem**:
```rust
pub struct SandboxManager {
    executor: Box<dyn SandboxExecutor>,
    mode: SandboxMode,
}

#[cfg(target_os = "macos")]
pub struct MacOSSandbox { /* Seatbelt via sandbox-exec */ }

#[cfg(target_os = "linux")]
pub struct LinuxSandbox { /* Landlock + seccomp */ }
```

**Why This Is Wrong**:
- Platform-specific sandboxing is HARD
- macOS Seatbelt requires root or entitlements
- Linux Landlock needs kernel 5.13+
- Windows AppContainer is experimental
- All of this is complex and fragile

**The User's Actual Threat Model**:
- They're running this on their own machine
- They're editing their own code
- They trust Claude to not be malicious
- They want to review changes before they happen

**Real Security Needs**:
1. **Ask before destructive operations** (rm, git push)
2. **Show what will change before changing it**
3. **Easy rollback** (git-based)

**The Gap**:
I'm building OS-level sandboxing when users need approval workflows.

**Fix**:
1. Start with approval gates (Ask before running any command)
2. Show diffs before writing files
3. Only add sandboxing if there's a real security need

---

### 9. ❌ **Async Everywhere: Complexity Without Benefit**

**The Problem**:
```rust
#[async_trait]
pub trait ModelProvider: Send + Sync {
    async fn complete(&self, request: &CompletionRequest) -> Result<Response>;
}

impl Coder {
    pub async fn send_message(&mut self, message: Message) -> Result<Response> {
        // async/await everywhere
    }
}
```

**Why This Is Wrong**:
- This tool makes ONE API call at a time
- There's no real concurrency (user waits for response)
- Async is infectious (everything becomes async)
- Adds complexity: Pin, Send, Sync, 'static lifetimes

**Where Async Helps**:
- ✅ Streaming responses (show output as it arrives)
- ✅ Parallel tool execution (maybe)

**Where Async Hurts**:
- ❌ Makes code harder to read
- ❌ Harder to debug
- ❌ More boilerplate
- ❌ Borrow checker fights

**The Gap**:
I'm using async because "modern Rust code is async" not because the problem needs it.

**Alternative**:
Use blocking HTTP client, spawn threads for true parallelism when needed.

**Fix**:
Start with blocking I/O. Add async only where it provides clear value (streaming).

---

### 10. ❌ **Not Built for AI-First Development: Missing the Meta-Problem**

**The BIGGEST Problem**:

I'm designing a tool to help developers code faster with AI... but I'm not using AI to build it.

**The Irony**:
- I'm writing detailed specifications
- Planning 12-week implementation phases
- Designing trait hierarchies
- Worrying about multi-model support

**What I Should Be Doing**:
1. Write a simple Python script
2. Use Claude Code to iterate on it
3. Add features based on what I actually need
4. Let AI help me build the AI tool

**The Gap**:
I'm in architect mode when I should be in builder mode.

**Evidence from Aider**:
- Started simple
- Grew organically
- 38.5k stars
- One developer
- Fast iteration

**What Claude Code Probably Does Well**:
- ✅ Gets out of your way
- ✅ Helps you think through problems
- ✅ Makes it easy to iterate
- ✅ Explains what it's doing
- ✅ Handles failure gracefully

**Fix**:
1. Build a 200-line Python script that calls Claude API
2. Use it to build itself better
3. Add features based on real pain points
4. THEN consider Rust rewrite if needed

---

## Summary: What Am I Actually Building?

### What I Proposed:
- ❌ Multi-model AI coding framework in Rust
- ❌ Library-first architecture
- ❌ MCP protocol support
- ❌ Platform-specific sandboxing
- ❌ 2M token context management
- ❌ CLI + TUI + MCP server
- ❌ 12-week implementation plan

### What Users Probably Need:
- ✅ Simple tool that works with Claude
- ✅ Edits files reliably
- ✅ Explains what it's doing
- ✅ Easy to review and rollback
- ✅ Fast iteration
- ✅ Gets out of the way

### Honest Questions for the User:

1. **When you use Claude Code, what makes you "successful"?**
   - Faster coding?
   - Better code quality?
   - Less frustration?
   - Specific features?

2. **What frustrates you about Claude Code?**
   - Anything missing?
   - Workflows that don't work well?
   - Features you wish existed?

3. **What would make you switch to a different tool?**
   - Specific missing features?
   - Better experience in some way?
   - Cost/pricing?

4. **Do you actually need:**
   - Multiple model support?
   - 2M token context?
   - MCP protocol?
   - Rust performance?
   - TUI interface?

---

## Recommended Path Forward

### Option 1: Start Minimal (Recommended)

**Week 1-2: Python MVP**
```python
# claude_coder.py (200 lines)
import anthropic
import sys

def main():
    client = anthropic.Anthropic()

    while True:
        user_input = input("ay> ")

        response = client.messages.create(
            model="claude-sonnet-4-5",
            messages=[{"role": "user", "content": user_input}],
            tools=[read_file_tool, write_file_tool],
        )

        # Handle tool calls, show results, iterate

if __name__ == "__main__":
    main()
```

**Test with real use cases**:
- Edit a file
- Add tests
- Refactor code
- Fix bugs

**Iterate based on what doesn't work**.

### Option 2: Fork and Extend

Fork one of the open-source tools:
- **Aider** if you want proven CLI patterns
- **Continue.dev** if you want IDE integration
- **OpenAI Codex** if you want Rust

Add the features you actually need.

### Option 3: Full Build (Original Plan)

Only do this if:
- You've used existing tools and found specific gaps
- You know exactly what problems you're solving
- You have time for 12+ weeks of development
- Rust is truly necessary

---

## Conclusion: The Gap I Missed

**I optimized for technical elegance over user success.**

I built an architecture that's:
- ✅ Well-designed
- ✅ Extensible
- ✅ Type-safe
- ✅ Performant

But probably:
- ❌ Over-engineered
- ❌ Slow to build
- ❌ Solving wrong problems
- ❌ Missing what makes Claude Code successful

**The real question is: What does the user actually need?**

Not what can I design. What will make them successful.
