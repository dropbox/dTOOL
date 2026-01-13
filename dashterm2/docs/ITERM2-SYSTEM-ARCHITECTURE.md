# iTerm2 Complete System Architecture

**Comprehensive analysis of every component, feature, and design decision**

---

## Executive Summary

| Metric | Value |
|--------|-------|
| **Total Lines of Code** | 567,000 |
| **Classes** | 858 |
| **Protocols** | 409 |
| **Source Files** | 1,320 |
| **Years of Development** | 20+ (since 2002) |

**Architecture Style:** Monolithic Cocoa application with MVC pattern

---

## Why iTerm2? (Terminal.app Comparison)

### The Problem with Terminal.app

Terminal.app ships with macOS and is... adequate. Apple maintains it as a utility, not a product. It receives minimal updates and lacks features that developers need daily.

### Feature Comparison

| Feature | Terminal.app | iTerm2 |
|---------|--------------|--------|
| **Split Panes** | ❌ No | ✅ Unlimited horizontal/vertical splits |
| **Hotkey Window** | ❌ No | ✅ System-wide hotkey summons terminal |
| **tmux Integration** | ❌ Text UI only | ✅ Native windows replace tmux UI |
| **Shell Integration** | ❌ No | ✅ Command tracking, marks, status |
| **Triggers** | ❌ No | ✅ 20+ actions on regex matches |
| **Search** | Basic | ✅ Regex, minimap, highlight all |
| **Profiles** | Limited | ✅ Unlimited, dynamic, auto-switching |
| **Python API** | ❌ No | ✅ Full automation |
| **GPU Rendering** | ❌ No | ✅ Metal acceleration |
| **Inline Images** | ❌ No | ✅ imgcat, Sixel, SVG |
| **Password Manager** | ❌ No | ✅ Keychain, 1Password, LastPass |
| **Semantic History** | ❌ No | ✅ Cmd-click opens files in editor |
| **Copy Mode** | ❌ No | ✅ Vim-style keyboard selection |
| **Instant Replay** | ❌ No | ✅ Scrub through terminal history |
| **Broadcast Input** | ❌ No | ✅ Type to multiple panes at once |
| **Annotations** | ❌ No | ✅ Add notes to output |
| **Coprocess** | ❌ No | ✅ Attach scripts to filter I/O |
| **Status Bar** | ❌ No | ✅ Git, CPU, memory, custom components |
| **Badge** | ❌ No | ✅ Watermark showing session info |
| **24-bit Color** | ✅ Yes | ✅ Yes |
| **Unicode** | Basic | ✅ Full Unicode 14, combining, emoji |
| **Touch Bar** | ❌ No | ✅ Custom buttons, function keys |
| **Session Restore** | Basic | ✅ Full state survives crashes |

### Why Developers Switch

**1. Split Panes**
Terminal.app forces you to use multiple windows or tabs. iTerm2 lets you split any pane infinitely:
```
┌─────────────┬─────────────┐
│   editor    │   server    │
│             ├─────────────┤
│             │    logs     │
└─────────────┴─────────────┘
```
One window, three contexts, all visible.

**2. Hotkey Window**
Press a global hotkey (e.g., `` ` ``) and iTerm2 slides in from any screen edge. Press again, it disappears. No Cmd-Tab, no hunting for windows. This alone converts people.

**3. tmux Integration**
tmux is essential for remote work - sessions persist if SSH drops. But tmux's text-based UI is clunky. iTerm2's `-CC` mode makes tmux windows appear as native tabs. You get tmux persistence with native UX.

**4. Shell Integration**
iTerm2 knows:
- When each command started/finished
- Whether it succeeded or failed (exit code)
- What directory you were in
- What host you're connected to

This enables: navigate between prompts, download files from remote via SCP, automatic profile switching per host.

**5. Triggers**
Regex patterns fire actions automatically:
- Highlight errors in red
- Send notification when build finishes
- Auto-type password when prompted
- Run script when pattern appears

**6. Semantic History**
Cmd-click on a filename opens it in your editor. Cmd-click on `src/foo.rs:42` opens line 42. Cmd-click on a URL opens the browser. Terminal.app has none of this.

**7. Profiles + Auto-Switching**
Different colors/fonts for different contexts:
- Production servers: red background (danger!)
- Development: dark theme
- Specific hosts: custom settings

Profiles switch automatically based on hostname, path, or username.

**8. Python API**
Full programmatic control:
```python
import iterm2
async with iterm2.Connection() as conn:
    app = await iterm2.async_get_app(conn)
    window = await app.current_window.async_create_tab()
    await window.current_session.async_send_text("ls\n")
```
Custom status bar components, triggers, automation - all in Python.

### Who Uses iTerm2

- **Developers** - The primary audience. Split panes, semantic history, editor integration.
- **DevOps/SRE** - tmux integration, triggers for alerts, broadcast input for multiple servers.
- **Data Scientists** - Inline images for plots, Python API for automation.
- **Security Researchers** - Logging, triggers, coprocesses for analysis.
- **Anyone who lives in the terminal** - If you spend hours daily in a terminal, the features compound.

### Why Terminal.app Still Exists

Terminal.app is:
- **Included** - Zero installation
- **Lightweight** - ~5MB vs iTerm2's ~30MB
- **Stable** - Apple tests it
- **Sufficient** - For occasional use, it works

Most users who try iTerm2 for a week don't go back. The productivity gains are too significant.

### The Gap DashTerm2 Fills

iTerm2 is great but has issues for 24/7 agent workloads:
- **Memory grows** - Scrollback is in-memory, unbounded
- **No global logging** - Must configure per-profile
- **Occasional crashes** - 3,348 open issues
- **macOS only** - Can't check agents from phone

DashTerm2 keeps everything that makes iTerm2 great and fixes what doesn't work for always-on terminals.

---

## Part 1: System Architecture

### 1.1 Application Lifecycle

```
┌──────────────────────────────────────────────────────────────────┐
│                         main.m                                    │
│                           │                                       │
│                           ▼                                       │
│                    NSApplicationMain()                            │
│                           │                                       │
│                           ▼                                       │
│  ┌────────────────────────────────────────────────────────────┐  │
│  │              iTermApplication (1,181 lines)                 │  │
│  │         Custom NSApplication subclass                       │  │
│  │  - Event routing (sendEvent:)                              │  │
│  │  - Hotkey handling                                          │  │
│  │  - Modal window tracking                                    │  │
│  │  - Touch Bar coordination                                   │  │
│  └────────────────────────────────────────────────────────────┘  │
│                           │                                       │
│                           ▼                                       │
│  ┌────────────────────────────────────────────────────────────┐  │
│  │         iTermApplicationDelegate (3,564 lines)              │  │
│  │              Main app delegate                              │  │
│  │  - Launch handling                                          │  │
│  │  - URL scheme handling (iterm2://)                         │  │
│  │  - Menu management                                          │  │
│  │  - Services provider                                        │  │
│  │  - State restoration                                        │  │
│  │  - AppleScript support                                      │  │
│  └────────────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────────┘
```

### 1.2 Core Object Hierarchy

```
iTermController (singleton)
    │
    ├── PseudoTerminal[] (windows) ─────── 13,263 lines
    │       │
    │       ├── PTYTab[] (tabs) ─────────── 7,237 lines
    │       │       │
    │       │       └── PTYSession[] ────── 21,894 lines (LARGEST CLASS)
    │       │               │
    │       │               ├── VT100Terminal (emulation)
    │       │               ├── VT100Screen (buffer)
    │       │               ├── PTYTextView (rendering)
    │       │               └── PTYTask (PTY I/O)
    │       │
    │       ├── iTermToolbeltView (toolbelt)
    │       └── PSMTabBarControl (tab bar)
    │
    └── ProfileModel (settings)
```

### 1.3 Main Classes by Size

| Class | Lines | Purpose |
|-------|-------|---------|
| **PTYSession** | 21,894 | Terminal session - the heart of iTerm2 |
| **PseudoTerminal** | 13,263 | Window controller |
| **VT100Terminal** | 8,500+ | Terminal emulation state machine |
| **PTYTextView** | 9,700 | Text rendering and input |
| **VT100Screen** | 4,000+ | Screen buffer management |
| **PTYTab** | 7,237 | Tab and split pane management |
| **ProfileModel** | 3,500 | Profile/settings storage |
| **iTermApplicationDelegate** | 3,564 | App lifecycle |
| **TmuxGateway** | 2,117 | tmux protocol |

---

## Part 2: Complete Feature Catalog

### 2.1 Terminal Emulation (39,000 lines)

The core terminal emulation handles VT100/xterm/ANSI escape sequences.

| Component | Lines | Description |
|-----------|-------|-------------|
| **VT100Terminal** | 8,500 | State machine, mode tracking |
| **VT100Screen** | 4,000 | Screen buffer, scrollback |
| **VT100Parser** | 2,000 | Escape sequence parsing |
| **VT100CSIParser** | 1,500 | CSI (Control Sequence Introducer) |
| **VT100DCSParser** | 800 | DCS (Device Control String) |
| **VT100AnsiParser** | 600 | ANSI sequences |
| **VT100XtermParser** | 1,200 | xterm extensions |
| **VT100OtherParser** | 500 | Misc sequences |
| **VT100StringParser** | 400 | String parameters |
| **VT100SixelParser** | 144 | Sixel graphics |
| **VT100Token** | 1,500 | Token representation |
| **VT100Grid** | 2,000 | Grid data structure |
| **VT100LineInfo** | 800 | Line metadata |
| **VT100Output** | 1,000 | Output encoding |
| **VT100GraphicRendition** | 1,215 | SGR (colors, styles) |

**Supported Standards:**
- VT100, VT102, VT220, VT320, VT420
- ANSI X3.64
- xterm (most extensions)
- iTerm2 proprietary extensions
- Sixel graphics
- 24-bit color (true color)
- Unicode 14.0

### 2.2 Session Management (35,234 lines)

| Component | Lines | Description |
|-----------|-------|-------------|
| **PTYSession** | 21,894 | Session lifecycle, I/O, state |
| **PTYSession+ARC** | 374 | ARC-compatible parts |
| **PTYSession+Scripting** | 572 | AppleScript support |
| **SessionView** | 2,000 | Session container view |
| **SessionTitleView** | 800 | Title bar |
| **iTermSessionFactory** | 1,200 | Session creation |
| **iTermSessionLauncher** | 900 | Process launching |
| **iTermBuriedSessions** | 324 | Hidden sessions |

**Session Features:**
- Process spawning (login shell, command)
- I/O multiplexing
- Encoding conversion (UTF-8, Latin-1, etc.)
- Logging
- Coprocess attachment
- tmux integration
- SSH integration (Conductor)
- Session restoration

### 2.3 Window/Tab Management (56,000+ lines)

| Component | Lines | Description |
|-----------|-------|-------------|
| **PseudoTerminal** | 13,263 | Window controller |
| **PseudoTerminal+TouchBar** | 731 | Touch Bar |
| **PseudoTerminal+WindowStyle** | 1,170 | Window styles |
| **PTYTab** | 7,237 | Tab management |
| **PTYWindow** | 2,000 | Custom NSWindow |
| **iTermRootTerminalView** | 1,500 | Root view |
| **PSMTabBarControl** | 5,000+ | Tab bar (third-party) |
| **WindowArrangements** | 635 | Save/restore layouts |

**Window Features:**
- Multiple window styles (normal, fullscreen, compact, borderless)
- Native tabs (macOS Sierra+)
- Traditional tab bar
- Split panes (horizontal/vertical)
- Zoom (maximize pane)
- Broadcast input
- Window arrangements (save/restore layouts)
- Per-window/tab/pane settings

### 2.4 Text Rendering (14,100 lines Metal + 9,700 PTYTextView)

| Component | Lines | Description |
|-----------|-------|-------------|
| **PTYTextView** | 7,237 | Main text view |
| **PTYTextView+ARC** | 2,466 | ARC parts |
| **Metal/*** | 14,100 | GPU rendering |
| **iTermTextDrawingHelper** | 3,000 | Text layout |
| **iTermCharacterSource** | 1,500 | Glyph generation |
| **CoreTextLineRenderingHelper** | 800 | CoreText integration |

**Rendering Features:**
- Metal GPU acceleration
- Legacy CoreGraphics fallback
- Ligature support
- Emoji rendering
- Powerline/Nerd Font glyphs
- Anti-aliasing options
- Thin strokes
- Retina support
- Background images
- Transparency/blur

### 2.5 tmux Integration (10,124 lines)

| Component | Lines | Description |
|-----------|-------|-------------|
| **TmuxGateway** | 2,117 | Protocol handler |
| **TmuxController** | 3,000 | Session coordination |
| **TmuxControllerRegistry** | 400 | Multiple tmux sessions |
| **TmuxDashboardController** | 1,200 | Dashboard UI |
| **TmuxLayoutParser** | 800 | Parse layout strings |
| **TmuxHistoryParser** | 600 | Parse history |
| **TmuxStateParser** | 500 | Parse state |
| **TmuxWindowOpener** | 700 | Window creation |
| **TmuxSessionsTable** | 400 | Sessions list |
| **TmuxWindowsTable** | 400 | Windows list |

**tmux Features:**
- Control mode (-CC)
- Native windows/tabs (not text UI)
- Automatic layout sync
- Session persistence
- Dashboard for management
- Multiple tmux servers
- Detach/reattach

### 2.6 Shell Integration (4,050 lines)

| Component | Lines | Description |
|-----------|-------|-------------|
| **iTermShellHistoryController** | 1,200 | Command history |
| **VT100RemoteHost** | 600 | Host tracking |
| **VT100WorkingDirectory** | 500 | CWD tracking |
| **VT100ScreenMark** | 800 | Prompt marks |
| **iTermMark** | 500 | Mark types |
| **iTermCommandHistoryCommandUseMO** | 450 | Core Data model |

**Shell Integration Features:**
- Automatic command tracking
- Per-host command history
- Directory tracking (frecency)
- Prompt marks (navigate between prompts)
- Command status (success/failure)
- Recent directories menu
- Automatic profile switching
- Upload/download with drag-drop

### 2.7 Search (10,256 lines)

| Component | Lines | Description |
|-----------|-------|-------------|
| **iTermFindDriver** | 2,000 | Search coordination |
| **iTermSearchEngine** | 1,500 | Search algorithm |
| **iTermFindOnPageHelper** | 1,200 | Find UI helper |
| **FindViewController** | 1,000 | Find bar UI |
| **iTermSearchResultsMinimapView** | 800 | Results minimap |
| **SearchResult** | 400 | Result model |

**Search Features:**
- Find in scrollback
- Regex support
- Case sensitivity toggle
- Results highlighting
- Minimap of results
- Find all (highlight all)
- Search history

### 2.8 Profiles (19,639 lines)

| Component | Lines | Description |
|-----------|-------|-------------|
| **ProfileModel** | 3,500 | Data model |
| **ProfileListView** | 2,000 | Profile list UI |
| **ProfilesWindowController** | 1,500 | Profiles window |
| **ProfilePreferencesViewController** | 2,500 | Edit profile |
| **ProfilesGeneralPreferencesViewController** | 1,200 | General tab |
| **ProfilesColorsPreferencesViewController** | 1,000 | Colors tab |
| **ProfilesTextPreferencesViewController** | 800 | Text tab |
| **ProfilesWindowPreferencesViewController** | 700 | Window tab |
| **ProfilesTerminalPreferencesViewController** | 600 | Terminal tab |
| **ProfilesSessionPreferencesViewController** | 600 | Session tab |
| **ProfilesKeysPreferencesViewController** | 500 | Keys tab |
| **ProfilesAdvancedPreferencesViewController** | 500 | Advanced tab |

**Profile Features:**
- Unlimited profiles
- Dynamic profiles (JSON files)
- Automatic profile switching (host, path, username)
- Import/export
- Profile search
- Default profile
- Profile tags
- Copy profile settings

### 2.9 Triggers (7,845 lines)

| Component | Lines | Description |
|-----------|-------|-------------|
| **Trigger** | 1,500 | Base class |
| **TriggerController** | 1,000 | Trigger management |
| **AlertTrigger** | 300 | Show alert |
| **BellTrigger** | 200 | Ring bell |
| **BounceTrigger** | 200 | Bounce dock icon |
| **AnnotateTrigger** | 300 | Add annotation |
| **CaptureTrigger** | 400 | Capture to toolbelt |
| **CoprocessTrigger** | 300 | Run coprocess |
| **HighlightTrigger** | 400 | Highlight text |
| **MarkTrigger** | 300 | Add mark |
| **MuteCoprocessTrigger** | 200 | Mute coprocess |
| **PasswordTrigger** | 400 | Open password manager |
| **RPCTrigger** | 300 | Call Python API |
| **SendTextTrigger** | 400 | Send text |
| **ScriptTrigger** | 300 | Run script |
| **SetDirectoryTrigger** | 200 | Set directory |
| **SetHostnameTrigger** | 200 | Set hostname |
| **SetTitleTrigger** | 300 | Set title |
| **ShellPromptTrigger** | 300 | Mark prompt |
| **StopTrigger** | 200 | Stop processing |
| **UserNotificationTrigger** | 300 | Send notification |

**Trigger Features:**
- Regex pattern matching
- 20+ action types
- Instant/background modes
- Capture groups (\1, \2, etc.)
- Conditional actions
- Priority ordering

### 2.10 Hotkey Windows (4,337 lines)

| Component | Lines | Description |
|-----------|-------|-------------|
| **iTermHotKeyController** | 1,500 | Hotkey management |
| **iTermProfileHotKey** | 800 | Profile-specific hotkeys |
| **iTermHotKeyWindowController** | 700 | Window animation |
| **iTermAppHotKeyProvider** | 500 | App-wide hotkeys |
| **iTermHotKeyProfileBindingController** | 400 | Binding management |
| **SBSystemPreferences** | 200 | System prefs bridge |

**Hotkey Features:**
- System-wide hotkeys
- Dedicated hotkey windows
- Slide in from any edge
- Pin to stay visible
- Multiple hotkey profiles
- Double-tap modifier
- Floating above other windows

### 2.11 Python API (10,172 lines)

| Component | Lines | Description |
|-----------|-------|-------------|
| **iTermAPIServer** | 2,000 | WebSocket server |
| **iTermAPIHelper** | 1,500 | API implementation |
| **iTermPythonRuntimeDownloader** | 1,200 | Runtime management |
| **iTermScriptConsole** | 800 | Script console UI |
| **iTermScriptHistory** | 600 | Script history |
| **iTermScriptArchive** | 500 | Script packaging |
| **iTermScriptImporter** | 400 | Import scripts |
| **iTermScriptExporter** | 400 | Export scripts |
| **APIScriptLauncher** | 300 | Script launching |
| **iTermBuiltInFunctions** | 1,500 | Built-in functions |

**API Features:**
- Full terminal control
- Custom status bar components
- Custom triggers
- Notifications/hooks
- Session/window/tab creation
- Profile manipulation
- Variable interpolation
- Async/await support

### 2.12 AI/Chat Integration (12,493 lines)

| Component | Lines | Description |
|-----------|-------|-------------|
| **ChatViewController** | 2,000 | Main chat UI |
| **ChatListViewController** | 1,500 | Conversation list |
| **ChatWindowController** | 1,000 | Chat window |
| **ChatService** | 1,200 | LLM backends |
| **ChatAgent** | 800 | Agent actions |
| **AITermController** | 1,000 | AI coordination |
| **CompletionsAnthropic** | 600 | Anthropic backend |
| **CompletionsOpenAI** | 600 | OpenAI backend |
| **CommandExplainer** | 500 | Explain commands |

**AI Features:**
- Chat sidebar
- Multiple LLM providers
- Terminal context awareness
- Command suggestions
- Output explanations
- Annotations from AI

### 2.13 Status Bar (16,436 lines)

| Component | Lines | Description |
|-----------|-------|-------------|
| **iTermStatusBarViewController** | 2,000 | Status bar UI |
| **iTermStatusBarLayout** | 1,500 | Layout management |
| **iTermStatusBarSetupViewController** | 1,200 | Configuration UI |
| **iTermStatusBar*Component** | 10,000+ | Various components |

**Status Bar Components:**
- Git branch/status
- Current directory
- Hostname
- Username
- CPU usage graph
- Memory usage graph
- Network throughput
- Battery
- Clock
- Search field
- Custom (Python API)
- Interpolated strings

### 2.14 Password Manager (5,924 lines)

| Component | Lines | Description |
|-----------|-------|-------------|
| **iTermPasswordManagerWindowController** | 2,000 | Main UI |
| **SSKeychain** | 1,500 | Keychain wrapper |
| **CommandLinePasswordDataSource** | 800 | CLI tool integration |
| **AdapterPasswordDataSource** | 600 | Password adapters |

**Password Features:**
- Keychain integration
- 1Password integration
- LastPass integration
- Auto-fill passwords
- Account management
- Security code handling

### 2.15 Semantic History (1,850 lines)

| Component | Lines | Description |
|-----------|-------|-------------|
| **iTermSemanticHistoryController** | 1,200 | Click handling |
| **iTermSemanticHistoryPrefsController** | 650 | Settings UI |

**Semantic History Features:**
- Cmd-click to open files
- Open in editor (VSCode, Sublime, etc.)
- Navigate to line number
- URL opening
- Custom handlers

### 2.16 Toolbelt (7,269 lines)

| Component | Lines | Description |
|-----------|-------|-------------|
| **iTermToolbeltView** | 1,059 | Container view |
| **ToolCapturedOutputView** | 800 | Captured output |
| **ToolCommandHistoryView** | 700 | Command history |
| **ToolDirectoriesView** | 600 | Recent directories |
| **ToolJobs** | 400 | Running jobs |
| **ToolNotes** | 500 | Session notes |
| **ToolPasteHistory** | 600 | Paste history |
| **ToolProfiles** | 500 | Quick profiles |
| **ToolWebView** | 700 | Web browser |
| **ToolCodecierge** | 800 | Code assistant |
| **ToolNamedMarks** | 600 | Named marks |

**Toolbelt Features:**
- Collapsible sidebar
- Multiple tools
- Drag to reorder
- Per-profile settings
- Custom tools (Python API)

### 2.17 Conductor/SSH (5,056 lines)

| Component | Lines | Description |
|-----------|-------|-------------|
| **Conductor** | 2,000 | SSH integration |
| **ConductorFileTransfer** | 1,200 | SFTP operations |
| **ConductorPayloadBuilder** | 800 | Command building |
| **ConductorRecovery** | 600 | Connection recovery |
| **ConductorRegistry** | 456 | Session registry |

**SSH Features:**
- Automatic shell integration over SSH
- File upload/download
- Connection persistence
- Automatic reconnection
- Integration triggers

### 2.18 File Transfer (2,011 lines)

| Component | Lines | Description |
|-----------|-------|-------------|
| **FileTransferManager** | 600 | Transfer coordination |
| **TransferrableFile** | 500 | File model |
| **DownloadHandler** | 400 | Download handling |
| **UploadHandler** | 400 | Upload handling |

**Transfer Features:**
- Drag-drop upload
- SCP download via shell integration
- Progress tracking
- Transfer history
- Automatic naming

### 2.19 Snippets (2,865 lines)

| Component | Lines | Description |
|-----------|-------|-------------|
| **iTermSnippetsModel** | 800 | Data model |
| **iTermSnippetsWindowController** | 700 | Management UI |
| **iTermSnippetsMenuBuilder** | 600 | Menu integration |
| **iTermSnippetInputViewController** | 500 | Input prompts |

**Snippet Features:**
- Text snippets
- Variable interpolation
- Input prompts
- Folder organization
- Import/export

### 2.20 Instant Replay (413 lines)

| Component | Lines | Description |
|-----------|-------|-------------|
| **iTermInstantReplayWindowController** | 413 | Replay UI |

**Instant Replay Features:**
- Scrub through history
- Timestamp display
- Frame-by-frame
- Export current view

### 2.21 Inline Images (5,905 lines)

| Component | Lines | Description |
|-----------|-------|-------------|
| **VT100InlineImageHelper** | 1,500 | Image parsing |
| **iTermImageMark** | 800 | Image in buffer |
| **iTermImageView** | 700 | Image rendering |
| **iTermImage** | 600 | Image model |
| **iTermGIFImage** | 500 | Animated GIF |
| **iTermSVGImage** | 400 | SVG support |

**Image Features:**
- Inline image protocol
- Animated GIF
- SVG
- Auto-scaling
- Click to preview
- imgcat integration

### 2.22 Copy Mode (1,500 lines)

| Component | Lines | Description |
|-----------|-------|-------------|
| **iTermCopyModeHandler** | 800 | Mode handling |
| **iTermCopyModeState** | 400 | State machine |
| **VT100CopyMode** | 300 | Integration |

**Copy Mode Features:**
- Vim-style selection
- Word/line/block selection
- Search within
- Mark navigation
- Keyboard-only operation

### 2.23 Annotations (267 lines)

| Component | Lines | Description |
|-----------|-------|-------------|
| **Annotation** | 267 | Annotation model |

**Annotation Features:**
- Add notes to output
- Persistent across sessions
- AI-generated annotations
- Export annotations

### 2.24 Broadcast Input (602 lines)

| Component | Lines | Description |
|-----------|-------|-------------|
| **BroadcastInputHelper** | 602 | Input broadcasting |

**Broadcast Features:**
- Send to all panes in tab
- Send to all panes in window
- Send to all sessions
- Toggle per-session

### 2.25 Coprocess (557 lines)

| Component | Lines | Description |
|-----------|-------|-------------|
| **Coprocess** | 557 | Coprocess management |

**Coprocess Features:**
- Attach script to session
- Bidirectional I/O
- Filter output
- Inject input

### 2.26 AppleScript (1,500+ lines)

Scattered across multiple files with +Scripting extensions.

**AppleScript Features:**
- Create windows/tabs/sessions
- Send text
- Read buffer
- Get/set properties
- Write documents
- Profile manipulation

### 2.27 Touch Bar (895 lines)

| Component | Lines | Description |
|-----------|-------|-------------|
| **PseudoTerminal+TouchBar** | 731 | Touch Bar integration |
| **iTermTouchBarButton** | 164 | Custom buttons |

**Touch Bar Features:**
- Function keys
- Color presets
- Custom buttons
- Status info
- Man page lookup

### 2.28 Unicode Support (20,456 lines)

| Component | Lines | Description |
|-----------|-------|-------------|
| **iTermUnicodeNormalization** | 1,500 | Normalization |
| **iTermUnicodeVersion** | 800 | Version handling |
| **iTermCharacterSource** | 2,000 | Glyph handling |
| **ComplexCharRegistry** | 1,200 | Combining characters |
| **charmaps** | 15,000 | Character mappings |

**Unicode Features:**
- Full Unicode 14.0
- Combining characters
- Emoji (including sequences)
- Right-to-left (partial)
- Ambiguous width handling
- Normalization forms

### 2.29 Accessibility (938 lines)

| Component | Lines | Description |
|-----------|-------|-------------|
| **PTYTextViewAccessibility** | 938 | Accessibility |

**Accessibility Features:**
- VoiceOver support
- Screen reader output
- Accessibility labels
- Focus tracking

### 2.30 Preferences System (25,055 lines)

| Component | Lines | Description |
|-----------|-------|-------------|
| **PreferencePanel** | 3,000 | Main prefs window |
| **iTermPreferences** | 2,500 | Preferences model |
| **iTermAdvancedSettingsModel** | 2,000 | Hidden settings |
| **GeneralPreferencesViewController** | 1,500 | General tab |
| **AppearancePreferencesViewController** | 1,200 | Appearance tab |
| **KeysPreferencesViewController** | 1,000 | Keys tab |
| **PointerPreferencesViewController** | 800 | Pointer tab |
| **iTermUserDefaults** | 600 | Defaults wrapper |

**300+ preferences** covering every aspect of the terminal.

---

## Part 3: How The System Works

### 3.1 Data Flow: Input to Output

```
┌─────────────┐    ┌─────────────┐    ┌─────────────┐
│  Keyboard   │───▶│ PTYTextView │───▶│  PTYTask    │
│   Input     │    │  (events)   │    │  (write)    │
└─────────────┘    └─────────────┘    └──────┬──────┘
                                              │
                                              ▼
┌─────────────┐    ┌─────────────┐    ┌─────────────┐
│  PTYTextView│◀───│ VT100Screen │◀───│ VT100Terminal│
│  (render)   │    │  (buffer)   │    │  (parse)    │
└─────────────┘    └─────────────┘    └──────┬──────┘
                                              │
                                              ▼
                                       ┌─────────────┐
                                       │  PTYTask    │
                                       │  (read)     │
                                       └─────────────┘
```

### 3.2 Key Press Processing

```
1. NSEvent received by NSApplication
2. iTermApplication.sendEvent: intercepts
3. Route to PTYTextView
4. PTYTextView.keyDown:
   ├─ Check key bindings
   ├─ Check hotkeys
   ├─ Check triggers
   └─ Send to session
5. PTYSession.writeTask:
6. PTYTask.write: → PTY → Shell process
```

### 3.3 Output Processing

```
1. Shell produces output
2. PTYTask reads from PTY (background thread)
3. Data queued for main thread
4. PTYSession.threadedReadTask: processes queue
5. VT100Terminal.execute: parses escape sequences
6. VT100Screen updates buffer
7. PTYTextView.setNeedsDisplay
8. Metal/CoreGraphics renders
```

### 3.4 Session State Machine

```
                    ┌─────────┐
                    │  Init   │
                    └────┬────┘
                         │ launch
                         ▼
┌─────────┐        ┌─────────┐        ┌─────────┐
│ Buried  │◀──────▶│ Running │───────▶│ Ended   │
└─────────┘ bury   └────┬────┘  exit  └────┬────┘
            unbury      │                   │
                        │ close             │ close
                        ▼                   ▼
                   ┌─────────┐        ┌─────────┐
                   │ Killing │───────▶│ Closed  │
                   └─────────┘        └─────────┘
```

---

## Part 4: macOS-Specific vs Portable

### 4.1 macOS-Specific (Cannot Port)

| Component | API | Why macOS-Only |
|-----------|-----|----------------|
| **Windows/Views** | AppKit | NSWindow, NSView |
| **Metal Rendering** | Metal | Apple GPU API |
| **Touch Bar** | NSTouchBar | MacBook Pro only |
| **Hotkey Registration** | Carbon | CGEventTap |
| **Keychain** | Security.framework | macOS Keychain |
| **Services Menu** | NSServices | macOS Services |
| **Dock Integration** | NSDockTile | macOS Dock |
| **AppleScript** | NSAppleScript | macOS scripting |
| **Accessibility** | NSAccessibility | macOS a11y |
| **Notifications** | NSUserNotification | macOS NC |
| **Full Screen** | NSWindow | macOS full screen |
| **Spaces** | NSWindow | macOS Spaces |
| **Pasteboard** | NSPasteboard | macOS clipboard |
| **Font Panel** | NSFontPanel | macOS fonts |

### 4.2 Portable (Could Reuse)

| Component | Why Portable |
|-----------|--------------|
| **VT100 Parsing** | Pure logic, no UI |
| **Screen Buffer** | Data structure |
| **tmux Protocol** | Text protocol |
| **Shell Integration** | Escape sequences |
| **Triggers** | Regex + actions |
| **Profiles** | Data/JSON |
| **Search** | Algorithm |
| **Unicode** | Character data |
| **API Protocol** | WebSocket/JSON |

### 4.3 Partially Portable

| Component | Portable Part | Platform Part |
|-----------|---------------|---------------|
| **PTY** | Interface | posix_openpt (Unix) / ConPTY (Win) |
| **File Transfer** | Protocol | File dialogs |
| **Preferences** | Model | NSUserDefaults |
| **Password** | Model | Keychain |

---

## Part 5: Design Analysis

### 5.1 Advantages of Current Design

| Advantage | Description |
|-----------|-------------|
| **Mature** | 20+ years of development, battle-tested |
| **Feature Complete** | Every terminal feature imaginable |
| **Native Feel** | True macOS citizen, follows HIG |
| **Performance** | Metal rendering, optimized |
| **Extensible** | Python API, triggers, coprocesses |
| **Well Documented** | Extensive user documentation |

### 5.2 Disadvantages of Current Design

| Disadvantage | Description |
|--------------|-------------|
| **Monolithic** | 567K lines, tightly coupled |
| **macOS Only** | AppKit everywhere, no abstraction |
| **Memory Model** | In-memory scrollback, unbounded growth |
| **Language Mix** | ObjC + Swift + ObjC++, inconsistent |
| **Technical Debt** | 20 years of accumulated cruft |
| **Testing** | Limited test coverage |
| **No Core Abstraction** | VT100 mixed with UI |

### 5.3 Coupling Analysis

```
HIGH COUPLING (hard to separate):
┌──────────────────────────────────────────────┐
│ PTYSession ←──────→ VT100Terminal            │
│      ↑                    ↑                  │
│      │                    │                  │
│      ▼                    ▼                  │
│ PTYTextView ←──────→ VT100Screen             │
└──────────────────────────────────────────────┘

LOW COUPLING (easy to separate):
┌──────────────┐  ┌──────────────┐  ┌──────────────┐
│   Triggers   │  │   Profiles   │  │  Toolbelt    │
└──────────────┘  └──────────────┘  └──────────────┘
```

### 5.4 State Management

**Global Singletons:**
- iTermController
- ProfileModel
- iTermPreferences
- iTermAPIServer
- TmuxControllerRegistry
- iTermHotKeyController

**Problems:**
- Hard to test
- Implicit dependencies
- Race conditions possible

---

## Part 6: Migration Path to Rust Core

### 6.1 What Rust Core Replaces

```
┌─────────────────────────────────────────────────────────┐
│                    RUST CORE                            │
│                                                         │
│  ┌───────────────────────────────────────────────────┐ │
│  │  VT100 Parser (vte crate or custom)               │ │
│  │  - Escape sequence parsing                        │ │
│  │  - State machine                                  │ │
│  └───────────────────────────────────────────────────┘ │
│                                                         │
│  ┌───────────────────────────────────────────────────┐ │
│  │  Screen Buffer                                    │ │
│  │  - Ring buffer for lines                          │ │
│  │  - Disk-backed scrollback                         │ │
│  │  - Selection tracking                             │ │
│  │  - Dirty region tracking                          │ │
│  └───────────────────────────────────────────────────┘ │
│                                                         │
│  ┌───────────────────────────────────────────────────┐ │
│  │  Search Engine                                    │ │
│  │  - Full-text index                                │ │
│  │  - Regex matching                                 │ │
│  │  - Incremental updates                            │ │
│  └───────────────────────────────────────────────────┘ │
│                                                         │
│  ┌───────────────────────────────────────────────────┐ │
│  │  Logging                                          │ │
│  │  - Compression (lz4/zstd)                         │ │
│  │  - Rotation                                       │ │
│  │  - Async I/O                                      │ │
│  └───────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────┘

Lines replaced: ~60,000 (VT100* + Screen* + Search*)
Lines kept: ~500,000 (everything else)
```

### 6.2 FFI Interface

```rust
// Rust side
#[repr(C)]
pub struct TerminalCore {
    // opaque
}

#[no_mangle]
pub extern "C" fn terminal_new() -> *mut TerminalCore;

#[no_mangle]
pub extern "C" fn terminal_write(
    term: *mut TerminalCore,
    data: *const u8,
    len: usize
);

#[no_mangle]
pub extern "C" fn terminal_get_line(
    term: *mut TerminalCore,
    line: i64,
    buffer: *mut u8,
    buffer_len: usize
) -> usize;

#[no_mangle]
pub extern "C" fn terminal_search(
    term: *mut TerminalCore,
    pattern: *const c_char,
    callback: extern "C" fn(line: i64, start: i32, end: i32)
);
```

```objc
// ObjC side
@interface DashTermCore : NSObject
- (void)write:(NSData *)data;
- (NSString *)lineAtIndex:(NSInteger)index;
- (void)searchForPattern:(NSString *)pattern
                callback:(void(^)(NSInteger line, NSRange range))callback;
@end
```

### 6.3 Migration Steps

1. **Define Interface** - C FFI between Rust and ObjC
2. **Build Rust Core** - VT100, buffer, search
3. **Create ObjC Wrapper** - DashTermCore class
4. **Replace VT100Terminal** - Calls Rust instead
5. **Replace VT100Screen** - Uses Rust buffer
6. **Validate** - Pass all tests
7. **Optimize** - Profile and tune

---

## Part 7: Recommendations

### 7.1 For DashTerm2

1. **Keep iTerm2 UI/Features** - 500K lines of value
2. **Replace Core with Rust** - 60K lines, huge benefit
3. **Add Disk-Backed Scrollback** - In Rust core
4. **Add Global Logging** - In Rust core
5. **Remove Cruft** - AI chat, browser (~15K lines)

### 7.2 Effort Estimate

| Phase | Effort | Result |
|-------|--------|--------|
| Rust core (VT100 + buffer) | 3-6 months | Cross-platform foundation |
| FFI integration | 1-2 months | Rust ↔ ObjC working |
| Feature parity | 2-3 months | Matches iTerm2 behavior |
| Disk-backed scrollback | 1-2 months | Memory efficiency |
| iOS app | 3-6 months | SwiftUI on Rust core |

**Total: 10-19 months to full cross-platform**

---

## Appendix: File Count by Subsystem

| Subsystem | Lines |
|-----------|-------|
| Session | 35,234 |
| Tab/Pane | 32,578 |
| Terminal | 30,942 |
| Window | 23,905 |
| Unicode | 20,456 |
| Profiles | 19,639 |
| Status Bar | 16,436 |
| Metal Rendering | 14,100 |
| AI/Chat | 12,493 |
| tmux | 10,124 |
| Search | 10,256 |
| API | 10,172 |
| Pane | 8,475 |
| Triggers | 7,845 |
| Command | 7,736 |
| Menu | 7,481 |
| Script | 7,579 |
| Toolbelt | 7,269 |
| Image | 5,905 |
| History | 5,914 |
| Password | 5,924 |
| Conductor | 5,056 |
| Hotkey | 4,337 |
| Shell | 4,050 |
| Open | 4,671 |
| Server | 4,410 |
| Client | 3,864 |
| Compose | 3,617 |
| URL | 3,158 |
| Snippet | 2,865 |
| Mouse | 2,714 |
| Log | 2,432 |
| Gateway | 2,117 |
| Download | 2,011 |
| Font | 1,967 |
| Popup | 1,902 |
| Porthole | 1,880 |
| Semantic | 1,850 |
| Save | 1,709 |
| Cursor | 1,634 |
| Scroll | 1,448 |
| Import | 1,383 |
| Socket | 1,351 |
| Graphic | 1,215 |
| Export | 1,194 |
| Split | 1,091 |
| Notification | 1,082 |
| Toolbelt | 1,059 |
| Smart | 1,056 |
| Keyboard | 938 |
| Touch | 895 |
| Protocol | 893 |
| Badge | 782 |
| Service | 666 |
| Alert | 646 |
| Arrangement | 635 |
| Broadcast | 602 |
| Coprocess | 557 |
| Restore | 470 |
| Instant | 413 |
| Buried | 324 |
| Annot | 267 |
| Notes | 197 |
| Sixel | 144 |
| Jobs | 121 |
| Animation | 77 |
| Bookmark | 54 |
