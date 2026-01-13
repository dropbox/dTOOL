# Kitty Terminal Emulator - Analysis Report

**Analyzed for dterm project**
**Date:** December 2024

---

## 1. Overview

### Basic Information

| Attribute | Value |
|-----------|-------|
| **Primary Author** | Kovid Goyal |
| **License** | GPL v3 |
| **Repository** | https://github.com/kovidgoyal/kitty |
| **Total LOC** | ~248,000 lines |
| **Languages** | C (~40%), Python (~30%), Go (~20%), GLSL (~5%), Objective-C (~5%) |
| **First Commit** | 2016 |
| **Active Development** | Yes (highly active) |

### Language Distribution

Kitty is a **hybrid architecture** terminal:

1. **C Core** - Performance-critical components (VT parser, screen buffer, rendering)
2. **Python Shell** - Configuration, window management, high-level logic
3. **Go Extensions** - Kittens (plugins) and some utilities
4. **GLSL Shaders** - GPU-accelerated rendering
5. **Objective-C** - macOS-specific features (Core Text, Cocoa integration)

### Key Files by Component

| Component | Primary Files |
|-----------|--------------|
| VT Parser | `kitty/vt-parser.c` (~1,700 lines) |
| Screen Buffer | `kitty/screen.c` (~250,000 bytes) |
| Graphics | `kitty/graphics.c` (~106,000 bytes) |
| Shaders | `kitty/shaders.c`, `kitty/*.glsl` |
| PTY | `kitty/child.c`, `kitty/child-monitor.c` |
| Font Rendering | `kitty/fonts.c`, `kitty/freetype.c`, `kitty/core_text.m` |
| Configuration | `kitty/config.py`, `kitty/options/` |

---

## 2. Architecture

### High-Level Structure

```
kitty/
├── kitty/           # C core and Python high-level logic
│   ├── *.c, *.h     # C terminal core
│   ├── *.py         # Python management layer
│   ├── *.glsl       # OpenGL shaders
│   ├── *.m          # Objective-C macOS code
│   ├── conf/        # Configuration parsing
│   ├── options/     # Options types and definitions
│   ├── rc/          # Remote control commands
│   ├── layout/      # Window layouts
│   └── fonts/       # Font configuration
├── kittens/         # Plugin system (Go + Python)
├── glfw/            # Bundled GLFW (modified)
├── tools/           # Go-based utilities
│   ├── cli/         # CLI utilities
│   ├── tui/         # TUI framework
│   ├── utils/       # Shared utilities
│   └── wcswidth/    # Unicode width calculation
└── 3rdparty/        # Third-party libraries
    ├── base64/      # SIMD-optimized base64
    └── ringbuf/     # Ring buffer implementation
```

### Key Abstractions

1. **Screen** (`kitty/screen.h`, `kitty/screen.c`)
   - Central state machine for terminal emulation
   - Contains line buffer, cursor, modes, selections
   - Manages both main and alternate screen buffers

2. **Line/LineBuf** (`kitty/line.c`, `kitty/line-buf.c`)
   - Efficient line storage with CPU and GPU cell types
   - Separate structures for rendering vs logical data

3. **Parser State (PS)** (`kitty/vt-parser.c`)
   - VTE state machine with UTF-8 decoder
   - Ring buffer for input processing
   - Lock-free communication with I/O thread

4. **GraphicsManager** (`kitty/graphics.c`)
   - Handles the Kitty graphics protocol
   - Manages textures, animations, z-ordering

### Threading Model

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│   Main Thread   │     │   I/O Thread    │     │   Talk Thread   │
│   (Python/GL)   │     │  (child-monitor)│     │ (remote control)│
├─────────────────┤     ├─────────────────┤     ├─────────────────┤
│ - Event loop    │     │ - PTY read/write│     │ - Unix socket   │
│ - Rendering     │     │ - Signal handler│     │ - Peer handling │
│ - Configuration │     │ - Child reaping │     │                 │
│ - Window mgmt   │     │ - Parse dispatch│     │                 │
└────────┬────────┘     └────────┬────────┘     └─────────────────┘
         │                       │
         └───────────────────────┘
              Mutex-protected
              screen buffers
```

**Key insight**: The parser runs in the I/O thread, parsing data directly into screen buffers with mutex protection. This keeps the main thread responsive.

---

## 3. Terminal Emulation

### VT Parser Implementation

**File**: `kitty/vt-parser.c`

The parser uses a **state machine** approach with the following states:

```c
typedef enum VTEState {
    VTE_NORMAL,      // Normal text processing
    VTE_ESC = ESC,   // After ESC character
    VTE_CSI,         // Control Sequence Introducer
    VTE_OSC,         // Operating System Command
    VTE_DCS,         // Device Control String
    VTE_APC,         // Application Program Command
    VTE_PM,          // Privacy Message
    VTE_SOS          // Start of String
} VTEState;
```

### Escape Sequence Handling

**Control Codes** (`kitty/control-codes.h`):

The parser handles standard ECMA-48/VT100+ sequences:

- **C0 Controls**: NUL, BEL, BS, HT, LF, VT, FF, CR, SO, SI, ESC, DEL
- **CSI Sequences**: Cursor movement, erase, insert, delete, SGR, modes
- **OSC Sequences**: Title, colors, clipboard, hyperlinks, shell integration
- **DCS Sequences**: DECRQSS, pending mode, Kitty DCS commands
- **APC Sequences**: Kitty graphics protocol

### CSI Parameter Parsing

```c
typedef struct ParsedCSI {
    char primary, secondary, trailer;
    CSIState state;
    unsigned num_params, num_digits;
    bool is_valid;
    uint64_t accumulator; int mult;
    int params[MAX_CSI_PARAMS];        // Up to 256 parameters
    uint8_t is_sub_param[MAX_CSI_PARAMS]; // Colon-separated sub-params
} ParsedCSI;
```

**Notable features**:
- Supports colon-separated sub-parameters (for SGR colors)
- Handles negative parameters
- Processes up to 256 parameters per sequence

### ECMA-48 Compliance

Kitty implements most ECMA-48 sequences plus extensions:

| Category | Support |
|----------|---------|
| Cursor Movement | Full (CUU, CUD, CUF, CUB, CUP, etc.) |
| Erase Functions | Full (ED, EL, ECH) |
| Insert/Delete | Full (ICH, DCH, IL, DL) |
| Scroll | Full (SU, SD) |
| SGR (Colors) | Full (256 color, TrueColor, sub-params) |
| Modes | Extensive (DEC private modes, Kitty modes) |
| OSC | Extended (52, 133, 8, graphics, etc.) |

---

## 4. Graphics Protocol

### Overview

The **Kitty Graphics Protocol** is a major innovation - enabling inline image display in terminals.

**File**: `kitty/graphics.c`, `kitty/parse-graphics-command.h`

### Protocol Design

Graphics commands use **APC** (Application Program Command) escape sequences:

```
ESC _ G <control-data> ; <payload> ESC \
```

Control data is key=value pairs:
- `a` - action (transmit, put, delete, etc.)
- `t` - transmission type (direct, file, temp file, shared memory)
- `f` - format (24-bit RGB, 32-bit RGBA, PNG)
- `i` - image ID
- `p` - placement ID
- `z` - z-index for layering
- `c`, `r` - columns, rows for placement
- `w`, `h` - width, height in pixels

### Data Structures

```c
typedef struct {
    unsigned char action, transmission_type, compressed, delete_action;
    uint32_t format, more, id, image_number, data_sz, data_offset, placement_id;
    uint32_t width, height, x_offset, y_offset;
    uint32_t cell_x_offset, cell_y_offset;
    uint32_t num_lines, num_cells;
    int32_t z_index;
    size_t payload_sz;
    bool unicode_placement;
} GraphicsCommand;

typedef struct {
    ImageRect src_rect, dest_rect;
    uint32_t texture_id, group_count;
    int z_index;
    id_type image_id, ref_id;
} ImageRenderData;
```

### Key Features

1. **Transmission Methods**:
   - Direct (inline base64)
   - File path
   - Temporary file
   - Shared memory (for local, zero-copy)

2. **Compression**: zlib compression support

3. **Placement**:
   - Cell-relative positioning
   - Pixel offsets within cells
   - Z-index layering (below/above text)

4. **Animation**: Frame-based animations with compositing

5. **Unicode Placeholders**: Reference images by Unicode PUA character

---

## 5. Rendering Pipeline

### OpenGL Architecture

Kitty uses **OpenGL 3.1+** (3.3 on macOS) with custom shaders.

**File**: `kitty/shaders.c`, `kitty/gl.c`

### Shader Programs

```c
enum {
    CELL_PROGRAM,             // Main cell rendering
    CELL_FG_PROGRAM,          // Foreground-only pass
    CELL_BG_PROGRAM,          // Background-only pass
    BORDERS_PROGRAM,          // Window borders
    GRAPHICS_PROGRAM,         // Image rendering
    GRAPHICS_PREMULT_PROGRAM, // Pre-multiplied alpha images
    GRAPHICS_ALPHA_MASK_PROGRAM,
    BGIMAGE_PROGRAM,          // Background image
    TINT_PROGRAM,             // Color tinting
    TRAIL_PROGRAM,            // Cursor trail effect
    BLIT_PROGRAM,             // Framebuffer blit
    ROUNDED_RECT_PROGRAM,     // Rounded rectangles
    NUM_PROGRAMS
};
```

### Cell Rendering Strategy

**Vertex Shader** (`kitty/cell_vertex.glsl`):

Each cell is rendered with:
1. **Instanced rendering** - One draw call for all visible cells
2. **GPU color resolution** - Color table lookups in shader
3. **Sprite-based glyphs** - Texture array for glyph atlas

```glsl
// Per-cell attributes (instanced)
layout(location=0) in uvec3 colors;    // fg, bg, decoration colors
layout(location=1) in uvec2 sprite_idx; // Glyph sprite indices
layout(location=2) in uint is_selected; // Selection flag
```

### Sprite Atlas Management

```c
typedef struct {
    int xnum, ynum, x, y, z, last_num_of_layers, last_ynum;
    GLuint texture_id;
    GLint max_texture_size, max_array_texture_layers;
    struct decorations_map {
        GLuint texture_id;
        unsigned width, height;
        size_t count;
    } decorations_map;
} SpriteMap;
```

- Uses **GL_TEXTURE_2D_ARRAY** for glyph sprites
- Separate texture for decorations (underlines, strikethrough)
- Dynamic reallocation when atlas fills

### Font Rendering

**Files**: `kitty/fonts.c`, `kitty/freetype.c`, `kitty/core_text.m`

- **FreeType** for glyph rasterization (Linux/cross-platform)
- **Core Text** for macOS native font rendering
- **HarfBuzz** for text shaping (ligatures, complex scripts)
- **Fontconfig** for font discovery on Linux

---

## 6. PTY Integration

### Process Spawning

**File**: `kitty/child.c`

```c
static PyObject*
spawn(PyObject *self, PyObject *args) {
    // Fork process
    pid_t pid = fork();

    switch(pid) {
        case 0: { // Child
            // Reset signal handlers
            // Change to working directory
            // Create new session (setsid)
            // Establish controlling terminal (TIOCSCTTY)
            // Redirect stdin/stdout/stderr to PTY
            // Wait for ready signal from parent
            // Close extra file descriptors
            // Execute shell/command
        }
    }
}
```

### I/O Monitoring

**File**: `kitty/child-monitor.c`

The `ChildMonitor` manages all child processes:

```c
typedef struct {
    PyObject_HEAD
    PyObject *dump_callback, *update_screen, *death_notify;
    unsigned int count;
    bool shutting_down;
    pthread_t io_thread, talk_thread;
    int talk_fd, listen_fd;
    LoopData io_loop_data;
    void (*parse_func)(void*, ParseData*, bool);
} ChildMonitor;
```

Key responsibilities:
1. **Polling PTY file descriptors** using `poll()`
2. **Reading PTY output** into parser buffers
3. **Signal handling** (SIGCHLD, SIGINT, SIGHUP, etc.)
4. **Process reaping** and death notification

### Platform Support

| Platform | PTY Implementation |
|----------|-------------------|
| Linux | `openpty()`, `/dev/ptmx` |
| macOS | `openpty()` via BSD APIs |
| BSD | `openpty()` |

**Note**: Kitty does **not** support Windows natively.

---

## 7. Configuration System

### Configuration Loading

**File**: `kitty/config.py`

```python
def load_config(*paths, overrides=None, accumulate_bad_lines=None):
    # Parse configuration files
    # Merge with defaults
    # Finalize keybindings
    # Finalize mouse mappings
    return Options(opts_dict)
```

Configuration is:
1. **Declarative** - Key-value pairs
2. **Hierarchical** - Include other config files
3. **Hot-reloadable** - Signal triggers reload

### Options Types

**File**: `kitty/options/types.py`

The `Options` class is auto-generated from definitions, containing:
- Display settings (font, colors, cursor)
- Behavior settings (scrollback, copy-on-select)
- Keybindings and mouse mappings
- Platform-specific options

### Kittens (Plugin System)

**Directory**: `kittens/`

Kittens are mini-programs that extend Kitty:

| Kitten | Purpose |
|--------|---------|
| `icat` | Display images |
| `diff` | Side-by-side diff viewer |
| `hints` | URL/path hints for quick selection |
| `unicode_input` | Unicode character picker |
| `themes` | Theme browser and switcher |
| `ssh` | Enhanced SSH with shell integration |
| `transfer` | File transfer over terminal |
| `clipboard` | Clipboard management |

**Architecture**:
- Written in Python or Go
- Can have TUI interfaces using `kittens/tui/`
- Communicate via escape sequences or remote control

**Runner** (`kittens/runner.py`):
```python
def import_kitten_main_module(config_dir, kitten):
    if kitten.endswith('.py'):
        # Custom kitten from file
        exec(compile(src, path, 'exec'), g)
    else:
        # Built-in kitten
        m = importlib.import_module(f'kittens.{kitten}.main')
    return {'start': m.main, 'end': m.handle_result}
```

---

## 8. Performance Optimizations

### SIMD String Processing

**Files**: `kitty/simd-string.c`, `kitty/simd-string-impl.h`

Kitty uses SIMD for:
- UTF-8 decoding
- Escape sequence detection
- Byte searching

```c
// Supports multiple SIMD levels
#if KITTY_SIMD_LEVEL == 128
    // SSE/NEON 128-bit
#elif KITTY_SIMD_LEVEL == 256
    // AVX2 256-bit
#endif

// Key functions
bool utf8_decode_to_esc(UTF8Decoder *d, const uint8_t *src, size_t src_sz);
const uint8_t* find_either_of_two_bytes(const uint8_t *haystack, size_t sz, uint8_t a, uint8_t b);
```

Uses **SIMDe** library for portable SIMD across x86 and ARM.

### Input Coalescing

```c
// In vt-parser.c
if (flush || pd->time_since_new_input >= OPT(input_delay) ||
    self->read.sz + 16 * 1024 > BUF_SZ) {
    // Process accumulated input
}
```

Input is coalesced to reduce render frequency during rapid output.

### GPU Optimization

1. **Instanced rendering** - Single draw call for entire grid
2. **Texture arrays** - Efficient glyph atlas
3. **Uniform buffer objects** - Batched uniform updates
4. **Dirty tracking** - Only update changed cells

### Memory Efficiency

1. **Compact cell representation**:
```c
typedef struct GPUCell {
    sprite_index sprite_idx[2];  // Glyph + combining
    uint32_t fg, bg, decoration; // Colors
} GPUCell; // 20 bytes per cell
```

2. **Ring buffer for parser input** - No allocations during parsing

3. **Disk cache for graphics** - Large images cached to disk

---

## 9. Keyboard Protocol

### Kitty Keyboard Protocol

**File**: `kitty/key_encoding.c`

Kitty implements an **enhanced keyboard protocol** that provides:
- Disambiguated key events
- Key release events
- Modifier state reporting
- Alternate key reporting

### Protocol Modes

```c
// Flags for keyboard encoding
bool cursor_key_mode;          // DECCKM
bool disambiguate;             // Report all keys uniquely
bool report_all_event_types;   // Include release events
bool report_alternate_key;     // Report shifted/alternate versions
bool report_text;              // Include generated text
bool embed_text;               // Embed text in escape sequence
```

### Key Encoding

Modern (enhanced) format:
```
CSI <key-code> ; <modifiers> ; <event-type> u
```

Example encodings:
```c
// Function keys with modifiers
case GLFW_FKEY_F1: S(1, 'P');   // ESC [ 1 P
case GLFW_FKEY_HOME: S(1, 'H'); // ESC [ 1 H

// With modifiers: ESC [ 1 ; 5 H (Ctrl+Home)
```

### Legacy Compatibility

The encoder falls back to legacy sequences when enhanced mode is disabled:
- Standard VT100 cursor keys
- xterm function key encoding
- Traditional modifier handling

---

## 10. Strengths

### Technical Excellence

1. **GPU Rendering**: The OpenGL-based renderer is exceptionally fast and efficient. Single draw calls for the entire cell grid minimize CPU-GPU communication.

2. **Graphics Protocol**: A well-designed, widely-adopted protocol for inline images. Now supported by multiple terminals (WezTerm, Konsole, iTerm2, etc.).

3. **Keyboard Protocol**: The enhanced keyboard protocol solves long-standing terminal keyboard ambiguity issues.

4. **Performance**: SIMD-optimized parsing, input coalescing, and efficient memory layout make it one of the fastest terminals.

5. **Feature Completeness**: Comprehensive VT emulation plus modern extensions (OSC 52, OSC 8, OSC 133).

### Architecture

1. **Clean Separation**: C for performance, Python for flexibility, Go for tooling.

2. **Extensible Plugin System**: Kittens provide powerful extensions without core bloat.

3. **Well-Documented Protocols**: Graphics and keyboard protocols have excellent documentation.

### Developer Experience

1. **Active Development**: Consistent updates with new features.

2. **Rich Ecosystem**: Many tools and libraries support Kitty protocols.

3. **Shell Integration**: OSC 133 prompt marking, working directory tracking.

---

## 11. Weaknesses/Limitations

### Platform Support

1. **No Windows Support**: Kitty explicitly does not support Windows, limiting its cross-platform appeal.

2. **No Mobile Support**: No iOS/iPadOS/Android versions.

### Architecture Issues

1. **GPL License**: The GPL v3 license prevents use in proprietary projects and complicates embedding.

2. **Python Dependency**: Requires Python runtime, adding ~30MB to distribution size.

3. **OpenGL Requirement**: No software rendering fallback for systems without GPU.

4. **Complex Build**: Building requires multiple toolchains (C, Python, Go, Rust for some tools).

### Technical Limitations

1. **No ConPTY**: Cannot use Windows pseudo-terminals.

2. **Bundled GLFW**: Ships modified GLFW, making updates complex.

3. **Limited Font Fallback**: Complex fallback logic can be unpredictable.

4. **Scrollback Memory**: Large scrollback consumes significant memory.

### Missing Features

1. **No Sixel Support**: Deliberately omitted in favor of Kitty protocol.

2. **No tmux Integration**: Graphics protocol doesn't pass through multiplexers.

3. **No Session Persistence**: No built-in session save/restore.

---

## 12. Lessons for dterm

### Adopt

1. **Graphics Protocol Design**
   - The Kitty graphics protocol is well-designed and widely adopted
   - Consider implementing it for compatibility with existing tools
   - Key insight: Use APC escapes (safe for non-supporting terminals)

2. **Enhanced Keyboard Protocol**
   - Essential for modern applications (neovim, etc.)
   - Well-documented specification to implement

3. **SIMD Parser Optimization**
   - Use SIMD for UTF-8 decoding and escape detection
   - SIMDe library provides good portability

4. **GPU Rendering Architecture**
   - Instanced rendering with texture arrays is highly efficient
   - Uniform buffer objects for batched state updates
   - Consider using compute shaders for even more flexibility

5. **Cell Data Structure**
   ```rust
   // Rust equivalent of Kitty's approach
   struct GPUCell {
       sprite_indices: [u32; 2],  // Glyph + combining
       fg: u32,
       bg: u32,
       decoration: u32,
   }
   ```

6. **Input Coalescing**
   - Don't render every byte of input
   - Coalesce for ~2-5ms before rendering
   - Essential for fast output (e.g., `cat large_file`)

### Avoid

1. **Python/Interpreted Language Core**
   - Adds distribution complexity
   - Memory overhead
   - dterm's pure Rust approach is better

2. **GPL License**
   - Limits adoption in proprietary contexts
   - Apache 2.0 (dterm's choice) is more permissive

3. **Platform-Specific Font APIs**
   - Kitty's Core Text vs FreeType split adds complexity
   - Consider a unified approach with optional native fallback

4. **Bundling Modified Dependencies**
   - Kitty bundles modified GLFW
   - Prefer standard dependencies or pure Rust alternatives

### Improve Upon

1. **Cross-Platform Support**
   - Implement ConPTY for Windows from day one
   - Mobile platforms via cross-compilation

2. **wgpu Instead of OpenGL**
   - More modern, better Vulkan/Metal/DX12 support
   - No legacy OpenGL concerns

3. **Formal Verification**
   - Kitty has no formal proofs
   - dterm's TLA+/Kani approach is stronger

4. **Configuration**
   - Consider a more structured format (TOML)
   - Live reload without signals

5. **Memory-Mapped Scrollback**
   - Kitty keeps scrollback in memory
   - Consider disk-backed scrollback for efficiency

### Protocol Compatibility

For tool ecosystem compatibility, consider implementing:

1. **Kitty Graphics Protocol** - Wide adoption
2. **Kitty Keyboard Protocol** - Needed by modern editors
3. **OSC 52** (Clipboard) - Standard
4. **OSC 133** (Shell integration) - Widely used
5. **OSC 8** (Hyperlinks) - Standard

---

## Appendix: Key File Reference

| Purpose | File | Lines |
|---------|------|-------|
| VT Parser | `kitty/vt-parser.c` | ~1,700 |
| Screen Buffer | `kitty/screen.c` | ~6,000 |
| Graphics Protocol | `kitty/graphics.c` | ~3,000 |
| Shader Management | `kitty/shaders.c` | ~2,000 |
| Cell Vertex Shader | `kitty/cell_vertex.glsl` | ~400 |
| PTY Child | `kitty/child.c` | ~200 |
| Child Monitor | `kitty/child-monitor.c` | ~2,000 |
| Key Encoding | `kitty/key_encoding.c` | ~700 |
| Configuration | `kitty/config.py` | ~200 |
| SIMD Strings | `kitty/simd-string-impl.h` | ~1,000 |
| Data Types | `kitty/data-types.h` | ~400 |
| Control Codes | `kitty/control-codes.h` | ~240 |

---

*Analysis prepared for the dterm project by examining the Kitty terminal emulator source code.*
