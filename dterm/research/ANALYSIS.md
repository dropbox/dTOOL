# iTerm2 (dashterm2) Analysis for dterm

**Analysis Date:** 2024-12-28
**Source Directory:** `/Users/ayates/dashterm2/sources/`
**Purpose:** Identify good ideas to incorporate and bad ideas to avoid for dterm development

---

## Executive Summary

iTerm2 is a mature, feature-rich terminal emulator with ~380,000 lines of Objective-C/Swift code across ~2,200 source files. While it has excellent features for shell integration, triggers, and tmux integration, its architecture shows significant technical debt accumulated over 20+ years of development. dterm should learn from its good ideas while avoiding its architectural pitfalls.

---

## Part 1: Good Ideas to Incorporate

### 1. Shell Integration (OSC 133)

**Implementation Location:** `VT100Terminal.m`, `VT100ScreenMutableState+TerminalDelegate.m`, `PromptStateMachine.swift`

**How It Works:**
- Uses FinalTerm's OSC 133 escape sequences (A, B, C, D codes)
- State machine (`PromptStateMachine.swift`) tracks: `ground` -> `receivingPrompt` -> `enteringCommand` -> `echoingBack` -> `executing`
- Shell sends escape codes at key moments:
  - `A` - Prompt is about to start
  - `B` - User is now entering command (prompt complete)
  - `C` - Command execution begins
  - `D` - Command finished (with return code)

**Mark Storage:**
- `VT100ScreenMark` stores per-command metadata: prompt range, command range, output start, return code, timestamps
- Uses `IntervalTree` for efficient lookup by screen position
- Marks persist through scrollback and are serialized for session restoration

**Good Ideas for dterm:**
1. Clean state machine for prompt tracking
2. Storing command boundaries with output start position
3. Promise-based return code handling
4. Separation of mark storage from terminal state

```
Prompt Flow:
[OSC 133;A] -> mark.promptRange.start
prompt text
[OSC 133;B] -> mark.promptRange.end, mark.commandRange.start
user types command
[OSC 133;C] -> mark.commandRange.end, mark.outputStart
command output
[OSC 133;D;retcode] -> mark.code, mark.endDate
```

### 2. Triggers System

**Implementation Location:** `Trigger.h/m`, `PTYTriggerEvaluator.m`, various `*Trigger.m` files

**Architecture:**
- Base `Trigger` class with subclasses for each action type
- Evaluator checks triggers on each line of output
- Supports partial-line (instant) triggers with rate limiting (0.5s minimum)
- Actions include: highlight, alert, send text, run script, capture output, set hostname, etc.

**Good Ideas for dterm:**
1. **Idempotent triggers** - Can re-run on same content safely
2. **Partial line support** - Triggers before line is complete
3. **Rate limiting** - Prevents CPU waste on fast output
4. **Match types** - Regex with precision levels (low, normal, high)
5. **Action abstraction** - Clean separation between matching and action execution

**Trigger Actions (20+ types):**
- `HighlightTrigger`, `AlertTrigger`, `BellTrigger`, `BounceTrigger`
- `SendTextTrigger`, `ScriptTrigger`, `CoprocessTrigger`
- `CaptureTrigger`, `AnnotateTrigger`, `MarkTrigger`
- `SetHostnameTrigger`, `SetDirectoryTrigger`
- `iTermShellPromptTrigger` (detects prompts without shell integration)

### 3. Tmux Integration

**Implementation Location:** `TmuxGateway.h/m`, `TmuxController.h/m`, `VT100TmuxParser.m`

**Control Mode Architecture:**
- `TmuxGateway` - Protocol-level communication with tmux
- `TmuxController` - High-level window/pane management
- Parser handles tmux's control mode output format

**Good Ideas for dterm:**
1. **Control mode parsing** - Separate parser for tmux protocol
2. **Subscription system** - `subscribeToFormat:target:block:` for tmux events
3. **Pause mode** - Flow control for slow clients
4. **Window/pane affinity** - Remembers which iTerm windows contain which tmux panes
5. **Dual-mode operation** - Works with or without tmux

### 4. Semantic History (Cmd-Click to Open Files)

**Implementation Location:** `iTermSemanticHistoryController.h/m`, `iTermURLActionFactory.m`

**How It Works:**
1. On click, extracts text around cursor using prefix/suffix analysis
2. Searches filesystem for matching paths
3. Handles line numbers in path syntax (`file.py:42`)
4. Opens in configured editor or default app

**Good Ideas for dterm:**
1. **Brute-force path detection** - Tries combinations of text around cursor
2. **Working directory awareness** - Resolves relative paths
3. **Line number extraction** - Parses `file:line:column` formats
4. **Configurable actions** - URL vs file vs custom command

### 5. Profiles System

**Implementation Location:** `ProfileModel.h/m`, `iTermProfilePreferences.m`

**Architecture:**
- Profiles stored as NSDictionary with GUID
- `ProfileModel` manages collections (shared instance + sessions instance)
- "Divorced" profiles for per-session modifications
- Dynamic profiles loaded from JSON files

**Good Ideas for dterm:**
1. **GUID-based identification** - Stable references
2. **Profile inheritance** - Base profile with overrides
3. **Session-specific divorce** - Modify without affecting stored profile
4. **Dynamic loading** - External JSON files auto-loaded

### 6. Session Restoration

**Implementation Location:** `iTermRestorableStateController.h/m`, `iTermRestorableSession.h`

**How It Works:**
- Saves window arrangements and session state
- Restores PTY with running processes (via server mode)
- Persists scrollback content and marks
- Handles both macOS state restoration API and custom restoration

**Good Ideas for dterm:**
1. **Process resurrection** - Server mode keeps processes alive
2. **Scrollback persistence** - Save/restore with compression
3. **Mark preservation** - Command history survives restarts
4. **Arrangement saving** - Window layouts remembered

### 7. Hotkey Window

**Implementation Location:** `iTermHotKeyController.m`, `iTermProfileHotKey.m`, `iTermCarbonHotKeyController.m`

**Architecture:**
- Uses Carbon API for global hotkey registration
- Separate window controller for hotkey windows
- Smooth animation for show/hide
- Works across spaces

**Good Ideas for dterm:**
1. **Carbon API** - Most reliable for global hotkeys on macOS
2. **Dedicated window class** - Special behavior for hotkey windows
3. **Space awareness** - Shows on current space
4. **Auto-hide** - Hides when losing focus (configurable)

### 8. Captured Output

**Implementation Location:** `CapturedOutput.h/m`, `CaptureTrigger.m`, `ToolCapturedOutputView.m`

**How It Works:**
- `CaptureTrigger` matches output and captures to `CapturedOutput` objects
- Links to `VT100ScreenMark` for navigation
- Displayed in toolbelt panel
- Clickable to jump to source line

**Good Ideas for dterm:**
1. **Trigger-based capture** - Flexible matching
2. **Mark linkage** - Output associated with commands
3. **Mergeable outputs** - Consecutive matches combine
4. **Toolbelt UI** - Non-intrusive display

### 9. Annotations

**Implementation Location:** `PTYAnnotation.h/m`, `AnnotateTrigger.m`

**How It Works:**
- `PTYAnnotation` objects stored in `IntervalTree`
- Associated with screen regions via intervals
- Trigger-created or manual
- Doppelganger pattern for thread safety

**Good Ideas for dterm:**
1. **Interval-based storage** - Efficient lookup
2. **Hide/show controls** - Non-destructive
3. **Trigger integration** - Auto-annotate patterns
4. **Thread-safe cloning** - Doppelganger pattern

### 10. Smart Selection

**Implementation Location:** `SmartSelectionController.h/m`, `iTermTextExtractor.m`

**How It Works:**
- Rules with regex patterns and precision levels
- Actions associated with matches
- Default rules for URLs, paths, emails, etc.
- User-configurable rules

**Good Ideas for dterm:**
1. **Precision scoring** - Higher precision = better match
2. **Extensible rules** - User-defined patterns
3. **Context actions** - Right-click menu based on match type
4. **Default set** - Useful out-of-box

---

## Part 2: Bad Ideas to Avoid

### 1. Performance Issues (~60 MB/s vs Alacritty's 200+ MB/s)

**Root Causes Identified:**

**a) Objective-C Message Dispatch Overhead:**
- Every character processed involves multiple ObjC message sends
- `screen_char_t` struct is 16+ bytes per cell with complex bit packing
- Virtual method calls through delegates everywhere

**b) Main Thread Bottleneck:**
- `PTYSession.m` is 21,894 lines - too much responsibility
- Triggers evaluated on every line of output
- Side effects scheduled through complex queuing system

**c) Inefficient Data Structures:**
```objc
// screen_char_t is 16 bytes per cell minimum
typedef struct screen_char_t {
    unichar code;                    // 2 bytes
    unsigned int foregroundColor : 8; // Complex bit packing...
    unsigned int fgGreen : 8;
    unsigned int fgBlue : 8;
    unsigned int backgroundColor : 8;
    unsigned int bgGreen : 8;
    unsigned int bgBlue : 8;
    unsigned int foregroundColorMode : 2;
    unsigned int backgroundColorMode : 2;
    unsigned int complexChar : 1;
    unsigned int bold : 1;
    // ... many more bits
    unsigned short urlCode;
} screen_char_t;
```

**d) Redundant Processing:**
- Parser creates token objects for every byte
- `VT100Token` has 40+ fields, most unused per token
- Double-width character handling is expensive

**Lessons for dterm:**
1. Use Rust's zero-cost abstractions instead of ObjC
2. Batch token processing - don't allocate per-character
3. Separate hot path (parsing) from cold path (triggers)
4. Consider SIMD for character processing

### 2. Memory Issues

**Problems:**
- `LineBuffer` grows unboundedly with scrollback
- Each cell stores URL code even when not a link
- IntervalTree allocates heavily for marks
- No memory-mapped scrollback option

**Evidence:**
```objc
// LineBuffer stores raw screen_char_t arrays
// 80 columns x 100,000 scrollback = 128MB minimum
// Plus metadata per line
typedef struct {
    NSUInteger totalBlocks;
    uint64_t currentBytes;
    uint64_t estimatedUnpackedBytes;
} LineBufferPackingStats;
```

**Lessons for dterm:**
1. Delta compression for scrollback (planned)
2. Memory-map large scrollback to disk
3. Lazy loading of off-screen content
4. Consider arena allocation for marks

### 3. Architectural Complexity

**God Objects:**
- `PTYSession.m`: 21,894 lines
- `PseudoTerminal.m`: 13,263 lines
- `Metal/iTermMetalDriver.m`: 121,126 lines (!!)
- `VT100Terminal.m`: 5,565 lines

**Delegate Explosion:**
- `VT100ScreenDelegate` - 100+ methods
- `VT100TerminalDelegate` - 80+ methods
- Circular delegate chains common

**Side Effect System:**
```objc
// Complex queuing to work around threading issues
- (void)addSideEffect:(void (^)(id<VT100ScreenDelegate> delegate))sideEffect name:(NSString *)name;
- (void)addUnmanagedPausedSideEffect:(void (^)(id<VT100ScreenDelegate> delegate, iTermTokenExecutorUnpauser *unpauser))block name:(NSString *)name;
```

**Lessons for dterm:**
1. Keep files under 1000 lines
2. Use composition over delegation
3. Clean separation: parser | state | renderer
4. No side effect queuing - proper threading instead

### 4. Technical Debt Patterns

**a) Mixed Memory Management:**
- Some files use ARC, others MRR (Manual Retain Release)
- `+MRR.m` files for MRR code
- Runtime switching between models

**b) Inconsistent Threading:**
```objc
// Mutation thread vs main thread
@property (class, atomic, readonly) BOOL performingJoinedBlock;
@property (atomic) BOOL performingSideEffect;
@property (atomic) BOOL performingPausedSideEffect;
```

**c) Doppelganger Pattern (Thread Safety Hack):**
```objc
// Creates parallel "doppelganger" objects for thread safety
- (id<VT100ScreenMarkReading>)doppelganger;
- (id<VT100ScreenMarkReading>)progenitor;
```

**d) Legacy Compatibility:**
- `legacy_screen_char_t` alongside `screen_char_t`
- Multiple parser paths for different terminal types
- Deprecated code paths still present

**Lessons for dterm:**
1. One memory management model (Rust ownership)
2. Explicit threading model from day 1
3. No parallel object graphs - use proper synchronization
4. Don't accumulate compatibility layers

### 5. Parser Design Issues

**Current Structure:**
- `VT100Parser` dispatches to 6+ sub-parsers
- `VT100Token` allocated per control sequence
- String table for complex characters (limits scalability)

```objc
// 40+ token types, each with different payload requirements
typedef enum {
    VT100_WAIT,
    VT100_NOTSUPPORT,
    VT100_SKIP,
    VT100_STRING,
    VT100_ASCIISTRING,
    // ... many more
} VT100TerminalTokenType;
```

**Lessons for dterm:**
1. State machine over token allocation
2. Zero-copy parsing where possible
3. Streaming rather than buffered
4. Avoid string table for common case (ASCII)

---

## Part 3: Architecture Recommendations for dterm

### Adopt From iTerm2:

| Feature | Priority | Complexity | Notes |
|---------|----------|------------|-------|
| OSC 133 Shell Integration | High | Medium | Essential for modern workflows |
| Trigger System | High | Medium | User-configurable automation |
| Tmux Control Mode | High | High | Native tmux integration |
| Smart Selection | Medium | Low | Better text detection |
| Semantic History | Medium | Medium | Cmd-click file opening |
| Captured Output | Medium | Low | Error aggregation |
| Session Restoration | High | High | Process survival |

### Avoid From iTerm2:

| Anti-Pattern | Alternative for dterm |
|--------------|----------------------|
| 20k+ line god objects | <1000 lines per module |
| Delegate explosion | Trait composition |
| Side effect queuing | Actor model |
| Doppelganger pattern | Arc<RwLock<T>> |
| Per-character allocation | Batch processing |
| Mixed ARC/MRR | Rust ownership |
| Runtime type switching | Compile-time generics |

### Data Structure Improvements:

```rust
// Instead of 16-byte screen_char_t:
struct Cell {
    code: u32,           // 4 bytes (Unicode scalar)
    style: StyleIndex,   // 2 bytes (index into style table)
    flags: CellFlags,    // 1 byte
}
// = 7 bytes vs 16+ bytes, plus deduplication

// Style table for attribute deduplication:
struct Style {
    fg: Color,
    bg: Color,
    attrs: Attributes,
}
```

### Performance Targets vs iTerm2:

| Metric | iTerm2 | dterm Target |
|--------|--------|--------------|
| Throughput | ~60 MB/s | 200+ MB/s |
| Input latency | ~10-15ms | <5ms |
| Memory (1M lines) | ~200MB | <100MB |
| Startup time | ~500ms | <100ms |

---

## Appendix: Key Source Files Reference

| Area | Files |
|------|-------|
| Terminal Emulation | `VT100Terminal.m`, `VT100Parser.m`, `VT100Screen*.m` |
| Grid/Buffer | `VT100Grid.m`, `LineBuffer.m`, `ScreenChar.m` |
| Shell Integration | `VT100ScreenMark.m`, `PromptStateMachine.swift` |
| Triggers | `Trigger.m`, `PTYTriggerEvaluator.m`, `*Trigger.m` |
| Tmux | `TmuxGateway.m`, `TmuxController.m` |
| Rendering | `Metal/iTermMetalDriver.m`, `Metal/Renderers/*` |
| Session | `PTYSession.m`, `PTYTask.m` |
| Profiles | `ProfileModel.m`, `iTermProfilePreferences.m` |
| Selection | `SmartSelectionController.m`, `iTermTextExtractor.m` |
