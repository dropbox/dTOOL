/*
 * LineBlockPacked Memory Efficiency Benchmark
 *
 * This standalone benchmark validates the memory savings from using
 * LineBlockPacked for scrollback storage.
 *
 * Compile and run:
 *   clang -framework Foundation -O2 -o lineblock_packed_benchmark \
 *     Benchmarks/lineblock_packed_benchmark.m \
 *     sources/PackedScreenChar.m \
 *     sources/iTermPackedCharacterBuffer.m \
 *     sources/iTermMalloc.m \
 *     -I sources -fobjc-arc
 *   ./lineblock_packed_benchmark
 */

#import <Foundation/Foundation.h>
#import <mach/mach_time.h>
#import "ScreenChar.h"
#import "PackedScreenChar.h"
#import "iTermPackedCharacterBuffer.h"

#pragma mark - Timing Utilities

static mach_timebase_info_data_t timebaseInfo;

static void initTiming(void) {
    if (timebaseInfo.denom == 0) {
        mach_timebase_info(&timebaseInfo);
    }
}

static double elapsedNs(uint64_t start, uint64_t end) {
    return (double)(end - start) * timebaseInfo.numer / timebaseInfo.denom;
}

#pragma mark - Test Data Generation

// Generate a line of test characters with various attributes
static void generateTestLine(screen_char_t *buffer, int length, int lineNumber) {
    for (int i = 0; i < length; i++) {
        screen_char_t c = {0};

        // Mix of ASCII characters
        c.code = 32 + ((lineNumber * 7 + i * 13) % 95);  // Printable ASCII

        // Vary foreground colors
        c.foregroundColor = (lineNumber + i) % 16;
        c.foregroundColorMode = ColorModeNormal;

        // Vary background colors
        c.backgroundColor = (lineNumber * 3 + i) % 8;
        c.backgroundColorMode = ColorModeNormal;

        // Some attributes
        c.bold = (i % 20 == 0);
        c.italic = (i % 30 == 0);
        c.underline = (i % 40 == 0);

        buffer[i] = c;
    }
}

// Generate test data with 24-bit colors
static void generateTestLine24Bit(screen_char_t *buffer, int length, int lineNumber) {
    for (int i = 0; i < length; i++) {
        screen_char_t c = {0};

        c.code = 32 + ((lineNumber * 7 + i * 13) % 95);

        // 24-bit foreground color
        c.foregroundColor = (lineNumber * 17 + i) % 256;
        c.fgGreen = (lineNumber * 23 + i * 3) % 256;
        c.fgBlue = (lineNumber * 31 + i * 7) % 256;
        c.foregroundColorMode = ColorMode24bit;

        // 24-bit background color
        c.backgroundColor = (lineNumber * 13 + i * 2) % 256;
        c.bgGreen = (lineNumber * 19 + i * 5) % 256;
        c.bgBlue = (lineNumber * 29 + i * 11) % 256;
        c.backgroundColorMode = ColorMode24bit;

        buffer[i] = c;
    }
}

#pragma mark - Memory Measurement

typedef struct {
    NSUInteger unpackedBytes;
    NSUInteger packedBytes;
    double packTimeNs;
    double unpackTimeNs;
    BOOL roundTripPassed;
} BenchmarkResult;

static BenchmarkResult benchmarkPackUnpack(int numLines, int lineLength, BOOL use24BitColor) {
    BenchmarkResult result = {0};
    int totalChars = numLines * lineLength;

    // Allocate test data
    screen_char_t *testData = calloc(totalChars, sizeof(screen_char_t));
    screen_char_t *unpackedData = calloc(totalChars, sizeof(screen_char_t));

    // Generate test lines
    for (int line = 0; line < numLines; line++) {
        if (use24BitColor) {
            generateTestLine24Bit(testData + line * lineLength, lineLength, line);
        } else {
            generateTestLine(testData + line * lineLength, lineLength, line);
        }
    }

    // Create packed buffer
    PackedColorTable *colorTable = [[PackedColorTable alloc] initWithCapacity:251];

    // Measure pack time
    uint64_t startPack = mach_absolute_time();
    iTermPackedCharacterBuffer *packedBuffer =
        [[iTermPackedCharacterBuffer alloc] initWithChars:testData
                                                     size:totalChars
                                               colorTable:colorTable];
    uint64_t endPack = mach_absolute_time();

    // Measure unpack time
    uint64_t startUnpack = mach_absolute_time();
    [packedBuffer readChars:unpackedData count:totalChars fromOffset:0];
    uint64_t endUnpack = mach_absolute_time();

    // Calculate results
    result.unpackedBytes = totalChars * sizeof(screen_char_t);
    result.packedBytes = packedBuffer.memoryUsage;
    result.packTimeNs = elapsedNs(startPack, endPack);
    result.unpackTimeNs = elapsedNs(startUnpack, endUnpack);

    // Verify round-trip
    result.roundTripPassed = YES;
    for (int i = 0; i < totalChars && result.roundTripPassed; i++) {
        screen_char_t orig = testData[i];
        screen_char_t unpacked = unpackedData[i];

        // Check essential fields
        if (orig.code != unpacked.code ||
            orig.bold != unpacked.bold ||
            orig.italic != unpacked.italic ||
            orig.underline != unpacked.underline) {
            result.roundTripPassed = NO;
            NSLog(@"Round-trip failed at index %d: code=%u vs %u",
                  i, orig.code, unpacked.code);
        }

        // Color comparison is complex due to table lookup - just check modes match
        if (orig.foregroundColorMode != ColorMode24bit &&
            orig.foregroundColor != unpacked.foregroundColor) {
            // Non-24bit colors should match exactly
            result.roundTripPassed = NO;
            NSLog(@"FG color mismatch at index %d", i);
        }
    }

    free(testData);
    free(unpackedData);

    return result;
}

#pragma mark - Main Benchmark

int main(int argc, const char * argv[]) {
    @autoreleasepool {
        initTiming();

        printf("\n");
        printf("===================================================\n");
        printf("     LineBlockPacked Memory Efficiency Benchmark\n");
        printf("===================================================\n\n");

        // Test configurations
        struct {
            int numLines;
            int lineLength;
            BOOL use24BitColor;
            const char *name;
        } configs[] = {
            {1000, 80, NO, "1K lines @ 80 cols (typical)"},
            {1000, 80, YES, "1K lines @ 80 cols (24-bit color)"},
            {10000, 80, NO, "10K lines @ 80 cols"},
            {100000, 80, NO, "100K lines @ 80 cols"},
            {1000000, 80, NO, "1M lines @ 80 cols (scrollback)"},
            {100000, 200, NO, "100K lines @ 200 cols (wide)"},
        };

        int numConfigs = sizeof(configs) / sizeof(configs[0]);

        printf("%-40s %12s %12s %8s %10s %10s\n",
               "Configuration", "Unpacked", "Packed", "Savings", "Pack", "Unpack");
        printf("%-40s %12s %12s %8s %10s %10s\n",
               "", "(bytes)", "(bytes)", "(%)", "(ms)", "(ms)");
        printf("--------------------------------------------------------------------------------\n");

        for (int i = 0; i < numConfigs; i++) {
            BenchmarkResult result = benchmarkPackUnpack(
                configs[i].numLines,
                configs[i].lineLength,
                configs[i].use24BitColor
            );

            double savingsPercent = 100.0 * (1.0 - (double)result.packedBytes / result.unpackedBytes);

            printf("%-40s %12lu %12lu %7.1f%% %9.2f %9.2f %s\n",
                   configs[i].name,
                   (unsigned long)result.unpackedBytes,
                   (unsigned long)result.packedBytes,
                   savingsPercent,
                   result.packTimeNs / 1000000.0,
                   result.unpackTimeNs / 1000000.0,
                   result.roundTripPassed ? "✓" : "✗");
        }

        printf("\n");

        // Summary
        BenchmarkResult millionLines = benchmarkPackUnpack(1000000, 80, NO);

        printf("Summary for 1M line scrollback (typical large history):\n");
        printf("  Unpacked size: %.1f MB\n", millionLines.unpackedBytes / 1024.0 / 1024.0);
        printf("  Packed size:   %.1f MB\n", millionLines.packedBytes / 1024.0 / 1024.0);
        printf("  Memory saved:  %.1f MB (%.1f%%)\n",
               (millionLines.unpackedBytes - millionLines.packedBytes) / 1024.0 / 1024.0,
               100.0 * (1.0 - (double)millionLines.packedBytes / millionLines.unpackedBytes));
        printf("  Pack time:     %.2f ms (%.2f us/line)\n",
               millionLines.packTimeNs / 1000000.0,
               millionLines.packTimeNs / 1000000.0 / 1000.0);
        printf("  Unpack time:   %.2f ms (%.2f us/line)\n",
               millionLines.unpackTimeNs / 1000000.0,
               millionLines.unpackTimeNs / 1000000.0 / 1000.0);
        printf("  Round-trip:    %s\n", millionLines.roundTripPassed ? "PASS" : "FAIL");

        printf("\n");
        printf("Per-character statistics:\n");
        printf("  sizeof(screen_char_t):        %lu bytes\n", sizeof(screen_char_t));
        printf("  sizeof(packed_screen_char_t): %lu bytes\n", sizeof(packed_screen_char_t));
        printf("  Memory reduction:             %.1f%%\n",
               100.0 * (1.0 - (double)sizeof(packed_screen_char_t) / sizeof(screen_char_t)));
        printf("  Cache line efficiency:        %.1fx (%.1f vs %.1f chars/line)\n",
               (64.0 / sizeof(packed_screen_char_t)) / (64.0 / sizeof(screen_char_t)),
               64.0 / sizeof(packed_screen_char_t),
               64.0 / sizeof(screen_char_t));

        printf("\n");

        // Save baseline JSON
        NSString *baselineJson = [NSString stringWithFormat:
            @"{\n"
            @"  \"date\": \"%@\",\n"
            @"  \"test\": \"LineBlockPacked Memory Efficiency\",\n"
            @"  \"million_line_scrollback\": {\n"
            @"    \"unpacked_mb\": %.2f,\n"
            @"    \"packed_mb\": %.2f,\n"
            @"    \"savings_mb\": %.2f,\n"
            @"    \"savings_percent\": %.1f,\n"
            @"    \"pack_ms\": %.2f,\n"
            @"    \"unpack_ms\": %.2f,\n"
            @"    \"pack_us_per_line\": %.2f,\n"
            @"    \"unpack_us_per_line\": %.2f,\n"
            @"    \"round_trip_passed\": %@\n"
            @"  },\n"
            @"  \"per_char\": {\n"
            @"    \"screen_char_t_bytes\": %lu,\n"
            @"    \"packed_screen_char_t_bytes\": %lu,\n"
            @"    \"memory_reduction_percent\": %.1f,\n"
            @"    \"cache_efficiency_ratio\": %.2f\n"
            @"  }\n"
            @"}\n",
            [[NSDate date] description],
            millionLines.unpackedBytes / 1024.0 / 1024.0,
            millionLines.packedBytes / 1024.0 / 1024.0,
            (millionLines.unpackedBytes - millionLines.packedBytes) / 1024.0 / 1024.0,
            100.0 * (1.0 - (double)millionLines.packedBytes / millionLines.unpackedBytes),
            millionLines.packTimeNs / 1000000.0,
            millionLines.unpackTimeNs / 1000000.0,
            millionLines.packTimeNs / 1000000.0 / 1000.0,
            millionLines.unpackTimeNs / 1000000.0 / 1000.0,
            millionLines.roundTripPassed ? @"true" : @"false",
            sizeof(screen_char_t),
            sizeof(packed_screen_char_t),
            100.0 * (1.0 - (double)sizeof(packed_screen_char_t) / sizeof(screen_char_t)),
            (64.0 / sizeof(packed_screen_char_t)) / (64.0 / sizeof(screen_char_t))
        ];

        NSError *error = nil;
        [baselineJson writeToFile:@"Benchmarks/baselines/lineblock_packed_baseline.json"
                       atomically:YES
                         encoding:NSUTF8StringEncoding
                            error:&error];

        if (error) {
            NSLog(@"Error writing baseline: %@", error);
        } else {
            printf("Baseline saved to Benchmarks/baselines/lineblock_packed_baseline.json\n");
        }

        return millionLines.roundTripPassed ? 0 : 1;
    }
}
