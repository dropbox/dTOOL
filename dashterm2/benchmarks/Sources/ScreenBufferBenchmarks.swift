//
//  ScreenBufferBenchmarks.swift
//  DashTerm2 Benchmarks
//
//  Screen buffer operation benchmarks simulating VT100Screen operations.
//

import Foundation

// MARK: - Screen Buffer Simulation

/// Simulated screen buffer for benchmarking
/// Based on VT100Screen patterns but standalone for benchmark isolation
final class SimulatedScreenBuffer {
    typealias ScreenLine = [ScreenCell]

    struct ScreenCell {
        var character: UnicodeScalar
        var foreground: UInt8
        var background: UInt8
        var flags: UInt8

        static var blank: ScreenCell {
            ScreenCell(character: " ", foreground: 7, background: 0, flags: 0)
        }
    }

    private(set) var width: Int
    private(set) var height: Int
    private var lines: [ScreenLine]
    private var history: [[ScreenLine]]
    private let maxHistorySize: Int

    var cursorX: Int = 0
    var cursorY: Int = 0

    init(width: Int, height: Int, maxHistorySize: Int = 10000) {
        self.width = width
        self.height = height
        self.maxHistorySize = maxHistorySize
        self.lines = (0..<height).map { _ in Self.blankLine(width: width) }
        self.history = []
    }

    static func blankLine(width: Int) -> ScreenLine {
        [ScreenCell](repeating: .blank, count: width)
    }

    // MARK: - Operations

    func insertLine(at index: Int) {
        guard index >= 0 && index < height else { return }

        // Move last line to history
        if history.count < maxHistorySize {
            if let lastLine = lines.last {
                history.append([lastLine])
            }
        }

        // Shift lines down
        lines.removeLast()
        lines.insert(Self.blankLine(width: width), at: index)
    }

    func deleteLine(at index: Int) {
        guard index >= 0 && index < height else { return }

        lines.remove(at: index)
        lines.append(Self.blankLine(width: width))
    }

    func scrollUp(lines count: Int) {
        guard count > 0 && count <= height else { return }

        // Move scrolled lines to history
        for i in 0..<count {
            if history.count < maxHistorySize {
                history.append([lines[i]])
            }
        }

        // Remove top lines and add blank at bottom
        lines.removeFirst(count)
        for _ in 0..<count {
            lines.append(Self.blankLine(width: width))
        }
    }

    func scrollDown(lines count: Int) {
        guard count > 0 && count <= height else { return }

        // Remove bottom lines and add blank at top
        lines.removeLast(count)
        for _ in 0..<count {
            lines.insert(Self.blankLine(width: width), at: 0)
        }
    }

    func resize(newWidth: Int, newHeight: Int) {
        // Resize width of existing lines
        var newLines: [ScreenLine] = []

        for i in 0..<min(height, newHeight) {
            var line = lines[i]
            if newWidth > width {
                line.append(contentsOf: [ScreenCell](repeating: .blank, count: newWidth - width))
            } else if newWidth < width {
                line = Array(line.prefix(newWidth))
            }
            newLines.append(line)
        }

        // Add new blank lines if height increased
        while newLines.count < newHeight {
            newLines.append(Self.blankLine(width: newWidth))
        }

        self.width = newWidth
        self.height = newHeight
        self.lines = newLines
    }

    func getHistoryLine(at index: Int) -> ScreenLine? {
        guard index >= 0 && index < history.count else { return nil }
        return history[index].first
    }

    func setCharacter(_ char: UnicodeScalar, at x: Int, y: Int) {
        guard x >= 0 && x < width && y >= 0 && y < height else { return }
        lines[y][x].character = char
    }

    func getCharacter(at x: Int, y: Int) -> UnicodeScalar? {
        guard x >= 0 && x < width && y >= 0 && y < height else { return nil }
        return lines[y][x].character
    }

    func fillRegion(x: Int, y: Int, width: Int, height: Int, with cell: ScreenCell) {
        for row in y..<min(y + height, self.height) {
            for col in x..<min(x + width, self.width) {
                lines[row][col] = cell
            }
        }
    }

    var historyCount: Int { history.count }
}

// MARK: - Screen Buffer Benchmarks

/// Benchmark: Line insertion operations
final class LineInsertionBenchmark: Benchmark {
    let name = "LineInsertion"
    let category: BenchmarkCategory = .screenBuffer
    let description = "Insert lines at various positions"

    private var buffer: SimulatedScreenBuffer!
    private let iterations = 1000

    func setUp() {
        buffer = SimulatedScreenBuffer(width: 132, height: 50)
    }

    func runIteration() -> UInt64 {
        PrecisionTimer.measure {
            for i in 0..<iterations {
                let position = i % buffer.height
                buffer.insertLine(at: position)
            }
        }
    }

    func tearDown() {
        buffer = nil
    }
}

/// Benchmark: Line deletion operations
final class LineDeletionBenchmark: Benchmark {
    let name = "LineDeletion"
    let category: BenchmarkCategory = .screenBuffer
    let description = "Delete lines at various positions"

    private var buffer: SimulatedScreenBuffer!
    private let iterations = 1000

    func setUp() {
        buffer = SimulatedScreenBuffer(width: 132, height: 50)
    }

    func runIteration() -> UInt64 {
        PrecisionTimer.measure {
            for i in 0..<iterations {
                let position = i % buffer.height
                buffer.deleteLine(at: position)
            }
        }
    }

    func tearDown() {
        buffer = nil
    }
}

/// Benchmark: Scroll up operations
final class ScrollUpBenchmark: Benchmark {
    let name = "ScrollUp"
    let category: BenchmarkCategory = .screenBuffer
    let description = "Scroll buffer up by various amounts"

    private var buffer: SimulatedScreenBuffer!
    private let iterations = 500

    func setUp() {
        buffer = SimulatedScreenBuffer(width: 132, height: 50)
    }

    func runIteration() -> UInt64 {
        PrecisionTimer.measure {
            for i in 0..<iterations {
                let scrollAmount = (i % 10) + 1
                buffer.scrollUp(lines: scrollAmount)
            }
        }
    }

    func tearDown() {
        buffer = nil
    }
}

/// Benchmark: Scroll down operations
final class ScrollDownBenchmark: Benchmark {
    let name = "ScrollDown"
    let category: BenchmarkCategory = .screenBuffer
    let description = "Scroll buffer down by various amounts"

    private var buffer: SimulatedScreenBuffer!
    private let iterations = 500

    func setUp() {
        buffer = SimulatedScreenBuffer(width: 132, height: 50)
    }

    func runIteration() -> UInt64 {
        PrecisionTimer.measure {
            for i in 0..<iterations {
                let scrollAmount = (i % 10) + 1
                buffer.scrollDown(lines: scrollAmount)
            }
        }
    }

    func tearDown() {
        buffer = nil
    }
}

/// Benchmark: Resize operations
final class ResizeBenchmark: Benchmark {
    let name = "Resize"
    let category: BenchmarkCategory = .screenBuffer
    let description = "Resize screen buffer"

    private var buffer: SimulatedScreenBuffer!
    private let resizeOperations: [(Int, Int)] = [
        (80, 24), (132, 50), (200, 60), (80, 40),
        (100, 30), (160, 48), (80, 24), (120, 36),
    ]

    func setUp() {
        buffer = SimulatedScreenBuffer(width: 80, height: 24)
    }

    func runIteration() -> UInt64 {
        PrecisionTimer.measure {
            for (width, height) in resizeOperations {
                buffer.resize(newWidth: width, newHeight: height)
            }
        }
    }

    func tearDown() {
        buffer = nil
    }
}

/// Benchmark: History buffer access
final class HistoryAccessBenchmark: Benchmark {
    let name = "HistoryAccess"
    let category: BenchmarkCategory = .screenBuffer
    let description = "Access historical lines"

    private var buffer: SimulatedScreenBuffer!
    private let historySize = 5000
    private let accessCount = 1000

    func setUp() {
        buffer = SimulatedScreenBuffer(width: 132, height: 50, maxHistorySize: historySize)
        // Fill history
        for _ in 0..<historySize {
            buffer.scrollUp(lines: 1)
        }
    }

    func runIteration() -> UInt64 {
        PrecisionTimer.measure {
            var checksum: UInt64 = 0
            for i in 0..<accessCount {
                let index = (i * 7) % buffer.historyCount // Pseudo-random access
                if let line = buffer.getHistoryLine(at: index) {
                    checksum += UInt64(line.count)
                }
            }
            blackHole(checksum)
        }
    }

    func tearDown() {
        buffer = nil
    }
}

/// Benchmark: Character writing
final class CharacterWriteBenchmark: Benchmark {
    let name = "CharacterWrite"
    let category: BenchmarkCategory = .screenBuffer
    let description = "Write characters to buffer"

    private var buffer: SimulatedScreenBuffer!
    private let writeCount = 10000

    func setUp() {
        buffer = SimulatedScreenBuffer(width: 132, height: 50)
    }

    func runIteration() -> UInt64 {
        let chars: [UnicodeScalar] = Array("ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789").compactMap { $0.unicodeScalars.first }

        return PrecisionTimer.measure {
            var x = 0
            var y = 0
            for i in 0..<writeCount {
                let char = chars[i % chars.count]
                buffer.setCharacter(char, at: x, y: y)
                x += 1
                if x >= buffer.width {
                    x = 0
                    y = (y + 1) % buffer.height
                }
            }
        }
    }

    func tearDown() {
        buffer = nil
    }
}

/// Benchmark: Region fill
final class RegionFillBenchmark: Benchmark {
    let name = "RegionFill"
    let category: BenchmarkCategory = .screenBuffer
    let description = "Fill rectangular regions"

    private var buffer: SimulatedScreenBuffer!
    private let fillCount = 100

    func setUp() {
        buffer = SimulatedScreenBuffer(width: 132, height: 50)
    }

    func runIteration() -> UInt64 {
        PrecisionTimer.measure {
            for i in 0..<fillCount {
                let x = (i * 3) % (buffer.width / 2)
                let y = (i * 5) % (buffer.height / 2)
                let w = min(20, buffer.width - x)
                let h = min(10, buffer.height - y)
                buffer.fillRegion(x: x, y: y, width: w, height: h, with: .blank)
            }
        }
    }

    func tearDown() {
        buffer = nil
    }
}

// MARK: - Registration

/// All screen buffer benchmarks
func createScreenBufferBenchmarks() -> [Benchmark] {
    [
        LineInsertionBenchmark(),
        LineDeletionBenchmark(),
        ScrollUpBenchmark(),
        ScrollDownBenchmark(),
        ResizeBenchmark(),
        HistoryAccessBenchmark(),
        CharacterWriteBenchmark(),
        RegionFillBenchmark(),
    ]
}
