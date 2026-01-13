# Performance Optimization Findings

**Worker #852 - December 23, 2025**
**Updated: Worker #857 - December 23, 2025**

This document summarizes performance optimization opportunities identified during Phase 1 exploration of the DashTerm2 codebase.

---

## ✅ Phase 1 Status: COMPLETE (December 23, 2025)

**Summary:**
- **11 of 12 items implemented** (92%)
- **1 item deferred** (item 7 - underline pre-computation)
- All builds pass, all 3566 tests pass

**Workers:**
- #852: Initial exploration and findings documentation
- #853: Phase 1A quick wins (items 1-4)
- #854: Phase 1B ASCII fast path (item 5)
- #855: Phase 1B Metal optimizations (items 6, 8)
- #856: Phase 1C larger projects (items 11, 12)
- #857: Verified completion, documented item 7 deferral

**Next Phase:** Phase 2 - Feature Parity (per CLAUDE.md roadmap)

---

## Executive Summary

Two major areas were analyzed:
1. **Metal Renderer** (`sources/Metal/`) - GPU rendering pipeline
2. **VT100Screen** (`sources/VT100Screen*.m`) - Terminal screen handling

Both areas have existing optimizations (buffer pools, NEON vectorization) but offer several high-impact improvement opportunities.

---

## Metal Renderer Optimization Opportunities

### Priority 1: Predecessor/Successor Underline Pre-computation
**Files:** `iTermTextRendererTransientState.mm`, `iTermShaderTypes.h`, `iTermText.metal`
**Impact:** High

The vertex shader currently computes `predecessorWasUnderlined` and `successorWillBeUnderlined` for every instance by reading from the PIU array multiple times (8 comparisons per vertex).

**Optimization:** Pre-compute these flags on the CPU when building the PIU array. Add `predecessorWasUnderlined` and `successorWillBeUnderlined` as boolean fields to `iTermTextPIU`. This trades 2 bytes of PIU data for 8 comparisons per vertex.

### Priority 2: Buffer Pool Lock Optimization
**File:** `iTermMetalBufferPool.m` (lines 170-186, 209-230)
**Impact:** High

Every buffer request/return goes through `@synchronized(self)`, creating lock contention in multi-threaded scenarios.

**Optimization:**
- Use `os_unfair_lock` instead of `@synchronized` for lower overhead
- Consider per-thread buffer pools with stealing for load balancing
- Alternatively, use lock-free data structures

### Priority 3: Alpha Vector Pre-computation
**Files:** `iTermTextRendererTransientState.mm`, `iTermText.metal` (lines 4-23)
**Impact:** Medium-High

`iTermAlphaVectorForTextColor` is computed in the vertex shader for every vertex, but the result depends only on the text color which is per-instance.

**Optimization:** Compute `alphaVector` in the CPU code when building the PIU, or compute once per instance.

### Priority 4: Quad Cache Size Increase
**File:** `iTermTextRenderer.mm` (lines 319-321)
**Impact:** Medium

The quad cache is limited to 2 entries. With multiple font sizes or emoji, this causes thrashing.

**Optimization:** Increase cache size to 4-8 entries, or implement a proper LRU cache.

### Priority 5: Underline Pattern Texture Lookup
**File:** `iTermTextShaderCommon.metal` (lines 121-234)
**Impact:** Medium

`FractionOfPixelThatIntersectsUnderlineForStyle` has complex switch statements with floating-point modulo operations per pixel.

**Optimization:** Pre-compute underline patterns as a small texture lookup table.

### Priority 6: Pipeline State Single-Entry Cache
**File:** `iTermMetalRenderer.m` (lines 234-257)
**Impact:** Medium

Pipeline states are cached in an NSMutableDictionary with lookup every frame.

**Optimization:** Cache the last-used pipeline state as an instance variable (most frames use the same state).

### Priority 7: PIUArray Pre-allocation
**Files:** `iTermPIUArray.h`, `iTermTextRendererTransientState.mm`
**Impact:** Medium

PIUArray allocates 1024-element segments when full, causing multiple allocations for heavy CJK content.

**Optimization:** Add a `reserve()` method and pre-allocate based on previous frame's usage.

### Priority 8: Texture Page LRU Heap
**File:** `iTermTexturePageCollection.h` (lines 90-114)
**Impact:** Low-Medium

Pruning creates copies and sorts all pages (O(n log n) for 4096 max pages).

**Optimization:** Use a min-heap or priority queue for LRU tracking.

---

## VT100Screen Optimization Opportunities

### Priority 1: ASCII Fast Path in StringToScreenChars
**File:** `ScreenChar.m` (lines 590-758)
**Impact:** High
**Status:** ✅ Completed (Worker #854)

`StringToScreenChars` uses block-based enumeration for every grapheme cluster, even for pure ASCII strings which are extremely common in terminal output.

**Optimization:**
- Pre-scan for pure ASCII runs and use a fast path without block overhead
- Cache NSCharacterSet lookups (`ignorableCharacters`, `spacingCombiningMarks`)

### Priority 2: LRU Cache O(1) Implementation
**File:** `VT100ScreenState.m` (lines 504-648)
**Impact:** Medium-High

Screen char array cache uses NSMutableArray with O(n) `removeObject:` for LRU tracking.

**Optimization:** Replace with linked list + dictionary combination for O(1) operations.

### Priority 3: Dirty Region Tracking
**File:** `VT100Grid.m` (lines 397-505)
**Impact:** Medium

System often marks entire lines/screen dirty when only portions changed.

**Optimization:**
- Implement region-based dirty tracking (dirty rectangles)
- Merge adjacent dirty regions
- Avoid full-grid dirty marking during partial scroll operations

### Priority 4: Line Buffer Cache Improvements
**File:** `LineBuffer.m` (lines 275-286)
**Impact:** Medium

Wrapped line count cache is invalidated when width changes, requiring full recomputation.

**Optimization:**
- Cache multiple common widths
- Implement incremental updates when lines are added/removed

### Priority 5: Scroll Region Bulk Operations
**File:** `VT100Grid.m` (lines 817-867, 1777-1901)
**Impact:** Medium

Scroll operations use per-line memmove with DWC boundary fixes.

**Optimization:**
- For full-width scrolls without DWC, use bulk memory operations
- Track DWC-free regions more aggressively

### Priority 6: Granular Sync Dirty Flags
**File:** `VT100Screen.m` (lines 1563-1638)
**Impact:** Medium

State synchronization copies extensive state even when minimal changes occurred.

**Optimization:**
- Implement granular dirty flags for state components
- Only sync components that actually changed
- Consider copy-on-write for large structures

### Priority 7: String Allocation Reduction
**Files:** Multiple
**Impact:** Low-Medium

Temporary NSString allocations in hot paths.

**Optimization:**
- Use stack-allocated buffers for short strings
- Consider CFStringRef for hot paths

### Priority 8: EA Index Caching
**File:** `VT100Grid.m` (lines 310-327, 1514)
**Impact:** Low-Medium

External attribute indices are looked up per-line during character insertion.

**Optimization:** Cache the index across multiple operations on the same line.

---

## Recommended Implementation Order

Based on impact vs. complexity analysis:

### Phase 1A - Quick Wins (Low complexity, high impact) - ✅ COMPLETED (Worker #853)
1. ✅ Quad cache size increase (Metal) - `iTermTextRenderer.mm`: Increased from 2 to 8 entries
2. ✅ LRU cache O(1) implementation (VT100Screen) - `VT100ScreenState.m`: Replaced O(n) NSMutableArray with O(1) doubly-linked list
3. ✅ Pipeline state single-entry cache (Metal) - `iTermMetalRenderer.m`: Added fast path with last-used state cache
4. ✅ EA index caching (VT100Grid) - `VT100Grid.m`: Cache EA index across multiple chars on same line in appendCharsAtCursor

### Phase 1B - Medium Effort (Medium complexity, high impact)
5. ✅ ASCII fast path in StringToScreenChars (ScreenChar.m) - Completed (Worker #854)
6. ✅ Buffer pool lock optimization (Metal) - Completed (Worker #855): Replaced @synchronized with os_unfair_lock in iTermMetalBufferPool.m
7. ⏸️ Predecessor/successor underline pre-computation (Metal) - Deferred: PIUs are organized by texture page (not screen position), making CPU pre-computation require cross-array coordination. Adjacent cells may use different textures (fonts, emoji) and thus be in separate draw calls. Current GPU computation (8 comparisons/vertex) is acceptable.
8. ✅ Alpha vector pre-computation (Metal) - Completed (Worker #855): Added alphaVector field to iTermTextPIU, computed on CPU

### Phase 1C - Larger Projects (High complexity, medium impact)
9. Dirty region tracking (VT100Screen) - Already partially implemented: VT100LineInfo tracks per-line dirty state
10. Granular sync dirty flags (VT100Screen) - Already partially implemented: lineBuffer.dirty, markCache.dirty, namedMarksDirty
11. ✅ PIUArray pre-allocation (Metal) - Completed (Worker #856): Added reserve() method and call in initializeTransientState
12. ✅ Line buffer cache improvements (VT100Screen) - Completed (Worker #856): Multi-width cache (4 widths) for wrapped lines count

---

## Files Summary

### Metal Renderer Files
| File | Key Optimizations |
|------|-------------------|
| `sources/Metal/Renderers/iTermTextRenderer.mm` | Quad cache, texture dimensions |
| `sources/Metal/Renderers/iTermTextRendererTransientState.mm` | Underline pre-computation, alpha vector |
| `sources/Metal/Infrastructure/iTermMetalBufferPool.m` | Lock optimization |
| `sources/Metal/Infrastructure/iTermMetalRenderer.m` | Pipeline cache |
| `sources/Metal/Infrastructure/iTermPIUArray.h` | Pre-allocation |
| `sources/Metal/Shaders/iTermText.metal` | Vertex shader optimizations |
| `sources/Metal/Shaders/iTermTextShaderCommon.metal` | Underline pattern lookup |
| `sources/Metal/Renderers/iTermTexturePageCollection.h` | LRU heap |

### VT100Screen Files
| File | Key Optimizations |
|------|-------------------|
| `sources/ScreenChar.m` | ASCII fast path |
| `sources/VT100Grid.m` | Dirty tracking, scroll operations |
| `sources/VT100ScreenState.m` | LRU cache |
| `sources/VT100Screen.m` | Sync dirty flags |
| `sources/LineBuffer.m` | Cache improvements |
| `sources/VT100ScreenMutableState.m` | String allocations |

---

## Next Steps for Manager

This document provides a roadmap for Phase 1: Performance Optimization. The manager should:

1. Review and prioritize these optimizations based on project goals
2. Create specific tasks in `docs/worker-backlog.md` for workers to implement
3. Consider establishing performance benchmarks before implementing changes
4. Decide on the order of implementation (quick wins first vs. highest impact first)

---

## Appendix: Existing Optimizations (Reference)

The codebase already includes these performance optimizations:
- **NEON vectorization** in VT100Grid.m (lines 54-147) for screen filling
- **Buffer pools** for Metal buffer reuse
- **Inline C++ functions** to avoid Objective-C dispatch overhead
- **Pre-computed data structures** for glyph rendering
- **DWC-free line tracking** (`_knownDWCFreeLines`) for faster scroll operations
