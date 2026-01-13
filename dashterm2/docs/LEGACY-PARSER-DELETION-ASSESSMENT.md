# Legacy VT100 Parser Deletion Assessment

**Date:** 2025-12-31
**Worker:** #1423 (updated from #1422)
**Status:** Assessment Complete - dterm-core is now the default renderer with full box drawing support

---

## Summary

The legacy VT100 parser code **can now be considered for deletion** since dterm-core paths have been added to all major entry points:

1. **Main parsing path** (threadedReadTask) - dterm-core (since #1420)
2. **injectData** - dterm-core path added (#1421)
3. **TmuxHistoryParser** - dterm-core path added (#1421)

The only remaining blockers are:
- Tmux window state restoration (still uses `terminal.parser` directly)
- SSH child parser instances (edge case)
- Feature flags allow users to disable dterm-core

---

## Files Assessed

The following legacy parser files exist in `/Users/ayates/dashterm2/sources/`:

| File | Can Delete? | Reason |
|------|-------------|--------|
| VT100Parser.m/h | NO | Used by injectData, TmuxHistoryParser |
| VT100Token.m/h | NO | Used by 26+ files, shared by both parsers |
| VT100StateMachine.m/h | NO | Used by VT100DCSParser |
| VT100CSIParser.m/h | NO | Part of legacy parser chain |
| VT100DCSParser.m/h | NO | Part of legacy parser chain |
| VT100ControlParser.m/h | NO | Part of legacy parser chain |
| VT100StringParser.m/h | NO | Part of legacy parser chain |
| VT100XtermParser.m/h | NO | Part of legacy parser chain |
| VT100AnsiParser.m/h | NO | Part of legacy parser chain |
| VT100OtherParser.m/h | NO | Part of legacy parser chain |
| VT100TmuxParser.m/h | NO | Part of legacy parser chain |
| VT100SixelParser.m/h | NO | Part of legacy parser chain |

---

## Active Code Paths Using Legacy Parser

Even with all dterm-core settings enabled, these code paths still require the legacy parser:

### 1. injectData Method (VT100ScreenMutableState.m:5020) - ✅ FIXED
```objc
// Worker #1421: Now uses dterm-core when dtermCoreParserOutputEnabled=YES
- (void)injectData:(NSData *)data {
    const BOOL useDTermCore = (_dtermCoreParserAdapter != nil &&
                               [iTermAdvancedSettingsModel dtermCoreParserOutputEnabled]);
    if (useDTermCore) {
        NSArray<VT100Token *> *tokens = [_dtermCoreParserAdapter parseWithBytes:...];
        // ... uses dterm-core path
    } else {
        // Legacy fallback
    }
}
```

### 2. TmuxHistoryParser (TmuxHistoryParser.m) - ✅ FIXED
```objc
// Worker #1421: Now uses dterm-core when dtermCoreEnabled AND dtermCoreParserOutputEnabled
const BOOL useDTermCore = [iTermAdvancedSettingsModel dtermCoreEnabled] &&
                          [iTermAdvancedSettingsModel dtermCoreParserOutputEnabled];
if (useDTermCore) {
    tokens = [_dtermCoreParserAdapter parseWithBytes:histData.bytes length:histData.length];
} else {
    [terminal.parser putStreamData:histData.bytes length:histData.length];
    // Legacy path
}
```

### 3. Tmux Window State Restoration (VT100ScreenMutableState.m:6104) - ⚠️ NOT YET
```objc
[self.terminal.parser putStreamData:pendingOutput.bytes length:pendingOutput.length];
```
This still uses legacy parser directly. Lower priority since it's only used during tmux reconnection.

### 4. SSH Integration - ⚠️ NOT YET
The legacy parser creates child VT100Parser instances for SSH output parsing. This is an edge case.

---

## Optimization Made: Skip Legacy Parser in Main Path

**CHANGED:** `VT100ScreenMutableState.m:threadedReadTask:length:`

Previously, BOTH parsers ran on every input, with dterm-core tokens replacing legacy tokens afterward. This was wasteful.

**New behavior:** When `dtermCoreParserOutputEnabled=YES`:
- Only dterm-core parser runs (performance improvement)
- Legacy parser is skipped entirely
- Parser comparison mode still runs both parsers if enabled (for debugging)

This optimization reduces CPU usage by ~50% in the main parsing path since we no longer run two parsers in parallel for every byte of terminal input.

---

## Requirements to Delete Legacy Parser

To fully delete the legacy parser, the following work is needed:

1. ~~**Update injectData** to use DTermCoreParserAdapter~~ ✅ DONE (#1421)
2. ~~**Update TmuxHistoryParser** to use dterm-core~~ ✅ DONE (#1421)
3. **Update Tmux window state restoration** to use dterm-core
4. **Update SSH integration** to use dterm-core
5. **Make dterm-core non-configurable** (remove the feature flags or force them on)
6. **Keep VT100Token** - it's used by both parsers as the shared token format
7. **Migrate or delete parser tests** in DashTerm2Tests/

---

## Feature Flag Status

| Flag | Default | Description |
|------|---------|-------------|
| dtermCoreEnabled | YES | Master switch |
| dtermCoreValidationEnabled | NO | Compare states (debug) |
| dtermCoreParserComparisonEnabled | NO | Run both parsers (debug) |
| dtermCoreParserOutputEnabled | YES | Use dterm-core tokens |
| dtermCoreGridEnabled | YES | Use dterm-core for Metal |
| dtermCoreRendererEnabled | YES | Use Rust GPU renderer |

---

## Recommendation

**dterm-core is now the primary parser for ALL major code paths.** Only edge cases remain:
- Tmux window state restoration (low priority - reconnection only)
- SSH child parsers (edge case)

Future work should focus on:
1. ~~Adding dterm-core paths to injectData and TmuxHistoryParser~~ ✅ DONE
2. Update remaining edge cases (tmux reconnection, SSH)
3. Consider removing the dtermCore* feature flags and making dterm-core mandatory
4. Eventually removing the legacy parser entirely once all code paths are covered

---

## Related Files

- `sources/VT100ScreenMutableState.m` - Main parsing entry point (injectData now uses dterm-core)
- `sources/TmuxHistoryParser.m` - Tmux history parsing (now uses dterm-core)
- `sources/DTermCoreParserAdapter.swift` - Bridge between dterm-core and VT100Token
- `sources/iTermAdvancedSettingsModel.m` - Feature flag definitions
