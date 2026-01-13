# Styling

Styles control color, spacing, borders, and text attributes.

```rust
use inky::prelude::*;

let text = TextNode::new("Alert")
    .color(Color::Red)
    .bold()
    .underline();
```

For reusable style sets, use `StyleSheet`:

```rust
use inky::prelude::*;

let mut sheet = StyleSheet::new();
let warning = sheet.define("warning").color(Color::Yellow).bold();

let text = TextNode::new("Caution").style(warning);
```
