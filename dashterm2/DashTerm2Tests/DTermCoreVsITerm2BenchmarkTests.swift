// DTermCoreVsITerm2BenchmarkTests.swift
// DashTerm2Tests
//
// Head-to-head comparison benchmarks: dterm-core (Rust) vs iTerm2 (ObjC).
// These tests feed identical input to both parsers and compare throughput.
//
// Run with:
// xcodebuild test -project DashTerm2.xcodeproj -scheme DashTerm2Tests \
//   -only-testing:DashTerm2Tests/DTermCoreVsITerm2BenchmarkTests

import XCTest
@testable import DashTerm2SharedARC

/// Side-by-side comparison benchmarks for terminal parsing.
///
/// Key metrics:
/// - **Throughput (MB/s)**: Higher is better
/// - **Latency (µs)**: Lower is better for interactive responsiveness
///
/// Test methodology:
/// 1. Generate identical payloads for both parsers
/// 2. Warm up to reduce JIT/cache effects
/// 3. Run multiple iterations and report averages
final class DTermCoreVsITerm2BenchmarkTests: XCTestCase {

    // MARK: - Test Configuration

    // Note: Large payloads (>100KB) can trigger edge cases in VT100Parser
    // under AddressSanitizer. Use smaller sizes for reliable ASAN testing.
    private let payloadSizes = [10_000, 50_000, 100_000]
    private let iterations = 10
    private let warmupIterations = 3

    // MARK: - Payload Generators

    /// Generate ASCII-only payload (fast path for both parsers).
    private func generateASCIIPayload(size: Int) -> Data {
        var bytes = [UInt8]()
        bytes.reserveCapacity(size)
        let printable: [UInt8] = Array(0x20...0x7E)
        for i in 0..<size {
            if i > 0 && i % 80 == 0 {
                bytes.append(0x0D)  // CR
                bytes.append(0x0A)  // LF
            } else {
                bytes.append(printable[i % printable.count])
            }
        }
        return Data(bytes)
    }

    /// Generate payload with SGR escape sequences.
    private func generateSGRPayload(size: Int) -> Data {
        var result = Data()
        result.reserveCapacity(size)

        let sequences = [
            "\u{1B}[1m",        // Bold
            "\u{1B}[0m",        // Reset
            "\u{1B}[31m",       // Red FG
            "\u{1B}[44m",       // Blue BG
            "\u{1B}[38;5;196m", // 256-color
            "\u{1B}[38;2;255;128;64m", // True color
        ]

        while result.count < size {
            result.append(contentsOf: "Text ".utf8)
            let seq = sequences[result.count % sequences.count]
            result.append(contentsOf: seq.utf8)
        }

        return Data(result.prefix(size))
    }

    /// Generate realistic compiler-like output.
    private func generateRealisticPayload(size: Int) -> Data {
        var result = Data()
        result.reserveCapacity(size)

        let lines = [
            "\u{1B}[32m   Compiling\u{1B}[0m foo v0.1.0\r\n",
            "\u{1B}[32m   Compiling\u{1B}[0m bar v0.2.0\r\n",
            "\u{1B}[31merror[E0382]\u{1B}[0m: borrow of moved value\r\n",
            "  --> src/main.rs:42:5\r\n",
            "   |\r\n",
            "42 |     println!(\"{}\", x);\r\n",
            "   |              ^ value borrowed here\r\n",
            "\u{1B}[33mwarning\u{1B}[0m: unused variable\r\n",
        ]

        var lineIndex = 0
        while result.count < size {
            let line = lines[lineIndex % lines.count]
            result.append(contentsOf: line.utf8)
            lineIndex += 1
        }

        return Data(result.prefix(size))
    }

    /// Generate cursor movement payload (simulates vim/htop-like updates).
    ///
    /// This is a critical workload for interactive terminal apps that do
    /// rapid cursor positioning (editors, TUIs, status bars).
    private func generateCursorPayload(size: Int) -> Data {
        var result = Data()
        result.reserveCapacity(size)

        // Mix of cursor movement sequences used by real TUI apps
        let sequences = [
            "\u{1B}[H",           // Home (CUP 1,1)
            "\u{1B}[5;10H",       // CUP row 5, col 10
            "\u{1B}[A",           // Cursor up
            "\u{1B}[B",           // Cursor down
            "\u{1B}[C",           // Cursor forward
            "\u{1B}[D",           // Cursor backward
            "\u{1B}[10G",         // CHA - cursor to column 10
            "\u{1B}[5d",          // VPA - cursor to row 5
            "\u{1B}[2J",          // Clear screen
            "\u{1B}[K",           // Clear to end of line
            "\u{1B}[1;24r",       // Set scroll region
            "\u{1B}7",            // Save cursor (DECSC)
            "\u{1B}8",            // Restore cursor (DECRC)
        ]

        var seqIndex = 0
        while result.count < size {
            // Alternate between cursor sequence and short text
            let seq = sequences[seqIndex % sequences.count]
            result.append(contentsOf: seq.utf8)
            result.append(contentsOf: "text".utf8)  // Short text between movements
            seqIndex += 1
        }

        return Data(result.prefix(size))
    }

    /// Generate erase/edit payload (simulates terminal redraws).
    ///
    /// This workload tests sequences that modify the screen buffer:
    /// insert/delete lines and characters, erase operations.
    private func generateErasePayload(size: Int) -> Data {
        var result = Data()
        result.reserveCapacity(size)

        let sequences = [
            "\u{1B}[K",           // Erase to end of line (EL 0)
            "\u{1B}[1K",          // Erase to start of line (EL 1)
            "\u{1B}[2K",          // Erase entire line (EL 2)
            "\u{1B}[J",           // Erase to end of screen (ED 0)
            "\u{1B}[1J",          // Erase to start of screen (ED 1)
            "\u{1B}[L",           // Insert line (IL)
            "\u{1B}[M",           // Delete line (DL)
            "\u{1B}[@",           // Insert character (ICH)
            "\u{1B}[P",           // Delete character (DCH)
            "\u{1B}[X",           // Erase character (ECH)
            "\u{1B}[3L",          // Insert 3 lines
            "\u{1B}[2M",          // Delete 2 lines
            "\u{1B}[5@",          // Insert 5 characters
            "\u{1B}[4P",          // Delete 4 characters
        ]

        var seqIndex = 0
        while result.count < size {
            let seq = sequences[seqIndex % sequences.count]
            result.append(contentsOf: seq.utf8)
            result.append(contentsOf: "line\r\n".utf8)  // Content between operations
            seqIndex += 1
        }

        return Data(result.prefix(size))
    }

    /// Generate OSC payload (window titles, hyperlinks, clipboard).
    ///
    /// This workload tests OSC (Operating System Command) sequences:
    /// - OSC 0/1/2: Set window/icon titles
    /// - OSC 8: Hyperlinks
    /// - OSC 52: Clipboard operations
    /// - OSC 4: Set/query color palette
    private func generateOSCPayload(size: Int) -> Data {
        var result = Data()
        result.reserveCapacity(size)

        let sequences = [
            "\u{1B}]0;Window Title\u{07}",                    // OSC 0: Set window+icon title
            "\u{1B}]2;Just Window Title\u{07}",               // OSC 2: Set window title only
            "\u{1B}]1;Icon Title\u{07}",                      // OSC 1: Set icon title only
            "\u{1B}]8;;https://example.com\u{07}",           // OSC 8: Start hyperlink
            "\u{1B}]8;;\u{07}",                               // OSC 8: End hyperlink
            "\u{1B}]8;id=link1;https://github.com\u{07}",    // OSC 8: Hyperlink with ID
            "\u{1B}]4;1;rgb:ff/00/00\u{07}",                 // OSC 4: Set palette color 1 to red
            "\u{1B}]4;2;rgb:00/ff/00\u{07}",                 // OSC 4: Set palette color 2 to green
            "\u{1B}]7;file:///Users/test/project\u{07}",     // OSC 7: Set working directory
            "\u{1B}]0;Terminal - ~/project\u{1B}\\",         // OSC 0 with ST terminator
        ]

        var seqIndex = 0
        while result.count < size {
            let seq = sequences[seqIndex % sequences.count]
            result.append(contentsOf: seq.utf8)
            result.append(contentsOf: "text output\r\n".utf8)
            seqIndex += 1
        }

        return Data(result.prefix(size))
    }

    /// Generate hyperlink-heavy payload (simulates ls --hyperlink output).
    ///
    /// Modern terminals support OSC 8 hyperlinks. This tests the common
    /// pattern of file listings with clickable links.
    private func generateHyperlinkPayload(size: Int) -> Data {
        var result = Data()
        result.reserveCapacity(size)

        // Simulate `ls --hyperlink` output
        let files = [
            ("README.md", "/home/user/project/README.md"),
            ("src", "/home/user/project/src"),
            ("Cargo.toml", "/home/user/project/Cargo.toml"),
            ("main.rs", "/home/user/project/src/main.rs"),
            ("lib.rs", "/home/user/project/src/lib.rs"),
        ]

        var fileIndex = 0
        while result.count < size {
            let (name, path) = files[fileIndex % files.count]
            // OSC 8 hyperlink format: ESC ] 8 ; params ; uri ST text ESC ] 8 ; ; ST
            let line = "\u{1B}]8;;file://\(path)\u{07}\(name)\u{1B}]8;;\u{07}  "
            result.append(contentsOf: line.utf8)

            if fileIndex % 5 == 4 {
                result.append(contentsOf: "\r\n".utf8)
            }
            fileIndex += 1
        }

        return Data(result.prefix(size))
    }

    // MARK: - Benchmark Infrastructure

    private func machTicksToNanoseconds(_ ticks: UInt64) -> UInt64 {
        var info = mach_timebase_info_data_t()
        mach_timebase_info(&info)
        return ticks * UInt64(info.numer) / UInt64(info.denom)
    }

    private func formatSize(_ bytes: Int) -> String {
        if bytes >= 1_000_000 {
            return "\(bytes / 1_000_000)MB"
        } else {
            return "\(bytes / 1000)KB"
        }
    }

    // MARK: - dterm-core Benchmarks

    private func benchmarkDTermCore(_ payload: Data) -> (throughputMBps: Double, avgLatencyNs: Double) {
        let dterm = DTermCoreIntegration(rows: 24, cols: 80)
        dterm.isEnabled = true

        // Warmup
        for _ in 0..<warmupIterations {
            dterm.reset()
            dterm.process(payload)
        }

        // Measure
        var totalNs: UInt64 = 0
        for _ in 0..<iterations {
            dterm.reset()
            let start = mach_absolute_time()
            dterm.process(payload)
            let elapsed = mach_absolute_time() - start
            totalNs += machTicksToNanoseconds(elapsed)
        }

        let avgNs = Double(totalNs) / Double(iterations)
        let throughput = Double(payload.count) / avgNs * 1e9 / (1024 * 1024)
        return (throughput, avgNs)
    }

    // MARK: - iTerm2 Parser Benchmarks

    private func benchmarkVT100Parser(_ payload: Data) -> (throughputMBps: Double, avgLatencyNs: Double) {
        // Estimate token count: roughly 1 token per 10 bytes for typical terminal output
        // Add extra capacity to avoid reallocation during benchmark
        let estimatedTokens = max(1024, payload.count / 10)

        // Note: We keep the parser alive across iterations because VT100Token objects
        // may reference data owned by the parser. Releasing the parser before
        // releasing tokens causes use-after-free crashes under AddressSanitizer.
        let parser = VT100Parser()
        parser.encoding = NSASCIIStringEncoding

        // Warmup
        for _ in 0..<warmupIterations {
            parser.clearStream()
            payload.withUnsafeBytes { ptr in
                parser.putStreamData(ptr.baseAddress?.assumingMemoryBound(to: CChar.self), length: Int32(payload.count))
            }
            var vector = CVector()
            CVectorCreate(&vector, Int32(estimatedTokens))
            _ = parser.addParsedTokens(to: &vector)
            // Recycle tokens back to the pool while parser is still alive
            CVectorRecycleVT100TokensAndDestroy(&vector)
        }

        // Measure
        var totalNs: UInt64 = 0
        for _ in 0..<iterations {
            parser.clearStream()
            let start = mach_absolute_time()
            payload.withUnsafeBytes { ptr in
                parser.putStreamData(ptr.baseAddress?.assumingMemoryBound(to: CChar.self), length: Int32(payload.count))
            }
            var vector = CVector()
            CVectorCreate(&vector, Int32(estimatedTokens))
            _ = parser.addParsedTokens(to: &vector)
            let elapsed = mach_absolute_time() - start
            totalNs += machTicksToNanoseconds(elapsed)
            // Recycle tokens back to the pool while parser is still alive
            CVectorRecycleVT100TokensAndDestroy(&vector)
        }

        let avgNs = Double(totalNs) / Double(iterations)
        let throughput = Double(payload.count) / avgNs * 1e9 / (1024 * 1024)
        return (throughput, avgNs)
    }

    // MARK: - Comparison Tests

    func test_comparison_ASCII() {
        print("\n========== ASCII Payload Comparison ==========")
        print("Size       dterm-core      VT100Parser     Speedup")
        print(String(repeating: "-", count: 55))

        for size in payloadSizes {
            let payload = generateASCIIPayload(size: size)

            let dtermResult = benchmarkDTermCore(payload)
            let vt100Result = benchmarkVT100Parser(payload)

            let speedup = dtermResult.throughputMBps / max(vt100Result.throughputMBps, 0.001)

            let sizeStr = formatSize(size).padding(toLength: 10, withPad: " ", startingAt: 0)
            let dtermStr = String(format: "%.1f MB/s", dtermResult.throughputMBps).padding(toLength: 15, withPad: " ", startingAt: 0)
            let vt100Str = String(format: "%.1f MB/s", vt100Result.throughputMBps).padding(toLength: 15, withPad: " ", startingAt: 0)
            let speedupStr = String(format: "%.2fx", speedup)
            print("\(sizeStr)\(dtermStr)\(vt100Str)\(speedupStr)")
        }

        print("==============================================\n")
        XCTAssertTrue(true, "Benchmark completed")
    }

    func test_comparison_SGR() {
        print("\n========== SGR Escape Sequences Comparison ==========")
        print("Size       dterm-core      VT100Parser     Speedup")
        print(String(repeating: "-", count: 55))

        for size in payloadSizes {
            let payload = generateSGRPayload(size: size)

            let dtermResult = benchmarkDTermCore(payload)
            let vt100Result = benchmarkVT100Parser(payload)

            let speedup = dtermResult.throughputMBps / max(vt100Result.throughputMBps, 0.001)

            let sizeStr = formatSize(size).padding(toLength: 10, withPad: " ", startingAt: 0)
            let dtermStr = String(format: "%.1f MB/s", dtermResult.throughputMBps).padding(toLength: 15, withPad: " ", startingAt: 0)
            let vt100Str = String(format: "%.1f MB/s", vt100Result.throughputMBps).padding(toLength: 15, withPad: " ", startingAt: 0)
            let speedupStr = String(format: "%.2fx", speedup)
            print("\(sizeStr)\(dtermStr)\(vt100Str)\(speedupStr)")
        }

        print("=====================================================\n")
        XCTAssertTrue(true, "Benchmark completed")
    }

    func test_comparison_Realistic() {
        print("\n========== Realistic Workload Comparison ==========")
        print("Size       dterm-core      VT100Parser     Speedup")
        print(String(repeating: "-", count: 55))

        for size in payloadSizes {
            let payload = generateRealisticPayload(size: size)

            let dtermResult = benchmarkDTermCore(payload)
            let vt100Result = benchmarkVT100Parser(payload)

            let speedup = dtermResult.throughputMBps / max(vt100Result.throughputMBps, 0.001)

            let sizeStr = formatSize(size).padding(toLength: 10, withPad: " ", startingAt: 0)
            let dtermStr = String(format: "%.1f MB/s", dtermResult.throughputMBps).padding(toLength: 15, withPad: " ", startingAt: 0)
            let vt100Str = String(format: "%.1f MB/s", vt100Result.throughputMBps).padding(toLength: 15, withPad: " ", startingAt: 0)
            let speedupStr = String(format: "%.2fx", speedup)
            print("\(sizeStr)\(dtermStr)\(vt100Str)\(speedupStr)")
        }

        print("===================================================\n")
        XCTAssertTrue(true, "Benchmark completed")
    }

    func test_comparison_Cursor() {
        print("\n========== Cursor Movement Comparison ==========")
        print("Size       dterm-core      VT100Parser     Speedup")
        print(String(repeating: "-", count: 55))

        for size in payloadSizes {
            let payload = generateCursorPayload(size: size)

            let dtermResult = benchmarkDTermCore(payload)
            let vt100Result = benchmarkVT100Parser(payload)

            let speedup = dtermResult.throughputMBps / max(vt100Result.throughputMBps, 0.001)

            let sizeStr = formatSize(size).padding(toLength: 10, withPad: " ", startingAt: 0)
            let dtermStr = String(format: "%.1f MB/s", dtermResult.throughputMBps).padding(toLength: 15, withPad: " ", startingAt: 0)
            let vt100Str = String(format: "%.1f MB/s", vt100Result.throughputMBps).padding(toLength: 15, withPad: " ", startingAt: 0)
            let speedupStr = String(format: "%.2fx", speedup)
            print("\(sizeStr)\(dtermStr)\(vt100Str)\(speedupStr)")
        }

        print("================================================\n")
        XCTAssertTrue(true, "Benchmark completed")
    }

    func test_comparison_Erase() {
        print("\n========== Erase/Edit Operations Comparison ==========")
        print("Size       dterm-core      VT100Parser     Speedup")
        print(String(repeating: "-", count: 55))

        for size in payloadSizes {
            let payload = generateErasePayload(size: size)

            let dtermResult = benchmarkDTermCore(payload)
            let vt100Result = benchmarkVT100Parser(payload)

            let speedup = dtermResult.throughputMBps / max(vt100Result.throughputMBps, 0.001)

            let sizeStr = formatSize(size).padding(toLength: 10, withPad: " ", startingAt: 0)
            let dtermStr = String(format: "%.1f MB/s", dtermResult.throughputMBps).padding(toLength: 15, withPad: " ", startingAt: 0)
            let vt100Str = String(format: "%.1f MB/s", vt100Result.throughputMBps).padding(toLength: 15, withPad: " ", startingAt: 0)
            let speedupStr = String(format: "%.2fx", speedup)
            print("\(sizeStr)\(dtermStr)\(vt100Str)\(speedupStr)")
        }

        print("====================================================\n")
        XCTAssertTrue(true, "Benchmark completed")
    }

    func test_comparison_OSC() {
        print("\n========== OSC Sequences Comparison ==========")
        print("Size       dterm-core      VT100Parser     Speedup")
        print(String(repeating: "-", count: 55))

        for size in payloadSizes {
            let payload = generateOSCPayload(size: size)

            let dtermResult = benchmarkDTermCore(payload)
            let vt100Result = benchmarkVT100Parser(payload)

            let speedup = dtermResult.throughputMBps / max(vt100Result.throughputMBps, 0.001)

            let sizeStr = formatSize(size).padding(toLength: 10, withPad: " ", startingAt: 0)
            let dtermStr = String(format: "%.1f MB/s", dtermResult.throughputMBps).padding(toLength: 15, withPad: " ", startingAt: 0)
            let vt100Str = String(format: "%.1f MB/s", vt100Result.throughputMBps).padding(toLength: 15, withPad: " ", startingAt: 0)
            let speedupStr = String(format: "%.2fx", speedup)
            print("\(sizeStr)\(dtermStr)\(vt100Str)\(speedupStr)")
        }

        print("==============================================\n")
        XCTAssertTrue(true, "Benchmark completed")
    }

    func test_comparison_Hyperlinks() {
        print("\n========== Hyperlinks (OSC 8) Comparison ==========")
        print("Size       dterm-core      VT100Parser     Speedup")
        print(String(repeating: "-", count: 55))

        for size in payloadSizes {
            let payload = generateHyperlinkPayload(size: size)

            let dtermResult = benchmarkDTermCore(payload)
            let vt100Result = benchmarkVT100Parser(payload)

            let speedup = dtermResult.throughputMBps / max(vt100Result.throughputMBps, 0.001)

            let sizeStr = formatSize(size).padding(toLength: 10, withPad: " ", startingAt: 0)
            let dtermStr = String(format: "%.1f MB/s", dtermResult.throughputMBps).padding(toLength: 15, withPad: " ", startingAt: 0)
            let vt100Str = String(format: "%.1f MB/s", vt100Result.throughputMBps).padding(toLength: 15, withPad: " ", startingAt: 0)
            let speedupStr = String(format: "%.2fx", speedup)
            print("\(sizeStr)\(dtermStr)\(vt100Str)\(speedupStr)")
        }

        print("=================================================\n")
        XCTAssertTrue(true, "Benchmark completed")
    }

    func test_generateComparisonReport() {
        var report = """

        ============================================================
        DTermCore vs iTerm2 Parser Comparison Report
        ============================================================
        Date: \(Date())
        Iterations: \(iterations) (+ \(warmupIterations) warmup)

        """

        let workloads: [(name: String, generator: (Int) -> Data)] = [
            ("ASCII", generateASCIIPayload),
            ("SGR Escapes", generateSGRPayload),
            ("Cursor Movement", generateCursorPayload),
            ("Erase/Edit", generateErasePayload),
            ("OSC Sequences", generateOSCPayload),
            ("Hyperlinks", generateHyperlinkPayload),
            ("Realistic", generateRealisticPayload),
        ]

        for (workloadName, generator) in workloads {
            report += "\n--- \(workloadName) Workload ---\n"
            report += "Size       dterm-core      VT100Parser     Speedup\n"

            for size in payloadSizes {
                let payload = generator(size)

                let dtermResult = benchmarkDTermCore(payload)
                let vt100Result = benchmarkVT100Parser(payload)

                let speedup = dtermResult.throughputMBps / max(vt100Result.throughputMBps, 0.001)

                let sizeStr = formatSize(size).padding(toLength: 10, withPad: " ", startingAt: 0)
                let dtermStr = String(format: "%.1f MB/s", dtermResult.throughputMBps).padding(toLength: 15, withPad: " ", startingAt: 0)
                let vt100Str = String(format: "%.1f MB/s", vt100Result.throughputMBps).padding(toLength: 15, withPad: " ", startingAt: 0)
                let speedupStr = String(format: "%.2fx", speedup)
                report += "\(sizeStr)\(dtermStr)\(vt100Str)\(speedupStr)\n"
            }
        }

        report += """

        ============================================================
        Notes:
        - dterm-core: Rust terminal emulator core with FFI bridge
        - VT100Parser: iTerm2's Objective-C parser
        - Speedup > 1.0x means dterm-core is faster
        - These benchmarks measure parsing only, not grid updates
        ============================================================

        """

        print(report)

        XCTAssertTrue(true, "Report generated successfully")
    }

    // MARK: - Single Line Latency (Interactive Feel)

    func test_singleLineLatency() {
        let singleLine = "$ ls -la\r\n".data(using: .utf8)!
        let iterations = 10000

        print("\n========== Single Line Latency (µs) ==========")

        // dterm-core
        let dterm = DTermCoreIntegration(rows: 24, cols: 80)
        dterm.isEnabled = true

        var dtermTotalNs: UInt64 = 0
        for _ in 0..<iterations {
            dterm.reset()
            let start = mach_absolute_time()
            dterm.process(singleLine)
            let elapsed = mach_absolute_time() - start
            dtermTotalNs += machTicksToNanoseconds(elapsed)
        }
        let dtermAvgUs = Double(dtermTotalNs) / Double(iterations) / 1000.0

        // VT100Parser - keep parser alive while tokens exist
        let parser = VT100Parser()
        parser.encoding = NSASCIIStringEncoding
        var vt100TotalNs: UInt64 = 0
        for _ in 0..<iterations {
            parser.clearStream()
            let start = mach_absolute_time()
            singleLine.withUnsafeBytes { ptr in
                parser.putStreamData(ptr.baseAddress?.assumingMemoryBound(to: CChar.self), length: Int32(singleLine.count))
            }
            var vector = CVector()
            CVectorCreate(&vector, 64)
            _ = parser.addParsedTokens(to: &vector)
            let elapsed = mach_absolute_time() - start
            vt100TotalNs += machTicksToNanoseconds(elapsed)
            // Recycle tokens back to the pool while parser is still alive
            CVectorRecycleVT100TokensAndDestroy(&vector)
        }
        let vt100AvgUs = Double(vt100TotalNs) / Double(iterations) / 1000.0

        let speedup = vt100AvgUs / dtermAvgUs

        print(String(format: "dterm-core:   %6.2f µs", dtermAvgUs))
        print(String(format: "VT100Parser:  %6.2f µs", vt100AvgUs))
        print(String(format: "Speedup:      %6.2fx", speedup))
        print("==============================================\n")

        // Both should be under 100µs for good interactive feel
        XCTAssertLessThan(dtermAvgUs, 100, "dterm-core should be under 100µs")
    }
}
