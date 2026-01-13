# Archive Notice for codex_dashflow

**Last Updated:** 2026-01-02 (Worker #2337 - Add missing Last Updated headers)

**Repository:** `git@github.com:dropbox/codex_dashflow.git`
**Status:** ARCHIVED

---

## Migration Notice

This repository has been archived. The functionality is being migrated to the DashFlow paragon apps.

### New Location

**Codex DashFlow** is now part of DashFlow's paragon app collection:

```
https://github.com/dropbox/dTOOL/dashflow
├── examples/apps/
│   ├── librarian/        # RAG over classic books
│   ├── codex-dashflow/   # AI-powered code generation and understanding
│   └── common/           # Shared components for paragon apps
```

### Why the Change?

1. **Unified Platform** - All paragon apps share common infrastructure
2. **Better Integration** - Direct access to DashFlow's optimization, introspection, and observability
3. **Consistent Tooling** - Same CLI patterns, telemetry, and deployment across apps
4. **Active Development** - DashFlow is actively maintained with continuous improvements

### Getting Started with DashFlow

```bash
# Clone DashFlow
git clone https://github.com/dropbox/dTOOL/dashflow.git
cd dashflow

# Use Librarian (ready now)
cargo run -p librarian -- query "Who is Captain Ahab?"

# Use Codex DashFlow
cargo run -p codex-dashflow -- --help
```

### Timeline

- **Phase 983:** Archive this repo with migration notice
- **Phase 984:** Create `examples/apps/codex-dashflow` skeleton in DashFlow
- **Phase 985+:** Migrate core functionality

### Questions?

See [ROADMAP_CURRENT.md](https://github.com/dropbox/dTOOL/dashflow/blob/main/ROADMAP_CURRENT.md#part-36-paragon-apps) for the full paragon apps roadmap.

---

## README.md Content for codex_dashflow Repo

Copy this to the codex_dashflow repo's README.md:

```markdown
# ⚠️ ARCHIVED - Migrated to DashFlow

This repository has been archived. **Codex DashFlow** is now part of the DashFlow paragon apps.

## New Location

```bash
git clone https://github.com/dropbox/dTOOL/dashflow.git
cd dashflow/examples/apps/codex-dashflow
```

## Why?

- Unified platform with shared infrastructure
- Better integration with DashFlow's optimization, introspection, and observability
- Active development and maintenance

## See Also

- [DashFlow Paragon Apps](https://github.com/dropbox/dTOOL/dashflow/blob/main/ROADMAP_CURRENT.md#part-36-paragon-apps)
- [Librarian](https://github.com/dropbox/dTOOL/dashflow/tree/main/examples/apps/librarian) - RAG over classic books

---

*This repo is read-only. Please use the DashFlow version for new development.*
```
