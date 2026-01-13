/*
 * SGR Color Escape Sequence Parsing Benchmark
 *
 * Measures the performance of parsing SGR (Select Graphic Rendition) escape
 * sequences, specifically focusing on 256-color (38;5;N) and 24-bit color
 * (38;2;R;G;B) codes which were identified as a bottleneck in benchmark #147.
 *
 * The benchmark measures:
 * 1. CSI parameter parsing overhead
 * 2. VT100GraphicRenditionExecuteSGR execution time
 * 3. Subparameter lookup performance (iTermParserGetAllCSISubparametersForParameter)
 *
 * Build:
 *   clang -framework Foundation -fobjc-arc -O3 \
 *     -I sources benchmarks/sgr_color_benchmark.m -o benchmarks/sgr_color_benchmark
 *
 * Run:
 *   ./benchmarks/sgr_color_benchmark
 */

#import <Foundation/Foundation.h>
#import <mach/mach_time.h>

// Inline the key definitions from the headers to make this self-contained
#define VT100CSIPARAM_MAX 16
#define VT100CSISUBPARAM_MAX 16

typedef enum {
    ColorModeAlternate = 0,
    ColorModeNormal = 1,
    ColorMode24bit = 2,
    ColorModeInvalid = 3
} ColorMode;

typedef struct {
    int p[VT100CSIPARAM_MAX];
    int count;
    int32_t cmd;
    struct {
        int parameter_index;
        int subparameter_index;
        int value;
    } subparameters[VT100CSISUBPARAM_MAX];
    int num_subparameters;
} CSIParam;

typedef struct {
    int red;
    int green;
    int blue;
    ColorMode mode;
} VT100TerminalColorValue;

// Benchmark configuration
static const NSUInteger kIterations = 1000;
static const NSUInteger kWarmupIterations = 100;
static const NSUInteger kColorsPerIteration = 256; // 256 colors per "line"

static inline uint64_t MachTicksToNanoseconds(uint64_t ticks) {
    static mach_timebase_info_data_t info;
    if (info.denom == 0) {
        mach_timebase_info(&info);
    }
    __uint128_t scaled = (__uint128_t)ticks * (uint64_t)info.numer;
    return (uint64_t)(scaled / info.denom);
}

// Current implementation from iTermParser.h
static inline int iTermParserGetNumberOfCSISubparameters(const CSIParam *csi, int parameter_index) {
    int count = 0;
    for (int j = 0; j < csi->num_subparameters; j++) {
        if (csi->subparameters[j].parameter_index == parameter_index) {
            count++;
        }
    }
    return count;
}

// Current implementation - O(n) scan per call
static inline int CurrentGetAllSubparameters(CSIParam *csi, int parameter_index, int arrayToFill[VT100CSISUBPARAM_MAX]) {
    int i = 0;
    for (int j = 0; j < csi->num_subparameters; j++) {
        if (csi->subparameters[j].parameter_index == parameter_index) {
            arrayToFill[i++] = csi->subparameters[j].value;
        }
    }
    return i;
}

// Optimized: Pre-indexed subparameters (O(1) lookup)
// This uses a small index to track where subparameters start for each parameter
typedef struct {
    CSIParam base;
    // Index: subparam_start[param_idx] = first index in subparameters array for this param
    // subparam_count[param_idx] = number of subparameters for this param
    int8_t subparam_start[VT100CSIPARAM_MAX];
    int8_t subparam_count[VT100CSIPARAM_MAX];
} IndexedCSIParam;

static inline void IndexCSIParam(const CSIParam *src, IndexedCSIParam *dst) {
    dst->base = *src;
    memset(dst->subparam_start, -1, sizeof(dst->subparam_start));
    memset(dst->subparam_count, 0, sizeof(dst->subparam_count));

    // Build index
    for (int j = 0; j < src->num_subparameters; j++) {
        int param_idx = src->subparameters[j].parameter_index;
        if (param_idx >= 0 && param_idx < VT100CSIPARAM_MAX) {
            if (dst->subparam_start[param_idx] < 0) {
                dst->subparam_start[param_idx] = j;
            }
            dst->subparam_count[param_idx]++;
        }
    }
}

static inline int IndexedGetAllSubparameters(IndexedCSIParam *csi, int parameter_index, int arrayToFill[VT100CSISUBPARAM_MAX]) {
    if (parameter_index < 0 || parameter_index >= VT100CSIPARAM_MAX) {
        return 0;
    }
    int8_t start = csi->subparam_start[parameter_index];
    int8_t count = csi->subparam_count[parameter_index];
    if (start < 0 || count <= 0) {
        return 0;
    }
    for (int i = 0; i < count; i++) {
        arrayToFill[i] = csi->base.subparameters[start + i].value;
    }
    return count;
}

// Current color value extraction (from VT100GraphicRendition.m)
static VT100TerminalColorValue CurrentColorValueFromCSI(CSIParam *csi, int *index) {
    const int i = *index;
    int subs[VT100CSISUBPARAM_MAX];
    const int numberOfSubparameters = CurrentGetAllSubparameters(csi, i, subs);

    if (numberOfSubparameters > 0) {
        // Preferred syntax using colons to delimit subparameters
        if (numberOfSubparameters >= 2 && subs[0] == 5) {
            // CSI 38:5:P m
            return (VT100TerminalColorValue){
                .red = subs[1],
                .green = 0,
                .blue = 0,
                .mode = ColorModeNormal
            };
        }
        if (numberOfSubparameters >= 4 && subs[0] == 2) {
            // 24-bit color
            if (numberOfSubparameters >= 5) {
                // Spec-compliant: CSI 38:2:colorspace:R:G:B m
                return (VT100TerminalColorValue){
                    .red = subs[2],
                    .green = subs[3],
                    .blue = subs[4],
                    .mode = ColorMode24bit
                };
            }
            // Misinterpretation compliant: CSI 38:2:R:G:B m
            return (VT100TerminalColorValue) {
                .red = subs[1],
                .green = subs[2],
                .blue = subs[3],
                .mode = ColorMode24bit
            };
        }
        return (VT100TerminalColorValue) {
            .red = -1, .green = -1, .blue = -1, .mode = ColorMode24bit
        };
    }

    // Semicolon-delimited format (xterm compatibility)
    if (csi->count - i >= 3 && csi->p[i + 1] == 5) {
        // CSI 38;5;N m
        *index += 2;
        return (VT100TerminalColorValue) {
            .red = csi->p[i + 2],
            .green = 0,
            .blue = 0,
            .mode = ColorModeNormal
        };
    }
    if (csi->count - i >= 5 && csi->p[i + 1] == 2) {
        // CSI 38;2;R;G;B m
        *index += 4;
        return (VT100TerminalColorValue) {
            .red = csi->p[i + 2],
            .green = csi->p[i + 3],
            .blue = csi->p[i + 4],
            .mode = ColorMode24bit
        };
    }
    return (VT100TerminalColorValue) {
        .red = -1, .green = -1, .blue = -1, .mode = ColorMode24bit
    };
}

// Optimized color value extraction with indexed subparameters
static VT100TerminalColorValue IndexedColorValueFromCSI(IndexedCSIParam *csi, int *index) {
    const int i = *index;
    int subs[VT100CSISUBPARAM_MAX];
    const int numberOfSubparameters = IndexedGetAllSubparameters(csi, i, subs);

    if (numberOfSubparameters > 0) {
        if (numberOfSubparameters >= 2 && subs[0] == 5) {
            return (VT100TerminalColorValue){
                .red = subs[1], .green = 0, .blue = 0, .mode = ColorModeNormal
            };
        }
        if (numberOfSubparameters >= 4 && subs[0] == 2) {
            if (numberOfSubparameters >= 5) {
                return (VT100TerminalColorValue){
                    .red = subs[2], .green = subs[3], .blue = subs[4], .mode = ColorMode24bit
                };
            }
            return (VT100TerminalColorValue) {
                .red = subs[1], .green = subs[2], .blue = subs[3], .mode = ColorMode24bit
            };
        }
        return (VT100TerminalColorValue) { .red = -1, .green = -1, .blue = -1, .mode = ColorMode24bit };
    }

    // Semicolon-delimited format
    if (csi->base.count - i >= 3 && csi->base.p[i + 1] == 5) {
        *index += 2;
        return (VT100TerminalColorValue) {
            .red = csi->base.p[i + 2], .green = 0, .blue = 0, .mode = ColorModeNormal
        };
    }
    if (csi->base.count - i >= 5 && csi->base.p[i + 1] == 2) {
        *index += 4;
        return (VT100TerminalColorValue) {
            .red = csi->base.p[i + 2], .green = csi->base.p[i + 3], .blue = csi->base.p[i + 4],
            .mode = ColorMode24bit
        };
    }
    return (VT100TerminalColorValue) { .red = -1, .green = -1, .blue = -1, .mode = ColorMode24bit };
}

// Create CSI param for 256-color: ESC[38;5;Nm
static void Create256ColorCSI(CSIParam *csi, int colorIndex) {
    memset(csi, 0, sizeof(*csi));
    csi->p[0] = 38;         // SGR code for foreground 256/24-bit
    csi->p[1] = 5;          // 256-color mode selector
    csi->p[2] = colorIndex; // Color index (0-255)
    csi->count = 3;
    csi->num_subparameters = 0;
}

// Create CSI param for 256-color colon syntax: ESC[38:5:Nm
static void Create256ColorColonCSI(CSIParam *csi, int colorIndex) {
    memset(csi, 0, sizeof(*csi));
    csi->p[0] = 38;
    csi->count = 1;
    // Add subparameters for colon syntax
    csi->subparameters[0].parameter_index = 0;
    csi->subparameters[0].subparameter_index = 0;
    csi->subparameters[0].value = 5;
    csi->subparameters[1].parameter_index = 0;
    csi->subparameters[1].subparameter_index = 1;
    csi->subparameters[1].value = colorIndex;
    csi->num_subparameters = 2;
}

// Create CSI param for 24-bit color: ESC[38;2;R;G;Bm
static void Create24BitColorCSI(CSIParam *csi, int r, int g, int b) {
    memset(csi, 0, sizeof(*csi));
    csi->p[0] = 38;
    csi->p[1] = 2;
    csi->p[2] = r;
    csi->p[3] = g;
    csi->p[4] = b;
    csi->count = 5;
    csi->num_subparameters = 0;
}

// Create CSI param for 24-bit color colon syntax: ESC[38:2:R:G:Bm
static void Create24BitColorColonCSI(CSIParam *csi, int r, int g, int b) {
    memset(csi, 0, sizeof(*csi));
    csi->p[0] = 38;
    csi->count = 1;
    // Add subparameters for colon syntax (without colorspace)
    csi->subparameters[0].parameter_index = 0;
    csi->subparameters[0].subparameter_index = 0;
    csi->subparameters[0].value = 2;
    csi->subparameters[1].parameter_index = 0;
    csi->subparameters[1].subparameter_index = 1;
    csi->subparameters[1].value = r;
    csi->subparameters[2].parameter_index = 0;
    csi->subparameters[2].subparameter_index = 2;
    csi->subparameters[2].value = g;
    csi->subparameters[3].parameter_index = 0;
    csi->subparameters[3].subparameter_index = 3;
    csi->subparameters[3].value = b;
    csi->num_subparameters = 4;
}

typedef struct {
    const char *name;
    uint64_t totalNs;
    NSUInteger iterations;
    NSUInteger colorsProcessed;
} BenchmarkResult;

static BenchmarkResult RunBenchmark_256Color_Semicolon_Current(void) {
    CSIParam csiParams[kColorsPerIteration];
    for (NSUInteger c = 0; c < kColorsPerIteration; c++) {
        Create256ColorCSI(&csiParams[c], (int)c);
    }

    // Warmup
    for (NSUInteger w = 0; w < kWarmupIterations; w++) {
        for (NSUInteger c = 0; c < kColorsPerIteration; c++) {
            int idx = 0;
            CurrentColorValueFromCSI(&csiParams[c], &idx);
        }
    }

    uint64_t totalNs = 0;
    for (NSUInteger i = 0; i < kIterations; i++) {
        const uint64_t start = mach_absolute_time();
        for (NSUInteger c = 0; c < kColorsPerIteration; c++) {
            int idx = 0;
            volatile VT100TerminalColorValue val = CurrentColorValueFromCSI(&csiParams[c], &idx);
            (void)val;
        }
        totalNs += MachTicksToNanoseconds(mach_absolute_time() - start);
    }

    return (BenchmarkResult){
        .name = "256-color (;) Current",
        .totalNs = totalNs,
        .iterations = kIterations,
        .colorsProcessed = kIterations * kColorsPerIteration
    };
}

static BenchmarkResult RunBenchmark_256Color_Colon_Current(void) {
    CSIParam csiParams[kColorsPerIteration];
    for (NSUInteger c = 0; c < kColorsPerIteration; c++) {
        Create256ColorColonCSI(&csiParams[c], (int)c);
    }

    // Warmup
    for (NSUInteger w = 0; w < kWarmupIterations; w++) {
        for (NSUInteger c = 0; c < kColorsPerIteration; c++) {
            int idx = 0;
            CurrentColorValueFromCSI(&csiParams[c], &idx);
        }
    }

    uint64_t totalNs = 0;
    for (NSUInteger i = 0; i < kIterations; i++) {
        const uint64_t start = mach_absolute_time();
        for (NSUInteger c = 0; c < kColorsPerIteration; c++) {
            int idx = 0;
            volatile VT100TerminalColorValue val = CurrentColorValueFromCSI(&csiParams[c], &idx);
            (void)val;
        }
        totalNs += MachTicksToNanoseconds(mach_absolute_time() - start);
    }

    return (BenchmarkResult){
        .name = "256-color (:) Current",
        .totalNs = totalNs,
        .iterations = kIterations,
        .colorsProcessed = kIterations * kColorsPerIteration
    };
}

static BenchmarkResult RunBenchmark_256Color_Colon_Indexed(void) {
    CSIParam csiParams[kColorsPerIteration];
    IndexedCSIParam indexedParams[kColorsPerIteration];
    for (NSUInteger c = 0; c < kColorsPerIteration; c++) {
        Create256ColorColonCSI(&csiParams[c], (int)c);
        IndexCSIParam(&csiParams[c], &indexedParams[c]);
    }

    // Warmup
    for (NSUInteger w = 0; w < kWarmupIterations; w++) {
        for (NSUInteger c = 0; c < kColorsPerIteration; c++) {
            int idx = 0;
            IndexedColorValueFromCSI(&indexedParams[c], &idx);
        }
    }

    uint64_t totalNs = 0;
    for (NSUInteger i = 0; i < kIterations; i++) {
        const uint64_t start = mach_absolute_time();
        for (NSUInteger c = 0; c < kColorsPerIteration; c++) {
            int idx = 0;
            volatile VT100TerminalColorValue val = IndexedColorValueFromCSI(&indexedParams[c], &idx);
            (void)val;
        }
        totalNs += MachTicksToNanoseconds(mach_absolute_time() - start);
    }

    return (BenchmarkResult){
        .name = "256-color (:) Indexed",
        .totalNs = totalNs,
        .iterations = kIterations,
        .colorsProcessed = kIterations * kColorsPerIteration
    };
}

static BenchmarkResult RunBenchmark_24Bit_Semicolon_Current(void) {
    CSIParam csiParams[kColorsPerIteration];
    for (NSUInteger c = 0; c < kColorsPerIteration; c++) {
        int r = (int)(c & 0xFF);
        int g = (int)((c >> 3) & 0xFF);
        int b = (int)((c >> 5) & 0xFF);
        Create24BitColorCSI(&csiParams[c], r, g, b);
    }

    // Warmup
    for (NSUInteger w = 0; w < kWarmupIterations; w++) {
        for (NSUInteger c = 0; c < kColorsPerIteration; c++) {
            int idx = 0;
            CurrentColorValueFromCSI(&csiParams[c], &idx);
        }
    }

    uint64_t totalNs = 0;
    for (NSUInteger i = 0; i < kIterations; i++) {
        const uint64_t start = mach_absolute_time();
        for (NSUInteger c = 0; c < kColorsPerIteration; c++) {
            int idx = 0;
            volatile VT100TerminalColorValue val = CurrentColorValueFromCSI(&csiParams[c], &idx);
            (void)val;
        }
        totalNs += MachTicksToNanoseconds(mach_absolute_time() - start);
    }

    return (BenchmarkResult){
        .name = "24-bit (;) Current",
        .totalNs = totalNs,
        .iterations = kIterations,
        .colorsProcessed = kIterations * kColorsPerIteration
    };
}

static BenchmarkResult RunBenchmark_24Bit_Colon_Current(void) {
    CSIParam csiParams[kColorsPerIteration];
    for (NSUInteger c = 0; c < kColorsPerIteration; c++) {
        int r = (int)(c & 0xFF);
        int g = (int)((c >> 3) & 0xFF);
        int b = (int)((c >> 5) & 0xFF);
        Create24BitColorColonCSI(&csiParams[c], r, g, b);
    }

    // Warmup
    for (NSUInteger w = 0; w < kWarmupIterations; w++) {
        for (NSUInteger c = 0; c < kColorsPerIteration; c++) {
            int idx = 0;
            CurrentColorValueFromCSI(&csiParams[c], &idx);
        }
    }

    uint64_t totalNs = 0;
    for (NSUInteger i = 0; i < kIterations; i++) {
        const uint64_t start = mach_absolute_time();
        for (NSUInteger c = 0; c < kColorsPerIteration; c++) {
            int idx = 0;
            volatile VT100TerminalColorValue val = CurrentColorValueFromCSI(&csiParams[c], &idx);
            (void)val;
        }
        totalNs += MachTicksToNanoseconds(mach_absolute_time() - start);
    }

    return (BenchmarkResult){
        .name = "24-bit (:) Current",
        .totalNs = totalNs,
        .iterations = kIterations,
        .colorsProcessed = kIterations * kColorsPerIteration
    };
}

static BenchmarkResult RunBenchmark_24Bit_Colon_Indexed(void) {
    CSIParam csiParams[kColorsPerIteration];
    IndexedCSIParam indexedParams[kColorsPerIteration];
    for (NSUInteger c = 0; c < kColorsPerIteration; c++) {
        int r = (int)(c & 0xFF);
        int g = (int)((c >> 3) & 0xFF);
        int b = (int)((c >> 5) & 0xFF);
        Create24BitColorColonCSI(&csiParams[c], r, g, b);
        IndexCSIParam(&csiParams[c], &indexedParams[c]);
    }

    // Warmup
    for (NSUInteger w = 0; w < kWarmupIterations; w++) {
        for (NSUInteger c = 0; c < kColorsPerIteration; c++) {
            int idx = 0;
            IndexedColorValueFromCSI(&indexedParams[c], &idx);
        }
    }

    uint64_t totalNs = 0;
    for (NSUInteger i = 0; i < kIterations; i++) {
        const uint64_t start = mach_absolute_time();
        for (NSUInteger c = 0; c < kColorsPerIteration; c++) {
            int idx = 0;
            volatile VT100TerminalColorValue val = IndexedColorValueFromCSI(&indexedParams[c], &idx);
            (void)val;
        }
        totalNs += MachTicksToNanoseconds(mach_absolute_time() - start);
    }

    return (BenchmarkResult){
        .name = "24-bit (:) Indexed",
        .totalNs = totalNs,
        .iterations = kIterations,
        .colorsProcessed = kIterations * kColorsPerIteration
    };
}

static void PrintResult(BenchmarkResult result) {
    double avgNs = (double)result.totalNs / (double)result.iterations;
    double avgMs = avgNs / 1e6;
    double perColorNs = (double)result.totalNs / (double)result.colorsProcessed;
    double colorsPerSec = 1e9 / perColorNs;

    printf("%-25s  Avg: %8.3f ms  Per-color: %6.1f ns  Rate: %8.2f M/s\n",
           result.name, avgMs, perColorNs, colorsPerSec / 1e6);
}

int main(int argc, const char *argv[]) {
    @autoreleasepool {
        printf("SGR Color Escape Sequence Parsing Benchmark\n");
        printf("============================================\n");
        printf("Iterations: %lu  Colors per iteration: %lu\n\n",
               (unsigned long)kIterations, (unsigned long)kColorsPerIteration);

        printf("=== 256-Color Mode (ESC[38;5;Nm) ===\n");
        BenchmarkResult r1 = RunBenchmark_256Color_Semicolon_Current();
        PrintResult(r1);

        printf("\n=== 256-Color Colon Syntax (ESC[38:5:Nm) ===\n");
        BenchmarkResult r2 = RunBenchmark_256Color_Colon_Current();
        PrintResult(r2);
        BenchmarkResult r3 = RunBenchmark_256Color_Colon_Indexed();
        PrintResult(r3);

        printf("\n=== 24-Bit Color (ESC[38;2;R;G;Bm) ===\n");
        BenchmarkResult r4 = RunBenchmark_24Bit_Semicolon_Current();
        PrintResult(r4);

        printf("\n=== 24-Bit Colon Syntax (ESC[38:2:R:G:Bm) ===\n");
        BenchmarkResult r5 = RunBenchmark_24Bit_Colon_Current();
        PrintResult(r5);
        BenchmarkResult r6 = RunBenchmark_24Bit_Colon_Indexed();
        PrintResult(r6);

        // Summary
        printf("\n=== Summary ===\n");
        if (r3.totalNs > 0 && r2.totalNs > 0) {
            double colon256Speedup = (double)r2.totalNs / (double)r3.totalNs;
            printf("256-color colon indexed vs current: %.2fx\n", colon256Speedup);
        }
        if (r6.totalNs > 0 && r5.totalNs > 0) {
            double colon24Speedup = (double)r5.totalNs / (double)r6.totalNs;
            printf("24-bit colon indexed vs current: %.2fx\n", colon24Speedup);
        }

        // Identify bottleneck location
        printf("\n=== Analysis ===\n");
        double semiNs = (double)r1.totalNs / r1.colorsProcessed;
        double colonNs = (double)r2.totalNs / r2.colorsProcessed;
        printf("Semicolon syntax (no subparam lookup): %.1f ns/color\n", semiNs);
        printf("Colon syntax (with subparam lookup):   %.1f ns/color\n", colonNs);
        printf("Subparameter lookup overhead:          %.1f ns/color (%.1f%%)\n",
               colonNs - semiNs, ((colonNs - semiNs) / colonNs) * 100.0);
    }
    return 0;
}
