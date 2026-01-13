# DTermCore Status

**Last Updated:** 2026-01-01
**Estimated Completion:** ~90% for DashTerm2 integration

---

## Completed Features

### Box Drawing (Commit #399)
- **Status:** COMPLETE
- **Characters:** U+2500-U+257F (Box Drawing), U+2580-U+259F (Block Elements), U+25E2-U+25FF (Geometric Shapes), U+1FB00-U+1FB3C (Sextants)
- **Hybrid Renderer:** Fixed - now calls `box_drawing::generate_box_drawing_vertices()`
- **Tests:** Comprehensive visibility tests added

### Grid Adapter Box Drawing Detection (Commit #401)
- **Status:** COMPLETE
- **Function:** `dterm_is_box_drawing_character(codepoint: u32) -> bool`
- **Usage from Objective-C:**
  ```objc
  uint32_t codepoint = cell.codepoint;
  BOOL isBoxDrawing = dterm_is_box_drawing_character(codepoint);
  ```

---

### Powerline Glyphs (Commit #401)
- **Status:** COMPLETE
- **Characters:** U+E0A0-U+E0D7
- **Next Steps:** Integrate in DashTerm2 and validate prompt rendering

### Dotted/Dashed Underlines (Commit #402)
- **Status:** COMPLETE
- **SGR:** 4:4 (dotted), 4:5 (dashed)
- **Next Steps:** Integrate in DashTerm2 and validate underline styles

### iTerm2 Inline Images (OSC 1337) (Commit #403)
- **Status:** COMPLETE
- **Protocol:** `ESC ] 1337 ; File=... ST`
- **Next Steps:** Integrate inline image FFI in DashTerm2

---

## In Progress

None (core).

---

## External Blockers

- DashTerm2 Metal shader update for 7-bit vertex flags (see `docs/METAL_SHADER_MIGRATION.md` and `READY-FOR-INTEGRATION-METAL-SHADER.md`).
- Buildkite agent provisioning to unblock Windows/Linux CI (see `docs/BUILDKITE_PROVISIONING.md`).

---

## Not Started

None.

---

## Integration Checklist

DashTerm2 can enable dterm-core settings when:

| Setting | Dependency | Status |
|---------|------------|--------|
| `dtermCoreEnabled` | Parser ready | READY |
| `dtermCoreParserOutputEnabled` | Grid adapter ready | READY |
| `dtermCoreGridEnabled` | Box drawing detection | READY (commit #401) |
| `dtermCoreRendererEnabled` | Box drawing rendering | READY (commit #399) |

---

## Ready for Integration

- `READY-FOR-INTEGRATION-BOX-DRAWING.md`
- `READY-FOR-INTEGRATION-POWERLINE.md`
- `READY-FOR-INTEGRATION-UNDERLINES.md`
- `READY-FOR-INTEGRATION-ITERM-IMAGES.md`
- `READY-FOR-INTEGRATION-METAL-SHADER.md`

---

*This file is auto-updated by dterm-core AI*
