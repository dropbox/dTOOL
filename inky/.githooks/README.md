# Git Hooks

This directory contains git hooks for quality enforcement.

## Setup

Run once to enable hooks:

```bash
git config core.hooksPath .githooks
```

## Hooks

### pre-commit

Runs before every commit:

1. **Format check** - `cargo fmt --check`
2. **Clippy** - Zero warnings required
3. **Tests** - All tests must pass
4. **Common issues** - Warns about dbg!, TODO/FIXME

If any check fails, the commit is rejected.

## Bypassing (Emergency Only)

```bash
git commit --no-verify -m "message"
```

Use sparingly. All checks will be enforced in CI.
