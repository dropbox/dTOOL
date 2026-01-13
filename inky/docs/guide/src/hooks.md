# Hooks

Hooks provide reactive state and event wiring for components.

Stable hooks:

- `use_signal`
- `use_input`
- `use_focus`
- `use_interval`
- `use_mouse`

Example:

```rust,no_run
use inky::prelude::*;

let count = use_signal(0);

let ui = BoxNode::new()
    .child(TextNode::new(format!("Count: {}", count.get())))
    .child(
        Input::new()
            .on_submit(move |_| count.update(|n| *n += 1)),
    )
    .into();
```

For higher-level mouse interactions (click, drag, drop), use `Clickable`,
`Draggable`, and `DropZone` components.
