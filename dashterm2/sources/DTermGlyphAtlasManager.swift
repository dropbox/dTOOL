//
//  DTermGlyphAtlasManager.swift
//  DashTerm2
//
//  Manages a glyph texture atlas using Core Text for rendering.
//  Works with ANY macOS font, including system fonts without file URLs.
//
//  This replaces the font-file-based approach with platform-native glyph
//  rendering, which is how Alacritty, kitty, and WezTerm handle fonts.
//
//  Created by DashTerm2 AI Worker on 2024-12-30.
//

import AppKit
import Metal
import CoreText

/// Manages a glyph texture atlas using Core Text for rendering.
///
/// ## Overview
///
/// This class provides high-quality glyph rendering for the GPU terminal renderer
/// using macOS Core Text APIs. Unlike font-file-based approaches, this works with:
///
/// - System fonts (Monaco, Menlo, SF Mono) that don't have accessible file URLs
/// - User-installed fonts
/// - Emoji (Apple Color Emoji)
/// - Any Unicode character supported by the font
///
/// ## Architecture
///
/// ```
/// Character → CTFont glyph ID → CGContext render → Atlas texture → GPU shader
/// ```
///
/// The atlas uses a simple row-based packing algorithm. When full, it grows
/// to the next power of 2 (up to 8192x8192).
///
/// ## Thread Safety
///
/// This class is designed for use on the main thread only (@MainActor).
///
@MainActor
public final class DTermGlyphAtlasManager {

    // MARK: - Types

    /// Glyph entry with atlas position and font metrics.
    public struct GlyphEntry: Equatable {
        public static func == (lhs: GlyphEntry, rhs: GlyphEntry) -> Bool {
            lhs.codepoint == rhs.codepoint && lhs.x == rhs.x && lhs.y == rhs.y &&
            lhs.width == rhs.width && lhs.height == rhs.height &&
            lhs.bearingX == rhs.bearingX && lhs.bearingY == rhs.bearingY &&
            lhs.advance == rhs.advance
        }
        /// Unicode codepoint this entry represents.
        public let codepoint: UInt32

        /// X offset in atlas texture (pixels).
        public let x: UInt16

        /// Y offset in atlas texture (pixels).
        public let y: UInt16

        /// Glyph bitmap width (pixels).
        public let width: UInt16

        /// Glyph bitmap height (pixels).
        public let height: UInt16

        /// Horizontal bearing (pixels from origin to left edge).
        public let bearingX: Int16

        /// Vertical bearing (pixels from baseline to top edge).
        public let bearingY: Int16

        /// Horizontal advance width (pixels to next character).
        public let advance: UInt16

        /// UV coordinates in atlas (normalized 0-1).
        public var uvRect: (u0: Float, v0: Float, u1: Float, v1: Float) = (0, 0, 0, 0)
    }

    // MARK: - Configuration

    /// Maximum atlas size (8192x8192 is the Metal limit on most devices).
    private static let maxAtlasSize: Int = 8192

    /// Initial atlas size.
    public static let initialAtlasSize: Int = 512

    /// Padding between glyphs to prevent texture bleeding.
    private static let glyphPadding: Int = 1

    // MARK: - Properties

    /// The Metal device for texture creation.
    private let device: MTLDevice

    /// Current font for glyph rendering.
    private var font: CTFont?

    /// Font size in points.
    private(set) var fontSize: CGFloat = 12.0

    /// Glyph cache: codepoint -> entry.
    private(set) var glyphCache: [UInt32: GlyphEntry] = [:]

    /// Atlas texture (R8 grayscale).
    private(set) var atlasTexture: MTLTexture?

    /// Atlas bitmap data (for CPU-side operations).
    private var atlasBitmap: UnsafeMutablePointer<UInt8>?

    /// Current atlas size in pixels (always square, power of 2).
    private(set) var atlasSize: Int = 512

    /// Next available X position in atlas.
    private var nextX: Int = 0

    /// Next available Y position in atlas.
    private var nextY: Int = 0

    /// Height of the current row being packed.
    private var currentRowHeight: Int = 0

    /// Cell width computed from font metrics.
    private(set) var cellWidth: CGFloat = 0

    /// Cell height computed from font metrics.
    private(set) var cellHeight: CGFloat = 0

    /// Font ascent (distance from baseline to top of tallest glyph).
    private(set) var fontAscent: CGFloat = 0

    /// Font descent (distance from baseline to bottom of lowest glyph).
    private(set) var fontDescent: CGFloat = 0

    /// Font leading (recommended line spacing).
    private(set) var fontLeading: CGFloat = 0

    /// Whether the atlas has been modified since last sync.
    private var isDirty: Bool = false

    /// Pending glyph entries that need to be synced to Rust.
    private(set) var pendingGlyphEntries: [GlyphEntry] = []

    // MARK: - Initialization

    /// Create a new glyph atlas manager.
    ///
    /// - Parameters:
    ///   - device: Metal device for texture creation
    ///   - initialSize: Initial atlas size (default 512, must be power of 2)
    public init(device: MTLDevice, initialSize: Int = 512) {
        self.device = device
        self.atlasSize = initialSize
        createAtlasTexture()
    }

    deinit {
        if let bitmap = atlasBitmap {
            bitmap.deallocate()
        }
    }

    // MARK: - Font Configuration

    /// Set the font for glyph rendering.
    ///
    /// This clears the existing glyph cache and pre-renders ASCII characters.
    ///
    /// - Parameter font: NSFont to use for rendering
    /// - Returns: true on success
    @discardableResult
    public func setFont(_ font: NSFont) -> Bool {
        self.font = font as CTFont
        self.fontSize = font.pointSize

        // Calculate font metrics
        fontAscent = CTFontGetAscent(self.font!)
        fontDescent = CTFontGetDescent(self.font!)
        fontLeading = CTFontGetLeading(self.font!)

        // Cell height from font metrics
        cellHeight = ceil(fontAscent + fontDescent + fontLeading)

        // Cell width from 'M' character advance (standard for monospace)
        cellWidth = calculateCellWidth()

        DLog("DTermGlyphAtlasManager: Font set to \(font.fontName) \(fontSize)pt")
        DLog("  Cell size: \(cellWidth) x \(cellHeight)")
        DLog("  Metrics: ascent=\(fontAscent) descent=\(fontDescent) leading=\(fontLeading)")

        // Clear existing cache
        resetAtlas()

        // Pre-render ASCII printable characters (32-126)
        prerenderASCII()

        return true
    }

    /// Calculate cell width from font metrics.
    private func calculateCellWidth() -> CGFloat {
        guard let font = font else { return 8.0 }

        // Try 'M' first (standard for monospace width)
        var width = advanceWidth(for: 0x4D) // 'M'

        // Fallback to '0' if 'M' gives weird results
        if width <= 0 {
            width = advanceWidth(for: 0x30) // '0'
        }

        // Fallback to font's average character width estimate
        if width <= 0 {
            let bbox = CTFontGetBoundingBox(font)
            width = bbox.width / 2 // Rough estimate
        }

        return ceil(width)
    }

    /// Get advance width for a single codepoint.
    private func advanceWidth(for codepoint: UInt32) -> CGFloat {
        guard let font = font else { return 0 }

        var glyph: CGGlyph = 0
        var char = UniChar(codepoint)
        guard CTFontGetGlyphsForCharacters(font, &char, &glyph, 1) else { return 0 }

        var advance = CGSize.zero
        CTFontGetAdvancesForGlyphs(font, .horizontal, &glyph, &advance, 1)
        return advance.width
    }

    // MARK: - Glyph Access

    /// Get or render a glyph entry for a codepoint.
    ///
    /// - Parameter codepoint: Unicode codepoint
    /// - Returns: Glyph entry, or nil if rendering failed
    public func getGlyph(codepoint: UInt32) -> GlyphEntry? {
        // Check cache first
        if let cached = glyphCache[codepoint] {
            return cached
        }

        // Render and cache
        return renderGlyph(codepoint: codepoint)
    }

    /// Check if a glyph is already cached.
    public func hasGlyph(codepoint: UInt32) -> Bool {
        return glyphCache[codepoint] != nil
    }

    /// Get all cached glyph entries.
    public var allGlyphEntries: [GlyphEntry] {
        return Array(glyphCache.values)
    }

    // MARK: - Glyph Rendering

    /// Render a glyph and add to atlas.
    private func renderGlyph(codepoint: UInt32) -> GlyphEntry? {
        guard let font = font else { return nil }

        // Convert codepoint to glyph ID
        let glyph = glyphForCodepoint(codepoint, font: font)

        // Get glyph metrics
        var glyphCopy = glyph
        var bounds = CGRect.zero
        CTFontGetBoundingRectsForGlyphs(font, .horizontal, &glyphCopy, &bounds, 1)

        var advance = CGSize.zero
        CTFontGetAdvancesForGlyphs(font, .horizontal, &glyphCopy, &advance, 1)

        // Handle empty glyphs (space, control characters)
        let padding = CGFloat(Self.glyphPadding)
        let glyphWidth = Int(ceil(bounds.width + padding * 2))
        let glyphHeight = Int(ceil(bounds.height + padding * 2))

        if glyphWidth <= Int(padding * 2) || glyphHeight <= Int(padding * 2) {
            // Empty glyph - no bitmap needed
            var entry = GlyphEntry(
                codepoint: codepoint,
                x: 0, y: 0, width: 0, height: 0,
                bearingX: 0, bearingY: 0,
                advance: UInt16(ceil(advance.width))
            )
            entry.uvRect = (0, 0, 0, 0)
            glyphCache[codepoint] = entry
            pendingGlyphEntries.append(entry)
            return entry
        }

        // Allocate position in atlas
        guard let position = allocateAtlasSpace(width: glyphWidth, height: glyphHeight) else {
            // Atlas full - grow and retry
            if !growAtlas() {
                DLog("DTermGlyphAtlasManager: Failed to grow atlas for codepoint U+\(String(codepoint, radix: 16))")
                return nil
            }
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

        // Calculate UV coordinates
        let size = Float(atlasSize)
        let uvRect = (
            u0: Float(position.x) / size,
            v0: Float(position.y) / size,
            u1: Float(position.x + glyphWidth) / size,
            v1: Float(position.y + glyphHeight) / size
        )

        // Create entry
        var entry = GlyphEntry(
            codepoint: codepoint,
            x: UInt16(position.x),
            y: UInt16(position.y),
            width: UInt16(glyphWidth),
            height: UInt16(glyphHeight),
            bearingX: Int16(bounds.origin.x - padding),
            bearingY: Int16(ceil(bounds.origin.y + bounds.height + padding)),
            advance: UInt16(ceil(advance.width))
        )
        entry.uvRect = uvRect

        glyphCache[codepoint] = entry
        pendingGlyphEntries.append(entry)
        isDirty = true

        return entry
    }

    /// Convert a Unicode codepoint to a CGGlyph.
    private func glyphForCodepoint(_ codepoint: UInt32, font: CTFont) -> CGGlyph {
        var glyph: CGGlyph = 0

        if codepoint <= 0xFFFF {
            // BMP character - single UniChar
            var char = UniChar(codepoint)
            if CTFontGetGlyphsForCharacters(font, &char, &glyph, 1) {
                return glyph
            }
        } else if codepoint <= 0x10FFFF {
            // Supplementary character - surrogate pair
            let adjusted = codepoint - 0x10000
            let high = UniChar((adjusted >> 10) + 0xD800)
            let low = UniChar((adjusted & 0x3FF) + 0xDC00)
            var chars: [UniChar] = [high, low]
            var glyphs: [CGGlyph] = [0, 0]
            if CTFontGetGlyphsForCharacters(font, &chars, &glyphs, 2) {
                return glyphs[0]
            }
        }

        // Fall back to replacement character
        var replacement: UniChar = 0xFFFD
        CTFontGetGlyphsForCharacters(font, &replacement, &glyph, 1)
        return glyph
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
            guard let baseAddress = ptr.baseAddress else { return }

            guard let context = CGContext(
                data: baseAddress,
                width: width,
                height: height,
                bitsPerComponent: 8,
                bytesPerRow: width,
                space: CGColorSpaceCreateDeviceGray(),
                bitmapInfo: CGImageAlphaInfo.none.rawValue
            ) else {
                DLog("DTermGlyphAtlasManager: Failed to create bitmap context")
                return
            }

            // Enable high-quality text rendering
            context.setAllowsAntialiasing(true)
            context.setShouldAntialias(true)
            context.setShouldSmoothFonts(true)
            context.setAllowsFontSmoothing(true)

            // Set white color (grayscale value becomes alpha in shader)
            context.setFillColor(gray: 1.0, alpha: 1.0)

            // Position glyph with padding offset
            let padding = CGFloat(Self.glyphPadding)
            var position = CGPoint(
                x: -bounds.origin.x + padding,
                y: -bounds.origin.y + padding
            )

            // Draw glyph
            var glyphCopy = glyph
            CTFontDrawGlyphs(font, &glyphCopy, &position, 1, context)
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

        // Check if new row fits
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

        // Copy row by row to atlas bitmap
        for row in 0..<height {
            let srcOffset = row * width
            let dstOffset = (y + row) * atlasSize + x
            bitmap.withUnsafeBytes { srcPtr in
                guard let srcBase = srcPtr.baseAddress else { return }
                memcpy(atlasBitmap.advanced(by: dstOffset), srcBase.advanced(by: srcOffset), width)
            }
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
            guard let baseAddress = ptr.baseAddress else { return }
            texture.replace(
                region: region,
                mipmapLevel: 0,
                withBytes: baseAddress,
                bytesPerRow: width
            )
        }
    }

    /// Create the atlas texture and bitmap.
    private func createAtlasTexture() {
        // Create Metal texture
        let descriptor = MTLTextureDescriptor.texture2DDescriptor(
            pixelFormat: .r8Unorm,
            width: atlasSize,
            height: atlasSize,
            mipmapped: false
        )
        descriptor.usage = [.shaderRead]
        descriptor.storageMode = .shared

        atlasTexture = device.makeTexture(descriptor: descriptor)

        if atlasTexture == nil {
            DLog("DTermGlyphAtlasManager: Failed to create atlas texture (\(atlasSize)x\(atlasSize))")
            return
        }

        // Allocate CPU-side bitmap
        if let existing = atlasBitmap {
            existing.deallocate()
        }

        let byteCount = atlasSize * atlasSize
        atlasBitmap = UnsafeMutablePointer<UInt8>.allocate(capacity: byteCount)
        atlasBitmap?.initialize(repeating: 0, count: byteCount)

        DLog("DTermGlyphAtlasManager: Created atlas texture (\(atlasSize)x\(atlasSize))")
    }

    /// Grow the atlas to the next power of 2.
    private func growAtlas() -> Bool {
        let newSize = atlasSize * 2

        guard newSize <= Self.maxAtlasSize else {
            DLog("DTermGlyphAtlasManager: Cannot grow atlas beyond \(Self.maxAtlasSize)x\(Self.maxAtlasSize)")
            return false
        }

        DLog("DTermGlyphAtlasManager: Growing atlas from \(atlasSize) to \(newSize)")

        // Save old data
        let oldSize = atlasSize
        let oldBitmap = atlasBitmap

        // Create new atlas
        atlasSize = newSize
        createAtlasTexture()

        // Copy old data to new atlas (top-left corner)
        if let oldBitmap = oldBitmap, let newBitmap = atlasBitmap {
            for row in 0..<oldSize {
                let srcOffset = row * oldSize
                let dstOffset = row * newSize
                memcpy(newBitmap.advanced(by: dstOffset), oldBitmap.advanced(by: srcOffset), oldSize)
            }
            oldBitmap.deallocate()
        }

        // Upload entire atlas to texture
        uploadFullAtlas()

        // Update UV coordinates for all cached glyphs
        recalculateUVCoordinates()

        return true
    }

    /// Upload the full atlas bitmap to the Metal texture.
    private func uploadFullAtlas() {
        guard let texture = atlasTexture, let bitmap = atlasBitmap else { return }

        let region = MTLRegion(
            origin: MTLOrigin(x: 0, y: 0, z: 0),
            size: MTLSize(width: atlasSize, height: atlasSize, depth: 1)
        )

        texture.replace(
            region: region,
            mipmapLevel: 0,
            withBytes: bitmap,
            bytesPerRow: atlasSize
        )
    }

    /// Recalculate UV coordinates for all cached glyphs after atlas resize.
    private func recalculateUVCoordinates() {
        let size = Float(atlasSize)

        for (codepoint, var entry) in glyphCache {
            entry.uvRect = (
                u0: Float(entry.x) / size,
                v0: Float(entry.y) / size,
                u1: Float(entry.x + entry.width) / size,
                v1: Float(entry.y + entry.height) / size
            )
            glyphCache[codepoint] = entry
        }

        // Mark all glyphs as pending since UVs changed
        pendingGlyphEntries = Array(glyphCache.values)
    }

    /// Reset the atlas (clears cache and re-creates texture).
    private func resetAtlas() {
        glyphCache.removeAll()
        pendingGlyphEntries.removeAll()
        nextX = 0
        nextY = 0
        currentRowHeight = 0
        atlasSize = Self.initialAtlasSize
        createAtlasTexture()
    }

    /// Pre-render ASCII printable characters.
    private func prerenderASCII() {
        for codepoint in UInt32(32)...UInt32(126) {
            _ = renderGlyph(codepoint: codepoint)
        }

        DLog("DTermGlyphAtlasManager: Pre-rendered \(glyphCache.count) ASCII glyphs")
    }

    // MARK: - Sync Interface

    /// Clear pending glyph entries after syncing to renderer.
    public func clearPendingGlyphEntries() {
        pendingGlyphEntries.removeAll()
        isDirty = false
    }

    /// Check if there are pending changes.
    public var hasPendingChanges: Bool {
        return isDirty || !pendingGlyphEntries.isEmpty
    }
}

// MARK: - Debug Support

extension DTermGlyphAtlasManager {
    /// Get atlas usage statistics.
    public var debugStats: String {
        let usedPixels = nextX + nextY * atlasSize
        let totalPixels = atlasSize * atlasSize
        let usage = Double(usedPixels) / Double(totalPixels) * 100.0

        return """
        DTermGlyphAtlasManager Stats:
          Atlas size: \(atlasSize)x\(atlasSize)
          Glyphs cached: \(glyphCache.count)
          Usage: \(String(format: "%.1f", usage))%
          Cell size: \(cellWidth)x\(cellHeight)
          Font: \(font != nil ? CTFontCopyFullName(font!) as String : "none")
        """
    }
}
