# Windows Terminal Source Code Analysis

**Analysis Date:** 2025-12-27
**Repository:** https://github.com/microsoft/terminal
**Analyzed For:** dterm project - Windows platform support research

---

## 1. Overview

### Basic Information

| Attribute | Value |
|-----------|-------|
| **Primary Language** | C++ (Modern C++17/20) |
| **License** | MIT License |
| **Lines of Code** | ~300,000 (C/C++ in src/) |
| **Copyright** | Microsoft Corporation |

### What This Repository Contains

1. **Windows Terminal** - The modern terminal application
2. **Windows Terminal Preview** - Preview/canary builds
3. **Windows Console Host** (`conhost.exe`) - The classic Windows console
4. **ConPTY** (Pseudoconsole) - The pseudo-terminal API for Windows
5. **Shared Components** - VT parser, text buffer, renderers
6. **Sample Projects** - ConPTY usage examples
7. **ColorTool** - Console color scheme utility

### Key Technologies

| Component | Technology |
|-----------|------------|
| Core Language | C++17/20 with WinRT |
| Build System | MSBuild, Visual Studio 2022 |
| GPU Rendering | Direct3D 11.2, DirectWrite, Direct2D |
| UI Framework | XAML Islands (UWP in Win32) |
| JSON Parsing | jsoncpp |
| Error Handling | WIL (Windows Implementation Library) |
| Testing | TAEF (Test Authoring and Execution Framework) |
| Fuzzing | LibFuzzer, OneFuzz |

---

## 2. Architecture

### High-Level Structure

```
Windows Terminal Architecture
=============================

+------------------------------------------------------------------+
|                    Windows Terminal (WindowsTerminal.exe)         |
|                         XAML Islands + Win32                      |
+------------------------------------------------------------------+
                                |
                    WinRT/C++/CX Interface
                                |
+------------------------------------------------------------------+
|                    TerminalApp (DLL)                              |
|         Tabs, Panes, Settings UI, Command Palette                 |
+------------------------------------------------------------------+
                                |
+------------------------------------------------------------------+
|                    TerminalControl (DLL)                          |
|              UWP XAML Control + DX Renderer                       |
+------------------------------------------------------------------+
                                |
+------------------------------------------------------------------+
|                    TerminalCore (LIB)                             |
|         Terminal State Machine, Buffer, Input/Output              |
+------------------------------------------------------------------+
                                |
+------------------------------------------------------------------+
|                    TerminalConnection (DLL)                       |
|           ConptyConnection, AzureConnection, etc.                 |
+------------------------------------------------------------------+
                                |
                           ConPTY API
                                |
+------------------------------------------------------------------+
|                    OpenConsole/conhost.exe                        |
|              Console Server, API Server, VT Engine                |
+------------------------------------------------------------------+
```

### Source Directory Organization

```
src/
├── cascadia/              # Windows Terminal specific code
│   ├── CascadiaPackage/   # MSIX packaging
│   ├── TerminalApp/       # Application shell (tabs, panes, UI)
│   ├── TerminalConnection/# Connection types (ConPTY, Azure, etc.)
│   ├── TerminalControl/   # XAML control + DX renderer
│   ├── TerminalCore/      # Core terminal state machine
│   ├── TerminalSettingsModel/ # JSON settings parsing
│   ├── TerminalSettingsEditor/ # Settings UI
│   ├── WindowsTerminal/   # Win32 window host
│   └── WpfTerminalControl/ # WPF version of terminal control
│
├── host/                  # Console host (conhost.exe)
│   ├── lib/              # Core host library
│   ├── dll/              # conhostv2.dll
│   ├── exe/              # OpenConsole.exe
│   └── ut_host/          # Unit tests
│
├── terminal/              # VT terminal components
│   ├── parser/           # VT sequence state machine
│   ├── adapter/          # VT dispatch to console API
│   └── input/            # Input handling
│
├── renderer/              # Rendering engines
│   ├── base/             # Abstract renderer interface
│   ├── atlas/            # Modern DX11/DirectWrite renderer
│   ├── gdi/              # Legacy GDI renderer
│   └── wddmcon/          # WDDM console renderer
│
├── buffer/out/            # Text buffer implementation
├── winconpty/             # ConPTY implementation
├── types/                 # Common types and utilities
├── inc/                   # Shared headers
│   └── til/              # Terminal Implementation Library
└── server/                # Console API server
```

### Key Abstractions

1. **ITerminalApi** - Interface for terminal operations
2. **IRenderEngine** - Abstract renderer interface
3. **IStateMachineEngine** - VT parser callback interface
4. **ITerminalConnection** - Connection abstraction (ConPTY, SSH, Azure)
5. **TextBuffer** - Screen buffer with scrollback
6. **Terminal** - Main terminal state class

---

## 3. Terminal Emulation

### VT Parser Implementation

**Location:** `/src/terminal/parser/`

The VT parser is a table-driven state machine based on the DEC VT500 series specification and Paul Williams' state machine (vt100.net).

**Key Files:**
- `stateMachine.hpp/cpp` - Main state machine (~70K lines)
- `InputStateMachineEngine.hpp/cpp` - Input sequence parsing
- `OutputStateMachineEngine.hpp/cpp` - Output sequence parsing
- `IStateMachineEngine.hpp` - Callback interface

**State Machine States:**
```cpp
enum class VTStates {
    Ground,           // Normal text
    Escape,           // ESC received
    EscapeIntermediate,
    CsiEntry,         // CSI sequence start
    CsiIntermediate,
    CsiIgnore,
    CsiParam,         // CSI parameter parsing
    CsiSubParam,
    OscParam,         // OSC sequence
    OscString,
    OscTermination,
    Ss3Entry,         // SS3 sequences
    Ss3Param,
    Vt52Param,        // VT52 compatibility
    DcsEntry,         // Device Control String
    DcsIgnore,
    DcsIntermediate,
    DcsParam,
    DcsPassThrough,
    SosPmApcString    // SOS/PM/APC strings
};
```

**Parameter Handling:**
- Maximum parameter value: 65535 (supports UTF-16 for win32-input-mode)
- Maximum parameters: 32
- Maximum sub-parameters per parameter: 6

### Terminal Dispatch Interface

**Location:** `/src/terminal/adapter/ITermDispatch.hpp`

Comprehensive interface for VT sequence actions:

```cpp
class ITermDispatch {
    // Cursor movement
    virtual void CursorUp(VTInt distance) = 0;
    virtual void CursorDown(VTInt distance) = 0;
    virtual void CursorPosition(VTInt line, VTInt column) = 0;

    // Screen manipulation
    virtual void EraseInDisplay(EraseType eraseType) = 0;
    virtual void ScrollUp(VTInt distance) = 0;

    // Graphics
    virtual void SetGraphicsRendition(VTParameters options) = 0;

    // Modes
    virtual void SetMode(ModeParams param) = 0;
    virtual void ResetMode(ModeParams param) = 0;

    // DCS handlers
    virtual StringHandler DefineSixelImage(...) = 0;
    virtual StringHandler DownloadDRCS(...) = 0;

    // ... 100+ more methods
};
```

### Supported Features

- **ECMA-48/ANSI** - Full support
- **DEC VT100-VT520** - Extensive support
- **xterm Extensions** - Many supported
- **Sixel Graphics** - Supported via `SixelParser.cpp`
- **DRCS (Soft Fonts)** - Supported via `FontBuffer.cpp`
- **OSC Sequences** - Extensive (clipboard, colors, hyperlinks)
- **Kitty Keyboard Protocol** - Partial support
- **Win32 Input Mode** - Microsoft extension for accurate key input

---

## 4. ConPTY (Pseudo Console)

**Location:** `/src/winconpty/`

ConPTY is Microsoft's pseudo-terminal implementation that allows terminal emulators to communicate with Windows console applications using VT sequences instead of the Windows Console API.

### How ConPTY Works

```
Terminal Emulator                    ConPTY                       Console App
      |                                |                              |
      |  CreatePseudoConsole()         |                              |
      |------------------------------->|                              |
      |                                |                              |
      |  PROC_THREAD_ATTRIBUTE_        |  CreateProcess()             |
      |  PSEUDOCONSOLE                 |----------------------------->|
      |                                |                              |
      |  Write VT to hInput            |                              |
      |------------------------------->| Parse VT, call Console API   |
      |                                |----------------------------->|
      |                                |                              |
      |                                | Console API output           |
      |                                |<-----------------------------|
      |  Read VT from hOutput          | Convert to VT                |
      |<-------------------------------|                              |
      |                                |                              |
      |  ResizePseudoConsole()         |                              |
      |------------------------------->| Signal pipe                  |
      |                                |                              |
```

### Core API

**Location:** `/src/winconpty/winconpty.cpp`

```cpp
// Create a pseudoconsole
HRESULT ConptyCreatePseudoConsole(
    COORD size,           // Terminal dimensions
    HANDLE hInput,        // Pipe for VT input to ConPTY
    HANDLE hOutput,       // Pipe for VT output from ConPTY
    DWORD dwFlags,        // PSEUDOCONSOLE_INHERIT_CURSOR, etc.
    HPCON* phPC           // Output handle
);

// Resize the pseudoconsole
HRESULT ConptyResizePseudoConsole(HPCON hPC, COORD size);

// Clear the buffer
HRESULT ConptyClearPseudoConsole(HPCON hPC, BOOL keepCursorRow);

// Show/hide window state
HRESULT ConptyShowHidePseudoConsole(HPCON hPC, bool show);

// Reparent the pseudo window
HRESULT ConptyReparentPseudoConsole(HPCON hPC, HWND newParent);

// Close the pseudoconsole
VOID ConptyClosePseudoConsole(HPCON hPC);
```

### PseudoConsole Structure

```cpp
typedef struct _PseudoConsole {
    HANDLE hSignal;        // Out-of-band signal pipe for resize, etc.
    HANDLE hPtyReference;  // Reference handle keeping conhost alive
    HANDLE hConPtyProcess; // Handle to the conhost process
} PseudoConsole;
```

### Signal Protocol

ConPTY uses a separate signal pipe for out-of-band communication:

```cpp
#define PTY_SIGNAL_SHOWHIDE_WINDOW  (1u)
#define PTY_SIGNAL_CLEAR_WINDOW     (2u)
#define PTY_SIGNAL_REPARENT_WINDOW  (3u)
#define PTY_SIGNAL_RESIZE_WINDOW    (8u)
```

### Flags

```cpp
#define PSEUDOCONSOLE_INHERIT_CURSOR        0x1
#define PSEUDOCONSOLE_GLYPH_WIDTH_GRAPHEMES 0x08
#define PSEUDOCONSOLE_GLYPH_WIDTH_WCSWIDTH  0x10
#define PSEUDOCONSOLE_GLYPH_WIDTH_CONSOLE   0x18
```

### Usage Pattern

**Location:** `/samples/ConPTY/EchoCon/EchoCon/EchoCon.cpp`

```cpp
// 1. Create pipes
HANDLE hPipeIn, hPipeOut;
CreatePipe(&hPipePTYIn, &hPipeOut, NULL, 0);
CreatePipe(&hPipeIn, &hPipePTYOut, NULL, 0);

// 2. Create pseudoconsole
HPCON hPC;
CreatePseudoConsole(consoleSize, hPipePTYIn, hPipePTYOut, 0, &hPC);

// 3. Set up process with PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE
STARTUPINFOEX startupInfo{};
UpdateProcThreadAttribute(
    startupInfo.lpAttributeList,
    0,
    PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE,
    hPC,
    sizeof(HPCON),
    NULL, NULL);

// 4. Create the child process
CreateProcess(..., &startupInfo.StartupInfo, ...);

// 5. Read VT output from hPipeIn, write VT input to hPipeOut
```

---

## 5. Rendering Pipeline

### Architecture

**Location:** `/src/renderer/`

Windows Terminal supports multiple rendering backends:

1. **AtlasEngine** (Default) - Modern Direct3D 11 + DirectWrite
2. **GDI Engine** - Legacy fallback
3. **WDDM Console** - Kernel-mode rendering for boot console

### Atlas Engine

**Location:** `/src/renderer/atlas/`

The Atlas Engine is a high-performance DirectX-based text renderer with a glyph cache.

**Key Components:**

1. **BackendD3D** - Primary renderer using Direct3D 11
2. **BackendD2D** - Fallback for Remote Desktop / software rendering
3. **Glyph Atlas** - Texture atlas for caching rendered glyphs

**Rendering Flow:**
```
1. _handleSettingsUpdate()    - Apply font/setting changes
2. _drawBackground()          - Fill background color bitmap
3. _drawCursorPart1()         - Draw cursor behind text
4. _drawText()                - Render all text glyphs
   ├── For each row
   │   ├── For each font face
   │   │   └── For each glyph
   │   │       ├── Look up in glyph cache
   │   │       ├── If missing, _drawGlyph()
   │   │       └── _appendQuad() - Stage for batch draw
   │   └── _drawGridlineRow() - Underlines, strikethrough
   └── _flushQuads()          - GPU draw call
5. _drawCursorPart2()         - Draw inverted cursor
6. _drawSelection()           - Draw selection highlight
7. _executeCustomShader()     - Apply pixel shader effects
```

### IRenderEngine Interface

**Location:** `/src/renderer/inc/IRenderEngine.hpp`

```cpp
class IRenderEngine {
    virtual HRESULT StartPaint() = 0;
    virtual HRESULT EndPaint() = 0;
    virtual HRESULT Present() = 0;

    virtual HRESULT Invalidate(const til::rect* region) = 0;
    virtual HRESULT InvalidateAll() = 0;

    virtual HRESULT PaintBackground() = 0;
    virtual HRESULT PaintBufferLine(
        std::span<const Cluster> clusters,
        til::point coord,
        bool fTrimLeft,
        bool lineWrapped) = 0;
    virtual HRESULT PaintCursor(const CursorOptions& options) = 0;
    virtual HRESULT PaintSelection(const til::rect& rect) = 0;

    virtual HRESULT UpdateFont(
        const FontInfoDesired& desired,
        FontInfo& actual) = 0;
    virtual HRESULT UpdateDpi(int dpi) = 0;
};
```

### DirectWrite Integration

**Location:** `/src/renderer/atlas/dwrite.cpp`

- Uses `IDWriteTextAnalyzer` for complex text shaping
- Supports variable fonts with axes (weight, width, slant)
- OpenType feature selection (ligatures, stylistic sets)
- Fallback font handling for missing glyphs

### Custom Pixel Shaders

**Location:** `/src/renderer/atlas/shader_ps.hlsl`

Windows Terminal supports custom HLSL pixel shaders for effects:
- Retro terminal effect (CRT scanlines)
- User-provided custom shaders

---

## 6. Configuration System

### JSON Settings

**Location:** `/src/cascadia/TerminalSettingsModel/`

Windows Terminal uses a layered JSON configuration system:

```
Layers (lowest to highest priority):
1. defaults.json        - Built-in defaults (read-only)
2. settings.json        - User settings
3. state.json          - Runtime state (window position, etc.)
```

### Default Settings Structure

**Location:** `/src/cascadia/TerminalSettingsModel/defaults.json`

```json
{
    "defaultProfile": "{guid}",

    // Global settings
    "initialCols": 120,
    "initialRows": 30,
    "launchMode": "default",
    "copyOnSelect": false,
    "wordDelimiters": " /\\()\"'...",

    // Profiles
    "profiles": [
        {
            "guid": "{...}",
            "name": "Windows PowerShell",
            "commandline": "powershell.exe",
            "colorScheme": "Campbell",
            "fontFace": "Cascadia Mono",
            "fontSize": 12,
            "padding": "8, 8, 8, 8"
        }
    ],

    // Color schemes
    "schemes": [
        {
            "name": "Campbell",
            "foreground": "#CCCCCC",
            "background": "#0C0C0C",
            "black": "#0C0C0C",
            ...
        }
    ],

    // Key bindings
    "actions": [...]
}
```

### JSON Utilities

**Location:** `/src/cascadia/TerminalSettingsModel/JsonUtils.h`

Type-safe JSON parsing with `ConversionTrait<T>`:

```cpp
template<typename T>
struct ConversionTrait {
    T FromJson(const Json::Value&);
    bool CanConvert(const Json::Value& json);
    Json::Value ToJson(const T& val);
    std::string TypeDescription() const;
};
```

### Profile Generators

Dynamic profile generation for:
- PowerShell Core (`PowershellCoreProfileGenerator.cpp`)
- WSL Distributions (`WslDistroGenerator.cpp`)
- Azure Cloud Shell (`AzureCloudShellGenerator.cpp`)
- Visual Studio (`VisualStudioGenerator.cpp`)
- SSH Hosts (`SshHostGenerator.cpp`)

---

## 7. Font Handling

### Font Configuration

**Location:** `/src/cascadia/TerminalSettingsModel/FontConfig.cpp`

```json
{
    "font": {
        "face": "Cascadia Code",
        "size": 12,
        "weight": "normal",
        "features": {
            "calt": 1,    // Contextual alternates (ligatures)
            "ss01": 1     // Stylistic set 1
        },
        "axes": {
            "wght": 400,  // Weight axis
            "wdth": 100   // Width axis
        }
    }
}
```

### DirectWrite Font Selection

1. Query system fonts via `IDWriteFontCollection`
2. Create `IDWriteFontFace` for selected font
3. Apply OpenType features via `IDWriteTextLayout`
4. Handle font fallback for missing glyphs

### Cascadia Code

Windows Terminal ships with Cascadia Code/Mono:
- Cascadia Code - With programming ligatures
- Cascadia Mono - Without ligatures
- Supports Powerline symbols
- Variable font with weight axis

---

## 8. Performance Optimizations

### Text Buffer

**Location:** `/src/buffer/out/`

1. **Run-Length Encoding (RLE)**
   - **Location:** `/src/inc/til/rle.h`
   - Attributes are stored as RLE-compressed runs
   - Efficient for consecutive cells with same attributes

2. **Row-based Storage**
   - Each row is a separate object (`Row.hpp`)
   - Rows contain: text (wchar_t), attributes (RLE), wide char flags

3. **Generational Updates**
   - **Location:** `/src/inc/til/generational.h`
   - Generation counters to track dirty state
   - Avoids unnecessary redraws

### Rendering Optimizations

1. **Glyph Caching**
   - Atlas texture stores pre-rendered glyphs
   - Hash-based lookup for glyph reuse
   - Automatic atlas resizing

2. **Dirty Region Tracking**
   - Only re-render changed areas
   - Scroll optimization with `InvalidateScroll()`

3. **Batched Draw Calls**
   - Accumulate quads, single draw call per batch
   - Instance buffer for glyph positions

4. **Dual Renderer Backends**
   - BackendD3D for GPU acceleration
   - BackendD2D fallback for Remote Desktop

### Memory Optimizations

1. **Small Vector Optimization**
   - **Location:** `/src/inc/til/small_vector.h`
   - Inline storage for small collections

2. **Single-Producer Single-Consumer Queue**
   - **Location:** `/src/inc/til/spsc.h`
   - Lock-free queue for inter-thread communication

3. **Custom Allocators**
   - Pool allocators for frequently allocated objects

### Threading Model

1. **Main Thread** - UI, rendering
2. **Connection Thread** - PTY I/O
3. **Parser Thread** - VT parsing (in some configurations)

Uses `til::ticket_lock` for reader/writer synchronization.

---

## 9. Strengths

### Technical Excellence

1. **Modern C++ Codebase**
   - C++17/20 features throughout
   - Smart pointers, RAII patterns
   - Strong type safety

2. **Comprehensive VT Support**
   - One of the most complete VT implementations
   - Sixel graphics, soft fonts, all DEC modes
   - Continuous fuzzing for parser robustness

3. **High-Performance Rendering**
   - Custom GPU-accelerated text renderer
   - Glyph caching with atlas
   - Sub-5ms input latency target

4. **ConPTY Innovation**
   - First-class PTY support for Windows
   - Enables modern terminal ecosystem
   - Well-documented API

### User Experience

1. **Rich Settings System**
   - JSON-based, highly customizable
   - Profile inheritance
   - Themes and color schemes

2. **Modern Features**
   - Tabs, panes, split views
   - GPU-accelerated rendering
   - Custom shaders support

3. **Accessibility**
   - UIA (UI Automation) support
   - Screen reader compatibility
   - High contrast mode

### Developer Experience

1. **Open Source**
   - MIT License
   - Active community
   - Regular releases

2. **Good Documentation**
   - Architecture docs
   - Spec documents for features
   - Sample code

3. **Testing Infrastructure**
   - Unit tests with TAEF
   - Integration tests
   - Fuzzing with OneFuzz

---

## 10. Weaknesses/Limitations

### Platform Limitations

1. **Windows-Only**
   - Deeply tied to Windows APIs
   - XAML Islands, WinRT dependencies
   - No cross-platform support

2. **Windows 10 2004+ Required**
   - No support for older Windows versions
   - ConPTY requires specific Windows builds

### Technical Debt

1. **Complex Build System**
   - Mix of MSBuild, props files
   - Heavy Visual Studio dependency
   - Slow build times

2. **GDI Legacy Code**
   - Still maintains GDI renderer
   - Some legacy console code paths
   - Mixed old/new patterns

3. **XAML Islands Complexity**
   - Complex interop layer
   - Performance overhead
   - Deployment challenges

### Missing Features

1. **No Built-in SSH**
   - Relies on external SSH clients
   - No integrated key management

2. **Limited Remote Support**
   - No native remote terminal protocol
   - No mosh-style roaming

3. **No Multiplexing**
   - Each pane is a separate process
   - No tmux-style session management

### Performance Issues

1. **Startup Time**
   - XAML Islands initialization overhead
   - First paint can be slow

2. **Memory Usage**
   - Each tab is a separate conhost process
   - Higher memory footprint than some alternatives

---

## 11. Lessons for dterm

### ConPTY Integration (Critical for Windows Support)

**Recommendation:** Use ConPTY as the primary PTY mechanism on Windows.

**Key Implementation Points:**

1. **Create ConPTY with pipes:**
```rust
// Rust pseudo-code for dterm
pub struct ConPty {
    handle: HPCON,
    input_pipe: Handle,   // Write VT here
    output_pipe: Handle,  // Read VT from here
    signal_pipe: Handle,  // For resize signals
}

impl ConPty {
    pub fn new(cols: u16, rows: u16) -> Result<Self> {
        // Create pipes
        // CreatePseudoConsole()
        // Return struct with handles
    }

    pub fn resize(&self, cols: u16, rows: u16) -> Result<()> {
        // ConptyResizePseudoConsole()
    }
}
```

2. **Use PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE** to attach child processes

3. **Handle signal pipe** for resize, show/hide, reparent

### VT Parser Design

**Recommendation:** Adopt similar state machine architecture.

**Key Points:**
- Table-driven state machine for performance
- Support C1 controls (both 7-bit and 8-bit)
- Parameter limits: 65535 max value, 32 parameters
- Separate input and output engines

### Rendering Architecture

**Recommendation:** Use wgpu for cross-platform GPU rendering, but study Atlas Engine patterns.

**Adoptable Patterns:**
1. Glyph atlas with hash-based lookup
2. RLE-compressed attributes
3. Dirty region tracking
4. Batched draw calls

### Buffer Design

**Recommendation:** Study the `TextBuffer` and `Row` implementations.

**Key Patterns:**
```rust
// Rust adaptation
struct Row {
    text: Vec<char>,
    attrs: Rle<TextAttribute>,
    line_rendition: LineRendition,
}

struct TextBuffer {
    rows: VecDeque<Row>,
    scrollback_size: usize,
    generation: u64,
}
```

### Configuration

**Recommendation:** JSON-based settings with profile inheritance.

**Adaptable Patterns:**
- Default + user layers
- Profile inheritance
- Dynamic profile generators (WSL, SSH hosts)
- Hot-reload support

### TIL (Terminal Implementation Library)

**Recommendation:** Study and potentially adapt these utilities:

1. **`til::rle`** - Run-length encoding for attributes
2. **`til::small_vector`** - Small buffer optimization
3. **`til::enumset`** - Type-safe enum flags
4. **`til::ticket_lock`** - Reader/writer lock
5. **`til::generational`** - Generation counters for dirty tracking

### Testing Strategy

**Recommendation:** Implement similar testing approach:

1. Unit tests for all components
2. VT parser fuzzing (critical!)
3. Integration tests for ConPTY
4. UI automation tests

### What NOT to Copy

1. **XAML Islands** - Too complex, Windows-specific
2. **WinRT dependencies** - Use native Rust abstractions
3. **Complex MSBuild** - Use Cargo
4. **GDI fallback** - Not needed with wgpu

---

## File Reference

### Critical Files for ConPTY Integration

| File | Purpose |
|------|---------|
| `/src/winconpty/winconpty.cpp` | ConPTY implementation |
| `/src/winconpty/winconpty.h` | ConPTY header |
| `/samples/ConPTY/EchoCon/` | Simple ConPTY example |
| `/samples/ConPTY/MiniTerm/` | C# ConPTY example |

### VT Parser Reference

| File | Purpose |
|------|---------|
| `/src/terminal/parser/stateMachine.hpp` | State machine header |
| `/src/terminal/parser/stateMachine.cpp` | State machine implementation |
| `/src/terminal/adapter/ITermDispatch.hpp` | Dispatch interface |
| `/src/terminal/adapter/adaptDispatch.cpp` | Dispatch implementation |

### Renderer Reference

| File | Purpose |
|------|---------|
| `/src/renderer/atlas/AtlasEngine.h` | Atlas engine header |
| `/src/renderer/atlas/BackendD3D.cpp` | D3D rendering |
| `/src/renderer/inc/IRenderEngine.hpp` | Renderer interface |

### Buffer Reference

| File | Purpose |
|------|---------|
| `/src/buffer/out/textBuffer.hpp` | Text buffer header |
| `/src/buffer/out/Row.hpp` | Row structure |
| `/src/inc/til/rle.h` | RLE implementation |

---

## Conclusion

Windows Terminal is a well-engineered, modern terminal that serves as an excellent reference for Windows platform support. The key insight for dterm is that **ConPTY is the essential component** - it provides Unix-like PTY semantics on Windows, enabling VT-based terminal communication.

For dterm's Windows support:
1. **Use ConPTY exclusively** - Don't try to use legacy Console API
2. **Study the VT parser** - Comprehensive and well-tested
3. **Adopt efficient patterns** - RLE attributes, glyph caching, dirty tracking
4. **Skip Windows-specific UI** - Use cross-platform solutions (wgpu, native Rust)

The ConPTY API is stable, well-documented, and available on Windows 10 1809+. This should be the foundation of dterm's Windows PTY implementation.
