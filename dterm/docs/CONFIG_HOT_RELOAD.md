# Configuration Hot Reload

This document describes how to apply runtime configuration updates in dterm-core
without restarting the terminal. This is intended for UI integrations that need
live preference changes (cursor, colors, modes, performance limits).

## API Overview

- `TerminalConfig`: snapshot of all configurable settings.
- `ConfigBuilder`: fluent builder for constructing a config.
- `ConfigChange`: enum describing what changed after applying a config.
- `ConfigObserver`: trait for reacting to config changes in the UI layer.
- `Terminal::apply_config(&TerminalConfig) -> Vec<ConfigChange>`: apply updates.
- `Terminal::current_config() -> TerminalConfig`: capture current settings.

`TerminalConfig` is `Send + Sync` so it can be shared across threads.

## Typical Flow

1. Build a new config from UI preferences.
2. Call `Terminal::apply_config()` and handle returned `ConfigChange` values.
3. Use `Terminal::current_config()` to snapshot and persist state when needed.

```rust
use dterm_core::config::{TerminalConfig, ConfigChange};
use dterm_core::terminal::Terminal;

let mut terminal = Terminal::new(80, 24);

let config = TerminalConfig::builder()
    .cursor_blink(false)
    .cursor_style(dterm_core::config::CursorStyle::Underline)
    .foreground_rgb(0xD0D0D0)
    .background_rgb(0x101010)
    .build();

let changes = terminal.apply_config(&config);
for change in changes {
    // Route UI updates based on change type.
    println!("config change: {:?}", change);
}
```

## Observer Pattern (Optional)

If your UI needs automatic updates, implement `ConfigObserver` and register it
with your UI layer. Use it to translate config changes into renderer updates
(e.g., cursor style or palette refresh).

## Supported Settings (High-Level)

- Cursor: style, blink, color, visibility
- Colors: foreground, background, palette overrides
- Modes: auto-wrap, focus reporting, bracketed paste
- Performance: memory budget

## Files

- `crates/dterm-core/src/config.rs`
- `crates/dterm-core/src/terminal/mod.rs`
