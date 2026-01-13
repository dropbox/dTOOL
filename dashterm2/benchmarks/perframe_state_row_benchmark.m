//
//  perframe_state_row_benchmark.m
//  DashTerm2
//
//  Benchmark to measure the performance improvement from pooling
//  iTermMetalPerFrameStateRow objects.
//
//  This benchmark simulates the per-frame allocation pattern in
//  metalDriverWillBeginDrawingFrame by creating and destroying
//  row objects at 60fps for various terminal sizes.
//
//  To compile and run:
//  clang -framework Foundation -framework CoreFoundation \
//        -O2 -o perframe_state_row_benchmark perframe_state_row_benchmark.m && \
//        ./perframe_state_row_benchmark
//

#import <Foundation/Foundation.h>
#import <CoreFoundation/CoreFoundation.h>
#import <mach/mach_time.h>

// Configuration
typedef struct {
    const char *name;
    int rows;
    int columns;
} BenchmarkConfig;

static BenchmarkConfig configs[] = {
    {"standard (24x80)", 24, 80},
    {"medium (50x120)", 50, 120},
    {"large (100x200)", 100, 200},
};

// Simulate iTermMetalPerFrameStateRow object structure
@interface MockPerFrameStateRow : NSObject {
  @public
    int _markStyle;
    BOOL _hoverState;
    BOOL _lineStyleMark;
    int _lineStyleMarkRightInset;
    id _screenCharLine;
    id _selectedIndexSet;
    id _date;
    BOOL _belongsToBlock;
    id _matches;
    NSRange _underlinedRange;
    BOOL _x_inDeselectedRegion;
    id _eaIndex;
}
@end

@implementation MockPerFrameStateRow
- (instancetype)init {
    self = [super init];
    if (self) {
        _markStyle = 0;
        _hoverState = NO;
        _lineStyleMark = NO;
        _lineStyleMarkRightInset = 0;
        _belongsToBlock = NO;
        _x_inDeselectedRegion = NO;
        _underlinedRange = NSMakeRange(0, 0);
    }
    return self;
}

- (void)repopulateForRow:(int)row {
    // Simulate repopulation work
    _markStyle = row % 3;
    _hoverState = (row % 10) == 0;
    _lineStyleMark = (row % 5) == 0;
    _lineStyleMarkRightInset = row % 4;
    _belongsToBlock = (row % 7) == 0;
    _x_inDeselectedRegion = (row % 11) == 0;
    _underlinedRange = NSMakeRange(row, 10);
}
@end

// Pool implementation (mirrors iTermMetalPerFrameStateRowPool)
@interface MockRowPool : NSObject {
    CFMutableArrayRef _pool;
    NSUInteger _poolCount;
}
+ (instancetype)sharedPool;
- (MockPerFrameStateRow *)acquireRow;
- (void)returnRow:(MockPerFrameStateRow *)row;
- (void)returnRows:(NSArray *)rows;
- (void)drain;
@end

@implementation MockRowPool

static const NSUInteger kMaxPoolSize = 128;

+ (instancetype)sharedPool {
    static MockRowPool *instance;
    static dispatch_once_t onceToken;
    dispatch_once(&onceToken, ^{
        instance = [[MockRowPool alloc] init];
    });
    return instance;
}

- (instancetype)init {
    self = [super init];
    if (self) {
        _pool = CFArrayCreateMutable(kCFAllocatorDefault, 32, &kCFTypeArrayCallBacks);
        _poolCount = 0;
    }
    return self;
}

- (void)dealloc {
    if (_pool)
        CFRelease(_pool);
}

- (MockPerFrameStateRow *)acquireRow {
    if (_poolCount > 0) {
        _poolCount--;
        // Get the row without transfer first, then remove from array
        MockPerFrameStateRow *row = (__bridge MockPerFrameStateRow *)CFArrayGetValueAtIndex(_pool, _poolCount);
        // The CFArray will release its reference when we remove it
        // We need to retain it first under ARC
        MockPerFrameStateRow *retained = row; // ARC retains
        CFArrayRemoveValueAtIndex(_pool, _poolCount);
        return retained;
    }
    return nil;
}

- (void)returnRow:(MockPerFrameStateRow *)row {
    if (!row)
        return;
    if (_poolCount < kMaxPoolSize) {
        // Clear references
        row->_screenCharLine = nil;
        row->_selectedIndexSet = nil;
        row->_date = nil;
        row->_matches = nil;
        row->_eaIndex = nil;
        CFArrayAppendValue(_pool, (__bridge const void *)row);
        _poolCount++;
    }
}

- (void)returnRows:(NSArray *)rows {
    for (MockPerFrameStateRow *row in rows) {
        [self returnRow:row];
    }
}

- (void)drain {
    if (_pool)
        CFArrayRemoveAllValues(_pool);
    _poolCount = 0;
}

@end

// Get high precision time in seconds
static double getTimeSeconds(void) {
    static mach_timebase_info_data_t timebase;
    if (timebase.denom == 0) {
        mach_timebase_info(&timebase);
    }
    uint64_t time = mach_absolute_time();
    return (double)time * timebase.numer / timebase.denom / 1e9;
}

// Benchmark without pooling (baseline)
static double benchmarkWithoutPooling(int rows, int iterations) {
    double startTime = getTimeSeconds();

    for (int iter = 0; iter < iterations; iter++) {
        @autoreleasepool {
            NSMutableArray *rowArray = [NSMutableArray arrayWithCapacity:rows];

            // Simulate frame: allocate all rows
            for (int r = 0; r < rows; r++) {
                MockPerFrameStateRow *row = [[MockPerFrameStateRow alloc] init];
                [row repopulateForRow:r];
                [rowArray addObject:row];
            }

            // End of frame: rows go out of scope (ARC releases)
        }
    }

    double endTime = getTimeSeconds();
    return (endTime - startTime) * 1000.0; // Return milliseconds
}

// Benchmark with pooling (optimized)
static double benchmarkWithPooling(int rows, int iterations) {
    MockRowPool *pool = [MockRowPool sharedPool];
    [pool drain]; // Start fresh

    double startTime = getTimeSeconds();

    for (int iter = 0; iter < iterations; iter++) {
        @autoreleasepool {
            NSMutableArray *rowArray = [NSMutableArray arrayWithCapacity:rows];

            // Simulate frame: acquire from pool or allocate
            for (int r = 0; r < rows; r++) {
                MockPerFrameStateRow *row = [pool acquireRow];
                if (!row) {
                    row = [[MockPerFrameStateRow alloc] init];
                }
                [row repopulateForRow:r];
                [rowArray addObject:row];
            }

            // End of frame: return rows to pool
            [pool returnRows:rowArray];
        }
    }

    double endTime = getTimeSeconds();
    return (endTime - startTime) * 1000.0; // Return milliseconds
}

int main(int argc, const char *argv[]) {
    @autoreleasepool {
        printf("=================================================\n");
        printf("iTermMetalPerFrameStateRow Pool Benchmark\n");
        printf("=================================================\n\n");
        printf("This benchmark measures the allocation overhead savings\n");
        printf("from pooling row state objects in metalDriverWillBeginDrawingFrame.\n\n");

        // Warmup
        printf("Warming up...\n");
        benchmarkWithoutPooling(50, 100);
        benchmarkWithPooling(50, 100);
        printf("\n");

        const int iterations = 1000; // Simulate 1000 frames (~16.7 seconds at 60fps)

        printf("Running %d iterations (simulating ~%.1f seconds at 60fps)...\n\n", iterations, iterations / 60.0);

        int numConfigs = sizeof(configs) / sizeof(configs[0]);
        for (int c = 0; c < numConfigs; c++) {
            BenchmarkConfig config = configs[c];

            printf("Configuration: %s (%d rows x %d columns = %d row objects)\n", config.name, config.rows,
                   config.columns, config.rows);

            double timeWithout = benchmarkWithoutPooling(config.rows, iterations);
            double timeWith = benchmarkWithPooling(config.rows, iterations);

            double speedup = timeWithout / timeWith;
            double perFrameWithout = timeWithout / iterations;
            double perFrameWith = timeWith / iterations;

            printf("  Without pooling: %.2f ms total (%.4f ms/frame)\n", timeWithout, perFrameWithout);
            printf("  With pooling:    %.2f ms total (%.4f ms/frame)\n", timeWith, perFrameWith);
            printf("  Speedup:         %.2fx faster\n", speedup);
            printf("  Savings:         %.4f ms/frame (%.2f ms/sec at 60fps)\n\n", perFrameWithout - perFrameWith,
                   (perFrameWithout - perFrameWith) * 60);
        }

        printf("=================================================\n");
        printf("Summary\n");
        printf("=================================================\n");
        printf("The pool eliminates per-frame object allocations by reusing\n");
        printf("row objects across frames. This reduces memory pressure and\n");
        printf("allocation overhead in the hot path of metalDriverWillBeginDrawingFrame.\n");
        printf("\n");
        printf("At 60fps, even small per-frame savings compound:\n");
        printf("  - 0.01 ms/frame = 0.6 ms/sec = 36 ms/min of CPU time saved\n");
        printf("  - 0.05 ms/frame = 3.0 ms/sec = 180 ms/min of CPU time saved\n");
    }
    return 0;
}
