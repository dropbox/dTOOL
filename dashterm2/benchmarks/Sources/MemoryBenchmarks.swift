//
//  MemoryBenchmarks.swift
//  DashTerm2 Benchmarks
//
//  Memory allocation and footprint benchmarks.
//

import Foundation

// MARK: - Memory Benchmark Utilities

/// Tracks allocations using malloc hooks (approximate)
final class AllocationTracker {
    private var allocationCount: Int = 0
    private var deallocationCount: Int = 0
    private var totalBytesAllocated: Int64 = 0

    private var isTracking = false

    func startTracking() {
        allocationCount = 0
        deallocationCount = 0
        totalBytesAllocated = 0
        isTracking = true
    }

    func stopTracking() -> (allocations: Int, deallocations: Int, bytesAllocated: Int64) {
        isTracking = false
        return (allocationCount, deallocationCount, totalBytesAllocated)
    }

    /// Manually track an allocation (called from test code)
    func trackAllocation(bytes: Int) {
        guard isTracking else { return }
        allocationCount += 1
        totalBytesAllocated += Int64(bytes)
    }

    func trackDeallocation() {
        guard isTracking else { return }
        deallocationCount += 1
    }
}

// MARK: - Memory Benchmarks

/// Benchmark: Object allocation patterns
final class AllocationPatternBenchmark: Benchmark {
    let name = "AllocationPattern"
    let category: BenchmarkCategory = .memory
    let description = "Measure allocation patterns during typical operations"

    private var allocations: [[Any]] = []

    func setUp() {
        allocations = []
    }

    func runIteration() -> UInt64 {
        let baseline = MemoryTracker.Snapshot.current()

        let duration = PrecisionTimer.measure {
            // Simulate typical terminal operations
            for _ in 0..<100 {
                // Line buffer allocations
                var lineBuffer: [UInt8] = []
                lineBuffer.reserveCapacity(256)
                for i in 0..<80 {
                    lineBuffer.append(UInt8(65 + (i % 26)))
                }

                // String allocations
                let str = String(decoding: lineBuffer, as: UTF8.self)
                allocations.append([lineBuffer, str])

                // Array reallocations
                var growingArray: [Int] = []
                for i in 0..<100 {
                    growingArray.append(i)
                }
                allocations.append([growingArray])
            }
        }

        let after = MemoryTracker.Snapshot.current()
        let deltaBytes = after.residentSize - baseline.residentSize

        // Clear allocations after each iteration to allow GC
        allocations.removeAll(keepingCapacity: true)

        blackHole(deltaBytes)
        return duration
    }

    func tearDown() {
        allocations = []
    }
}

/// Benchmark: Memory footprint for scrollback buffer sizes
final class ScrollbackFootprintBenchmark: Benchmark {
    let name = "ScrollbackFootprint"
    let category: BenchmarkCategory = .memory
    let description = "Measure memory for various scrollback sizes"

    private var buffer: SimulatedScreenBuffer?
    private var additionalMetricsStore: [String: Double] = [:]

    func setUp() {}

    func runIteration() -> UInt64 {
        var footprints: [Int: Int64] = [:]

        let duration = PrecisionTimer.measure {
            for scrollbackSize in [1000, 5000, 10000, 50000] {
                autoreleasepool {
                    let baseline = MemoryTracker.Snapshot.current()

                    // Create buffer with scrollback
                    let buf = SimulatedScreenBuffer(width: 132, height: 50, maxHistorySize: scrollbackSize)

                    // Fill scrollback
                    for _ in 0..<scrollbackSize {
                        buf.scrollUp(lines: 1)
                    }

                    let after = MemoryTracker.Snapshot.current()
                    footprints[scrollbackSize] = after.residentSize - baseline.residentSize

                    self.buffer = buf
                }
            }
        }

        // Store footprints as additional metrics
        for (size, bytes) in footprints {
            additionalMetricsStore["scrollback_\(size)_kb"] = Double(bytes) / 1024.0
        }

        buffer = nil
        return duration
    }

    func collectAdditionalMetrics() -> [String: Double] {
        additionalMetricsStore
    }

    func tearDown() {
        buffer = nil
        additionalMetricsStore = [:]
    }
}

/// Benchmark: Peak memory during operations
final class PeakMemoryBenchmark: Benchmark {
    let name = "PeakMemory"
    let category: BenchmarkCategory = .memory
    let description = "Track peak memory during intensive operations"

    private var tempStorage: [[UInt8]] = []
    private var peakMemory: Int64 = 0

    func setUp() {
        tempStorage = []
        peakMemory = 0
    }

    func runIteration() -> UInt64 {
        let baseline = MemoryTracker.Snapshot.current()
        var currentPeak: Int64 = 0

        let duration = PrecisionTimer.measure {
            // Simulate memory-intensive operations
            for wave in 0..<5 {
                // Allocation wave
                for _ in 0..<(100 * (wave + 1)) {
                    var buffer: [UInt8] = []
                    buffer.reserveCapacity(1024)
                    for i in 0..<1024 {
                        buffer.append(UInt8(i % 256))
                    }
                    tempStorage.append(buffer)
                }

                // Check peak
                let current = MemoryTracker.Snapshot.current()
                let delta = current.residentSize - baseline.residentSize
                if delta > currentPeak {
                    currentPeak = delta
                }

                // Partial deallocation
                if tempStorage.count > 50 {
                    tempStorage.removeFirst(tempStorage.count / 2)
                }
            }
        }

        peakMemory = currentPeak
        tempStorage.removeAll()

        return duration
    }

    func collectAdditionalMetrics() -> [String: Double] {
        ["peak_memory_kb": Double(peakMemory) / 1024.0]
    }

    func tearDown() {
        tempStorage = []
    }
}

/// Benchmark: String allocation overhead
final class StringAllocationBenchmark: Benchmark {
    let name = "StringAllocation"
    let category: BenchmarkCategory = .memory
    let description = "Measure string allocation and interning overhead"

    private var strings: [String] = []
    private let iterations = 1000

    func setUp() {
        strings = []
        strings.reserveCapacity(iterations)
    }

    func runIteration() -> UInt64 {
        PrecisionTimer.measure {
            for i in 0..<iterations {
                // Create various string types
                let ascii = String(repeating: "A", count: 80)
                let unicode = String(repeating: "日", count: 40)
                let mixed = "Hello 世界 🌍 Test \(i)"

                strings.append(ascii)
                strings.append(unicode)
                strings.append(mixed)
            }
        }
    }

    func tearDown() {
        strings = []
    }
}

/// Benchmark: Array growth patterns
final class ArrayGrowthBenchmark: Benchmark {
    let name = "ArrayGrowth"
    let category: BenchmarkCategory = .memory
    let description = "Measure array reallocation during growth"

    private var arrays: [[Int]] = []
    private let targetSize = 10000

    func setUp() {
        arrays = []
    }

    func runIteration() -> UInt64 {
        PrecisionTimer.measure {
            // Test 1: No capacity hint
            var array1: [Int] = []
            for i in 0..<targetSize {
                array1.append(i)
            }
            arrays.append(array1)

            // Test 2: With capacity hint
            var array2: [Int] = []
            array2.reserveCapacity(targetSize)
            for i in 0..<targetSize {
                array2.append(i)
            }
            arrays.append(array2)

            // Test 3: Batch append
            var array3: [Int] = []
            let batch = Array(0..<1000)
            for _ in 0..<(targetSize / 1000) {
                array3.append(contentsOf: batch)
            }
            arrays.append(array3)
        }
    }

    func tearDown() {
        arrays = []
    }
}

// MARK: - Registration

/// All memory benchmarks
func createMemoryBenchmarks() -> [Benchmark] {
    [
        AllocationPatternBenchmark(),
        ScrollbackFootprintBenchmark(),
        PeakMemoryBenchmark(),
        StringAllocationBenchmark(),
        ArrayGrowthBenchmark(),
    ]
}
