# Visualization Components

These components are stable and render inline charts:

- `Heatmap`
- `Sparkline`
- `Plot`

Example:

```rust
use inky::prelude::*;

let chart = Plot::new()
    .series(Series::line(vec![0.1, 0.2, 0.4, 0.8]))
    .width(Dimension::percent(100.0))
    .height(Dimension::px(10.0));
```
