# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |

## Reporting a Vulnerability

If you discover a security vulnerability in inky-tui, please report it responsibly:

1. **Do not** open a public GitHub issue for security vulnerabilities
2. Email the maintainer at security@dropbox.com with details
3. Include:
   - Description of the vulnerability
   - Steps to reproduce
   - Potential impact
   - Any suggested fixes

## Response Timeline

- **Initial Response**: Within 48 hours
- **Assessment**: Within 7 days
- **Fix Timeline**: Depends on severity
  - Critical: Within 24 hours
  - High: Within 7 days
  - Medium: Within 30 days
  - Low: Next release

## Security Considerations

### Terminal Escape Sequences

inky-tui processes terminal escape sequences. The library:
- Validates escape sequence parameters
- Bounds-checks all buffer accesses
- Does not execute shell commands from terminal input

### Memory Safety

- Written in safe Rust with minimal unsafe blocks
- All unsafe blocks have SAFETY comments
- Regularly tested with Miri for undefined behavior
- No `unwrap()` in library code (only in tests/examples)

### Dependencies

Dependencies are regularly audited using `cargo audit`. Run:

```bash
cargo install cargo-audit
cargo audit
```

### Input Validation

- User input is bounded (e.g., input field max length)
- Unicode width is properly calculated
- No buffer overflows possible due to Rust's safety guarantees

## Known Limitations

- OSC 52 clipboard operations trust the terminal emulator
- Signal handlers modify global state (thread-safe via atomic operations)
- GPU buffer access (Tier 3) requires trusting the terminal's shared memory
