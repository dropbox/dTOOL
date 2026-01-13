# Getting Started

Add inky to your Cargo.toml:

```toml
[dependencies]
inky = "0.1"
```

Minimal example:

```rust,no_run
use inky::prelude::*;

fn main() -> Result<()> {
    App::new()
        .render(|_ctx| {
            BoxNode::new()
                .padding(1)
                .child(TextNode::new("Hello, inky").color(Color::Cyan).bold())
                .into()
        })
        .run()?;
    Ok(())
}
```

Run an example from the repo:

```bash
cargo run --example hello
```
