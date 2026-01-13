# Error Message Style Guide

**Last Updated:** 2026-01-02 (Worker #2337 - Add missing Last Updated headers)

Guidelines for consistent error messages across DashFlow crates.

## General Rules

1. **Be specific**: State what went wrong, not just that something failed
2. **Be actionable**: Include what the user can do to fix it
3. **Be consistent**: Use the same format across crates
4. **Be lowercase after colon**: `Error: could not connect` not `Error: Could not connect`
5. **No trailing newline**: The output system handles newlines
6. **No trailing period in single-line errors**: `Error: file not found` not `Error: file not found.`

## Error Format

### User-Facing Errors

Format: `Error: <what failed> - <how to fix>`

```rust
// Good
eprintln!("Error: API key not set. Set OPENAI_API_KEY environment variable.");
eprintln!("Error: file not found: {}", path);
eprintln!("Error: invalid input - expected JSON object");

// Bad
eprintln!("Error!");
eprintln!("Error: {}", e);  // Too vague
eprintln!("ERROR: SOMETHING WENT WRONG");  // Don't shout
```

### API Key Errors

Standard format for missing API keys:

```rust
// Pattern
eprintln!("Error: {provider} API key not set. Set {env_var} environment variable.");

// Examples
eprintln!("Error: OpenAI API key not set. Set OPENAI_API_KEY environment variable.");
eprintln!("Error: Anthropic API key not set. Set ANTHROPIC_API_KEY environment variable.");
```

### File Errors

Include the path:

```rust
// Good
eprintln!("Error: file not found: {}", path);
eprintln!("Error: cannot read file: {} - {}", path, io_error);

// Bad
eprintln!("Error: file not found");  // What file?
```

### Network Errors

Include endpoint and error type:

```rust
// Good
eprintln!("Error: connection failed to {} - {}", endpoint, error);
eprintln!("Error: request timed out after {}s to {}", timeout, endpoint);

// Bad
eprintln!("Error: network error");  // Too vague
```

## Rust Error Types

### thiserror Format

```rust
#[derive(thiserror::Error, Debug)]
pub enum MyError {
    // Good: specific, includes context
    #[error("failed to connect to {endpoint}: {source}")]
    ConnectionFailed {
        endpoint: String,
        #[source]
        source: reqwest::Error,
    },

    // Good: actionable
    #[error("API key not configured. Set {env_var} environment variable")]
    MissingApiKey { env_var: &'static str },

    // Bad: too vague
    #[error("something went wrong")]
    Unknown,
}
```

### anyhow Context

```rust
// Good: adds context at each level
let content = std::fs::read_to_string(&path)
    .with_context(|| format!("failed to read config file: {}", path))?;

let config: Config = serde_json::from_str(&content)
    .with_context(|| format!("failed to parse config file: {}", path))?;

// Bad: loses context
let content = std::fs::read_to_string(&path)?;  // Generic IO error
```

## Error Categories

| Category | Format | Example |
|----------|--------|---------|
| Missing config | `Error: X not set. Set Y.` | `Error: API key not set. Set OPENAI_API_KEY.` |
| File not found | `Error: file not found: <path>` | `Error: file not found: config.yaml` |
| Parse error | `Error: invalid X: <details>` | `Error: invalid JSON: unexpected token at line 5` |
| Network | `Error: <action> failed to <endpoint>: <reason>` | `Error: connection failed to api.openai.com: timeout` |
| Validation | `Error: invalid <field>: <reason>` | `Error: invalid temperature: must be between 0 and 2` |

## Do NOT

1. **Log and return error**: Choose one - don't do both
2. **Include stack traces in user errors**: Use debug logging for that
3. **Use generic errors**: Always include context
4. **Expose internal details**: Don't show internal paths or struct names to users

## Testing Error Messages

```rust
#[test]
fn test_error_message_format() {
    let result = do_something_that_fails();
    let error = result.unwrap_err().to_string();

    // Check format
    assert!(error.starts_with("Error:") || error.contains("failed"));
    // Check context
    assert!(error.contains("config.yaml"));
}
```

## Migration Checklist

When updating existing error messages:
1. Find all `eprintln!("Error` patterns
2. Ensure each includes what failed and how to fix
3. Standardize API key error messages
4. Add file paths to file-related errors
5. Add endpoint to network errors
