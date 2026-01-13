//! Standard UI scenarios for benchmarking.
//!
//! These scenarios define common UI patterns that are benchmarked
//! across different TUI frameworks for fair comparison.

/// Generate test messages for chat UI scenarios.
pub fn generate_messages(count: usize) -> Vec<(String, String)> {
    (0..count)
        .map(|i| {
            let role = if i % 2 == 0 { "user" } else { "assistant" };
            let content = format!(
                "This is message {} with typical content that might appear in a chat. \
                 It includes some text that wraps across lines for realistic rendering.",
                i
            );
            (role.to_string(), content)
        })
        .collect()
}

/// Generate text grid content.
pub fn generate_grid_text(rows: usize, cols: usize) -> Vec<Vec<String>> {
    (0..rows)
        .map(|r| (0..cols).map(|c| format!("[{},{}]", r, c)).collect())
        .collect()
}
