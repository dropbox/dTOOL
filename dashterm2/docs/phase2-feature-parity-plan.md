# Phase 2: Feature Parity Plan

**Created:** Worker #858 - December 23, 2025
**Status:** In Progress

---

## Executive Summary

Phase 1 Performance Optimization is complete (11/12 items implemented). Phase 2 focuses on Browser Extension Feature Parity - ensuring the WebExtensionsFramework has the key Chrome APIs needed for common extensions.

---

## Current State

### WebExtensionsFramework Status (December 23, 2025)

| Metric | Status |
|--------|--------|
| **Tests** | 302 passed, 0 failures, 6 skipped |
| **Build** | ✅ Compiles successfully |
| **APIs Implemented** | chrome.runtime, chrome.storage |
| **APIs Missing** | chrome.tabs, chrome.action, declarativeNetRequest |

### Implemented Chrome APIs

| API | Handler File | JavaScript File | Status |
|-----|--------------|-----------------|--------|
| `chrome.runtime.sendMessage` | SendMessageHandler.swift | chrome-runtime-api.js | ✅ Complete |
| `chrome.runtime.onMessage` | (in SendMessageHandler) | chrome-runtime-api.js | ✅ Complete |
| `chrome.runtime.getPlatformInfo` | GetPlatformInfoHandler.swift | chrome-runtime-api.js | ✅ Complete |
| `chrome.storage.local.*` | StorageHandlers.swift | chrome-storage-api.js | ✅ Complete |
| `chrome.storage.sync.*` | StorageHandlers.swift | chrome-storage-api.js | ✅ Complete |
| `chrome.storage.session.*` | StorageHandlers.swift | chrome-storage-api.js | ✅ Complete |
| `chrome.storage.managed.*` | StorageHandlers.swift | chrome-storage-api.js | ✅ Complete (read-only) |

---

## Phase 2 Priorities

### Priority 1: chrome.tabs API (Critical)

**Why:** Most extensions need tab management. Required by test extension `custom-user-agent`.

**Methods to Implement:**

| Method | Priority | Description |
|--------|----------|-------------|
| `chrome.tabs.query()` | P0 | Query tabs by properties (active, currentWindow, url) |
| `chrome.tabs.get()` | P0 | Get tab by ID |
| `chrome.tabs.getCurrent()` | P0 | Get the current tab |
| `chrome.tabs.create()` | P1 | Create a new tab |
| `chrome.tabs.update()` | P1 | Update tab properties (url, active, pinned) |
| `chrome.tabs.remove()` | P1 | Close tabs |
| `chrome.tabs.sendMessage()` | P1 | Send message to content script in tab |

**Files to Create:**
- `WebExtensionsFramework/Sources/APIHandlers/TabsHandler.swift`
- `WebExtensionsFramework/Resources/JavaScript/chrome-tabs-api.js`
- `WebExtensionsFramework/Tests/.../TabsHandlerTests.swift`

**API Declaration to Add (BrowserExtensionAPIDeclarations.swift):**
```swift
APIDefinition(name: "tabs", templateName: "chrome-tabs-api", generator: makeChromeTabs)
```

### Priority 2: chrome.action API (High)

**Why:** Needed for toolbar button extensions. Required by test extension `custom-user-agent`.

**Methods to Implement:**

| Method | Priority | Description |
|--------|----------|-------------|
| `chrome.action.setIcon()` | P1 | Set toolbar icon |
| `chrome.action.setTitle()` | P1 | Set tooltip text |
| `chrome.action.setBadgeText()` | P1 | Set badge text |
| `chrome.action.setBadgeBackgroundColor()` | P2 | Set badge color |
| `chrome.action.onClicked.addListener()` | P1 | Handle clicks |

### Priority 3: declarativeNetRequest API (Medium)

**Why:** Required for header modification, ad blocking. Required by test extension `custom-user-agent`.

**Methods to Implement:**

| Method | Priority | Description |
|--------|----------|-------------|
| `chrome.declarativeNetRequest.updateDynamicRules()` | P1 | Add/remove dynamic rules |
| `chrome.declarativeNetRequest.getDynamicRules()` | P1 | Get current rules |

### Priority 4: Additional Manifest Fields

**Implement in ExtensionManifest.swift:**

| Field | Priority | Description |
|-------|----------|-------------|
| `icons` | P1 | Extension icons (required by spec) |
| `action` | P1 | Toolbar button config |
| `web_accessible_resources` | P2 | Extension resource access |
| `commands` | P2 | Keyboard shortcuts |
| `content_security_policy` | P2 | Security policy |

---

## Implementation Order

### Week 1: chrome.tabs Foundation
1. Create TabsHandler.swift with basic query/get/getCurrent
2. Create chrome-tabs-api.js template
3. Add TabsHandlerTests.swift with TDD
4. Wire into BrowserExtensionAPIDeclarations.swift
5. Run APIGenerator to create protocol stubs

### Week 2: chrome.tabs Complete + chrome.action Start
1. Add tabs.create, tabs.update, tabs.remove
2. Add tabs.sendMessage
3. Start chrome.action API (setIcon, setTitle, onClicked)

### Week 3: declarativeNetRequest + Manifest Fields
1. Implement declarativeNetRequest basics
2. Add missing manifest field parsing
3. Integration testing with custom-user-agent extension

---

## Architecture Notes

### How to Add a New Chrome API

1. **Define the API** in `Shared/BrowserExtensionAPIDeclarations.swift`:
   ```swift
   let chromeAPIs: [APIDefinition] = [
       // ... existing
       APIDefinition(name: "tabs", templateName: "chrome-tabs-api", generator: makeChromeTabs)
   ]
   ```

2. **Create the generator function**:
   ```swift
   func makeChromeTabs(inputs: APIInputs) -> APISequence {
       APISequence {
           Namespace("tabs", freeze: false, preventExtensions: true) {
               AsyncFunction("query", returns: [Tab].self) {
                   Argument("queryInfo", type: TabQueryInfo.self)
               }
               // ... more methods
           }
       }
   }
   ```

3. **Run the API generator**:
   ```bash
   cd WebExtensionsFramework
   swift run APIGenerator
   ```
   This generates:
   - Handler protocols in `Generated/`
   - JavaScript template bodies

4. **Implement the handler** in `Sources/APIHandlers/TabsHandler.swift`:
   ```swift
   @MainActor
   class TabsQueryHandler: TabsQueryHandlerProtocol {
       var requiredPermissions: [BrowserExtensionAPIPermission] { [.tabs] }

       func handle(request: TabsQueryRequest,
                   context: BrowserExtensionContext) async throws -> [Tab] {
           // Implementation
       }
   }
   ```

5. **Create the JavaScript template** in `Resources/JavaScript/chrome-tabs-api.js`:
   ```javascript
   ;(function() {
       'use strict';
       {{TABS_BODY}}
       Object.defineProperty(window.chrome, 'tabs', {
           value: tabs,
           writable: false,
           configurable: false,
           enumerable: true
       });
       true;
   })();
   ```

6. **Write tests** following TDD (tests first, then implementation)

### Key Files Reference

| Purpose | Location |
|---------|----------|
| API Definitions | `Shared/BrowserExtensionAPIDeclarations.swift` |
| Code Generator | `Shared/BrowserExtensionAPIGenerator.swift` |
| Handler Implementations | `Sources/APIHandlers/*.swift` |
| JavaScript Templates | `Resources/JavaScript/*.js` |
| Tests | `Tests/WebExtensionsFrameworkTests/` |
| Test Extensions | `test-extensions/` |

---

## Test Extension: custom-user-agent

The `custom-user-agent` extension in `test-extensions/` serves as the MVP target. It requires:
- chrome.tabs (query, update)
- chrome.storage (local)
- chrome.action (toolbar button)
- chrome.declarativeNetRequest (header modification)
- chrome.runtime (messaging)

When this extension runs completely, the core architecture is validated.

---

## Dependencies and Blockers

### No External Blockers

All required APIs can be implemented without external dependencies.

### Internal Dependencies

| Task | Depends On |
|------|------------|
| chrome.tabs.sendMessage | chrome.tabs.query working |
| chrome.action | Browser UI integration |
| declarativeNetRequest | WKWebView URL scheme handler |

---

## Success Criteria

Phase 2 is complete when:
1. ✅ chrome.tabs API implemented (query, get, getCurrent, create, update, remove, sendMessage)
2. ✅ chrome.action API implemented (setIcon, setTitle, setBadgeText, onClicked)
3. ✅ declarativeNetRequest basics implemented
4. ✅ custom-user-agent test extension runs without errors
5. ✅ All WebExtensionsFramework tests pass
6. ✅ DashTerm2 build and tests pass

---

## References

- [Chrome Extensions API Reference](https://developer.chrome.com/docs/extensions/reference/)
- WebExtensionsFramework/CLAUDE.md - Framework-specific practices
- WebExtensionsFramework/Documentation/manifest-fields/manifest-v3-spec.md - Manifest spec
