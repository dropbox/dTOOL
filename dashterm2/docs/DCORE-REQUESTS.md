# DCore FFI Requests

This file tracks requests from dashterm2 workers for new FFI in dterm-core.

---

## Completed Requests

### Request: DCS Sequence Callbacks
**Date**: 2025-12-28
**Worker**: Manager
**Priority**: High
**Status**: ✅ COMPLETED (dterm-core v0.2.0)
**Description**: DashTerm2 needs DCS sequence support for Sixel graphics and DECRQSS.

**Implemented FFI**:
```c
// DCS callback for Sixel and other DCS sequences
typedef void (*DtermDCSCallback)(void *context, const uint8_t *data, size_t len, uint8_t final_byte);
void dterm_terminal_set_dcs_callback(dterm_terminal_t *term, DtermDCSCallback callback, void *context);

// High-level Sixel API (stub - returns false until Sixel implemented)
typedef struct dterm_sixel_image_t {
    uint32_t width;
    uint32_t height;
    uint32_t *pixels;  // RGBA
} dterm_sixel_image_t;
bool dterm_terminal_has_sixel_image(const dterm_terminal_t *term);
bool dterm_terminal_get_sixel_image(dterm_terminal_t *term, dterm_sixel_image_t *out);
void dterm_sixel_image_free(uint32_t *pixels);
```

**Notes**: The Sixel high-level API returns false until Sixel parsing is implemented (Gap 8 in HINT.md). The DCS callback can be used for raw DCS data.

---

### Request: Line Content Extraction for Search Indexing
**Date**: 2025-12-28
**Worker**: Manager
**Priority**: Medium
**Status**: ✅ COMPLETED (dterm-core v0.2.0)
**Description**: Need to extract line text content for search indexing and comparison with iTerm2's LineBuffer.

**Implemented FFI**:
```c
// Get total lines (visible + scrollback)
size_t dterm_terminal_total_lines(const dterm_terminal_t *term);

// Get text content of a line
// Returns bytes written, or required size if buffer is NULL
// line_index: 0 = first scrollback line, scrollback_lines = first visible row
size_t dterm_terminal_get_line_text(
    const dterm_terminal_t *term,
    size_t line_index,
    char *buffer,
    size_t buffer_size
);

// Convenience: get visible line text (row 0 = top of screen)
size_t dterm_terminal_get_visible_line_text(
    const dterm_terminal_t *term,
    uint16_t row,
    char *buffer,
    size_t buffer_size
);
```

**Usage Example** (Swift):
```swift
// Get total line count
let total = dterm_terminal_total_lines(terminal)

// Get line text (query size first)
let size = dterm_terminal_get_line_text(terminal, lineIndex, nil, 0)
var buffer = [UInt8](repeating: 0, count: size)
dterm_terminal_get_line_text(terminal, lineIndex, &buffer, buffer.count)
let text = String(cString: buffer)
```

---

### Request: Damage Iteration API
**Date**: 2025-12-28
**Worker**: Manager
**Priority**: Medium
**Status**: ✅ COMPLETED (dterm-core v0.2.0)
**Description**: Need to iterate over damaged regions efficiently for rendering.

**Implemented FFI**:
```c
// Damage bounds for a row
typedef struct dterm_row_damage_t {
    uint16_t row;
    uint16_t left;   // first damaged column (inclusive)
    uint16_t right;  // last damaged column (exclusive)
} dterm_row_damage_t;

// Get all damaged rows (returns count, fills array up to max_count)
size_t dterm_terminal_get_damage(
    const dterm_terminal_t *term,
    dterm_row_damage_t *out_damages,
    size_t max_count
);

// Check if specific row is damaged
bool dterm_terminal_row_is_damaged(const dterm_terminal_t *term, uint16_t row);

// Get damage bounds for specific row
bool dterm_terminal_get_row_damage(
    const dterm_terminal_t *term,
    uint16_t row,
    uint16_t *out_left,
    uint16_t *out_right
);
```

**Usage Example** (Swift):
```swift
// Get damaged rows for partial rendering
var damages = [DtermRowDamage](repeating: DtermRowDamage(), count: 100)
let count = dterm_terminal_get_damage(terminal, &damages, 100)
for i in 0..<count {
    let d = damages[Int(i)]
    renderRowRange(row: d.row, left: d.left, right: d.right)
}
dterm_terminal_clear_damage(terminal)
```

---

## Pending Requests

(None - all requested features have been implemented)

---

## Previously Requested (Now Resolved)

### Request: VT100 Implementation Gaps
**Date**: 2025-12-28
**Worker**: #1474
**Priority**: Medium
**Status**: ✅ RESOLVED - All features verified implemented in dterm-core

**Verification (2025-12-29):**

All features listed below are **already implemented** in dterm-core. The test failures in dashterm2 may be due to:
1. Subtle behavior differences between iTerm2 and dterm-core
2. Test harness issues in the comparison tests
3. FFI integration issues (not calling the dterm-core functions correctly)

| Category | Feature | dterm-core Status | Implementation Location |
|----------|---------|-------------------|------------------------|
| **Cursor Movement** | CNL (CSI E) | ✅ Implemented | `terminal/mod.rs:3203-3207` |
| **Cursor Movement** | CPL (CSI F) | ✅ Implemented | `terminal/mod.rs:3208-3212` |
| **Cursor Style** | DECSCUSR | ✅ Implemented | `terminal/mod.rs:3838-3841, 5580-5594` |
| **Character Sets** | Line Drawing Mode | ✅ Implemented | `terminal/mod.rs:1002-1046` (full G0-G3 with SI/SO/SS2/SS3) |
| **Modes** | DECCKM | ✅ Implemented | `terminal/mod.rs` (application_cursor_keys mode) |
| **Modes** | Bracketed Paste (2004) | ✅ Implemented | `terminal/mod.rs` (bracketed_paste mode) |
| **Modes** | Insert Mode (IRM) | ✅ Implemented | `terminal/mod.rs` (insert_mode) |
| **Modes** | Origin Mode (DECOM) | ✅ Implemented | `terminal/mod.rs` (origin_mode with scroll regions) |
| **Control Sequences** | REP (CSI b) | ✅ Implemented | `terminal/mod.rs:4818-4828` |
| **Control Sequences** | RIS | ✅ Implemented | `terminal/mod.rs:3851-3901` |
| **Tab Stops** | CBT (CSI Z) | ✅ Implemented | `terminal/mod.rs` (cursor_backward_tab) |
| **Wide Characters** | Wide char handling | ✅ Implemented | `grid/mod.rs` (wide char spacer cells) |
| **Wide Characters** | Emoji handling | ✅ Implemented | `grapheme/mod.rs` (UAX #29 segmentation) |

**Next Steps for dashterm2:**
1. Verify FFI calls are correctly invoking dterm-core functions
2. Compare specific test inputs byte-by-byte between iTerm2 and dterm-core
3. Check if scrollback/damage tracking tests are using correct APIs

---

## How to Add a Request

```markdown
### Request: [Feature Name]
**Date**: YYYY-MM-DD
**Worker**: #N
**Priority**: High/Medium/Low
**Description**: What you need and why
**Proposed FFI**:
\`\`\`c
// Your proposed C function signatures
\`\`\`
```

---

## Version History

| Version | Date | Changes |
|---------|------|---------|
| v0.2.0 | 2025-12-28 | Added DCS callbacks, line text extraction, damage iteration APIs |
| v0.1.0 | Initial | Basic terminal, grid, parser, search, checkpoint APIs |
