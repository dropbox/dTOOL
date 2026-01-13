# DashTerm2 Agentic Integration Test Plan

**Purpose:** Prove DashTerm2 is at least as good as iTerm2 for agentic coding workflows

**Primary Use Case:** `run_worker.sh` - autonomous AI coding agent running continuously

---

## Test Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    AGENTIC WORKFLOW TEST PYRAMID                            │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  Level 4: End-to-End Worker Test                                           │
│  └── Actually run worker for N iterations, verify commits                   │
│                                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  Level 3: Integration Tests (test_agentic_workflow.sh) ← PARAGON           │
│  ├── App launch, window creation                                            │
│  ├── Command execution, output capture                                      │
│  ├── Streaming output, pipe chains                                          │
│  ├── Long-running processes                                                 │
│  ├── ANSI colors, Unicode                                                   │
│  ├── Signal handling, exit codes                                            │
│  └── Full worker simulation                                                 │
│                                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  Level 2: Component Tests                                                   │
│  ├── PTY read/write                                                         │
│  ├── VT100 escape sequence parsing                                          │
│  ├── Screen buffer management                                               │
│  ├── Scrollback buffer                                                      │
│  └── Shell integration                                                      │
│                                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  Level 1: Unit Tests (existing BugRegressionTests) ← CURRENT               │
│  └── Pattern/fix verification tests                                         │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Critical Path: Agentic Workflow

Based on `run_worker.sh`, these are the operations that MUST work:

### 1. Process Execution
| Operation | Worker Usage | Test |
|-----------|--------------|------|
| Shell command execution | `claude --dangerously-skip-permissions -p "$PROMPT"` | `test_02_command_execution` |
| Pipe chains | `claude ... \| filter \| tee \| convert` | `test_04_pipe_chains` |
| Background processes | File watching, heartbeat | `test_10_signal_interrupt` |
| Exit code capture | `exit_code=${PIPESTATUS[0]}` | `test_09_exit_codes` |

### 2. I/O Handling
| Operation | Worker Usage | Test |
|-----------|--------------|------|
| Streaming stdout | JSON events from claude CLI | `test_03_streaming_output` |
| Long-running output | Worker iterations | `test_05_long_running_process` |
| Large output | 20GB+ codex logs | `test_stress_large_output` |
| Binary data | Occasional non-text in stream | `test_binary_data_handling` |

### 3. Terminal Features
| Feature | Worker Usage | Test |
|---------|--------------|------|
| ANSI colors | Status messages, errors | `test_06_ansi_colors` |
| Unicode | Checkmarks, emoji (✓ ✗ 📝) | `test_07_unicode` |
| Cursor movement | Progress indicators | `test_cursor_movement` |
| Line wrapping | Long lines in output | `test_line_wrapping` |

### 4. Session Management
| Feature | Worker Usage | Test |
|---------|--------------|------|
| Session persistence | Worker runs for hours/days | `test_session_persistence` |
| Reconnection | Resume after disconnect | `test_reconnection` |
| Multiple sessions | Monitoring alongside worker | `test_multiple_sessions` |

---

## Comprehensive Test Suite

### Phase 1: Core Agentic Workflow (PRIORITY)

```bash
tests/integration/
├── test_agentic_workflow.sh      # ← PARAGON (created)
├── test_worker_simulation.sh     # Full worker loop simulation
├── test_claude_cli_compat.sh     # Claude CLI specific patterns
└── test_pipe_stress.sh           # Heavy pipe chain testing
```

**Tests:**
1. **test_agentic_workflow.sh** (DONE)
   - 11 tests covering core workflow

2. **test_worker_simulation.sh** (TODO)
   - Run actual worker script for 3 iterations
   - Verify git commits created
   - Check log files generated
   - Verify heartbeat updates
   - Test crash recovery

3. **test_claude_cli_compat.sh** (TODO)
   - Stream JSON parsing
   - `--output-format stream-json` handling
   - Permission prompts (should not appear in headless)
   - Verbose mode output

4. **test_pipe_stress.sh** (TODO)
   - 10-pipe chain
   - Large data through pipes (100MB)
   - Rapid pipe creation/destruction

### Phase 2: Terminal Emulation

```bash
tests/integration/
├── test_pty_basic.sh             # PTY read/write
├── test_vt100_escapes.sh         # All escape sequences
├── test_screen_buffer.sh         # Screen management
└── test_scrollback.sh            # History buffer
```

**Tests:**
1. **test_pty_basic.sh**
   - Echo input
   - Raw mode
   - Cooked mode
   - Control characters (Ctrl+C, Ctrl+D, Ctrl+Z)

2. **test_vt100_escapes.sh**
   - Cursor movement (up, down, left, right)
   - Clear screen
   - Clear line
   - Colors (16, 256, 24-bit)
   - Bold, italic, underline
   - Alternate screen buffer

3. **test_screen_buffer.sh**
   - Resize handling
   - Wrap vs truncate
   - Tab stops

4. **test_scrollback.sh**
   - Scroll up/down
   - Search in history
   - Large history (100k lines)

### Phase 3: Robustness

```bash
tests/integration/
├── test_stress_output.sh         # Large output handling
├── test_rapid_input.sh           # Fast typing/paste
├── test_memory_stability.sh      # Long-running memory
└── test_concurrent_sessions.sh   # Many sessions
```

**Tests:**
1. **test_stress_output.sh**
   - 1GB output stream
   - 1M lines output
   - Binary data mixed with text
   - Rapid small outputs (10k/sec)

2. **test_rapid_input.sh**
   - Fast paste (1MB text)
   - Rapid keystrokes
   - Bracketed paste mode

3. **test_memory_stability.sh**
   - Run for 24 hours
   - Monitor memory usage
   - Check for leaks

4. **test_concurrent_sessions.sh**
   - 10 simultaneous sessions
   - All running commands
   - Split panes

### Phase 4: Comparison Tests

```bash
tests/integration/
├── test_vs_iterm2_basic.sh       # Feature parity
├── test_vs_iterm2_perf.sh        # Performance comparison
└── test_vs_iterm2_compat.sh      # Compatibility
```

**Tests:**
1. **test_vs_iterm2_basic.sh**
   - Run same commands in both
   - Compare output
   - Compare behavior

2. **test_vs_iterm2_perf.sh**
   - Latency: keystroke to echo
   - Throughput: lines per second
   - Memory: baseline and under load
   - CPU: idle and active

3. **test_vs_iterm2_compat.sh**
   - Shell integration
   - AppleScript API
   - URL handling
   - File handling

---

## Test Execution

### Quick Smoke Test (< 1 minute)
```bash
./tests/integration/test_agentic_workflow.sh
```

### Full Integration Suite (< 10 minutes)
```bash
./scripts/run-integration-tests.sh
```

### Comparison Suite (< 30 minutes)
```bash
./scripts/run-comparison-tests.sh
```

### Stress Test (24 hours)
```bash
./scripts/run-stress-tests.sh
```

---

## Success Criteria

### Minimum Viable (MVP)
- [ ] `test_agentic_workflow.sh` passes all 11 tests
- [ ] Worker can run 10 iterations without crash
- [ ] No memory leaks after 1 hour

### Production Ready
- [ ] All Phase 1-3 tests pass
- [ ] Performance within 10% of iTerm2
- [ ] 24-hour stress test passes
- [ ] Worker can run 100+ iterations

### Superior to iTerm2
- [ ] All tests pass
- [ ] Performance exceeds iTerm2
- [ ] Additional features working (AI, etc.)
- [ ] Worker runs indefinitely

---

## Implementation Priority

1. **NOW:** Run `test_agentic_workflow.sh` to establish baseline
2. **This Week:** Create `test_worker_simulation.sh`
3. **Next Week:** Phase 2 terminal emulation tests
4. **Ongoing:** Phase 3 robustness tests
5. **Milestone:** Phase 4 comparison tests

---

## Running the Paragon Test

```bash
# Build the app first
xcodebuild -project DashTerm2.xcodeproj -scheme DashTerm2 -configuration Development build CODE_SIGNING_ALLOWED=NO

# Run the paragon integration test
./tests/integration/test_agentic_workflow.sh
```

This will launch DashTerm2, run 11 tests, and report pass/fail for the core agentic workflow.
