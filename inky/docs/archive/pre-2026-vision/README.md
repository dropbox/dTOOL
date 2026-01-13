# Archive: Pre-2026 Vision Documents

> **ARCHIVED**: These documents capture inky's original vision and architecture before the 2026-01 design refocus. They contain valuable technical detail that informed the current design but are superseded by `/docs/DESIGN_PHILOSOPHY.md` and `/docs/ROADMAP.md`.

---

## Context

These documents were created during inky's initial development phases, focused on:
- Terminal as "the new browser for AI"
- Three rendering tiers (ANSI, Retained, GPU)
- Technical architecture and implementation phases

## Document Index

| Document | Purpose |
|----------|---------|
| `VISION_TERMINAL_NATIVE_EDITOR.md` | Vision for terminal-native editor with buffers, iPhone design, dterm integration |
| `ARCHITECTURE_PLAN.md` | Comprehensive technical specification with API details, phase tracking, benchmarks |

## Key Technical Insights (Still Valid)

These concepts from the original docs remain valid:

1. **Three Rendering Tiers** - ANSI (Tier 1), Retained (Tier 2), GPU (Tier 3)
2. **8-byte Cell Layout** - GPU-compatible cell structure
3. **Taffy Layout Engine** - Flexbox/Grid via Taffy
4. **Zero-latency Input** - Immediate echo system
5. **AI Perception API** - `as_text()`, `as_tokens()`, `semantic_diff()`

## What Changed in 2026

The 2026 design refocus added:

1. **AI as Primary Developer** - Framework optimized for Claude Code and Codex code generation
2. **Pit of Success** - API must be shorter than DIY to be used by AI
3. **Two Paradigms** - Accept both Ink-style (Claude) and ratatui-style (Codex) patterns
4. **God-Tier Output** - Same API call, dramatically better visual result
5. **Strong Opinions** - One canonical way per operation

## Final Output

The original vision evolved into:
- `/docs/DESIGN_PHILOSOPHY.md` - The authoritative design philosophy
- `/docs/ROADMAP.md` - Implementation roadmap

---

*Archived: 2026-01-01*
