# Contour Terminal Emulator - Analysis Report

**Analyzed for:** dterm research
**Date:** 2025-12-27
**Source:** https://github.com/contour-terminal/contour

---

## 1. Overview

### Basic Information

| Property | Value |
|----------|-------|
| **Language** | C++20 |
| **License** | Apache 2.0 |
| **Lines of Code** | ~79,500 (C++/H files) |
| **Primary Author** | Christian Parpart |
| **First Release** | ~2018 |
| **Latest Version** | v0.5.x series |

### Project Summary

Contour is a modern, GPU-accelerated terminal emulator written in C++20. It emphasizes:
- VT520 compliance with extensive standards support
- Cross-platform operation (Linux, macOS, Windows, FreeBSD, OpenBSD)
- GPU-accelerated rendering via OpenGL 3.3+
- Modern features: ligatures, Sixel graphics, hyperlinks, Vi-mode

### Platforms Supported

- Linux (primary)
- macOS
- Windows 10+ (via ConPTY)
- FreeBSD / OpenBSD

---

## 2. Architecture

### High-Level Structure

```
src/
├── contour/        # Application layer (Qt-based GUI)
├── crispy/         # Utility library (LRU cache, CLI, logging, etc.)
├── text_shaper/    # Font shaping abstraction layer
├── vtbackend/      # Terminal emulation core (Screen, Grid, Terminal)
├── vtparser/       # VT sequence parser (state machine)
├── vtpty/          # PTY abstraction (Unix, ConPTY, SSH)
└── vtrasterizer/   # GPU rendering (texture atlas, text rendering)
```

### Module Organization

#### `vtparser/` - VT Sequence Parser
- **Files:** `Parser.h`, `Parser-impl.h`, `ParserEvents.h`, `ParserExtension.h`
- **Purpose:** State machine-based VT sequence parser
- **Design:** Template-based with compile-time table generation

#### `vtbackend/` - Terminal Backend
- **Files:** ~86 files including `Terminal.cpp` (88KB), `Screen.cpp` (150KB), `Grid.cpp` (43KB)
- **Purpose:** Full terminal emulation engine
- **Key Classes:** `Terminal`, `Screen<Cell>`, `Grid<Cell>`, `Line<Cell>`

#### `vtrasterizer/` - GPU Rendering
- **Files:** `Renderer.h`, `TextRenderer.cpp`, `TextureAtlas.h`, `BoxDrawingRenderer.cpp`
- **Purpose:** OpenGL-based rendering pipeline
- **Design:** Texture atlas with LRU caching

#### `vtpty/` - PTY Abstraction
- **Files:** `Pty.h`, `UnixPty.cpp`, `ConPty.cpp`, `SshSession.cpp`
- **Purpose:** Platform-agnostic PTY interface
- **Platforms:** Unix (openpty), Windows (ConPTY), SSH

#### `crispy/` - Utility Library
- **Files:** 60+ utility files
- **Notable:** `StrongLRUHashtable.h` (custom LRU), `ring.h` (ring buffer), `BufferObject.h`

#### `text_shaper/` - Font Shaping
- **Files:** `open_shaper.cpp` (HarfBuzz), `directwrite_shaper.cpp`, `coretext_locator.mm`
- **Purpose:** Platform-specific font location and shaping

---

## 3. Terminal Emulation - VT Parser Implementation

### Parser Design (`src/vtparser/`)

The parser is a **table-driven state machine** inspired by the DEC ANSI parser documented at vt100.net/emu/dec_ansi_parser.

#### State Machine States

```cpp
enum class State : uint8_t {
    Ground,              // Normal text processing
    Escape,              // After ESC received
    EscapeIntermediate,  // Processing ESC intermediates
    CSI_Entry,           // Control Sequence Introducer entry
    CSI_Param,           // CSI parameter collection
    CSI_Intermediate,    // CSI intermediate collection
    CSI_Ignore,          // Malformed CSI, ignoring
    DCS_Entry,           // Device Control String entry
    DCS_Param,           // DCS parameter collection
    DCS_Intermediate,    // DCS intermediate collection
    DCS_PassThrough,     // DCS data passthrough
    DCS_Ignore,          // Malformed DCS
    OSC_String,          // Operating System Command
    APC_String,          // Application Program Command
    PM_String,           // Private Message
    IgnoreUntilST,       // Start of String (SOS)
};
```

#### Key Design Patterns

1. **Compile-time Table Generation**
   ```cpp
   constexpr ParserTable ParserTable::get() {
       auto t = ParserTable{};
       // Entry/exit actions
       t.entry(State::Ground, Action::GroundStart);
       t.exit(State::Ground, Action::PrintEnd);
       // Transitions
       t.transition(State::Escape, State::CSI_Entry, 0x5B);
       // ...
       return t;
   }
   ```

2. **Event Listener Pattern via Concepts**
   ```cpp
   template <typename T>
   concept ParserEventsConcept = requires(T& handler, ...) {
       { handler.print(char32_t{}) } -> std::same_as<void>;
       { handler.execute(char{}) } -> std::same_as<void>;
       { handler.dispatchCSI(char{}) } -> std::same_as<void>;
       // ...
   };
   ```

3. **Bulk Text Optimization**
   The parser includes a fast path for bulk ASCII text processing:
   ```cpp
   auto Parser::parseBulkText(char const* begin, char const* end) noexcept
       -> std::tuple<ProcessKind, size_t>
   {
       if (_state != State::Ground) return {ProcessKind::FallbackToFSM, 0};
       // Unicode scan with grapheme cluster awareness
       auto const [cellCount, subStart, subEnd] = unicode::scan_text(...);
       _eventListener.print(text, cellCount);
       return {ProcessKind::ContinueBulk, count};
   }
   ```

---

## 4. Rendering Pipeline

### Architecture (`src/vtrasterizer/`)

The rendering system uses a **texture atlas** approach with specialized renderers:

```cpp
class Renderer {
    BackgroundRenderer _backgroundRenderer;
    ImageRenderer _imageRenderer;
    TextRenderer _textRenderer;
    DecorationRenderer _decorationRenderer;
    CursorRenderer _cursorRenderer;
};
```

### Texture Atlas System

```cpp
struct AtlasProperties {
    Format format {};              // Red, RGB, RGBA
    ImageSize tileSize {};         // Tile dimensions
    crispy::strong_hashtable_size hashCount {};  // LRU slots
    crispy::lru_capacity tileCount {};           // Total tiles
    uint32_t directMappingCount {};              // Fast-path tiles (A-Za-z0-9)
};
```

Key features:
- **Direct Mapping:** Common ASCII glyphs get fixed atlas positions (no LRU lookup)
- **LRU Eviction:** Less common glyphs use LRU cache
- **Batch Rendering:** Tiles are batched for GPU submission

### Text Rendering (`TextRenderer.cpp`)

- Uses HarfBuzz for complex text shaping
- Supports ligatures (Fira Code, etc.)
- Grapheme cluster-aware rendering
- Text cluster grouping for efficiency (`TextClusterGrouper.cpp`)

### Box Drawing (`BoxDrawingRenderer.cpp`)

A notable feature: **109KB** dedicated to box drawing character rendering.
- Programmatic generation of box drawing glyphs
- Pixel-perfect alignment at any cell size
- Support for full Unicode box drawing range

---

## 5. PTY Integration

### Abstraction Layer (`src/vtpty/Pty.h`)

```cpp
class Pty {
public:
    virtual void start() = 0;
    virtual void close() = 0;
    virtual bool isClosed() const noexcept = 0;
    virtual std::optional<ReadResult> read(
        crispy::buffer_object<char>& storage,
        std::optional<std::chrono::milliseconds> timeout,
        size_t size) = 0;
    virtual int write(std::string_view buf) = 0;
    virtual PageSize pageSize() const noexcept = 0;
    virtual void resizeScreen(PageSize cells, std::optional<ImageSize> pixels) = 0;
};
```

### Platform Implementations

#### Unix (`UnixPty.cpp`)
- Uses `openpty()` / `forkpty()`
- Platform-specific includes (`<util.h>` macOS, `<pty.h>` Linux)
- `read_selector` for async I/O
- Stdout "fast pipe" optimization

#### Windows (`ConPty.cpp`)
- ConPTY API (Windows 10 1809+)
- External ConPTY DLL support for Windows 10 mouse input workaround

#### SSH (`SshSession.cpp`)
- 53KB implementation for remote terminal sessions
- Full SSH terminal support

---

## 6. Configuration System

### YAML-Based Configuration (`src/contour/Config.h`)

Configuration is loaded from `~/.config/contour/contour.yml` using yaml-cpp.

#### Key Configuration Structures

```cpp
struct CursorConfig {
    CursorShape cursorShape { CursorShape::Block };
    CursorDisplay cursorDisplay { CursorDisplay::Steady };
    std::chrono::milliseconds cursorBlinkInterval;
};

struct HistoryConfig {
    MaxHistoryLineCount maxHistoryLineCount { LineCount(1000) };
    LineCount historyScrollMultiplier { LineCount(3) };
    bool autoScrollOnUpdate { true };
};

struct StatusLineConfig {
    StatusDisplayType initialType { StatusDisplayType::Indicator };
    StatusDisplayPosition position { StatusDisplayPosition::Bottom };
    IndicatorConfig indicator;
};
```

#### Notable Features
- Runtime configuration reload
- Profile system (multiple configs per terminal)
- Reflection-based serialization (`reflection-cpp`)
- Input mapping configuration

---

## 7. VT Standards Compliance

### VT Type Support (`src/vtbackend/VTType.h`)

```cpp
enum class VTType : uint8_t {
    VT100 = 0,
    VT220 = 1,
    VT240 = 2,
    VT330 = 18,
    VT340 = 19,
    VT320 = 24,
    VT420 = 41,
    VT510 = 61,
    VT520 = 64,
    VT525 = 65,
};
```

### Function Definitions (`src/vtbackend/Functions.h`)

Comprehensive function registry with **~130+ VT sequences** defined:

#### C0 Controls
```cpp
constexpr auto BEL = detail::C0('\x07', "BEL", "Bell");
constexpr auto BS  = detail::C0('\x08', "BS", "Backspace");
constexpr auto TAB = detail::C0('\x09', "TAB", "Tab");
constexpr auto LF  = detail::C0('\x0A', "LF", "Line Feed");
// ...
```

#### CSI Sequences
```cpp
constexpr auto CUP = detail::CSI(nullopt, 0, 2, nullopt, 'H', VTType::VT100, ...);
constexpr auto SGR = detail::CSI(nullopt, 0, ArgsMax, nullopt, 'm', VTType::VT100, ...);
constexpr auto DECSTBM = detail::CSI(nullopt, 0, 2, nullopt, 'r', VTType::VT100, ...);
// VT420+ sequences
constexpr auto DECCARA = detail::CSI(nullopt, 5, ArgsMax, '$', 'r', VTType::VT420, ...);
constexpr auto DECCRA = detail::CSI(nullopt, 0, 8, '$', 'v', VTType::VT420, ...);
// VT520 sequences
constexpr auto DECPS = detail::CSI(nullopt, 3, 18, ',', '~', VTType::VT520, ...);
```

#### DCS Sequences
```cpp
constexpr auto DECSIXEL = detail::DCS(nullopt, 0, 3, nullopt, 'q', VTType::VT330, ...);
constexpr auto DECRQSS = detail::DCS(nullopt, 0, 0, '$', 'q', VTType::VT420, ...);
```

#### OSC Sequences
```cpp
constexpr auto HYPERLINK = detail::OSC(8, VTExtension::Unknown, ...);
constexpr auto CLIPBOARD = detail::OSC(52, VTExtension::XTerm, ...);
constexpr auto COLORFG = detail::OSC(10, VTExtension::XTerm, ...);
```

### Control Code Definitions (`src/vtbackend/ControlCode.h`)

Complete C0, C1 (7-bit and 8-bit) control code enumerations with documentation.

### Dynamic Sequence Enabling/Disabling

```cpp
class SupportedSequences {
    void reset(VTType vt) noexcept;           // Enable sequences up to VT level
    void disableSequence(Function seq) noexcept;
    void enableSequence(Function seq) noexcept;
};
```

---

## 8. Performance Optimizations

### Bulk Text Processing

The parser bypasses the state machine for continuous printable text:
```cpp
// In Parser-impl.h
auto const chunk = std::string_view(input, distance(input, end));
auto const [cellCount, subStart, subEnd] = unicode::scan_text(_scanState, chunk, maxCharCount);
_eventListener.print(text, cellCount);
```

### Synchronized Output (DEC Mode 2026)

```cpp
enum class DECMode : uint16_t {
    BatchedRendering = 2026,  // Synchronized output
    // ...
};
```

When enabled, rendering is deferred until mode is disabled, reducing flicker.

### LRU Cache with Strong Hashing

Custom LRU implementation (`StrongLRUHashtable.h`) with:
- FNV-1a hashing
- Power-of-two sizing
- Direct mapping fast path

### Ring Buffer for Scrollback

```cpp
template <CellConcept Cell>
using Lines = crispy::ring<Line<Cell>>;
```

Efficient scrollback with O(1) line addition/removal.

### Benchmarking Infrastructure

Dedicated benchmark tool (`bench-headless.cpp`) using `termbench-pro`:
- Parser-only benchmarks
- Grid operation benchmarks
- PTY throughput benchmarks

---

## 9. Strengths

### 1. Exceptional Standards Compliance
- Most comprehensive VT520 implementation among modern terminals
- Explicit conformance level tracking per sequence
- Full ECMA-48 and DEC private sequence support

### 2. Clean Modular Architecture
- Clear separation: parser / backend / pty / rasterizer
- Each module has single responsibility
- Easy to understand component boundaries

### 3. Type Safety
- Extensive use of `boxed-cpp` for type-safe primitives
- Strong typing for coordinates, sizes, colors
- Concept-based template constraints

### 4. Performance-Conscious Design
- Bulk text fast path in parser
- Texture atlas with direct mapping
- Synchronized output support
- Comprehensive benchmark suite

### 5. Cross-Platform PTY Abstraction
- Clean trait-based PTY interface
- ConPTY support with workarounds
- SSH session support built-in

### 6. Extensive Documentation
- Inline documentation for VT sequences
- Design documents in `docs/drafts/`
- References to specifications

---

## 10. Weaknesses/Limitations

### 1. Complexity
- 80K+ LOC is substantial for a terminal emulator
- Some files are very large (`Screen.cpp` at 150KB)
- Learning curve for contributors

### 2. C++ Specific Challenges
- Build complexity (vcpkg, Qt, many dependencies)
- Template-heavy code increases compile times
- Memory safety relies on careful coding

### 3. Qt Dependency
- GUI layer tightly coupled to Qt
- Limits embedding in other frameworks
- Significant dependency footprint

### 4. Limited Mobile Support
- No iOS/Android implementations
- Desktop-focused architecture

### 5. Single Maintainer Risk
- Primarily single-author project
- Knowledge concentration

---

## 11. Lessons for dterm

### Patterns Worth Adopting

#### 1. State Machine Parser Design
The compile-time table generation pattern is excellent:
```rust
// Rust equivalent
const fn build_parser_table() -> ParserTable {
    let mut table = ParserTable::new();
    table.transition(State::Escape, State::CsiEntry, b'[');
    // ...
    table
}
static PARSER_TABLE: ParserTable = build_parser_table();
```

#### 2. VT Function Registry
The declarative function definition approach:
```rust
// Define sequences with metadata
const CUP: VtFunction = VtFunction {
    category: Category::CSI,
    final_byte: b'H',
    min_params: 0,
    max_params: 2,
    conformance: VTType::VT100,
    documentation: "Cursor Position",
};
```

#### 3. Bulk Text Fast Path
Critical for performance:
```rust
fn parse_bulk_text(&mut self, input: &[u8]) -> Option<usize> {
    if self.state != State::Ground { return None; }
    // Scan for printable ASCII run
    let count = input.iter()
        .take_while(|&&b| b >= 0x20 && b <= 0x7E)
        .count();
    if count > 0 {
        self.handler.print(&input[..count]);
        Some(count)
    } else {
        None
    }
}
```

#### 4. PTY Trait Design
Clean abstraction:
```rust
pub trait Pty: Send {
    fn read(&mut self, buf: &mut [u8], timeout: Option<Duration>) -> io::Result<usize>;
    fn write(&mut self, data: &[u8]) -> io::Result<usize>;
    fn resize(&mut self, cols: u16, rows: u16) -> io::Result<()>;
    fn close(&mut self);
}
```

#### 5. Conformance Level Tracking
Enable/disable sequences based on terminal type:
```rust
pub struct SequenceRegistry {
    active: Vec<VtFunction>,
    conformance_level: VTType,
}

impl SequenceRegistry {
    pub fn set_conformance(&mut self, level: VTType) {
        self.active.retain(|f| f.conformance <= level);
    }
}
```

### Patterns to Avoid or Improve

#### 1. File Size
Break up large files early. `Screen.cpp` at 150KB is hard to navigate.

#### 2. Template Complexity
Rust traits are cleaner than C++ concepts for this use case.

#### 3. Build Complexity
dterm should aim for minimal dependencies and simple builds.

#### 4. GUI Coupling
Keep rendering abstraction separate from any specific GUI framework.

### Specific Code to Study

1. **Parser State Machine:** `src/vtparser/Parser-impl.h` lines 125-382
2. **Function Definitions:** `src/vtbackend/Functions.h` (entire file)
3. **Control Codes:** `src/vtbackend/ControlCode.h` (complete C0/C1)
4. **PTY Abstraction:** `src/vtpty/Pty.h` and `UnixPty.cpp`
5. **Texture Atlas:** `src/vtrasterizer/TextureAtlas.h`
6. **LRU Cache:** `src/crispy/StrongLRUHashtable.h`

### Key Insights for dterm

1. **Standards First:** Contour's explicit VT level conformance tracking should be adopted. Every sequence should know which VT level it belongs to.

2. **Parser Performance:** The bulk text fast path provides ~50x performance improvement according to their benchmarks. This is essential.

3. **Synchronized Output:** DEC mode 2026 is critical for modern TUI applications. Must implement.

4. **Box Drawing:** Programmatic box drawing generation (109KB in Contour) ensures pixel-perfect rendering at any size.

5. **Cell Concept:** The template-based cell abstraction allows different cell representations. dterm could use a similar trait-based approach in Rust.

6. **Testing:** The extensive test suite (`Screen_test.cpp` at 144KB) demonstrates the value of comprehensive VT sequence testing.

---

## References

- [Contour GitHub](https://github.com/contour-terminal/contour)
- [VT100.net Parser](https://vt100.net/emu/dec_ansi_parser)
- [VT510 Manual](https://vt100.net/docs/vt510-rm/)
- [ECMA-48](http://www.ecma-international.org/publications-and-standards/standards/ecma-48)
- [XTerm Control Sequences](https://invisible-island.net/xterm/ctlseqs/ctlseqs.html)
