# High-Performance Terminal-AI Interface Design

## Goal
Sub-millisecond latency for AI to read terminal state.
Real-time awareness of terminal changes.

---

## Architecture: Hybrid MCP + Shared Memory

```
┌─────────────────────────────────────────────────────────────────┐
│                        DashTerm2 Process                         │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────────────┐  │
│  │ dterm-core  │───►│ Shared Mem  │◄───│ MCP Server Thread   │  │
│  │  Terminal   │    │   Buffer    │    │  (for control ops)  │  │
│  └─────────────┘    └──────┬──────┘    └─────────┬───────────┘  │
│         │                  │                     │              │
│         ▼                  │                     │              │
│  ┌─────────────┐           │                     │              │
│  │ Mach Port   │───────────┼─────────────────────┘              │
│  │ Notifier    │           │                                    │
│  └─────────────┘           │                                    │
└────────────────────────────┼────────────────────────────────────┘
                             │
        ┌────────────────────┴────────────────────┐
        │           Shared Memory Region          │
        │  ┌────────────────────────────────────┐ │
        │  │ Header (64 bytes)                  │ │
        │  │  - version: u32                    │ │
        │  │  - rows: u16, cols: u16            │ │
        │  │  - cursor_row: u16, cursor_col: u16│ │
        │  │  - sequence_num: u64 (monotonic)   │ │
        │  │  - dirty_flags: u64                │ │
        │  └────────────────────────────────────┘ │
        │  ┌────────────────────────────────────┐ │
        │  │ Grid (rows * cols * 8 bytes)       │ │
        │  │  - Direct copy of dterm-core cells │ │
        │  │  - 24x80 = 15KB, 50x200 = 80KB     │ │
        │  └────────────────────────────────────┘ │
        │  ┌────────────────────────────────────┐ │
        │  │ Command History Ring Buffer        │ │
        │  │  - Last N commands + outputs       │ │
        │  │  - Semantic blocks from OSC 133    │ │
        │  └────────────────────────────────────┘ │
        └─────────────────────────────────────────┘
                             │
        ┌────────────────────┴────────────────────┐
        │              AI Client Process          │
        │  ┌─────────────────────────────────────┐│
        │  │ Memory-Mapped Reader               ││
        │  │  - mmap() the shared region        ││
        │  │  - Read without syscall            ││
        │  │  - Check sequence_num for changes  ││
        │  └─────────────────────────────────────┘│
        │  ┌─────────────────────────────────────┐│
        │  │ Mach Port Listener                 ││
        │  │  - Receives change notifications   ││
        │  │  - Wake on terminal update         ││
        │  └─────────────────────────────────────┘│
        │  ┌─────────────────────────────────────┐│
        │  │ MCP Adapter (for Claude Code)      ││
        │  │  - Translates MCP calls to memreads││
        │  │  - JSON response from memory       ││
        │  └─────────────────────────────────────┘│
        └─────────────────────────────────────────┘
```

---

## Performance Comparison

| Operation | Standard MCP | Shared Memory | Improvement |
|-----------|--------------|---------------|-------------|
| Read screen | 2-5ms | **<0.1ms** | 20-50x |
| Detect change | Poll 100ms | **<0.01ms** (notification) | 10000x |
| Get cursor pos | 1-2ms | **<0.001ms** | 1000x |
| Command history | 5-10ms | **<0.5ms** | 10-20x |

---

## Implementation

### 1. Shared Memory Setup (DashTerm2 side)

```swift
// DTermSharedMemory.swift
import Darwin

class DTermSharedMemory {
    static let regionName = "com.dashterm.terminal.shm"
    static let regionSize = 1024 * 1024  // 1MB

    private var shmFd: Int32 = -1
    private var buffer: UnsafeMutableRawPointer?

    struct Header {
        var version: UInt32
        var rows: UInt16
        var cols: UInt16
        var cursorRow: UInt16
        var cursorCol: UInt16
        var sequenceNum: UInt64
        var dirtyFlags: UInt64
        var reserved: (UInt64, UInt64, UInt64, UInt64, UInt64)
    }

    func create() throws {
        // Create shared memory
        shmFd = shm_open(Self.regionName, O_CREAT | O_RDWR, 0o644)
        guard shmFd >= 0 else { throw POSIXError(.ENOENT) }

        // Set size
        ftruncate(shmFd, off_t(Self.regionSize))

        // Map into address space
        buffer = mmap(nil, Self.regionSize,
                      PROT_READ | PROT_WRITE,
                      MAP_SHARED, shmFd, 0)
    }

    func updateGrid(from terminal: DTermCore) {
        guard let buffer = buffer else { return }

        let header = buffer.assumingMemoryBound(to: Header.self)
        header.pointee.rows = UInt16(terminal.rows)
        header.pointee.cols = UInt16(terminal.cols)
        header.pointee.cursorRow = UInt16(terminal.cursorRow)
        header.pointee.cursorCol = UInt16(terminal.cursorCol)

        // Copy grid data directly (zero-copy from dterm-core if possible)
        let gridOffset = MemoryLayout<Header>.size
        let gridPtr = buffer.advanced(by: gridOffset)
        terminal.copyGridTo(gridPtr, maxBytes: Self.regionSize - gridOffset)

        // Increment sequence number (atomic)
        OSAtomicIncrement64(&header.pointee.sequenceNum)
    }
}
```

### 2. Change Notification (Mach Ports)

```swift
// DTermChangeNotifier.swift
import Darwin

class DTermChangeNotifier {
    private var notifyPort: mach_port_t = MACH_PORT_NULL

    func setup() {
        // Create notification port
        mach_port_allocate(mach_task_self_, MACH_PORT_RIGHT_RECEIVE, &notifyPort)

        // Register with bootstrap server so clients can find it
        bootstrap_register(bootstrap_port, "com.dashterm.notify", notifyPort)
    }

    func notifyClients() {
        // Send empty message to wake up all listeners
        var msg = mach_msg_header_t()
        msg.msgh_bits = MACH_MSGH_BITS(MACH_MSG_TYPE_COPY_SEND, 0)
        msg.msgh_size = mach_msg_size_t(MemoryLayout<mach_msg_header_t>.size)
        msg.msgh_remote_port = notifyPort
        msg.msgh_local_port = MACH_PORT_NULL
        msg.msgh_id = 1  // TERMINAL_CHANGED

        mach_msg(&msg, MACH_SEND_MSG, msg.msgh_size, 0,
                 MACH_PORT_NULL, MACH_MSG_TIMEOUT_NONE, MACH_PORT_NULL)
    }
}
```

### 3. Client Reader (for AI process)

```swift
// DTermReader.swift - runs in AI client process
import Darwin

class DTermReader {
    private var buffer: UnsafeMutableRawPointer?
    private var lastSeqNum: UInt64 = 0

    func connect() throws {
        let fd = shm_open("com.dashterm.terminal.shm", O_RDONLY, 0)
        guard fd >= 0 else { throw POSIXError(.ENOENT) }

        buffer = mmap(nil, 1024*1024, PROT_READ, MAP_SHARED, fd, 0)
        close(fd)
    }

    func hasChanges() -> Bool {
        guard let buffer = buffer else { return false }
        let header = buffer.assumingMemoryBound(to: DTermSharedMemory.Header.self)
        return header.pointee.sequenceNum != lastSeqNum
    }

    func readScreen() -> String {
        guard let buffer = buffer else { return "" }

        let header = buffer.assumingMemoryBound(to: DTermSharedMemory.Header.self)
        lastSeqNum = header.pointee.sequenceNum

        let rows = Int(header.pointee.rows)
        let cols = Int(header.pointee.cols)

        // Read grid directly from shared memory
        let gridOffset = MemoryLayout<DTermSharedMemory.Header>.size
        let gridPtr = buffer.advanced(by: gridOffset)
            .assumingMemoryBound(to: UInt64.self)  // 8-byte cells

        var result = ""
        for row in 0..<rows {
            for col in 0..<cols {
                let cell = gridPtr[row * cols + col]
                let codepoint = UInt32(cell & 0xFFFF)
                if let scalar = Unicode.Scalar(codepoint) {
                    result.append(Character(scalar))
                }
            }
            result.append("\n")
        }
        return result
    }
}
```

### 4. MCP Adapter (bridges to Claude Code)

```swift
// DTermMCPAdapter.swift
// This is what Claude Code actually connects to

class DTermMCPAdapter {
    let reader = DTermReader()

    func handleMCPRequest(_ request: MCPRequest) -> MCPResponse {
        switch request.method {
        case "read_screen":
            // Fast path: read from shared memory
            let screen = reader.readScreen()
            return MCPResponse(result: ["content": screen])

        case "subscribe_changes":
            // Set up push notifications
            // ...

        default:
            // Fall back to standard MCP for complex ops
            return standardMCPHandler(request)
        }
    }
}
```

---

## Binary Protocol Option

For even more speed, replace JSON with binary:

```rust
// In dterm-core: binary message format
#[repr(C, packed)]
struct ScreenUpdateMessage {
    msg_type: u8,        // 1 = screen update
    seq_num: u64,
    rows: u16,
    cols: u16,
    cursor_row: u16,
    cursor_col: u16,
    // Followed by: rows * cols * 8 bytes of cell data
}

// Serialization: just memcpy the struct
// Deserialization: cast pointer to struct
// No parsing overhead!
```

---

## Integration with Claude Code

Claude Code uses standard MCP. We can't change that. But we can:

1. **Run MCP server in-process** - No IPC overhead
2. **Pre-serialize common responses** - Cache JSON for unchanged screens
3. **Incremental updates** - Send diffs instead of full screen
4. **Async notifications** - Push changes via MCP notifications

```json
// MCP notification (server → client)
{
  "jsonrpc": "2.0",
  "method": "notifications/resources/updated",
  "params": {
    "uri": "terminal://screen",
    "changes": {
      "rows_changed": [5, 6, 7],
      "cursor": {"row": 7, "col": 23}
    }
  }
}
```

---

## Performance Targets

| Metric | Target | How |
|--------|--------|-----|
| Screen read latency | <0.1ms | Shared memory |
| Change detection | <0.01ms | Sequence number check |
| Notification latency | <1ms | Mach ports |
| Memory overhead | <2MB | Compact binary format |
| CPU overhead | <1% | Event-driven, no polling |

---

## Implementation Priority

1. **Phase 1**: Standard MCP server (works with Claude Code today)
2. **Phase 2**: Shared memory for DashTerm2 ↔ local tools
3. **Phase 3**: Binary protocol for maximum performance
4. **Phase 4**: Push notifications for real-time AI awareness

---

## Why This Matters

With sub-millisecond terminal access, AI can:
- **Watch commands execute in real-time**
- **React to errors immediately**
- **Provide contextual suggestions as you type**
- **See exactly what you see, when you see it**

This transforms AI from "ask and wait" to "always aware".
