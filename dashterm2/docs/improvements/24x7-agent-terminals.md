# 24/7 Agent Terminal Requirements

**Use case:** Terminals running AI agents continuously for days/weeks/months.

**Requirements:**
- Never crash
- Never slow down
- Never run out of memory
- Never lose output
- Logs captured automatically

---

## Problem 1: Memory Grows Forever

**Current behavior:**
- Scrollback buffer grows until `maxScrollbackLines`
- Each line uses ~500 bytes (with attributes)
- 1M lines × 500 bytes = 500MB per session
- 10 sessions = 5GB just for scrollback

**For 24/7 agents:**
- Agents generate massive output (builds, tests, logs)
- Memory grows until swap, then crash
- Or OOM killer terminates the app

### Solution: Bounded Memory with Disk Spillover

```objc
// New: iTermDiskBackedScrollback

@interface iTermDiskBackedScrollback : NSObject

// Keep only N lines in RAM (the "hot" region)
@property (nonatomic) NSUInteger linesInMemory;  // Default: 10,000

// Older lines spill to compressed file on disk
@property (nonatomic, readonly) NSString *spillFilePath;

// Total lines (memory + disk)
@property (nonatomic, readonly) NSUInteger totalLines;

// Memory usage stays constant regardless of total lines
@property (nonatomic, readonly) NSUInteger memoryUsage;

@end

@implementation iTermDiskBackedScrollback

- (void)appendLine:(ScreenCharArray *)line {
    [self.hotBuffer addLine:line];

    if (self.hotBuffer.count > self.linesInMemory) {
        // Spill oldest lines to disk (compressed)
        NSArray *coldLines = [self.hotBuffer removeOldestLines:1000];
        [self.diskStorage appendCompressedLines:coldLines];
    }
}

- (ScreenCharArray *)lineAtIndex:(NSUInteger)index {
    if (index >= self.totalLines - self.hotBuffer.count) {
        // Line is in memory
        return [self.hotBuffer lineAtIndex:index - self.diskLineCount];
    } else {
        // Line is on disk - decompress on demand
        return [self.diskStorage lineAtIndex:index];
    }
}

@end
```

**Memory profile:**
```
Before: 500MB after 1M lines, grows forever
After:  50MB constant (10K lines in RAM), disk grows but memory stable
```

---

## Problem 2: No Automatic Logging

**Current behavior:**
- Logging disabled by default
- Must configure per-profile
- No compression
- No retention

**For 24/7 agents:**
- Need to debug what happened 3 days ago
- Need audit trail
- Can't afford to lose output

### Solution: Global Compressed Logging

```objc
// New global preferences
extern NSString *const kPreferenceKeyGlobalAutoLog;           // BOOL, default YES for agent mode
extern NSString *const kPreferenceKeyGlobalLogDirectory;      // String, default ~/.dashterm/logs/
extern NSString *const kPreferenceKeyGlobalLogRetentionDays;  // Int, default 30
extern NSString *const kPreferenceKeyGlobalLogCompression;    // BOOL, default YES

// New: iTermGlobalLogger - singleton that handles all session logging

@interface iTermGlobalLogger : NSObject

+ (instancetype)sharedInstance;

// Called when any session outputs data
- (void)logData:(NSData *)data
    forSession:(NSString *)sessionGUID
     timestamp:(NSDate *)timestamp;

// Compression happens in background
// Files rotated daily
// Old files cleaned up automatically

@end

@implementation iTermGlobalLogger {
    dispatch_queue_t _compressionQueue;
    NSMutableDictionary<NSString *, NSFileHandle *> *_sessionHandles;
    z_stream _zstream;  // zlib compression stream
}

- (void)logData:(NSData *)data forSession:(NSString *)sessionGUID timestamp:(NSDate *)timestamp {
    dispatch_async(_compressionQueue, ^{
        // Get or create compressed file handle for this session
        NSFileHandle *handle = [self handleForSession:sessionGUID date:timestamp];

        // Compress data
        NSData *compressed = [self compress:data];

        // Write (already on background queue)
        [handle writeData:compressed];
    });
}

- (void)dailyMaintenance {
    // Rotate logs (new file per day per session)
    // Delete logs older than retention period
    // Report disk usage
}

@end
```

**Log format:**
```
~/.dashterm/logs/
├── 2025-12-27/
│   ├── session-abc123.log.gz      (compressed)
│   ├── session-def456.log.gz
│   └── index.json                  (session metadata)
├── 2025-12-26/
│   └── ...
└── usage.json                      (total size, cleanup dates)
```

**Compression stats:**
```
Raw:        100 MB/day typical agent output
Compressed: 10 MB/day (90% compression on text)
30 days:    300 MB total
```

---

## Problem 3: Crashes on Edge Cases

**Current behavior:**
- 367 bugs fixed, but edge cases remain
- Force unwraps in Swift
- Unchecked array bounds in ObjC
- Unicode edge cases

**For 24/7 agents:**
- Any crash = lost work, broken automation
- Agents hit more edge cases than humans (weird output, binary data)

### Solution: Defensive Everything

```objc
// Audit all array access
// Before:
id obj = array[index];  // Crash if out of bounds

// After:
id obj = [array it_objectAtIndex:index];  // Returns nil if out of bounds

// Category on NSArray
@implementation NSArray (SafeAccess)
- (nullable id)it_objectAtIndex:(NSUInteger)index {
    if (index >= self.count) {
        DLog(@"Array access out of bounds: %lu >= %lu", index, self.count);
        return nil;
    }
    return self[index];
}
@end
```

```swift
// Audit all force unwraps
// Before:
let value = dict["key"]!  // Crash if nil

// After:
guard let value = dict["key"] else {
    DLog("Missing expected key")
    return
}
```

**Automated audit:**
```bash
# Find remaining force unwraps
grep -r "!\." sources/*.swift | grep -v "// safe:" | wc -l

# Find unchecked array access
grep -r "\[.*\]" sources/*.m | grep -v "it_objectAtIndex" | wc -l
```

---

## Problem 4: Performance Degrades Over Time

**Current behavior:**
- Search slows as scrollback grows
- Rendering slows with many tabs
- Memory fragmentation over days

**For 24/7 agents:**
- Terminal must be as fast on day 30 as day 1

### Solution: Constant-Time Operations

```objc
// Search: Pre-index as lines arrive
@interface iTermSearchIndex : NSObject

// Index updated incrementally as output arrives
- (void)indexLine:(NSString *)line atPosition:(NSUInteger)position;

// Search is O(results) not O(scrollback)
- (NSArray<NSNumber *> *)searchFor:(NSString *)query;

@end

// Rendering: Only render visible region
@interface iTermLazyRenderer : NSObject

// Only keep GPU resources for visible lines + small buffer
@property (nonatomic) NSRange visibleRange;

// Lines outside visible range have no GPU memory
- (void)renderVisibleLines;

@end
```

---

## Problem 5: Tab Switching Lags

**Current behavior:**
- Each tab has full session in memory
- Switching to old tab may need to page in

**For 24/7 agents:**
- Many tabs (one per agent/task)
- Switching must be instant

### Solution: Hot/Cold Tab Management

```objc
@interface iTermTabThermalManager : NSObject

// Tabs accessed recently stay "hot" (full data in RAM)
// Tabs not accessed for N minutes go "warm" (visible region only)
// Tabs not accessed for N hours go "cold" (metadata only, data on disk)

- (void)tabDidBecomeActive:(PTYTab *)tab;
- (void)tabDidBecomeInactive:(PTYTab *)tab;

// Background thread manages thermal state
- (void)thermalMaintenanceLoop;

@end

// Thermal states
typedef NS_ENUM(NSInteger, TabThermalState) {
    TabThermalStateHot,    // Full data in RAM, instant access
    TabThermalStateWarm,   // Visible + 1000 lines, fast access
    TabThermalStateCold,   // Metadata only, load on demand
};
```

---

## Implementation Priority

### Phase 1: Stability (Week 1-2)
1. [ ] Audit all force unwraps in Swift
2. [ ] Audit all array access in ObjC
3. [ ] Add safe access categories
4. [ ] Crash reporting for any remaining issues

### Phase 2: Memory (Week 3-4)
1. [ ] Implement disk-backed scrollback
2. [ ] Add memory usage monitoring
3. [ ] Implement tab thermal management
4. [ ] Target: <100MB per session regardless of scrollback

### Phase 3: Logging (Week 5-6)
1. [ ] Add global logging preference
2. [ ] Implement compressed logging
3. [ ] Add retention/rotation
4. [ ] Simple settings UI

### Phase 4: Performance (Week 7-8)
1. [ ] Implement incremental search index
2. [ ] Optimize rendering for visible region only
3. [ ] Profile and fix any remaining bottlenecks

---

## Success Metrics

| Metric | Current | Target |
|--------|---------|--------|
| Memory after 1M lines | 500MB+ | <100MB |
| Memory after 30 days | Unbounded | <500MB |
| Crash rate (per week) | Unknown | 0 |
| Tab switch time | 50-200ms | <16ms |
| Search 1M lines | 1-3s | <100ms |
| Log storage (30 days) | N/A (disabled) | <500MB |

---

## Settings UI

```
┌─────────────────────────────────────────────────────────────────────┐
│  Settings > Agent Mode (24/7)                                       │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ☑ Enable Agent Mode                                                │
│    Optimizes for long-running terminal sessions                     │
│                                                                     │
│  Memory Management                                                  │
│  ─────────────────                                                  │
│    Lines in memory: [10,000 ▼]                                      │
│    Older lines saved to: ~/.dashterm/scrollback/          [Change] │
│                                                                     │
│  Automatic Logging                                                  │
│  ─────────────────                                                  │
│    ☑ Save all terminal output                                       │
│    Location: ~/.dashterm/logs/                            [Change] │
│    Compression: ☑ Enabled                                           │
│    Keep for: [30 days ▼]                                            │
│    Current usage: 247 MB                              [Clear Logs] │
│                                                                     │
│  Stability                                                          │
│  ─────────────────                                                  │
│    ☑ Send anonymous crash reports                                   │
│    Last crash: Never                                                │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

---

## Quick Wins (Can Do Now)

1. **Global logging toggle** - Add preference, hook into existing logger
2. **Compression** - Add zlib to existing logger (few lines of code)
3. **Memory warning** - Alert when session exceeds threshold
4. **Crash on force unwrap audit** - grep + fix pattern
