# GPU Renderer: Core Text Glyph Atlas Implementation Plan

**Status: COMPLETE**

> This plan was implemented in Workers #1660-1680. See `sources/DTermGlyphAtlasManager.swift`,
> `sources/DTermCore.swift` (platform glyph methods), and `sources/DTermMetalView.swift`
> (`usePlatformGlyphAtlas` method) for the implementation.

## Problem Statement

The current GPU renderer requires raw TTF/OTF font file data, which fails for macOS system fonts (Monaco, Menlo, SF Mono) that don't have accessible file URLs. The current workaround substitutes JetBrains Mono, which doesn't respect user font preferences.

## Solution: Platform-Side Glyph Atlas

Use Core Text to render glyphs directly to a texture atlas on the Swift side, bypassing the need for font file data entirely. This is the approach used by Alacritty, kitty, and WezTerm.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│  Swift (DTermGlyphAtlasManager)                             │
│  - CTFontDrawGlyphs renders glyph to bitmap                 │
│  - Packs glyphs into MTLTexture atlas                       │
│  - Tracks glyph metrics (advance, bearing, UV coords)       │
│  - Provides atlas texture to Metal pipeline                 │
└───────────────────────────────┬─────────────────────────────┘
                                │ Character → GlyphEntry mapping
                                ▼
┌─────────────────────────────────────────────────────────────┐
│  dterm-core (Rust) - MODIFIED                               │
│  - Receives glyph entries from Swift (codepoint → UV rect)  │
│  - Generates vertex data with Swift-provided UVs            │
│  - NO font file loading, NO internal rasterization          │
└─────────────────────────────────────────────────────────────┘
```

## Implementation Steps

### Step 1: Create DTermGlyphAtlasManager.swift

New Swift class that manages glyph rendering and atlas packing:

```swift
// sources/DTermGlyphAtlasManager.swift

import AppKit
import Metal
import CoreText

/// Manages a glyph texture atlas using Core Text for rendering.
/// Works with ANY macOS font, including system fonts without file URLs.
@MainActor
public final class DTermGlyphAtlasManager {

    // MARK: - Types

    /// Glyph entry with atlas position and metrics.
    public struct GlyphEntry {
        public let codepoint: UInt32
        public let x: UInt16          // X offset in atlas (pixels)
        public let y: UInt16          // Y offset in atlas (pixels)
        public let width: UInt16      // Glyph width (pixels)
        public let height: UInt16     // Glyph height (pixels)
        public let bearingX: Int16    // Left side bearing (pixels)
        public let bearingY: Int16    // Top bearing (pixels)
        public let advance: UInt16    // Horizontal advance (pixels)
    }

    // MARK: - Properties

    /// The Metal device for texture creation.
    private let device: MTLDevice

    /// Current font for glyph rendering.
    private var font: CTFont?

    /// Font size in points.
    private var fontSize: CGFloat = 12.0

    /// Glyph cache: codepoint -> entry.
    private var glyphCache: [UInt32: GlyphEntry] = [:]

    /// Atlas texture (R8 grayscale).
    private(set) var atlasTexture: MTLTexture?

    /// Atlas bitmap data (for incremental updates).
    private var atlasBitmap: UnsafeMutableRawPointer?

    /// Current atlas size in pixels (always square, power of 2).
    private(set) var atlasSize: Int = 512

    /// Next available position in atlas (row-major packing).
    private var nextX: Int = 0
    private var nextY: Int = 0
    private var currentRowHeight: Int = 0

    /// Cell dimensions computed from font metrics.
    private(set) var cellWidth: CGFloat = 0
    private(set) var cellHeight: CGFloat = 0

    // MARK: - Initialization

    public init(device: MTLDevice, initialSize: Int = 512) {
        self.device = device
        self.atlasSize = initialSize
        createAtlasTexture()
    }

    deinit {
        if let bitmap = atlasBitmap {
            free(bitmap)
        }
    }

    // MARK: - Font Configuration

    /// Set the font for glyph rendering.
    /// - Parameters:
    ///   - font: NSFont to use
    /// - Returns: true on success
    public func setFont(_ font: NSFont) -> Bool {
        self.font = font as CTFont
        self.fontSize = font.pointSize

        // Calculate cell dimensions from font metrics
        let ascent = CTFontGetAscent(self.font!)
        let descent = CTFontGetDescent(self.font!)
        let leading = CTFontGetLeading(self.font!)

        cellHeight = ceil(ascent + descent + leading)

        // Get advance of 'M' for cell width (or average char width)
        var glyph: CGGlyph = 0
        var char: UniChar = 0x4D // 'M'
        CTFontGetGlyphsForCharacters(self.font!, &char, &glyph, 1)

        var advance = CGSize.zero
        CTFontGetAdvancesForGlyphs(self.font!, .horizontal, &glyph, &advance, 1)
        cellWidth = ceil(advance.width)

        // Clear cache - new font means new glyphs
        glyphCache.removeAll()
        nextX = 0
        nextY = 0
        currentRowHeight = 0

        // Pre-render ASCII printable characters (32-126)
        prerenderASCII()

        return true
    }

    // MARK: - Glyph Rendering

    /// Get or render a glyph entry for a codepoint.
    public func getGlyph(codepoint: UInt32) -> GlyphEntry? {
        // Check cache first
        if let cached = glyphCache[codepoint] {
            return cached
        }

        // Render and cache
        return renderGlyph(codepoint: codepoint)
    }

    /// Render a glyph and add to atlas.
    private func renderGlyph(codepoint: UInt32) -> GlyphEntry? {
        guard let font = font else { return nil }

        // Convert codepoint to glyph ID
        var glyph: CGGlyph = 0
        if codepoint <= 0xFFFF {
            var char = UniChar(codepoint)
            if !CTFontGetGlyphsForCharacters(font, &char, &glyph, 1) {
                // Use replacement character for missing glyphs
                var replacement: UniChar = 0xFFFD
                CTFontGetGlyphsForCharacters(font, &replacement, &glyph, 1)
            }
        } else {
            // Handle surrogate pairs for characters > U+FFFF
            let high = UniChar((codepoint - 0x10000) >> 10) + 0xD800
            let low = UniChar((codepoint - 0x10000) & 0x3FF) + 0xDC00
            var chars: [UniChar] = [high, low]
            var glyphs: [CGGlyph] = [0, 0]
            CTFontGetGlyphsForCharacters(font, &chars, &glyphs, 2)
            glyph = glyphs[0]
        }

        // Get glyph metrics
        var bounds = CGRect.zero
        CTFontGetBoundingRectsForGlyphs(font, .horizontal, &glyph, &bounds, 1)

        var advance = CGSize.zero
        CTFontGetAdvancesForGlyphs(font, .horizontal, &glyph, &advance, 1)

        // Calculate glyph dimensions with padding
        let padding: CGFloat = 1
        let glyphWidth = Int(ceil(bounds.width + padding * 2))
        let glyphHeight = Int(ceil(bounds.height + padding * 2))

        guard glyphWidth > 0 && glyphHeight > 0 else {
            // Empty glyph (space, etc.)
            let entry = GlyphEntry(
                codepoint: codepoint,
                x: 0, y: 0, width: 0, height: 0,
                bearingX: 0, bearingY: 0,
                advance: UInt16(ceil(advance.width))
            )
            glyphCache[codepoint] = entry
            return entry
        }

        // Allocate position in atlas
        guard let position = allocateAtlasSpace(width: glyphWidth, height: glyphHeight) else {
            // Atlas full - need to grow
            growAtlas()
            return renderGlyph(codepoint: codepoint) // Retry
        }

        // Render glyph to bitmap
        let bitmap = renderGlyphToBitmap(
            font: font,
            glyph: glyph,
            bounds: bounds,
            width: glyphWidth,
            height: glyphHeight
        )

        // Copy to atlas
        copyToAtlas(bitmap: bitmap, x: position.x, y: position.y, width: glyphWidth, height: glyphHeight)

        // Create entry
        let entry = GlyphEntry(
            codepoint: codepoint,
            x: UInt16(position.x),
            y: UInt16(position.y),
            width: UInt16(glyphWidth),
            height: UInt16(glyphHeight),
            bearingX: Int16(bounds.origin.x - padding),
            bearingY: Int16(bounds.origin.y + bounds.height + padding),
            advance: UInt16(ceil(advance.width))
        )

        glyphCache[codepoint] = entry
        return entry
    }

    /// Render a single glyph to a grayscale bitmap.
    private func renderGlyphToBitmap(
        font: CTFont,
        glyph: CGGlyph,
        bounds: CGRect,
        width: Int,
        height: Int
    ) -> [UInt8] {
        var bitmap = [UInt8](repeating: 0, count: width * height)

        bitmap.withUnsafeMutableBytes { ptr in
            guard let context = CGContext(
                data: ptr.baseAddress,
                width: width,
                height: height,
                bitsPerComponent: 8,
                bytesPerRow: width,
                space: CGColorSpaceCreateDeviceGray(),
                bitmapInfo: CGImageAlphaInfo.none.rawValue
            ) else { return }

            // Setup antialiasing
            context.setAllowsAntialiasing(true)
            context.setShouldAntialias(true)
            context.setShouldSmoothFonts(true)
            context.setAllowsFontSmoothing(true)

            // Set white color for glyph (grayscale = alpha)
            context.setFillColor(gray: 1.0, alpha: 1.0)

            // Position glyph
            var pos = CGPoint(
                x: -bounds.origin.x + 1,  // +1 for padding
                y: -bounds.origin.y + 1
            )

            // Draw glyph
            var glyphCopy = glyph
            CTFontDrawGlyphs(font, &glyphCopy, &pos, 1, context)
        }

        return bitmap
    }

    // MARK: - Atlas Management

    /// Allocate space in the atlas for a glyph.
    private func allocateAtlasSpace(width: Int, height: Int) -> (x: Int, y: Int)? {
        // Check if glyph fits on current row
        if nextX + width <= atlasSize {
            let pos = (x: nextX, y: nextY)
            nextX += width
            currentRowHeight = max(currentRowHeight, height)
            return pos
        }

        // Move to next row
        nextX = 0
        nextY += currentRowHeight
        currentRowHeight = 0

        // Check if fits
        if nextY + height > atlasSize {
            return nil // Atlas full
        }

        let pos = (x: nextX, y: nextY)
        nextX += width
        currentRowHeight = height
        return pos
    }

    /// Copy glyph bitmap to atlas.
    private func copyToAtlas(bitmap: [UInt8], x: Int, y: Int, width: Int, height: Int) {
        guard let atlasBitmap = atlasBitmap else { return }

        for row in 0..<height {
            let srcOffset = row * width
            let dstOffset = (y + row) * atlasSize + x
            memcpy(atlasBitmap.advanced(by: dstOffset), bitmap.withUnsafeBytes { $0.baseAddress!.advanced(by: srcOffset) }, width)
        }

        // Update Metal texture
        updateTextureRegion(x: x, y: y, width: width, height: height, data: bitmap)
    }

    /// Update a region of the Metal texture.
    private func updateTextureRegion(x: Int, y: Int, width: Int, height: Int, data: [UInt8]) {
        guard let texture = atlasTexture else { return }

        let region = MTLRegion(
            origin: MTLOrigin(x: x, y: y, z: 0),
            size: MTLSize(width: width, height: height, depth: 1)
        )

        data.withUnsafeBytes { ptr in
            texture.replace(
                region: region,
                mipmapLevel: 0,
                withBytes: ptr.baseAddress!,
                bytesPerRow: width
            )
        }
    }

    /// Create the atlas texture.
    private func createAtlasTexture() {
        let descriptor = MTLTextureDescriptor.texture2DDescriptor(
            pixelFormat: .r8Unorm,
            width: atlasSize,
            height: atlasSize,
            mipmapped: false
        )
        descriptor.usage = [.shaderRead]
        descriptor.storageMode = .shared

        atlasTexture = device.makeTexture(descriptor: descriptor)

        // Allocate bitmap for CPU-side atlas
        if let existing = atlasBitmap {
            free(existing)
        }
        atlasBitmap = calloc(atlasSize * atlasSize, 1)
    }

    /// Grow the atlas when full.
    private func growAtlas() {
        let newSize = atlasSize * 2
        guard newSize <= 8192 else {
            DLog("DTermGlyphAtlasManager: Cannot grow atlas beyond 8192x8192")
            return
        }

        DLog("DTermGlyphAtlasManager: Growing atlas from \(atlasSize) to \(newSize)")

        // Save old data
        let oldSize = atlasSize
        let oldBitmap = atlasBitmap

        // Create new atlas
        atlasSize = newSize
        createAtlasTexture()

        // Copy old data to new atlas
        if let oldBitmap = oldBitmap, let newBitmap = atlasBitmap {
            for row in 0..<oldSize {
                memcpy(
                    newBitmap.advanced(by: row * newSize),
                    oldBitmap.advanced(by: row * oldSize),
                    oldSize
                )
            }
            free(oldBitmap)
        }

        // Upload entire new atlas to texture
        if let texture = atlasTexture, let bitmap = atlasBitmap {
            let region = MTLRegion(
                origin: MTLOrigin(x: 0, y: 0, z: 0),
                size: MTLSize(width: newSize, height: newSize, depth: 1)
            )
            texture.replace(
                region: region,
                mipmapLevel: 0,
                withBytes: bitmap,
                bytesPerRow: newSize
            )
        }
    }

    /// Pre-render ASCII printable characters.
    private func prerenderASCII() {
        for codepoint in UInt32(32)...UInt32(126) {
            _ = renderGlyph(codepoint: codepoint)
        }
    }

    // MARK: - UV Coordinate Helpers

    /// Get normalized UV coordinates for a glyph entry.
    public func getUVRect(for entry: GlyphEntry) -> (u0: Float, v0: Float, u1: Float, v1: Float) {
        let size = Float(atlasSize)
        return (
            u0: Float(entry.x) / size,
            v0: Float(entry.y) / size,
            u1: Float(entry.x + entry.width) / size,
            v1: Float(entry.y + entry.height) / size
        )
    }
}
```

### Step 2: Modify dterm-core FFI (Rust)

Add new FFI functions in `~/dterm/crates/dterm-core/src/ffi/hybrid_renderer.rs`:

```rust
// NEW: Set glyph entry from platform-rendered bitmap
#[no_mangle]
pub unsafe extern "C" fn dterm_hybrid_renderer_set_platform_glyph(
    renderer: *mut DtermHybridRenderer,
    codepoint: u32,
    x: u16,
    y: u16,
    width: u16,
    height: u16,
    bearing_x: i16,
    bearing_y: i16,
    advance: u16,
) -> bool {
    // Store glyph entry without rasterization
    // Renderer will use these UV coords when generating vertices
}

// NEW: Set atlas size (platform manages texture)
#[no_mangle]
pub unsafe extern "C" fn dterm_hybrid_renderer_set_atlas_size(
    renderer: *mut DtermHybridRenderer,
    size: u32,
) -> bool {
    // Store atlas size for UV calculation
}

// NEW: Set cell dimensions (computed by platform from font metrics)
#[no_mangle]
pub unsafe extern "C" fn dterm_hybrid_renderer_set_cell_size(
    renderer: *mut DtermHybridRenderer,
    width: f32,
    height: f32,
) -> bool {
    // Store cell size for vertex generation
}

// MODIFY: Build should use platform-provided glyph entries
// No internal rasterization needed
```

### Step 3: Update DTermCore.swift Integration

Modify `DTermHybridRenderer` in `sources/DTermCore.swift`:

```swift
extension DTermHybridRenderer {
    /// Set font using platform-side glyph atlas (works with ANY font).
    public func setFontWithAtlas(_ atlasManager: DTermGlyphAtlasManager) -> Bool {
        guard let handle = handle else { return false }

        // Tell Rust the cell dimensions
        dterm_hybrid_renderer_set_cell_size(
            handle,
            Float(atlasManager.cellWidth),
            Float(atlasManager.cellHeight)
        )

        // Tell Rust the atlas size
        dterm_hybrid_renderer_set_atlas_size(
            handle,
            UInt32(atlasManager.atlasSize)
        )

        // Upload all cached glyph entries to Rust
        for (codepoint, entry) in atlasManager.glyphCache {
            dterm_hybrid_renderer_set_platform_glyph(
                handle,
                codepoint,
                entry.x,
                entry.y,
                entry.width,
                entry.height,
                entry.bearingX,
                entry.bearingY,
                entry.advance
            )
        }

        return true
    }
}
```

### Step 4: Update DTermMetalView.swift

Modify to use the new glyph atlas manager:

```swift
// Add property
private var glyphAtlasManager: DTermGlyphAtlasManager?

// Modify setFont
@objc @discardableResult
public func setFont(_ font: NSFont) -> Bool {
    guard let device = metalDevice else { return false }

    // Create glyph atlas manager if needed
    if glyphAtlasManager == nil {
        glyphAtlasManager = DTermGlyphAtlasManager(device: device)
    }

    // Configure font in atlas manager
    guard glyphAtlasManager?.setFont(font) == true else { return false }

    // Sync with hybrid renderer
    return hybridRenderer?.setFontWithAtlas(glyphAtlasManager!) ?? false
}

// Modify render() to use atlas manager's texture
private func render() {
    // ...existing code...

    // Use platform-managed atlas texture instead of Rust-managed
    if let atlas = glyphAtlasManager?.atlasTexture {
        encoder.setFragmentTexture(atlas, index: 0)
    }

    // ...existing code...
}
```

## File Changes Summary

| File | Change Type | Description |
|------|-------------|-------------|
| `sources/DTermGlyphAtlasManager.swift` | NEW | Swift glyph atlas manager using Core Text |
| `~/dterm/crates/dterm-core/src/ffi/hybrid_renderer.rs` | MODIFY | Add platform glyph entry FFI functions |
| `~/dterm/crates/dterm-core/src/hybrid_renderer.rs` | MODIFY | Store platform glyph entries, skip internal rasterization |
| `DTermCore/include/dterm.h` | MODIFY | Add new FFI function declarations |
| `sources/DTermCore.swift` | MODIFY | Add `setFontWithAtlas` method |
| `sources/DTermMetalView.swift` | MODIFY | Integrate glyph atlas manager |

## Benefits

1. **Works with ANY font** - System fonts, user fonts, emoji, etc.
2. **Respects user preferences** - Renders the actual selected font
3. **No file URL extraction** - Uses Core Text APIs directly
4. **Proper antialiasing** - Uses macOS font smoothing
5. **Matches proven approach** - Same technique as Alacritty/kitty/WezTerm

## Testing Checklist

- [ ] Monaco (system font without file URL)
- [ ] Menlo (system font)
- [ ] SF Mono (system font)
- [ ] JetBrains Mono (user-installed font with file URL)
- [ ] Emoji characters (Apple Color Emoji)
- [ ] Unicode characters (CJK, Arabic, etc.)
- [ ] Font size changes
- [ ] Window resize (atlas regrowth)
- [ ] Performance: 60 FPS with large scrollback

## Worker Instructions

1. **First**: Implement `DTermGlyphAtlasManager.swift` exactly as specified
2. **Second**: Add FFI functions to dterm-core (in `~/dterm/` repo)
3. **Third**: Rebuild dterm-core: `cd ~/dterm && cargo build --release -p dterm-core --features ffi`
4. **Fourth**: Run `scripts/build-dterm-core.sh` to copy updated lib/header
5. **Fifth**: Update Swift integration in `DTermCore.swift`
6. **Sixth**: Update `DTermMetalView.swift`
7. **Seventh**: Build and test with Monaco font

**CRITICAL**: Do NOT modify the bundled font fallback approach until this is working. The fallback is a safety net.
