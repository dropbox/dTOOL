# DashFlow Upgrade Guide

How to upgrade DashFlow and applications built on it.

---

## Quick Reference

| Task | Command |
|------|---------|
| Upgrade DashFlow | `cd ~/dashflow && git pull` |
| Update app (local path) | Automatic on next `cargo build` |
| Update app (git dep) | `cargo update -p dashflow` |
| Pin to specific version | Use `rev = "commit_hash"` in Cargo.toml |

---

## Dependency Strategies

### Option 1: Local Path (Development)

Best for: Active development on both platform and application.

```toml
# In your app's Cargo.toml
[dependencies]
dashflow = { path = "../dashflow/crates/dashflow" }
dashflow-openai = { path = "../dashflow/crates/dashflow-openai" }
```

**Upgrade process:**
```bash
# 1. Update DashFlow
cd ~/dashflow
git pull origin main

# 2. Rebuild your app (picks up changes automatically)
cd ~/your-app
cargo build
```

**Pros:** Instant updates, easy debugging
**Cons:** Breaking changes propagate immediately

---

### Option 2: Git Dependency (Recommended for Stability)

Best for: Production applications, CI/CD pipelines.

```toml
# In your app's Cargo.toml
[dependencies]
dashflow = { git = "https://github.com/dropbox/dTOOL/dashflow.git", branch = "main" }
dashflow-openai = { git = "https://github.com/dropbox/dTOOL/dashflow.git", branch = "main" }
```

**Upgrade process:**
```bash
# Update to latest on branch
cargo update -p dashflow
cargo update -p dashflow-openai
# ... or update all DashFlow crates at once:
cargo update

# Rebuild
cargo build
```

**Pros:** Controlled updates, reproducible builds via Cargo.lock
**Cons:** Must explicitly update

---

### Option 3: Pinned to Commit (Maximum Stability)

Best for: Production deployments requiring exact reproducibility.

```toml
# In your app's Cargo.toml
[dependencies]
dashflow = { git = "https://github.com/dropbox/dTOOL/dashflow.git", rev = "da2423f" }
```

**Upgrade process:**
```bash
# 1. Find the new commit you want
cd ~/dashflow
git log --oneline -10

# 2. Update the rev in Cargo.toml
# Change: rev = "da2423f"
# To:     rev = "new_commit_hash"

# 3. Rebuild
cargo build
```

**Pros:** Completely reproducible, immune to upstream changes
**Cons:** Manual updates required

---

## Upgrading DashFlow Itself

```bash
cd ~/dashflow

# 1. Check current branch
git branch --show-current

# 2. Fetch latest
git fetch origin

# 3. Check for breaking changes
git log --oneline HEAD..origin/main | head -20

# 4. Pull updates
git pull origin main

# 5. Run tests to verify
cargo test
```

### If You Have Local Changes

```bash
# Option A: Stash and reapply
git stash
git pull origin main
git stash pop

# Option B: Create a feature branch
git checkout -b my-feature
# ... make changes ...
git checkout main
git pull origin main
git checkout my-feature
git rebase main
```

---

## Handling Breaking Changes

### Check the Changelog

```bash
cat ~/dashflow/CHANGELOG.md | head -100
```

### Check API Stability Policy

See `~/dashflow/docs/API_STABILITY.md` for:
- Semantic versioning guarantees
- Deprecation timelines
- Breaking change policy

### Common Migration Patterns

```rust
// Old API (deprecated)
let graph = StateGraph::new();

// New API
let graph = StateGraph::builder().build();
```

Check migration guides in `~/dashflow/docs/` for version-specific changes.

---

## Recommended Workflow

### For Development

1. Use local path dependencies
2. Pull DashFlow updates frequently
3. Fix breaking changes immediately

### For Production

1. Use git dependencies with branch = "main"
2. Update via `cargo update` on a schedule
3. Test thoroughly before deploying
4. Consider pinning to specific commits for critical systems

---

## Troubleshooting

### Build Fails After DashFlow Update

```bash
# Check what changed
cd ~/dashflow
git log --oneline -10
git diff HEAD~5..HEAD --stat

# Look for breaking changes in CHANGELOG
grep -i "breaking" CHANGELOG.md | head -10
```

### Dependency Conflicts

```bash
# Clear Cargo cache and rebuild
cargo clean
cargo build
```

### Reverting to Previous Version

```bash
# With git dependency
cargo update -p dashflow --precise <old_commit>

# With local path
cd ~/dashflow
git checkout <old_commit>
```

---

## Version Compatibility Matrix

| DashFlow Version | Rust Version | Key Changes |
|------------------|--------------|-------------|
| 1.11.x | 1.75+ | Current stable |
| 1.10.x | 1.75+ | Added DashOptimize |
| 1.9.x | 1.70+ | Streaming improvements |

---

## Automation

### Pre-commit Hook for Version Check

Add to your app's `.git/hooks/pre-commit`:

```bash
#!/bin/bash
# Warn if DashFlow is newer than last tested version
DASHFLOW_REV=$(cd ~/dashflow && git rev-parse --short HEAD)
echo "Building against DashFlow $DASHFLOW_REV"
```

### CI/CD Dependency Caching

> **Note:** DashFlow uses internal Dropbox CI, not GitHub Actions. The `.github/` directory does not exist in this repository. The example below is a template for teams using GitHub Actions in their own projects.

```yaml
# GitHub Actions example
- name: Cache DashFlow
  uses: actions/cache@v3
  with:
    path: |
      ~/.cargo/git/db/dashflow-*
    key: dashflow-${{ hashFiles('Cargo.lock') }}
```
