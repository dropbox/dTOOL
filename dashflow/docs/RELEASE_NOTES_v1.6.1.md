# v1.6.1 - Compatibility & Quality

**Release Date:** 2025-11-10
**Type:** Patch Release (Backward Compatible)

## What's Fixed

### v1.0 API Compatibility (Fixes Upgrade Pain!)
- Added compatibility shims for v1.0 → v1.6 migration
- `add_conditional_edge()` now works (shims to `add_conditional_edges()`)
- `add_parallel_edge()` now works (shims to `add_parallel_edges()`)
- Apps using v1.0 API can now upgrade with minimal changes

### GraphBuilder Pattern
- New builder pattern for graph construction
- More extensible API (future features won't break existing code)
- Example: `GraphBuilder::new(graph).with_checkpointer(cp).build()`

### Quality Improvements
- Optimization claims audited for factual accuracy
- README reorganized (clear and focused)
- Mermaid diagram added (visual workflow example)
- Zero warnings maintained
- 59 unused test dependencies removed
- License compliance verified

### Documentation
- Migration guide: v1.0 → v1.6
- App architecture guide (build upgradable apps)
- Framework stability improvements documented

## Migration from v1.6.0

**No breaking changes.** This is a pure quality/compatibility release.

If upgrading from v1.0:
- See `docs/MIGRATION_v1.0_to_v1.6.md`
- v1.0 API now supported via compatibility shims

## Statistics

- 106 commits since v1.6.0
- 30,221 insertions, 3,583 deletions (net: +26,638 lines)
- All tests passing (6,071 tests, 100%)
- Zero compiler and clippy warnings
