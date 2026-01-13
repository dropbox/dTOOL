//
//  eaindex_cache_benchmark.m
//  DashTerm2
//
//  Benchmark to measure the benefit of caching externalAttributeIndexForLine: results
//  alongside screenCharArrayForLine: caching. This simulates how the Metal renderer
//  queries both screen content and external attributes for each row every frame.
//
//  Build and run:
//    clang -O3 -fobjc-arc -framework Foundation -framework CoreFoundation \
//      benchmarks/eaindex_cache_benchmark.m -o benchmarks/eaindex_cache_benchmark && \
//      benchmarks/eaindex_cache_benchmark
//

#import <Foundation/Foundation.h>
#import <mach/mach_time.h>

static const NSUInteger kViewportRows = 60;
static const NSUInteger kLineWidth = 120;
static const NSUInteger kCacheLimit = 256;
static volatile uint32_t gSyntheticAccumulator = 0;

static uint64_t AbsoluteTimeInNanoseconds(uint64_t start, uint64_t end) {
    static dispatch_once_t onceToken;
    static mach_timebase_info_data_t timebase;
    dispatch_once(&onceToken, ^{
        mach_timebase_info(&timebase);
    });
    const uint64_t elapsed = end - start;
    return elapsed * timebase.numer / timebase.denom;
}

// Simulated external attribute index - stores URL/underline info per column
@interface SimulatedEAIndex : NSObject <NSCopying>
@property (nonatomic, readonly) NSUInteger attributeCount;
- (instancetype)initWithAttributeCount:(NSUInteger)count;
@end

@implementation SimulatedEAIndex {
    NSMutableDictionary<NSNumber *, NSNumber *> *_attributes;
}

- (instancetype)initWithAttributeCount:(NSUInteger)count {
    self = [super init];
    if (self) {
        _attributes = [NSMutableDictionary dictionaryWithCapacity:count];
        for (NSUInteger i = 0; i < count; i++) {
            _attributes[@(i * 10)] = @(i + 1);  // Sparse attributes at every 10th column
        }
        _attributeCount = count;
    }
    return self;
}

- (instancetype)initEmpty {
    self = [super init];
    if (self) {
        _attributes = nil;
        _attributeCount = 0;
    }
    return self;
}

- (id)copyWithZone:(NSZone *)zone {
    // Optimization: return nil for empty indices (matches #136 optimization)
    if (_attributeCount == 0) {
        return nil;
    }
    SimulatedEAIndex *copy = [[SimulatedEAIndex alloc] init];
    copy->_attributes = [_attributes mutableCopy];
    copy->_attributeCount = _attributeCount;
    return copy;
}

- (NSUInteger)lookupColumn:(NSUInteger)column {
    NSNumber *value = _attributes[@(column)];
    return value ? value.unsignedIntegerValue : 0;
}

@end

// Cache entry matching VT100ScreenLineCacheEntry structure
@interface CacheEntry : NSObject
@property (nonatomic, strong) NSData *screenCharArray;
@property (nonatomic, strong) SimulatedEAIndex *eaIndex;
@property (nonatomic, assign) BOOL hasQueriedEAIndex;
@property (nonatomic, assign) BOOL representsGridLine;
@property (nonatomic, assign) int gridLineIndex;
@property (nonatomic, assign) NSTimeInterval gridTimestamp;
@end

@implementation CacheEntry
@end

@interface SimulatedScreen : NSObject
- (instancetype)initWithInitialLines:(NSUInteger)count width:(NSUInteger)width eaIndexDensity:(float)density;
- (NSData *)screenCharArrayForLine:(NSUInteger)line useCache:(BOOL)useCache;
- (SimulatedEAIndex *)externalAttributeIndexForLine:(NSUInteger)line useCache:(BOOL)useCache;
- (void)mutateRandomVisibleLineWithViewport:(NSRange)viewport;
- (void)scrollBy:(NSUInteger)lines;
@property (nonatomic, readonly) NSUInteger scrollOffset;
@end

@implementation SimulatedScreen {
    NSMutableArray<NSMutableData *> *_lines;
    NSMutableArray<SimulatedEAIndex *> *_eaIndices;
    NSUInteger _width;
    NSMutableDictionary<NSNumber *, CacheEntry *> *_cache;
    NSMutableArray<NSNumber *> *_cacheOrder;
    NSUInteger _scrollOffset;
    uint64_t _rngState;
    float _eaIndexDensity;
}

- (instancetype)initWithInitialLines:(NSUInteger)count width:(NSUInteger)width eaIndexDensity:(float)density {
    self = [super init];
    if (self) {
        _width = width;
        _eaIndexDensity = density;
        _lines = [NSMutableArray arrayWithCapacity:count];
        _eaIndices = [NSMutableArray arrayWithCapacity:count];
        for (NSUInteger i = 0; i < count; i++) {
            [_lines addObject:[self generateLineDataWithSeed:(uint32_t)i]];
            [_eaIndices addObject:[self generateEAIndexForLine:i]];
        }
        _cache = [NSMutableDictionary dictionaryWithCapacity:kCacheLimit];
        _cacheOrder = [NSMutableArray arrayWithCapacity:kCacheLimit];
        _rngState = 0x123456789ULL;
    }
    return self;
}

- (uint32_t)nextRandom {
    _rngState ^= _rngState << 7;
    _rngState ^= _rngState >> 9;
    _rngState ^= _rngState << 8;
    return (uint32_t)_rngState;
}

- (NSMutableData *)generateLineDataWithSeed:(uint32_t)seed {
    NSMutableData *data = [NSMutableData dataWithLength:(_width + 1) * sizeof(uint16_t)];
    uint16_t *cells = (uint16_t *)data.mutableBytes;
    uint32_t value = seed + 1;
    for (NSUInteger idx = 0; idx < _width; idx++) {
        value = value * 1103515245 + 12345;
        cells[idx] = (uint16_t)((value >> 16) & 0x7FFF) % 126 + 32;
    }
    cells[_width] = 0;
    return data;
}

- (SimulatedEAIndex *)generateEAIndexForLine:(NSUInteger)line {
    // Simulate typical terminal content: most lines have no external attributes,
    // but some lines (URLs, underlines) have sparse attributes
    float roll = (float)((line * 12345 + 67890) % 1000) / 1000.0f;
    if (roll < _eaIndexDensity) {
        // Line has external attributes (e.g., a URL or underline)
        NSUInteger attrCount = 1 + (line % 5);  // 1-5 attributes per line
        return [[SimulatedEAIndex alloc] initWithAttributeCount:attrCount];
    }
    return [[SimulatedEAIndex alloc] initEmpty];
}

- (void)evictOldCacheEntriesIfNeeded {
    while (_cache.count > kCacheLimit && _cacheOrder.count > 0) {
        NSNumber *key = _cacheOrder.firstObject;
        [_cacheOrder removeObjectAtIndex:0];
        [_cache removeObjectForKey:key];
    }
}

- (void)touchCacheKey:(NSNumber *)key {
    [_cacheOrder removeObject:key];
    [_cacheOrder addObject:key];
}

- (NSMutableData *)dataForAbsoluteLine:(NSUInteger)absoluteLine {
    while (absoluteLine >= _lines.count) {
        [_lines addObject:[self generateLineDataWithSeed:(uint32_t)_lines.count]];
        [_eaIndices addObject:[self generateEAIndexForLine:_lines.count - 1]];
    }
    return _lines[absoluteLine];
}

- (SimulatedEAIndex *)eaIndexForAbsoluteLine:(NSUInteger)absoluteLine {
    while (absoluteLine >= _eaIndices.count) {
        [_lines addObject:[self generateLineDataWithSeed:(uint32_t)_lines.count]];
        [_eaIndices addObject:[self generateEAIndexForLine:_eaIndices.count]];
    }
    return _eaIndices[absoluteLine];
}

- (NSData *)screenCharArrayForLine:(NSUInteger)line useCache:(BOOL)useCache {
    const NSUInteger absoluteLine = _scrollOffset + line;
    NSNumber *key = @(absoluteLine);

    if (useCache) {
        CacheEntry *entry = _cache[key];
        if (entry && entry.screenCharArray) {
            [self touchCacheKey:key];
            return entry.screenCharArray;
        }
    }

    // Simulate expensive line lookup and processing
    NSMutableData *source = [self dataForAbsoluteLine:absoluteLine];
    NSData *copy = [NSData dataWithBytes:source.bytes length:source.length];

    // Simulate per-line processing cost
    const uint16_t *cells = (const uint16_t *)copy.bytes;
    uint32_t checksum = 0;
    for (NSUInteger rep = 0; rep < 256; rep++) {
        for (NSUInteger idx = 0; idx < _width; idx++) {
            checksum += cells[idx] * (uint32_t)(idx + 1 + rep);
        }
    }
    gSyntheticAccumulator ^= checksum;

    if (useCache) {
        CacheEntry *entry = _cache[key];
        if (!entry) {
            entry = [[CacheEntry alloc] init];
            _cache[key] = entry;
            [_cacheOrder addObject:key];
            [self evictOldCacheEntriesIfNeeded];
        }
        entry.screenCharArray = copy;
    }
    return copy;
}

- (SimulatedEAIndex *)externalAttributeIndexForLine:(NSUInteger)line useCache:(BOOL)useCache {
    const NSUInteger absoluteLine = _scrollOffset + line;
    NSNumber *key = @(absoluteLine);

    if (useCache) {
        CacheEntry *entry = _cache[key];
        if (entry && entry.hasQueriedEAIndex) {
            [self touchCacheKey:key];
            return entry.eaIndex;
        }
    }

    // Simulate metadata lookup cost (determining if line is in scrollback vs grid,
    // fetching metadata from linebuffer or grid). In real code:
    // - numLinesWithWidth: traverses LineBlockMetadataArray
    // - metadataForLineNumber: does binary search in linebuffer
    // - For grid lines, it's cheaper (direct array access)
    SimulatedEAIndex *source = [self eaIndexForAbsoluteLine:absoluteLine];

    // Simulate the work of metadataOnLine: more realistically
    // This includes the numLinesWithWidth computation and metadata fetch
    uint32_t work = 0;
    for (NSUInteger i = 0; i < 512; i++) {
        work += (uint32_t)(absoluteLine * i + _width);
        work ^= (uint32_t)((i + 1) * 7919);  // Prime multiplication
    }
    gSyntheticAccumulator ^= work;

    // Copy the index (this was optimized in #136 for empty indices)
    SimulatedEAIndex *result = [source copy];

    if (useCache) {
        CacheEntry *entry = _cache[key];
        if (!entry) {
            entry = [[CacheEntry alloc] init];
            _cache[key] = entry;
            [_cacheOrder addObject:key];
            [self evictOldCacheEntriesIfNeeded];
        }
        entry.eaIndex = result;
        entry.hasQueriedEAIndex = YES;
    }
    return result;
}

- (void)mutateRandomVisibleLineWithViewport:(NSRange)viewport {
    if (viewport.length == 0) return;
    const NSUInteger relative = (NSUInteger)([self nextRandom] % viewport.length);
    const NSUInteger absoluteLine = _scrollOffset + viewport.location + relative;
    NSMutableData *line = [self dataForAbsoluteLine:absoluteLine];
    uint16_t *cells = (uint16_t *)line.mutableBytes;
    for (NSUInteger idx = 0; idx < MIN((NSUInteger)8, _width); idx++) {
        cells[idx] = (cells[idx] + 1) % 126 + 32;
    }
    [_cache removeObjectForKey:@(absoluteLine)];
    [_cacheOrder removeObject:@(absoluteLine)];
}

- (void)scrollBy:(NSUInteger)lines {
    if (lines == 0) return;
    _scrollOffset += lines;
    if (_cache.count == 0) return;
    NSMutableArray<NSNumber *> *dropped = [NSMutableArray array];
    [_cache enumerateKeysAndObjectsUsingBlock:^(NSNumber *key, CacheEntry *obj, BOOL *stop) {
        if (key.unsignedLongLongValue < _scrollOffset) {
            [dropped addObject:key];
        }
    }];
    for (NSNumber *key in dropped) {
        [_cache removeObjectForKey:key];
        [_cacheOrder removeObject:key];
    }
}

- (NSUInteger)scrollOffset {
    return _scrollOffset;
}

@end

typedef struct {
    double timeMs;
    uint64_t checksum;
} BenchmarkResult;

static BenchmarkResult RunScenario(BOOL useCache, NSUInteger frames, NSUInteger viewportRows,
                                   NSUInteger mutateEveryNFrames, NSUInteger scrollEveryNFrames,
                                   float eaIndexDensity) {
    SimulatedScreen *screen = [[SimulatedScreen alloc] initWithInitialLines:(frames + viewportRows + 512)
                                                                      width:kLineWidth
                                                             eaIndexDensity:eaIndexDensity];
    uint64_t checksum = 0;
    const uint64_t start = mach_absolute_time();

    for (NSUInteger frame = 1; frame <= frames; frame++) {
        // Simulate Metal renderer's per-frame row population:
        // For each row, it calls screenCharArrayForLine: AND externalAttributeIndexForLine:
        for (NSUInteger row = 0; row < viewportRows; row++) {
            NSData *line = [screen screenCharArrayForLine:row useCache:useCache];
            SimulatedEAIndex *eaIndex = [screen externalAttributeIndexForLine:row useCache:useCache];

            const uint16_t *cells = (const uint16_t *)line.bytes;
            checksum += cells[0];
            if (eaIndex) {
                checksum += [eaIndex lookupColumn:0];
            }
        }

        if (mutateEveryNFrames && (frame % mutateEveryNFrames == 0)) {
            [screen mutateRandomVisibleLineWithViewport:NSMakeRange(0, viewportRows)];
        }
        if (scrollEveryNFrames && (frame % scrollEveryNFrames == 0)) {
            [screen scrollBy:1];
        }
    }

    const uint64_t end = mach_absolute_time();
    return (BenchmarkResult){
        .timeMs = (double)AbsoluteTimeInNanoseconds(start, end) / 1e6,
        .checksum = checksum
    };
}

static void PrintScenario(NSString *name, NSUInteger frames, NSUInteger mutateEvery, NSUInteger scrollEvery,
                          float eaIndexDensity) {
    BenchmarkResult uncached = RunScenario(NO, frames, kViewportRows, mutateEvery, scrollEvery, eaIndexDensity);
    BenchmarkResult cached = RunScenario(YES, frames, kViewportRows, mutateEvery, scrollEvery, eaIndexDensity);

    printf("\nScenario: %s\n", name.UTF8String);
    printf("  EA Index density: %.0f%%\n", eaIndexDensity * 100);
    printf("  Without cache: %8.2f ms\n", uncached.timeMs);
    printf("  With cache:    %8.2f ms\n", cached.timeMs);
    if (cached.timeMs > 0) {
        printf("  Speedup:       %6.2fx\n", uncached.timeMs / cached.timeMs);
    }
    if (uncached.checksum != cached.checksum) {
        printf("  Warning: checksum mismatch (%llu vs %llu)\n",
               (unsigned long long)uncached.checksum, (unsigned long long)cached.checksum);
    }
}

int main(int argc, const char *argv[]) {
    @autoreleasepool {
        printf("DashTerm2 External Attribute Index Cache Benchmark\n");
        printf("===================================================\n");
        printf("Viewport: %lu rows, width %lu, cache limit %lu\n",
               (unsigned long)kViewportRows, (unsigned long)kLineWidth, (unsigned long)kCacheLimit);
        printf("\nThis benchmark measures the benefit of caching externalAttributeIndexForLine:\n");
        printf("results alongside screenCharArrayForLine: caching. The Metal renderer calls\n");
        printf("both methods for every row on every frame.\n");

        // Test with different EA index densities
        printf("\n--- Low EA density (10%% of lines have URLs/underlines) ---\n");
        PrintScenario(@"Static viewport", 2000, 0, 0, 0.10f);
        PrintScenario(@"Cursor updates every 60 frames", 2000, 60, 0, 0.10f);
        PrintScenario(@"Steady scroll", 2000, 90, 40, 0.10f);

        printf("\n--- No EA attributes (plain text terminal) ---\n");
        PrintScenario(@"Static viewport", 2000, 0, 0, 0.0f);
        PrintScenario(@"Cursor updates every 60 frames", 2000, 60, 0, 0.0f);

        printf("\n--- High EA density (50%% of lines have attributes) ---\n");
        PrintScenario(@"Static viewport", 2000, 0, 0, 0.50f);
        PrintScenario(@"Cursor updates every 60 frames", 2000, 60, 0, 0.50f);

        printf("\n(Lower is better, higher speedup is better)\n");
        printf("gSyntheticAccumulator = %u (prevents optimizer from eliminating work)\n", gSyntheticAccumulator);
    }
    return 0;
}
