//
//  main.swift
//  DashTerm2 Benchmarks
//
//  Command-line benchmark runner entry point.
//

import Foundation

// MARK: - Command Line Parsing

struct CommandLineOptions {
    var categories: Set<BenchmarkCategory> = Set(BenchmarkCategory.allCases)
    var outputFormat: BenchmarkConfiguration.OutputFormat = .console
    var jsonOutputPath: String?
    var baselinePath: String?
    var saveBaseline: Bool = false
    var baselineOutputPath: String?
    var regressionThreshold: Double = 10.0
    var quick: Bool = false
    var listOnly: Bool = false
    var help: Bool = false

    static func parse(_ args: [String]) -> CommandLineOptions {
        var options = CommandLineOptions()
        var i = 1 // Skip program name

        while i < args.count {
            let arg = args[i]

            switch arg {
            case "-h", "--help":
                options.help = true

            case "-c", "--category":
                i += 1
                if i < args.count {
                    let categoryName = args[i]
                    if let category = BenchmarkCategory.allCases.first(where: {
                        $0.rawValue.lowercased() == categoryName.lowercased() ||
                        $0.rawValue.lowercased().replacingOccurrences(of: " ", with: "-") == categoryName.lowercased()
                    }) {
                        if options.categories.count == BenchmarkCategory.allCases.count {
                            options.categories = [category]
                        } else {
                            options.categories.insert(category)
                        }
                    } else {
                        fputs("Warning: Unknown category '\(categoryName)'\n", stderr)
                    }
                }

            case "--json":
                i += 1
                if i < args.count {
                    options.outputFormat = .json
                    options.jsonOutputPath = args[i]
                }

            case "--compare":
                i += 1
                if i < args.count {
                    options.baselinePath = args[i]
                }

            case "--save-baseline":
                options.saveBaseline = true
                i += 1
                if i < args.count && !args[i].hasPrefix("-") {
                    options.baselineOutputPath = args[i]
                } else {
                    i -= 1 // No path given, use default
                }

            case "--threshold":
                i += 1
                if i < args.count, let threshold = Double(args[i]) {
                    options.regressionThreshold = threshold
                }

            case "--quick":
                options.quick = true

            case "--list":
                options.listOnly = true

            case "--no-metal":
                options.categories.remove(.metalRenderer)

            default:
                if !arg.hasPrefix("-") {
                    // Treat as category name
                    if let category = BenchmarkCategory.allCases.first(where: {
                        $0.rawValue.lowercased() == arg.lowercased() ||
                        $0.rawValue.lowercased().replacingOccurrences(of: " ", with: "-") == arg.lowercased()
                    }) {
                        if options.categories.count == BenchmarkCategory.allCases.count {
                            options.categories = [category]
                        } else {
                            options.categories.insert(category)
                        }
                    }
                }
            }

            i += 1
        }

        return options
    }
}

func printUsage() {
    print("""
    DashTerm2 Performance Benchmark Suite

    USAGE:
        DashTermBenchmarks [OPTIONS] [CATEGORIES...]

    OPTIONS:
        -h, --help              Show this help message
        -c, --category NAME     Run only specified category (can be repeated)
        --json PATH             Output results as JSON to PATH
        --compare PATH          Compare results against baseline at PATH
        --save-baseline [PATH]  Save results as new baseline
        --threshold PERCENT     Regression threshold (default: 10.0)
        --quick                 Quick run with fewer iterations
        --list                  List available benchmarks without running
        --no-metal              Skip Metal renderer benchmarks

    CATEGORIES:
        text-rendering          Text processing benchmarks
        screen-buffer           Screen buffer operation benchmarks
        memory                  Memory allocation benchmarks
        metal-renderer          Metal rendering benchmarks (simulated)

    EXAMPLES:
        # Run all benchmarks
        DashTermBenchmarks

        # Run only text benchmarks
        DashTermBenchmarks --category text-rendering

        # Quick run with JSON output
        DashTermBenchmarks --quick --json results.json

        # Compare against baseline
        DashTermBenchmarks --compare baselines/baseline.json

        # Save new baseline
        DashTermBenchmarks --save-baseline baselines/baseline_$(date +%Y%m%d).json

    """)
}

func listBenchmarks(_ benchmarks: [Benchmark]) {
    print("\nAvailable Benchmarks:")
    print("=====================\n")

    var currentCategory: BenchmarkCategory?

    for benchmark in benchmarks.sorted(by: { $0.category.rawValue < $1.category.rawValue }) {
        if benchmark.category != currentCategory {
            currentCategory = benchmark.category
            print("\n[\(benchmark.category.rawValue)]")
        }
        print("  • \(benchmark.name): \(benchmark.description)")
    }

    print("\nTotal: \(benchmarks.count) benchmarks\n")
}

// MARK: - Main

func main() -> Int32 {
    let options = CommandLineOptions.parse(CommandLine.arguments)

    if options.help {
        printUsage()
        return 0
    }

    // Create all benchmarks
    var allBenchmarks: [Benchmark] = []
    allBenchmarks.append(contentsOf: createTextBenchmarks())
    allBenchmarks.append(contentsOf: createScreenBufferBenchmarks())
    allBenchmarks.append(contentsOf: createMemoryBenchmarks())
    allBenchmarks.append(contentsOf: createMetalBenchmarks())

    if options.listOnly {
        listBenchmarks(allBenchmarks)
        return 0
    }

    // Configure runner
    let config: BenchmarkConfiguration
    if options.quick {
        config = BenchmarkConfiguration(
            warmupIterations: 2,
            measurementIterations: 10,
            minimumDuration: 0.5,
            collectMemoryMetrics: true,
            categories: options.categories,
            outputFormat: options.outputFormat,
            baselinePath: options.baselinePath,
            regressionThreshold: options.regressionThreshold
        )
    } else {
        config = BenchmarkConfiguration(
            warmupIterations: 5,
            measurementIterations: 100,
            minimumDuration: 1.0,
            collectMemoryMetrics: true,
            categories: options.categories,
            outputFormat: options.outputFormat,
            baselinePath: options.baselinePath,
            regressionThreshold: options.regressionThreshold
        )
    }

    let runner = BenchmarkRunner(configuration: config)
    runner.register(allBenchmarks)

    // Run benchmarks
    let results = runner.run()

    // Output JSON if requested
    if let jsonPath = options.jsonOutputPath {
        if let jsonData = runner.generateJSONReport() {
            do {
                try jsonData.write(to: URL(fileURLWithPath: jsonPath))
                print("\nJSON report saved to: \(jsonPath)")
            } catch {
                fputs("Error writing JSON: \(error)\n", stderr)
            }
        }
    }

    // Save baseline if requested
    if options.saveBaseline {
        let baselinePath = options.baselineOutputPath ??
            "Benchmarks/baselines/baseline_\(formattedDate()).json"

        if let jsonData = runner.generateJSONReport() {
            do {
                let url = URL(fileURLWithPath: baselinePath)
                try FileManager.default.createDirectory(
                    at: url.deletingLastPathComponent(),
                    withIntermediateDirectories: true
                )
                try jsonData.write(to: url)
                print("\nBaseline saved to: \(baselinePath)")
            } catch {
                fputs("Error saving baseline: \(error)\n", stderr)
            }
        }
    }

    // Compare against baseline if provided
    if let baselinePath = options.baselinePath {
        do {
            let baselineData = try Data(contentsOf: URL(fileURLWithPath: baselinePath))
            let decoder = JSONDecoder()
            let baseline = try decoder.decode(BenchmarkReport.self, from: baselineData)

            let comparator = BaselineComparator(threshold: options.regressionThreshold)
            let comparisons = comparator.compare(baseline: baseline.results, current: results)
            comparator.printComparison(comparisons)

            // Return non-zero exit code if regressions detected
            let regressions = comparisons.filter { $0.isRegression }
            if !regressions.isEmpty {
                return 2 // Exit code 2 = regressions detected
            }
        } catch {
            fputs("Error loading baseline: \(error)\n", stderr)
            return 1
        }
    }

    return 0
}

func formattedDate() -> String {
    let formatter = DateFormatter()
    formatter.dateFormat = "yyyyMMdd_HHmmss"
    return formatter.string(from: Date())
}

// Run main
exit(main())
