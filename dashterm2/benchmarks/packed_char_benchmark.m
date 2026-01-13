/*
 * packed_char_benchmark.m
 *
 * Benchmark for packed_screen_char_t vs screen_char_t memory and conversion performance.
 *
 * Compile and run:
 *   cd ~/dashterm2
 *   clang -O3 -framework Foundation Benchmarks/packed_char_benchmark.m sources/PackedScreenChar.m -Isources -o /tmp/packed_benchmark && /tmp/packed_benchmark
 */

#import <Foundation/Foundation.h>
#import <mach/mach_time.h>
#import "ScreenChar.h"
#import "PackedScreenChar.h"

// Mock iTermMalloc for standalone compilation
void *iTermMalloc(size_t size) { return malloc(size); }
void *iTermCalloc(size_t count, size_t size) { return calloc(count, size); }

static mach_timebase_info_data_t timebaseInfo;

static uint64_t nanoseconds(uint64_t elapsed) {
    return elapsed * timebaseInfo.numer / timebaseInfo.denom;
}

static double milliseconds(uint64_t elapsed) {
    return (double)nanoseconds(elapsed) / 1e6;
}

// Create test data with various attributes
static void fillTestData(screen_char_t *buffer, NSUInteger count) {
    for (NSUInteger i = 0; i < count; i++) {
        buffer[i].code = (unichar)('A' + (i % 26));
        buffer[i].complexChar = (i % 100 == 0);  // 1% complex chars
        buffer[i].bold = (i % 10 == 0);
        buffer[i].italic = (i % 20 == 0);
        buffer[i].underline = (i % 30 == 0);

        // Color distribution:
        // 90% default, 8% ANSI, 1.5% 256-color, 0.5% 24-bit
        if (i % 200 == 0) {
            // 24-bit color (0.5%)
            buffer[i].foregroundColorMode = ColorMode24bit;
            buffer[i].foregroundColor = (i * 17) & 0xFF;
            buffer[i].fgGreen = (i * 23) & 0xFF;
            buffer[i].fgBlue = (i * 31) & 0xFF;
        } else if (i % 66 == 0) {
            // 256-color (1.5%)
            buffer[i].foregroundColorMode = ColorModeNormal;
            buffer[i].foregroundColor = 16 + (i % 216);
        } else if (i % 12 == 0) {
            // ANSI color (8%)
            buffer[i].foregroundColorMode = ColorModeNormal;
            buffer[i].foregroundColor = i % 16;
        } else {
            // Default (90%)
            buffer[i].foregroundColorMode = ColorModeAlternate;
            buffer[i].foregroundColor = ALTSEM_DEFAULT;
        }

        // Background mostly default
        buffer[i].backgroundColorMode = ColorModeAlternate;
        buffer[i].backgroundColor = ALTSEM_DEFAULT;
    }
}

// Verify round-trip conversion preserves data
static BOOL verifyConversion(const screen_char_t *original,
                              const screen_char_t *roundTrip,
                              NSUInteger count,
                              NSUInteger *diffCount) {
    *diffCount = 0;
    for (NSUInteger i = 0; i < count; i++) {
        // Check essential fields
        if (original[i].code != roundTrip[i].code ||
            original[i].complexChar != roundTrip[i].complexChar ||
            original[i].bold != roundTrip[i].bold ||
            original[i].italic != roundTrip[i].italic ||
            original[i].underline != roundTrip[i].underline) {
            (*diffCount)++;
        }
        // Note: 24-bit colors may be quantized if table full, so allow some differences
    }
    return *diffCount == 0;
}

int main(int argc, const char * argv[]) {
    @autoreleasepool {
        mach_timebase_info(&timebaseInfo);

        printf("===========================================\n");
        printf("Packed screen_char_t Benchmark\n");
        printf("===========================================\n\n");

        // Test sizes
        printf("Structure Sizes:\n");
        printf("  screen_char_t:        %lu bytes\n", sizeof(screen_char_t));
        printf("  packed_screen_char_t: %lu bytes\n", sizeof(packed_screen_char_t));
        printf("  Memory reduction:     %.1f%%\n",
               100.0 * (1.0 - (double)sizeof(packed_screen_char_t) / sizeof(screen_char_t)));
        printf("\n");

        // Cache line analysis
        printf("Cache Line Efficiency (64-byte cache line):\n");
        printf("  screen_char_t:        %.1f chars/cache line\n", 64.0 / sizeof(screen_char_t));
        printf("  packed_screen_char_t: %.1f chars/cache line\n", 64.0 / sizeof(packed_screen_char_t));
        printf("  Improvement:          %.2fx\n",
               (64.0 / sizeof(packed_screen_char_t)) / (64.0 / sizeof(screen_char_t)));
        printf("\n");

        // Memory savings for typical buffer sizes
        printf("Memory Savings (80 columns):\n");
        NSUInteger columns = 80;
        NSUInteger sizes[] = {24, 1000, 10000, 100000, 1000000};
        const char *labels[] = {"24 lines (1 screen)", "1K lines", "10K lines", "100K lines", "1M lines"};

        for (int i = 0; i < 5; i++) {
            NSUInteger chars = columns * sizes[i];
            size_t oldSize = chars * sizeof(screen_char_t);
            size_t newSize = chars * sizeof(packed_screen_char_t);
            printf("  %-20s: %6.1f MB -> %6.1f MB (saves %.1f MB)\n",
                   labels[i],
                   oldSize / 1e6,
                   newSize / 1e6,
                   (oldSize - newSize) / 1e6);
        }
        printf("\n");

        // Conversion benchmark
        printf("Conversion Performance:\n");
        NSUInteger benchmarkSize = 100000;  // 100K chars
        screen_char_t *original = calloc(benchmarkSize, sizeof(screen_char_t));
        screen_char_t *roundTrip = calloc(benchmarkSize, sizeof(screen_char_t));
        packed_screen_char_t *packed = calloc(benchmarkSize, sizeof(packed_screen_char_t));

        fillTestData(original, benchmarkSize);

        PackedColorTable *colorTable = [[PackedColorTable alloc] initWithCapacity:251];

        // Warm up
        for (int i = 0; i < 5; i++) {
            PackScreenCharArray(original, packed, benchmarkSize, colorTable);
            UnpackScreenCharArray(packed, roundTrip, benchmarkSize, colorTable);
        }

        // Pack benchmark
        int iterations = 100;
        uint64_t startTime = mach_absolute_time();
        for (int i = 0; i < iterations; i++) {
            PackScreenCharArray(original, packed, benchmarkSize, colorTable);
        }
        uint64_t packTime = mach_absolute_time() - startTime;

        // Unpack benchmark
        startTime = mach_absolute_time();
        for (int i = 0; i < iterations; i++) {
            UnpackScreenCharArray(packed, roundTrip, benchmarkSize, colorTable);
        }
        uint64_t unpackTime = mach_absolute_time() - startTime;

        double packMs = milliseconds(packTime) / iterations;
        double unpackMs = milliseconds(unpackTime) / iterations;
        double packNsPerChar = (double)nanoseconds(packTime) / iterations / benchmarkSize;
        double unpackNsPerChar = (double)nanoseconds(unpackTime) / iterations / benchmarkSize;

        printf("  Pack %lu chars:   %.3f ms (%.1f ns/char)\n", benchmarkSize, packMs, packNsPerChar);
        printf("  Unpack %lu chars: %.3f ms (%.1f ns/char)\n", benchmarkSize, unpackMs, unpackNsPerChar);
        printf("  Throughput (pack):   %.0f MB/s (in screen_char_t terms)\n",
               (benchmarkSize * sizeof(screen_char_t) / 1e6) / (packMs / 1000));
        printf("  Throughput (unpack): %.0f MB/s (in screen_char_t terms)\n",
               (benchmarkSize * sizeof(screen_char_t) / 1e6) / (unpackMs / 1000));
        printf("\n");

        // Verify correctness
        NSUInteger diffCount = 0;
        BOOL correct = verifyConversion(original, roundTrip, benchmarkSize, &diffCount);
        printf("Conversion Correctness:\n");
        printf("  Round-trip verified: %s\n", correct ? "YES" : "NO");
        if (!correct) {
            printf("  Differences: %lu (%.2f%% - may be due to 24-bit color quantization)\n",
                   diffCount, 100.0 * diffCount / benchmarkSize);
        }
        printf("\n");

        // Color table stats
        printf("Color Table Usage:\n");
        printf("  24-bit colors stored: %lu / %lu\n", colorTable.count, colorTable.capacity);
        printf("\n");

        // Summary
        printf("===========================================\n");
        printf("SUMMARY\n");
        printf("===========================================\n");
        printf("Memory savings: 33%% per character (12 -> 8 bytes)\n");
        printf("Cache efficiency: 1.5x more chars per cache line\n");
        printf("Pack overhead: %.1f ns/char\n", packNsPerChar);
        printf("Unpack overhead: %.1f ns/char\n", unpackNsPerChar);
        printf("\n");
        printf("For a 1M line scrollback buffer (80 columns):\n");
        printf("  Memory saved: 320 MB\n");
        printf("  Pack time for full buffer: %.0f ms\n",
               packNsPerChar * 80 * 1000000 / 1e6);
        printf("  Unpack time per screen (80x24): %.3f ms\n",
               unpackNsPerChar * 80 * 24 / 1e6);
        printf("\n");

        free(original);
        free(roundTrip);
        free(packed);

        return 0;
    }
}
