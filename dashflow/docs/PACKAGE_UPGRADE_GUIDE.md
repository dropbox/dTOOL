# Package Upgrade Methodology

**Last Updated:** 2026-01-03 (Worker #2368 - Add repeatable upstream upgrade methodology)

This document describes a repeatable process for upgrading a third-party package or reference implementation while preserving DashFlow-specific integration hooks (observability, telemetry, quality gates, etc.).

---

## Step 1: Baseline Analysis (Upstream)

1. Identify the upstream release artifact you are targeting (crate version / npm version / git tag).
2. Record a reproducible pointer:
   - If a git tag exists: record the tag name.
   - If no tag exists (common for monorepos): record the upstream commit SHA and the release artifact version (e.g., npm `@openai/codex`).
3. Capture upstream structure and high-level module ownership:
   - CLI entrypoints (commands, flags, configuration)
   - Runtime/core libraries
   - Tool integrations (shell, file, search, git)
   - Transport/runtime integrations (MCP, websocket, http)
   - Telemetry / logging / tracing / persistence

**Deliverable:** `docs/<PACKAGE>_UPSTREAM_ANALYSIS.md` with directory layout and module map.

---

## Step 2: Feature Comparison Matrix

Create a feature matrix that compares upstream vs DashFlow implementation and defines an action per feature.

**Template**

| Feature | Upstream | DashFlow | Action | Notes |
|--------:|:-------:|:--------:|:------:|------|
| Interactive chat | ✓ | ? | Port | Identify TUI dependency |
| Tool calls | ✓ | ✓ | Preserve | Ensure telemetry hooks |
| MCP server | ✓ | ? | Add | Follow upstream transport model |

**Deliverable:** `docs/<PACKAGE>_FEATURE_MATRIX.md` (or included in the upgrade plan doc).

---

## Step 3: Incremental Migration Plan

Prefer small, verifiable slices:

1. Update dependencies and build config
2. Port one feature at a time (keep user-facing behavior stable)
3. Preserve DashFlow integration points:
   - WAL/trace persistence (GraphEvents, TelemetrySink)
   - Redaction controls
   - Cost tracking and budget enforcement (where applicable)
4. Add tests that run real code (E2E tests that require keys are `#[ignore]`)
5. Document the delta in a single “upgrade plan” doc

**Deliverable:** `docs/<PACKAGE>_UPGRADE_PLAN.md` with a prioritized checklist.

---

## Step 4: Verification Gates (Required)

1. `./scripts/preflight.sh`
2. `timeout 120 cargo check` (must produce zero warnings)
3. `timeout 300 cargo test` for affected crates
4. For changes involving telemetry persistence, run at least one integration test that validates events/traces are emitted and persisted

**Deliverable:** a WORKER commit that includes the results in the commit message (not in chat).
