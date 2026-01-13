# MANAGER DIRECTIVE - DashTerm2 Metal Shader Migration

**Date:** 2025-12-31
**From:** MANAGER
**To:** DashTerm2 maintainers / DashTerm2 AI
**Priority:** HIGH
**Status:** ACTION REQUIRED

---

## Action Required

DashTerm2's Metal shader must be updated to use dterm-core's new 7-bit vertex
flag layout. This is required for correct cursor, selection, and decoration
rendering with current dterm-core output.

See `docs/METAL_SHADER_MIGRATION.md` for:
- new flag layout and constants
- Metal shader template
- migration checklist

