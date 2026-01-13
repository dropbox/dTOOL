# Custom Widgets

Implement the `Widget` trait when you need custom rendering logic that can't be expressed with the built-in components.

## The Widget Trait

```rust,no_run
use inky::node::{CustomNode, Widget, WidgetContext};
use inky::prelude::{Color, Node};
use inky::render::{Cell, Painter};

struct Gauge {
    value: f32,
}

impl Widget for Gauge {
    fn render(&self, ctx: &WidgetContext, painter: &mut Painter) {
        let filled = (ctx.width as f32 * self.value) as u16;
        for x in 0..filled.min(ctx.width) {
            let cell = Cell::new('=').with_fg(Color::Green);
            painter.buffer_mut().set(ctx.x + x, ctx.y, cell);
        }
    }

    fn measure(&self, available_width: u16, _available_height: u16) -> (u16, u16) {
        (available_width, 1)
    }
}

// Convert to Node for use in layouts
let node: Node = CustomNode::new(Gauge { value: 0.7 }).into();
```

## WidgetContext

The `WidgetContext` provides layout information:

| Field | Type | Description |
|-------|------|-------------|
| `x` | `u16` | Absolute X position |
| `y` | `u16` | Absolute Y position |
| `width` | `u16` | Allocated width |
| `height` | `u16` | Allocated height |

## measure() Method

The `measure()` method tells the layout engine how much space the widget needs:

```rust,no_run
fn measure(&self, available_width: u16, available_height: u16) -> (u16, u16) {
    // Return (width, height)
    (available_width, 1)  // Full width, 1 row tall
}
```

Return values:
- `(0, 0)` - Widget uses no space
- `(available_width, 1)` - Full width, single line
- `(specific, specific)` - Fixed size

## Example: Progress Meter

```rust,no_run
use inky::node::{CustomNode, Widget, WidgetContext};
use inky::prelude::{Color, Node};
use inky::render::{Cell, Painter};

struct ProgressMeter {
    value: f32,
    label: String,
}

impl Widget for ProgressMeter {
    fn render(&self, ctx: &WidgetContext, painter: &mut Painter) {
        let bar_width = ctx.width.saturating_sub(self.label.len() as u16 + 3);
        let filled = (bar_width as f32 * self.value) as u16;

        // Draw label
        for (i, ch) in self.label.chars().enumerate() {
            let cell = Cell::new(ch).with_fg(Color::White);
            painter.buffer_mut().set(ctx.x + i as u16, ctx.y, cell);
        }

        // Draw bar
        let bar_start = ctx.x + self.label.len() as u16 + 1;
        painter.buffer_mut().set(bar_start, ctx.y, Cell::new('['));

        for x in 0..bar_width {
            let ch = if x < filled { '#' } else { '-' };
            let color = if x < filled { Color::Green } else { Color::DarkGray };
            painter.buffer_mut().set(bar_start + 1 + x, ctx.y, Cell::new(ch).with_fg(color));
        }

        painter.buffer_mut().set(bar_start + bar_width + 1, ctx.y, Cell::new(']'));
    }

    fn measure(&self, available_width: u16, _available_height: u16) -> (u16, u16) {
        (available_width, 1)
    }
}
```

## Using Custom Widgets in Layouts

Custom widgets integrate with flexbox like any other node:

```rust,no_run
use inky::prelude::*;

let layout = BoxNode::new()
    .flex_direction(FlexDirection::Column)
    .gap(1.0)
    .child(TextNode::new("CPU Usage:"))
    .child(CustomNode::new(Gauge { value: 0.45 }))
    .child(TextNode::new("Memory:"))
    .child(CustomNode::new(Gauge { value: 0.72 }))
    .into();
```
