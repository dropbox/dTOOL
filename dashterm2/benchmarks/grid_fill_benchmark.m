/*
 * VT100Grid Fill Benchmark
 *
 * DashTerm2 Performance Benchmark Suite
 * Iteration #142: Evaluate VT100GridFillRunWithChar optimization potential
 *
 * This benchmark measures the performance of filling screen character arrays,
 * comparing the current doubling-memcpy approach vs potential alternatives.
 *
 * Usage:
 *   clang -framework Foundation -fobjc-arc -O3 \
 *     benchmarks/grid_fill_benchmark.m -o benchmarks/grid_fill_benchmark
 *   ./benchmarks/grid_fill_benchmark
 */

#import <Foundation/Foundation.h>
#include <mach/mach_time.h>
#include <stdint.h>
#include <string.h>

#if __has_include(<arm_neon.h>) && defined(__aarch64__)
#import <arm_neon.h>
#define HAS_NEON 1
#else
#define HAS_NEON 0
#endif

// Replicate the actual screen_char_t structure (12 bytes)
typedef struct {
    uint16_t code;           // 2 bytes
    uint8_t foregroundColor; // 1 byte
    uint8_t fgGreen;         // 1 byte
    uint8_t fgBlue;          // 1 byte
    uint8_t backgroundColor; // 1 byte
    uint8_t bgGreen;         // 1 byte
    uint8_t bgBlue;          // 1 byte
    uint32_t flags;          // 4 bytes (packed bit fields)
} screen_char_t;

_Static_assert(sizeof(screen_char_t) == 12, "screen_char_t must be 12 bytes");

// Timer utilities
static mach_timebase_info_data_t sTimebaseInfo = {0, 0};

static double machTimeToNs(uint64_t elapsed) {
    if (sTimebaseInfo.denom == 0) {
        mach_timebase_info(&sTimebaseInfo);
    }
    return (double)elapsed * sTimebaseInfo.numer / sTimebaseInfo.denom;
}

static double machTimeToMs(uint64_t elapsed) {
    return machTimeToNs(elapsed) / 1e6;
}

#pragma mark - Implementation 1: Current (doubling memcpy)

static inline void FillRunWithChar_Current(screen_char_t *line, int start, int length, screen_char_t value) {
    if (length <= 0) {
        return;
    }
    screen_char_t *dest = line + start;
    dest[0] = value;
    int filled = 1;
    while (filled < length) {
        const int chunk = (filled < length - filled) ? filled : (length - filled);
        memcpy(dest + filled, dest, chunk * sizeof(screen_char_t));
        filled += chunk;
    }
}

#pragma mark - Implementation 2: Simple loop (baseline)

static inline void FillRunWithChar_Loop(screen_char_t *line, int start, int length, screen_char_t value) {
    if (length <= 0) {
        return;
    }
    screen_char_t *dest = line + start;
    for (int i = 0; i < length; i++) {
        dest[i] = value;
    }
}

#pragma mark - Implementation 3: memset_pattern for 12-byte structs

static inline void FillRunWithChar_Pattern(screen_char_t *line, int start, int length, screen_char_t value) {
    if (length <= 0) {
        return;
    }
    screen_char_t *dest = line + start;
    // memset_pattern16 requires 16 bytes, but we have 12-byte structs
    // We need to manually fill since there's no memset_pattern12
    // Let's use memset_pattern16 with padding (not ideal, but for testing)

    // Since screen_char_t is 12 bytes, we use a different approach:
    // Fill 4 elements at a time (48 bytes) using memcpy
    if (length >= 4) {
        dest[0] = value;
        dest[1] = value;
        dest[2] = value;
        dest[3] = value;
        int filled = 4;
        while (filled < length) {
            int chunk = (filled * 4 <= length - filled) ? filled : (length - filled);
            if (chunk > 0) {
                memcpy(dest + filled, dest, chunk * sizeof(screen_char_t));
                filled += chunk;
            }
        }
    } else {
        for (int i = 0; i < length; i++) {
            dest[i] = value;
        }
    }
}

#pragma mark - Implementation 4: Unrolled loop

static inline void FillRunWithChar_Unrolled(screen_char_t *line, int start, int length, screen_char_t value) {
    if (length <= 0) {
        return;
    }
    screen_char_t *dest = line + start;
    int i = 0;

    // Unroll by 8
    for (; i + 7 < length; i += 8) {
        dest[i] = value;
        dest[i + 1] = value;
        dest[i + 2] = value;
        dest[i + 3] = value;
        dest[i + 4] = value;
        dest[i + 5] = value;
        dest[i + 6] = value;
        dest[i + 7] = value;
    }

    // Handle remainder
    for (; i < length; i++) {
        dest[i] = value;
    }
}

#pragma mark - Implementation 5: NEON optimized (ARM64 only)

#if HAS_NEON
// NEON approach: Store 12-byte values using partial vector stores
// Since 12 doesn't divide evenly into SIMD widths, we use overlapping stores
static inline void FillRunWithChar_NEON(screen_char_t *line, int start, int length, screen_char_t value) {
    if (length <= 0) {
        return;
    }
    screen_char_t *dest = line + start;

    // For small fills, use simple loop
    if (length < 8) {
        for (int i = 0; i < length; i++) {
            dest[i] = value;
        }
        return;
    }

    // Pre-fill a 96-byte buffer (8 screen_char_t) for bulk copies
    // 96 bytes = 6 x 16 bytes = 6 NEON registers
    __attribute__((aligned(16))) screen_char_t pattern[8];
    for (int i = 0; i < 8; i++) {
        pattern[i] = value;
    }

    // Load the pattern into NEON registers
    uint8x16_t v0 = vld1q_u8((const uint8_t *)&pattern[0]);      // bytes 0-15
    uint8x16_t v1 = vld1q_u8((const uint8_t *)&pattern[0] + 16); // bytes 16-31
    uint8x16_t v2 = vld1q_u8((const uint8_t *)&pattern[0] + 32); // bytes 32-47
    uint8x16_t v3 = vld1q_u8((const uint8_t *)&pattern[0] + 48); // bytes 48-63
    uint8x16_t v4 = vld1q_u8((const uint8_t *)&pattern[0] + 64); // bytes 64-79
    uint8x16_t v5 = vld1q_u8((const uint8_t *)&pattern[0] + 80); // bytes 80-95

    uint8_t *ptr = (uint8_t *)dest;
    int bytes_to_fill = length * sizeof(screen_char_t);
    int bytes_filled = 0;

    // Bulk fill 96 bytes at a time (8 screen_char_t)
    while (bytes_filled + 96 <= bytes_to_fill) {
        vst1q_u8(ptr + bytes_filled, v0);
        vst1q_u8(ptr + bytes_filled + 16, v1);
        vst1q_u8(ptr + bytes_filled + 32, v2);
        vst1q_u8(ptr + bytes_filled + 48, v3);
        vst1q_u8(ptr + bytes_filled + 64, v4);
        vst1q_u8(ptr + bytes_filled + 80, v5);
        bytes_filled += 96;
    }

    // Handle remainder (up to 7 screen_char_t = 84 bytes)
    int remaining = (bytes_to_fill - bytes_filled) / sizeof(screen_char_t);
    screen_char_t *remaining_dest = (screen_char_t *)(ptr + bytes_filled);
    for (int i = 0; i < remaining; i++) {
        remaining_dest[i] = value;
    }
}
#endif

#pragma mark - Benchmark configuration

typedef struct {
    const char *name;
    int length;
} BenchConfig;

static const BenchConfig kConfigs[] = {
    {"Small (10 chars)", 10},           {"Line clear (80 chars)", 80}, {"Wide line (200 chars)", 200},
    {"Large clear (1000 chars)", 1000}, {"Full screen (24x80)", 1920}, {"Large screen (60x200)", 12000},
};
static const int kNumConfigs = sizeof(kConfigs) / sizeof(kConfigs[0]);

typedef void (*FillFunc)(screen_char_t *, int, int, screen_char_t);

typedef struct {
    const char *name;
    FillFunc func;
} Implementation;

static const Implementation kImplementations[] = {
    {"Current (doubling)", FillRunWithChar_Current}, {"Simple loop", FillRunWithChar_Loop},
    {"Pattern (4x init)", FillRunWithChar_Pattern},  {"Unrolled (8x)", FillRunWithChar_Unrolled},
#if HAS_NEON
    {"NEON vectorized", FillRunWithChar_NEON},
#endif
};
static const int kNumImplementations = sizeof(kImplementations) / sizeof(kImplementations[0]);

#pragma mark - Benchmark execution

static double runBenchmark(FillFunc func, int length, int iterations) {
    // Allocate buffer with some padding
    screen_char_t *buffer = calloc(length + 16, sizeof(screen_char_t));

    // Create test value
    screen_char_t testValue = {.code = 'X',
                               .foregroundColor = 255,
                               .fgGreen = 255,
                               .fgBlue = 255,
                               .backgroundColor = 0,
                               .bgGreen = 0,
                               .bgBlue = 0,
                               .flags = 0x12345678};

    // Warmup
    for (int i = 0; i < 100; i++) {
        func(buffer, 0, length, testValue);
    }

    // Timed run
    uint64_t start = mach_absolute_time();
    for (int i = 0; i < iterations; i++) {
        func(buffer, 0, length, testValue);
    }
    uint64_t end = mach_absolute_time();

    // Verify correctness
    for (int i = 0; i < length; i++) {
        if (memcmp(&buffer[i], &testValue, sizeof(screen_char_t)) != 0) {
            fprintf(stderr, "ERROR: Verification failed at index %d\n", i);
            break;
        }
    }

    free(buffer);
    return machTimeToMs(end - start);
}

int main(int argc, const char *argv[]) {
    @autoreleasepool {
        printf("=======================================================\n");
        printf("VT100Grid Fill Benchmark\n");
        printf("DashTerm2 - Iteration #142\n");
        printf("=======================================================\n\n");

        printf("Configuration:\n");
        printf("  sizeof(screen_char_t): %zu bytes\n", sizeof(screen_char_t));
        printf("  Implementations tested: %d\n", kNumImplementations);
        printf("  Configurations: %d\n\n", kNumConfigs);

#if HAS_NEON
        printf("  NEON: Available (ARM64)\n\n");
#else
        printf("  NEON: Not available (Intel or non-ARM64)\n\n");
#endif

        // Results table
        printf("%-25s", "Configuration");
        for (int impl = 0; impl < kNumImplementations; impl++) {
            printf(" %18s", kImplementations[impl].name);
        }
        printf(" %10s\n", "Best");

        printf("%-25s", "-------------------------");
        for (int impl = 0; impl < kNumImplementations; impl++) {
            printf(" %18s", "------------------");
        }
        printf(" %10s\n", "----------");

        double totalTimes[kNumImplementations];
        memset(totalTimes, 0, sizeof(totalTimes));

        for (int cfg = 0; cfg < kNumConfigs; cfg++) {
            BenchConfig config = kConfigs[cfg];

            // Scale iterations inversely with size for ~consistent benchmark time
            int iterations = 100000 / (1 + config.length / 100);
            if (iterations < 1000)
                iterations = 1000;

            printf("%-25s", config.name);

            double times[kNumImplementations];
            double minTime = INFINITY;
            int bestImpl = 0;

            for (int impl = 0; impl < kNumImplementations; impl++) {
                times[impl] = runBenchmark(kImplementations[impl].func, config.length, iterations);
                totalTimes[impl] += times[impl];

                if (times[impl] < minTime) {
                    minTime = times[impl];
                    bestImpl = impl;
                }

                printf(" %15.3f ms", times[impl]);
            }

            // Calculate speedup of best vs current
            double speedup = times[0] / minTime;
            if (bestImpl == 0) {
                printf(" %10s\n", "current");
            } else {
                printf(" %7.2fx (%s)\n", speedup, kImplementations[bestImpl].name);
            }
        }

        // Summary
        printf("\n%-25s", "TOTAL");
        double minTotal = INFINITY;
        int bestTotal = 0;
        for (int impl = 0; impl < kNumImplementations; impl++) {
            printf(" %15.3f ms", totalTimes[impl]);
            if (totalTimes[impl] < minTotal) {
                minTotal = totalTimes[impl];
                bestTotal = impl;
            }
        }
        double totalSpeedup = totalTimes[0] / minTotal;
        printf(" %7.2fx (%s)\n", totalSpeedup, kImplementations[bestTotal].name);

        printf("\n=======================================================\n");
        printf("Analysis:\n");
        printf("=======================================================\n");

        // Compare each implementation to current
        printf("\nSpeedup vs Current (doubling memcpy):\n");
        for (int impl = 1; impl < kNumImplementations; impl++) {
            double speedup = totalTimes[0] / totalTimes[impl];
            printf("  %s: %.2fx %s\n", kImplementations[impl].name, speedup,
                   speedup > 1.0 ? "faster" : (speedup < 1.0 ? "SLOWER" : "same"));
        }

        printf("\n=======================================================\n");
        printf("Conclusion:\n");
        printf("=======================================================\n");

        if (bestTotal == 0) {
            printf("The current implementation (doubling memcpy) is optimal.\n");
            printf("No optimization needed.\n");
        } else {
            printf("Best implementation: %s (%.2fx faster overall)\n", kImplementations[bestTotal].name, totalSpeedup);
            printf("\nRecommendation: Consider switching to %s for fill operations.\n",
                   kImplementations[bestTotal].name);
        }

        return 0;
    }
}
