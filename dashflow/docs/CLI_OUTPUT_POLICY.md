# CLI Output Policy

**Last Updated:** 2026-01-02 (Worker #2337 - Add missing Last Updated headers)

Guidelines for stdout/stderr output in DashFlow CLI commands.

## Output Types

### User Output (println!)

Use `println!` for:
- **Results**: Final output the user requested (query results, status)
- **Progress**: User-visible progress indicators
- **Tables/Lists**: Formatted data displays
- **Interactive prompts**: Questions to the user

Example:
```rust
// Good: User requested this output
println!("Found {} documents", docs.len());
println!("{}", formatted_table);
```

### Operational Logs (tracing)

Use `tracing::info!`, `tracing::debug!`, etc. for:
- **Debug information**: Internal state, timing
- **Operational details**: Connection status, cache hits
- **Verbose mode output**: Additional info when --verbose is passed

Example:
```rust
// Good: Operational detail, not user-requested
tracing::debug!("Connecting to {}", endpoint);
tracing::info!(documents = docs.len(), "Retrieved documents");
```

### Errors (eprintln! or tracing::error!)

Use `eprintln!` for:
- **User-facing errors**: Validation failures, missing config
- **Fatal errors**: Unrecoverable problems

Use `tracing::error!` for:
- **Operational errors**: Network failures, retries
- **Errors that should appear in logs**

Example:
```rust
// User-facing: use eprintln!
eprintln!("Error: API key not set. Set OPENAI_API_KEY environment variable.");

// Operational: use tracing
tracing::error!(error = ?e, "Failed to connect to endpoint");
```

## Command-Specific Guidelines

### Informational Commands (status, introspect, analyze)
- Primary output: `println!` (user requested this info)
- Connection/timing details: `tracing::debug!`

### Training Commands (train, optimize, eval)
- Progress updates: `println!` with progress bars
- Epoch details: `tracing::info!` (unless --verbose)
- Errors: `eprintln!` for user errors, `tracing::error!` for system errors

### Package Commands (pkg)
- Download progress: `println!`
- Cache operations: `tracing::debug!`
- Verification results: `println!`

### Watch/Streaming Commands (tail, watch)
- Real-time events: `println!` (this is the output)
- Connection status: `tracing::info!`

## General Rules

1. **Don't mix**: If a command uses tracing for user output, the output may not appear without tracing configuration
2. **Test without tracing**: User output should work even if no tracing subscriber is configured
3. **Structured over strings**: Prefer `tracing::info!(count = 5, "Items")` over `tracing::info!("5 items")`
4. **No debug in user output**: Never `println!("{:?}", value)` for user output
5. **Colors matter**: Use `colored` crate for terminal colors in user output

## Migration Strategy

When auditing existing code:
1. Keep user-facing output as println!
2. Convert debug/verbose output to tracing
3. Keep user-facing errors as eprintln!
4. Convert operational errors to tracing::error!
5. Add --verbose flag to show tracing::info! level

## Testing

Test CLI output by:
1. Running without RUST_LOG set (should show user output)
2. Running with RUST_LOG=debug (should show additional tracing)
3. Redirecting stderr to test error messages
