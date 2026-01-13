//
//  BenchmarkProtocol.swift
//  DashTerm2 Benchmarks
//
//  Performance benchmark protocol and supporting types.
//

import Foundation

/// Result of a single benchmark measurement
struct BenchmarkMeasurement {
    let durationNanoseconds: UInt64
    let memoryBytesUsed: Int64
    let additionalMetrics: [String: Double]

    var durationMilliseconds: Double {
        Double(durationNanoseconds) / 1_000_000.0
    }

    var durationMicroseconds: Double {
        Double(durationNanoseconds) / 1_000.0
    }
}

/// Aggregated statistics for a benchmark
struct BenchmarkStatistics: Codable {
    let name: String
    let category: String
    let iterations: Int
    let meanMs: Double
    let stddevMs: Double
    let minMs: Double
    let maxMs: Double
    let p50Ms: Double
    let p75Ms: Double
    let p95Ms: Double
    let p99Ms: Double
    let variance: Double
    let memoryBytes: Int64?
    let additionalMetrics: [String: Double]?

    var isReliable: Bool {
        // Consider reliable if coefficient of variation < 10%
        guard meanMs > 0 else { return false }
        return (stddevMs / meanMs) < 0.10
    }
}

/// Benchmark category for grouping related benchmarks
enum BenchmarkCategory: String, Codable, CaseIterable {
    case textRendering = "Text Rendering"
    case screenBuffer = "Screen Buffer"
    case memory = "Memory"
    case metalRenderer = "Metal Renderer"

    var requiresGUI: Bool {
        switch self {
        case .metalRenderer:
            return true
        default:
            return false
        }
    }
}

/// Configuration for running benchmarks
struct BenchmarkConfiguration {
    /// Number of warmup iterations before measurement
    let warmupIterations: Int

    /// Number of measurement iterations
    let measurementIterations: Int

    /// Minimum time to run benchmark (may override iteration count)
    let minimumDuration: TimeInterval

    /// Whether to collect memory metrics
    let collectMemoryMetrics: Bool

    /// Categories to run
    let categories: Set<BenchmarkCategory>

    /// Output format
    let outputFormat: OutputFormat

    /// Comparison baseline path
    let baselinePath: String?

    /// Regression threshold percentage
    let regressionThreshold: Double

    enum OutputFormat {
        case console
        case json
        case both
    }

    static var `default`: BenchmarkConfiguration {
        BenchmarkConfiguration(
            warmupIterations: 5,
            measurementIterations: 100,
            minimumDuration: 1.0,
            collectMemoryMetrics: true,
            categories: Set(BenchmarkCategory.allCases.filter { !$0.requiresGUI }),
            outputFormat: .console,
            baselinePath: nil,
            regressionThreshold: 10.0
        )
    }

    static var quick: BenchmarkConfiguration {
        BenchmarkConfiguration(
            warmupIterations: 2,
            measurementIterations: 10,
            minimumDuration: 0.5,
            collectMemoryMetrics: false,
            categories: Set(BenchmarkCategory.allCases.filter { !$0.requiresGUI }),
            outputFormat: .console,
            baselinePath: nil,
            regressionThreshold: 10.0
        )
    }
}

/// Protocol for implementing a benchmark
protocol Benchmark {
    /// Unique name for this benchmark
    var name: String { get }

    /// Category this benchmark belongs to
    var category: BenchmarkCategory { get }

    /// Description of what this benchmark measures
    var description: String { get }

    /// Set up any required state before benchmark runs
    func setUp()

    /// Tear down state after benchmark completes
    func tearDown()

    /// Run one iteration of the benchmark
    /// Returns duration in nanoseconds
    func runIteration() -> UInt64

    /// Optional: Additional metrics to collect
    func collectAdditionalMetrics() -> [String: Double]
}

extension Benchmark {
    func setUp() {}
    func tearDown() {}
    func collectAdditionalMetrics() -> [String: Double] { [:] }
}

/// High-precision timer using mach_absolute_time
final class PrecisionTimer {
    private static var timebaseInfo: mach_timebase_info = {
        var info = mach_timebase_info()
        mach_timebase_info(&info)
        return info
    }()

    private var startTime: UInt64 = 0

    func start() {
        startTime = mach_absolute_time()
    }

    func stop() -> UInt64 {
        let endTime = mach_absolute_time()
        let elapsed = endTime - startTime
        return elapsed * UInt64(Self.timebaseInfo.numer) / UInt64(Self.timebaseInfo.denom)
    }

    static func measure(_ block: () -> Void) -> UInt64 {
        let timer = PrecisionTimer()
        timer.start()
        block()
        return timer.stop()
    }
}

/// Memory tracking utilities
final class MemoryTracker {
    struct Snapshot {
        let residentSize: Int64
        let virtualSize: Int64
        let peakResident: Int64

        static func current() -> Snapshot {
            var info = task_vm_info_data_t()
            var count = mach_msg_type_number_t(MemoryLayout<task_vm_info_data_t>.size / MemoryLayout<natural_t>.size)

            let result = withUnsafeMutablePointer(to: &info) { infoPtr in
                infoPtr.withMemoryRebound(to: integer_t.self, capacity: Int(count)) { ptr in
                    task_info(mach_task_self_, task_flavor_t(TASK_VM_INFO), ptr, &count)
                }
            }

            guard result == KERN_SUCCESS else {
                return Snapshot(residentSize: 0, virtualSize: 0, peakResident: 0)
            }

            return Snapshot(
                residentSize: Int64(info.phys_footprint),
                virtualSize: Int64(info.virtual_size),
                peakResident: Int64(info.resident_size_peak)
            )
        }
    }

    private var baseline: Snapshot?

    func captureBaseline() {
        baseline = Snapshot.current()
    }

    func measureDelta() -> Int64 {
        let current = Snapshot.current()
        guard let baseline = baseline else { return current.residentSize }
        return current.residentSize - baseline.residentSize
    }
}
