# DashTerm2 Integration Test Plan

**Created:** December 22, 2025
**Purpose:** Make DashTerm2 rock-solid with comprehensive integration testing

---

## Current State

### Existing Tests
- **Unit Tests (DashTerm2Tests):** 590 bug regression tests
- **UI Tests (DashTerm2UITests):** 7 smoke tests for launch/window/menu

### Gap Analysis
The current tests verify:
- ✅ App launches
- ✅ Windows/tabs can be created
- ✅ Menus exist

The current tests DO NOT verify:
- ❌ Terminal I/O actually works (typing commands, seeing output)
- ❌ PTY emulation is correct
- ❌ Shell integration functions
- ❌ Copy/paste in terminal
- ❌ Split panes work
- ❌ Scrollback buffer works
- ❌ ANSI escape sequences render correctly

---

## Test Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    INTEGRATION TESTS                         │
├─────────────────────────────────────────────────────────────┤
│  Level 1: Smoke Tests ✅ DONE                               │
│  └── App launches, windows work, menus exist                │
├─────────────────────────────────────────────────────────────┤
│  Level 2: Terminal I/O Tests 🔴 NEEDED                      │
│  ├── Can type text into terminal                            │
│  ├── Can execute shell command                              │
│  ├── Output appears correctly                               │
│  ├── Can use Ctrl+C to interrupt                            │
│  └── Can use Ctrl+D to send EOF                             │
├─────────────────────────────────────────────────────────────┤
│  Level 3: PTY Emulation Tests 🔴 NEEDED                     │
│  ├── ANSI color codes render correctly                      │
│  ├── Cursor movement (up, down, left, right)                │
│  ├── Line wrapping works                                    │
│  ├── Alternate screen buffer (vim, less)                    │
│  └── Unicode/emoji rendering                                │
├─────────────────────────────────────────────────────────────┤
│  Level 4: Feature Tests 🔴 NEEDED                           │
│  ├── Copy/paste operations                                  │
│  ├── Find in terminal                                       │
│  ├── Split panes (horizontal/vertical)                      │
│  ├── Scrollback buffer navigation                           │
│  └── Profile switching                                      │
├─────────────────────────────────────────────────────────────┤
│  Level 5: Shell Integration Tests 🔴 NEEDED                 │
│  ├── Command status markers                                 │
│  ├── Current directory tracking                             │
│  ├── Command history integration                            │
│  └── Prompt detection                                       │
└─────────────────────────────────────────────────────────────┘
```

---

## Implementation Strategy

### Approach 1: XCUITest with Accessibility (Recommended)

Use XCUITest to interact with the terminal via accessibility APIs.

```swift
class TerminalIOTests: XCTestCase {
    func test_canExecuteCommand() {
        let app = XCUIApplication()
        app.launch()

        // Wait for terminal to be ready
        let window = app.windows.firstMatch
        XCTAssertTrue(window.waitForExistence(timeout: 10))

        // Type a command
        app.typeText("echo 'INTEGRATION_TEST_MARKER'\n")

        // Wait for output
        sleep(1)

        // Verify output appears (via accessibility or screenshot comparison)
        // This is the tricky part - need to read terminal content
    }
}
```

**Challenge:** XCUITest can type into the terminal but can't easily read the terminal output because it's rendered as a custom view, not standard text elements.

### Approach 2: AppleScript Integration

Use AppleScript to control the app and verify state.

```applescript
tell application "DashTerm2"
    activate
    tell current session of current window
        write text "echo 'TEST_MARKER_12345'"
        delay 1
        set terminalContents to contents
        if terminalContents contains "TEST_MARKER_12345" then
            return "PASS"
        else
            return "FAIL"
        end if
    end tell
end tell
```

**Advantage:** AppleScript can read terminal contents directly via the app's scripting interface.

### Approach 3: PTY Test Harness (Unit Test Level)

Test the PTY/VT100 emulation at the code level, not through UI.

```swift
class PTYEmulationTests: XCTestCase {
    func test_ansiColorCodes() {
        // Create a mock terminal screen
        let screen = VT100Screen(width: 80, height: 24)

        // Feed ANSI escape sequence
        screen.process("\u{1b}[31mRed Text\u{1b}[0m")

        // Verify the character attributes
        XCTAssertEqual(screen.characterAt(0, 0).foregroundColor, .red)
    }
}
```

**Advantage:** Fast, deterministic, tests core terminal emulation logic.

### Approach 4: Hybrid (Recommended Final Strategy)

Combine approaches for comprehensive coverage:

| Test Type | Approach | What It Tests |
|-----------|----------|---------------|
| PTY Emulation | Unit tests | Escape sequences, rendering logic |
| Terminal I/O | AppleScript | Commands work, output appears |
| UI Workflows | XCUITest | Menus, tabs, windows |
| Visual Regression | Screenshot diff | UI looks correct |

---

## Implementation Phases

### Phase 1: AppleScript Terminal I/O Tests (Week 1)

Create AppleScript-based tests that:
1. Launch app
2. Execute commands
3. Verify output
4. Test basic workflows

**Deliverable:** `scripts/integration-tests/terminal_io_tests.applescript`

### Phase 2: PTY Unit Tests (Week 2)

Create unit tests for:
1. VT100 escape sequence handling
2. Character attribute parsing
3. Cursor movement
4. Scrolling behavior

**Deliverable:** `DashTerm2Tests/PTYEmulationTests.swift`

### Phase 3: Enhanced UI Tests (Week 3)

Expand XCUITest suite for:
1. Split pane operations
2. Copy/paste via menu
3. Find panel
4. Preferences window

**Deliverable:** Expanded `DashTerm2UITests/`

### Phase 4: Visual Regression (Week 4)

Implement screenshot comparison:
1. Capture baseline screenshots
2. Compare against known-good
3. Flag visual regressions

**Deliverable:** `scripts/visual-regression/`

---

## Test Execution

### CI/CD Integration

```yaml
# .github/workflows/integration-tests.yml
name: Integration Tests

on: [push, pull_request]

jobs:
  integration:
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v3

      - name: Build App
        run: xcodebuild build -scheme DashTerm2 -configuration Development

      - name: Run UI Tests
        run: xcodebuild test -scheme DashTerm2UITests -destination 'platform=macOS'

      - name: Run AppleScript Tests
        run: osascript scripts/integration-tests/run_all.applescript
```

### Local Execution

```bash
# Run all integration tests
./scripts/run-integration-tests.sh

# Run specific level
./scripts/run-integration-tests.sh --level 2  # Terminal I/O only
```

---

## Success Criteria

### Definition of "Perfect"

The system is "perfect" when:

1. **All 746 CRITICAL bugs** have fixes AND regression tests
2. **All regression tests pass** (0 failures)
3. **No placeholder tests** (0 `XCTAssertTrue(true, ...)`)
4. **Integration tests pass:**
   - App launches reliably
   - Terminal I/O works correctly
   - All PTY escape sequences handled
   - Copy/paste works
   - Split panes work
   - Shell integration works
5. **No crashes** in 24-hour stress test
6. **No interference** with iTerm2 running simultaneously

### Metrics to Track

| Metric | Current | Target |
|--------|---------|--------|
| Unit test count | 590 | 590+ |
| Placeholder tests | 63 | 0 |
| UI tests | 7 | 25+ |
| AppleScript tests | 0 | 10+ |
| PTY emulation tests | ? | 50+ |
| Test pass rate | ? | 100% |

---

## Next Steps

1. **Immediate:** Run existing UI tests to establish baseline
2. **This week:** Create AppleScript terminal I/O test suite
3. **Next week:** Add PTY emulation unit tests
4. **Ongoing:** Worker continues converting placeholders to real tests

---

## Appendix: Key Files

- UI Tests: `DashTerm2UITests/DashTerm2UITests.swift`
- Unit Tests: `DashTerm2Tests/BugRegressionTests.swift`
- PTY Code: `sources/VT100Screen*.m`, `sources/VT100Terminal.m`
- AppleScript API: `api/API.script`, `iTerm2.sdef`
