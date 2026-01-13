# Feature Union: dterm Must Have ALL of These

**Goal:** dterm = Union of ALL features from ALL competitors

**Last Updated:** 2025-12-29

---

## CORE FEATURES (dterm-core Library)

These features MUST be in dterm-core. No exceptions.

### VT Escape Sequences - COMPLETE SET

#### C0 Controls (ALL REQUIRED)
| Code | Name | Status |
|------|------|--------|
| NUL | Null | ✅ |
| BEL | Bell | ✅ |
| BS | Backspace | ✅ |
| HT | Horizontal Tab | ✅ |
| LF | Line Feed | ✅ |
| VT | Vertical Tab | ✅ |
| FF | Form Feed | ✅ |
| CR | Carriage Return | ✅ |
| SO | Shift Out | ✅ |
| SI | Shift In | ✅ |
| ESC | Escape | ✅ |
| DEL | Delete | ✅ |

#### C1 Controls (ALL REQUIRED)
| Code | Name | Status |
|------|------|--------|
| IND | Index | ✅ |
| NEL | Next Line | ✅ |
| HTS | Horizontal Tab Set | ✅ |
| RI | Reverse Index | ✅ |
| SS2/SS3 | Single Shift | ✅ |
| DCS | Device Control String | ✅ |
| CSI | Control Sequence Introducer | ✅ |
| OSC | Operating System Command | ✅ |
| PM | Privacy Message | ✅ |
| APC | Application Program Command | ✅ |
| SOS | Start of String | ✅ |
| ST | String Terminator | ✅ |

#### CSI Sequences - Cursor (ALL REQUIRED)
| Seq | Name | Status |
|-----|------|--------|
| CUU | Cursor Up | ✅ |
| CUD | Cursor Down | ✅ |
| CUF | Cursor Forward | ✅ |
| CUB | Cursor Backward | ✅ |
| CUP | Cursor Position | ✅ |
| HVP | Horizontal Vertical Position | ✅ |
| CNL | Cursor Next Line | ✅ |
| CPL | Cursor Previous Line | ✅ |
| CHA | Cursor Character Absolute | ✅ |
| VPA | Vertical Position Absolute | ✅ |
| VPR | Vertical Position Relative | ✅ |
| HPA | Horizontal Position Absolute | ✅ |
| HPR | Horizontal Position Relative | ✅ |

#### CSI Sequences - Editing (ALL REQUIRED)
| Seq | Name | Status |
|-----|------|--------|
| ED | Erase in Display | ✅ |
| EL | Erase in Line | ✅ |
| ECH | Erase Character | ✅ |
| DCH | Delete Character | ✅ |
| ICH | Insert Character | ✅ |
| IL | Insert Line | ✅ |
| DL | Delete Line | ✅ |
| DECERA | Erase Rectangular Area | ❓ CHECK |
| DECFRA | Fill Rectangular Area | ❓ CHECK |
| DECCARA | Change Attrs Rectangular | ❓ CHECK |
| DECCRA | Copy Rectangular Area | ❓ CHECK |
| DECSERA | Selective Erase Rect | ❓ CHECK |

#### CSI Sequences - Scrolling (ALL REQUIRED)
| Seq | Name | Status |
|-----|------|--------|
| SU | Scroll Up | ✅ |
| SD | Scroll Down | ✅ |
| DECSTBM | Set Top/Bottom Margins | ✅ |
| DECSLRM | Set Left/Right Margins | ❓ CHECK |

#### SGR - ALL Attributes (ALL REQUIRED)
| Code | Name | Status |
|------|------|--------|
| 0 | Reset | ✅ |
| 1 | Bold | ✅ |
| 2 | Dim/Faint | ✅ |
| 3 | Italic | ✅ |
| 4 | Underline | ✅ |
| 4:0 | No Underline | ✅ |
| 4:1 | Single Underline | ✅ |
| 4:2 | Double Underline | ✅ |
| 4:3 | Curly Underline | ❓ CHECK |
| 4:4 | Dotted Underline | ❓ CHECK |
| 4:5 | Dashed Underline | ❓ CHECK |
| 5 | Slow Blink | ✅ |
| 6 | Rapid Blink | ✅ |
| 7 | Reverse Video | ✅ |
| 8 | Conceal/Hidden | ✅ |
| 9 | Strikethrough | ✅ |
| 21 | Double Underline | ✅ |
| 53 | Overline | ✅ |
| 58 | Underline Color | ❓ CHECK |
| 30-37 | FG Colors | ✅ |
| 40-47 | BG Colors | ✅ |
| 90-97 | Bright FG | ✅ |
| 100-107 | Bright BG | ✅ |
| 38;5;n | 256 FG Color | ✅ |
| 48;5;n | 256 BG Color | ✅ |
| 38;2;r;g;b | RGB FG | ✅ |
| 48;2;r;g;b | RGB BG | ✅ |

#### DEC Private Modes (ALL REQUIRED)
| Mode | Name | Status |
|------|------|--------|
| 1 | DECCKM (Cursor Keys) | ✅ |
| 2 | DECANM (VT52 Mode) | ✅ |
| 3 | DECCOLM (132 Columns) | ✅ |
| 5 | DECSCNM (Reverse Video) | ✅ |
| 6 | DECOM (Origin Mode) | ✅ |
| 7 | DECAWM (Auto Wrap) | ✅ |
| 8 | DECARM (Auto Repeat) | ✅ |
| 25 | DECTCEM (Cursor Visible) | ✅ |
| 69 | DECLRMM (Left/Right Margin) | ❓ CHECK |
| 1000 | Mouse Normal Tracking | ✅ |
| 1002 | Mouse Button Event | ✅ |
| 1003 | Mouse Any Event | ✅ |
| 1004 | Focus Reporting | ✅ |
| 1005 | UTF-8 Mouse | ✅ |
| 1006 | SGR Mouse | ✅ |
| 1015 | URXVT Mouse | ✅ |
| 1016 | SGR Pixel Mouse | ❓ CHECK |
| 1047/1048/1049 | Alt Screen | ✅ |
| 2004 | Bracketed Paste | ✅ |
| 2026 | Synchronized Output | ✅ |

#### OSC Sequences (ALL REQUIRED)
| OSC | Name | Status |
|-----|------|--------|
| 0 | Window Title + Icon | ✅ |
| 1 | Icon Name | ✅ |
| 2 | Window Title | ✅ |
| 4 | Color Palette | ✅ |
| 7 | Current Working Directory | ✅ |
| 8 | Hyperlinks | ✅ |
| 10 | Foreground Color | ✅ |
| 11 | Background Color | ✅ |
| 12 | Cursor Color | ✅ |
| 52 | Clipboard | ✅ |
| 104 | Reset Color | ✅ |
| 110-117 | Reset Various Colors | ✅ |
| 133 | Shell Integration | ✅ |
| 1337 | iTerm2 Extensions | ✅ |

---

### Graphics Protocols (ALL 3 REQUIRED)

| Protocol | Status |
|----------|--------|
| Sixel Graphics | ✅ |
| Kitty Graphics Protocol | ✅ |
| iTerm2 Image Protocol (OSC 1337) | ✅ |
| Kitty Graphics - Direct | ✅ |
| Kitty Graphics - File Path | ❓ CHECK |
| Kitty Graphics - Temp File | ❓ CHECK |
| Kitty Graphics - Shared Memory | ❓ CHECK |
| Kitty Graphics - zlib Compression | ❓ CHECK |
| Kitty Graphics - Animations | ❓ CHECK |
| Kitty Graphics - Unicode Placeholders | ❓ CHECK |
| Kitty Graphics - Z-index | ❓ CHECK |
| DRCS (Soft Fonts) | ✅ |

---

### Keyboard Protocol (ALL REQUIRED)

| Feature | Status |
|---------|--------|
| Kitty Keyboard Protocol | ✅ |
| Disambiguate Escape Codes | ✅ |
| Report Event Types | ✅ |
| Report Alternate Keys | ✅ |
| Report All Keys as Escapes | ✅ |
| Report Associated Text | ✅ |
| Key Release Events | ❓ CHECK |
| Bracketed Paste | ✅ |
| Application Cursor Keys | ✅ |
| Application Keypad | ✅ |

---

### Unicode/Text (ALL REQUIRED)

| Feature | Status |
|---------|--------|
| UTF-8 Full Support | ✅ |
| Unicode 15+ | ✅ |
| Grapheme Clusters (UAX #29) | ✅ |
| Combining Characters | ✅ |
| Wide Characters (CJK) | ✅ |
| Emoji (including ZWJ) | ✅ |
| BiDi Text (Arabic/Hebrew) | ✅ |
| Right-to-Left | ✅ |
| Character Sets (G0-G3) | ✅ |
| Box Drawing Characters | ✅ |
| Variation Selectors | ❓ CHECK |

---

### Shell Integration (ALL REQUIRED)

| Feature | Status |
|---------|--------|
| OSC 7 (CWD) | ✅ |
| OSC 133 Prompt Marking | ✅ |
| OSC 133 A (Prompt Start) | ✅ |
| OSC 133 B (Prompt End) | ✅ |
| OSC 133 C (Command Start) | ✅ |
| OSC 133 D (Command End) | ✅ |
| Exit Status Tracking | ❓ CHECK |
| Command Duration | ❓ CHECK |
| **Block-Based Output** | ❌ GAP |

---

### Search Features (ALL REQUIRED)

| Feature | Status |
|---------|--------|
| Text Search | ✅ |
| Regex Search | ✅ |
| Incremental Search | ✅ |
| Search Highlighting | ✅ |
| Trigram Index (O(1) for 1M lines) | ❓ CHECK |

---

### Selection Features (ALL REQUIRED)

| Feature | Status |
|---------|--------|
| Mouse Selection | UI LAYER |
| Keyboard Selection (Vi Mode) | ✅ |
| Word Selection | ✅ |
| Line Selection | ✅ |
| Rectangular/Block Selection | ❓ CHECK |
| **Smart Selection (URL, Path, Email)** | ❌ GAP |

---

### Session Features (ALL REQUIRED)

| Feature | Status |
|---------|--------|
| Session State Persistence | ✅ |
| Crash Recovery/Checkpoints | ✅ |
| Tiered Scrollback (Hot/Warm/Cold) | ✅ |
| Session Serialization | ✅ |

---

### Conformance Levels (ALL REQUIRED)

| Level | Status |
|-------|--------|
| VT52 Mode | ✅ |
| VT100 | ✅ |
| VT220 | ✅ |
| VT320 | ❓ CHECK |
| VT420 | ❓ CHECK |
| VT510/520 | ❓ CHECK |
| xterm Extensions | ✅ |

---

## NON-CORE FEATURES (UI Layer or Bridge)

These are NOT in dterm-core but provided by UI layer or bridge.

### Rendering (UI Layer)
- GPU Rendering (OpenGL/Metal/Vulkan/wgpu)
- Glyph Atlas/Texture Cache
- Font Shaping (HarfBuzz)
- Ligatures
- Custom Shaders
- Background Images
- Window Transparency
- Cursor Rendering

### Input (UI Layer)
- IME Support
- Touch Input
- Key Bindings Configuration

### Window Management (UI Layer)
- Tabs
- Splits/Panes
- Window Decorations
- High DPI Support

### Platform (Bridge Layer)
- PTY Spawning (openpty/ConPTY)
- SSH Integration
- WSL Integration
- Process Management

### Configuration (UI Layer)
- Config File Format (TOML/JSON/etc.)
- Hot Reload
- Profiles
- Themes

### AI Features (Separate Module)
- Natural Language Commands (Warp)
- Error Explanation
- Suggestions

---

## GAP SUMMARY: Core Features Missing

| Feature | Source | Priority |
|---------|--------|----------|
| Block-Based Output | Warp | HIGH |
| Smart Selection | iTerm2 | HIGH |
| Rectangular Selection | Kitty, WezTerm | MEDIUM |
| Kitty Graphics - Full Feature Set | Kitty | MEDIUM |
| VT420/VT520 Full Compliance | Contour | MEDIUM |
| Curly/Dotted/Dashed Underline | Kitty | LOW |
| Underline Color (SGR 58) | Kitty | LOW |
| SGR Pixel Mouse (1016) | Kitty | LOW |
| Key Release Events | Kitty | LOW |
| Variation Selectors | Unicode | LOW |

---

## VERIFICATION CHECKLIST

Run these to verify feature completeness:

```bash
# Check SGR support
grep -r "SGR\|select_graphic" crates/dterm-core/src/

# Check mouse modes
grep -r "1016\|pixel.*mouse" crates/dterm-core/src/

# Check Kitty graphics features
grep -r "kitty.*graphics\|APC" crates/dterm-core/src/

# Check rectangular operations
grep -r "DECERA\|DECFRA\|DECCRA\|rectangular" crates/dterm-core/src/

# Check VT levels
grep -r "VT52\|VT320\|VT420\|VT520" crates/dterm-core/src/
```

---

## TARGET: Feature Superset

When complete, dterm-core will have:
- **100%** of Contour's VT compliance
- **100%** of Kitty's keyboard protocol
- **100%** of WezTerm's graphics protocols
- **100%** of Warp's shell integration
- **100%** of Ghostty's performance features
- **PLUS** dterm's unique: formal verification, crash recovery, tiered storage
