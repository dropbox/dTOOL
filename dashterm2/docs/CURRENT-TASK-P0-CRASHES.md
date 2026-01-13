# CURRENT TASK: Fix P0 Crash Bugs

**Priority:** P0 - Critical
**Category:** Crashes and Hangs
**Status:** ✅ COMPLETE (all 289 P0 bugs triaged, 98 fixed)

---

## WORKER INSTRUCTIONS

Fix the P0 crash bugs listed below, starting with the highest issue numbers (most recent).
Each crash fix should be one commit.

---

## Bug #11877: Crash in NSIndexSetEnumerate (null pointer)

**Upstream:** https://gitlab.com/gnachman/iterm2/-/issues/11877
**Severity:** CRASH
**Repro:** Leave DashTerm2 open for days, run `git diff`, press `g`

### Analysis

The crash happens in `__NSIndexSetEnumerate` called from `LineBlock.mm`:

```
Thread 0 Crashed: Dispatch queue: com.apple.main-thread
0   Foundation                      __NSIndexSetEnumerate + 760
1   DashTerm2                          -[LineBlock offsetOfWrappedLineInBufferAtOffset:wrappedLineNumber:bufferLength:width:metadata:] + 740
```

**Crash Address:** 0x8 (NULL + 8 bytes = dereferencing field in NULL struct)

**Root Cause Location:**
- File: `sources/LineBlock.mm`, line ~1003
- Method: `offsetOfWrappedLineInBufferAtOffset:wrappedLineNumber:bufferLength:width:metadata:`

**Code Path:**
1. Line 994: Gets `metadata` from provider
2. Line 996: Checks if `metadata->doubleWidthCharacters` is valid
3. Line 1003: Calls `[metadata->doubleWidthCharacters offsetForWrappedLine:n totalLines:&lines]`

**Likely Bug:**
After `populateDoubleWidthCharacterCacheInMetadata:` (lines 997-1000), the code accesses
`metadata->doubleWidthCharacters` at line 1003, but the provider might have been
mutated in between, and the original `metadata` pointer (line 994) is now stale.

**Fix Approach:**
1. Re-fetch metadata after populating the cache
2. OR add null check before calling `offsetForWrappedLine:`
3. OR ensure the metadata provider returns a stable pointer

**Files to Edit:**
- `sources/LineBlock.mm`

### Fix Code (Suggested)

```objc
// In offsetOfWrappedLineInBufferAtOffset:wrappedLineNumber:bufferLength:width:metadata:
// After line 1001, re-fetch the metadata:
const LineBlockMetadata *metadata = iTermLineBlockMetadataProviderGetImmutable(metadataProvider);
// Then add null check:
if (!metadata->doubleWidthCharacters) {
    ITAssertWithMessage(NO, @"doubleWidthCharacters is nil after populate");
    return n * width;  // Fallback
}
```

---

## Bug #12625: Beachball on Export (main thread blocked)

**Upstream:** https://gitlab.com/gnachman/iterm2/-/issues/12625
**Severity:** HANG
**Repro:** Export terminal contents with large scrollback

### Analysis

Export operation runs on main thread, blocking GUI when writing large files.

**Fix Approach:**
Move file I/O to background queue using GCD:

```objc
dispatch_async(dispatch_get_global_queue(QOS_CLASS_USER_INITIATED, 0), ^{
    // Do file I/O here
    NSError *error = nil;
    [data writeToFile:path options:NSDataWritingAtomic error:&error];
    dispatch_async(dispatch_get_main_queue(), ^{
        // Update UI with result
    });
});
```

**Files to Edit:**
- `sources/ImportExport.swift` (likely)
- Search for export-related methods that write to disk

---

## Bug #11776: Crash on emoji output

**Upstream:** https://gitlab.com/gnachman/iterm2/-/issues/11776
**Severity:** CRASH
**Repro:** Run `pipx upgrade-all` or output emoji characters

### Analysis

Likely issue in text rendering with emoji/unicode characters.

**Fix Approach:**
1. Check emoji handling in `PTYTextView.m` and `iTermTextDrawingHelper.m`
2. Look for buffer overflows or invalid index access
3. Add bounds checking for emoji character sequences

---

## Bug #11747: Crash when switching focus

**Upstream:** https://gitlab.com/gnachman/iterm2/-/issues/11747
**Severity:** CRASH
**Repro:** Switch focus to another app repeatedly

### Analysis

Likely race condition in focus handling.

**Fix Approach:**
1. Check `PseudoTerminal.m` focus handling code
2. Look for weak reference issues
3. Add null checks in delegate callbacks

---

## Bug #10666: Crash during high output scrolling

**Upstream:** https://gitlab.com/gnachman/iterm2/-/issues/10666
**Severity:** CRASH
**Repro:** Run command with lots of output (e.g., `find /`)

### Analysis

Likely buffer overflow or threading issue during rapid output.

**Fix Approach:**
1. Check `PTYSession.m` output handling
2. Review `LineBuffer` and `LineBlock` threading
3. Add synchronization for buffer access

---

## Priority Order

Fix these in order (highest issue number = most recent = fix first):

1. ~~#12625 - Beachball on export~~ DONE
2. ~~#12323 - Bell flooding~~ DONE
3. ~~#11877 - Crash in NSIndexSetEnumerate~~ DONE
4. ~~#11776 - Crash on emoji output~~ DONE
5. ~~#11747 - Crash when switching focus~~ DONE
6. ~~#10666 - Crash during high output scrolling~~ DONE
7. Continue with remaining ~285 crash bugs in `docs/UPSTREAM-ISSUES.md`

---

## Testing

After each fix:
1. Build: `xcodebuild -project DashTerm2.xcodeproj -scheme DashTerm2 -configuration Development build CODE_SIGNING_ALLOWED=NO CODE_SIGN_IDENTITY="-"`
2. Run smoke test: `./scripts/smoke-test.sh`
3. Try to reproduce the original bug

---

## Commit Format

```
# N: Fix upstream #ISSUE_NUMBER - Brief description

Detailed explanation of what was wrong and how it was fixed.

Upstream: https://gitlab.com/gnachman/iterm2/-/issues/ISSUE_NUMBER
```

---

## Already Fixed

- [x] #12625 - Beachball on export (fixed in commit 2052c45bf)
- [x] #12323 - Bell flooding bricks DashTerm2 (fixed in commit f4b7f2f76)
- [x] #11877 - Crash in NSIndexSetEnumerate (fixed by re-fetching metadata after cache population)
- [x] #11776 - Crash on emoji output (fixed by adding bounds check in ScreenChar.m)
- [x] #11747 - Crash when switching focus (fixed by adding nil checks in windowDidBecomeKey/windowDidResignKey/windowDidResignMain in PseudoTerminal.m)
- [x] #10666 - Crash during high output scrolling (fixed by adding bounds checks in iTermLineBlockArray.m and safe exit in LineBuffer.m)
