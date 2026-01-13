//
//  BenchmarkRunner.swift
//  DashTerm2 Benchmarks
//
//  Orchestrates benchmark execution and result collection.
//

import Foundation

/// Runs benchmarks and collects results
final class BenchmarkRunner {
    private let configuration: BenchmarkConfiguration
    private var benchmarks: [Benchmark] = []
    private var results: [BenchmarkStatistics] = []

    init(configuration: BenchmarkConfiguration = .default) {
        self.configuration = configuration
    }

    func register(_ benchmark: Benchmark) {
        benchmarks.append(benchmark)
    }

    func register(_ benchmarks: [Benchmark]) {
        self.benchmarks.append(contentsOf: benchmarks)
    }

    func run() -> [BenchmarkStatistics] {
        results = []

        let filteredBenchmarks = benchmarks.filter {
            configuration.categories.contains($0.category)
        }

        printHeader()

        var currentCategory: BenchmarkCategory?

        for benchmark in filteredBenchmarks {
            if benchmark.category != currentCategory {
                currentCategory = benchmark.category
                printCategoryHeader(benchmark.category)
            }

            let stats = runBenchmark(benchmark)
            results.append(stats)
            printResult(stats)
        }

        printFooter()

        return results
    }

    private func runBenchmark(_ benchmark: Benchmark) -> BenchmarkStatistics {
        benchmark.setUp()
        defer { benchmark.tearDown() }

        var measurements: [UInt64] = []
        let memoryTracker = MemoryTracker()

        // Warmup
        for _ in 0..<configuration.warmupIterations {
            _ = benchmark.runIteration()
        }

        // Measurement
        memoryTracker.captureBaseline()
        let startTime = Date()
        var iteration = 0

        while iteration < configuration.measurementIterations ||
              Date().timeIntervalSince(startTime) < configuration.minimumDuration {
            let duration = benchmark.runIteration()
            measurements.append(duration)
            iteration += 1
        }

        let memoryDelta = configuration.collectMemoryMetrics ? memoryTracker.measureDelta() : nil
        let additionalMetrics = benchmark.collectAdditionalMetrics()

        return calculateStatistics(
            name: benchmark.name,
            category: benchmark.category,
            measurements: measurements,
            memoryBytes: memoryDelta,
            additionalMetrics: additionalMetrics.isEmpty ? nil : additionalMetrics
        )
    }

    private func calculateStatistics(
        name: String,
        category: BenchmarkCategory,
        measurements: [UInt64],
        memoryBytes: Int64?,
        additionalMetrics: [String: Double]?
    ) -> BenchmarkStatistics {
        let count = measurements.count
        guard count > 0 else {
            return BenchmarkStatistics(
                name: name,
                category: category.rawValue,
                iterations: 0,
                meanMs: 0,
                stddevMs: 0,
                minMs: 0,
                maxMs: 0,
                p50Ms: 0,
                p75Ms: 0,
                p95Ms: 0,
                p99Ms: 0,
                variance: 0,
                memoryBytes: memoryBytes,
                additionalMetrics: additionalMetrics
            )
        }

        // Convert to milliseconds
        let msValues = measurements.map { Double($0) / 1_000_000.0 }
        let sorted = msValues.sorted()

        let mean = msValues.reduce(0, +) / Double(count)
        let variance = msValues.map { pow($0 - mean, 2) }.reduce(0, +) / Double(count)
        let stddev = sqrt(variance)

        func percentile(_ p: Double) -> Double {
            let index = Int(Double(count - 1) * p)
            return sorted[index]
        }

        return BenchmarkStatistics(
            name: name,
            category: category.rawValue,
            iterations: count,
            meanMs: mean,
            stddevMs: stddev,
            minMs: sorted.first ?? 0,
            maxMs: sorted.last ?? 0,
            p50Ms: percentile(0.50),
            p75Ms: percentile(0.75),
            p95Ms: percentile(0.95),
            p99Ms: percentile(0.99),
            variance: variance,
            memoryBytes: memoryBytes,
            additionalMetrics: additionalMetrics
        )
    }

    // MARK: - Output Formatting

    private func printHeader() {
        guard case .console = configuration.outputFormat,
              configuration.outputFormat != .json else { return }

        print("""

        ╔═══════════════════════════════════════════════════════════════════╗
        ║           DashTerm2 Performance Benchmark Suite                   ║
        ╚═══════════════════════════════════════════════════════════════════╝

        System: \(systemInfo())
        Date: \(ISO8601DateFormatter().string(from: Date()))
        Configuration: \(configuration.measurementIterations) iterations, \(configuration.warmupIterations) warmup

        """)
    }

    private func printCategoryHeader(_ category: BenchmarkCategory) {
        guard configuration.outputFormat != .json else { return }

        print("""

        ── \(category.rawValue) ──────────────────────────────────────────────

        """)
    }

    private func printResult(_ stats: BenchmarkStatistics) {
        guard configuration.outputFormat != .json else { return }

        let reliabilityIndicator = stats.isReliable ? "✓" : "⚠"
        let name = stats.name.padding(toLength: 25, withPad: " ", startingAt: 0)
        let mean = String(format: "%8.3f", stats.meanMs)
        let stddev = String(format: "±%.3f", stats.stddevMs)
        let p95 = String(format: "%.3f", stats.p95Ms)

        print("  \(reliabilityIndicator) \(name) \(mean)ms \(stddev)ms  p95: \(p95)ms  [\(stats.iterations) iter]")
    }

    private func printFooter() {
        guard configuration.outputFormat != .json else { return }

        let reliable = results.filter { $0.isReliable }.count
        let total = results.count

        print("""

        ────────────────────────────────────────────────────────────────────
        Completed: \(total) benchmarks (\(reliable) reliable, \(total - reliable) with high variance)
        Legend: ✓ = variance < 10%, ⚠ = high variance (results may be unreliable)

        """)
    }

    private func systemInfo() -> String {
        var size = 0
        sysctlbyname("hw.model", nil, &size, nil, 0)
        var model = [CChar](repeating: 0, count: size)
        sysctlbyname("hw.model", &model, &size, nil, 0)
        let modelString = String(cString: model)

        let processInfo = ProcessInfo.processInfo
        let os = processInfo.operatingSystemVersionString
        let memory = processInfo.physicalMemory / (1024 * 1024 * 1024)

        return "\(modelString), \(os), \(memory)GB RAM"
    }

    // MARK: - JSON Output

    func generateJSONReport() -> Data? {
        let report = BenchmarkReport(
            timestamp: ISO8601DateFormatter().string(from: Date()),
            system: BenchmarkReport.SystemInfo(
                model: getSystemModel(),
                os: ProcessInfo.processInfo.operatingSystemVersionString,
                memoryGB: Int(ProcessInfo.processInfo.physicalMemory / (1024 * 1024 * 1024)),
                cpuCores: ProcessInfo.processInfo.processorCount
            ),
            configuration: BenchmarkReport.ConfigurationInfo(
                warmupIterations: configuration.warmupIterations,
                measurementIterations: configuration.measurementIterations,
                minimumDuration: configuration.minimumDuration
            ),
            results: results
        )

        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        return try? encoder.encode(report)
    }

    private func getSystemModel() -> String {
        var size = 0
        sysctlbyname("hw.model", nil, &size, nil, 0)
        var model = [CChar](repeating: 0, count: size)
        sysctlbyname("hw.model", &model, &size, nil, 0)
        return String(cString: model)
    }
}

/// Complete benchmark report structure for JSON output
struct BenchmarkReport: Codable {
    let timestamp: String
    let system: SystemInfo
    let configuration: ConfigurationInfo
    let results: [BenchmarkStatistics]

    struct SystemInfo: Codable {
        let model: String
        let os: String
        let memoryGB: Int
        let cpuCores: Int
    }

    struct ConfigurationInfo: Codable {
        let warmupIterations: Int
        let measurementIterations: Int
        let minimumDuration: TimeInterval
    }
}

// MARK: - Baseline Comparison

final class BaselineComparator {
    struct Comparison {
        let benchmark: String
        let baselineMs: Double
        let currentMs: Double
        let deltaPercent: Double
        let isRegression: Bool
    }

    private let threshold: Double

    init(threshold: Double = 10.0) {
        self.threshold = threshold
    }

    func compare(baseline: [BenchmarkStatistics], current: [BenchmarkStatistics]) -> [Comparison] {
        var comparisons: [Comparison] = []

        let baselineDict = Dictionary(uniqueKeysWithValues: baseline.map { ($0.name, $0) })

        for currentStat in current {
            guard let baselineStat = baselineDict[currentStat.name] else { continue }

            let deltaPercent = ((currentStat.meanMs - baselineStat.meanMs) / baselineStat.meanMs) * 100
            let isRegression = deltaPercent > threshold

            comparisons.append(Comparison(
                benchmark: currentStat.name,
                baselineMs: baselineStat.meanMs,
                currentMs: currentStat.meanMs,
                deltaPercent: deltaPercent,
                isRegression: isRegression
            ))
        }

        return comparisons
    }

    func printComparison(_ comparisons: [Comparison]) {
        print("""

        ── Baseline Comparison ──────────────────────────────────────────────

        """)

        for comp in comparisons {
            let indicator: String
            if comp.deltaPercent < -5 {
                indicator = "🚀" // Significant improvement
            } else if comp.deltaPercent < 5 {
                indicator = "✓"  // Within tolerance
            } else if comp.isRegression {
                indicator = "🔴" // Regression
            } else {
                indicator = "⚠️"  // Warning
            }

            let name = comp.benchmark.padding(toLength: 25, withPad: " ", startingAt: 0)
            let baseline = String(format: "%8.3f", comp.baselineMs)
            let current = String(format: "%8.3f", comp.currentMs)
            let delta = String(format: "%+.1f%%", comp.deltaPercent)

            print("  \(indicator) \(name) \(baseline)ms → \(current)ms (\(delta))")
        }

        let regressions = comparisons.filter { $0.isRegression }
        if !regressions.isEmpty {
            print("""

            ⚠️  WARNING: \(regressions.count) regression(s) detected (>\(Int(threshold))% slower)

            """)
        }
    }
}
