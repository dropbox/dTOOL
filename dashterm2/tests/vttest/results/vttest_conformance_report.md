# DashTerm2 vttest Conformance Report

**Date:** 2025-12-30
**Parser:** dterm-core (via DTermCoreParserAdapter)
**vttest Version:** VT100 test program, version 2.7 (20251205)

## Test Environment

- **macOS Version:** 15.7.3
- **Xcode Version:** Xcode 16.2
- **DashTerm2 Build:** Development
- **dterm-core Enabled:** YES
- **Parser Comparison:** YES
- **Parser Output:** YES (using dterm-core tokens)

## Test Results

### Menu 1: Cursor Movement Tests

| Test | Result | Notes |
|------|--------|-------|
| CUU (Cursor Up) | | |
| CUD (Cursor Down) | | |
| CUF (Cursor Forward) | | |
| CUB (Cursor Back) | | |
| CUP (Cursor Position) | | |
| HVP (Horizontal/Vertical Position) | | |
| Cursor absolute moves | | |
| Cursor relative moves | | |
| Cursor wraparound | | |

### Menu 2: Screen Features Tests

| Test | Result | Notes |
|------|--------|-------|
| ED (Erase in Display) | | |
| EL (Erase in Line) | | |
| DCH (Delete Character) | | |
| ICH (Insert Character) | | |
| IL (Insert Line) | | |
| DL (Delete Line) | | |
| DECAWM (Auto Wrap) | | |
| DECSTBM (Scroll Region) | | |
| DECOM (Origin Mode) | | |

### Menu 3: Character Set Tests

| Test | Result | Notes |
|------|--------|-------|
| DEC Special Graphics | | |
| UK Character Set | | |
| G0/G1 Set Selection | | |
| SI/SO (Shift In/Out) | | |

### Menu 6: Terminal Reports Tests

| Test | Result | Notes |
|------|--------|-------|
| DA (Device Attributes) | | |
| DSR (Device Status Report) | | |
| CPR (Cursor Position Report) | | |

### Menu 8: VT102 Features Tests

| Test | Result | Notes |
|------|--------|-------|
| DECSC/DECRC (Save/Restore Cursor) | | |
| Additional scroll regions | | |
| Insert/delete operations | | |

### Menu 11: Non-VT100 Features Tests

| Test | Result | Notes |
|------|--------|-------|
| Cursor styles | | |
| 256-color support | | |
| True color (RGB) | | |
| Bracketed paste mode | | |
| Alternate screen buffer | | |
| Mouse tracking | | |

## Summary

| Category | Tests | Pass | Fail | Skip |
|----------|-------|------|------|------|
| Cursor Movement | | | | |
| Screen Features | | | | |
| Character Sets | | | | |
| Terminal Reports | | | | |
| VT102 Features | | | | |
| Non-VT100 Features | | | | |
| **Total** | | | | |

## Notes

- Fill in this report after running vttest manually in DashTerm2
- Mark PASS for tests that display correctly
- Mark FAIL for tests with incorrect display
- Mark SKIP for tests not applicable to DashTerm2

## Related Documents

- `docs/DTERM-AI-DIRECTIVE-V3.md` - Phase 3 directive
- `docs/dterm-core-validation.log` - Parser validation log
- `~/dterm/docs/CONFORMANCE.md` - dterm-core conformance details
