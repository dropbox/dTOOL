/*
 * Row Data Pool Benchmark
 *
 * Measures allocation performance for iTermMetalRowData with and without pooling.
 * This benchmark simulates the per-frame allocation pattern in Metal rendering.
 *
 * Build:
 *   clang -framework Foundation -fobjc-arc -O3 -I../sources -I../sources/Metal/Infrastructure \
 *     benchmarks/row_data_pool_benchmark.m -o benchmarks/row_data_pool_benchmark
 *
 * Run:
 *   ./benchmarks/row_data_pool_benchmark
 */

#import <Foundation/Foundation.h>
#import <mach/mach_time.h>

// ============================================================================
// Minimal re-implementations for standalone benchmark
// (We can't link against the full app, so we create minimal versions)
// ============================================================================

// Simulated iTermMetalGlyphKey (actual size from the app)
typedef struct {
    unsigned char type;
    union {
        struct {
            unsigned short code;
            unsigned short combiningSuccessor;
            unsigned char isComplex;
            unsigned char boxDrawing;
            unsigned char drawable;
        } regular;
        struct {
            unsigned int fontID;
            unsigned short glyphNumber;
            double positionX;
            double positionY;
            unsigned int flags;
        } decomposed;
    } payload;
    unsigned char typeface;
    unsigned char thinStrokes;
    int visualColumn;
    int logicalIndex;
} BenchmarkGlyphKey;

// Simulated iTermMetalGlyphAttributes
typedef struct {
    float foregroundColor[4];
    float backgroundColor[4];
    float unprocessedBackgroundColor[4];
    unsigned char hasUnderlineColor;
    float underlineColor[4];
    unsigned char underlineStyle;
    unsigned char annotation;
} BenchmarkGlyphAttributes;

// Simulated iTermMetalBackgroundColorRLE
typedef struct {
    float color[4];
    unsigned short count;
} BenchmarkBackgroundColorRLE;

// ============================================================================
// Benchmark Data Classes (simulating iTermData subclasses)
// ============================================================================

@interface BenchmarkData : NSObject
@property (nonatomic, readonly) void *mutableBytes;
@property (nonatomic) NSUInteger length;
- (instancetype)initWithLength:(NSUInteger)length;
@end

@implementation BenchmarkData {
    void *_mutableBytes;
    NSUInteger _length;
}

- (instancetype)initWithLength:(NSUInteger)length {
    self = [super init];
    if (self) {
        _mutableBytes = malloc(length + 64); // Guard region
        _length = length;
    }
    return self;
}

- (void)dealloc {
    if (_mutableBytes) {
        free(_mutableBytes);
    }
}

- (void)setLength:(NSUInteger)length {
    _length = length;
    _mutableBytes = realloc(_mutableBytes, length + 64);
}

@end

// ============================================================================
// Benchmark Row Data (simulating iTermMetalRowData)
// ============================================================================

@interface BenchmarkRowData : NSObject
@property (nonatomic) int y;
@property (nonatomic) long long absLine;
@property (nonatomic, strong) BenchmarkData *keysData;
@property (nonatomic, strong) BenchmarkData *attributesData;
@property (nonatomic, strong) BenchmarkData *backgroundColorRLEData;
@property (nonatomic) int numberOfDrawableGlyphs;
@property (nonatomic) int numberOfBackgroundRLEs;
@end

@implementation BenchmarkRowData
@end

// ============================================================================
// Benchmark Row Data Pool (simulating iTermMetalRowDataPool)
// ============================================================================

static const NSUInteger kMaxPoolSize = 128;

@interface BenchmarkRowDataPool : NSObject
@property (nonatomic, readonly) NSUInteger pooledCount;
@property (nonatomic, readonly) NSUInteger totalCreated;
@property (nonatomic, readonly) NSUInteger totalAcquisitions;
@property (nonatomic, readonly) NSUInteger totalReturns;

+ (instancetype)sharedPool;
- (BenchmarkRowData *)acquireRowDataForColumns:(int)columns;
- (void)returnRowData:(BenchmarkRowData *)rowData;
- (void)returnRowDataArray:(NSArray<BenchmarkRowData *> *)rowDataArray;
- (void)drain;
@end

@implementation BenchmarkRowDataPool {
    CFMutableArrayRef _pool;
    NSUInteger _poolCount;
}

+ (instancetype)sharedPool {
    static BenchmarkRowDataPool *instance;
    static dispatch_once_t onceToken;
    dispatch_once(&onceToken, ^{
        instance = [[BenchmarkRowDataPool alloc] init];
    });
    return instance;
}

- (instancetype)init {
    self = [super init];
    if (self) {
        _pool = CFArrayCreateMutable(kCFAllocatorDefault, 32, &kCFTypeArrayCallBacks);
        _poolCount = 0;
        _totalCreated = 0;
        _totalAcquisitions = 0;
        _totalReturns = 0;
    }
    return self;
}

- (void)dealloc {
    if (_pool) {
        CFRelease(_pool);
    }
}

- (NSUInteger)pooledCount {
    return _poolCount;
}

- (BenchmarkRowData *)acquireRowDataForColumns:(int)columns {
    _totalAcquisitions++;

    BenchmarkRowData *rowData = nil;

    if (_poolCount > 0) {
        _poolCount--;
        // Get value and retain it before removing from array
        CFTypeRef value = CFArrayGetValueAtIndex(_pool, _poolCount);
        CFRetain(value);
        CFArrayRemoveValueAtIndex(_pool, _poolCount);
        rowData = (__bridge_transfer BenchmarkRowData *)value;

        // Ensure buffers are large enough
        const NSUInteger glyphKeySize = sizeof(BenchmarkGlyphKey) * columns;
        const NSUInteger attributesSize = sizeof(BenchmarkGlyphAttributes) * columns;
        const NSUInteger rleSize = sizeof(BenchmarkBackgroundColorRLE) * columns;

        if (rowData.keysData.length < glyphKeySize) {
            rowData.keysData.length = glyphKeySize;
        }
        if (rowData.attributesData.length < attributesSize) {
            rowData.attributesData.length = attributesSize;
        }
        if (rowData.backgroundColorRLEData.length < rleSize) {
            rowData.backgroundColorRLEData.length = rleSize;
        }
    } else {
        _totalCreated++;
        rowData = [[BenchmarkRowData alloc] init];

        const NSUInteger glyphKeySize = sizeof(BenchmarkGlyphKey) * columns;
        const NSUInteger attributesSize = sizeof(BenchmarkGlyphAttributes) * columns;
        const NSUInteger rleSize = sizeof(BenchmarkBackgroundColorRLE) * columns;

        rowData.keysData = [[BenchmarkData alloc] initWithLength:glyphKeySize];
        rowData.attributesData = [[BenchmarkData alloc] initWithLength:attributesSize];
        rowData.backgroundColorRLEData = [[BenchmarkData alloc] initWithLength:rleSize];
    }

    // Reset transient properties
    rowData.y = 0;
    rowData.absLine = 0;
    rowData.numberOfDrawableGlyphs = 0;
    rowData.numberOfBackgroundRLEs = 0;

    return rowData;
}

- (void)returnRowData:(BenchmarkRowData *)rowData {
    if (!rowData)
        return;

    _totalReturns++;

    if (_poolCount < kMaxPoolSize) {
        CFArrayAppendValue(_pool, (__bridge const void *)rowData);
        _poolCount++;
    }
}

- (void)returnRowDataArray:(NSArray<BenchmarkRowData *> *)rowDataArray {
    for (BenchmarkRowData *rowData in rowDataArray) {
        [self returnRowData:rowData];
    }
}

- (void)drain {
    if (_pool) {
        CFArrayRemoveAllValues(_pool);
    }
    _poolCount = 0;
}

@end

// ============================================================================
// Benchmark Utilities
// ============================================================================

static inline uint64_t MachTicksToNanoseconds(uint64_t ticks) {
    static mach_timebase_info_data_t info;
    if (info.denom == 0) {
        mach_timebase_info(&info);
    }
    __uint128_t scaled = (__uint128_t)ticks * (uint64_t)info.numer;
    return (uint64_t)(scaled / info.denom);
}

// ============================================================================
// Benchmark: No Pooling (current baseline behavior)
// ============================================================================

static void BenchmarkNoPooling(int rows, int columns, int frames, double *avgNsPerFrame) {
    uint64_t totalNs = 0;

    for (int f = 0; f < frames; f++) {
        @autoreleasepool {
            uint64_t start = mach_absolute_time();

            NSMutableArray<BenchmarkRowData *> *rowDataArray = [NSMutableArray arrayWithCapacity:rows];

            for (int y = 0; y < rows; y++) {
                BenchmarkRowData *rowData = [[BenchmarkRowData alloc] init];
                rowData.y = y;
                rowData.absLine = y + 1000;
                rowData.keysData = [[BenchmarkData alloc] initWithLength:sizeof(BenchmarkGlyphKey) * columns];
                rowData.attributesData =
                    [[BenchmarkData alloc] initWithLength:sizeof(BenchmarkGlyphAttributes) * columns];
                rowData.backgroundColorRLEData =
                    [[BenchmarkData alloc] initWithLength:sizeof(BenchmarkBackgroundColorRLE) * columns];
                [rowDataArray addObject:rowData];
            }

            // Simulate frame completion (objects released when array goes out of scope)
            uint64_t delta = mach_absolute_time() - start;
            totalNs += MachTicksToNanoseconds(delta);
        }
    }

    *avgNsPerFrame = (double)totalNs / (double)frames;
}

// ============================================================================
// Benchmark: With Pooling (optimized behavior)
// ============================================================================

static void BenchmarkWithPooling(int rows, int columns, int frames, double *avgNsPerFrame) {
    BenchmarkRowDataPool *pool = [BenchmarkRowDataPool sharedPool];
    [pool drain]; // Start fresh

    uint64_t totalNs = 0;

    for (int f = 0; f < frames; f++) {
        @autoreleasepool {
            uint64_t start = mach_absolute_time();

            NSMutableArray<BenchmarkRowData *> *rowDataArray = [NSMutableArray arrayWithCapacity:rows];

            for (int y = 0; y < rows; y++) {
                BenchmarkRowData *rowData = [pool acquireRowDataForColumns:columns];
                rowData.y = y;
                rowData.absLine = y + 1000;
                [rowDataArray addObject:rowData];
            }

            // Simulate frame completion
            [pool returnRowDataArray:rowDataArray];

            uint64_t delta = mach_absolute_time() - start;
            totalNs += MachTicksToNanoseconds(delta);
        }
    }

    *avgNsPerFrame = (double)totalNs / (double)frames;
}

// ============================================================================
// Main
// ============================================================================

int main(int argc, const char *argv[]) {
    @autoreleasepool {
        printf("Row Data Pool Allocation Benchmark\n");
        printf("===================================\n\n");

        // Test configurations
        typedef struct {
            int rows;
            int columns;
            const char *name;
        } Config;

        Config configs[] = {
            {24, 80, "24x80 (standard)"},
            {50, 120, "50x120 (medium)"},
            {100, 200, "100x200 (large)"},
        };

        const int numConfigs = sizeof(configs) / sizeof(configs[0]);
        const int warmupFrames = 100;
        const int benchmarkFrames = 1000;

        for (int c = 0; c < numConfigs; c++) {
            Config cfg = configs[c];
            printf("Configuration: %s (%d rows x %d columns)\n", cfg.name, cfg.rows, cfg.columns);
            printf("Frames: %d (warmup: %d)\n\n", benchmarkFrames, warmupFrames);

            double noPoolNs, poolNs;

            // Warmup
            BenchmarkNoPooling(cfg.rows, cfg.columns, warmupFrames, &noPoolNs);
            BenchmarkWithPooling(cfg.rows, cfg.columns, warmupFrames, &poolNs);

            // Benchmark
            BenchmarkNoPooling(cfg.rows, cfg.columns, benchmarkFrames, &noPoolNs);
            BenchmarkWithPooling(cfg.rows, cfg.columns, benchmarkFrames, &poolNs);

            double noPoolMs = noPoolNs / 1e6;
            double poolMs = poolNs / 1e6;
            double speedup = noPoolNs / poolNs;

            printf("  No Pooling:   %.4f ms/frame (%.1f us/row)\n", noPoolMs, (noPoolNs / cfg.rows) / 1000.0);
            printf("  With Pooling: %.4f ms/frame (%.1f us/row)\n", poolMs, (poolNs / cfg.rows) / 1000.0);
            printf("  Speedup:      %.2fx faster\n\n", speedup);

            // Calculate allocations saved per second at 60 fps
            int allocsPerFrame = cfg.rows * 4; // 4 objects per row
            int allocsSavedPerSecond = allocsPerFrame * 60;
            printf("  Allocations saved at 60fps: %d/sec\n", allocsSavedPerSecond);

            // Pool stats
            BenchmarkRowDataPool *pool = [BenchmarkRowDataPool sharedPool];
            printf("  Pool stats: created=%lu, acquisitions=%lu, returns=%lu\n\n", (unsigned long)pool.totalCreated,
                   (unsigned long)pool.totalAcquisitions, (unsigned long)pool.totalReturns);

            [pool drain];
        }

        printf("Benchmark complete.\n");
    }
    return 0;
}
