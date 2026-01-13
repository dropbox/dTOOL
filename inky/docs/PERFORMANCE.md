# Inky Performance Guide

## TL;DR: Inky is Fast

When used correctly (stable tree between frames), inky's render pipeline is **1.8x faster than ratatui**.

| Scenario | inky (render-only) | ratatui | Comparison |
|----------|-------------------|---------|------------|
| 100 messages | **54µs** | 99µs | inky 1.8x faster |
| 10 messages | **7.4µs** | ~10µs | inky faster |

## Understanding the Numbers

Inky has two rendering modes:

### 1. Incremental Render (Real Apps)

For real applications where the UI structure doesn't change every frame:

```
Tree Building → Layout (cached) → Render → Buffer → Diff → Terminal
                    ↓
              ~0µs (cached)
```

**Performance:** 50-100µs for typical UIs

This is the common case:
- Chat apps: messages don't change every keystroke
- File browsers: directory listing is stable
- Log viewers: content is append-only
- Dashboards: data changes, structure doesn't

### 2. Cold Render (Benchmarks)

For synthetic benchmarks that rebuild the entire tree every frame:

```
Tree Building → Layout (full compute) → Render → Buffer → Diff → Terminal
                    ↓
              2-3ms (Taffy)
```

**Performance:** 2-5ms depending on tree complexity

This represents the worst case:
- First render of a new screen
- After major structural changes
- Synthetic benchmarks

## Why Cold Renders Are Slower

The bottleneck is **Taffy flexbox layout computation**, not inky's code:

| Phase | Time (100 msgs) | % Total |
|-------|-----------------|---------|
| Taffy layout | ~2.6ms | 94% |
| Tree building | ~100µs | 4% |
| Hash computation | ~50µs | 2% |
| Render to buffer | ~54µs | 2% |

Taffy provides full CSS Flexbox semantics (like the browser), which is inherently more expensive than ratatui's immediate-mode layout.

## Optimizing Your App

### DO: Keep Tree Structure Stable

```rust
// Good: Update data, keep structure
fn render(messages: &[Message]) -> Node {
    BoxNode::new()
        .child(header())           // Always present
        .child(message_list(messages))  // Content changes
        .child(input_bar())        // Always present
        .into()
}
```

### DON'T: Rebuild Entire Tree Unnecessarily

```rust
// Bad: Creating new wrapper nodes for no reason
fn render(messages: &[Message]) -> Node {
    if messages.len() > 10 {
        BoxNode::new()  // Different structure!
            .child(scroll_wrapper(messages))
            .into()
    } else {
        BoxNode::new()  // Forces full re-layout
            .child(message_list(messages))
            .into()
    }
}
```

### DO: Use Conditional Content, Not Structure

```rust
// Good: Same structure, different visibility
fn render(show_details: bool, item: &Item) -> Node {
    BoxNode::new()
        .child(item_header(item))
        .child(if show_details {
            item_details(item)
        } else {
            TextNode::new("")  // Placeholder preserves structure
        })
        .into()
}
```

## Benchmark Results

### Cold Benchmarks (Worst Case)

New tree every frame - primarily measures Taffy layout time:

| Scenario | Time |
|----------|------|
| chat_ui/10_msgs | ~1.0ms |
| chat_ui/100_msgs | ~2.8ms |
| chat_ui/1000_msgs | ~13ms |
| text_grid/10x10 | ~1.4ms |
| text_grid/50x50 | ~10ms |

### Incremental Benchmarks (Realistic)

Same tree, render only:

| Scenario | Render Only | With Layout Cache |
|----------|-------------|-------------------|
| chat_ui/10_msgs | **7.4µs** | 12µs |
| chat_ui/100_msgs | **54µs** | 95µs |

## Layout Caching

Inky automatically caches layout results when:
1. Tree structure is semantically unchanged (same node types, styles, text)
2. Viewport size is unchanged

The caching uses structure hashing - even though NodeIds change on every render, equivalent trees produce the same hash and reuse cached layout.

## When Inky Excels

- **Streaming output**: Append-only content is very fast
- **Stable UIs**: Chat, file browsers, dashboards
- **Complex layouts**: Flexbox gives you powerful layout with minimal code

## When to Consider Alternatives

- **Extremely simple UIs**: If you only need `printf`-style output, use println
- **Canvas-style graphics**: If drawing individual pixels, use ratatui
- **Rebuilding entire UI each frame**: Consider if you really need to

## Comparison with ratatui

| Aspect | inky | ratatui |
|--------|------|---------|
| Layout model | Flexbox (automatic) | Manual (you specify x,y,w,h) |
| Cold render | Slower (Taffy overhead) | Faster (no layout engine) |
| Incremental render | **Faster** (caching) | - |
| Developer ergonomics | Higher (React-like) | Lower (more control) |
| Memory | Higher (tree + layout) | Lower (immediate mode) |

Choose based on your needs:
- **inky**: Complex UIs, productivity tools, AI apps
- **ratatui**: Simple UIs, games, visualizations

## Future Optimizations

Planned improvements (see WORKER_DIRECTIONS.md):
1. Arena allocation for nodes (reduce cold render allocations)
2. Bypass Taffy for simple layouts (row/column only)
3. Parallel layout computation for independent subtrees
