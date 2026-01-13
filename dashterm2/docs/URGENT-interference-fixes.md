# URGENT: DashTerm2/iTerm2 Interference Fixes

**Created:** December 21, 2025
**Priority:** P0 - CRITICAL - Fix BEFORE any other work
**Reason:** DashTerm2 and iTerm2 running simultaneously causes conflicts

---

## TOP 5 CRITICAL INTERFERENCE ISSUES

These MUST be fixed ASAP to allow development to proceed:

### 1. XPC Helper Bundle Identifiers (CRITICAL)

**Problem:** XPC services still use `com.iterm2.*` bundle IDs, causing the wrong service to be launched when both apps run.

**File:** `/Users/ayates/dashterm2/DashTerm2.xcodeproj/project.pbxproj`

**Changes Required:**
| Line | Old Value | New Value |
|------|-----------|-----------|
| 21480, 21555, 21629, 21703 | `com.iterm2.sandboxed-worker` | `com.dashterm.dashterm2.sandboxed-worker` |
| 23070, 23149, 23228, 23307 | `com.iterm2.pidinfo` | `com.dashterm.dashterm2.pidinfo` |
| 23394, 23479, 23563, 23647 | `com.iterm2.iTermProxy` | `com.dashterm.dashterm2.iTermProxy` |
| 24037, 24117, 24196, 24275 | `com.iterm2.ModernTests` | `com.dashterm.dashterm2.ModernTests` |
| 22397, 24552, 24621, 24690 | `com.iterm2.$(PRODUCT_NAME:rfc1034identifier)` | `com.dashterm.dashterm2.$(PRODUCT_NAME:rfc1034identifier)` |

---

### 2. TERM_PROGRAM Environment Variable (CRITICAL)

**Problem:** Both apps set `TERM_PROGRAM=iTerm.app`, making shell integration unable to distinguish between them.

**File:** `sources/PTYSession.m:2900`

**Change:**
```objc
// OLD:
env[@"TERM_PROGRAM"] = @"iTerm.app";

// NEW:
env[@"TERM_PROGRAM"] = @"DashTerm.app";
```

---

### 3. Pasteboard Type for Tab Dragging (HIGH)

**Problem:** Tab drag/drop uses `com.iterm2.psm.controlitem`, causing tabs to be draggable between apps.

**Files to change:**
- `ThirdParty/PSMTabBarControl/source/PSMTabBarControl.m` (lines 249, 474, 1861, 1887, 1918, 1934)
- `ThirdParty/PSMTabBarControl/source/PSMTabDragAssistant.m` (line 165)

**Change:** Replace ALL occurrences:
```objc
// OLD:
@"com.iterm2.psm.controlitem"

// NEW:
@"com.dashterm.dashterm2.psm.controlitem"
```

---

### 4. SSH Secret Socket Paths (HIGH)

**Problem:** SSH secret forwarding looks in iTerm2's directories.

**Files:**
- `OtherResources/it2ssh` (line 46)
- `OtherResources/Utilities/it2ssh` (line 46)

**Change:**
```bash
# OLD:
for SOCKET in ~/.config/iterm2/sockets/secrets ~/.iterm2/sockets/secrets ~/.iterm2-1/sockets/secrets

# NEW:
for SOCKET in ~/.config/dashterm2/sockets/secrets ~/.dashterm2/sockets/secrets ~/.dashterm2-1/sockets/secrets
```

---

### 5. Dispatch Queue Names (MEDIUM)

**Problem:** Dispatch queues still use `com.iterm2.*` names, causing debugging confusion.

**Changes:**

| File | Line | Old | New |
|------|------|-----|-----|
| `pidinfo/pidinfo.m` | 46 | `com.iterm2.pidinfo` | `com.dashterm.dashterm2.pidinfo` |
| `sources/LineBlock.mm` | 255 | `com.iterm2.lineblock-dealloc` | `com.dashterm.dashterm2.lineblock-dealloc` |
| `pwmplugin/Sources/iterm2-keepassxc-adapter/main.swift` | 305 | `com.iterm2.keepassxc-adapter` | `com.dashterm.dashterm2.keepassxc-adapter` |
| `BetterFontPicker/BetterFontPicker/SystemFontClassifier.swift` | 19 | `com.iterm2.font-classifier` | `com.dashterm.dashterm2.font-classifier` |

---

## ADDITIONAL INTERFERENCE ISSUES (Fix After Top 5)

### 6. Error Domains

**File:** `SignedArchive/SignedArchive/SIGError.m:11`
```objc
// OLD:
NSString *const SIGErrorDomain = @"com.iterm2.sig";

// NEW:
NSString *const SIGErrorDomain = @"com.dashterm.dashterm2.sig";
```

### 7. Test Error Domains (Low Priority - Tests Only)

**File:** `DashTerm2Tests/iTermPromiseTests.m:20`
```objc
// Change domain from "com.iterm2.promise-tests" to "com.dashterm.dashterm2.promise-tests"
```

**File:** `DashTerm2Tests/iTermWeakReferenceTest.m:175-176`
```objc
// Change queue names from "com.iterm2.WeakReferenceTest*" to "com.dashterm.dashterm2.WeakReferenceTest*"
```

### 8. Submodule Bundle Identifiers

These are in separate .xcodeproj files:

**BetterFontPicker:**
- File: `BetterFontPicker/BetterFontPicker.xcodeproj/project.pbxproj`
- Change: `com.iterm2.BetterFontPicker` -> `com.dashterm.dashterm2.BetterFontPicker`

**SearchableComboListView:**
- File: `SearchableComboListView/SearchableComboListView.xcodeproj/project.pbxproj`
- Change: `com.iterm2.SearchableComboListView` -> `com.dashterm.dashterm2.SearchableComboListView`

**MultiCursor:**
- File: `submodules/MultiCursor/MultiCursor.xcodeproj/project.pbxproj`
- Change: `com.iterm2.MultiCursor` -> `com.dashterm.dashterm2.MultiCursor`

### 9. Sparkle Update Key

**File:** `sources/iTermController.m:1340`
```objc
// OLD:
@"SUFeedAlternateAppNameKey" : @"iTerm"

// NEW:
@"SUFeedAlternateAppNameKey" : @"DashTerm2"
```

---

## ADDITIONAL CRITICAL INTERFERENCE (Issues 10-22)

### 10. Keychain Service Names (CRITICAL - Password Sharing)

**Problem:** Both apps read/write to same keychain items for password manager.

**File:** `sources/KeychainPasswordDataSource.swift:14-15`
```swift
// OLD:
static let legacy = "iTerm2"
static let legacyBrowser = "iTerm2-Browser"

// NEW:
static let legacy = "DashTerm2"
static let legacyBrowser = "DashTerm2-Browser"
```

### 11. AI API Keychain Items (HIGH)

**Problem:** Both apps access same keychain for OpenAI API keys.

**File:** `sources/AITermControllerObjC.swift:23-24`
```swift
// OLD:
private static let legacyService = "iTerm2 API Keys"
private static let legacyAccount = "OpenAI API Key for iTerm2"

// NEW:
private static let legacyService = "DashTerm2 API Keys"
private static let legacyAccount = "OpenAI API Key for DashTerm2"
```

### 12. LastPass Group Prefix (HIGH - Password Sharing)

**Problem:** Both apps use same LastPass group, sharing passwords.

**File:** `sources/LastPassDataSource.swift:242`
```swift
// OLD:
let groupPrefix = (browser ? "" : "iTerm2/")

// NEW:
let groupPrefix = (browser ? "" : "DashTerm2/")
```

### 13. OnePassword Tags (HIGH - Password Sharing)

**Problem:** Both apps filter by same 1Password tags.

**File:** `sources/OnePasswordDataSource.swift:47-48, 283, 484, 488`
```swift
// OLD:
tag = browser ? nil : "iTerm2"
tagToExclude = browser ? "iTerm2" : nil
$0.tags?.contains("iTerm2-no-otp")
Set(["iTerm2-no-otp"])

// NEW:
tag = browser ? nil : "DashTerm2"
tagToExclude = browser ? "DashTerm2" : nil
$0.tags?.contains("DashTerm2-no-otp")
Set(["DashTerm2-no-otp"])
```

### 14. Shell Integration Script Names (MEDIUM)

**Problem:** Both apps inject same shell integration script files.

**File:** `sources/ShellIntegrationInjection.swift:115-125`
```swift
// OLD:
local("iterm2_shell_integration.bash")
local("iterm2_shell_integration.zsh")
local("iterm2_shell_integration.fish")
local("iterm2-shell-integration-loader.fish")

// NEW:
local("dashterm2_shell_integration.bash")
local("dashterm2_shell_integration.zsh")
local("dashterm2_shell_integration.fish")
local("dashterm2-shell-integration-loader.fish")
```

**Also rename the actual resource files in OtherResources/**

### 15. Browser WebKit Message Handler (MEDIUM)

**Problem:** WebKit message handler name conflicts.

**File:** `sources/Browser/Editing Detector/iTermBrowserEditingDetectorHandler.swift:12`
```swift
// OLD:
static let messageHandlerName = "iTerm2EditingDetector"

// NEW:
static let messageHandlerName = "DashTerm2EditingDetector"
```

### 16. Ad Blocker Identifier (MEDIUM)

**Problem:** Content blocker uses same identifier.

**File:** `sources/Browser/Ad Blocking/iTermBrowserAdblockManager.swift:234`
```swift
// OLD:
forIdentifier: "iTerm2-Adblock"

// NEW:
forIdentifier: "DashTerm2-Adblock"
```

### 17. Browser URL Schemes (MEDIUM)

**Problem:** Internal browser URL schemes conflict.

**Files:** Multiple in `sources/Browser/`
```swift
// OLD:
"iterm2-file://"
"iterm2-about:"

// NEW:
"dashterm2-file://"
"dashterm2-about:"
```

**Affected files:**
- `sources/Browser/Core/iTermBrowserManager.swift:225,1320`
- `sources/Browser/Local Pages/iTermBrowserLocalPageManager.swift:103,110,184,219,222`
- `sources/Browser/Local Pages/iTermBrowserWelcomePageHandler.swift:62,65`
- `sources/Browser/Local Pages/iTermBrowserFileHandler.swift:25,124`
- `sources/Browser/Local Pages/iTermBrowserStaticPageHandler.swift` (comments)

### 18. Shell Integration Window Controller (MEDIUM)

**File:** `sources/iTermShellIntegrationWindowController.m:276,409`
```objc
// OLD:
URLForResource:@"iterm2_shell_integration"
grep iterm2_shell_integration

// NEW:
URLForResource:@"dashterm2_shell_integration"
grep dashterm2_shell_integration
```

### 19. Config Directory Paths

**Problem:** Both apps may use `.iterm2` config directories.

**Files to check:** `NSFileManager+iTerm.m`, `SecretServer.swift`
- Change `.iterm2` -> `.dashterm2`
- Change `~/.config/iterm2` -> `~/.config/dashterm2`

### 20. Terminfo Helper Define

**File:** `sources/iTermTerminfoHelper.m:8`
```objc
// OLD:
#define entry iterm2_terminfo_entry

// NEW:
#define entry dashterm2_terminfo_entry
```

### 21. Expression Parser Error Domains

**File:** `sources/iTermExpressionParser.m` (multiple locations)
```objc
// OLD:
@"com.iterm2.call"
@"com.iterm2.parser"

// NEW:
@"com.dashterm.dashterm2.call"
@"com.dashterm.dashterm2.parser"
```

### 22. All Remaining iterm2.com URL References (LOW - Documentation Only)

These are documentation/help URLs that should be updated:
- `sources/NerdFontInstaller.swift:159` - nerd fonts download
- `sources/LastPassDataSource.swift:581` - lastpass-cli help
- `sources/DonateViewController.swift:17` - donate URL
- `sources/AITermControllerRegistrationHelper.swift:65-86` - AI registration URLs
- `sources/Browser/Core/iTermBrowserGateway.swift:107,155` - browser plugin help
- `sources/SSHConfigurationWindowController.swift:294` - SSH wiki
- `sources/ProfilesColorsPreferencesViewController.m:35` - color gallery
- Various other documentation URLs

**Note:** These are LOW priority but should eventually be updated or redirected.

### 23. Rename DashTerm2.xcodeproj to DashTerm2.xcodeproj (MEDIUM) ✅ COMPLETE

**Status:** COMPLETED in iteration 616

**Changes Made:**
1. Renamed directory: `DashTerm2.xcodeproj` -> `DashTerm2.xcodeproj`
2. Renamed scheme: `iTerm2.xcscheme` -> `DashTerm2.xcscheme`
3. Updated container references in all `.xcscheme` files
4. Updated `.xctestplan` container reference
5. Updated all references in `CLAUDE.md`, `ci.yml`, `smoke-test.sh`, `add-uitest-target.py`, `.gitignore`

---

## VERIFICATION CHECKLIST

After making all changes:

1. **Build succeeds:**
   ```bash
   xcodebuild -project DashTerm2.xcodeproj -scheme DashTerm2 -configuration Development build CODE_SIGNING_ALLOWED=NO
   ```

2. **Grep verification (should return 0 results for source files):**
   ```bash
   grep -r "com\.iterm2\." sources/ pidinfo/ SignedArchive/ --include="*.m" --include="*.mm" --include="*.swift" | grep -v "//.*iterm2" | grep -v "iterm2.com"
   ```

3. **Test with both apps running:**
   - Launch iTerm2
   - Launch DashTerm2 from Xcode
   - Verify no XPC errors in Console.app
   - Verify shell integration works correctly
   - Verify tab dragging doesn't cross apps

---

## WORKER INSTRUCTIONS

**ITERATION:** This is a P0 task. Begin immediately.

**TOTAL ISSUES:** 22 interference sources identified

**ORDER:**
1. Fix issues 1-5 (CRITICAL) in first commit
2. Build and verify
3. Fix issues 6-13 (HIGH - includes keychain/password conflicts) in second commit
4. Build and verify
5. Fix issues 14-22 (MEDIUM/LOW) in third commit
6. Build and verify
7. Run comprehensive grep to find any remaining `iterm2` references

**COMMIT FORMAT:**
```
# N: Fix DashTerm2/iTerm2 interference (Part X of 3)

**Current Plan**: docs/URGENT-interference-fixes.md
**Checklist**: Build succeeded

## Changes
[List all changes made]

## Next AI: [Continue interference fixes OR Continue with TASK 0 retro tests]
```

**COMPREHENSIVE VERIFICATION:**
```bash
# Find ALL remaining iterm2 references (should be minimal after fixes)
grep -ri "iterm2" sources/ --include="*.m" --include="*.mm" --include="*.swift" --include="*.h" | grep -v "// " | grep -v "DashTerm2" | grep -v "iterm2.com" | wc -l

# Should approach 0 for non-URL, non-comment references
```
