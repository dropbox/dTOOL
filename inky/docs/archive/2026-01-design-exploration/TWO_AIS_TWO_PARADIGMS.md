# Two AIs, Two Paradigms: Why Inky Must Bridge Both

**Date:** 2026-01-01
**Key Insight:** Claude Code (Ink/React) and Codex (TUI/ratatui) represent fundamentally different mental models. Inky must serve both.

---

## The Two Most Important AI Systems

### Claude Code → Ink (JavaScript, React-like)

```javascript
// What Claude generates (trained on Ink patterns):
import {render, Box, Text} from 'ink';

const App = () => (
  <Box flexDirection="column">
    <Text color="green">Success!</Text>
    <Text>Operation complete.</Text>
  </Box>
);

render(<App />);
```

**Mental model:**
- Components compose
- Declarative structure
- JSX syntax
- State flows down
- React patterns everywhere

### Codex → TUI/ratatui (Rust, immediate-mode)

```rust
// What Codex generates (trained on ratatui patterns):
use ratatui::prelude::*;

fn render(frame: &mut Frame, area: Rect) {
    let text = vec![
        Line::from(Span::styled("Success!", Style::default().fg(Color::Green))),
        Line::from("Operation complete."),
    ];
    frame.render_widget(Paragraph::new(text), area);
}
```

**Mental model:**
- Widgets render to frames
- Imperative drawing
- Style objects
- Layout via Rect
- Rust idioms everywhere

---

## The Problem: Different Training, Different Output

When asked to "show a success message," these AIs generate VERY different code:

| Aspect | Claude (Ink) | Codex (ratatui) |
|--------|--------------|-----------------|
| **Structure** | `<Text color="green">` | `Span::styled(..., Style::...)` |
| **Composition** | JSX nesting | `vec![]` of Lines |
| **Styling** | Props: `color="green"` | Method chain: `.fg(Color::Green)` |
| **Layout** | `flexDirection="column"` | `Rect`, `Layout::default()` |
| **Rendering** | `render(<App />)` | `frame.render_widget(...)` |

**If inky only matches one paradigm, it fails the other AI.**

---

## Analysis: What Each AI Expects

### Claude Code Expectations (from Ink training)

1. **Component-like composition**
   ```rust
   // Claude expects something like:
   Box::new()
       .direction(Column)
       .child(Text::new("Success!").green())
       .child(Text::new("Done"))
   ```

2. **Props as methods**
   ```rust
   // Styling via chainable methods, like React props
   Text::new("Hello").color("green").bold()
   ```

3. **Declarative structure**
   ```rust
   // Describe WHAT, not HOW
   terminal.render(my_component);
   ```

4. **Familiar names**
   - `Box`, `Text`, `Flex` (not `Paragraph`, `Block`, `Layout`)

### Codex Expectations (from ratatui training)

1. **Line/Span structure**
   ```rust
   // Codex expects something like:
   let lines = vec![
       Line::from(vec![Span::styled("Success!", green)]),
       Line::from("Done"),
   ];
   ```

2. **Style objects**
   ```rust
   // Styling via Style struct
   Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
   ```

3. **Frame-based rendering**
   ```rust
   // Render to a frame with area
   frame.render_widget(widget, area);
   ```

4. **Familiar names**
   - `Line`, `Span`, `Paragraph`, `Block`, `Rect`

---

## The Bridge: Inky Must Accept Both

### Strategy 1: Dual Constructors

```rust
// For Claude (Ink-trained):
terminal.say(
    Box::column()
        .child(Text::new("Success!").green())
        .child(Text::new("Done"))
);

// For Codex (ratatui-trained):
terminal.say(vec![
    Line::styled("Success!", Color::Green),
    Line::from("Done"),
]);

// Both produce identical output
```

### Strategy 2: Universal `Into<Content>`

```rust
// Everything becomes Content
impl Into<Content> for Box { ... }
impl Into<Content> for Vec<Line> { ... }
impl Into<Content> for Line { ... }
impl Into<Content> for Span { ... }
impl Into<Content> for &str { ... }

// terminal.say() accepts anything
terminal.say("Hello");                    // &str
terminal.say(Text::new("Hello").bold());  // Component
terminal.say(line!["Hello"].green());     // Line
terminal.say(vec![line1, line2]);         // Vec<Line>
```

### Strategy 3: Compatibility Aliases

```rust
// ratatui names → inky names
pub use inky::Line;        // Same as ratatui::Line
pub use inky::Span;        // Same as ratatui::Span
pub use inky::Style;       // Similar to ratatui::Style
pub use inky::Paragraph;   // → inky::TextBlock

// Ink names → inky names
pub use inky::Box;         // Layout container
pub use inky::Text;        // Styled text
pub use inky::Flex;        // Flex layout
```

---

## The Unified API Surface

Inky's API must feel natural to BOTH AIs:

### For Display (Both AIs)

```rust
// Simple (both AIs generate this)
terminal.say("Hello");
terminal.error("Failed");
terminal.success("Done");

// Claude-style (component composition)
terminal.say(
    column![
        text!("Success!").green(),
        text!("Done")
    ]
);

// Codex-style (lines and spans)
terminal.say(vec![
    Line::styled("Success!", Color::Green),
    Line::from("Done"),
]);
```

### For Rich Content (Both AIs)

```rust
// Universal (both AIs)
terminal.code(source, "rust");
terminal.diff(old, new);
terminal.table(data);

// Claude-style (component)
terminal.show(
    CodeBlock::new(source)
        .language("rust")
        .line_numbers(true)
);

// Codex-style (widget)
terminal.render_widget(
    Paragraph::new(highlighted_lines)
        .block(Block::default().borders(Borders::ALL)),
    area
);
```

### For Input (Both AIs)

```rust
// Universal
let name = terminal.ask("What's your name?");
let confirm = terminal.confirm("Continue?");

// Both AIs generate the same code
// Because it's short and obvious
```

---

## Design Principles for Dual-AI Support

### Principle 1: Shortest Path Wins

Whatever BOTH AIs generate naturally should work:

```rust
// If Claude generates:
terminal.say(Text::new("Hello").green());

// And Codex generates:
terminal.say(Line::styled("Hello", Color::Green));

// BOTH must work. BOTH must produce identical output.
```

### Principle 2: Common Vocabulary

Use names both AIs know:

| Use | Don't Use | Why |
|-----|-----------|-----|
| `Line` | `Row` | Both know "Line" |
| `Span` | `Run` | Both know "Span" |
| `Style` | `Format` | Both know "Style" |
| `Box` | `Container` | Ink uses Box |
| `color` | `foreground` | Simpler |
| `bold` | `add_modifier(Bold)` | Simpler |

### Principle 3: Bidirectional Conversion

Everything converts to everything:

```rust
// Line → Box content
let box_content: Box = line.into();

// Box → Lines
let lines: Vec<Line> = box_node.into();

// Both directions work
// AIs can mix paradigms
```

### Principle 4: Same Output Regardless

```rust
// These three calls produce IDENTICAL terminal output:
terminal.say("Hello".green());
terminal.say(Text::new("Hello").green());
terminal.say(Line::styled("Hello", Color::Green));

// AI doesn't need to know which is "right"
// They're all right
```

---

## Implementation: The Adapter Layer

```rust
/// Universal content that accepts both paradigms
pub enum Content {
    // Direct text
    Text(String),

    // Component tree (Claude/Ink style)
    Node(Box<dyn Node>),

    // Line structure (Codex/ratatui style)
    Lines(Vec<Line>),
}

impl From<&str> for Content { ... }
impl From<String> for Content { ... }
impl From<Box> for Content { ... }
impl From<Text> for Content { ... }
impl From<Line> for Content { ... }
impl From<Span> for Content { ... }
impl From<Vec<Line>> for Content { ... }

impl Terminal {
    /// Display anything
    pub fn say(&self, content: impl Into<Content>) {
        let content = content.into();
        // Render appropriately
        self.render(content);
    }
}
```

---

## The Test: Both AIs Succeed

### Test 1: Simple Output

```
Prompt: "Display a green success message saying 'Build complete'"
```

**Claude generates:**
```rust
terminal.say(Text::new("Build complete").green());
```

**Codex generates:**
```rust
terminal.say(Line::styled("Build complete", Color::Green));
```

**Both work. Both produce:**
```
✓ Build complete
```

### Test 2: Structured Output

```
Prompt: "Show a code block with syntax highlighting"
```

**Claude generates:**
```rust
terminal.show(
    CodeBlock::new(source)
        .language("rust")
);
```

**Codex generates:**
```rust
terminal.code(source, "rust");
```

**Both work. Both produce identical highlighted code.**

### Test 3: Complex Layout

```
Prompt: "Show a header, then a list of items, then a footer"
```

**Claude generates:**
```rust
terminal.say(column![
    text!("Header").bold(),
    list!(items),
    text!("Footer").dim()
]);
```

**Codex generates:**
```rust
terminal.say(vec![
    Line::styled("Header", Style::default().bold()),
    // ... list lines ...
    Line::styled("Footer", Style::default().dim()),
]);
```

**Both work.**

---

## What This Means for Inky

### Must Have

1. **Line/Span types** that work like ratatui (for Codex)
2. **Box/Text components** that work like Ink (for Claude)
3. **Universal Into<Content>** that accepts both
4. **Identical rendering** regardless of which API used

### Nice to Have

1. **Macros** that reduce boilerplate for both styles
2. **Syntax sugar** that feels natural to both AIs
3. **Error messages** that guide toward correct usage

### Must NOT Have

1. **Paradigm preference** - neither style is "better"
2. **Conversion overhead** - both paths equally efficient
3. **Feature gaps** - anything possible in one is possible in other

---

## The Mantra

> "Claude and Codex generate different code. Inky accepts both. Output is identical."

**The framework doesn't care HOW you describe the UI. It cares that the UI is god-tier.**

---

## Summary

| AI | Training | Paradigm | Inky Support |
|----|----------|----------|--------------|
| Claude Code | Ink (React) | Components | `Box`, `Text`, composition |
| Codex | ratatui | Lines/Spans | `Line`, `Span`, `Vec<Line>` |
| Both | - | Universal | `terminal.say()`, `terminal.code()` |

**Inky bridges both paradigms. Both AIs produce god-tier output. That's the product.**
