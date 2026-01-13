# Ghostty Terminal Emulator Analysis

**Analysis Date:** December 2024
**Version Analyzed:** tip (commit 38664af)
**Analyst:** dterm research team

---

## 1. Overview

### Language and Technology Stack

- **Primary Language:** Zig (approximately 222,000 lines across 602 `.zig` files)
- **Secondary Languages:**
  - Swift (~27,000 lines across 128 files for macOS/iOS native UI)
  - C/C++ (for SIMD optimizations in `src/simd/`)
  - Blueprint/GTK (for Linux UI)

### License

- **MIT License** (Copyright 2024 Mitchell Hashimoto, Ghostty contributors)
- This is a permissive open-source license allowing commercial use, modification, and distribution.

### Lines of Code (Approximate)

| Component | LOC |
|-----------|-----|
| Zig Core | ~222,000 |
| Swift (macOS/iOS) | ~27,000 |
| Total | ~250,000+ |

Key file sizes:
- `Terminal.zig`: 11,677 lines (state machine, terminal operations)
- `Screen.zig`: 9,035 lines (screen buffer management)
- `PageList.zig`: 10,852 lines (scrollback/page management)
- `Config.zig`: 10,255 lines (configuration parsing)
- `Surface.zig`: 6,686 lines (main terminal surface abstraction)
- `stream.zig`: 3,449 lines (VT stream processing)

### Contributors

The git history in this checkout shows **Mitchell Hashimoto** as the primary author. The project was developed privately before being open-sourced in late 2024.

### Release History

Ghostty was publicly released in December 2024 after approximately 2+ years of private development. The project is under active development with frequent updates.

---

## 2. Architecture

### High-Level Structure

Ghostty follows a layered architecture with clear separation between:

```
┌─────────────────────────────────────────────────────────────────┐
│                    PLATFORM UI (Native)                          │
│           macOS/SwiftUI • Linux/GTK • iOS/SwiftUI               │
└─────────────────────────────────────────────────────────────────┘
                              │
                              │ C FFI / Embedded API
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                      APPRT (App Runtime)                         │
│        Abstracts platform-specific windowing and input          │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                         SURFACE                                  │
│     Main terminal widget - owns PTY, renderer, terminal         │
└─────────────────────────────────────────────────────────────────┘
                              │
              ┌───────────────┼───────────────┐
              ▼               ▼               ▼
┌──────────────────┐ ┌───────────────┐ ┌────────────────┐
│    TERMINAL      │ │   RENDERER    │ │    TERMIO      │
│ State machine,   │ │ Metal/OpenGL  │ │ PTY I/O, exec  │
│ parser, screen   │ │ GPU rendering │ │ shell spawn    │
└──────────────────┘ └───────────────┘ └────────────────┘
```

### Module Organization

Key directories in `src/`:

| Directory | Purpose |
|-----------|---------|
| `terminal/` | Core terminal emulation (parser, screen, pages, styles) |
| `renderer/` | GPU rendering (Metal, OpenGL abstractions) |
| `termio/` | Terminal I/O, PTY management, process execution |
| `apprt/` | Application runtime abstraction (GTK, embedded, none) |
| `font/` | Font loading, shaping, atlas management |
| `config/` | Configuration parsing and management |
| `simd/` | SIMD-optimized routines (UTF-8, base64, VT parsing) |
| `os/` | Platform-specific OS utilities |
| `input/` | Input handling and key binding |
| `inspector/` | Terminal inspector/debugger |

### Key Abstractions

1. **Surface** (`src/Surface.zig`): The main abstraction representing a terminal instance. Owns:
   - Terminal state machine
   - Renderer
   - PTY/IO thread
   - Font metrics
   - Input state

2. **App** (`src/App.zig`): Application-level state managing multiple surfaces, config, and the main event loop.

3. **Terminal** (`src/terminal/Terminal.zig`): Core terminal emulation with:
   - Screen set (primary/alternate)
   - Scrolling regions
   - Mode flags
   - Color state

---

## 3. Terminal Emulation

### VT Parser Implementation

**Location:** `src/terminal/Parser.zig`

The VT parser is a **state machine implementation** based directly on the [vt100.net DEC ANSI parser](https://vt100.net/emu/dec_ansi_parser):

```zig
pub const State = enum {
    ground,
    escape,
    escape_intermediate,
    csi_entry,
    csi_intermediate,
    csi_param,
    csi_ignore,
    dcs_entry,
    dcs_param,
    dcs_intermediate,
    dcs_passthrough,
    dcs_ignore,
    osc_string,
    sos_pm_apc_string,
};
```

Key design decisions:

- **Lookup table driven:** Uses `parse_table.zig` for state transitions
- **Zero allocation during parsing:** All state is stack-allocated
- **Maximum 24 CSI parameters:** Chosen empirically based on real-world usage (SGR commands)
- **Maximum 4 intermediate characters:** Doubles as UTF-8 decode buffer

### Action Types

The parser produces a union type of actions:

```zig
pub const Action = union(enum) {
    print: u21,           // Unicode codepoint to draw
    execute: u8,          // C0/C1 control function
    csi_dispatch: CSI,    // CSI sequence
    esc_dispatch: ESC,    // Escape sequence
    osc_dispatch: osc.Command,  // OSC command
    dcs_hook: DCS,        // DCS start
    dcs_put: u8,          // DCS data
    dcs_unhook: void,     // DCS end
    apc_start: void,      // APC start
    apc_put: u8,          // APC data
    apc_end: void,        // APC end
};
```

### Escape Sequence Handling

**Location:** `src/terminal/stream.zig`

The stream handler translates parser actions into terminal operations. Key implementation patterns:

1. **SIMD-accelerated UTF-8 decoding:** For printable text runs
2. **Separate handlers for each CSI/OSC type**
3. **Mode-aware processing:** Different behavior based on terminal modes

Supported escape sequences include:
- Full ECMA-48/ANSI compliance
- DEC private modes (DECSET/DECRST)
- xterm extensions (OSC sequences)
- Kitty keyboard protocol
- Kitty graphics protocol (via `src/terminal/kitty/`)
- OSC 8 hyperlinks
- OSC 133 shell integration

---

## 4. Rendering Pipeline

### Multi-Renderer Architecture

**Location:** `src/renderer/`

Ghostty implements a **generic renderer pattern** using Zig's comptime generics:

```zig
pub fn Renderer(comptime GraphicsAPI: type) type {
    return struct {
        // Common rendering logic
        api: GraphicsAPI,
        // ...
    };
}
```

### Supported Backends

| Backend | File | Platform |
|---------|------|----------|
| Metal | `Metal.zig` | macOS, iOS |
| OpenGL | `OpenGL.zig` | Linux |
| WebGL | `WebGL.zig` | WASM (stub) |

### Metal Renderer Details

**Location:** `src/renderer/Metal.zig`, `src/renderer/metal/`

Key features:
- **Direct Metal usage** (not MoltenVK or other abstraction)
- **Triple buffering** (`swap_chain_count = 3`)
- **IOSurface-based layer** for efficient display
- **Unified/managed memory detection** for discrete vs integrated GPUs
- **AutoreleasePool management** per frame

```zig
pub const custom_shader_target: shadertoy.Target = .msl;
pub const custom_shader_y_is_down = true;  // Metal coordinate system
pub const swap_chain_count = 3;
```

### Generic Renderer

**Location:** `src/renderer/generic.zig`

The generic renderer provides:
- Cell-based rendering abstraction
- Font shaping integration
- Image/Kitty graphics support
- Background image handling
- Custom shader (shadertoy) support
- Search match highlighting

### Font Rendering

**Location:** `src/font/`

Font handling is sophisticated:

| Component | File | Purpose |
|-----------|------|---------|
| Collection | `Collection.zig` | Font family/style management |
| SharedGrid | `SharedGrid.zig` | Glyph caching per-config |
| Atlas | `Atlas.zig` | Texture atlas for GPU |
| Shaper | `shape.zig` | Text shaping (likely HarfBuzz) |
| Discovery | `discovery.zig` | System font enumeration |

Backend options:
```zig
pub const Backend = enum {
    freetype,      // Cross-platform
    coretext,      // macOS
    web_canvas,    // WASM
};
```

---

## 5. PTY Integration

### Cross-Platform PTY Abstraction

**Location:** `src/pty.zig`

The PTY implementation uses compile-time selection:

```zig
pub const Pty = switch (builtin.os.tag) {
    .windows => WindowsPty,
    .ios => NullPty,
    else => PosixPty,
};
```

### POSIX Implementation

Uses `openpty()` from libc with:
- UTF-8 mode enabled (`IUTF8` flag)
- CLOEXEC on master fd
- Proper signal reset in child
- setsid() for process group
- TIOCSCTTY for controlling terminal

### Windows Implementation

Uses **ConPTY** (Windows Pseudo Console):
- Named pipes for I/O (for overlapped/async support)
- `CreatePseudoConsole` API
- Process-wide counter for unique pipe names

### Termio Layer

**Location:** `src/termio/`

The termio layer provides:
- **Thread-based I/O** for responsive terminal under load
- **Backend abstraction** (`backend.zig`)
- **Mailbox system** for thread communication (`mailbox.zig`)
- **Shell integration** (`shell_integration.zig`) for OSC 133

---

## 6. Configuration System

### Configuration Architecture

**Location:** `src/config/`

The main config struct (`Config.zig`) is **extremely comprehensive** at ~10,000 lines. It uses:

- **CLI-compatible field names** with hyphen syntax (`@"font-family"`)
- **Pandoc-flavored markdown** for documentation comments
- **Repeatable options** for multi-value configs
- **Conditional configuration** based on system state (light/dark mode)

### Key Configuration Features

```zig
// Font configuration
@"font-family": RepeatableString = .{},
@"font-size": f32 = switch (builtin.os.tag) {
    .macos => 13,
    else => 12,
},

// Font variations for variable fonts
@"font-variation": RepeatableFontVariation = .{},

// Codepoint-to-font mapping
@"font-codepoint-map": RepeatableCodepointMap = .{},
```

### Configuration Loading

**Location:** `src/config/file_load.zig`

- Multiple config file locations (XDG on Linux, ~/Library on macOS)
- Hot reload support
- Theme loading (`theme.zig`)
- URL handling for remote configs (`url.zig`)

### Compatibility Layer

The config includes **compatibility handlers** for deprecated options:
```zig
pub const compatibility = std.StaticStringMap(
    cli.CompatibilityHandler(Config),
).initComptime(&.{
    .{ "background-blur-radius", cli.compatibilityRenamed(Config, "background-blur") },
    .{ "adw-toolbar-style", cli.compatibilityRenamed(Config, "gtk-toolbar-style") },
    // ... more migrations
});
```

---

## 7. Platform Abstraction

### Application Runtime (apprt)

**Location:** `src/apprt.zig`, `src/apprt/`

The apprt provides platform abstraction for:
- Window creation and management
- Input handling
- Clipboard access
- Color scheme detection
- IPC between instances

### Runtime Selection

```zig
pub const runtime = switch (build_config.artifact) {
    .exe => switch (build_config.app_runtime) {
        .none => none,
        .gtk => gtk,
    },
    .lib => embedded,       // libghostty
    .wasm_module => browser,
};
```

### Platform-Specific Code

| Platform | Directory | Technology |
|----------|-----------|------------|
| macOS | `macos/` | SwiftUI + libghostty |
| Linux | `src/apprt/gtk/` | GTK4 + libadwaita |
| iOS | `macos/Sources/App/iOS/` | SwiftUI |
| Embedded | `src/apprt/embedded.zig` | C API |

### libghostty

The `embedded.zig` module provides a **C-compatible API** for embedding:
- Used by the macOS Swift app
- ~71,000 lines of embedded integration code
- Exposes terminal as a widget/view

---

## 8. Performance Optimizations

### SIMD Optimizations

**Location:** `src/simd/`

SIMD is used for hot paths:

1. **VT parsing** (`vt.cpp`, `vt.zig`): UTF-8 decode until control sequence
2. **Base64** (`base64.cpp`): For OSC 52 clipboard, Kitty graphics
3. **Index searching** (`index_of.cpp`): Fast byte searching
4. **Codepoint width** (`codepoint_width.cpp`): Unicode width calculation

Example SIMD function:
```zig
pub fn utf8DecodeUntilControlSeq(
    input: []const u8,
    output: []u32,
) DecodeResult {
    if (comptime options.simd) {
        // Use C++ SIMD implementation
        return ghostty_simd_decode_utf8_until_control_seq(...);
    }
    // Scalar fallback
    return utf8DecodeUntilControlSeqScalar(input, output);
}
```

### Fast Memory Operations

**Location:** `src/fastmem.zig`

Custom memory operations preferring libc when available:
```zig
pub inline fn move(comptime T: type, dest: []T, source: []const T) void {
    if (builtin.link_libc) {
        _ = memmove(dest.ptr, source.ptr, source.len * @sizeOf(T));
    } else {
        @memmove(dest, source);
    }
}
```

Also includes optimized rotation operations for scrolling.

### Page-Based Memory Architecture

**Location:** `src/terminal/page.zig`, `src/terminal/PageList.zig`

The terminal uses a **page-based architecture**:

- Pages are **contiguous, page-aligned memory blocks**
- Easy to serialize/copy entire pages
- Memory pooling with preheating
- Bitmap allocators for graphemes and strings

```zig
pub const Page = struct {
    memory: []align(std.heap.page_size_min) u8,
    rows: Offset(Row),
    cells: Offset(Cell),
    // ...
};
```

### Threaded Architecture

- **Dedicated IO thread** for PTY read/write
- **Separate renderer thread** with mailbox communication
- **Search thread** for async search operations

### Inline Assert Optimization

**Location:** `src/quirks.zig`

Custom assert that's guaranteed to inline in release builds:
```zig
pub const inlineAssert = switch (builtin.mode) {
    .Debug => std.debug.assert,
    .ReleaseSmall, .ReleaseSafe, .ReleaseFast => (struct {
        inline fn assert(ok: bool) void {
            if (!ok) unreachable;
        }
    }).assert,
};
```

This addresses a 15-20% performance hit from non-inlined stdlib asserts in hot loops.

---

## 9. Memory Safety Approach

### Zig Safety Features

Ghostty leverages Zig's built-in safety:

1. **Compile-time bounds checking** on slices
2. **No null pointers** - optional types are explicit
3. **No implicit type coercion**
4. **Guaranteed initialization**

### Explicit Error Handling

All fallible operations use Zig's error unions:
```zig
pub fn open(size: winsize) OpenError!Pty {
    // ...
    if (c.openpty(&master_fd, &slave_fd, null, null, @ptrCast(&sizeCopy)) < 0)
        return error.OpenptyFailed;
    // ...
}
```

### Memory Pool Strategy

Rather than individual allocations, Ghostty uses **memory pools**:

```zig
pub const MemoryPool = struct {
    alloc: Allocator,
    nodes: NodePool,
    pages: PagePool,
    pins: PinPool,

    pub fn init(gen_alloc: Allocator, page_alloc: Allocator, preheat: usize) !MemoryPool {
        // Preheat pools to avoid allocation during operation
    }
};
```

### Offset-Based Pointers

For serialization safety, internal references use offsets:
```zig
const Offset = size.Offset;
rows: Offset(Row),
cells: Offset(Cell),
```

### Testing

The project uses:
- **Unit tests** throughout (run with `zig build test`)
- **Valgrind** for memory leak detection on Linux
- **Address sanitizers** available

---

## 10. Strengths

### Exceptional Qualities

1. **Performance**:
   - SIMD-optimized parsing
   - Direct Metal/OpenGL (no abstraction layers)
   - Dedicated IO thread for low-latency
   - Page-based memory for cache efficiency

2. **Standards Compliance**:
   - Comprehensive xterm audit ([#632](https://github.com/ghostty-org/ghostty/issues/632))
   - Full ECMA-48 support
   - Kitty keyboard and graphics protocols

3. **Native Platform Experience**:
   - macOS: Full SwiftUI app with Metal renderer
   - Linux: GTK4 + libadwaita integration
   - Not a lowest-common-denominator cross-platform app

4. **Code Quality**:
   - Clear module separation
   - Extensive inline documentation
   - Type-safe configuration system
   - Comptime generics for zero-cost abstractions

5. **Modern Terminal Features**:
   - Kitty graphics protocol
   - OSC 8 hyperlinks
   - Shell integration (OSC 133)
   - Custom shaders (shadertoy)

6. **libghostty**:
   - Embeddable terminal library
   - C API for other languages
   - Proven by macOS app using it

### Architectural Wins

- **Parser based on vt100.net reference** - well-documented, correct
- **Generic renderer pattern** - easy to add new backends
- **Configuration system** - extremely comprehensive yet type-safe
- **Memory pooling** - predictable allocation patterns

---

## 11. Weaknesses/Limitations

### Current Limitations

1. **Windows Support**:
   - Listed as "not yet" on roadmap
   - ConPTY code exists but no app runtime

2. **Single Primary Developer**:
   - Most commits from Mitchell Hashimoto
   - Knowledge concentration risk

3. **Zig Language Maturity**:
   - Zig is pre-1.0 (currently ~0.13)
   - Build system changes between versions
   - Smaller ecosystem than Rust/C++

4. **iOS Limitations**:
   - NullPty implementation (no real PTY)
   - Remote-only terminal use case

5. **Documentation**:
   - Code is well-commented but architectural docs are sparse
   - No formal specification documents

### Technical Debt

1. **Large Files**: Some files exceed 10,000 lines (Terminal.zig, Config.zig)
2. **C/C++ SIMD**: Non-Zig code for SIMD could be pure Zig
3. **Platform Quirks**: `quirks.zig` indicates accumulated workarounds

### Missing Features

Per the README roadmap:
- Windows terminals (PowerShell, Cmd, WSL)
- "Fancy features" (unspecified)
- Complete native settings UI on all platforms

---

## 12. Lessons for dterm

### Patterns Worth Adopting

1. **Page-Based Memory Architecture**
   - Self-contained, serializable pages
   - Memory pooling with preheating
   - Offset-based internal references
   - Excellent for remote sync use case

2. **Parser State Machine**
   - Table-driven vt100.net-based parser
   - Zero allocation during parsing
   - Clear action types

3. **Generic Renderer Pattern**
   - Comptime generics for backend abstraction
   - Common logic in generic, specific in backends
   - Easy to add wgpu backend

4. **Platform Abstraction (apprt)**
   - Clear separation between core and platform
   - Traits-like interface pattern
   - C API for embedding (libghostty model)

5. **Configuration System**
   - Type-safe, CLI-compatible
   - Conditional configuration support
   - Hot reload capability
   - Compatibility migration layer

6. **Threaded I/O Architecture**
   - Dedicated IO thread
   - Mailbox-based communication
   - Separate render thread

### SIMD Considerations

For dterm:
- Consider Rust SIMD crates (e.g., `packed_simd`, `wide`)
- Key targets: UTF-8 decode, escape sequence scanning
- Fallback scalar implementations essential

### What to Do Differently

1. **Use Rust** (per dterm's design):
   - More mature ecosystem
   - Better tooling (cargo, docs)
   - Formal verification tools (Kani)

2. **TLA+ First**:
   - Ghostty lacks formal specifications
   - dterm should spec state machines before implementing

3. **Smaller Files**:
   - Break up large modules earlier
   - Terminal.zig pattern is hard to navigate

4. **Document Architecture**:
   - Create architecture docs from start
   - Don't rely solely on code comments

### Specific Code Worth Studying

| File | Reason |
|------|--------|
| `src/terminal/Parser.zig` | State machine implementation |
| `src/terminal/page.zig` | Memory-efficient page design |
| `src/pty.zig` | Cross-platform PTY abstraction |
| `src/renderer/generic.zig` | Generic renderer pattern |
| `src/simd/vt.zig` | SIMD optimization patterns |
| `src/fastmem.zig` | Memory operation optimizations |
| `src/config/Config.zig` | Comprehensive config system |

---

## Summary

Ghostty is a **well-engineered, high-performance terminal emulator** that demonstrates excellent architectural decisions, particularly around:

- State machine-based VT parsing
- Page-based memory management
- Native platform integration via libghostty
- Multi-threaded I/O for responsiveness

For dterm, the most valuable takeaways are:
1. The page-based memory architecture for efficient sync
2. The parser design based on the vt100.net reference
3. The platform abstraction pattern for native experiences
4. The threaded I/O model for low-latency operation

Ghostty proves that a terminal can be simultaneously fast, feature-rich, and native. Its main limitations (Windows support, single developer) are organizational rather than architectural.

---

*Analysis prepared for the dterm project. See CLAUDE.md for dterm's design goals.*
