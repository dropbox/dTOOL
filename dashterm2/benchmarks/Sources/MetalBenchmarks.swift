//
//  MetalBenchmarks.swift
//  DashTerm2 Benchmarks
//
//  Metal renderer benchmarks (requires GUI).
//  These benchmarks simulate the Metal rendering pipeline without
//  requiring the actual Metal context, useful for CI.
//

import Foundation

// MARK: - Simulated Metal Types

/// Simulated glyph cache entry
struct SimulatedGlyphEntry {
    let character: UnicodeScalar
    let textureX: Int
    let textureY: Int
    let width: Int
    let height: Int
    let advanceWidth: Float
}

/// Simulated texture page for glyph atlas
final class SimulatedTexturePage {
    let pageSize: Int
    private var usedSpace: Int = 0
    private var entries: [UnicodeScalar: SimulatedGlyphEntry] = [:]

    init(pageSize: Int = 2048) {
        self.pageSize = pageSize
    }

    var isFull: Bool {
        usedSpace >= pageSize * pageSize * 3 / 4 // 75% threshold
    }

    func addGlyph(_ character: UnicodeScalar, width: Int, height: Int) -> SimulatedGlyphEntry? {
        guard !isFull else { return nil }

        let entry = SimulatedGlyphEntry(
            character: character,
            textureX: usedSpace % pageSize,
            textureY: usedSpace / pageSize,
            width: width,
            height: height,
            advanceWidth: Float(width)
        )

        entries[character] = entry
        usedSpace += width * height
        return entry
    }

    func getGlyph(_ character: UnicodeScalar) -> SimulatedGlyphEntry? {
        entries[character]
    }

    var entryCount: Int { entries.count }
}

/// Simulated glyph cache with multiple texture pages
final class SimulatedGlyphCache {
    private var pages: [SimulatedTexturePage] = []
    private var glyphMap: [UnicodeScalar: (pageIndex: Int, entry: SimulatedGlyphEntry)] = [:]
    private var hits: Int = 0
    private var misses: Int = 0

    init() {
        pages.append(SimulatedTexturePage())
    }

    func getOrCreateGlyph(_ character: UnicodeScalar) -> SimulatedGlyphEntry {
        if let cached = glyphMap[character] {
            hits += 1
            return cached.entry
        }

        misses += 1

        // Try to add to existing page
        let width = character.isASCII ? 10 : 20
        let height = 20

        for (index, page) in pages.enumerated() {
            if let entry = page.addGlyph(character, width: width, height: height) {
                glyphMap[character] = (index, entry)
                return entry
            }
        }

        // Need a new page
        let newPage = SimulatedTexturePage()
        pages.append(newPage)

        if let entry = newPage.addGlyph(character, width: width, height: height) {
            glyphMap[character] = (pages.count - 1, entry)
            return entry
        }

        // Shouldn't happen - new page should always have space
        // swiftlint:disable:next no_bare_fatal_error
        fatalError("Failed to add glyph to new page") // it_fatalError unavailable in SPM benchmark target
    }

    var hitRate: Double {
        let total = hits + misses
        guard total > 0 else { return 0 }
        return Double(hits) / Double(total)
    }

    var pageCount: Int { pages.count }

    func resetStats() {
        hits = 0
        misses = 0
    }

    func clear() {
        pages = [SimulatedTexturePage()]
        glyphMap = [:]
        hits = 0
        misses = 0
    }
}

/// Simulated render batch
struct SimulatedRenderBatch {
    var glyphCount: Int = 0
    var vertexCount: Int = 0
    var triangleCount: Int = 0

    mutating func addGlyph(_ entry: SimulatedGlyphEntry) {
        glyphCount += 1
        vertexCount += 4  // quad
        triangleCount += 2
    }
}

// MARK: - Metal Benchmarks

/// Benchmark: Glyph cache lookup performance
final class GlyphCacheBenchmark: Benchmark {
    let name = "GlyphCache"
    let category: BenchmarkCategory = .metalRenderer
    let description = "Glyph cache hit/miss performance"

    private var cache: SimulatedGlyphCache!
    private let lookupCount = 10000
    private var hitRate: Double = 0

    func setUp() {
        cache = SimulatedGlyphCache()
        // Pre-populate with common ASCII
        for scalar in (32..<127).compactMap({ UnicodeScalar($0) }) {
            _ = cache.getOrCreateGlyph(scalar)
        }
        cache.resetStats()
    }

    func runIteration() -> UInt64 {
        let asciiChars = (32..<127).compactMap { UnicodeScalar($0) }
        let unicodeChars = (0x4E00..<0x4E50).compactMap { UnicodeScalar($0) }
        let allChars = asciiChars + unicodeChars

        let duration = PrecisionTimer.measure {
            for i in 0..<lookupCount {
                let char = allChars[i % allChars.count]
                _ = cache.getOrCreateGlyph(char)
            }
        }

        hitRate = cache.hitRate
        cache.resetStats()

        return duration
    }

    func collectAdditionalMetrics() -> [String: Double] {
        [
            "hit_rate": hitRate,
            "page_count": Double(cache?.pageCount ?? 0)
        ]
    }

    func tearDown() {
        cache = nil
    }
}

/// Benchmark: Batch assembly performance
final class BatchAssemblyBenchmark: Benchmark {
    let name = "BatchAssembly"
    let category: BenchmarkCategory = .metalRenderer
    let description = "Render batch assembly performance"

    private var cache: SimulatedGlyphCache!
    private let linesPerFrame = 50
    private let charsPerLine = 132

    func setUp() {
        cache = SimulatedGlyphCache()
        // Pre-populate cache
        for scalar in (32..<127).compactMap({ UnicodeScalar($0) }) {
            _ = cache.getOrCreateGlyph(scalar)
        }
    }

    func runIteration() -> UInt64 {
        let chars = (32..<127).compactMap { UnicodeScalar($0) }

        return PrecisionTimer.measure {
            var batch = SimulatedRenderBatch()

            for line in 0..<linesPerFrame {
                for col in 0..<charsPerLine {
                    let char = chars[(line * charsPerLine + col) % chars.count]
                    let entry = cache.getOrCreateGlyph(char)
                    batch.addGlyph(entry)
                }
            }

            blackHole(batch.glyphCount)
            blackHole(batch.vertexCount)
        }
    }

    func tearDown() {
        cache = nil
    }
}

/// Benchmark: Texture atlas management
final class TextureAtlasBenchmark: Benchmark {
    let name = "TextureAtlas"
    let category: BenchmarkCategory = .metalRenderer
    let description = "Texture atlas allocation performance"

    private var cache: SimulatedGlyphCache!

    func setUp() {
        cache = SimulatedGlyphCache()
    }

    func runIteration() -> UInt64 {
        cache.clear()

        return PrecisionTimer.measure {
            // Simulate loading many different characters
            // ASCII
            for scalar in (32..<127).compactMap({ UnicodeScalar($0) }) {
                _ = cache.getOrCreateGlyph(scalar)
            }

            // Extended Latin
            for scalar in (0x00C0..<0x0100).compactMap({ UnicodeScalar($0) }) {
                _ = cache.getOrCreateGlyph(scalar)
            }

            // CJK subset
            for scalar in (0x4E00..<0x4F00).compactMap({ UnicodeScalar($0) }) {
                _ = cache.getOrCreateGlyph(scalar)
            }

            // Greek
            for scalar in (0x0370..<0x0400).compactMap({ UnicodeScalar($0) }) {
                _ = cache.getOrCreateGlyph(scalar)
            }
        }
    }

    func collectAdditionalMetrics() -> [String: Double] {
        ["texture_pages": Double(cache?.pageCount ?? 0)]
    }

    func tearDown() {
        cache = nil
    }
}

/// Benchmark: Simulated frame render timing
final class FrameRenderBenchmark: Benchmark {
    let name = "FrameRender"
    let category: BenchmarkCategory = .metalRenderer
    let description = "Simulated frame render pipeline"

    private var cache: SimulatedGlyphCache!
    private let screenWidth = 132
    private let screenHeight = 50

    func setUp() {
        cache = SimulatedGlyphCache()
        // Pre-populate
        for scalar in (32..<127).compactMap({ UnicodeScalar($0) }) {
            _ = cache.getOrCreateGlyph(scalar)
        }
    }

    func runIteration() -> UInt64 {
        let chars = (32..<127).compactMap { UnicodeScalar($0) }

        return PrecisionTimer.measure {
            // Phase 1: Build render batches
            var batches: [SimulatedRenderBatch] = []
            var currentBatch = SimulatedRenderBatch()
            let maxBatchSize = 1000

            for row in 0..<screenHeight {
                for col in 0..<screenWidth {
                    let charIndex = (row * screenWidth + col) % chars.count
                    let entry = cache.getOrCreateGlyph(chars[charIndex])
                    currentBatch.addGlyph(entry)

                    if currentBatch.glyphCount >= maxBatchSize {
                        batches.append(currentBatch)
                        currentBatch = SimulatedRenderBatch()
                    }
                }
            }

            if currentBatch.glyphCount > 0 {
                batches.append(currentBatch)
            }

            // Phase 2: Simulate GPU submission
            var totalTriangles = 0
            for batch in batches {
                totalTriangles += batch.triangleCount
            }

            blackHole(totalTriangles)
            blackHole(batches.count)
        }
    }

    func tearDown() {
        cache = nil
    }
}

/// Benchmark: Color processing
final class ColorProcessingBenchmark: Benchmark {
    let name = "ColorProcessing"
    let category: BenchmarkCategory = .metalRenderer
    let description = "Color conversion and blending"

    private let colorCount = 10000

    func runIteration() -> UInt64 {
        PrecisionTimer.measure {
            var resultR: Float = 0
            var resultG: Float = 0
            var resultB: Float = 0

            for i in 0..<colorCount {
                // Simulate ANSI to RGB conversion
                let ansiColor = i % 256
                let (r, g, b) = ansiToRGB(ansiColor)

                // Simulate alpha blending
                let alpha: Float = 0.8
                resultR = resultR * (1 - alpha) + r * alpha
                resultG = resultG * (1 - alpha) + g * alpha
                resultB = resultB * (1 - alpha) + b * alpha
            }

            blackHole(resultR)
            blackHole(resultG)
            blackHole(resultB)
        }
    }

    private func ansiToRGB(_ color: Int) -> (Float, Float, Float) {
        if color < 16 {
            // Standard colors
            let r = (color & 1) != 0 ? 1.0 : 0.0
            let g = (color & 2) != 0 ? 1.0 : 0.0
            let b = (color & 4) != 0 ? 1.0 : 0.0
            let bright: Float = color >= 8 ? 1.0 : 0.7
            return (Float(r) * bright, Float(g) * bright, Float(b) * bright)
        } else if color < 232 {
            // 216 color cube
            let index = color - 16
            let r = Float(index / 36) / 5.0
            let g = Float((index / 6) % 6) / 5.0
            let b = Float(index % 6) / 5.0
            return (r, g, b)
        } else {
            // Grayscale
            let gray = Float(color - 232) / 23.0
            return (gray, gray, gray)
        }
    }
}

// MARK: - Registration

/// All Metal renderer benchmarks
func createMetalBenchmarks() -> [Benchmark] {
    [
        GlyphCacheBenchmark(),
        BatchAssemblyBenchmark(),
        TextureAtlasBenchmark(),
        FrameRenderBenchmark(),
        ColorProcessingBenchmark(),
    ]
}
