/*
 * DVR LZ4 Compression Benchmark
 *
 * DashTerm2 Performance Benchmark Suite
 * Iteration #127: DVR LZ4 Compression
 *
 * This benchmark measures:
 * 1. Compression ratio of LZ4 on terminal screen buffers
 * 2. Compression and decompression throughput
 * 3. Memory savings from compression
 *
 * Usage:
 *   clang -O2 -framework Foundation -lcompression dvr_compression_benchmark.m -o dvr_compression_benchmark
 *   ./dvr_compression_benchmark
 */

#import <Foundation/Foundation.h>
#include <compression.h>
#include <mach/mach_time.h>
#include <stdint.h>

// Screen dimensions for benchmarking (typical terminal sizes)
typedef struct {
    const char *name;
    int width;
    int height;
} ScreenSize;

static const ScreenSize kScreenSizes[] = {
    {"Small (80x24)", 80, 24},
    {"Medium (120x40)", 120, 40},
    {"Large (200x60)", 200, 60},
    {"XL (300x100)", 300, 100},
};
static const int kNumScreenSizes = sizeof(kScreenSizes) / sizeof(kScreenSizes[0]);

// Simulated screen_char_t structure (24 bytes in DashTerm2)
typedef struct {
    uint32_t code;         // Unicode code point
    uint32_t foreground;   // Foreground color
    uint32_t background;   // Background color
    uint16_t flags;        // Attribute flags
    uint8_t underline;     // Underline style
    uint8_t bold;          // Bold flag
    uint8_t italic;        // Italic flag
    uint8_t blink;         // Blink flag
    uint8_t invisible;     // Invisible flag
    uint8_t strikethrough; // Strikethrough flag
} BenchScreenChar;

// Content patterns for benchmarking
typedef enum {
    kContentEmpty,        // Empty screen (all spaces)
    kContentTypicalShell, // Typical shell output (commands, prompts)
    kContentCodeEditor,   // Code with syntax highlighting
    kContentRandomText,   // Random ASCII text
    kContentUnicode,      // Unicode with various characters
    kContentHighEntropy,  // Random bytes (worst case for compression)
} ContentType;

static const char *kContentNames[] = {"Empty Screen", "Typical Shell", "Code Editor",
                                      "Random ASCII", "Unicode Text",  "High Entropy"};

// Timer utilities
static double machTimeToMs(uint64_t elapsed) {
    static mach_timebase_info_data_t sTimebaseInfo = {0, 0};
    if (sTimebaseInfo.denom == 0) {
        mach_timebase_info(&sTimebaseInfo);
    }
    return (double)elapsed * sTimebaseInfo.numer / sTimebaseInfo.denom / 1e6;
}

// Fill screen buffer with content pattern
static void fillScreenBuffer(BenchScreenChar *buffer, int width, int height, ContentType content) {
    size_t numChars = width * height;

    switch (content) {
        case kContentEmpty:
            // All spaces with default attributes
            for (size_t i = 0; i < numChars; i++) {
                buffer[i] = (BenchScreenChar){.code = ' ',
                                              .foreground = 0xFFFFFF, // White
                                              .background = 0x000000, // Black
                                              .flags = 0};
            }
            break;

        case kContentTypicalShell:
            // Simulate shell output with prompts, commands, output
            for (int y = 0; y < height; y++) {
                for (int x = 0; x < width; x++) {
                    size_t idx = y * width + x;
                    if (x < 3 && y % 5 == 0) {
                        // Prompt "$> " in green
                        buffer[idx] = (BenchScreenChar){
                            .code = (x == 0 ? '$' : (x == 1 ? '>' : ' ')),
                            .foreground = 0x00FF00,
                            .background = 0x000000,
                        };
                    } else if (y % 5 == 0) {
                        // Command in white
                        buffer[idx] = (BenchScreenChar){
                            .code = 'a' + (x % 26),
                            .foreground = 0xFFFFFF,
                            .background = 0x000000,
                        };
                    } else {
                        // Output in gray
                        buffer[idx] = (BenchScreenChar){
                            .code = (x < width - 5) ? ('a' + ((x + y) % 26)) : ' ',
                            .foreground = 0xAAAAAA,
                            .background = 0x000000,
                        };
                    }
                }
            }
            break;

        case kContentCodeEditor:
            // Simulate code with syntax highlighting
            for (int y = 0; y < height; y++) {
                for (int x = 0; x < width; x++) {
                    size_t idx = y * width + x;
                    uint32_t fg;
                    char c;

                    if (x < 4) {
                        // Line numbers in yellow
                        fg = 0xFFFF00;
                        c = '0' + ((y + 1) % 10);
                    } else if (x < 8 && y % 3 == 0) {
                        // Keywords in blue
                        fg = 0x6666FF;
                        c = "func    "[x - 4];
                    } else if (x >= width - 10 && y % 2 == 0) {
                        // Comments in green
                        fg = 0x00AA00;
                        c = (x == width - 10) ? '/' : (x == width - 9 ? '/' : 'x');
                    } else {
                        // Regular code in white
                        fg = 0xFFFFFF;
                        c = (x < width / 2) ? ('a' + ((x + y) % 26)) : ' ';
                    }

                    buffer[idx] = (BenchScreenChar){
                        .code = c,
                        .foreground = fg,
                        .background = 0x1E1E1E, // Dark editor background
                    };
                }
            }
            break;

        case kContentRandomText:
            // Random ASCII printable characters
            srand(42); // Deterministic for reproducibility
            for (size_t i = 0; i < numChars; i++) {
                buffer[i] = (BenchScreenChar){
                    .code = 32 + (rand() % 95), // ASCII printable
                    .foreground = 0xFFFFFF,
                    .background = 0x000000,
                };
            }
            break;

        case kContentUnicode:
            // Unicode characters with varied attributes
            for (int y = 0; y < height; y++) {
                for (int x = 0; x < width; x++) {
                    size_t idx = y * width + x;
                    // Mix of ASCII, Latin Extended, CJK, Emoji
                    uint32_t codePoints[] = {'A', 0x00E9, 0x4E2D, 0x1F600, ' '};
                    buffer[idx] = (BenchScreenChar){
                        .code = codePoints[(x + y) % 5],
                        .foreground = 0xFF0000 + (y * 0x100) + x,
                        .background = 0x000000,
                        .flags = (uint16_t)(x % 8),
                    };
                }
            }
            break;

        case kContentHighEntropy:
            // High entropy data (worst case for compression)
            srand(12345);
            for (size_t i = 0; i < numChars; i++) {
                buffer[i] = (BenchScreenChar){
                    .code = rand(),
                    .foreground = rand(),
                    .background = rand(),
                    .flags = (uint16_t)rand(),
                    .underline = rand() % 256,
                    .bold = rand() % 2,
                    .italic = rand() % 2,
                    .blink = rand() % 2,
                    .invisible = rand() % 2,
                    .strikethrough = rand() % 2,
                };
            }
            break;
    }
}

// Benchmark results structure
typedef struct {
    const char *screenName;
    const char *contentName;
    size_t uncompressedSize;
    size_t compressedSize;
    double compressionRatio;
    double compressTimeMs;
    double decompressTimeMs;
    double compressThroughputMBps;
    double decompressThroughputMBps;
} BenchmarkResult;

// Run compression benchmark for a single configuration
static BenchmarkResult runBenchmark(ScreenSize screen, ContentType content, int iterations) {
    BenchmarkResult result = {0};
    result.screenName = screen.name;
    result.contentName = kContentNames[content];

    // Allocate buffers
    size_t bufferSize = screen.width * screen.height * sizeof(BenchScreenChar);
    BenchScreenChar *sourceBuffer = malloc(bufferSize);
    uint8_t *compressedBuffer = malloc(bufferSize); // LZ4 output <= input
    uint8_t *decompressedBuffer = malloc(bufferSize);

    // Fill source buffer
    fillScreenBuffer(sourceBuffer, screen.width, screen.height, content);

    result.uncompressedSize = bufferSize;

    // Warmup
    for (int i = 0; i < 5; i++) {
        compression_encode_buffer(compressedBuffer, bufferSize, (const uint8_t *)sourceBuffer, bufferSize, NULL,
                                  COMPRESSION_LZ4);
    }

    // Benchmark compression
    uint64_t compressStart = mach_absolute_time();
    size_t compressedSize = 0;
    for (int i = 0; i < iterations; i++) {
        compressedSize = compression_encode_buffer(compressedBuffer, bufferSize, (const uint8_t *)sourceBuffer,
                                                   bufferSize, NULL, COMPRESSION_LZ4);
    }
    uint64_t compressEnd = mach_absolute_time();

    result.compressedSize = compressedSize;
    result.compressionRatio = (double)bufferSize / compressedSize;
    result.compressTimeMs = machTimeToMs(compressEnd - compressStart) / iterations;
    result.compressThroughputMBps = (bufferSize / 1e6) / (result.compressTimeMs / 1e3);

    // Benchmark decompression
    uint64_t decompressStart = mach_absolute_time();
    for (int i = 0; i < iterations; i++) {
        compression_decode_buffer(decompressedBuffer, bufferSize, compressedBuffer, compressedSize, NULL,
                                  COMPRESSION_LZ4);
    }
    uint64_t decompressEnd = mach_absolute_time();

    result.decompressTimeMs = machTimeToMs(decompressEnd - decompressStart) / iterations;
    result.decompressThroughputMBps = (bufferSize / 1e6) / (result.decompressTimeMs / 1e3);

    // Verify decompression
    if (memcmp(sourceBuffer, decompressedBuffer, bufferSize) != 0) {
        fprintf(stderr, "ERROR: Decompression verification failed!\n");
    }

    free(sourceBuffer);
    free(compressedBuffer);
    free(decompressedBuffer);

    return result;
}

// Print results table
static void printResults(BenchmarkResult *results, int count) {
    printf("\n%-18s %-16s %10s %10s %8s %10s %10s %12s %12s\n", "Screen Size", "Content", "Orig (KB)", "Comp (KB)",
           "Ratio", "Comp ms", "Decomp ms", "Comp MB/s", "Decomp MB/s");
    printf("%-18s %-16s %10s %10s %8s %10s %10s %12s %12s\n", "----------", "-------", "--------", "--------", "-----",
           "-------", "---------", "---------", "-----------");

    for (int i = 0; i < count; i++) {
        BenchmarkResult *r = &results[i];
        printf("%-18s %-16s %10.1f %10.1f %7.2fx %10.3f %10.3f %12.1f %12.1f\n", r->screenName, r->contentName,
               r->uncompressedSize / 1024.0, r->compressedSize / 1024.0, r->compressionRatio, r->compressTimeMs,
               r->decompressTimeMs, r->compressThroughputMBps, r->decompressThroughputMBps);
    }
}

// Generate JSON output
static void printJSON(BenchmarkResult *results, int count) {
    printf("\n{\n  \"benchmark\": \"DVR LZ4 Compression\",\n");
    printf("  \"timestamp\": \"%s\",\n", [[[NSDate date] description] UTF8String]);
    printf("  \"results\": [\n");

    for (int i = 0; i < count; i++) {
        BenchmarkResult *r = &results[i];
        printf("    {\n");
        printf("      \"screen\": \"%s\",\n", r->screenName);
        printf("      \"content\": \"%s\",\n", r->contentName);
        printf("      \"uncompressed_bytes\": %zu,\n", r->uncompressedSize);
        printf("      \"compressed_bytes\": %zu,\n", r->compressedSize);
        printf("      \"compression_ratio\": %.2f,\n", r->compressionRatio);
        printf("      \"compress_ms\": %.3f,\n", r->compressTimeMs);
        printf("      \"decompress_ms\": %.3f,\n", r->decompressTimeMs);
        printf("      \"compress_mbps\": %.1f,\n", r->compressThroughputMBps);
        printf("      \"decompress_mbps\": %.1f\n", r->decompressThroughputMBps);
        printf("    }%s\n", i < count - 1 ? "," : "");
    }

    printf("  ]\n}\n");
}

int main(int argc, const char *argv[]) {
    @autoreleasepool {
        printf("=======================================================\n");
        printf("DVR LZ4 Compression Benchmark\n");
        printf("DashTerm2 - Iteration #127\n");
        printf("=======================================================\n\n");

        printf("Configuration:\n");
        printf("  Compression algorithm: LZ4 (Apple Compression Framework)\n");
        printf("  screen_char_t size: %zu bytes\n", sizeof(BenchScreenChar));
        printf("  Iterations per test: 100\n\n");

        const int iterations = 100;
        const int numContentTypes = 6;
        int totalTests = kNumScreenSizes * numContentTypes;
        BenchmarkResult *results = calloc(totalTests, sizeof(BenchmarkResult));

        int resultIdx = 0;
        for (int s = 0; s < kNumScreenSizes; s++) {
            for (int c = 0; c < numContentTypes; c++) {
                results[resultIdx++] = runBenchmark(kScreenSizes[s], (ContentType)c, iterations);
            }
        }

        printResults(results, totalTests);

        // Summary statistics
        printf("\n=======================================================\n");
        printf("Summary by Content Type:\n");
        printf("=======================================================\n");

        for (int c = 0; c < numContentTypes; c++) {
            double avgRatio = 0;
            double avgCompMBps = 0;
            double avgDecompMBps = 0;
            int count = 0;

            for (int i = 0; i < totalTests; i++) {
                if (strcmp(results[i].contentName, kContentNames[c]) == 0) {
                    avgRatio += results[i].compressionRatio;
                    avgCompMBps += results[i].compressThroughputMBps;
                    avgDecompMBps += results[i].decompressThroughputMBps;
                    count++;
                }
            }

            avgRatio /= count;
            avgCompMBps /= count;
            avgDecompMBps /= count;

            printf("  %-16s: %.2fx ratio, %.1f MB/s compress, %.1f MB/s decompress\n", kContentNames[c], avgRatio,
                   avgCompMBps, avgDecompMBps);
        }

        // Output JSON for baseline storage
        printf("\n=======================================================\n");
        printf("JSON Output (for baselines):\n");
        printf("=======================================================\n");
        printJSON(results, totalTests);

        free(results);

        return 0;
    }
}
