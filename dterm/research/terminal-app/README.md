# Terminal.app Reverse Engineering Analysis

**Platform:** macOS
**Binary:** `/System/Applications/Utilities/Terminal.app`
**Version:** 2.14 (build 455.1)
**Analysis Date:** 2024-12-27
**Tools Used:** radare2 6.0.7, r2ghidra (Ghidra decompiler)

---

## Executive Summary

Terminal.app is Apple's built-in terminal emulator, in development since 1991. Through reverse engineering via decompilation, we've analyzed its architecture to understand design decisions and identify areas where dTerm can improve.

**Key Finding:** Terminal.app uses **pure CPU-based rendering** via CoreGraphics/CoreText. No Metal, OpenGL, or GPU acceleration is present in the binary. This represents a significant performance opportunity for dTerm.

---

## Binary Overview

| Property | Value |
|----------|-------|
| Binary Size | 2.1 MB (universal arm64e + x86_64) |
| Text Segment | 802 KB |
| Data Segment | 131 KB |
| Objective-C Classes | ~100 classes (TT* prefix) |
| GPU Usage | **None** (confirmed: no Metal/OpenGL references) |
| Rendering | CoreGraphics + CoreText (CPU) |
| PTY | Unix `forkpty()` via libutil |

### Linked Frameworks

```
ApplicationServices.framework
AudioToolbox.framework (bell sounds)
Carbon.framework
Cocoa.framework
CoreAudio.framework
CoreServices.framework
DataDetectorsCore.framework (URL detection)
QuartzCore.framework
CoreFoundation.framework
CoreGraphics.framework
CoreText.framework
AppKit.framework
libncurses.5.4.dylib (terminfo)
libicucore.A.dylib (Unicode)
```

---

## Architecture

### Class Hierarchy

```
TTApplication (NSApplication)
├── TTWindowController (NSWindowController)
│   ├── TTWindow (NSWindow)
│   ├── TTTabView (NSTabView)
│   │   └── TTTabViewItem (NSTabViewItem)
│   │       └── TTTabController
│   │           ├── TTView (NSView) - Main terminal view
│   │           ├── TTLogicalScreen - Terminal state machine
│   │           ├── TTVT100Emulator - Escape sequence parser
│   │           └── TTShell - PTY management
│   └── TTPane (NSView)
├── TTProfileManager
├── TTWorkspaceManager
└── TTIOManager - I/O coordination
```

### Data Flow

```
PTY Input
    │
    ▼
TTIOManager (I/O thread)
    │
    ▼
TTVT100Emulator.decodeData: (Parser)
    │
    ├──► TTLogicalScreen (State updates)
    │         │
    │         ▼
    │    TTMultiLineBuffer (Storage)
    │         │
    │         ▼
    │    TTScrollbackTextStorage
    │
    ▼
TTView.drawRect: (Rendering)
    │
    ▼
CoreGraphics/CoreText (CPU)
    │
    ▼
Display
```

---

## Core Components (Decompiled)

### 1. VT100 Parser (`TTVT100Emulator`)

The parser uses a **translation table-based state machine** for O(1) byte classification:

- 2KB lookup table (256 entries × 8 bytes)
- Direct table indexing for ASCII (< 0x81)
- Extended handling for UTF-8 multi-byte sequences
- State stored at offset `0x08` in emulator object
- Translation table pointer at offset `0x30`

**Supported escape sequences:**
- CSI (0x5b `[`) - Control Sequence Introducer
- OSC (0x5d `]`) - Operating System Commands
- DCS (0x5c `\`) - Device Control Strings
- Standard VT100/VT220/xterm sequences

See: `decompiled/TTVT100Emulator_decodeData.c`

### 2. Buffer Management (`TTMultiLineBuffer`)

Uses **run-length encoding** for efficient attribute storage:

**Object Layout:**
```
Offset  Field
0x08    chars (raw UTF-8 bytes array)
0x10    runs (attribute runs array)
0x18    lineOffsets (line start indices)
0x30    unicharCacheGeneration (invalidation counter)
```

**Attribute Run Structure (~32 bytes):**
```c
struct AttributeRun {
    uint64_t length;      // Byte count for this run
    uint16_t flags;       // bold, italic, underline, blink, inverse
    uint64_t fgColor;     // Foreground (index or RGB)
    uint64_t bgColor;     // Background (index or RGB)
};
```

**Key optimization:** Adjacent characters with identical attributes share one run record, avoiding per-character allocation.

See: `decompiled/TTMultiLineBuffer_append.c`

### 3. Screen State (`TTLogicalScreen`)

Manages terminal grid state with dirty tracking:

**Key Methods:**
- `markLineAsDirty:` - Bitmap-based dirty tracking
- `clearDirtyLines` - Reset after render
- `getLine:UTF8Characters:runs:` - Extract line data
- `setRowCount:columnCount:` - Resize handling
- `activateAlternateScreen:clearing:` - Alt buffer support

**Dirty Tracking (bitmap-based):**
```c
// 64-bit words, one bit per line
void markLineAsDirty(screen, line) {
    screen->dirtyBitmap[line / 64] |= (1ULL << (line % 64));
}
```

### 4. Rendering (`TTView`)

**Critical finding:** Pure CPU rendering, no GPU acceleration.

**Render Pipeline:**
1. `drawRect:` entry point
2. Setup CGContext with identity transform
3. Configure antialiasing based on scale factor
4. Get dirty rectangles via `getRectsBeingDrawn:`
5. Fill backgrounds with `NSRectFillUsingOperation` (CPU compositing)
6. Call `drawAttributedStringsToScreen:` for text
7. Build `NSAttributedString` per line
8. Render via CoreText

**Key inefficiencies:**
- `NSAttributedString` allocation per line per frame
- No glyph caching or texture atlas
- Line-by-line drawing, not batched
- Main thread rendering, no dedicated render thread

See: `decompiled/TTView_drawRect.c`

### 5. PTY Management (`TTShell`)

Standard Unix PTY via `forkpty()`:

**Initialization:**
1. Query `TERM` environment variable
2. Validate via `tgetent()` (terminfo lookup)
3. Fallback chain: xterm-256color → xterm → vt100 → unknown
4. Setup locale from `NSLocale`
5. Configure environment variables

**File Descriptors:**
- Master FD stored at offset `0x28` (initialized to -1)
- Slave FD passed to child process

See: `decompiled/TTShell_init.c`

---

## Additional Features (Previously Undocumented)

### 6. Bookmarks and Marks System

Terminal.app has a sophisticated **command navigation** system:

**Classes:** `TTInsertBookmarkSheetController`, mark-related methods in `TTView`

**Features:**
- **Bookmarks**: Named markers users can insert at any line
- **Automatic marks**: Lines are marked when Enter is pressed (command prompts)
- **Navigation**: Jump to previous/next marker or bookmark
- **Selection**: Select text between markers
- **Clear to marker**: Delete output up to a bookmark

**Key Methods:**
```objc
- insertBookmarkWithName:
- jumpToPreviousMarker:
- jumpToNextMarker:
- selectToPreviousMarker:
- selectToNextMarker:
- clearToBookmark:
- markAndSendReturn:
```

**Touch Bar Integration:** Dedicated markers/bookmarks Touch Bar items

### 7. Bonjour Service Browser

Discovers network services for quick SSH/Telnet connections:

**Classes:** `ServiceBrowser`, `ConnectToService`

**Features:**
- Browses `_ssh._tcp` and `_telnet._tcp` Bonjour services
- User-defined services list
- Connection UI with command line preview
- NSNetServiceBrowser integration

**Key Methods:**
```objc
- netServiceBrowser:didFindService:moreComing:
- netServiceBrowser:didRemoveService:moreComing:
- netServiceDidResolveAddress:
- connectToServer:
```

### 8. AppleScript Automation

Full scriptable interface via SDEF (24KB definition file):

**URL:** `Terminal.sdef`

**Scriptable Objects:**
- `application` - Default/startup settings
- `window` - Size, position, tabs
- `tab` - Contents, history, TTY, processes, settings
- `settings set` - Colors, fonts, dimensions, title options

**Key Commands:**
```applescript
do script "command" in window 1
get contents of tab 1 of window 1
get history of tab 1 of window 1
set current settings of tab 1 to settings set "Pro"
```

**Properties Exposed:**
- `contents` - Currently visible text
- `history` - Full scrollback buffer
- `busy` - Whether running a process
- `processes` - List of running processes
- `tty` - PTY device path (e.g., `/dev/ttys001`)

### 9. Workspaces (Window Groups)

Save and restore window arrangements:

**Class:** `TTWorkspace`

**Features:**
- Named window groups
- Encode/decode via NSCoder (state restoration)
- Property list serialization
- Window restoration on app launch

**Key Methods:**
```objc
- encodeWithCoder:
- initWithCoder:
- propertyListRepresentation
- initWithPropertyListRepresentation:
- applicationDidRestoreWindows:
```

### 10. URL Scheme Handlers

Registered URL schemes:

| Scheme | Purpose |
|--------|---------|
| `ssh://` | Open SSH connection |
| `telnet://` | Open Telnet connection |
| `x-man-page://` | Open man page (e.g., `x-man-page://ls`) |

**Handler:** `get URL` AppleScript command

### 11. Touch Bar Support

Full Touch Bar integration:

**Items:**
- New Remote Connection
- Markers navigation (prev/next)
- Insert Bookmark
- Navigate Bookmarks
- Man page lookup
- Background color picker
- Option-as-Meta toggle

**Key Methods:**
```objc
- touchBar:makeItemForIdentifier:
- makeTouchBarMarkersItem
- makeTouchBarInsertBookmarkItem
- makeTouchBarNewRemoteConnectionItem
- makeTouchBarBackgroundColorItem
```

### 12. Services Menu Integration

macOS Services support:

**Provided Services:**
- "New Terminal at Folder" - Opens terminal at selected folder
- "New Terminal Tab at Folder"
- Other text-based services

**Implementation:** `NSServices` in Info.plist

### 13. Secure Keyboard Entry

Protection against keyloggers:

**Class:** Methods in `TTWindowController`

**Features:**
- Toggle via menu (Shell → Secure Keyboard Entry)
- Uses macOS `EnableSecureEventInput()` API
- Prevents other apps from reading keystrokes

**Key Methods:**
```objc
- enableSecureInput:
- toggleSecureKeyboardEntry:
- setSecureKeyboardEntry:
```

### 14. Bracketed Paste Mode

Protection against paste injection attacks:

**Implementation:** `bracketedPasteMode` property

**Security:**
- Detects paste-end escape sequence in pasted text
- Warns user if pasted text could escape bracketed mode
- Prevents arbitrary command execution from malicious clipboard content

**Error Message:**
> "Bracketed Paste Mode is enabled in the terminal and the text to send contains the escape sequence that marks the end of pasted text. This is invalid for Bracketed Paste Mode, and could allow subsequent content in the text to perform arbitrary commands."

### 15. Inspector Window

View and modify terminal properties:

**Class:** `TTInspectorController`

**Features:**
- Settings controller access
- Title field editing
- Background color well
- Working directory URL
- Represented URL (for proxy icons)
- State restoration support

### 16. Find Panel

In-terminal text search:

**Class:** `TTFindPanel`

**Features:**
- Pattern-based search
- Case sensitivity options
- Find next/previous
- Recent patterns history
- Find pasteboard integration (shared across apps)

### 17. Data Detectors

Automatic URL and file path detection:

**Class:** `TTDataDetectorsSoftLinking`

**Features:**
- Clickable URLs in terminal output
- File path detection
- Uses `DataDetectorsCore.framework`
- Soft-linked for graceful degradation

### 18. Quarantine Support

macOS security integration:

**Class:** `Quarantine`

**Features:**
- Tracks file provenance
- Sets quarantine attributes on downloaded scripts
- Gatekeeper integration

### 19. East Asian Width Handling

Unicode character width support:

**Property:** `isEastAsianAmbiguousWide`

**Purpose:** Handle ambiguous-width characters (characters that are narrow in Western contexts but wide in East Asian contexts). User preference for how to render these.

### 20. Encoding Controller

Multiple text encoding support:

**Class:** `TTEncodingController`

**Features:**
- ~100+ text encodings
- Enable/disable individual encodings
- Default encodings preset
- Per-profile encoding setting

**Key Methods:**
```objc
- availableEncodings
- enabledEncodings
- enableAll:
- disableAll:
- revertToDefaults:
```

### 21. Process Information

Track processes running in terminal:

**Class:** `TTProcessInfo`

**Properties:**
- `pid` - Process ID
- `name` - Process name
- `command` - Full command
- `argv` - Argument vector
- `pathname` - Executable path
- `tty` - TTY device
- `user` - Running user

**Used for:**
- Tab title updates
- "Close tab?" warnings
- Process list in AppleScript

### 22. Key Mappings

Custom keyboard shortcuts:

**Class:** `TTKeyMappingsController`

**Features:**
- Add/edit/delete key mappings
- Map keys to escape sequences
- Map keys to actions
- Per-profile key bindings

---

## Efficiency Analysis

### What Terminal.app Does Well

| Technique | Benefit |
|-----------|---------|
| Table-driven parser | O(1) byte classification |
| Run-length encoding | Efficient attribute storage |
| Dirty line bitmap | Skip unchanged lines |
| UTF-8/UTF-16 caching | Avoid repeated conversion |
| Threaded I/O | Non-blocking PTY reads |

### What Terminal.app Does Poorly

| Issue | Impact |
|-------|--------|
| CPU rendering | Every pixel through CoreGraphics |
| NSAttributedString per line | Heavy allocation each frame |
| No draw batching | Line-by-line, not instanced |
| No texture atlas | No glyph caching |
| Main thread render | UI thread blocked during draw |
| No frame coalescing | Renders on every I/O event |

### Performance Comparison

| Metric | Terminal.app | GPU Terminal (target) |
|--------|--------------|----------------------|
| Character storage | 32-byte run struct | 10-16 byte packed cell |
| Render calls/frame | N (one per line) | 1 (instanced draw) |
| Glyph rendering | CoreText per line | Texture atlas lookup |
| Frame budget @ 60fps | ~16ms+ | <1ms |
| Scrollback memory | NSAttributedString | Compressed pages |

---

## Data Structures

### Cell Attributes (Reverse Engineered)

```c
// Packed into uint16_t flags field
struct CellAttrs {
    unsigned bold      : 1;
    unsigned dim       : 1;
    unsigned underline : 1;
    unsigned inverted  : 1;
    unsigned blink     : 1;
    unsigned invisible : 1;
    unsigned tab       : 1;
    unsigned marked    : 1;
    unsigned custom    : 1;
    unsigned italic    : 1;
    // ansiForegroundColor and ansiBackgroundColor stored separately
};
```

### Color Table

- 256-color palette support
- Dynamic colors (OSC 4)
- Special colors: foreground, background, cursor, selection, bold
- RGB true color via separate path

---

## Recommendations for dTerm

### Keep from Terminal.app

1. **Table-driven parser** - O(1) is optimal
2. **Dirty line tracking** - Essential optimization
3. **Run-length concept** - But with denser encoding

### Improve Upon

1. **Cell storage:**
```rust
#[repr(C)]
struct Cell {
    glyph: u32,       // 4 bytes - codepoint or glyph ID
    fg: u16,          // 2 bytes - palette index or RGB flag
    bg: u16,          // 2 bytes
    attrs: u16,       // 2 bytes - packed bitflags
}  // 10 bytes vs 32+ bytes
```

2. **GPU rendering:**
```rust
fn render_frame(&mut self, dirty: &BitSet) {
    let vertices: Vec<GlyphVertex> = dirty.iter()
        .flat_map(|line| self.grid.line(line).cells())
        .map(|cell| GlyphVertex::from(cell))
        .collect();

    self.gpu.draw_instanced(&vertices);  // Single draw call
}
```

3. **Texture atlas for glyphs:**
```rust
struct GlyphAtlas {
    texture: wgpu::Texture,
    cache: HashMap<GlyphKey, AtlasRegion>,
    // Rasterize on-demand, cache in GPU texture
}
```

4. **Render thread separation:**
```rust
// Terminal.app: main thread does everything
// dTerm: dedicated render thread
std::thread::spawn(move || {
    loop {
        let frame = rx.recv();  // Wait for frame data
        render_to_gpu(&frame);  // Off main thread
    }
});
```

---

## Feature Summary

| Category | Features |
|----------|----------|
| **Core** | VT100/VT220 emulation, UTF-8, 256 colors, true color |
| **Navigation** | Bookmarks, marks, jump to prompt, select between markers |
| **Network** | Bonjour service browser, SSH/Telnet URL schemes |
| **Automation** | Full AppleScript support, Services menu |
| **UI** | Tabs, workspaces, inspector, find panel, Touch Bar |
| **Security** | Secure keyboard, bracketed paste protection, quarantine |
| **Intl** | 100+ encodings, East Asian width, IME support |
| **Integration** | Data detectors, URL handling, man page URLs |

**Total: 22 documented feature areas** in ~800KB of code.

---

## Files in This Directory

```
research/terminal-app/
├── README.md                           # This file (comprehensive analysis)
└── decompiled/
    ├── TTVT100Emulator_decodeData.c   # Parser main loop
    ├── TTMultiLineBuffer_append.c      # Buffer management
    ├── TTView_drawRect.c               # Rendering entry
    ├── TTShell_init.c                  # PTY initialization
    ├── TTLogicalScreen_methods.txt     # 100+ method listing
    └── class_hierarchy.txt             # All 100 ObjC classes
```

---

## Tools Used

```bash
# Install radare2
brew install radare2

# Install Ghidra decompiler plugin
r2pm -U
r2pm -ci r2ghidra

# Decompile a method
r2 -q -e bin.relocs.apply=true -c "aaa; s <address>; pdg" /tmp/Terminal_binary

# List Objective-C classes
r2 -q -c "ic" /path/to/binary

# List methods of a class
r2 -q -c "ic <ClassName>" /path/to/binary
```

---

## Legal Note

This analysis is for research purposes to inform the design of dTerm. No Apple source code was accessed. All information was derived from:
- Binary analysis of publicly distributed software
- Public documentation and specifications
- Standard reverse engineering techniques

Terminal.app is © 1991-2024 Apple Inc.
