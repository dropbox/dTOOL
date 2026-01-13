# dterm-alacritty-bridge

Compatibility layer to run Alacritty-style frontends on top of dterm-core.

Status: minimal grid/term wrapper. API coverage will expand as integration
work continues.

## Usage

```rust
use dterm_alacritty_bridge::{Config, Term, VoidListener};

let config = Config::default();
let dims = (24usize, 80usize);
let mut term = Term::new(config, &dims, VoidListener);
term.process(b"echo hello\r");
```
