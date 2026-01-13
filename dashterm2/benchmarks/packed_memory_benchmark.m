/*
 * Packed Memory Efficiency Benchmark (Standalone)
 *
 * This is a standalone benchmark that verifies the memory savings from
 * using packed screen_char_t storage. It duplicates minimal definitions
 * to avoid complex header dependencies.
 *
 * Compile and run:
 *   clang -framework Foundation -O2 -o Benchmarks/packed_memory_benchmark \
 *     Benchmarks/packed_memory_benchmark.m -fobjc-arc
 *   ./Benchmarks/packed_memory_benchmark
 *
 * Expected results:
 *   - 33% memory reduction (12 bytes -> 8 bytes per character)
 *   - 1M line scrollback: ~305 MB savings
 *   - Pack/unpack: ~2-4 μs per line
 */

#import <Foundation/Foundation.h>
#import <mach/mach_time.h>
#import <stdint.h>

#pragma mark - Minimal screen_char_t Definition

// Simplified screen_char_t - approximates the 12-byte screen_char_t from ScreenChar.h
// The real screen_char_t is 12 bytes with careful packing. We simulate it here.
typedef struct __attribute__((packed)) {
    uint32_t code;              // 4 bytes - Unicode code point
    uint8_t foregroundColor;    // 1 byte
    uint8_t fgGreen;            // 1 byte
    uint8_t fgBlue;             // 1 byte
    uint8_t backgroundColor;    // 1 byte
    uint8_t bgGreen;            // 1 byte
    uint8_t bgBlue;             // 1 byte
    uint8_t modes;              // 1 byte - fg/bg color modes packed
    uint8_t flags;              // 1 byte - bold, italic, etc packed
} mini_screen_char_t;

// 12 bytes total - matches real screen_char_t
_Static_assert(sizeof(mini_screen_char_t) == 12, "mini_screen_char_t must be 12 bytes");

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
    uint64_t fgIs24Bit : 1;
    uint64_t bgIs24Bit : 1;
    uint64_t reserved : 15;
} mini_packed_char_t;

_Static_assert(sizeof(mini_packed_char_t) == 8, "mini_packed_char_t must be 8 bytes");

#pragma mark - Color Table (simplified)

#define MAX_24BIT_COLORS 251
#define COLOR_24BIT_BASE 261

@interface MiniColorTable : NSObject {
    uint32_t _colors[MAX_24BIT_COLORS];  // RGB packed
    int _count;
}
- (int)insertRed:(uint8_t)r green:(uint8_t)g blue:(uint8_t)b;
- (void)getColorAtIndex:(int)idx red:(uint8_t*)r green:(uint8_t*)g blue:(uint8_t*)b;
@end

@implementation MiniColorTable

- (instancetype)init {
    self = [super init];
    if (self) {
        _count = 0;
        memset(_colors, 0, sizeof(_colors));
    }
    return self;
}

- (int)insertRed:(uint8_t)r green:(uint8_t)g blue:(uint8_t)b {
    uint32_t packed = (r << 16) | (g << 8) | b;

    // Search for existing
    for (int i = 0; i < _count; i++) {
        if (_colors[i] == packed) {
            return i;
        }
    }

    // Add new if space
    if (_count < MAX_24BIT_COLORS) {
        _colors[_count] = packed;
        return _count++;
    }

    // Table full - return 0 (quantize to first entry)
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

#pragma mark - Pack/Unpack Functions

// Extract mode bits from packed modes byte
#define FG_MODE(m) ((m) & 0x03)
#define BG_MODE(m) (((m) >> 2) & 0x03)
#define MAKE_MODES(fg, bg) (((fg) & 0x03) | (((bg) & 0x03) << 2))

// Extract flag bits from packed flags byte
#define FLAG_COMPLEX  0x01
#define FLAG_BOLD     0x02
#define FLAG_FAINT    0x04
#define FLAG_ITALIC   0x08
#define FLAG_BLINK    0x10
#define FLAG_ULINE    0x20
#define FLAG_STRIKE   0x40
#define FLAG_IMAGE    0x80

static mini_packed_char_t packChar(mini_screen_char_t src, MiniColorTable *colorTable) {
    mini_packed_char_t dst = {0};

    dst.code = src.code & 0x1FFFFF;  // 21 bits
    dst.complexChar = (src.flags & FLAG_COMPLEX) ? 1 : 0;
    dst.bold = (src.flags & FLAG_BOLD) ? 1 : 0;
    dst.faint = (src.flags & FLAG_FAINT) ? 1 : 0;
    dst.italic = (src.flags & FLAG_ITALIC) ? 1 : 0;
    dst.blink = (src.flags & FLAG_BLINK) ? 1 : 0;
    dst.underline = (src.flags & FLAG_ULINE) ? 1 : 0;
    dst.strikethrough = (src.flags & FLAG_STRIKE) ? 1 : 0;
    dst.image = (src.flags & FLAG_IMAGE) ? 1 : 0;

    // Handle foreground color
    if (FG_MODE(src.modes) == 2) {  // 24-bit
        int idx = [colorTable insertRed:src.foregroundColor
                                  green:src.fgGreen
                                   blue:src.fgBlue];
        dst.fgColor = COLOR_24BIT_BASE + idx;
        dst.fgIs24Bit = 1;
    } else {
        dst.fgColor = src.foregroundColor & 0x1FF;
    }

    // Handle background color
    if (BG_MODE(src.modes) == 2) {  // 24-bit
        int idx = [colorTable insertRed:src.backgroundColor
                                  green:src.bgGreen
                                   blue:src.bgBlue];
        dst.bgColor = COLOR_24BIT_BASE + idx;
        dst.bgIs24Bit = 1;
    } else {
        dst.bgColor = src.backgroundColor & 0x1FF;
    }

    return dst;
}

static mini_screen_char_t unpackChar(mini_packed_char_t src, MiniColorTable *colorTable) {
    mini_screen_char_t dst = {0};

    dst.code = src.code;

    // Pack flags into flags byte
    uint8_t flags = 0;
    if (src.complexChar) flags |= FLAG_COMPLEX;
    if (src.bold) flags |= FLAG_BOLD;
    if (src.faint) flags |= FLAG_FAINT;
    if (src.italic) flags |= FLAG_ITALIC;
    if (src.blink) flags |= FLAG_BLINK;
    if (src.underline) flags |= FLAG_ULINE;
    if (src.strikethrough) flags |= FLAG_STRIKE;
    if (src.image) flags |= FLAG_IMAGE;
    dst.flags = flags;

    uint8_t fgMode = 0, bgMode = 0;

    // Handle foreground color
    if (src.fgIs24Bit) {
        int idx = src.fgColor - COLOR_24BIT_BASE;
        uint8_t r, g, b;
        [colorTable getColorAtIndex:idx red:&r green:&g blue:&b];
        dst.foregroundColor = r;
        dst.fgGreen = g;
        dst.fgBlue = b;
        fgMode = 2;  // 24-bit
    } else {
        dst.foregroundColor = src.fgColor & 0xFF;
        fgMode = (src.fgColor > 255) ? 1 : 0;
    }

    // Handle background color
    if (src.bgIs24Bit) {
        int idx = src.bgColor - COLOR_24BIT_BASE;
        uint8_t r, g, b;
        [colorTable getColorAtIndex:idx red:&r green:&g blue:&b];
        dst.backgroundColor = r;
        dst.bgGreen = g;
        dst.bgBlue = b;
        bgMode = 2;
    } else {
        dst.backgroundColor = src.bgColor & 0xFF;
        bgMode = (src.bgColor > 255) ? 1 : 0;
    }

    dst.modes = MAKE_MODES(fgMode, bgMode);

    return dst;
}

#pragma mark - Timing

static mach_timebase_info_data_t timebaseInfo;

static void initTiming(void) {
    if (timebaseInfo.denom == 0) {
        mach_timebase_info(&timebaseInfo);
    }
}

static double elapsedNs(uint64_t start, uint64_t end) {
    return (double)(end - start) * timebaseInfo.numer / timebaseInfo.denom;
}

#pragma mark - Test Generation

static void generateTestLine(mini_screen_char_t *buffer, int length, int lineNumber) {
    for (int i = 0; i < length; i++) {
        mini_screen_char_t c = {0};
        c.code = 32 + ((lineNumber * 7 + i * 13) % 95);
        c.foregroundColor = (lineNumber + i) % 16;
        c.backgroundColor = (lineNumber * 3 + i) % 8;
        c.modes = 0;  // Normal color mode

        uint8_t flags = 0;
        if (i % 20 == 0) flags |= FLAG_BOLD;
        if (i % 30 == 0) flags |= FLAG_ITALIC;
        if (i % 40 == 0) flags |= FLAG_ULINE;
        c.flags = flags;

        buffer[i] = c;
    }
}

#pragma mark - Benchmark

typedef struct {
    NSUInteger unpackedBytes;
    NSUInteger packedBytes;
    double packTimeNs;
    double unpackTimeNs;
    BOOL roundTripPassed;
} BenchmarkResult;

static BenchmarkResult runBenchmark(int numLines, int lineLength) {
    BenchmarkResult result = {0};
    int totalChars = numLines * lineLength;

    // Allocate buffers
    mini_screen_char_t *original = calloc(totalChars, sizeof(mini_screen_char_t));
    mini_packed_char_t *packed = calloc(totalChars, sizeof(mini_packed_char_t));
    mini_screen_char_t *unpacked = calloc(totalChars, sizeof(mini_screen_char_t));

    // Generate test data
    for (int line = 0; line < numLines; line++) {
        generateTestLine(original + line * lineLength, lineLength, line);
    }

    MiniColorTable *colorTable = [[MiniColorTable alloc] init];

    // Pack
    uint64_t startPack = mach_absolute_time();
    for (int i = 0; i < totalChars; i++) {
        packed[i] = packChar(original[i], colorTable);
    }
    uint64_t endPack = mach_absolute_time();

    // Unpack
    uint64_t startUnpack = mach_absolute_time();
    for (int i = 0; i < totalChars; i++) {
        unpacked[i] = unpackChar(packed[i], colorTable);
    }
    uint64_t endUnpack = mach_absolute_time();

    // Calculate results
    result.unpackedBytes = totalChars * sizeof(mini_screen_char_t);
    result.packedBytes = totalChars * sizeof(mini_packed_char_t);
    result.packTimeNs = elapsedNs(startPack, endPack);
    result.unpackTimeNs = elapsedNs(startUnpack, endUnpack);

    // Verify round-trip
    result.roundTripPassed = YES;
    for (int i = 0; i < totalChars && result.roundTripPassed; i++) {
        if (original[i].code != unpacked[i].code ||
            original[i].flags != unpacked[i].flags ||
            original[i].foregroundColor != unpacked[i].foregroundColor ||
            original[i].backgroundColor != unpacked[i].backgroundColor) {
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
        initTiming();

        printf("\n");
        printf("===================================================\n");
        printf("   Packed Character Memory Efficiency Benchmark\n");
        printf("===================================================\n\n");

        printf("Structure sizes:\n");
        printf("  mini_screen_char_t:  %lu bytes\n", sizeof(mini_screen_char_t));
        printf("  mini_packed_char_t:  %lu bytes\n", sizeof(mini_packed_char_t));
        printf("  Memory reduction:    %.1f%%\n\n",
               100.0 * (1.0 - (double)sizeof(mini_packed_char_t) / sizeof(mini_screen_char_t)));

        // Test configurations
        struct {
            int numLines;
            int lineLength;
            const char *name;
        } configs[] = {
            {1000, 80, "1K lines @ 80 cols"},
            {10000, 80, "10K lines @ 80 cols"},
            {100000, 80, "100K lines @ 80 cols"},
            {1000000, 80, "1M lines @ 80 cols"},
            {1000000, 132, "1M lines @ 132 cols"},
        };

        int numConfigs = sizeof(configs) / sizeof(configs[0]);

        printf("%-25s %12s %12s %8s %10s %10s\n",
               "Configuration", "Unpacked", "Packed", "Savings", "Pack", "Unpack");
        printf("%-25s %12s %12s %8s %10s %10s\n",
               "", "(MB)", "(MB)", "(%)", "(ms)", "(ms)");
        printf("------------------------------------------------------------------------\n");

        for (int i = 0; i < numConfigs; i++) {
            BenchmarkResult result = runBenchmark(configs[i].numLines, configs[i].lineLength);

            double savingsPercent = 100.0 * (1.0 - (double)result.packedBytes / result.unpackedBytes);

            printf("%-25s %12.1f %12.1f %7.1f%% %9.1f %9.1f %s\n",
                   configs[i].name,
                   result.unpackedBytes / 1024.0 / 1024.0,
                   result.packedBytes / 1024.0 / 1024.0,
                   savingsPercent,
                   result.packTimeNs / 1000000.0,
                   result.unpackTimeNs / 1000000.0,
                   result.roundTripPassed ? "✓" : "✗");
        }

        printf("\n");

        // Summary for 1M lines
        BenchmarkResult summary = runBenchmark(1000000, 80);

        printf("Summary (1M lines @ 80 cols - typical large scrollback):\n");
        printf("  Memory saved:     %.1f MB\n",
               (summary.unpackedBytes - summary.packedBytes) / 1024.0 / 1024.0);
        printf("  Pack throughput:  %.1f μs/line (%.1f GB/s)\n",
               summary.packTimeNs / 1000.0 / 1000000.0,
               (summary.unpackedBytes / (summary.packTimeNs / 1000000000.0)) / 1024.0 / 1024.0 / 1024.0);
        printf("  Unpack throughput: %.1f μs/line (%.1f GB/s)\n",
               summary.unpackTimeNs / 1000.0 / 1000000.0,
               (summary.unpackedBytes / (summary.unpackTimeNs / 1000000000.0)) / 1024.0 / 1024.0 / 1024.0);
        printf("  Round-trip:       %s\n\n", summary.roundTripPassed ? "PASS" : "FAIL");

        // Save baseline
        NSString *baselineJson = [NSString stringWithFormat:
            @"{\n"
            @"  \"date\": \"%@\",\n"
            @"  \"test\": \"Packed Character Memory Benchmark\",\n"
            @"  \"struct_sizes\": {\n"
            @"    \"unpacked_bytes\": %lu,\n"
            @"    \"packed_bytes\": %lu,\n"
            @"    \"reduction_percent\": %.1f\n"
            @"  },\n"
            @"  \"million_line_scrollback\": {\n"
            @"    \"unpacked_mb\": %.2f,\n"
            @"    \"packed_mb\": %.2f,\n"
            @"    \"savings_mb\": %.2f,\n"
            @"    \"pack_ms\": %.2f,\n"
            @"    \"unpack_ms\": %.2f,\n"
            @"    \"pack_us_per_line\": %.2f,\n"
            @"    \"unpack_us_per_line\": %.2f,\n"
            @"    \"round_trip_passed\": %@\n"
            @"  }\n"
            @"}\n",
            [[NSDate date] description],
            sizeof(mini_screen_char_t),
            sizeof(mini_packed_char_t),
            100.0 * (1.0 - (double)sizeof(mini_packed_char_t) / sizeof(mini_screen_char_t)),
            summary.unpackedBytes / 1024.0 / 1024.0,
            summary.packedBytes / 1024.0 / 1024.0,
            (summary.unpackedBytes - summary.packedBytes) / 1024.0 / 1024.0,
            summary.packTimeNs / 1000000.0,
            summary.unpackTimeNs / 1000000.0,
            summary.packTimeNs / 1000.0 / 1000000.0,
            summary.unpackTimeNs / 1000.0 / 1000000.0,
            summary.roundTripPassed ? @"true" : @"false"
        ];

        NSError *error = nil;
        [baselineJson writeToFile:@"Benchmarks/baselines/packed_memory_baseline.json"
                       atomically:YES
                         encoding:NSUTF8StringEncoding
                            error:&error];

        if (error) {
            NSLog(@"Error writing baseline: %@", error);
        } else {
            printf("Baseline saved to Benchmarks/baselines/packed_memory_baseline.json\n");
        }

        return summary.roundTripPassed ? 0 : 1;
    }
}
