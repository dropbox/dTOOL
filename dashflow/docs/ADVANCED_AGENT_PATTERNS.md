# Advanced Agent Patterns Guide

**Last Updated:** 2026-01-02 (Worker #2337 - Add missing Last Updated headers)

**Version:** 1.11.3
**Author:** Andrew Yates © 2026
**Purpose:** Comprehensive guide to sophisticated AI agent architectures

---

## Overview

This guide covers **4 advanced agent patterns** beyond basic ReAct agents. Each pattern solves specific challenges in AI agent design.

| Pattern | Best For | Complexity | Use Case |
|---------|----------|------------|----------|
| **ReAct** | Simple tasks, tool use | ⭐ Low | "Search and summarize X" |
| **Plan & Execute** | Complex multi-step tasks | ⭐⭐ Medium | "Research topic and write 10-page report" |
| **Reflection** | Quality-critical outputs | ⭐⭐ Medium | "Write blog post (must be excellent)" |
| **Multi-Agent Debate** | Decisions requiring diverse perspectives | ⭐⭐⭐ High | "Should we invest in AI infrastructure?" |

---

## 1. ReAct Agent (Baseline)

### How It Works

**Loop:** Thought → Action → Observation (repeat until done)

```
User: "What is 15% of Tokyo's population?"

Thought: I need to find Tokyo's population
Action: search("Tokyo population 2025")
Observation: Tokyo has 14 million people

Thought: Now calculate 15%
Action: calculator("14000000 * 0.15")
Observation: 2100000

Thought: I have the answer
Action: Finish[2.1 million people]
```

### Code Example

```rust
use dashflow::core::agents::ReActAgent;
use dashflow_openai::ChatOpenAI;

let llm = ChatOpenAI::new().with_model("gpt-4o-mini");

let agent = ReActAgent::new(llm)
    .with_tools(vec![search_tool, calculator_tool])
    .with_max_iterations(10);

let result = agent.run("What is 15% of Tokyo's population?").await?;
```

### When to Use

✅ **Good for:**
- Simple 2-3 step tasks
- Tool use required
- Straightforward questions
- Quick responses needed

❌ **Not good for:**
- Complex multi-step tasks (use Plan & Execute)
- Quality-critical outputs (use Reflection)
- Decisions requiring debate (use Multi-Agent Debate)

---

## 2. Plan & Execute Agent (Task Decomposition)

### How It Works

**Phase 1: Planning** - Break complex task into steps
**Phase 2: Execution** - Execute each step systematically
**Phase 3: Adaptation** - Adjust plan if needed

```
User: "Research AI trends and write a comprehensive 10-page report"

=== PLANNING PHASE ===
Planner Agent:
  Step 1: Search for "AI trends 2025"
  Step 2: Identify top 5 trends
  Step 3: Deep research each trend
  Step 4: Organize findings by category
  Step 5: Write introduction (2 pages)
  Step 6: Write trend sections (5 pages)
  Step 7: Write analysis section (2 pages)
  Step 8: Write conclusion (1 page)
  Step 9: Review and edit

=== EXECUTION PHASE ===
Executor Agent:
  [Step 1] Searching... ✓ Found 50 articles
  [Step 2] Analyzing... ✓ Top 5: GenAI, Multimodal, Agents, Safety, Edge AI
  [Step 3] Deep dive... ✓ 10 pages of research notes
  [Step 4] Organizing... ✓ Structured by theme
  [Step 5] Writing intro... ✓ 2 pages complete
  [Step 6] Writing trends... ✓ 5 pages complete
  [Step 7] Writing analysis... ✓ 2 pages complete
  [Step 8] Writing conclusion... ✓ 1 page complete
  [Step 9] Reviewing... ✓ Report complete!

=== ADAPTATION (if needed) ===
  If step fails → Replanning
  If new info → Adjust remaining steps
```

### Code Example

```rust
use dashflow::core::agent_patterns::PlanAndExecuteAgent;

let planner_llm = ChatOpenAI::new().with_model("gpt-4o"); // Smart model
let executor_llm = ChatOpenAI::new().with_model("gpt-4o-mini"); // Fast model

let agent = PlanAndExecuteAgent::new()
    .with_planner_llm(planner_llm)
    .with_executor_llm(executor_llm)
    .with_tools(vec![search, wikipedia, calculator])
    .with_max_iterations(20);

let result = agent.run(
    "Research the top 3 AI trends of 2025 and write a detailed analysis"
).await?;

// Result contains:
// - The plan (list of steps)
// - Execution trace (what happened at each step)
// - Final output (the completed report)
```

### Design Principles

**1. Smart Planning, Fast Execution**
- Use GPT-4/Claude for planning (need intelligence)
- Use GPT-4o-mini/Haiku for execution (need speed)
- Cost optimization: Expensive model once, cheap model many times

**2. Adaptive Planning**
```rust
// Plan can be adjusted based on execution results
if step_result.reveals_new_information {
    plan = replan_with_new_info(plan, step_result);
}
```

**3. Progress Tracking**
```rust
// Track completion
println!("Progress: {}/{} steps complete", current_step, total_steps);
println!("Estimated remaining: {:?}", estimated_time);
```

### When to Use

✅ **Perfect for:**
- Complex multi-step tasks (>5 steps)
- Research projects
- Content creation (reports, analyses)
- Tasks requiring organization
- Long-running workflows

❌ **Overkill for:**
- Simple 1-2 step tasks (use ReAct)
- Tasks where quality matters more than structure (use Reflection)

---

## 3. Reflection Agent (Quality-Driven Iteration)

### How It Works

**Loop:** Generate → Critique → Revise (repeat until quality threshold met)

```
User: "Write a blog post about Rust's memory safety"

=== ITERATION 1 ===
Actor (Writer):
  "Rust is safe because it uses a borrow checker..."
  (150 words, basic explanation)

Critic:
  Quality Score: 3/10
  Issues:
  - Too brief (needs 500+ words)
  - Missing examples
  - No comparison to other languages
  - Lacks technical depth
  → Verdict: REVISE

=== ITERATION 2 ===
Actor (Writer):
  "Rust achieves memory safety without garbage collection...
   For example, consider this code: ...
   Compared to C++, Rust prevents: ...
   The borrow checker enforces: ..."
  (450 words, with examples)

Critic:
  Quality Score: 7/10
  Issues:
  - Good examples!
  - Good comparisons!
  - Still needs: performance implications section
  - Still needs: conclusion
  → Verdict: REVISE

=== ITERATION 3 ===
Actor (Writer):
  [Previous content + performance section + conclusion]
  (650 words, comprehensive)

Critic:
  Quality Score: 9/10
  Strengths:
  - Clear explanations ✓
  - Good examples ✓
  - Comprehensive coverage ✓
  - Professional tone ✓
  Minor: Could add one more real-world example
  → Verdict: ACCEPTABLE (exceeds 8/10 threshold)

=== FINAL OUTPUT ===
  The blog post from iteration 3
```

### Code Example

```rust
use dashflow::core::agent_patterns::ReflectionAgent;

let writer_llm = ChatOpenAI::new().with_model("gpt-4o");
let critic_llm = ChatOpenAI::new().with_model("gpt-4o"); // Same or different

let agent = ReflectionAgent::new()
    .with_actor_llm(writer_llm)
    .with_critic_llm(critic_llm)
    .with_quality_threshold(8.0)  // 0-10 scale
    .with_max_iterations(5);

let result = agent.run(
    "Write a 500-word blog post about Rust's memory safety features"
).await?;

// Result contains:
// - All iterations (drafts + critiques)
// - Quality scores per iteration
// - Final output (the best version)
```

### Design Principles

**1. Actor-Critic Architecture**
- **Actor:** Generates content (optimistic, creative)
- **Critic:** Evaluates quality (skeptical, thorough)
- Separation of concerns → Better quality

**2. Quality-Driven Convergence**
```rust
loop {
    draft = actor.generate(task);
    (score, critique) = critic.evaluate(draft);

    if score >= quality_threshold {
        return draft; // Good enough!
    }

    if iterations >= max_iterations {
        return draft; // Best we got
    }

    task = incorporate_critique(task, critique);
    iterations += 1;
}
```

**3. Critique Types**
```rust
enum CritiqueType {
    Structure,    // Organization, flow
    Content,      // Completeness, accuracy
    Style,        // Tone, clarity
    Technical,    // Correctness, depth
}

// Critic can focus on different aspects
let critic = CriticAgent::new()
    .with_focus(vec![CritiqueType::Content, CritiqueType::Technical]);
```

### When to Use

✅ **Perfect for:**
- Quality-critical outputs (customer-facing content)
- Creative work (writing, design)
- Technical documentation
- Code generation (generate → review → fix)
- Legal/compliance documents

❌ **Overkill for:**
- Simple information retrieval
- Time-sensitive tasks (iterations take time)
- Tasks where "good enough" is fine

---

## 4. Multi-Agent Debate (Collaborative Reasoning)

### How It Works

**Multiple agents debate** from different perspectives, **moderator synthesizes** consensus.

```
User: "Should we invest $10M in building an AI infrastructure?"

=== ROUND 1 ===
Conservative Agent (Risk-averse):
  "No. Risks:
  - Unproven technology
  - High capital expenditure
  - Uncertain ROI
  - Talent acquisition challenges
  We should wait and see."

Aggressive Agent (Innovative):
  "Yes! Opportunities:
  - First-mover advantage
  - Competitive moat
  - Long-term cost savings
  - Strategic positioning
  Act now or fall behind."

Pragmatic Agent (Balanced):
  "Conditional yes:
  - Start with pilot ($2M)
  - Measure results (6 months)
  - Scale if successful
  - Mitigates risk while exploring upside"

=== ROUND 2 ===
Conservative:
  "Pragmatic has a point. Pilot reduces risk. But:
  - Even $2M is significant
  - Need clear success metrics
  - What if pilot fails?"

Aggressive:
  "Pilot is compromise. But:
  - $2M too small to prove value
  - Need $5M minimum for meaningful test
  - Timid approach = missed opportunity"

Pragmatic:
  "Agree with Conservative on metrics. Propose:
  - $3M pilot (middle ground)
  - 4 clear KPIs
  - 6-month decision point
  - Pre-defined scale plan"

=== ROUND 3 ===
[Further refinement...]

=== MODERATOR SYNTHESIS ===
Moderator:
  "Consensus emerging around:

  Recommendation: Invest $3M in 6-month pilot

  Success Criteria:
  1. Reduce inference costs by 40%
  2. Improve latency to <100ms (p95)
  3. Handle 10k req/sec
  4. Maintain 99.9% uptime

  Decision Framework:
  - If 3+ criteria met → Scale to $10M
  - If 2 criteria met → Extend pilot
  - If <2 criteria met → Discontinue

  This balances innovation (Aggressive), risk management (Conservative),
  and practicality (Pragmatic)."
```

### Code Example

```rust
use dashflow::core::agent_patterns::MultiAgentDebate;

let conservative = Agent::new("conservative")
    .with_llm(llm.clone())
    .with_perspective("Risk-averse, cautious, focus on downsides");

let aggressive = Agent::new("aggressive")
    .with_llm(llm.clone())
    .with_perspective("Bold, innovative, focus on opportunities");

let pragmatic = Agent::new("pragmatic")
    .with_llm(llm.clone())
    .with_perspective("Balanced, practical, focus on feasibility");

let moderator = Agent::new("moderator")
    .with_llm(gpt4) // Smart model for synthesis
    .with_role("Synthesize debate into consensus recommendation");

let debate = MultiAgentDebate::new()
    .with_debaters(vec![conservative, aggressive, pragmatic])
    .with_moderator(moderator)
    .with_rounds(3)
    .with_consensus_threshold(0.7);

let result = debate.run(
    "Should we invest $10M in AI infrastructure?"
).await?;

// Result contains:
// - All debate rounds
// - Each agent's positions
// - Evolution of arguments
// - Moderator's synthesis
// - Final recommendation
```

### Design Principles

**1. Diverse Perspectives**
```rust
// Create agents with different personalities/roles
let debaters = vec![
    Agent::new("optimist").with_bias("positive"),
    Agent::new("pessimist").with_bias("negative"),
    Agent::new("analyst").with_bias("data-driven"),
    Agent::new("ethicist").with_bias("moral considerations"),
];
```

**2. Structured Debate**
```rust
for round in 0..num_rounds {
    // Each agent speaks
    for agent in &debaters {
        let argument = agent.argue(topic, previous_arguments);
        arguments.push(argument);
    }

    // Moderator checks for consensus
    if moderator.has_consensus(&arguments) {
        break;
    }
}
```

**3. Consensus Detection**
```rust
// Moderator identifies agreement
let consensus = moderator.find_common_ground(&arguments);

// Or: Vote on proposals
let votes = debaters.vote_on(&proposals);
let winner = proposals.max_by_key(|p| votes[p]);
```

### When to Use

✅ **Perfect for:**
- Important decisions (investment, strategy, architecture)
- Ethical considerations
- Complex trade-offs
- Situations requiring diverse viewpoints
- Risk assessment

❌ **Overkill for:**
- Simple factual questions
- Time-sensitive decisions
- Tasks with objective answers
- Individual creative work

---

## Comparison Matrix

### Task Type → Agent Pattern

| Task | ReAct | Plan & Execute | Reflection | Debate |
|------|-------|----------------|------------|--------|
| **"What is X?"** | ✅ Perfect | ❌ Overkill | ❌ Overkill | ❌ Overkill |
| **"Research X and summarize"** | ✅ Good | ✅ Better | ⚠️ OK | ❌ Overkill |
| **"Create 10-page report on X"** | ❌ Too simple | ✅ Perfect | ⚠️ OK | ❌ Overkill |
| **"Write excellent blog post"** | ❌ No quality loop | ⚠️ OK | ✅ Perfect | ❌ Overkill |
| **"Should we do X?"** (decision) | ❌ No perspectives | ⚠️ Shallow | ⚠️ One view | ✅ Perfect |
| **"Generate code for X"** | ✅ Good | ⚠️ OK | ✅ Better | ❌ Overkill |

---

## Real-World Examples

### Example 1: Research Report (Plan & Execute)

**Task:** "Research quantum computing and write a 10-page report for executives"

**Why Plan & Execute:**
- Complex task (10 pages!)
- Multiple steps (research, organize, write)
- Needs structure
- Long-running (30+ minutes)

**How it works:**
```rust
let agent = PlanAndExecuteAgent::new()
    .with_planner_llm(gpt4)
    .with_executor_llm(gpt4_mini)
    .with_tools(vec![web_search, arxiv, wikipedia]);

// Plan generated:
// 1. Search "quantum computing basics"
// 2. Search "quantum computing applications"
// 3. Search "quantum computing companies"
// 4. Organize by: basics, applications, companies, future
// 5. Write intro (context, importance)
// 6. Write section 1: Basics (2 pages)
// 7. Write section 2: Applications (3 pages)
// 8. Write section 3: Companies (2 pages)
// 9. Write section 4: Future (2 pages)
// 10. Write conclusion (1 page)

let report = agent.run(task).await?;
// → 10-page executive report
```

### Example 2: Blog Post (Reflection)

**Task:** "Write a 500-word blog post about Rust (must be excellent)"

**Why Reflection:**
- Quality critical (public-facing)
- Creative work (writing)
- Multiple dimensions (content, style, technical accuracy)

**How it works:**
```rust
let agent = ReflectionAgent::new()
    .with_actor_llm(claude)
    .with_critic_llm(gpt4)  // Different perspective!
    .with_quality_threshold(8.5);

// Iteration 1: Draft (5/10) - Too technical
// Iteration 2: Draft (7/10) - Better but needs examples
// Iteration 3: Draft (9/10) - Excellent!

let post = agent.run("Write 500-word blog post about Rust").await?;
```

### Example 3: Technical Decision (Multi-Agent Debate)

**Task:** "Should we use microservices or monolith architecture?"

**Why Debate:**
- Important decision (architecture)
- Multiple valid perspectives
- Trade-offs to consider
- Need diverse expertise

**How it works:**
```rust
let debaters = vec![
    Agent::new("architect").with_perspective("Scalability and maintainability"),
    Agent::new("ops").with_perspective("Operational complexity and cost"),
    Agent::new("developer").with_perspective("Development velocity and DX"),
];

let debate = MultiAgentDebate::new()
    .with_debaters(debaters)
    .with_moderator(cto_agent)
    .with_rounds(3);

let decision = debate.run(
    "Microservices vs monolith for our new product?"
).await?;

// Moderator synthesis:
// "Start with modular monolith. Clear boundaries. Extract to microservices
// when: (1) team >20 people, (2) independent scaling needed, (3) polyglot
// requirements. This balances velocity (dev), simplicity (ops), and future
// optionality (architect)."
```

---

## Combining Patterns

### Pattern Composition

**You can combine patterns for maximum power:**

#### Example: Plan + Reflect

```rust
// Step 1: Plan the work
let planner = PlanAndExecuteAgent::new()...;
let plan = planner.create_plan(task).await?;

// Step 2: Execute each step with reflection
for step in plan.steps {
    let reflection_agent = ReflectionAgent::new()...;
    let output = reflection_agent.run(step).await?;
    results.push(output);
}

// Get structured execution + quality output!
```

#### Example: Debate + Execute

```rust
// Step 1: Debate to decide approach
let debate = MultiAgentDebate::new()...;
let approach = debate.run("Which algorithm to use?").await?;

// Step 2: Execute chosen approach
let executor = ReActAgent::new()...;
let result = executor.run(&format!("Implement {}", approach)).await?;
```

---

## Implementation Architecture

### Pattern Hierarchy

```
Agent Patterns:
├── Basic
│   └── ReAct (Thought → Action → Observation)
│
├── Complex
│   ├── Plan & Execute (Planning → Execution)
│   └── Reflection (Generate → Critique → Revise)
│
└── Multi-Agent
    └── Debate (Multiple agents → Synthesis)
```

### Shared Infrastructure

All patterns use:
- **LLM abstraction** (any model)
- **Tool integration** (any tool)
- **Memory system** (conversation history)
- **Callbacks** (monitoring, logging)
- **Error handling** (retry, fallback)

---

## Performance Characteristics

| Pattern | Latency | Cost | Quality | Complexity |
|---------|---------|------|---------|------------|
| **ReAct** | Low (1-3 LLM calls) | $ | Good | Simple |
| **Plan & Execute** | High (10-20 LLM calls) | $$$ | Very Good | Medium |
| **Reflection** | Medium (3-8 LLM calls) | $$ | Excellent | Medium |
| **Debate** | High (6-30 LLM calls) | $$$$ | Excellent | High |

**Cost Example (GPT-4o):**
- ReAct: 5k tokens × $5/M = $0.025
- Plan & Execute: 50k tokens × $5/M = $0.25
- Reflection: 20k tokens × $5/M = $0.10
- Debate (3 agents, 3 rounds): 100k tokens × $5/M = $0.50

---

## Best Practices

### 1. Choose the Right Pattern

**Decision Tree:**
```
Is task complex (>5 steps)?
  YES → Plan & Execute
  NO  → Is quality critical?
          YES → Reflection
          NO  → Is decision requiring perspectives?
                  YES → Debate
                  NO  → ReAct
```

### 2. Model Selection

**Planning:** Use smart models (GPT-4, Claude Opus)
**Execution:** Use fast models (GPT-4o-mini, Haiku)
**Critique:** Use smart models (different perspective)

### 3. Set Appropriate Thresholds

**Quality threshold:**
- Blog posts: 8/10 (high quality)
- Internal docs: 6/10 (good enough)
- Customer-facing: 9/10 (excellent)

**Max iterations:**
- ReAct: 10-15 (prevent loops)
- Plan & Execute: 20-30 (complex tasks)
- Reflection: 5-8 (diminishing returns)
- Debate: 3-5 rounds (convergence)

### 4. Monitor Costs

```rust
// Track token usage
agent.with_callback(CostTracker::new());

let result = agent.run(task).await?;
println!("Cost: ${:.2}", result.cost_usd);
```

### 5. Implement Timeouts

```rust
// Prevent runaway agents
agent.with_timeout(Duration::from_secs(300));  // 5 minutes max
```

---

## Testing Strategy

### For Each Pattern

**Unit Tests:**
- Test planning logic
- Test critique parsing
- Test consensus detection
- Test error handling

**Integration Tests:**
- End-to-end with real LLM
- Verify convergence
- Measure quality improvement

**Property Tests:**
- Termination (always finishes)
- Progress (quality improves or stays same)
- Idempotency (same input → same output range)

---

## Examples in Codebase

**ReAct-style agents:**
- `crates/dashflow/examples/crag_agent.rs` - Corrective RAG with tool use
- `crates/dashflow/examples/cascading_agent.rs` - Cascading reasoning

**Planning and Execution:**
- `crates/dashflow/examples/financial_analysis_agent.rs` - Multi-step analysis
- `crates/dashflow/examples/quality_enforced_agent.rs` - Quality-gated execution

**Reflection and Self-Critique:**
- `crates/dashflow/examples/confidence_routing_agent.rs` - Confidence-based routing
- `crates/dashflow/examples/dual_path_agent.rs` - Parallel evaluation paths

**Multi-Agent Patterns:**
- `crates/dashflow/examples/multi_agent_research.rs` - Collaborative research
- `crates/dashflow/examples/multi_strategy_agent.rs` - Strategy coordination

---

## Future Patterns (v1.4+)

**Tree of Thoughts:**
- Explore multiple reasoning paths
- Prune bad paths
- Find optimal solution

**Mixture of Agents:**
- Aggregate outputs from multiple agents
- Ensemble decisions
- Improve robustness

**Hierarchical Agents:**
- Manager → Sub-managers → Workers
- Delegation chains
- Large-scale coordination

---

**Status:** v1.3.0 Implementation Complete
**Examples:** Available in `crates/dashflow/examples/`
**Documentation:** This guide

**Author:** Andrew Yates © 2026
