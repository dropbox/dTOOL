/*
 * Pack/Unpack Micro-Benchmark
 *
 * This benchmark measures the performance impact of loop unrolling and
 * fast-path optimizations for pack/unpack routines.
 *
 * It implements both a baseline (simple loop) and optimized version
 * (loop unrolling, fast-path, prefetch) to show the performance delta.
 *
 * Compile and run:
 *   clang -framework Foundation -O2 -o benchmarks/pack_unpack_microbenchmark \
 *     benchmarks/pack_unpack_microbenchmark.m -fobjc-arc
 *   ./benchmarks/pack_unpack_microbenchmark
 */

#import <Foundation/Foundation.h>
#import <mach/mach_time.h>

#pragma mark - Minimal screen_char_t Definition

// ColorMode enum (from ScreenChar.h)
typedef NS_ENUM(unsigned int, ColorMode) {
    ColorModeAlternate = 0,
    ColorModeNormal = 1,
    ColorMode24bit = 2,
};

// Simplified screen_char_t (12 bytes, matching real structure)
// Note: Without __attribute__((packed)) the compiler aligns to 12 bytes naturally
typedef struct {
    uint16_t code;                      // 2 bytes - Unicode code point

    unsigned int foregroundColor : 8;   // Bit fields within 32-bit units
    unsigned int fgGreen : 8;
    unsigned int fgBlue : 8;

    unsigned int backgroundColor : 8;
    unsigned int bgGreen : 8;
    unsigned int bgBlue : 8;

    unsigned int foregroundColorMode : 2;
    unsigned int backgroundColorMode : 2;
    unsigned int complexChar : 1;
    unsigned int bold : 1;
    unsigned int faint : 1;
    unsigned int italic : 1;
    unsigned int blink : 1;
    unsigned int underline : 1;
    unsigned int image : 1;
    unsigned int strikethrough : 1;
    unsigned int invisible : 1;
    unsigned int inverse : 1;
    unsigned int guarded : 1;
    unsigned int virtualPlaceholder : 1;
} test_screen_char_t;

// Verify size matches real screen_char_t (should be 12 bytes)
_Static_assert(sizeof(test_screen_char_t) == 12, "test_screen_char_t must be 12 bytes");

#pragma mark - Packed Structure (8 bytes)

typedef struct __attribute__((packed)) {
    uint64_t code : 21;
    uint64_t fgColor : 9;
    uint64_t bgColor : 9;
    uint64_t complexChar : 1;
    uint64_t bold : 1;
    uint64_t faint : 1;
    uint64_t italic : 1;
    uint64_t blink : 1;
    uint64_t underline : 1;
    uint64_t strikethrough : 1;
    uint64_t image : 1;
    uint64_t invisible : 1;
    uint64_t inverse : 1;
    uint64_t guarded : 1;
    uint64_t virtualPlaceholder : 1;
    uint64_t fgIs24Bit : 1;
    uint64_t bgIs24Bit : 1;
    uint64_t reserved : 11;
} test_packed_char_t;

_Static_assert(sizeof(test_packed_char_t) == 8, "test_packed_char_t must be 8 bytes");

#pragma mark - Color Constants

typedef NS_ENUM(uint16_t, PackedColorIndex) {
    kPackedColorDefault = 256,
    kPackedColorSelected = 257,
    kPackedColorCursor = 258,
    kPackedColorReversedDefault = 259,
    kPackedColorSystemMessage = 260,
    kPackedColor24BitBase = 261,
};

#define ALTSEM_DEFAULT 0
#define ALTSEM_SELECTED 1
#define ALTSEM_CURSOR 2

#pragma mark - Color Table

@interface TestColorTable : NSObject {
    uint32_t _colors[251];
    int _count;
}
- (int)insertRed:(uint8_t)r green:(uint8_t)g blue:(uint8_t)b;
- (void)getColorAtIndex:(int)idx red:(uint8_t*)r green:(uint8_t*)g blue:(uint8_t*)b;
@end

@implementation TestColorTable

- (instancetype)init {
    self = [super init];
    if (self) {
        _count = 0;
        memset(_colors, 0, sizeof(_colors));
    }
    return self;
}

- (int)insertRed:(uint8_t)r green:(uint8_t)g blue:(uint8_t)b {
    uint32_t packed = ((uint32_t)r << 16) | ((uint32_t)g << 8) | b;
    for (int i = 0; i < _count; i++) {
        if (_colors[i] == packed) return i;
    }
    if (_count < 251) {
        _colors[_count] = packed;
        return _count++;
    }
    return 0;
}

- (void)getColorAtIndex:(int)idx red:(uint8_t*)r green:(uint8_t*)g blue:(uint8_t*)b {
    if (idx >= 0 && idx < _count) {
        uint32_t packed = _colors[idx];
        *r = (packed >> 16) & 0xFF;
        *g = (packed >> 8) & 0xFF;
        *b = packed & 0xFF;
    } else {
        *r = *g = *b = 0;
    }
}

@end

#pragma mark - Baseline Pack/Unpack (Simple Loop)

static inline BOOL CharNeeds24Bit(test_screen_char_t c) {
    return c.foregroundColorMode == ColorMode24bit || c.backgroundColorMode == ColorMode24bit;
}

static inline BOOL PackedCharNeeds24Bit(test_packed_char_t c) {
    return c.fgIs24Bit || c.bgIs24Bit;
}

static test_packed_char_t PackCharBaseline(test_screen_char_t src, TestColorTable *colorTable) {
    test_packed_char_t dst = {0};

    dst.code = src.code & 0x1FFFFF;
    dst.complexChar = src.complexChar;
    dst.bold = src.bold;
    dst.faint = src.faint;
    dst.italic = src.italic;
    dst.blink = src.blink;
    dst.underline = src.underline;
    dst.image = src.image;
    dst.strikethrough = src.strikethrough;
    dst.invisible = src.invisible;
    dst.inverse = src.inverse;
    dst.guarded = src.guarded;
    dst.virtualPlaceholder = src.virtualPlaceholder;

    dst.fgIs24Bit = 0;
    dst.bgIs24Bit = 0;

    // Foreground color
    switch (src.foregroundColorMode) {
        case ColorModeAlternate:
            dst.fgColor = kPackedColorDefault + src.foregroundColor;
            break;
        case ColorModeNormal:
            dst.fgColor = src.foregroundColor & 0xFF;
            break;
        case ColorMode24bit:
            if (colorTable) {
                int idx = [colorTable insertRed:src.foregroundColor
                                          green:src.fgGreen
                                           blue:src.fgBlue];
                dst.fgColor = kPackedColor24BitBase + idx;
                dst.fgIs24Bit = 1;
            } else {
                dst.fgColor = src.foregroundColor & 0xFF;
            }
            break;
    }

    // Background color
    switch (src.backgroundColorMode) {
        case ColorModeAlternate:
            dst.bgColor = kPackedColorDefault + src.backgroundColor;
            break;
        case ColorModeNormal:
            dst.bgColor = src.backgroundColor & 0xFF;
            break;
        case ColorMode24bit:
            if (colorTable) {
                int idx = [colorTable insertRed:src.backgroundColor
                                          green:src.bgGreen
                                           blue:src.bgBlue];
                dst.bgColor = kPackedColor24BitBase + idx;
                dst.bgIs24Bit = 1;
            } else {
                dst.bgColor = src.backgroundColor & 0xFF;
            }
            break;
    }

    return dst;
}

static test_screen_char_t UnpackCharBaseline(test_packed_char_t src, TestColorTable *colorTable) {
    test_screen_char_t dst = {0};

    dst.code = src.code;
    dst.complexChar = src.complexChar;
    dst.bold = src.bold;
    dst.faint = src.faint;
    dst.italic = src.italic;
    dst.blink = src.blink;
    dst.underline = src.underline;
    dst.image = src.image;
    dst.strikethrough = src.strikethrough;
    dst.invisible = src.invisible;
    dst.inverse = src.inverse;
    dst.guarded = src.guarded;
    dst.virtualPlaceholder = src.virtualPlaceholder;

    // Foreground color
    if (src.fgIs24Bit && colorTable) {
        int idx = src.fgColor - kPackedColor24BitBase;
        uint8_t r, g, b;
        [colorTable getColorAtIndex:idx red:&r green:&g blue:&b];
        dst.foregroundColor = r;
        dst.fgGreen = g;
        dst.fgBlue = b;
        dst.foregroundColorMode = ColorMode24bit;
    } else if (src.fgColor >= kPackedColorDefault && src.fgColor <= kPackedColorSystemMessage) {
        dst.foregroundColorMode = ColorModeAlternate;
        dst.foregroundColor = src.fgColor - kPackedColorDefault;
    } else {
        dst.foregroundColorMode = ColorModeNormal;
        dst.foregroundColor = src.fgColor & 0xFF;
    }

    // Background color
    if (src.bgIs24Bit && colorTable) {
        int idx = src.bgColor - kPackedColor24BitBase;
        uint8_t r, g, b;
        [colorTable getColorAtIndex:idx red:&r green:&g blue:&b];
        dst.backgroundColor = r;
        dst.bgGreen = g;
        dst.bgBlue = b;
        dst.backgroundColorMode = ColorMode24bit;
    } else if (src.bgColor >= kPackedColorDefault && src.bgColor <= kPackedColorSystemMessage) {
        dst.backgroundColorMode = ColorModeAlternate;
        dst.backgroundColor = src.bgColor - kPackedColorDefault;
    } else {
        dst.backgroundColorMode = ColorModeNormal;
        dst.backgroundColor = src.bgColor & 0xFF;
    }

    return dst;
}

// Baseline array function (simple loop, no optimizations)
static void PackArrayBaseline(const test_screen_char_t *src,
                              test_packed_char_t *dst,
                              NSUInteger count,
                              TestColorTable *colorTable) {
    for (NSUInteger i = 0; i < count; i++) {
        dst[i] = PackCharBaseline(src[i], colorTable);
    }
}

static void UnpackArrayBaseline(const test_packed_char_t *src,
                                test_screen_char_t *dst,
                                NSUInteger count,
                                TestColorTable *colorTable) {
    for (NSUInteger i = 0; i < count; i++) {
        dst[i] = UnpackCharBaseline(src[i], colorTable);
    }
}

#pragma mark - Optimized Pack/Unpack (Loop Unrolling + Fast Path)

// Fast pack: no 24-bit color handling
static inline test_packed_char_t PackCharFast(test_screen_char_t src) {
    test_packed_char_t dst = {0};

    dst.code = src.code & 0x1FFFFF;
    dst.complexChar = src.complexChar;
    dst.bold = src.bold;
    dst.faint = src.faint;
    dst.italic = src.italic;
    dst.blink = src.blink;
    dst.underline = src.underline;
    dst.image = src.image;
    dst.strikethrough = src.strikethrough;
    dst.invisible = src.invisible;
    dst.inverse = src.inverse;
    dst.guarded = src.guarded;
    dst.virtualPlaceholder = src.virtualPlaceholder;
    dst.fgIs24Bit = 0;
    dst.bgIs24Bit = 0;

    // Fast color handling (no 24-bit)
    if (src.foregroundColorMode == ColorModeAlternate) {
        dst.fgColor = kPackedColorDefault + src.foregroundColor;
    } else {
        dst.fgColor = src.foregroundColor & 0xFF;
    }

    if (src.backgroundColorMode == ColorModeAlternate) {
        dst.bgColor = kPackedColorDefault + src.backgroundColor;
    } else {
        dst.bgColor = src.backgroundColor & 0xFF;
    }

    return dst;
}

// Fast unpack: no 24-bit color handling
static inline test_screen_char_t UnpackCharFast(test_packed_char_t src) {
    test_screen_char_t dst = {0};

    dst.code = src.code;
    dst.complexChar = src.complexChar;
    dst.bold = src.bold;
    dst.faint = src.faint;
    dst.italic = src.italic;
    dst.blink = src.blink;
    dst.underline = src.underline;
    dst.image = src.image;
    dst.strikethrough = src.strikethrough;
    dst.invisible = src.invisible;
    dst.inverse = src.inverse;
    dst.guarded = src.guarded;
    dst.virtualPlaceholder = src.virtualPlaceholder;

    if (src.fgColor >= kPackedColorDefault && src.fgColor <= kPackedColorSystemMessage) {
        dst.foregroundColorMode = ColorModeAlternate;
        dst.foregroundColor = src.fgColor - kPackedColorDefault;
    } else {
        dst.foregroundColorMode = ColorModeNormal;
        dst.foregroundColor = src.fgColor & 0xFF;
    }

    if (src.bgColor >= kPackedColorDefault && src.bgColor <= kPackedColorSystemMessage) {
        dst.backgroundColorMode = ColorModeAlternate;
        dst.backgroundColor = src.bgColor - kPackedColorDefault;
    } else {
        dst.backgroundColorMode = ColorModeNormal;
        dst.backgroundColor = src.bgColor & 0xFF;
    }

    return dst;
}

// Optimized array pack with loop unrolling and fast-path
static void PackArrayOptimized(const test_screen_char_t *src,
                               test_packed_char_t *dst,
                               NSUInteger count,
                               TestColorTable *colorTable) {
    if (count == 0) return;

    const NSUInteger kPrefetchDistance = 8;
    const NSUInteger kUnrollFactor = 4;
    const NSUInteger mainLoopEnd = count - (count % kUnrollFactor);

    NSUInteger i = 0;

    while (i < mainLoopEnd) {
        // Prefetch
        if (i + kPrefetchDistance < count) {
            __builtin_prefetch(&src[i + kPrefetchDistance], 0, 3);
        }

        // Check if any of 4 chars need 24-bit
        BOOL needs24Bit = CharNeeds24Bit(src[i]) ||
                          CharNeeds24Bit(src[i + 1]) ||
                          CharNeeds24Bit(src[i + 2]) ||
                          CharNeeds24Bit(src[i + 3]);

        if (__builtin_expect(needs24Bit, 0)) {
            // Slow path
            dst[i] = PackCharBaseline(src[i], colorTable);
            dst[i + 1] = PackCharBaseline(src[i + 1], colorTable);
            dst[i + 2] = PackCharBaseline(src[i + 2], colorTable);
            dst[i + 3] = PackCharBaseline(src[i + 3], colorTable);
        } else {
            // Fast path (unrolled)
            dst[i] = PackCharFast(src[i]);
            dst[i + 1] = PackCharFast(src[i + 1]);
            dst[i + 2] = PackCharFast(src[i + 2]);
            dst[i + 3] = PackCharFast(src[i + 3]);
        }

        i += kUnrollFactor;
    }

    // Remainder
    while (i < count) {
        if (__builtin_expect(CharNeeds24Bit(src[i]), 0)) {
            dst[i] = PackCharBaseline(src[i], colorTable);
        } else {
            dst[i] = PackCharFast(src[i]);
        }
        i++;
    }
}

// Optimized array unpack with loop unrolling and fast-path
static void UnpackArrayOptimized(const test_packed_char_t *src,
                                 test_screen_char_t *dst,
                                 NSUInteger count,
                                 TestColorTable *colorTable) {
    if (count == 0) return;

    const NSUInteger kPrefetchDistance = 16;
    const NSUInteger kUnrollFactor = 4;
    const NSUInteger mainLoopEnd = count - (count % kUnrollFactor);

    NSUInteger i = 0;

    while (i < mainLoopEnd) {
        // Prefetch
        if (i + kPrefetchDistance < count) {
            __builtin_prefetch(&src[i + kPrefetchDistance], 0, 3);
        }

        // Check if any of 4 chars need 24-bit
        BOOL needs24Bit = PackedCharNeeds24Bit(src[i]) ||
                          PackedCharNeeds24Bit(src[i + 1]) ||
                          PackedCharNeeds24Bit(src[i + 2]) ||
                          PackedCharNeeds24Bit(src[i + 3]);

        if (__builtin_expect(needs24Bit, 0)) {
            // Slow path
            dst[i] = UnpackCharBaseline(src[i], colorTable);
            dst[i + 1] = UnpackCharBaseline(src[i + 1], colorTable);
            dst[i + 2] = UnpackCharBaseline(src[i + 2], colorTable);
            dst[i + 3] = UnpackCharBaseline(src[i + 3], colorTable);
        } else {
            // Fast path (unrolled)
            dst[i] = UnpackCharFast(src[i]);
            dst[i + 1] = UnpackCharFast(src[i + 1]);
            dst[i + 2] = UnpackCharFast(src[i + 2]);
            dst[i + 3] = UnpackCharFast(src[i + 3]);
        }

        i += kUnrollFactor;
    }

    // Remainder
    while (i < count) {
        if (__builtin_expect(PackedCharNeeds24Bit(src[i]), 0)) {
            dst[i] = UnpackCharBaseline(src[i], colorTable);
        } else {
            dst[i] = UnpackCharFast(src[i]);
        }
        i++;
    }
}

#pragma mark - Timing

static mach_timebase_info_data_t g_timebaseInfo;

static void InitTiming(void) {
    if (g_timebaseInfo.denom == 0) {
        mach_timebase_info(&g_timebaseInfo);
    }
}

static double ElapsedNs(uint64_t start, uint64_t end) {
    return (double)(end - start) * g_timebaseInfo.numer / g_timebaseInfo.denom;
}

#pragma mark - Test Data Generation

static void GenerateTestData(test_screen_char_t *buffer, int count, double percent24Bit) {
    for (int i = 0; i < count; i++) {
        test_screen_char_t c = {0};
        c.code = 32 + (i % 95);

        BOOL use24Bit = ((i * 17) % 100) < (percent24Bit * 100);

        if (use24Bit) {
            c.foregroundColor = (i * 3) % 256;
            c.fgGreen = (i * 7) % 256;
            c.fgBlue = (i * 13) % 256;
            c.foregroundColorMode = ColorMode24bit;
            c.backgroundColor = (i * 19) % 256;
            c.bgGreen = (i * 29) % 256;
            c.bgBlue = (i * 37) % 256;
            c.backgroundColorMode = ColorMode24bit;
        } else {
            c.foregroundColor = i % 16;
            c.foregroundColorMode = ColorModeNormal;
            c.backgroundColor = i % 8;
            c.backgroundColorMode = ColorModeNormal;
        }

        if (i % 20 == 0) c.bold = 1;
        if (i % 30 == 0) c.italic = 1;
        if (i % 40 == 0) c.underline = 1;

        buffer[i] = c;
    }
}

#pragma mark - Benchmark

typedef struct {
    const char *name;
    double packNsPerChar;
    double unpackNsPerChar;
    double packGBps;
    double unpackGBps;
    BOOL roundTripPassed;
} BenchmarkResult;

static BenchmarkResult RunBenchmark(const char *name,
                                    void (*packFunc)(const test_screen_char_t*, test_packed_char_t*, NSUInteger, TestColorTable*),
                                    void (*unpackFunc)(const test_packed_char_t*, test_screen_char_t*, NSUInteger, TestColorTable*),
                                    int numChars,
                                    double percent24Bit) {
    BenchmarkResult result = {0};
    result.name = name;

    test_screen_char_t *original = calloc(numChars, sizeof(test_screen_char_t));
    test_packed_char_t *packed = calloc(numChars, sizeof(test_packed_char_t));
    test_screen_char_t *unpacked = calloc(numChars, sizeof(test_screen_char_t));

    GenerateTestData(original, numChars, percent24Bit);

    TestColorTable *colorTable = [[TestColorTable alloc] init];

    // Warm up
    packFunc(original, packed, numChars, colorTable);
    unpackFunc(packed, unpacked, numChars, colorTable);

    // Reset
    memset(packed, 0, numChars * sizeof(test_packed_char_t));
    memset(unpacked, 0, numChars * sizeof(test_screen_char_t));
    colorTable = [[TestColorTable alloc] init];

    // Benchmark pack (3 runs, take median)
    double packTimes[3];
    for (int run = 0; run < 3; run++) {
        colorTable = [[TestColorTable alloc] init];
        memset(packed, 0, numChars * sizeof(test_packed_char_t));

        uint64_t start = mach_absolute_time();
        packFunc(original, packed, numChars, colorTable);
        uint64_t end = mach_absolute_time();
        packTimes[run] = ElapsedNs(start, end);
    }
    // Sort and take median
    for (int i = 0; i < 2; i++) {
        for (int j = i + 1; j < 3; j++) {
            if (packTimes[i] > packTimes[j]) {
                double tmp = packTimes[i];
                packTimes[i] = packTimes[j];
                packTimes[j] = tmp;
            }
        }
    }
    double packTimeNs = packTimes[1];  // Median

    // Benchmark unpack (3 runs, take median)
    double unpackTimes[3];
    for (int run = 0; run < 3; run++) {
        memset(unpacked, 0, numChars * sizeof(test_screen_char_t));

        uint64_t start = mach_absolute_time();
        unpackFunc(packed, unpacked, numChars, colorTable);
        uint64_t end = mach_absolute_time();
        unpackTimes[run] = ElapsedNs(start, end);
    }
    for (int i = 0; i < 2; i++) {
        for (int j = i + 1; j < 3; j++) {
            if (unpackTimes[i] > unpackTimes[j]) {
                double tmp = unpackTimes[i];
                unpackTimes[i] = unpackTimes[j];
                unpackTimes[j] = tmp;
            }
        }
    }
    double unpackTimeNs = unpackTimes[1];  // Median

    // Calculate results
    result.packNsPerChar = packTimeNs / numChars;
    result.unpackNsPerChar = unpackTimeNs / numChars;

    double packBytes = numChars * sizeof(test_screen_char_t);
    double unpackBytes = numChars * sizeof(test_packed_char_t);
    result.packGBps = (packBytes / (packTimeNs / 1e9)) / 1e9;
    result.unpackGBps = (unpackBytes / (unpackTimeNs / 1e9)) / 1e9;

    // Verify round-trip (spot check)
    result.roundTripPassed = YES;
    for (int i = 0; i < numChars && result.roundTripPassed; i += 1000) {
        if (original[i].code != unpacked[i].code ||
            original[i].bold != unpacked[i].bold) {
            result.roundTripPassed = NO;
        }
    }

    free(original);
    free(packed);
    free(unpacked);

    return result;
}

#pragma mark - Main

int main(int argc, const char * argv[]) {
    @autoreleasepool {
        InitTiming();

        printf("\n");
        printf("================================================================\n");
        printf("  Pack/Unpack Micro-Benchmark: Baseline vs Optimized\n");
        printf("================================================================\n\n");

        printf("Structure sizes:\n");
        printf("  test_screen_char_t:  %lu bytes\n", sizeof(test_screen_char_t));
        printf("  test_packed_char_t:  %lu bytes\n", sizeof(test_packed_char_t));
        printf("  Memory reduction:    %.1f%%\n\n",
               100.0 * (1.0 - (double)sizeof(test_packed_char_t) / sizeof(test_screen_char_t)));

        struct {
            int numChars;
            double percent24Bit;
            const char *desc;
        } tests[] = {
            {8000000, 0.0, "Fast path (0% 24-bit)"},
            {8000000, 0.05, "Mixed (5% 24-bit)"},
            {8000000, 0.20, "Mixed (20% 24-bit)"},
            {8000000, 1.0, "Slow path (100% 24-bit)"},
        };
        int numTests = sizeof(tests) / sizeof(tests[0]);

        NSMutableArray *jsonResults = [NSMutableArray array];

        for (int t = 0; t < numTests; t++) {
            printf("Test: %s (%d chars)\n", tests[t].desc, tests[t].numChars);
            printf("--------------------------------------------------------------\n");
            printf("  %-20s | %8s | %8s | %6s | %6s | %s\n",
                   "Method", "Pack", "Unpack", "Pack", "Unpack", "OK");
            printf("  %-20s | %8s | %8s | %6s | %6s |\n",
                   "", "(ns/ch)", "(ns/ch)", "(GB/s)", "(GB/s)");
            printf("  ------------------------------------------------------\n");

            BenchmarkResult baseline = RunBenchmark(
                "Baseline (simple)",
                PackArrayBaseline,
                UnpackArrayBaseline,
                tests[t].numChars,
                tests[t].percent24Bit
            );
            printf("  %-20s | %8.2f | %8.2f | %6.2f | %6.2f | %s\n",
                   baseline.name,
                   baseline.packNsPerChar,
                   baseline.unpackNsPerChar,
                   baseline.packGBps,
                   baseline.unpackGBps,
                   baseline.roundTripPassed ? "PASS" : "FAIL");

            BenchmarkResult optimized = RunBenchmark(
                "Optimized (unroll)",
                PackArrayOptimized,
                UnpackArrayOptimized,
                tests[t].numChars,
                tests[t].percent24Bit
            );
            printf("  %-20s | %8.2f | %8.2f | %6.2f | %6.2f | %s\n",
                   optimized.name,
                   optimized.packNsPerChar,
                   optimized.unpackNsPerChar,
                   optimized.packGBps,
                   optimized.unpackGBps,
                   optimized.roundTripPassed ? "PASS" : "FAIL");

            double packSpeedup = baseline.packNsPerChar / optimized.packNsPerChar;
            double unpackSpeedup = baseline.unpackNsPerChar / optimized.unpackNsPerChar;

            printf("\n  Speedup: %.2fx pack, %.2fx unpack\n\n",
                   packSpeedup, unpackSpeedup);

            NSDictionary *testResult = @{
                @"test": @(tests[t].desc),
                @"num_chars": @(tests[t].numChars),
                @"percent_24bit": @(tests[t].percent24Bit),
                @"baseline": @{
                    @"pack_ns_per_char": @(baseline.packNsPerChar),
                    @"unpack_ns_per_char": @(baseline.unpackNsPerChar),
                    @"pack_gbps": @(baseline.packGBps),
                    @"unpack_gbps": @(baseline.unpackGBps)
                },
                @"optimized": @{
                    @"pack_ns_per_char": @(optimized.packNsPerChar),
                    @"unpack_ns_per_char": @(optimized.unpackNsPerChar),
                    @"pack_gbps": @(optimized.packGBps),
                    @"unpack_gbps": @(optimized.unpackGBps)
                },
                @"speedup": @{
                    @"pack": @(packSpeedup),
                    @"unpack": @(unpackSpeedup)
                }
            };
            [jsonResults addObject:testResult];
        }

        // Summary
        printf("================================================================\n");
        printf("  SUMMARY\n");
        printf("================================================================\n\n");

        // Fast-path summary
        BenchmarkResult fastBase = RunBenchmark("baseline", PackArrayBaseline, UnpackArrayBaseline, 8000000, 0.0);
        BenchmarkResult fastOpt = RunBenchmark("optimized", PackArrayOptimized, UnpackArrayOptimized, 8000000, 0.0);

        printf("Fast-path performance (typical terminal content):\n");
        printf("  Baseline pack:    %.2f ns/char (%.2f GB/s)\n",
               fastBase.packNsPerChar, fastBase.packGBps);
        printf("  Optimized pack:   %.2f ns/char (%.2f GB/s)\n",
               fastOpt.packNsPerChar, fastOpt.packGBps);
        printf("  Pack speedup:     %.2fx\n\n",
               fastBase.packNsPerChar / fastOpt.packNsPerChar);

        printf("  Baseline unpack:  %.2f ns/char (%.2f GB/s)\n",
               fastBase.unpackNsPerChar, fastBase.unpackGBps);
        printf("  Optimized unpack: %.2f ns/char (%.2f GB/s)\n",
               fastOpt.unpackNsPerChar, fastOpt.unpackGBps);
        printf("  Unpack speedup:   %.2fx\n\n",
               fastBase.unpackNsPerChar / fastOpt.unpackNsPerChar);

        // Real-world impact
        printf("Real-world impact (1M lines @ 80 cols = 80M chars):\n");
        double charsPerLine = 80;
        double lines = 1000000;
        double totalChars = charsPerLine * lines;
        printf("  Baseline pack time:   %.1f ms\n", (fastBase.packNsPerChar * totalChars) / 1e6);
        printf("  Optimized pack time:  %.1f ms\n", (fastOpt.packNsPerChar * totalChars) / 1e6);
        printf("  Time saved:           %.1f ms\n\n",
               ((fastBase.packNsPerChar - fastOpt.packNsPerChar) * totalChars) / 1e6);

        // Save JSON
        NSDictionary *fullReport = @{
            @"date": [[NSDate date] description],
            @"test": @"Pack/Unpack Micro-Benchmark",
            @"implementation": @"Loop unrolling + fast-path optimizations",
            @"struct_sizes": @{
                @"screen_char_t": @(sizeof(test_screen_char_t)),
                @"packed_char_t": @(sizeof(test_packed_char_t)),
                @"reduction_percent": @(100.0 * (1.0 - (double)sizeof(test_packed_char_t) / sizeof(test_screen_char_t)))
            },
            @"summary": @{
                @"baseline_pack_ns": @(fastBase.packNsPerChar),
                @"optimized_pack_ns": @(fastOpt.packNsPerChar),
                @"pack_speedup": @(fastBase.packNsPerChar / fastOpt.packNsPerChar),
                @"baseline_unpack_ns": @(fastBase.unpackNsPerChar),
                @"optimized_unpack_ns": @(fastOpt.unpackNsPerChar),
                @"unpack_speedup": @(fastBase.unpackNsPerChar / fastOpt.unpackNsPerChar)
            },
            @"tests": jsonResults
        };

        NSError *error = nil;
        NSData *jsonData = [NSJSONSerialization dataWithJSONObject:fullReport
                                                          options:NSJSONWritingPrettyPrinted
                                                            error:&error];
        if (jsonData) {
            NSString *jsonString = [[NSString alloc] initWithData:jsonData encoding:NSUTF8StringEncoding];
            [jsonString writeToFile:@"benchmarks/baselines/pack_unpack_microbenchmark.json"
                         atomically:YES
                           encoding:NSUTF8StringEncoding
                              error:&error];
            if (!error) {
                printf("Results saved to benchmarks/baselines/pack_unpack_microbenchmark.json\n");
            }
        }

        return 0;
    }
}
