# Layout

Layout is powered by Taffy and follows CSS flexbox rules. Use style setters on nodes
or the `style!{}` macro to configure layout properties.

```rust
use inky::prelude::*;

let row = BoxNode::new()
    .flex_direction(FlexDirection::Row)
    .justify_content(JustifyContent::SpaceBetween)
    .child(TextNode::new("Left"))
    .child(TextNode::new("Right"))
    .into();
```
