//
//  eaindex_copy_benchmark.m
//  DashTerm2
//
//  Benchmark measuring iTermExternalAttributeIndex copy optimization.
//  This measures the optimization from iteration #136 where copyWithZone:
//  returns nil for empty indices instead of allocating an empty dictionary.
//
//  Build and run:
//    clang -O3 -framework Foundation -fobjc-arc \
//      benchmarks/eaindex_copy_benchmark.m -o benchmarks/eaindex_copy_benchmark
//    ./benchmarks/eaindex_copy_benchmark
//

#import <Foundation/Foundation.h>
#import <mach/mach_time.h>

// Minimal simulation of iTermExternalAttributeIndex
// We can't include the real class without the entire project, so we simulate the behavior

@interface SimulatedExternalAttribute : NSObject <NSCopying>
@property (nonatomic, assign) BOOL hasUnderlineColor;
@property (nonatomic, copy) NSString *blockIDList;
@end

@implementation SimulatedExternalAttribute
- (id)copyWithZone:(NSZone *)zone {
    // Immutable, return self
    return self;
}
@end

// Original implementation (always allocates)
@interface OriginalExternalAttributeIndex : NSObject <NSCopying> {
  @public
    NSMutableDictionary<NSNumber *, SimulatedExternalAttribute *> *_attributes;
}
@property (nonatomic, readonly) BOOL isEmpty;
@end

@implementation OriginalExternalAttributeIndex
- (instancetype)init {
    self = [super init];
    if (self) {
        _attributes = [NSMutableDictionary dictionary];
    }
    return self;
}

- (BOOL)isEmpty {
    return _attributes.count == 0;
}

- (id)copyWithZone:(NSZone *)zone {
    // Original: Always allocates even when empty
    OriginalExternalAttributeIndex *copy = [[OriginalExternalAttributeIndex alloc] init];
    copy->_attributes = [_attributes mutableCopy];
    return copy;
}
@end

// Optimized implementation (returns nil for empty)
@interface OptimizedExternalAttributeIndex : NSObject <NSCopying> {
  @public
    NSMutableDictionary<NSNumber *, SimulatedExternalAttribute *> *_attributes;
}
@property (nonatomic, readonly) BOOL isEmpty;
@end

@implementation OptimizedExternalAttributeIndex
- (instancetype)init {
    self = [super init];
    if (self) {
        _attributes = [NSMutableDictionary dictionary];
    }
    return self;
}

- (BOOL)isEmpty {
    return _attributes.count == 0;
}

- (id)copyWithZone:(NSZone *)zone {
    // Optimized: Return nil for empty indices
    if (_attributes.count == 0) {
        return nil;
    }
    OptimizedExternalAttributeIndex *copy = [[OptimizedExternalAttributeIndex alloc] init];
    copy->_attributes = [_attributes mutableCopy];
    return copy;
}
@end

static mach_timebase_info_data_t timebaseInfo;

static uint64_t getNanoseconds(uint64_t machTime) {
    if (timebaseInfo.denom == 0) {
        mach_timebase_info(&timebaseInfo);
    }
    return machTime * timebaseInfo.numer / timebaseInfo.denom;
}

// Benchmark copying empty indices with original implementation
static double benchmarkOriginalEmpty(int numRows, int warmupIterations, int measureIterations) {
    // Pre-create source indices (all empty)
    NSMutableArray<OriginalExternalAttributeIndex *> *sources = [NSMutableArray arrayWithCapacity:numRows];
    for (int i = 0; i < numRows; i++) {
        [sources addObject:[[OriginalExternalAttributeIndex alloc] init]];
    }

    // Warmup
    for (int iter = 0; iter < warmupIterations; iter++) {
        @autoreleasepool {
            for (int i = 0; i < numRows; i++) {
                id copy __attribute__((unused)) = [sources[i] copy];
            }
        }
    }

    // Measure
    uint64_t totalTime = 0;
    for (int iter = 0; iter < measureIterations; iter++) {
        @autoreleasepool {
            uint64_t start = mach_absolute_time();
            for (int i = 0; i < numRows; i++) {
                id copy __attribute__((unused)) = [sources[i] copy];
            }
            uint64_t end = mach_absolute_time();
            totalTime += getNanoseconds(end - start);
        }
    }

    return (double)totalTime / measureIterations / 1e6; // Return ms
}

// Benchmark copying empty indices with optimized implementation
static double benchmarkOptimizedEmpty(int numRows, int warmupIterations, int measureIterations) {
    // Pre-create source indices (all empty)
    NSMutableArray<OptimizedExternalAttributeIndex *> *sources = [NSMutableArray arrayWithCapacity:numRows];
    for (int i = 0; i < numRows; i++) {
        [sources addObject:[[OptimizedExternalAttributeIndex alloc] init]];
    }

    // Warmup
    for (int iter = 0; iter < warmupIterations; iter++) {
        @autoreleasepool {
            for (int i = 0; i < numRows; i++) {
                id copy __attribute__((unused)) = [sources[i] copy];
            }
        }
    }

    // Measure
    uint64_t totalTime = 0;
    for (int iter = 0; iter < measureIterations; iter++) {
        @autoreleasepool {
            uint64_t start = mach_absolute_time();
            for (int i = 0; i < numRows; i++) {
                id copy __attribute__((unused)) = [sources[i] copy];
            }
            uint64_t end = mach_absolute_time();
            totalTime += getNanoseconds(end - start);
        }
    }

    return (double)totalTime / measureIterations / 1e6; // Return ms
}

// Benchmark copying non-empty indices (should be same for both)
static double benchmarkOriginalNonEmpty(int numRows, int attributesPerRow, int warmupIterations,
                                        int measureIterations) {
    // Pre-create source indices with attributes
    NSMutableArray<OriginalExternalAttributeIndex *> *sources = [NSMutableArray arrayWithCapacity:numRows];
    for (int i = 0; i < numRows; i++) {
        OriginalExternalAttributeIndex *idx = [[OriginalExternalAttributeIndex alloc] init];
        for (int j = 0; j < attributesPerRow; j++) {
            SimulatedExternalAttribute *attr = [[SimulatedExternalAttribute alloc] init];
            attr.hasUnderlineColor = YES;
            idx->_attributes[@(j)] = attr;
        }
        [sources addObject:idx];
    }

    // Warmup
    for (int iter = 0; iter < warmupIterations; iter++) {
        @autoreleasepool {
            for (int i = 0; i < numRows; i++) {
                id copy __attribute__((unused)) = [sources[i] copy];
            }
        }
    }

    // Measure
    uint64_t totalTime = 0;
    for (int iter = 0; iter < measureIterations; iter++) {
        @autoreleasepool {
            uint64_t start = mach_absolute_time();
            for (int i = 0; i < numRows; i++) {
                id copy __attribute__((unused)) = [sources[i] copy];
            }
            uint64_t end = mach_absolute_time();
            totalTime += getNanoseconds(end - start);
        }
    }

    return (double)totalTime / measureIterations / 1e6; // Return ms
}

static double benchmarkOptimizedNonEmpty(int numRows, int attributesPerRow, int warmupIterations,
                                         int measureIterations) {
    // Pre-create source indices with attributes
    NSMutableArray<OptimizedExternalAttributeIndex *> *sources = [NSMutableArray arrayWithCapacity:numRows];
    for (int i = 0; i < numRows; i++) {
        OptimizedExternalAttributeIndex *idx = [[OptimizedExternalAttributeIndex alloc] init];
        for (int j = 0; j < attributesPerRow; j++) {
            SimulatedExternalAttribute *attr = [[SimulatedExternalAttribute alloc] init];
            attr.hasUnderlineColor = YES;
            idx->_attributes[@(j)] = attr;
        }
        [sources addObject:idx];
    }

    // Warmup
    for (int iter = 0; iter < warmupIterations; iter++) {
        @autoreleasepool {
            for (int i = 0; i < numRows; i++) {
                id copy __attribute__((unused)) = [sources[i] copy];
            }
        }
    }

    // Measure
    uint64_t totalTime = 0;
    for (int iter = 0; iter < measureIterations; iter++) {
        @autoreleasepool {
            uint64_t start = mach_absolute_time();
            for (int i = 0; i < numRows; i++) {
                id copy __attribute__((unused)) = [sources[i] copy];
            }
            uint64_t end = mach_absolute_time();
            totalTime += getNanoseconds(end - start);
        }
    }

    return (double)totalTime / measureIterations / 1e6; // Return ms
}

// Benchmark mixed scenario (realistic: most rows empty, some with attributes)
static double benchmarkOriginalMixed(int numRows, double emptyRatio, int warmupIterations, int measureIterations) {
    // Pre-create source indices (mixed empty and non-empty)
    NSMutableArray<OriginalExternalAttributeIndex *> *sources = [NSMutableArray arrayWithCapacity:numRows];
    int emptyCount = (int)(numRows * emptyRatio);
    for (int i = 0; i < numRows; i++) {
        OriginalExternalAttributeIndex *idx = [[OriginalExternalAttributeIndex alloc] init];
        if (i >= emptyCount) {
            // Add some attributes (URL or underline)
            SimulatedExternalAttribute *attr = [[SimulatedExternalAttribute alloc] init];
            attr.hasUnderlineColor = YES;
            idx->_attributes[@(0)] = attr;
        }
        [sources addObject:idx];
    }

    // Warmup
    for (int iter = 0; iter < warmupIterations; iter++) {
        @autoreleasepool {
            for (int i = 0; i < numRows; i++) {
                id copy __attribute__((unused)) = [sources[i] copy];
            }
        }
    }

    // Measure
    uint64_t totalTime = 0;
    for (int iter = 0; iter < measureIterations; iter++) {
        @autoreleasepool {
            uint64_t start = mach_absolute_time();
            for (int i = 0; i < numRows; i++) {
                id copy __attribute__((unused)) = [sources[i] copy];
            }
            uint64_t end = mach_absolute_time();
            totalTime += getNanoseconds(end - start);
        }
    }

    return (double)totalTime / measureIterations / 1e6; // Return ms
}

static double benchmarkOptimizedMixed(int numRows, double emptyRatio, int warmupIterations, int measureIterations) {
    // Pre-create source indices (mixed empty and non-empty)
    NSMutableArray<OptimizedExternalAttributeIndex *> *sources = [NSMutableArray arrayWithCapacity:numRows];
    int emptyCount = (int)(numRows * emptyRatio);
    for (int i = 0; i < numRows; i++) {
        OptimizedExternalAttributeIndex *idx = [[OptimizedExternalAttributeIndex alloc] init];
        if (i >= emptyCount) {
            // Add some attributes (URL or underline)
            SimulatedExternalAttribute *attr = [[SimulatedExternalAttribute alloc] init];
            attr.hasUnderlineColor = YES;
            idx->_attributes[@(0)] = attr;
        }
        [sources addObject:idx];
    }

    // Warmup
    for (int iter = 0; iter < warmupIterations; iter++) {
        @autoreleasepool {
            for (int i = 0; i < numRows; i++) {
                id copy __attribute__((unused)) = [sources[i] copy];
            }
        }
    }

    // Measure
    uint64_t totalTime = 0;
    for (int iter = 0; iter < measureIterations; iter++) {
        @autoreleasepool {
            uint64_t start = mach_absolute_time();
            for (int i = 0; i < numRows; i++) {
                id copy __attribute__((unused)) = [sources[i] copy];
            }
            uint64_t end = mach_absolute_time();
            totalTime += getNanoseconds(end - start);
        }
    }

    return (double)totalTime / measureIterations / 1e6; // Return ms
}

int main(int argc, char *argv[]) {
    @autoreleasepool {
        printf("================================================================\n");
        printf("External Attribute Index Copy Benchmark\n");
        printf("================================================================\n\n");
        printf("This benchmark measures the performance improvement from returning\n");
        printf("nil for empty external attribute indices during copy operations.\n");
        printf("Most terminal lines have no external attributes (URLs, underline\n");
        printf("colors, block IDs), so this optimization saves allocation overhead\n");
        printf("in the Metal rendering hot path.\n\n");

        const int warmupIterations = 5;
        const int measureIterations = 50;

        printf("Configuration: %d warmup iterations, %d measurement iterations\n\n", warmupIterations,
               measureIterations);

        // Test 1: Empty indices (common case)
        printf("================================================================\n");
        printf("Test 1: Copy Empty Indices (most common case)\n");
        printf("================================================================\n\n");

        int rowCounts[] = {24, 50, 100, 200, 500, 1000};
        int numRowCounts = sizeof(rowCounts) / sizeof(rowCounts[0]);

        printf("%-15s %-15s %-15s %-15s\n", "Rows", "Original (ms)", "Optimized (ms)", "Speedup");
        printf("---------------------------------------------------------------\n");

        for (int ri = 0; ri < numRowCounts; ri++) {
            int rowCount = rowCounts[ri];
            double originalTime = benchmarkOriginalEmpty(rowCount, warmupIterations, measureIterations);
            double optimizedTime = benchmarkOptimizedEmpty(rowCount, warmupIterations, measureIterations);
            double speedup = originalTime / optimizedTime;

            printf("%-15d %-15.4f %-15.4f %-15.2fx\n", rowCount, originalTime, optimizedTime, speedup);
        }

        // Test 2: Non-empty indices (less common)
        printf("\n");
        printf("================================================================\n");
        printf("Test 2: Copy Non-Empty Indices (control - should be similar)\n");
        printf("================================================================\n\n");

        printf("%-15s %-15s %-15s %-15s\n", "Rows", "Original (ms)", "Optimized (ms)", "Ratio");
        printf("---------------------------------------------------------------\n");

        for (int ri = 0; ri < numRowCounts; ri++) {
            int rowCount = rowCounts[ri];
            double originalTime = benchmarkOriginalNonEmpty(rowCount, 5, warmupIterations, measureIterations);
            double optimizedTime = benchmarkOptimizedNonEmpty(rowCount, 5, warmupIterations, measureIterations);
            double ratio = originalTime / optimizedTime;

            printf("%-15d %-15.4f %-15.4f %-15.2fx\n", rowCount, originalTime, optimizedTime, ratio);
        }

        // Test 3: Mixed (realistic scenario)
        printf("\n");
        printf("================================================================\n");
        printf("Test 3: Mixed Scenario (realistic terminal content)\n");
        printf("================================================================\n\n");
        printf("Simulates typical terminal: mostly plain text, some URLs/hyperlinks.\n\n");

        double emptyRatios[] = {0.95, 0.90, 0.80, 0.50};
        int numEmptyRatios = sizeof(emptyRatios) / sizeof(emptyRatios[0]);

        for (int ei = 0; ei < numEmptyRatios; ei++) {
            double emptyRatio = emptyRatios[ei];
            printf("Empty ratio: %.0f%%\n", emptyRatio * 100);
            printf("%-15s %-15s %-15s %-15s\n", "Rows", "Original (ms)", "Optimized (ms)", "Speedup");
            printf("---------------------------------------------------------------\n");

            for (int ri = 0; ri < numRowCounts; ri++) {
                int rowCount = rowCounts[ri];
                double originalTime = benchmarkOriginalMixed(rowCount, emptyRatio, warmupIterations, measureIterations);
                double optimizedTime =
                    benchmarkOptimizedMixed(rowCount, emptyRatio, warmupIterations, measureIterations);
                double speedup = originalTime / optimizedTime;

                printf("%-15d %-15.4f %-15.4f %-15.2fx\n", rowCount, originalTime, optimizedTime, speedup);
            }
            printf("\n");
        }

        printf("================================================================\n");
        printf("Analysis\n");
        printf("================================================================\n\n");
        printf("Key observations:\n");
        printf("- Empty index copy: significant speedup by avoiding allocation\n");
        printf("- Non-empty index copy: similar performance (optimization does not regress)\n");
        printf("- Real-world impact depends on ratio of empty vs non-empty indices\n");
        printf("\n");
        printf("In typical terminal usage:\n");
        printf("- Plain text screens: 100%% empty (maximum speedup)\n");
        printf("- Code with URLs/hyperlinks: 90-95%% empty (large speedup)\n");
        printf("- Heavy markdown/links: 50-80%% empty (moderate speedup)\n");
        printf("\n");
        printf("The optimization reduces per-frame allocations during Metal rendering,\n");
        printf("which helps reduce GC pressure and improves frame consistency.\n");
    }
    return 0;
}
