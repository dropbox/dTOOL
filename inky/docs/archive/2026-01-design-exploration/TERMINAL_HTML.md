# Terminal HTML: Simple by Default, Powerful When Needed

**Date:** 2026-01-01
**Insight:** HTML got the easy stuff right. The DOM got bloated. Can we learn from both?

---

## The HTML Lesson

HTML's killer feature isn't the angle brackets. It's the **semantic defaults**:

```html
<input type="text">     <!-- Keyboard works. Focus works. Selection works. -->
<select>                <!-- Arrow keys work. Type-to-search works. -->
<button>                <!-- Enter/Space work. Focus ring works. -->
<form>                  <!-- Tab order works. Submit works. -->
```

You don't configure any of this. It just works.

**Then we ruined it** with:
- CSS resets that break default styles
- JavaScript frameworks that reimplement `<select>`
- Build tools to compile HTML-in-JS back to HTML
- 500KB of React to make a button

---

## The Terminal Opportunity

Terminals don't have 30 years of browser baggage. We can design it right.

### Principle 1: Semantic Primitives That Just Work

```rust
// This should be ALL you need for a basic form
let form = form![
    field!("name", "Your name"),
    field!("email", "Email address"),
    select!("country", ["USA", "Canada", "UK", "Other"]),
    checkbox!("subscribe", "Subscribe to newsletter"),
    buttons![
        button!("Submit").primary(),
        button!("Cancel"),
    ],
];
```

**What you get for free:**
- Tab/Shift+Tab navigation
- Focus ring on focused field
- Enter submits, Escape cancels
- Type to filter in select
- Checkbox toggles on Space
- Form validation (if you add constraints)
- Accessible labels
- Works over SSH

**What you DON'T have to do:**
- Handle key events
- Manage focus state
- Style the focus ring
- Implement select dropdown
- Wire up submission

### Principle 2: Progressive Complexity

```
Level 0: Semantic primitives     form![field!("name")]
Level 1: Styling                 field!("name").style(...)
Level 2: Behavior customization  field!("name").on_change(...)
Level 3: Custom rendering        CustomField::new(...).into()
Level 4: Raw escape sequences    term.write("\x1b[31m...")
```

You only drop to a lower level when you need to. Most apps stay at Level 0-1.

### Principle 3: No Framework Required

```rust
// A complete, working TUI in 10 lines
fn main() -> Result<()> {
    let result = dialog![
        "Create New Project",
        field!("name", "Project name"),
        field!("path", "Location").default("./"),
        select!("template", ["Empty", "Web App", "CLI Tool"]),
    ].run()?;  // Blocking, returns when user submits or cancels

    if let Some(data) = result {
        println!("Creating {} at {}", data["name"], data["path"]);
    }
    Ok(())
}
```

No event loop. No state management. No framework. Just ask a question, get an answer.

---

## The Semantic Primitives

### Text Display

```rust
// Simple
print!["Hello, world!"];

// Styled
print!["Hello, ", "world!".bold()];

// Multi-line
print![
    "Line 1",
    "Line 2",
    "Line 3".dim(),
];
```

### Input

```rust
// Single line input
let name = input!("What's your name?").run()?;

// With placeholder and default
let path = input!("Save location")
    .placeholder("/path/to/file")
    .default("./output.txt")
    .run()?;

// Password
let pwd = password!("Enter password").run()?;

// Multi-line
let message = textarea!("Your message")
    .rows(5)
    .run()?;
```

### Selection

```rust
// Single select
let choice = select!("Pick one", ["A", "B", "C"]).run()?;

// Multi select
let choices = multiselect!("Pick several", ["A", "B", "C"]).run()?;

// Confirm
let ok = confirm!("Are you sure?").run()?;  // true/false
```

### Forms (Grouped Input)

```rust
let user = form![
    "Create Account",
    field!("username", "Username").required(),
    field!("email", "Email").email(),
    password!("password", "Password").min(8),
    password!("confirm", "Confirm").matches("password"),
    checkbox!("tos", "I agree to the Terms of Service").required(),
].run()?;

// user: Option<HashMap<String, Value>>
```

### Lists and Tables

```rust
// Simple list
list!["Item 1", "Item 2", "Item 3"];

// Selectable list
let item = list!["Item 1", "Item 2", "Item 3"]
    .selectable()
    .run()?;

// Table
table![
    ["Name", "Age", "City"],
    ["Alice", "30", "NYC"],
    ["Bob", "25", "LA"],
];
```

### Progress and Status

```rust
// Spinner while working
spinner!("Loading...").run(|| {
    heavy_computation()
})?;

// Progress bar
progress!("Downloading")
    .run(|bar| {
        for i in 0..100 {
            download_chunk(i);
            bar.set(i);
        }
    })?;

// Status line (non-blocking)
status!["Ready", "•".green(), "Connected"];
```

### Layout (When Needed)

```rust
// Horizontal
hbox![widget_a(), widget_b()];

// Vertical
vbox![widget_a(), widget_b()];

// Split panes
hsplit![
    sidebar().width(30),
    main_content().flex(1),
];

// Only use these when semantic primitives aren't enough
```

---

## What We DON'T Include (Anti-Bloat)

### No Virtual DOM
The terminal is already diffed at the buffer level. Double-diffing is waste.

### No Component Lifecycle
No `componentDidMount`, `useEffect`, `ngOnInit`. If you need setup, do it before render.

### No CSS-like Cascade
Styles are explicit, not inherited through a cascade. What you see is what you get.

### No Template Syntax
No `{{interpolation}}`, no `v-if`, no `*ngFor`. It's Rust. Use Rust.

### No Dependency Injection
No providers, no context, no services. Pass data explicitly.

### No State Management Framework
No Redux, no MobX, no signals. Use variables. Use structs.

### No Build Step
No webpack, no bundler, no transpiler. `cargo build`, done.

---

## The Escape Hatches

When semantic primitives aren't enough:

### Level 1: Style Overrides

```rust
field!("name")
    .style(Style::new()
        .border(Border::Rounded)
        .fg(Color::Cyan))
```

### Level 2: Event Handlers

```rust
field!("name")
    .on_change(|value| validate_username(value))
    .on_submit(|value| save_username(value))
```

### Level 3: Custom Widgets

```rust
// Implement the Widget trait
struct MyWidget { ... }

impl Widget for MyWidget {
    fn render(&self, ctx: &Context) -> Node { ... }
    fn handle(&mut self, event: Event) -> Option<Action> { ... }
}

// Use it alongside semantic primitives
form![
    field!("name"),
    MyWidget::new(),  // Custom widget
    button!("Submit"),
]
```

### Level 4: Raw Terminal Access

```rust
// When you really need it
term.write("\x1b[?1049h");  // Alternate screen
term.write("\x1b[31m");     // Red
term.write("Raw output");
term.write("\x1b[0m");      // Reset
term.flush();
```

---

## Comparison: Browser vs Terminal

| Aspect | Browser (Bloated) | Terminal HTML (Lean) |
|--------|-------------------|----------------------|
| Basic form | React + CSS + bundler | `form![field!("x")]` |
| Lines of code | 500+ | 5 |
| Dependencies | 200MB node_modules | 0 |
| Build time | 30 seconds | 0 (it's Rust) |
| Bundle size | 500KB+ | N/A |
| Works offline | Maybe | Always |
| Works over SSH | No | Yes |
| Accessible | If you remember | By default |

---

## Real Examples

### Example 1: Git Commit Helper

```rust
fn main() -> Result<()> {
    let commit = form![
        "Create Commit",
        select!("type", ["feat", "fix", "docs", "refactor", "test"]),
        field!("scope", "Scope").optional(),
        field!("subject", "Subject").required(),
        textarea!("body", "Body").optional(),
        checkbox!("breaking", "Breaking change"),
    ].run()?;

    if let Some(c) = commit {
        let msg = format_commit(&c);
        exec!("git", "commit", "-m", &msg)?;
    }
    Ok(())
}
```

### Example 2: Server Installer

```rust
fn main() -> Result<()> {
    print![
        "Server Installer".bold(),
        "",
    ];

    let config = form![
        field!("hostname", "Hostname").default("localhost"),
        field!("port", "Port").default("8080").number(),
        select!("database", ["PostgreSQL", "MySQL", "SQLite"]),
        checkbox!("ssl", "Enable SSL"),
    ].run()?.expect("cancelled");

    spinner!("Installing...").run(|| {
        install_server(&config)
    })?;

    print!["Done!".green().bold()];
    Ok(())
}
```

### Example 3: File Browser

```rust
fn main() -> Result<()> {
    let files = list_files(".")?;

    loop {
        let selection = list![&files]
            .title("Select a file")
            .searchable()
            .run()?;

        match selection {
            Some(file) if file.is_dir() => {
                cd(&file);
                files = list_files(".")?;
            }
            Some(file) => {
                open(&file);
                break;
            }
            None => break,
        }
    }
    Ok(())
}
```

---

## The Philosophy

1. **Defaults should be good.** Don't require configuration for common cases.

2. **Semantics over structure.** `form![]` instead of `div.form > div.field > input`.

3. **Behaviors are built-in.** Tab navigation, focus, selection—free.

4. **Escape hatches exist.** But you shouldn't need them often.

5. **No ceremony.** A simple question should be a simple line of code.

6. **It's just Rust.** No DSL, no template language, no magic strings.

---

## What This Means for Inky

Inky currently has:
- `BoxNode`, `TextNode` — structural primitives ✓
- `Input`, `Scroll` — some semantic components ✓
- Layout engine — ✓

Inky should add:
- `form![]`, `field![]`, `select![]` — high-level semantic macros
- `.run()` method — blocking execution for simple scripts
- More semantic components — `table![]`, `progress![]`, `spinner![]`
- Blessed defaults — so you don't have to configure everything

The goal: **Make the simple things simple, and the complex things possible.**

---

## Summary

```
┌─────────────────────────────────────────────────────────────┐
│                                                             │
│   HTML got this right:                                      │
│   <input type="text">  ←  Just works                       │
│                                                             │
│   The DOM got bloated:                                      │
│   document.querySelector().addEventListener()...            │
│                                                             │
│   Terminal HTML:                                            │
│   input!("Name")  ←  Just works, no bloat                  │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

**The web taught us what to do (semantic primitives) and what not to do (framework churn, build complexity, API bloat).**

Let's learn from both.
