# AI Tooling

Inky includes helper APIs for agent-style workflows. These APIs are currently
**unstable** and may change in the 0.x series.

## Perception API

`Perception` turns a rendered buffer into structured signals for an agent or
automation layer.

```rust
use inky::perception::Perception;
use inky::render::{Buffer, Cell};

let mut buffer = Buffer::new(5, 1);
buffer.set(0, 0, Cell::new('H'));
buffer.set(1, 0, Cell::new('i'));

let perception = Perception::new(&buffer);
let text = perception.as_text();
```

### Text Extraction

- `as_text()` - Plain text content, suitable for LLM context.
- `as_marked_text()` - Text with inline style markers like `[bold]text[/bold]`.
- `as_tokens()` - Structured `Token` spans with position and style metadata.

### Change Detection

Track changes between frames to focus agent attention:

```rust
use inky::perception::Perception;
use inky::render::Buffer;

let prev = Buffer::new(80, 24);
let current = Buffer::new(80, 24);

let diff = Perception::semantic_diff(&prev, &current);
println!("Summary: {}", diff.summary());

for (row, content) in &diff.added_lines {
    println!("Added at row {}: {}", row, content);
}
for (row, content) in &diff.modified_lines {
    println!("Changed at row {}: {}", row, content);
}
```

### Region Reading

Read specific areas or search for content:

```rust
use inky::perception::{Perception, Region};
use inky::render::Buffer;

let buffer = Buffer::new(80, 24);
let perception = Perception::new(&buffer);

// Read a rectangular region
let region = Region::new(10, 5, 20, 3);
let content = perception.read_region(&region);

// Find all occurrences of a string
let matches = perception.find("error");
for (x, y) in matches {
    println!("Found at ({}, {})", x, y);
}
```

### Image snapshots

When the optional `image` feature is enabled, you can capture PNG snapshots:

```toml
inky-tui = { version = "0.1", features = ["image"] }
```

```rust
use inky::perception::Perception;
use inky::render::Buffer;

let buffer = Buffer::new(80, 24);
let perception = Perception::new(&buffer);
let png_bytes = perception.as_image(2);
let png_base64 = perception.as_image_base64(2);
```

## Shared Memory Perception

`SharedPerception` lets external processes read terminal state from shared
memory without copying. This is useful for AI agents that run out of process.

```rust
use inky::perception::{discover_shared_buffers, SharedPerception};

let buffers = discover_shared_buffers();
if let Some((pid, path)) = buffers.first() {
    if let Ok(mut perception) = SharedPerception::open(path) {
        if matches!(perception.poll_update(), Ok(true)) {
            let text = perception.as_text();
            println!("PID {}:\n{}", pid, text);
        }
    }
}
```

## Clipboard API

`Clipboard` provides OSC 52 clipboard interactions for terminals that support
it.

```rust
use inky::clipboard::{Clipboard, ClipboardSelection};

fn main() -> std::io::Result<()> {
    Clipboard::copy("Hello from inky")?;
    Clipboard::copy_to_selection("Primary selection", ClipboardSelection::Primary)?;
    Ok(())
}
```

For paste workflows, request the paste sequence and parse the response:

```rust
use inky::clipboard::Clipboard;

Clipboard::request_paste()?;
```

Then call `Clipboard::parse_paste_response` on the incoming terminal payload.
