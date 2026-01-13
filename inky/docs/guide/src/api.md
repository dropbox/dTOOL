# API Reference

This section summarizes the stability of the public API. Stable APIs should remain
compatible within the current 0.x series. Unstable APIs may change as inky evolves.

## Stable

- Core modules: `app`, `diff`, `layout`, `node`, `style`
- Rendering pipeline: `render` (CPU buffer and painter)
- Components: `Input`, `Select`, `Progress`, `Spinner`, `Scroll`, `Stack`, `Spacer`,
  `Heatmap`, `Sparkline`, `Plot`
- Hooks: `use_signal`, `use_input`, `use_focus`, `use_interval`
- Terminal: `Terminal`, `CrosstermBackend`, events, signal handling
- Styling: `StyleSheet`

## Unstable

- GPU rendering: `render::gpu`, `DtermBackend`
- AI and agent tooling: `perception`, `clipboard`
- High-level systems: `animation`, `accessibility`, `elm`
- AI assistant components: `Markdown`, `ChatView`, `DiffView`, `StatusBar`, `Image`,
  `Transform`

If you depend on unstable APIs, be prepared for minor breaking changes in 0.x.
