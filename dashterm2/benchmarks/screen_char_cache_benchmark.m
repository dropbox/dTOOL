//
//  screen_char_cache_benchmark.m
//  DashTerm2
//
//  Benchmark to measure the benefit of caching screenCharArrayForLine: results
//  across frames. This simulates a terminal viewport being re-rendered repeatedly
//  with and without caching.
//
//  Build and run:
//    clang -O3 -fobjc-arc -framework Foundation -framework CoreFoundation \
//      Benchmarks/screen_char_cache_benchmark.m -o /tmp/screen_char_cache_benchmark && \
//      /tmp/screen_char_cache_benchmark
//

#import <Foundation/Foundation.h>
#import <mach/mach_time.h>

static const NSUInteger kViewportRows = 60;
static const NSUInteger kLineWidth = 120;
static const NSUInteger kCacheLimit = 512;
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

@interface SimulatedScreen : NSObject
- (instancetype)initWithInitialLines:(NSUInteger)count width:(NSUInteger)width;
- (NSData *)screenCharArrayForRelativeLine:(NSUInteger)relativeLine useCache:(BOOL)useCache;
- (void)mutateRandomVisibleLineWithViewport:(NSRange)viewport;
- (void)scrollBy:(NSUInteger)lines;
@property (nonatomic, readonly) NSUInteger scrollOffset;
@end

@implementation SimulatedScreen {
    NSMutableArray<NSMutableData *> *_lines;
    NSUInteger _width;
    NSMutableDictionary<NSNumber *, NSData *> *_cache;
    NSMutableArray<NSNumber *> *_cacheOrder;
    NSUInteger _scrollOffset;
    uint64_t _rngState;
}

- (instancetype)initWithInitialLines:(NSUInteger)count width:(NSUInteger)width {
    self = [super init];
    if (self) {
        _width = width;
        _lines = [NSMutableArray arrayWithCapacity:count];
        for (NSUInteger i = 0; i < count; i++) {
            [_lines addObject:[self generateLineDataWithSeed:(uint32_t)i]];
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
    cells[_width] = 0; // Continuation code placeholder
    return data;
}

- (NSMutableData *)dataForAbsoluteLine:(NSUInteger)absoluteLine {
    while (absoluteLine >= _lines.count) {
        [_lines addObject:[self generateLineDataWithSeed:(uint32_t)_lines.count]];
    }
    return _lines[absoluteLine];
}

- (void)evictOldCacheEntriesIfNeeded {
    while (_cache.count > kCacheLimit && _cacheOrder.count > 0) {
        NSNumber *key = _cacheOrder.firstObject;
        [_cacheOrder removeObjectAtIndex:0];
        [_cache removeObjectForKey:key];
    }
}

- (void)cacheScreenCharArray:(NSData *)line absoluteLine:(NSUInteger)absoluteLine {
    NSNumber *key = @(absoluteLine);
    _cache[key] = line;
    [_cacheOrder removeObject:key];
    [_cacheOrder addObject:key];
    [self evictOldCacheEntriesIfNeeded];
}

- (NSData *)screenCharArrayForRelativeLine:(NSUInteger)relativeLine useCache:(BOOL)useCache {
    const NSUInteger absoluteLine = _scrollOffset + relativeLine;
    if (useCache) {
        NSNumber *key = @(absoluteLine);
        NSData *cached = _cache[key];
        if (cached) {
            [_cacheOrder removeObject:key];
            [_cacheOrder addObject:key];
            return cached;
        }
    }

    NSMutableData *source = [self dataForAbsoluteLine:absoluteLine];
    NSData *copy = [NSData dataWithBytes:source.bytes length:source.length];

    // Simulate expensive per-line processing (wcwidth, bidi, attribute scanning)
    const uint16_t *cells = (const uint16_t *)copy.bytes;
    uint32_t checksum = 0;
    for (NSUInteger rep = 0; rep < 512; rep++) {
        for (NSUInteger idx = 0; idx < _width; idx++) {
            checksum += cells[idx] * (uint32_t)(idx + 1 + rep);
        }
    }
    gSyntheticAccumulator ^= checksum;
    if (useCache) {
        [self cacheScreenCharArray:copy absoluteLine:absoluteLine];
    }
    return copy;
}

- (void)mutateRandomVisibleLineWithViewport:(NSRange)viewport {
    if (viewport.length == 0) {
        return;
    }
    const NSUInteger relative = (NSUInteger)([self nextRandom] % (viewport.length));
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
    if (lines == 0) {
        return;
    }
    _scrollOffset += lines;
    if (_cache.count == 0) {
        return;
    }
    NSMutableArray<NSNumber *> *dropped = [NSMutableArray array];
    [_cache enumerateKeysAndObjectsUsingBlock:^(NSNumber *key, NSData *obj, BOOL *stop) {
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

static double RunScenario(BOOL useCache, NSUInteger frames, NSUInteger viewportRows, NSUInteger mutateEveryNFrames,
                          NSUInteger scrollEveryNFrames, uint64_t *checksumOut) {
    SimulatedScreen *screen = [[SimulatedScreen alloc] initWithInitialLines:(frames + viewportRows + 512)
                                                                      width:kLineWidth];
    uint64_t checksum = 0;
    const uint64_t start = mach_absolute_time();
    for (NSUInteger frame = 1; frame <= frames; frame++) {
        for (NSUInteger row = 0; row < viewportRows; row++) {
            NSData *line = [screen screenCharArrayForRelativeLine:row useCache:useCache];
            const uint16_t *cells = (const uint16_t *)line.bytes;
            checksum += cells[0];
        }
        if (mutateEveryNFrames && (frame % mutateEveryNFrames == 0)) {
            NSRange viewport = NSMakeRange(0, viewportRows);
            [screen mutateRandomVisibleLineWithViewport:viewport];
        }
        if (scrollEveryNFrames && (frame % scrollEveryNFrames == 0)) {
            [screen scrollBy:1];
        }
    }
    const uint64_t end = mach_absolute_time();
    if (checksumOut) {
        *checksumOut = checksum;
    }
    return (double)AbsoluteTimeInNanoseconds(start, end) / 1e6;
}

static void PrintScenario(NSString *name, NSUInteger frames, NSUInteger mutateEvery, NSUInteger scrollEvery) {
    uint64_t checksumUncached = 0;
    uint64_t checksumCached = 0;
    double uncached = RunScenario(NO, frames, kViewportRows, mutateEvery, scrollEvery, &checksumUncached);
    double cached = RunScenario(YES, frames, kViewportRows, mutateEvery, scrollEvery, &checksumCached);

    printf("\nScenario: %s\n", name.UTF8String);
    printf("  Without cache: %8.2f ms\n", uncached);
    printf("  With cache:    %8.2f ms\n", cached);
    if (cached > 0) {
        printf("  Speedup:       %6.2fx\n", uncached / cached);
    }
    if (checksumUncached != checksumCached) {
        printf("  Warning: checksum mismatch (%llu vs %llu)\n", (unsigned long long)checksumUncached,
               (unsigned long long)checksumCached);
    }
}

int main(int argc, const char *argv[]) {
    @autoreleasepool {
        printf("DashTerm2 ScreenCharArray Cache Benchmark\n");
        printf("==========================================\n");
        printf("Viewport: %lu rows, width %lu\n", (unsigned long)kViewportRows, (unsigned long)kLineWidth);

        PrintScenario(@"Static viewport (no mutations)", 2500, 0, 0);
        PrintScenario(@"Cursor updates every 60 frames", 2500, 60, 0);
        PrintScenario(@"Steady scroll (mutate & scroll)", 2500, 90, 40);
    }
    return 0;
}
