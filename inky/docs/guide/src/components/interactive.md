# Interactive Components

These components are stable and intended for general use:

- `Input`
- `Select`
- `Progress`
- `Spinner`
- `Scroll`
- `Clickable`
- `Draggable`
- `DropZone`
- `Stack`
- `Spacer`

Example:

```rust
use inky::prelude::*;

let form = BoxNode::new()
    .child(Input::new().placeholder("Search"))
    .child(Select::new(vec![
        SelectOption::new("One"),
        SelectOption::new("Two"),
    ]))
    .into();
```

Mouse interaction example:

```rust
use inky::prelude::*;

let _button = Clickable::new(TextNode::new("Click me"))
    .on_click(|event| {
        println!("Clicked at ({}, {})", event.local_x, event.local_y);
    });

let _draggable = Draggable::new(TextNode::new("Drag me"));
let _drop_zone = DropZone::new(TextNode::new("Drop here"));
```
