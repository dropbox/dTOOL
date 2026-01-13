# Rust vs Python DashFlow Performance Comparison

## Summary

- **Total benchmarks compared**: 22
- **Average speedup**: 226.2×
- **Median speedup**: 25.6×
- **Minimum speedup**: 0.6×
- **Maximum speedup**: 2432.0×

## Speedup Distribution

- **> 100×**: 3 benchmarks (13.6%)
- **10-100×**: 9 benchmarks (40.9%)
- **2-10×**: 2 benchmarks (9.1%)
- **< 2×**: 8 benchmarks (36.4%)

## Top 10 Performance Improvements

| Rank | Benchmark | Speedup | Python | Rust |
|------|-----------|---------|--------|------|
| 1 | tool_call_simple | 2432.0× | 133.33 μs | 54.82 ns |
| 2 | tool_call_with_processing | 1914.4× | 133.41 μs | 69.69 ns |
| 3 | lambda_runnable_simple | 176.8× | 223.86 μs | 1.27 μs |
| 4 | runnable_batch_10 | 82.1× | 1.06 ms | 12.97 μs |
| 5 | clone_human_message | 78.9× | 2.57 μs | 32.57 ns |
| 6 | clone_ai_message | 72.9× | 2.71 μs | 37.18 ns |
| 7 | passthrough_runnable | 67.3× | 97.27 μs | 1.44 μs |
| 8 | deserialize_human_message_simple | 42.4× | 5.30 μs | 124.94 ns |
| 9 | serialize_message_batch_10 | 29.7× | 35.43 μs | 1.19 μs |
| 10 | serialize_human_message_simple | 26.9× | 2.60 μs | 96.72 ns |

## Full Comparison Table

| Benchmark | Rust | Python | Speedup |
|-----------|------|--------|---------|
| character_splitter_large | 9.29 μs | 5.53 μs | 0.6× |
| character_splitter_medium | 1.93 μs | 1.34 μs | 0.7× |
| character_splitter_small | 1.47 μs | 913.75 ns | 0.6× |
| clone_ai_message | 37.18 ns | 2.71 μs | 72.9× |
| clone_config | 54.72 ns | 45.88 ns | 0.8× |
| clone_human_message | 32.57 ns | 2.57 μs | 78.9× |
| create_config_with_metadata | 130.20 ns | 127.92 ns | 1.0× |
| create_config_with_tags | 67.04 ns | 118.88 ns | 1.8× |
| deserialize_human_message_simple | 124.94 ns | 5.30 μs | 42.4× |
| lambda_runnable_simple | 1.27 μs | 223.86 μs | 176.8× |
| passthrough_runnable | 1.44 μs | 97.27 μs | 67.3× |
| recursive_splitter_large | 487.77 μs | 349.61 μs | 0.7× |
| recursive_splitter_medium | 46.43 μs | 39.20 μs | 0.8× |
| render_complex_template | 443.87 ns | 2.66 μs | 6.0× |
| render_simple_fstring | 122.32 ns | 1.28 μs | 10.5× |
| render_template_long_content | 390.66 ns | 1.91 μs | 4.9× |
| runnable_batch_10 | 12.97 μs | 1.06 ms | 82.1× |
| serialize_ai_message | 164.49 ns | 4.21 μs | 25.6× |
| serialize_human_message_simple | 96.72 ns | 2.60 μs | 26.9× |
| serialize_message_batch_10 | 1.19 μs | 35.43 μs | 29.7× |
| tool_call_simple | 54.82 ns | 133.33 μs | 2432.0× |
| tool_call_with_processing | 69.69 ns | 133.41 μs | 1914.4× |
