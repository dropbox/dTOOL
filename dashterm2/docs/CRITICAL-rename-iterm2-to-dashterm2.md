# CRITICAL: Complete iTerm2 → DashTerm2 Rename

**Priority:** P0 - BLOCKING - App crashes due to naming inconsistencies
**Created:** December 22, 2025

---

## THE PROBLEM

The app crashes on launch because of mixed iTerm2/DashTerm2 naming:
- Xcode builds products to different folders (`Development/` vs `Variant-ASan/Development/`)
- `DashTerm2.debug.dylib` goes missing because build paths get confused
- The scheme still references `iTerm2.app` and `iTerm2` blueprints

## REQUIRED FIXES

### 1. Rename Xcode Scheme References (CRITICAL)

**File:** `DashTerm2.xcodeproj/xcshareddata/xcschemes/DashTerm2.xcscheme`

Replace ALL occurrences:
```xml
<!-- OLD -->
BuildableName = "iTerm2.app"
BlueprintName = "iTerm2"

<!-- NEW -->
BuildableName = "DashTerm2.app"
BlueprintName = "DashTerm2"
```

Lines to fix: 18, 19, 44 (test bundle), 84, 85, 126, 127

### 2. Rename Target in project.pbxproj (CRITICAL)

**File:** `DashTerm2.xcodeproj/project.pbxproj`

The main target is still named `iTerm2`. Rename it to `DashTerm2`:
- Search for `name = iTerm2;` in target definitions
- Search for `productName = iTerm2;`
- Update PRODUCT_NAME build settings

### 3. Rename Test Target

**File:** `DashTerm2.xcodeproj/project.pbxproj`

- `DashTerm2Tests` → `DashTerm2Tests` or keep but ensure consistent
- `iTerm2SharedARC` → consider renaming to `DashTerm2SharedARC`

### 4. Update All .xcscheme Files

Check ALL schemes in `xcshareddata/xcschemes/`:
- `DashTerm2.xcscheme`
- `DashTerm2Tests.xcscheme` (if exists)
- Any other schemes

### 5. Verify PRODUCT_NAME Settings

In project.pbxproj, ensure:
```
PRODUCT_NAME = DashTerm2;
```

Not:
```
PRODUCT_NAME = iTerm2;
```

---

## VERIFICATION

After changes:

```bash
# Clean everything
rm -rf ~/Library/Developer/Xcode/DerivedData/DashTerm2-*

# Build
xcodebuild -project DashTerm2.xcodeproj -scheme DashTerm2 -configuration Development build CODE_SIGNING_ALLOWED=NO CODE_SIGN_IDENTITY="-"

# Verify dylib exists
ls -la ~/Library/Developer/Xcode/DerivedData/DashTerm2-*/Build/Products/Development/DashTerm2.app/Contents/MacOS/DashTerm2.debug.dylib

# Run the app
open ~/Library/Developer/Xcode/DerivedData/DashTerm2-*/Build/Products/Development/DashTerm2.app
```

---

## WHY THIS MATTERS

When Xcode sees `iTerm2` as the target name but the project is `DashTerm2.xcodeproj`, it creates confusion:
1. Build products go to inconsistent paths
2. Debug dylibs don't get copied to the right place
3. The app crashes with "Library not loaded: DashTerm2.debug.dylib"

---

## COMMIT FORMAT

```
[MANAGER] Critical: Rename iTerm2 → DashTerm2 in Xcode project

Fixes app crash on launch due to missing DashTerm2.debug.dylib.
The mixed naming caused Xcode to build to inconsistent paths.

Changes:
- Renamed main target from iTerm2 to DashTerm2
- Updated scheme references
- Fixed PRODUCT_NAME settings
```
