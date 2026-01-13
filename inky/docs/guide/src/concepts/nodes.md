# Nodes

Nodes are the fundamental units of the UI tree. The core node types include:

- `BoxNode`: flexbox container.
- `TextNode`: styled text.
- `StaticNode`: cached subtree for render efficiency.

Example:

```rust
use inky::prelude::*;

let ui = BoxNode::new()
    .padding(1)
    .child(TextNode::new("Title").bold())
    .child(TextNode::new("Body text"))
    .into();
```
