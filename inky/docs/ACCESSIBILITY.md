# Accessibility Guide

This document describes the accessibility features in inky and provides guidance for building accessible terminal UI applications.

## Overview

inky provides foundational accessibility features for terminal UIs:

| Feature | Status | Notes |
|---------|--------|-------|
| Keyboard navigation | Implemented | Tab/Shift+Tab focus cycling |
| Focus management | Implemented | FocusContext, FocusHandle |
| Colorblind-safe palettes | Partial | Viridis, Grayscale recommended |
| Screen reader support | Not implemented | Future work |
| High contrast mode | Not implemented | Future work |

## Keyboard Navigation

### Built-in Shortcuts

| Key | Action |
|-----|--------|
| Tab | Focus next component |
| Shift+Tab | Focus previous component |
| Ctrl+C | Quit application |

### Component Navigation Methods

Components expose navigation methods that applications can wire to keyboard events:

**Input component:**
- `move_left()`, `move_right()` - cursor movement
- `move_home()`, `move_end()` - jump to start/end
- `insert(char)`, `backspace()`, `delete()` - text editing

**Select component:**
- `select_next()`, `select_prev()` - move selection
- `select_first()`, `select_last()` - jump to start/end
- Automatically skips disabled options

**Scroll component:**
- `scroll_up()`, `scroll_down()` - line scrolling
- `page_up()`, `page_down()` - page scrolling
- `scroll_to_top()`, `scroll_to_bottom()` - jump to edges
- `scroll_into_view(line)` - ensure line is visible

### Wiring Keyboard Events

```rust
use inky::prelude::*;
use crossterm::event::{KeyCode, KeyEvent};

fn handle_key(select: &mut Select, key: KeyEvent) {
    match key.code {
        KeyCode::Up => select.select_prev(),
        KeyCode::Down => select.select_next(),
        KeyCode::Home => select.select_first(),
        KeyCode::End => select.select_last(),
        KeyCode::Enter => {
            // Handle selection
            if let Some(value) = select.get_selected_value() {
                println!("Selected: {}", value);
            }
        }
        _ => {}
    }
}
```

## Focus Management

### Using Focus Hooks

```rust
use inky::hooks::{use_focus, FocusHandle};

// Get a focus handle for your component
let handle: FocusHandle = use_focus();

// Check if focused
if handle.is_focused() {
    // Render with focus styling
}

// Programmatically focus
handle.focus();

// Remove focus
handle.blur();
```

### Focus Navigation Functions

```rust
use inky::hooks::{focus_next, focus_prev};

// Move focus forward (called automatically on Tab)
focus_next();

// Move focus backward (called automatically on Shift+Tab)
focus_prev();
```

### Focus Order

Components are focused in registration order (the order `use_focus()` is called). To control focus order, register components in your desired sequence.

## Color Accessibility

### Colorblind-Safe Palettes

For data visualization, use colorblind-safe palettes:

```rust
use inky::components::{Heatmap, HeatmapPalette};

// Recommended: Viridis is colorblind-safe
let heatmap = Heatmap::new(data)
    .palette(HeatmapPalette::Viridis);

// Also safe: Grayscale
let heatmap = Heatmap::new(data)
    .palette(HeatmapPalette::Grayscale);

// NOT colorblind-safe: RedGreen
// Avoid unless absolutely necessary
let heatmap = Heatmap::new(data)
    .palette(HeatmapPalette::RedGreen);  // 8% of men cannot distinguish
```

### Palette Recommendations

| Palette | Colorblind-Safe | Best For |
|---------|-----------------|----------|
| Viridis | Yes | General data visualization |
| Grayscale | Yes | Maximum compatibility |
| Plasma | Mostly | Scientific visualization |
| Heat | Mostly | Temperature/intensity data |
| Cool | Mostly | Temperature/water data |
| RedGreen | No | Avoid for accessibility |

### Custom Colors

When choosing colors for your UI:

1. **Avoid red/green alone** - Use additional cues (icons, patterns, position)
2. **Ensure sufficient contrast** - Dark text on light backgrounds or vice versa
3. **Don't rely solely on color** - Add labels, borders, or symbols
4. **Test with colorblind simulators** - Tools like Color Oracle can help

## Focus Indicators

Components should provide visual focus indicators. inky components use these conventions:

- **Input**: Border color changes, cursor visible
- **Select**: Selected item is bold when focused
- Default focus color: `Color::BrightCyan`

### Customizing Focus Appearance

```rust
use inky::components::Input;
use inky::style::Color;

let input = Input::new()
    .focus_color(Color::BrightYellow)  // Custom focus color
    .focused(true);
```

## Best Practices

### 1. Support Keyboard-Only Navigation

Ensure all functionality is accessible via keyboard:

```rust
// Handle arrow keys for lists
match key.code {
    KeyCode::Up => select.select_prev(),
    KeyCode::Down => select.select_next(),
    // ...
}
```

### 2. Provide Clear Focus Indicators

Make it obvious which element has focus:

```rust
let border_color = if is_focused {
    Color::BrightCyan
} else {
    Color::White
};
```

### 3. Use Semantic Indicators

Add non-color cues for important information:

```rust
// Good: Uses symbol AND color
let status = if success {
    TextNode::new("✓ Success").color(Color::Green)
} else {
    TextNode::new("✗ Error").color(Color::Red)
};

// Better: Also works in monochrome
let status = if success {
    TextNode::new("[OK] Success").color(Color::Green)
} else {
    TextNode::new("[ERR] Error").color(Color::Red)
};
```

### 4. Disable Options Properly

Use the disabled state for unavailable options:

```rust
use inky::components::SelectOption;

let options = vec![
    SelectOption::new("Available"),
    SelectOption::new("Unavailable").disabled(),  // Grayed out, skipped in nav
];
```

### 5. Support Password Masking

For sensitive input, use password mode:

```rust
use inky::components::Input;

let password_field = Input::new()
    .placeholder("Password")
    .password()  // Characters shown as •
    .mask_char('*');  // Custom mask character
```

## Future Work

The following accessibility features are planned for future releases:

1. **Screen reader integration** - Announce focus changes and content
2. **High contrast themes** - System-wide high contrast mode
3. **Reduced motion** - Respect OS reduced motion preferences
4. **Tab index** - Explicit focus order control
5. **Focus traps** - Keep focus within modal dialogs
6. **Skip links** - Jump to main content
7. **ARIA-like semantics** - Role and label system for components

## Testing Accessibility

### Manual Testing

1. **Keyboard-only test**: Unplug your mouse and navigate your entire UI
2. **Color test**: View your UI in grayscale (many monitors have this option)
3. **Focus test**: Verify you can always see which element is focused

### Automated Testing

```rust
#[test]
fn test_focus_navigation() {
    let ctx = Arc::new(RwLock::new(FocusContext::new()));

    let handle1 = FocusHandle::new(NodeId::new(), ctx.clone());
    let handle2 = FocusHandle::new(NodeId::new(), ctx.clone());

    ctx.write().unwrap().focus_next();
    assert!(handle1.is_focused());

    ctx.write().unwrap().focus_next();
    assert!(handle2.is_focused());
}
```

## Resources

- [WCAG 2.1 Guidelines](https://www.w3.org/WAI/WCAG21/quickref/)
- [Color Oracle](https://colororacle.org/) - Colorblind simulator
- [Viridis Colormap](https://cran.r-project.org/web/packages/viridis/) - Colorblind-safe palette
