# Archive: 2026-01 Design Exploration

> **ARCHIVED**: These documents capture the design exploration process that led to inky's final design philosophy. They are preserved for historical reference but are superseded by `/docs/DESIGN_PHILOSOPHY.md` and `/docs/ROADMAP.md`.

---

## Context

These documents were created during a design conversation on 2026-01-01, exploring how inky should serve AI code generation (Claude Code, OpenAI Codex) for building god-tier terminal applications.

## Document Index

| Document | Purpose |
|----------|---------|
| `CODEX_PORTER_PHASE7_FEEDBACK.md` | Feedback from porting Codex TUI to inky |
| `DESIGN_PROPOSALS_2026.md` | Three proposals for merging node-tree and lines-and-spans |
| `TERMINAL_REALITY_2026.md` | Analysis of what modern terminals actually support |
| `UNIFIED_ARCHITECTURE.md` | Proposal for unifying both paradigms via `Into<Node>` |
| `TERMINAL_HTML.md` | Exploration of HTML-like semantic primitives |
| `CRITICAL_ANALYSIS_2026.md` | Self-critique of the design proposals |
| `THESIS_2026_TERMINAL.md` | Core thesis for 2026 terminal framework |
| `CRITIQUE_FROM_AI_USER.md` | 10 critiques from AI user perspective |
| `SYNTHESIS_STRONG_OPINIONS.md` | Reconciliation: 8 strong opinions |
| `AI_AS_DEVELOPER.md` | Key insight: AI is the primary developer |
| `PIT_OF_SUCCESS.md` | Making framework shorter than DIY |
| `INKY_MANIFESTO.md` | Final manifesto: god-tier apps for Claude + Codex |
| `TWO_AIS_TWO_PARADIGMS.md` | Bridging Ink (Claude) and ratatui (Codex) patterns |
| `BETTER_THAN_EXPEDIENT.md` | Why inky must exceed both Ink and ratatui |

## Key Insights (Summary)

1. **AI is the developer** - Framework optimizes for AI code generation, not human developers
2. **Two AIs, two paradigms** - Must accept both Ink-style and ratatui-style patterns
3. **Shorter wins** - Framework must require fewer tokens than DIY
4. **God-tier output** - Same API call, dramatically better visual result
5. **Strong opinions** - One canonical way per operation reduces AI sprawl

## Final Output

The exploration culminated in:
- `/docs/DESIGN_PHILOSOPHY.md` - The authoritative design philosophy
- `/docs/ROADMAP.md` - Implementation roadmap

---

*Archived: 2026-01-01*
