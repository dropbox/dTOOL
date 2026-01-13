# DTermCore vs iTerm2 VT100 Feature Parity Analysis

**Generated:** 2025-12-28
**Updated:** 2025-12-28 - Added 60+ comparison tests for DCS/DSR/modes/edge cases
**Previous:** Added DECRQSS, reverse wraparound mode 45, DECSCNM, cursor blink, DECNKM, DECCOLM, DECALN
**Purpose:** Track VT100/xterm feature gaps between dterm-core (Rust) and iTerm2 (ObjC)

## ⚠️ CRITICAL: Library Rebuild Required

**Issue:** The `DTermCore/lib/libdterm_core.a` library does not export the dterm FFI symbols.
The Rust library compiles but symbols are not being exported to the static library.

**Symptoms:**
- All `_dterm_terminal_*` symbols are undefined during linking
- Test target fails to build
- DTermCoreComparisonTests cannot run

**Resolution Required:**
1. In the dterm-core Rust project, ensure `crate-type = ["staticlib"]` in Cargo.toml
2. Add `#[no_mangle] pub extern "C" fn` to all FFI functions
3. Rebuild with `cargo build --release`
4. Copy the rebuilt `.a` file to `DTermCore/lib/libdterm_core.a`

**Blocked Tests (60+):** All DTermCoreComparisonTests are blocked until library is rebuilt.

---

## Executive Summary

**dterm-core** is a focused terminal emulation core with excellent performance (~180-240 MB/s throughput) and growing feature coverage. **iTerm2's VT100Terminal** is a mature, feature-complete implementation supporting virtually every VT100/VT200/VT300/xterm extension.

### dterm-core Strengths
- **Performance**: 180-240 MB/s parse throughput (benchmarked)
- **Memory Efficiency**: 12 bytes/cell vs 16 bytes/cell for iTerm2
- **Memory Safety**: Rust prevents buffer overflows and crashes
- **Tiered Scrollback**: Ring buffer + hot/warm/cold tiers with compression
- **Damage Tracking**: Efficient dirty region tracking for rendering
- **Mouse Reporting**: Full mouse tracking with X10 and SGR encoding

### Current Feature Coverage

| Category | dterm-core | iTerm2 | Gap |
|----------|-----------|--------|-----|
| Basic ASCII | 100% | 100% | None |
| Cursor Movement | ~90% | 100% | Minor |
| SGR Attributes | ~90% | 100% | Underline color |
| Erase Operations | ~90% | 100% | Minor |
| Private Modes | **~90%** | 100% | Minor |
| Mouse Reporting | **100%** | 100% | **None** ✓ |
| Character Sets | **100%** | 100% | **None** ✓ |
| OSC Sequences | **~70%** | 100% | **Moderate** |
| DCS Sequences | **~30%** | 100% | **Moderate** |
| Device Reports | **~90%** | 100% | **Minor** ✓ |
| Image Support | 0% | 100% | Large |
| **Hyperlinks** | **100%** | 100% | **None** ✓ |
| **Color Palette** | **100%** | 100% | **None** ✓ |

---

## ✅ Recently Completed Features

### DECRQSS - Request Selection or Setting (COMPLETE - 2025-12-28)
**Impact:** Applications can query terminal settings via DCS sequences

DECRQSS (DEC Request Selection or Setting) allows applications to query the current
value of terminal settings. The terminal responds with DECRPSS containing the current
setting value.

**Format:**
- Request: `DCS $ q <setting> ST` (ESC P $ q <setting> ESC \)
- Response: `DCS 1 $ r <value><setting> ST` (success) or `DCS 0 $ r ST` (error)

**Supported Settings:**

| Setting | Mnemonic | Description | Example Response |
|---------|----------|-------------|------------------|
| SGR | `m` | Current text attributes | `0;1;31m` (reset+bold+red) |
| DECSCUSR | `(space)q` | Cursor style | `2 q` (steady block) |
| DECSTBM | `r` | Scroll region margins | `1;24r` (top=1, bottom=24) |
| DECSCL | `"p` | Conformance level | `63;1"p` (VT300) |
| DECSCA | `"q` | Protected attribute | `0"q` (no protection) |
| DECSLPP | `t` | Lines per page | `24t` |

**Implementation Details:**
- DCS sequence parsing via `dcs_hook`, `dcs_put`, `dcs_unhook` callbacks
- State machine tracks DCS type (Decrqss, Unknown, None)
- Data bytes accumulated up to 256 byte limit
- Response generated on `dcs_unhook` and queued to response buffer

**Example:**
```bash
# Query current SGR attributes
printf '\eP$qm\e\\'
# Response: ESC P 1 $ r 0 m ESC \ (default style)

# Set bold+red, then query
printf '\e[1;31m'
printf '\eP$qm\e\\'
# Response: ESC P 1 $ r 0;1;31 m ESC \

# Query cursor style
printf '\eP$q q\e\\'
# Response: ESC P 1 $ r 1  q ESC \ (blinking block)

# Query unknown setting (returns error)
printf '\eP$qx\e\\'
# Response: ESC P 0 $ r ESC \
```

**Use Cases:**
- Terminal multiplexers (tmux, screen) querying terminal state
- Applications that need to preserve and restore terminal settings
- Terminal capability detection beyond terminfo

### Reverse Wraparound Mode (COMPLETE - 2025-12-28)
**Impact:** Backspace navigation across line boundaries for editors and line-editing applications

Reverse wraparound mode (DEC private mode 45) allows the cursor to move backwards
across line boundaries. When enabled, pressing backspace at column 0 wraps the
cursor to the last column of the previous line.

- Enable: `CSI ? 45 h`
- Disable: `CSI ? 45 l`

**FFI Functions:**
- `dterm_terminal_is_reverse_wraparound()` - Check if reverse wraparound is enabled
- `modes.reverse_wraparound` - Mode flag in `dterm_modes_t`

**Swift API:**
- `terminal.isReverseWraparound` - Bool property
- `terminal.modes.reverseWraparound` - In DTermModes struct

**Example:**
```bash
# Enable reverse wraparound
printf '\e[?45h'

# Now backspace at column 0 wraps to previous line

# Disable reverse wraparound
printf '\e[?45l'
```

**Use Cases:**
- Line editors that allow cursor movement across wrapped lines
- Terminal-based text editors with seamless backspace behavior
- Applications that implement their own line wrapping

### Additional Private Modes (COMPLETE - 2025-12-28)
**Impact:** Expanded DEC private mode support for terminal applications

Newly exposed private modes and escape sequences:

#### DECSCNM - Reverse Video Mode (Mode 5)
Inverts foreground and background colors for the entire screen.
- Enable: `CSI ? 5 h`
- Disable: `CSI ? 5 l`

**FFI Functions:**
- `dterm_terminal_is_reverse_video()` - Check if reverse video is enabled
- `modes.reverse_video` - Mode flag in `dterm_modes_t`

**Swift API:**
- `terminal.isReverseVideo` - Bool property
- `terminal.modes.reverseVideo` - In DTermModes struct

**Example:**
```bash
# Enable reverse video
printf '\e[?5h'

# Disable reverse video
printf '\e[?5l'
```

#### Cursor Blink Mode (Mode 12)
Controls whether the cursor should blink.
- Enable: `CSI ? 12 h`
- Disable: `CSI ? 12 l`

**FFI Functions:**
- `dterm_terminal_cursor_blink_enabled()` - Check if cursor blink is enabled
- `modes.cursor_blink` - Mode flag in `dterm_modes_t`

**Swift API:**
- `terminal.cursorBlinkEnabled` - Bool property
- `terminal.modes.cursorBlink` - In DTermModes struct

#### DECNKM - Application Keypad Mode (Mode 66)
Controls numeric keypad behavior. When enabled, the numeric keypad sends
application escape sequences instead of numeric characters.
- Enable: `CSI ? 66 h`
- Disable: `CSI ? 66 l`

**FFI Functions:**
- `dterm_terminal_application_keypad_enabled()` - Check if application keypad is enabled
- `modes.application_keypad` - Mode flag in `dterm_modes_t`

**Swift API:**
- `terminal.applicationKeypadEnabled` - Bool property
- `terminal.modes.applicationKeypad` - In DTermModes struct

#### DECCOLM - 132-Column Mode (Mode 3)
Switches between 80-column and 132-column modes.
- Enable 132-column: `CSI ? 3 h`
- Enable 80-column: `CSI ? 3 l`

Note: This mode affects the logical column count. The host application should
resize the terminal accordingly when this mode changes.

**FFI Functions:**
- `dterm_terminal_is_132_column_mode()` - Check if 132-column mode is enabled
- `modes.column_mode_132` - Mode flag in `dterm_modes_t`

**Swift API:**
- `terminal.is132ColumnMode` - Bool property
- `terminal.modes.columnMode132` - In DTermModes struct

#### DECALN - Screen Alignment Test (ESC # 8)
Fills the screen with uppercase 'E' characters for alignment testing.
Used to verify character spacing and screen alignment.

**FFI Functions:**
- `dterm_terminal_screen_alignment_test()` - Execute alignment test

**Swift API:**
- `terminal.screenAlignmentTest()` - Method to execute

**Example:**
```bash
# Fill screen with 'E' characters
printf '\e#8'
```

### Left-Right Margin Mode (COMPLETE - 2025-12-28)
**Impact:** Full support for split-screen terminal applications and complex TUI layouts

Complete implementation of DEC private mode 69 (DECLRMM) and DECSLRM:
- **Mode 69 (DECLRMM)** - Enable/disable left-right margin mode via `CSI ? 69 h/l`
- **DECSLRM** - Set left-right margins via `CSI Pl ; Pr s`

**Features Implemented:**
- Cursor movement respects left-right margins (forward, back, positioning)
- Carriage return (CR) moves to left margin
- Next line (NEL) moves to left margin of next line
- Line wrap respects right margin
- Tab stops limited by right margin
- Insert/delete characters bounded by margins
- Insert/delete lines affect only columns within margins

**API Functions:**
- `left_right_margins()` - Get current margins (0-indexed, inclusive)
- `modes().left_right_margin_mode` - Check if mode is enabled

**FFI Functions:**
- `dterm_terminal_is_left_right_margin_mode_active()` - Check mode status
- `dterm_terminal_get_left_right_margins()` - Get margin bounds

**Example:**
```bash
# Enable left-right margin mode
printf '\e[?69h'

# Set margins: left=10, right=60 (1-indexed)
printf '\e[10;60s'

# Text and cursor movement now bounded by columns 10-60

# Disable mode (resets margins to full width)
printf '\e[?69l'
```

### Color Palette Support (COMPLETE - 2025-12-28)
**Impact:** Full color theming support for terminal applications

Complete implementation of xterm 256-color palette management including:
- **OSC 4** - Query and set individual palette colors (0-255)
- **OSC 10-19** - Dynamic colors (foreground, background, cursor, selection, etc.)
- **OSC 104** - Reset individual or all palette colors
- **OSC 110-119** - Reset dynamic colors

**Color Formats Supported:**
- `rgb:RRRR/GGGG/BBBB` - X11 color specification (1-4 hex digits per component)
- `#RGB`, `#RRGGBB`, `#RRRRGGGGBBBB` - Hash notation
- Named colors: black, red, green, blue, yellow, magenta, cyan, white, gray, orange

**API Functions:**
- `palette_color(index)` - Get palette color
- `set_palette_color(index, rgb)` - Set palette color
- `reset_palette_color(index)` - Reset to default
- `reset_all_colors()` - Reset entire palette
- `dynamic_color(type)` - Get dynamic color
- `set_dynamic_color(type, rgb)` - Set dynamic color
- `reset_dynamic_color(type)` - Reset dynamic color

**FFI Functions:**
- `dterm_terminal_get_palette_color()` - Get palette color
- `dterm_terminal_set_palette_color()` - Set palette color
- `dterm_terminal_reset_palette_color()` - Reset palette color
- `dterm_terminal_reset_all_colors()` - Reset all colors
- `dterm_terminal_get_dynamic_color()` - Get dynamic color
- `dterm_terminal_set_dynamic_color()` - Set dynamic color
- `dterm_terminal_reset_dynamic_color()` - Reset dynamic color

**Example:**
```bash
# Query palette color 1 (red)
printf '\e]4;1;?\a'

# Set palette color 1 to bright green
printf '\e]4;1;rgb:00/ff/00\a'

# Set background to dark gray
printf '\e]11;#1a1a1a\a'

# Reset all colors
printf '\e]104\a'
```

### OSC 8 Hyperlinks (COMPLETE - 2025-12-28)
**Impact:** Clickable URLs in terminal output, file references, documentation links

OSC 8 hyperlinks allow applications to create clickable links in terminal output.
Format: `ESC ] 8 ; params ; uri ST text ESC ] 8 ; ; ST`

**Features Implemented:**
- Full OSC 8 parsing with params and URI support
- Optional `id=xxx` parameter for link grouping (hover highlighting)
- Hyperlink storage with memory-efficient cell references
- Garbage collection for unused hyperlinks
- BEL (0x07) and ST (ESC \) terminators supported

**API Functions:**
- `hyperlink_at(row, col)` - Get hyperlink at a cell position
- `has_active_hyperlink()` - Check if new characters will have a hyperlink
- `active_hyperlink()` - Get the currently active hyperlink
- `gc_hyperlinks()` - Clean up unused hyperlinks

**FFI Functions:**
- `dterm_terminal_get_hyperlink()` - Get hyperlink at position
- `dterm_hyperlink_free()` - Free hyperlink struct
- `dterm_terminal_has_active_hyperlink()` - Check for active hyperlink
- `dterm_terminal_gc_hyperlinks()` - Run garbage collection

**Example:**
```bash
# Create a hyperlink
printf '\e]8;;https://example.com\aClick here\e]8;;\a'
```

### OSC 52 Clipboard Access (COMPLETE - 2025-12-28)
**Impact:** Clipboard integration for vim, tmux, and remote session copy/paste

OSC 52 allows applications to set, query, and clear the system clipboard.
Format: `ESC ] 52 ; Pc ; Pd ST`
- `Pc`: Selection target(s) - 'c' (clipboard), 'p' (primary), 's' (select), '0'-'7' (cut buffers)
- `Pd`: Base64-encoded data, '?' to query, or empty to clear

**Features Implemented:**
- Full OSC 52 parsing with multiple selection targets
- Set clipboard with base64-encoded content
- Query clipboard (terminal responds with current content)
- Clear clipboard
- Callback-based architecture for host integration
- Support for all selection types (clipboard, primary, secondary, select, cut buffers 0-7)

**Rust API:**
- `set_clipboard_callback(callback)` - Register handler for clipboard operations
- `ClipboardOperation::Set { selections, content }` - Set clipboard content
- `ClipboardOperation::Query { selections }` - Query clipboard content
- `ClipboardOperation::Clear { selections }` - Clear clipboard

**FFI Functions:**
- `dterm_terminal_set_clipboard_callback()` - Register C callback for clipboard
- `dterm_clipboard_selection_mask()` - Convert selection to bitmask
- `DtermClipboardOp` - Struct with operation details
- `DtermClipboardSelection` - Enum for selection targets
- `DtermClipboardOpType` - Set/Query/Clear operation type

**Swift API (DTermCore.swift):**
- `ClipboardSelection` - Swift enum for selection targets
- `ClipboardOperation` - Operation with type, selections, and content
- `DTermClipboardHandler` protocol - Implement to handle clipboard operations
- `terminal.setClipboardHandler(_:)` - Set the clipboard handler

**Example:**
```bash
# Set clipboard to "Hello"
printf '\e]52;c;SGVsbG8=\a'

# Query clipboard
printf '\e]52;c;?\a'

# Clear clipboard
printf '\e]52;c;\a'

# Set both clipboard and primary selection
printf '\e]52;cp;SGVsbG8=\a'
```

**Example Swift Usage:**
```swift
class MyClipboardHandler: DTermClipboardHandler {
    func handleClipboard(_ operation: ClipboardOperation) -> String? {
        switch operation.type {
        case .set:
            if let content = operation.content {
                NSPasteboard.general.setString(content, forType: .string)
            }
            return nil
        case .query:
            return NSPasteboard.general.string(forType: .string)
        case .clear:
            NSPasteboard.general.clearContents()
            return nil
        }
    }
}

terminal.setClipboardHandler(MyClipboardHandler())
```

### Synchronized Updates Mode 2026 (COMPLETE - 2025-12-28)
**Impact:** Prevents flickering during complex screen updates in TUI applications

Mode 2026 allows applications to batch screen updates and have the terminal
render them atomically, preventing partial/flickering updates.

**How it works:**
1. Application sends `CSI ? 2026 h` to start buffering
2. Application performs multiple screen operations
3. Application sends `CSI ? 2026 l` to flush buffer and render

**API Functions:**
- `is_synchronized_update_active()` - Check if buffering is active
- `modes().synchronized_updates` - Mode flag in terminal modes

**FFI Functions:**
- `dterm_terminal_is_synchronized_update_active()` - Check sync mode

**Example:**
```bash
# Enable synchronized updates
printf '\e[?2026h'
# ... perform complex screen operations ...
# Disable (triggers render)
printf '\e[?2026l'
```

---

### Mouse Reporting (COMPLETE - 2025-12-28)
**Impact:** vim, htop, tmux, and most TUI applications now fully supported

| Mode | Code | Description | Status |
|------|------|-------------|--------|
| Normal tracking | 1000 | Basic mouse clicks | ✅ Complete |
| Button motion | 1002 | Track mouse during button press | ✅ Complete |
| Any motion | 1003 | Track all mouse movement | ✅ Complete |
| Focus reporting | 1004 | Window focus changes | ✅ Complete |
| SGR encoding | 1006 | Extended coordinate format | ✅ Complete |

**API Functions:**
- `encode_mouse_press()` - Encode button press events
- `encode_mouse_release()` - Encode button release events
- `encode_mouse_motion()` - Encode motion events (1002/1003 modes)
- `encode_mouse_wheel()` - Encode scroll wheel events
- `encode_focus_event()` - Encode focus in/out events

Both X10 (legacy) and SGR (extended) encoding formats are supported.

### DEC Line Drawing Characters (COMPLETE - Pre-existing)
**Impact:** Boxes, borders, tree views in TUI apps fully supported

Implemented features:
- G0/G1/G2/G3 character set designation (ESC ( 0, ESC ) 0, etc.)
- SO/SI switching between G0/G1
- SS2/SS3 single shift commands
- DEC Special Graphics mapping (0x61-0x7E → line drawing characters)

---

## ✅ Device Reports (MOSTLY COMPLETE - 2025-12-28)

**Impact:** Applications can now query terminal capabilities

### Device Status Reports (DSR)

| Sequence | Description | Status |
|----------|-------------|--------|
| CSI 5 n | Status report | ✅ Complete |
| CSI 6 n | Cursor position report | ✅ Complete |
| CSI ? 6 n | Extended cursor position | ✅ Complete |
| CSI ? 15 n | Printer status | ✅ Complete (no printer) |
| CSI ? 25 n | UDK status | ✅ Complete |
| CSI ? 26 n | Keyboard status | ✅ Complete |

### Device Attributes

| Sequence | Description | Status |
|----------|-------------|--------|
| DA1 (CSI c) | Primary device attributes | ✅ Complete (VT220) |
| DA2 (CSI > c) | Secondary device attributes | ✅ Complete (v2700) |
| DA3 (CSI = c) | Tertiary device attributes | Missing |

**API:**
- Response bytes are queued and can be retrieved via `take_response()` or `read_response()`
- DA2 reports version 2700 for vim underline_rgb and mouse_sgr compatibility
- DA1 reports VT220 with color, sixel (placeholder), and NRCS support

### Cursor Style DECSCUSR (COMPLETE)
✅ Already implemented and exposed via `cursor_style()` API.

CSI Ps SP q support:
- 0, 1: Blinking block ✓
- 2: Steady block ✓
- 3: Blinking underline ✓
- 4: Steady underline ✓
- 5: Blinking bar ✓
- 6: Steady bar ✓

---

## ✅ Shell Integration (COMPLETE - Pre-existing)

### OSC 7 - Working Directory (COMPLETE)
The terminal tracks working directory changes via OSC 7 sequences.

**Format:** `ESC ] 7 ; file://hostname/path BEL`

**API:**
- `cwd()` returns the current working directory as `PathBuf`
- `DirectoryChanged` event emitted on changes

### OSC 133 - Shell Integration (COMPLETE)
FinalTerm/iTerm2-style shell integration for command tracking.

| Sequence | Description | Status |
|----------|-------------|--------|
| OSC 133 ; A | Prompt start | ✅ Complete |
| OSC 133 ; B | Command start | ✅ Complete |
| OSC 133 ; C ; cmd | Command executed | ✅ Complete |
| OSC 133 ; D ; exit_code | Command finished | ✅ Complete |

**API:**
- `current_command()` returns the currently running command
- `command_history()` returns recent commands with exit codes
- Events: `CommandStarted`, `CommandFinished`, `PromptShown`

---

## Important Missing Features

### OSC Sequences (Priority: P1)

| OSC | Description | Status | Use Case |
|-----|-------------|--------|----------|
| 0-2 | Window title | ✅ Complete | Window management |
| 4 | Query/set palette | ✅ **Complete** | Color schemes |
| 7 | Set working directory | ✅ Complete | Shell integration |
| 8 | Hyperlinks | ✅ **Complete** | Clickable URLs |
| 10-19 | Dynamic colors | ✅ **Complete** | Theme changes |
| 52 | Clipboard access | ✅ **Complete** | Copy/paste |
| 104 | Reset palette colors | ✅ **Complete** | Theme reset |
| 110-119 | Reset dynamic colors | ✅ **Complete** | Theme reset |
| 133 | FinalTerm/shell integration | ✅ Complete | Command boundaries |

### Private Modes (Priority: P2)

| Mode | Description | Status |
|------|-------------|--------|
| 3 | 132-column mode (DECCOLM) | ✅ **Complete** |
| 5 | Reverse video (DECSCNM) | ✅ **Complete** |
| 12 | Cursor blink | ✅ **Complete** |
| 45 | Reverse wraparound | ✅ **Complete** |
| 66 | Keypad application mode (DECNKM) | ✅ **Complete** |
| 69 | Left-right margin mode | ✅ **Complete** |
| 2026 | Synchronized updates | ✅ **Complete** |

### ESC Sequences

| Sequence | Description | Status |
|----------|-------------|--------|
| ESC # 8 | DECALN screen alignment test | ✅ **Complete** |

### DCS Sequences (Priority: P2)

| Feature | Description | Status |
|---------|-------------|--------|
| DECRQSS | Request settings | ✅ **Complete** |
| Sixel | Graphics format | Missing |
| Synchronized updates | =1s/=2s | Missing |

---

## Feature Implementation Roadmap

### Phase 1: Basic TUI Support ✅ COMPLETE
1. ✅ Mouse reporting (modes 1000, 1002, 1003, 1006)
2. ✅ DEC line drawing characters (G0-G3, SI/SO, SS2/SS3)
3. ✅ Cursor position report (DSR 6)
4. ✅ DECSCUSR cursor style parsing
5. ✅ Focus reporting (mode 1004)
6. ✅ SGR mouse encoding (mode 1006)

### Phase 2: Shell Integration ✅ COMPLETE
1. ✅ OSC 7 (working directory)
2. ✅ OSC 133 (command boundaries)
3. ✅ Bracketed paste (2004)
4. ✅ Device attributes (DA1/DA2/DSR)

### Phase 3: Advanced Features (IN PROGRESS)
1. ✅ Hyperlinks (OSC 8) - COMPLETE
2. ✅ Synchronized updates (mode 2026) - COMPLETE
3. ✅ Color palette (OSC 4/10-19/104/110-119) - COMPLETE
4. ✅ Left-right margins (mode 69 / DECSLRM) - COMPLETE
5. Sixel graphics

### Phase 4: Full Parity
1. All device reports (DA3, etc.)
2. Rectangular area operations
3. Kitty keyboard protocol
4. iTerm2 proprietary extensions

---

## Testing Strategy

### Compatibility Test Suite

Use vttest (standard VT100 test suite):
```bash
# Install vttest
brew install vttest

# Run against terminal
vttest
```

### Application Compatibility Testing

| Application | Tests | Priority |
|-------------|-------|----------|
| vim/neovim | Mouse, cursor style, visual mode | P0 |
| htop | Mouse, line drawing | P0 |
| tmux | Mouse, alternate screen, line drawing | P0 |
| tig | Mouse, scrolling | P1 |
| lazygit | Mouse, colors | P1 |
| less | Mouse wheel, alternate screen | P1 |

### Escape Sequence Test Files

Create test data files with specific sequences:
```bash
# Test mouse reporting
printf '\e[?1000h'  # Enable
printf 'Click me'
printf '\e[?1000l'  # Disable

# Test line drawing
printf '\e(0'       # Enable
printf 'lqqk'       # Top-left corner, horizontal, top-right
printf '\e(B'       # Disable
```

---

## File References

### dterm-core
- Header: `DTermCore/include/dterm.h`
- Swift bindings: `sources/DTermCore.swift`
- Integration: `sources/DTermCoreIntegration.swift`

### iTerm2 Parser (Reference Implementation)
- Token definitions: `sources/VT100Token.h`
- CSI parser: `sources/VT100CSIParser.h`
- Terminal state: `sources/VT100Terminal.m`
- SGR handling: `sources/VT100GraphicRendition.h`

---

## Decision: Integration vs Replacement

### Option A: Parallel Processing (Current)
dterm-core runs in parallel with iTerm2's parser for comparison/validation.
- **Pro:** No feature parity required
- **Pro:** Can validate dterm-core output against iTerm2
- **Con:** Double parsing overhead
- **Con:** No performance benefit to user

### Option B: Gradual Replacement
Replace specific iTerm2 subsystems with dterm-core equivalents.
- **Pro:** Incremental improvement
- **Pro:** Can leverage dterm-core strengths (search, scrollback)
- **Con:** Complex integration
- **Con:** Risk of subtle incompatibilities

### Option C: Full Replacement (Long-term Goal)
Complete replacement of iTerm2's VT100 parser with dterm-core.
- **Pro:** Full performance benefit
- **Pro:** Memory safety
- **Con:** Requires full feature parity
- **Con:** ~6-12 months of feature work

### Recommendation

**Phase 2 (current):** Continue parallel processing for validation.
**Phase 3:** Replace scrollback/search with dterm-core.
**Phase 4:** Full replacement once feature parity achieved.

---

## Appendix: VT100Token.h Sequence Count

From iTerm2's VT100Token.h, the parser handles:
- 78 CSI sequences
- 24 DCS sequences
- 30+ OSC sequences
- 20+ ESC sequences
- 15+ private modes

Total: **~150 distinct escape sequences** vs dterm-core's **~30**.
