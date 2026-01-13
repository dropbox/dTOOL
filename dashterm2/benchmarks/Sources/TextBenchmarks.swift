//
//  TextBenchmarks.swift
//  DashTerm2 Benchmarks
//
//  Text rendering performance benchmarks.
//

import Foundation

// MARK: - Test Data Generation

/// Generates test content for benchmarks
final class BenchmarkTestData {
    static let shared = BenchmarkTestData()

    // ASCII test content
    lazy var asciiLine80: String = String(repeating: "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!@#$%^&*()_+-=[]{}|", count: 1)[..<String.Index(utf16Offset: 80, in: String(repeating: "A", count: 80))].description

    lazy var ascii1K: [String] = (0..<1000).map { _ in generateASCIILine() }
    lazy var ascii10K: [String] = (0..<10000).map { _ in generateASCIILine() }
    lazy var ascii100K: [String] = (0..<100000).map { _ in generateASCIILine() }

    // Unicode test content
    lazy var unicode1K: [String] = (0..<1000).map { _ in generateUnicodeLine() }
    lazy var unicode10K: [String] = (0..<10000).map { _ in generateUnicodeLine() }

    // Emoji test content
    lazy var emoji1K: [String] = (0..<1000).map { _ in generateEmojiLine() }

    // Mixed content
    lazy var mixed1K: [String] = (0..<1000).map { i -> String in
        switch i % 3 {
        case 0: return generateASCIILine()
        case 1: return generateUnicodeLine()
        default: return generateEmojiLine()
        }
    }

    private func generateASCIILine() -> String {
        let chars = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!@#$%^&*()-_=+[]{}|;:',.<>?/`~ "
        return String((0..<80).compactMap { _ in chars.randomElement() })
    }

    private func generateUnicodeLine() -> String {
        // Mix of various Unicode ranges
        let ranges: [ClosedRange<UInt32>] = [
            0x0400...0x04FF,  // Cyrillic
            0x3040...0x309F,  // Hiragana
            0x4E00...0x4FFF,  // CJK (subset)
            0x0370...0x03FF,  // Greek
            0x0590...0x05FF,  // Hebrew
        ]

        return String((0..<40).compactMap { _ -> Character? in
            guard let range = ranges.randomElement(),
                  let scalar = UnicodeScalar(UInt32.random(in: range)) else {
                return nil
            }
            return Character(scalar)
        })
    }

    private func generateEmojiLine() -> String {
        let emojis = ["😀", "😎", "🎉", "🚀", "💻", "🔥", "✨", "🎯", "📱", "🌟",
                      "👍", "❤️", "🎨", "🎵", "🏆", "⚡", "🌈", "🎁", "🔔", "💡",
                      "🐱", "🐶", "🦊", "🦁", "🐼", "🦄", "🐸", "🐙", "🦋", "🌸"]
        return (0..<40).compactMap { _ in emojis.randomElement() }.joined()
    }
}

// MARK: - Text Processing Benchmarks

/// Benchmark: Process 1K lines of ASCII text
final class TextRender1KBenchmark: Benchmark {
    let name = "TextRender1K"
    let category: BenchmarkCategory = .textRendering
    let description = "Process 1,000 lines of ASCII text"

    private var lines: [String] = []

    func setUp() {
        lines = BenchmarkTestData.shared.ascii1K
    }

    func runIteration() -> UInt64 {
        PrecisionTimer.measure {
            // Simulate text processing pipeline
            for line in lines {
                processLine(line)
            }
        }
    }

    @inline(never)
    private func processLine(_ line: String) {
        // Simulate character-by-character processing
        var charCount = 0
        var widthAccum = 0
        for scalar in line.unicodeScalars {
            charCount += 1
            widthAccum += characterWidth(scalar)
        }
        // Prevent optimizer from eliminating work
        blackHole(charCount)
        blackHole(widthAccum)
    }
}

/// Benchmark: Process 10K lines of ASCII text
final class TextRender10KBenchmark: Benchmark {
    let name = "TextRender10K"
    let category: BenchmarkCategory = .textRendering
    let description = "Process 10,000 lines of ASCII text"

    private var lines: [String] = []

    func setUp() {
        lines = BenchmarkTestData.shared.ascii10K
    }

    func runIteration() -> UInt64 {
        PrecisionTimer.measure {
            for line in lines {
                processLine(line)
            }
        }
    }

    @inline(never)
    private func processLine(_ line: String) {
        var charCount = 0
        var widthAccum = 0
        for scalar in line.unicodeScalars {
            charCount += 1
            widthAccum += characterWidth(scalar)
        }
        blackHole(charCount)
        blackHole(widthAccum)
    }
}

/// Benchmark: Process 100K lines of ASCII text
final class TextRender100KBenchmark: Benchmark {
    let name = "TextRender100K"
    let category: BenchmarkCategory = .textRendering
    let description = "Process 100,000 lines of ASCII text"

    private var lines: [String] = []

    func setUp() {
        lines = BenchmarkTestData.shared.ascii100K
    }

    func runIteration() -> UInt64 {
        PrecisionTimer.measure {
            for line in lines {
                processLine(line)
            }
        }
    }

    @inline(never)
    private func processLine(_ line: String) {
        var charCount = 0
        var widthAccum = 0
        for scalar in line.unicodeScalars {
            charCount += 1
            widthAccum += characterWidth(scalar)
        }
        blackHole(charCount)
        blackHole(widthAccum)
    }
}

/// Benchmark: Process Unicode text
final class UnicodeTextBenchmark: Benchmark {
    let name = "UnicodeText"
    let category: BenchmarkCategory = .textRendering
    let description = "Process 1,000 lines with Unicode characters"

    private var lines: [String] = []

    func setUp() {
        lines = BenchmarkTestData.shared.unicode1K
    }

    func runIteration() -> UInt64 {
        PrecisionTimer.measure {
            for line in lines {
                processUnicodeLine(line)
            }
        }
    }

    @inline(never)
    private func processUnicodeLine(_ line: String) {
        var charCount = 0
        var widthAccum = 0
        var isDoubleWidth = false

        for scalar in line.unicodeScalars {
            charCount += 1
            let width = characterWidth(scalar)
            widthAccum += width
            if width == 2 {
                isDoubleWidth = true
            }
        }
        blackHole(charCount)
        blackHole(widthAccum)
        blackHole(isDoubleWidth)
    }
}

/// Benchmark: Process emoji-heavy text
final class EmojiTextBenchmark: Benchmark {
    let name = "EmojiText"
    let category: BenchmarkCategory = .textRendering
    let description = "Process 1,000 lines with emoji"

    private var lines: [String] = []

    func setUp() {
        lines = BenchmarkTestData.shared.emoji1K
    }

    func runIteration() -> UInt64 {
        PrecisionTimer.measure {
            for line in lines {
                processEmojiLine(line)
            }
        }
    }

    @inline(never)
    private func processEmojiLine(_ line: String) {
        var graphemeCount = 0
        var scalarsPerGrapheme: [Int] = []

        // Process grapheme clusters (important for emoji)
        for char in line {
            graphemeCount += 1
            scalarsPerGrapheme.append(char.unicodeScalars.count)
        }
        blackHole(graphemeCount)
        blackHole(scalarsPerGrapheme.count)
    }
}

/// Benchmark: Process mixed content
final class MixedContentBenchmark: Benchmark {
    let name = "MixedContent"
    let category: BenchmarkCategory = .textRendering
    let description = "Process 1,000 lines of mixed ASCII/Unicode/emoji"

    private var lines: [String] = []

    func setUp() {
        lines = BenchmarkTestData.shared.mixed1K
    }

    func runIteration() -> UInt64 {
        PrecisionTimer.measure {
            for line in lines {
                processMixedLine(line)
            }
        }
    }

    @inline(never)
    private func processMixedLine(_ line: String) {
        var asciiCount = 0
        var unicodeCount = 0
        var emojiCount = 0

        for scalar in line.unicodeScalars {
            if scalar.isASCII {
                asciiCount += 1
            } else if scalar.value >= 0x1F600 && scalar.value <= 0x1F64F {
                emojiCount += 1
            } else {
                unicodeCount += 1
            }
        }
        blackHole(asciiCount)
        blackHole(unicodeCount)
        blackHole(emojiCount)
    }
}

// MARK: - Helper Functions

/// Determine character display width (simplified wcwidth)
@inline(__always)
func characterWidth(_ scalar: UnicodeScalar) -> Int {
    let value = scalar.value

    // ASCII printable
    if value >= 0x20 && value < 0x7F {
        return 1
    }

    // Control characters
    if value < 0x20 || value == 0x7F {
        return 0
    }

    // CJK and other wide characters
    if (value >= 0x1100 && value <= 0x115F) ||  // Hangul Jamo
       (value >= 0x2E80 && value <= 0xA4CF) ||  // CJK
       (value >= 0xAC00 && value <= 0xD7A3) ||  // Hangul Syllables
       (value >= 0xF900 && value <= 0xFAFF) ||  // CJK Compatibility
       (value >= 0xFE10 && value <= 0xFE1F) ||  // Vertical forms
       (value >= 0xFE30 && value <= 0xFE6F) ||  // CJK Compatibility Forms
       (value >= 0xFF00 && value <= 0xFF60) ||  // Fullwidth forms
       (value >= 0xFFE0 && value <= 0xFFE6) ||  // Fullwidth signs
       (value >= 0x20000 && value <= 0x2FFFF) { // CJK Extension
        return 2
    }

    return 1
}

/// Prevent compiler from optimizing away computation
@inline(never)
func blackHole<T>(_ value: T) {
    // Force the value to be computed by passing through an opaque function
    withUnsafePointer(to: value) { _ in }
}

// MARK: - Registration

/// All text benchmarks
func createTextBenchmarks() -> [Benchmark] {
    [
        TextRender1KBenchmark(),
        TextRender10KBenchmark(),
        TextRender100KBenchmark(),
        UnicodeTextBenchmark(),
        EmojiTextBenchmark(),
        MixedContentBenchmark(),
    ]
}
