# Better Than Expedient: Why Inky Must Surpass Both

**Date:** 2026-01-01
**Key Insight:** Ink and ratatui get the job done. That's not good enough. Inky must be *better* while remaining *familiar*.

---

## The Current State: Expedient, Not Excellent

### Ink: Gets the Job Done (For React Devs)

**What Ink does well:**
- React mental model (component composition)
- Familiar JSX syntax
- Easy to get started
- Works

**What Ink does poorly:**
- JavaScript performance limitations
- No streaming-first design
- Accessibility is bolted on
- Testing is awkward
- Node.js dependency
- "Good enough" output quality

### ratatui: Gets the Job Done (For Rust Devs)

**What ratatui does well:**
- Native Rust performance
- Immediate-mode simplicity
- Large widget library
- Works

**What ratatui does poorly:**
- Verbose API (too many tokens)
- Line/Span boilerplate everywhere
- No semantic content types
- Accessibility is an afterthought
- Styling is tedious
- "Functional" output quality

---

## The Opportunity: Exceed Both

Inky shouldn't just match Ink and ratatui. It should be **demonstrably better** at what matters:

| Dimension | Ink | ratatui | Inky (Target) |
|-----------|-----|---------|---------------|
| **Output quality** | Good | Functional | God-tier |
| **API brevity** | Medium | Verbose | Minimal |
| **Streaming** | Possible | Possible | Native |
| **Accessibility** | Partial | Minimal | Built-in |
| **Testability** | Awkward | Possible | First-class |
| **AI generation** | Okay | Okay | Optimized |

---

## What "Better Than Expedient" Means

### 1. Better Output Quality

**Expedient (ratatui):**
```
Error: File not found
```

**God-tier (inky):**
```
┌─ Error ────────────────────────────────────────┐
│ ✗ File not found: config.yaml                  │
│                                                │
│   Searched in:                                 │
│   • ./config.yaml                              │
│   • ~/.config/myapp/config.yaml                │
│                                                │
│   Run 'myapp init' to create a config file.   │
└────────────────────────────────────────────────┘
```

**Same API call. Dramatically better output.**

### 2. Better API Brevity

**Expedient (ratatui):**
```rust
let text = Text::from(vec![
    Line::from(vec![
        Span::styled("Error: ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
        Span::raw(message),
    ]),
]);
frame.render_widget(Paragraph::new(text), area);
```

**God-tier (inky):**
```rust
terminal.error(message);
```

**Same visual result. 90% fewer tokens.**

### 3. Better Streaming

**Expedient (Ink):**
```javascript
// Have to manage state manually
const [text, setText] = useState('');
useEffect(() => {
  stream.on('data', chunk => setText(t => t + chunk));
}, []);
return <Text>{text}</Text>;
```

**God-tier (inky):**
```rust
terminal.stream(ai_response).await;
```

**Streaming is the default, not a special case.**

### 4. Better Accessibility

**Expedient (both):**
```rust
// Hope the screen reader figures it out
println!("Error: {}", message);
```

**God-tier (inky):**
```rust
terminal.error(message);
// Framework automatically:
// - Announces "Error: {message}" to screen readers
// - Sets appropriate ARIA role
// - Provides navigation structure
```

**Accessibility from semantics, not annotations.**

### 5. Better Testing

**Expedient (Ink):**
```javascript
// Render to string, parse manually, hope it works
const {lastFrame} = render(<App />);
expect(lastFrame()).toContain('Error');
```

**God-tier (inky):**
```rust
let mock = MockTerminal::new();
app.run(&mut mock).await;

assert_eq!(mock.output(), vec![
    Output::Error { message: "File not found", hint: Some("Run init") }
]);
```

**Test semantics, not strings.**

---

## The Design Principle: Familiar But Better

AIs are trained on Ink and ratatui. Inky must:

1. **Feel familiar** — Use concepts both AIs know
2. **Be shorter** — Fewer tokens for same result
3. **Produce better output** — God-tier, not functional
4. **Handle hard stuff** — Streaming, accessibility, testing

### The Formula

```
Inky = (Ink concepts + ratatui concepts)
       - boilerplate
       + god-tier defaults
       + streaming native
       + accessibility built-in
```

---

## Concrete Improvements Over Each

### Over Ink

| Ink Limitation | Inky Improvement |
|----------------|------------------|
| JS performance | Rust native |
| Manual streaming | `terminal.stream()` |
| State management needed | Stateless API |
| JSX syntax required | Chainable methods |
| Node.js dependency | Zero runtime deps |

### Over ratatui

| ratatui Limitation | Inky Improvement |
|--------------------|------------------|
| Line/Span boilerplate | `terminal.say("text")` |
| Style::default().fg()... | `.red().bold()` |
| Manual accessibility | Automatic from semantics |
| Widget render dance | Direct methods |
| No semantic types | `Code`, `Diff`, `Error` built-in |

---

## The Benchmark: AI Token Efficiency

For AI code generation, fewer tokens = better.

### Task: "Show a red error message"

**Ink (~45 tokens):**
```javascript
import {render, Text} from 'ink';
render(<Text color="red">Error: {message}</Text>);
```

**ratatui (~60 tokens):**
```rust
let text = Line::from(Span::styled(
    format!("Error: {}", message),
    Style::default().fg(Color::Red)
));
frame.render_widget(Paragraph::new(text), area);
```

**inky (~12 tokens):**
```rust
terminal.error(message);
```

**5x fewer tokens than Ink. 5x fewer than ratatui.**

---

## The Quality Bar: God-Tier Default Output

When AI generates `terminal.error(message)`, the output must be:

### Functional (What ratatui produces)
```
Error: File not found
```

### Good (What Ink might produce)
```
✗ Error: File not found
```

### God-Tier (What inky produces)
```
┌─ Error ────────────────────────────────────────┐
│ ✗ File not found: config.yaml                  │
│                                                │
│   The configuration file could not be located. │
│                                                │
│   Searched in:                                 │
│   • ./config.yaml                              │
│   • ~/.config/myapp/config.yaml                │
│   • /etc/myapp/config.yaml                     │
│                                                │
│   Suggestions:                                 │
│   • Run 'myapp init' to create a config file  │
│   • Set CONFIG_PATH environment variable       │
│                                                │
│   Documentation: https://myapp.dev/config     │
└────────────────────────────────────────────────┘
```

**Same one-line API call. Dramatically better UX.**

---

## Why "Expedient" Isn't Enough

### Expedient serves developers.
### God-tier serves users.

The human using the terminal app doesn't care if it was easy to code. They care if:
- It looks beautiful
- It's accessible
- It's responsive
- It helps them succeed

**Ink and ratatui optimize for developer convenience.**
**Inky optimizes for end-user experience.**

That's the difference.

---

## The Contract

Inky promises:

1. **Familiar to AIs** — Concepts from Ink and ratatui
2. **Shorter than both** — Fewer tokens for same result
3. **Better output than both** — God-tier by default
4. **More capable than both** — Streaming, accessibility, testing built-in

If inky doesn't deliver on ALL FOUR, it has failed.

---

## Summary

| Framework | Designed For | Output Quality | Token Efficiency |
|-----------|--------------|----------------|------------------|
| Ink | React devs | Good | Medium |
| ratatui | Rust devs | Functional | Poor |
| **inky** | **AI + Users** | **God-tier** | **Excellent** |

**Ink and ratatui are expedient. They get the job done.**

**Inky must be excellent. It must get the job done *brilliantly*.**

---

## The Mantra

> "Expedient is the enemy of excellent. Inky exists because 'good enough' isn't."
