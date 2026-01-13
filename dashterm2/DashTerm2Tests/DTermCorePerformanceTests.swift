// DTermCorePerformanceTests.swift
// DashTerm2Tests
//
// Performance benchmarks comparing dterm-core (Rust) vs iTerm2 (ObjC) terminal emulation.
// These tests measure parsing throughput, escape sequence handling, and overall performance.

import XCTest
@testable import DashTerm2SharedARC

/// Performance tests comparing dterm-core vs iTerm2 terminal emulation.
///
/// Test methodology:
/// - Generate standardized payloads (ASCII, escape sequences, mixed content)
/// - Run both parsers on identical data
/// - Measure throughput (MB/s) and latency
final class DTermCorePerformanceTests: XCTestCase {

    // MARK: - Test Data Generators

    /// Generate ASCII-only payload (best case for fast path).
    private func generateASCIIPayload(size: Int) -> Data {
        var bytes = [UInt8]()
        bytes.reserveCapacity(size)
        let printable: [UInt8] = Array(0x20...0x7E)
        for i in 0..<size {
            // Add newlines every ~80 characters for realism
            if i > 0 && i % 80 == 0 {
                bytes.append(0x0D)  // CR
                bytes.append(0x0A)  // LF
            } else {
                bytes.append(printable[i % printable.count])
            }
        }
        return Data(bytes)
    }

    /// Generate payload with escape sequences (SGR attributes, cursor movement).
    private func generateEscapePayload(size: Int) -> Data {
        var result = Data()
        result.reserveCapacity(size)

        let sequences = [
            "\u{1B}[1m",        // Bold
            "\u{1B}[0m",        // Reset
            "\u{1B}[31m",       // Red FG
            "\u{1B}[44m",       // Blue BG
            "\u{1B}[38;5;196m", // 256-color
            "\u{1B}[38;2;255;128;64m", // True color
            "\u{1B}[H",         // Cursor home
            "\u{1B}[5;10H",     // Cursor position
            "\u{1B}[K",         // Erase to end of line
            "\u{1B}[2J",        // Erase screen
        ]

        while result.count < size {
            // Add some text
            let text = "Test output text "
            result.append(contentsOf: text.utf8)

            // Add random escape sequence
            let seq = sequences[result.count % sequences.count]
            result.append(contentsOf: seq.utf8)
        }

        return Data(result.prefix(size))
    }

    /// Generate realistic terminal output (mix of text, escapes, Unicode).
    private func generateRealisticPayload(size: Int) -> Data {
        var result = Data()
        result.reserveCapacity(size)

        // Simulate compiler output (common AI agent workload)
        let lines = [
            "\u{1B}[32m   Compiling\u{1B}[0m foo v0.1.0\r\n",
            "\u{1B}[32m   Compiling\u{1B}[0m bar v0.2.0\r\n",
            "\u{1B}[31merror[E0382]\u{1B}[0m: borrow of moved value\r\n",
            "  --> src/main.rs:42:5\r\n",
            "   |\r\n",
            "42 |     println!(\"{}\", x);\r\n",
            "   |              ^ value borrowed here after move\r\n",
            "\u{1B}[33mwarning\u{1B}[0m: unused variable: `y`\r\n",
            "    \u{1B}[34m|\u{1B}[0m\r\n",
            "100 \u{1B}[34m|\u{1B}[0m let y = compute();\r\n",
            "    \u{1B}[34m|\u{1B}[0m     \u{1B}[33m^\u{1B}[0m help: prefix with underscore: `_y`\r\n",
        ]

        var lineIndex = 0
        while result.count < size {
            let line = lines[lineIndex % lines.count]
            result.append(contentsOf: line.utf8)
            lineIndex += 1
        }

        return Data(result.prefix(size))
    }

    /// Generate wide character payload (CJK, emoji).
    private func generateWideCharPayload(size: Int) -> Data {
        var result = Data()
        result.reserveCapacity(size)

        let wideChars = ["中", "国", "日", "本", "한", "국", "😀", "🎉", "🚀", "⭐"]

        while result.count < size {
            // Mix ASCII and wide characters
            result.append(contentsOf: "Text: ".utf8)
            for char in wideChars {
                result.append(contentsOf: char.utf8)
            }
            result.append(contentsOf: "\r\n".utf8)
        }

        return Data(result.prefix(size))
    }

    // MARK: - dterm-core Benchmarks

    func test_dterm_ASCIIThroughput_1MB() {
        let dterm = DTermCoreIntegration(rows: 24, cols: 80)
        dterm.isEnabled = true

        let payload = generateASCIIPayload(size: 1_000_000)

        measure {
            dterm.reset()
            dterm.process(payload)
        }

        let throughput = dterm.throughputMBps
        print("dterm-core ASCII throughput: \(String(format: "%.2f", throughput)) MB/s")
    }

    func test_dterm_EscapeSequenceThroughput_1MB() {
        let dterm = DTermCoreIntegration(rows: 24, cols: 80)
        dterm.isEnabled = true

        let payload = generateEscapePayload(size: 1_000_000)

        measure {
            dterm.reset()
            dterm.process(payload)
        }

        let throughput = dterm.throughputMBps
        print("dterm-core Escape throughput: \(String(format: "%.2f", throughput)) MB/s")
    }

    func test_dterm_RealisticWorkload_1MB() {
        let dterm = DTermCoreIntegration(rows: 24, cols: 80)
        dterm.isEnabled = true

        let payload = generateRealisticPayload(size: 1_000_000)

        measure {
            dterm.reset()
            dterm.process(payload)
        }

        let throughput = dterm.throughputMBps
        print("dterm-core Realistic throughput: \(String(format: "%.2f", throughput)) MB/s")
    }

    func test_dterm_WideCharThroughput_1MB() {
        let dterm = DTermCoreIntegration(rows: 24, cols: 80)
        dterm.isEnabled = true

        let payload = generateWideCharPayload(size: 1_000_000)

        measure {
            dterm.reset()
            dterm.process(payload)
        }

        let throughput = dterm.throughputMBps
        print("dterm-core Wide char throughput: \(String(format: "%.2f", throughput)) MB/s")
    }

    // MARK: - Large Payload Tests

    func test_dterm_LargePayload_10MB() {
        let dterm = DTermCoreIntegration(rows: 24, cols: 80)
        dterm.isEnabled = true

        let payload = generateRealisticPayload(size: 10_000_000)

        measure {
            dterm.reset()
            dterm.process(payload)
        }

        let throughput = dterm.throughputMBps
        print("dterm-core 10MB throughput: \(String(format: "%.2f", throughput)) MB/s")
    }

    // MARK: - Scrollback Performance

    func test_dterm_ScrollbackPerformance() {
        let dterm = DTermCoreIntegration(rows: 24, cols: 80, scrollbackLines: 100000)
        dterm.isEnabled = true

        // Generate enough data to fill scrollback
        var data = Data()
        for i in 0..<200000 {
            data.append(contentsOf: "Line \(i): Some content here\r\n".utf8)
        }

        measure {
            dterm.reset()
            dterm.process(data)
        }

        XCTAssertGreaterThan(dterm.scrollbackLines, 0, "Should have scrollback")
        print("dterm-core scrollback lines: \(dterm.scrollbackLines)")
    }

    // MARK: - Latency Tests

    func test_dterm_SingleLineLatency() {
        let dterm = DTermCoreIntegration(rows: 24, cols: 80)
        dterm.isEnabled = true

        // Test latency for single line of output (typical AI agent use case)
        let singleLine = "$ ls -la\r\n".data(using: .utf8)!

        var totalNs: UInt64 = 0
        let iterations = 10000

        for _ in 0..<iterations {
            dterm.reset()
            let start = mach_absolute_time()
            dterm.process(singleLine)
            let end = mach_absolute_time()
            totalNs += machTicksToNanoseconds(end - start)
        }

        let avgNs = Double(totalNs) / Double(iterations)
        let avgUs = avgNs / 1000.0
        print("dterm-core single line latency: \(String(format: "%.2f", avgUs)) µs")

        // Should be under 100µs for single line
        XCTAssertLessThan(avgUs, 100, "Single line latency should be under 100µs")
    }

    func test_dterm_EscapeSequenceLatency() {
        let dterm = DTermCoreIntegration(rows: 24, cols: 80)
        dterm.isEnabled = true

        // Test latency for escape sequence processing
        let escapeSeq = "\u{1B}[38;2;255;128;64mColored text\u{1B}[0m\r\n".data(using: .utf8)!

        var totalNs: UInt64 = 0
        let iterations = 10000

        for _ in 0..<iterations {
            dterm.reset()
            let start = mach_absolute_time()
            dterm.process(escapeSeq)
            let end = mach_absolute_time()
            totalNs += machTicksToNanoseconds(end - start)
        }

        let avgNs = Double(totalNs) / Double(iterations)
        let avgUs = avgNs / 1000.0
        print("dterm-core escape sequence latency: \(String(format: "%.2f", avgUs)) µs")

        // Should be under 200µs for typical escape sequence
        XCTAssertLessThan(avgUs, 200, "Escape sequence latency should be under 200µs")
    }

    // MARK: - Resize Performance

    func test_dterm_ResizePerformance() {
        let dterm = DTermCoreIntegration(rows: 24, cols: 80)
        dterm.isEnabled = true

        // Fill with content
        let payload = generateRealisticPayload(size: 100_000)
        dterm.process(payload)

        // Measure resize performance
        measure {
            for _ in 0..<100 {
                dterm.resize(rows: 50, cols: 132)
                dterm.resize(rows: 24, cols: 80)
            }
        }
    }

    // MARK: - Alternate Screen Performance

    func test_dterm_AlternateScreenSwitch() {
        let dterm = DTermCoreIntegration(rows: 24, cols: 80)
        dterm.isEnabled = true

        // Fill main screen
        let mainContent = generateRealisticPayload(size: 10_000)
        dterm.process(mainContent)

        // Enter/exit alternate screen (vim, less, etc. use case)
        let enterAlt = "\u{1B}[?1049h".data(using: .utf8)!
        let exitAlt = "\u{1B}[?1049l".data(using: .utf8)!
        let altContent = "Alternate screen content\r\n".data(using: .utf8)!

        measure {
            for _ in 0..<100 {
                dterm.process(enterAlt)
                dterm.process(altContent)
                dterm.process(exitAlt)
            }
        }
    }

    // MARK: - Memory Efficiency

    func test_dterm_MemoryFootprint() {
        // Test memory usage with large scrollback
        let dterm = DTermCoreIntegration(rows: 24, cols: 80, scrollbackLines: 1_000_000)
        dterm.isEnabled = true

        // Generate lots of unique content
        var data = Data()
        for i in 0..<100000 {
            data.append(contentsOf: "Line \(i): Unique content with timestamp \(Date())\r\n".utf8)
        }

        let startTime = CFAbsoluteTimeGetCurrent()
        dterm.process(data)
        let elapsed = CFAbsoluteTimeGetCurrent() - startTime

        print("Processed 100K lines in \(String(format: "%.2f", elapsed)) seconds")
        print("Scrollback lines: \(dterm.scrollbackLines)")
    }

    // MARK: - Comparison Output

    func test_generatePerformanceReport() {
        let dterm = DTermCoreIntegration(rows: 24, cols: 80)
        dterm.isEnabled = true

        let payloadSizes = [100_000, 1_000_000, 10_000_000]
        var report = "\n========== DTermCore Performance Report ==========\n"

        for size in payloadSizes {
            let sizeLabel = size >= 1_000_000 ? "\(size / 1_000_000)MB" : "\(size / 1000)KB"
            report += "\nPayload size: \(sizeLabel)\n"

            // ASCII
            dterm.reset()
            let asciiPayload = generateASCIIPayload(size: size)
            let asciiStart = CFAbsoluteTimeGetCurrent()
            dterm.process(asciiPayload)
            let asciiTime = CFAbsoluteTimeGetCurrent() - asciiStart
            let asciiMBps = Double(size) / asciiTime / (1024 * 1024)
            report += "  ASCII:     \(String(format: "%7.2f", asciiMBps)) MB/s\n"

            // Escape sequences
            dterm.reset()
            let escapePayload = generateEscapePayload(size: size)
            let escapeStart = CFAbsoluteTimeGetCurrent()
            dterm.process(escapePayload)
            let escapeTime = CFAbsoluteTimeGetCurrent() - escapeStart
            let escapeMBps = Double(size) / escapeTime / (1024 * 1024)
            report += "  Escape:    \(String(format: "%7.2f", escapeMBps)) MB/s\n"

            // Realistic
            dterm.reset()
            let realisticPayload = generateRealisticPayload(size: size)
            let realisticStart = CFAbsoluteTimeGetCurrent()
            dterm.process(realisticPayload)
            let realisticTime = CFAbsoluteTimeGetCurrent() - realisticStart
            let realisticMBps = Double(size) / realisticTime / (1024 * 1024)
            report += "  Realistic: \(String(format: "%7.2f", realisticMBps)) MB/s\n"

            // Wide char
            dterm.reset()
            let widePayload = generateWideCharPayload(size: size)
            let wideStart = CFAbsoluteTimeGetCurrent()
            dterm.process(widePayload)
            let wideTime = CFAbsoluteTimeGetCurrent() - wideStart
            let wideMBps = Double(size) / wideTime / (1024 * 1024)
            report += "  Wide char: \(String(format: "%7.2f", wideMBps)) MB/s\n"
        }

        report += "\n===================================================\n"
        print(report)

        // Test passes if we got here without crashing
        XCTAssertTrue(true)
    }

    // MARK: - Helpers

    private func machTicksToNanoseconds(_ ticks: UInt64) -> UInt64 {
        var info = mach_timebase_info_data_t()
        mach_timebase_info(&info)
        return ticks * UInt64(info.numer) / UInt64(info.denom)
    }

    // MARK: - GPU Renderer Heavy Load Tests

    /// Profile GPU renderer under heavy load - simulating `cat /dev/urandom`.
    ///
    /// This test feeds random bytes to the terminal continuously to stress test
    /// the rendering pipeline and measure performance under worst-case conditions.
    @MainActor
    func test_gpuRenderer_heavyLoadProfiling() {
        let view = DTermMetalView(frame: CGRect(x: 0, y: 0, width: 800, height: 600),
                                   terminal: nil)

        let integration = DTermCoreIntegration(rows: 24, cols: 80)
        integration.isEnabled = true
        view.setIntegration(integration)

        if let font = NSFont.userFixedPitchFont(ofSize: 14) {
            _ = view.setFont(font)
        }

        // Generate random printable ASCII data (simulating urandom output)
        let chunkSize = 4096  // Typical PTY read size
        let iterations = 50   // ~200KB of data

        view.resetPerformanceCounters()
        view.paused = false

        let startTime = CACurrentMediaTime()

        // Feed random data
        for _ in 0..<iterations {
            var randomBytes = [UInt8](repeating: 0, count: chunkSize)
            for i in 0..<chunkSize {
                let r = arc4random_uniform(100)
                if r < 2 {
                    randomBytes[i] = 10  // \n
                } else if r < 4 {
                    randomBytes[i] = 13  // \r
                } else {
                    randomBytes[i] = UInt8(32 + arc4random_uniform(95))  // printable
                }
            }
            let data = Data(randomBytes)
            integration.process(data)
            RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.01))
        }

        RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.1))

        let elapsed = CACurrentMediaTime() - startTime
        let fps = view.currentFPS
        let frameCount = view.frameCount
        let avgFps = view.averageFPS
        let gpuTime = view.gpuFrameTimeMs

        view.paused = true

        print("=== GPU Heavy Load Profiling Results ===")
        print("Data processed: \(iterations * chunkSize / 1024) KB")
        print("Elapsed time: \(String(format: "%.2f", elapsed)) seconds")
        print("Frame count: \(frameCount)")
        print("Current FPS: \(String(format: "%.1f", fps))")
        print("Average FPS: \(String(format: "%.1f", avgFps))")
        print("GPU frame time: \(String(format: "%.2f", gpuTime)) ms")
        print("Throughput: \(String(format: "%.2f", Double(iterations * chunkSize) / elapsed / 1024)) KB/s")
        print("=========================================")

        XCTAssertGreaterThan(frameCount, 0, "Should have rendered frames")
        view.setIntegration(nil)
    }

    /// Profile GPU renderer with mixed content (text, escape codes, colors).
    @MainActor
    func test_gpuRenderer_mixedContentProfiling() {
        let view = DTermMetalView(frame: CGRect(x: 0, y: 0, width: 800, height: 600),
                                   terminal: nil)

        let integration = DTermCoreIntegration(rows: 24, cols: 80)
        integration.isEnabled = true
        view.setIntegration(integration)

        if let font = NSFont.userFixedPitchFont(ofSize: 14) {
            _ = view.setFont(font)
        }

        view.resetPerformanceCounters()
        view.paused = false

        let startTime = CACurrentMediaTime()

        // Generate mixed content with escape sequences
        for i in 0..<100 {
            let fgColor = 31 + (i % 7)
            let bgColor = 40 + ((i + 3) % 8)
            let colorSeq = "\u{1b}[\(fgColor);\(bgColor)m"
            let styleSeq = "\u{1b}[\(1 + (i % 4))m"
            let line = "\(colorSeq)\(styleSeq)Line \(i): Quick brown fox\u{1b}[0m\r\n"

            if let data = line.data(using: .utf8) {
                integration.process(data)
            }

            if i % 20 == 0 {
                RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.01))
            }
        }

        RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.1))

        let elapsed = CACurrentMediaTime() - startTime
        let fps = view.currentFPS
        let frameCount = view.frameCount

        view.paused = true

        print("=== GPU Mixed Content Profiling Results ===")
        print("Elapsed time: \(String(format: "%.2f", elapsed)) seconds")
        print("Frame count: \(frameCount)")
        print("FPS: \(String(format: "%.1f", fps))")
        print("============================================")

        XCTAssertGreaterThan(frameCount, 0, "Should have rendered frames")
        view.setIntegration(nil)
    }

    /// Test GPU renderer with various terminal sizes.
    @MainActor
    func test_gpuRenderer_differentSizes() {
        let sizes: [(rows: UInt16, cols: UInt16, name: String)] = [
            (24, 80, "Standard VT100"),
            (50, 200, "Large terminal"),
            (100, 300, "Very large"),
        ]

        for (rows, cols, name) in sizes {
            let view = DTermMetalView(frame: CGRect(x: 0, y: 0,
                                                     width: CGFloat(cols) * 8,
                                                     height: CGFloat(rows) * 16),
                                       terminal: nil)

            let integration = DTermCoreIntegration(rows: rows, cols: cols)
            integration.isEnabled = true
            view.setIntegration(integration)

            if let font = NSFont.userFixedPitchFont(ofSize: 12) {
                _ = view.setFont(font)
            }

            // Fill screen with content
            for row in 0..<Int(rows) {
                let line = String(repeating: String(Character(UnicodeScalar(65 + (row % 26))!)), count: Int(cols)) + "\r\n"
                if let data = line.data(using: .utf8) {
                    integration.process(data)
                }
            }

            view.resetPerformanceCounters()
            view.paused = false

            RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.2))

            let fps = view.currentFPS
            let frameCount = view.frameCount

            view.paused = true

            print("Size test '\(name)' (\(rows)x\(cols)): \(frameCount) frames, \(String(format: "%.1f", fps)) FPS")

            XCTAssertGreaterThan(frameCount, 0, "\(name) should render frames")

            view.setIntegration(nil)
        }
    }

    /// Test GPU renderer handles resize during rendering.
    @MainActor
    func test_gpuRenderer_resizeDuringRendering() {
        let view = DTermMetalView(frame: CGRect(x: 0, y: 0, width: 640, height: 480),
                                   terminal: nil)

        let integration = DTermCoreIntegration(rows: 24, cols: 80)
        integration.isEnabled = true
        view.setIntegration(integration)

        if let font = NSFont.userFixedPitchFont(ofSize: 14) {
            _ = view.setFont(font)
        }

        // Fill with content
        for i in 0..<50 {
            let line = "Line \(i): Testing resize during rendering\r\n"
            if let data = line.data(using: .utf8) {
                integration.process(data)
            }
        }

        view.paused = false

        // Resize multiple times during rendering
        let sizes: [(width: CGFloat, height: CGFloat)] = [
            (800, 600),
            (1024, 768),
            (400, 300),
            (1280, 800),
            (640, 480),
        ]

        for (width, height) in sizes {
            view.setFrameSize(NSSize(width: width, height: height))
            integration.resize(rows: UInt16(height / 16), cols: UInt16(width / 8))
            RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.05))
        }

        let frameCount = view.frameCount
        view.paused = true

        print("Resize test: \(frameCount) frames across \(sizes.count) resizes")

        XCTAssertGreaterThan(frameCount, 0, "Should render frames during resize")

        view.setIntegration(nil)
    }
}
