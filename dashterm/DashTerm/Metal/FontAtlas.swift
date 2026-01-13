//
//  FontAtlas.swift
//  DashTerm
//
//  Core Text-based font atlas for GPU text rendering
//

import Foundation
import Metal
import CoreText
import CoreGraphics

/// Glyph metrics for positioning text
struct GlyphMetrics {
    let uvRect: CGRect       // Position in atlas (0-1 normalized coordinates)
    let size: CGSize         // Glyph size in pixels
    let bearing: CGPoint     // Offset from baseline
    let advance: CGFloat     // Horizontal advance
}

/// Font atlas for GPU text rendering
class FontAtlas {
    let texture: MTLTexture
    let font: CTFont
    private var glyphMetrics: [Character: GlyphMetrics] = [:]

    // Atlas layout
    let atlasWidth: Int = 512
    let atlasHeight: Int = 512

    /// ASCII printable characters (32-126) plus common symbols
    static let defaultCharset: String = {
        var chars = ""
        for i: UInt8 in 32...126 {
            chars.append(Character(UnicodeScalar(i)))
        }
        return chars
    }()

    init?(device: MTLDevice, fontName: String = "SF Mono", fontSize: CGFloat = 14.0) {
        // Create font - try requested font first, fallback to Menlo
        let requestedFont = CTFontCreateWithName(fontName as CFString, fontSize, nil)
        let fallbackFont = CTFontCreateWithName("Menlo" as CFString, fontSize, nil)

        // CTFontCreateWithName always returns a font (falls back to system font)
        // Use the requested font if it matches the name, otherwise use Menlo
        let requestedName = CTFontCopyFamilyName(requestedFont) as String
        self.font = requestedName.lowercased().contains(fontName.lowercased().prefix(4)) ? requestedFont : fallbackFont

        // Create atlas texture
        let textureDescriptor = MTLTextureDescriptor.texture2DDescriptor(
            pixelFormat: .r8Unorm,
            width: atlasWidth,
            height: atlasHeight,
            mipmapped: false
        )
        textureDescriptor.usage = [.shaderRead]

        guard let texture = device.makeTexture(descriptor: textureDescriptor) else {
            return nil
        }
        self.texture = texture

        // Rasterize glyphs to atlas
        rasterizeGlyphs()
    }

    private func rasterizeGlyphs() {
        // Create bitmap context
        let bytesPerPixel = 1
        let bytesPerRow = atlasWidth * bytesPerPixel
        let bitmapData = UnsafeMutableRawPointer.allocate(
            byteCount: atlasWidth * atlasHeight * bytesPerPixel,
            alignment: 1
        )
        memset(bitmapData, 0, atlasWidth * atlasHeight * bytesPerPixel)
        defer { bitmapData.deallocate() }

        guard let context = CGContext(
            data: bitmapData,
            width: atlasWidth,
            height: atlasHeight,
            bitsPerComponent: 8,
            bytesPerRow: bytesPerRow,
            space: CGColorSpaceCreateDeviceGray(),
            bitmapInfo: CGImageAlphaInfo.none.rawValue
        ) else { return }

        // Configure context for text rendering
        context.setAllowsAntialiasing(true)
        context.setShouldAntialias(true)
        context.setAllowsFontSmoothing(true)
        context.setShouldSmoothFonts(true)

        // Set white text on transparent background
        context.setFillColor(gray: 1.0, alpha: 1.0)

        // Layout glyphs in a grid
        let padding: CGFloat = 2
        var x: CGFloat = padding
        var y: CGFloat = padding
        let lineHeight = CTFontGetAscent(font) + CTFontGetDescent(font) + CTFontGetLeading(font) + padding * 2

        for char in Self.defaultCharset {
            // Get glyph for character
            var chars = [UniChar](String(char).utf16)
            var glyphs = [CGGlyph](repeating: 0, count: chars.count)
            CTFontGetGlyphsForCharacters(font, &chars, &glyphs, chars.count)

            guard glyphs[0] != 0 else { continue }

            // Get glyph bounds
            var boundingRect = CGRect.zero
            CTFontGetBoundingRectsForGlyphs(font, .default, glyphs, &boundingRect, 1)

            var advances = CGSize.zero
            CTFontGetAdvancesForGlyphs(font, .default, glyphs, &advances, 1)

            let glyphWidth = ceil(boundingRect.width + padding * 2)
            let glyphHeight = ceil(lineHeight)

            // Wrap to next row if needed
            if x + glyphWidth > CGFloat(atlasWidth) {
                x = padding
                y += lineHeight
            }

            // Check if we've run out of atlas space
            if y + glyphHeight > CGFloat(atlasHeight) {
                break
            }

            // Draw glyph
            let baseline = y + CTFontGetAscent(font)
            let drawX = x - boundingRect.origin.x + padding

            context.saveGState()

            // Draw the glyph
            let position = CGPoint(x: drawX, y: CGFloat(atlasHeight) - baseline)
            CTFontDrawGlyphs(font, glyphs, [position], 1, context)

            context.restoreGState()

            // Store metrics (normalized UV coordinates)
            let uvRect = CGRect(
                x: x / CGFloat(atlasWidth),
                y: y / CGFloat(atlasHeight),
                width: glyphWidth / CGFloat(atlasWidth),
                height: glyphHeight / CGFloat(atlasHeight)
            )

            glyphMetrics[char] = GlyphMetrics(
                uvRect: uvRect,
                size: CGSize(width: glyphWidth, height: glyphHeight),
                bearing: CGPoint(x: boundingRect.origin.x, y: boundingRect.origin.y),
                advance: advances.width
            )

            x += glyphWidth
        }

        // Upload to texture
        texture.replace(
            region: MTLRegionMake2D(0, 0, atlasWidth, atlasHeight),
            mipmapLevel: 0,
            withBytes: bitmapData,
            bytesPerRow: bytesPerRow
        )
    }

    /// Get metrics for a character
    func metrics(for char: Character) -> GlyphMetrics? {
        return glyphMetrics[char]
    }

    /// Get metrics for all characters in a string
    func metrics(for string: String) -> [(Character, GlyphMetrics)] {
        return string.compactMap { char in
            guard let metrics = glyphMetrics[char] else { return nil }
            return (char, metrics)
        }
    }

    /// Calculate the width of a string in pixels
    func measureString(_ string: String) -> CGFloat {
        return string.reduce(0) { total, char in
            total + (glyphMetrics[char]?.advance ?? 0)
        }
    }

    /// Line height for the font
    var lineHeight: CGFloat {
        return CTFontGetAscent(font) + CTFontGetDescent(font) + CTFontGetLeading(font)
    }

    /// Ascent of the font (height above baseline)
    var ascent: CGFloat {
        return CTFontGetAscent(font)
    }

    /// Descent of the font (height below baseline)
    var descent: CGFloat {
        return CTFontGetDescent(font)
    }
}
