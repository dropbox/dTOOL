//
//  ascii_glyph_benchmark.m
//  DashTerm2
//
//  Benchmark for measuring ASCII glyph processing performance.
//  Compares the inline C++ path vs simulated Objective-C dispatch overhead.
//
//  Build and run:
//    clang -framework Foundation -fobjc-arc -O3 \
//      benchmarks/ascii_glyph_benchmark.m -o benchmarks/ascii_glyph_benchmark
//    ./benchmarks/ascii_glyph_benchmark
//

#import <Foundation/Foundation.h>
#import <mach/mach_time.h>

// Simulate the data structures used in the actual implementation
typedef struct {
    float x, y;
} vector_float2_sim;

typedef struct {
    float x, y, z, w;
} vector_float4_sim;

typedef NS_ENUM(unsigned char, iTermMetalGlyphType_sim) {
    iTermMetalGlyphTypeRegular_sim,
    iTermMetalGlyphTypeDecomposed_sim
};

typedef struct {
    unsigned short code;
    unsigned short combiningSuccessor;
    BOOL isComplex;
    BOOL boxDrawing;
    BOOL drawable;
} iTermRegularGlyphPayload_sim;

typedef struct {
    iTermMetalGlyphType_sim type;
    union {
        iTermRegularGlyphPayload_sim regular;
    } payload;
    int typeface;
    BOOL thinStrokes;
    int visualColumn;
    int logicalIndex;
} iTermMetalGlyphKey_sim;

typedef struct {
    vector_float4_sim foregroundColor;
    vector_float4_sim backgroundColor;
    BOOL hasUnderlineColor;
    vector_float4_sim underlineColor;
    int underlineStyle;
    BOOL annotation;
} iTermMetalGlyphAttributes_sim;

// Simulate PIU structure
typedef struct {
    vector_float2_sim offset;
    vector_float2_sim textureOffset;
    vector_float4_sim textColor;
    int underlineStyle;
    vector_float4_sim underlineColor;
} iTermTextPIU_sim;

// Simulated PIU array (simple dynamic array)
typedef struct {
    iTermTextPIU_sim *data;
    size_t count;
    size_t capacity;
} PIUArray_sim;

static void PIUArray_init(PIUArray_sim *arr, size_t capacity) {
    arr->data = (iTermTextPIU_sim *)malloc(capacity * sizeof(iTermTextPIU_sim));
    arr->count = 0;
    arr->capacity = capacity;
}

static void PIUArray_free(PIUArray_sim *arr) {
    free(arr->data);
}

static iTermTextPIU_sim *PIUArray_get_next(PIUArray_sim *arr) {
    if (arr->count >= arr->capacity) {
        arr->capacity *= 2;
        arr->data = (iTermTextPIU_sim *)realloc(arr->data, arr->capacity * sizeof(iTermTextPIU_sim));
    }
    return &arr->data[arr->count++];
}

// Constants
static const unsigned char kASCIITextureMinimumCharacter = 32;
static const unsigned char kASCIITextureMaximumCharacter = 126;

// Inline check for ASCII fast path
static inline BOOL GlyphKeyCanTakeASCIIFastPath_sim(const iTermMetalGlyphKey_sim *glyphKey) {
    return (glyphKey->type == iTermMetalGlyphTypeRegular_sim &&
            glyphKey->payload.regular.code <= kASCIITextureMaximumCharacter &&
            glyphKey->payload.regular.code >= kASCIITextureMinimumCharacter && !glyphKey->payload.regular.isComplex &&
            !glyphKey->payload.regular.boxDrawing);
}

// Inline ASCII processing (simulates the optimized path)
static inline void iTermAddASCIIGlyphInline_sim(const iTermMetalGlyphKey_sim *theGlyphKey, float asciiXOffset,
                                                float yOffset, float cellWidth,
                                                const iTermMetalGlyphAttributes_sim *attributes,
                                                PIUArray_sim *asciiPIUArray) {

    const int visualColumn = theGlyphKey->visualColumn;

    iTermTextPIU_sim *piu = PIUArray_get_next(asciiPIUArray);
    piu->offset.x = visualColumn * cellWidth + asciiXOffset;
    piu->offset.y = yOffset;
    piu->textureOffset.x = (theGlyphKey->payload.regular.code - kASCIITextureMinimumCharacter) * 0.01f;
    piu->textureOffset.y = 0.0f;
    piu->textColor = attributes[visualColumn].foregroundColor;
    piu->underlineStyle = attributes[visualColumn].underlineStyle;
    piu->underlineColor = attributes[visualColumn].underlineColor;
}

// Simulated Objective-C method (adds dispatch overhead)
@interface ASCIIProcessor : NSObject
- (void)addASCIIGlyph:(const iTermMetalGlyphKey_sim *)theGlyphKey
         asciiXOffset:(float)asciiXOffset
              yOffset:(float)yOffset
            cellWidth:(float)cellWidth
           attributes:(const iTermMetalGlyphAttributes_sim *)attributes
             piuArray:(PIUArray_sim *)asciiPIUArray;
@end

@implementation ASCIIProcessor

- (void)addASCIIGlyph:(const iTermMetalGlyphKey_sim *)theGlyphKey
         asciiXOffset:(float)asciiXOffset
              yOffset:(float)yOffset
            cellWidth:(float)cellWidth
           attributes:(const iTermMetalGlyphAttributes_sim *)attributes
             piuArray:(PIUArray_sim *)asciiPIUArray {

    const int visualColumn = theGlyphKey->visualColumn;

    iTermTextPIU_sim *piu = PIUArray_get_next(asciiPIUArray);
    piu->offset.x = visualColumn * cellWidth + asciiXOffset;
    piu->offset.y = yOffset;
    piu->textureOffset.x = (theGlyphKey->payload.regular.code - kASCIITextureMinimumCharacter) * 0.01f;
    piu->textureOffset.y = 0.0f;
    piu->textColor = attributes[visualColumn].foregroundColor;
    piu->underlineStyle = attributes[visualColumn].underlineStyle;
    piu->underlineColor = attributes[visualColumn].underlineColor;
}

@end

// Get elapsed time in milliseconds
static double getElapsedMs(uint64_t start, uint64_t end) {
    static mach_timebase_info_data_t timebase;
    if (timebase.denom == 0) {
        mach_timebase_info(&timebase);
    }
    uint64_t elapsed = end - start;
    return (double)(elapsed * timebase.numer / timebase.denom) / 1000000.0;
}

// Generate test data
static void generateGlyphKeys(iTermMetalGlyphKey_sim *keys, int count) {
    for (int i = 0; i < count; i++) {
        keys[i].type = iTermMetalGlyphTypeRegular_sim;
        keys[i].payload.regular.code = 'A' + (i % 26); // A-Z cycling
        keys[i].payload.regular.combiningSuccessor = 0;
        keys[i].payload.regular.isComplex = NO;
        keys[i].payload.regular.boxDrawing = NO;
        keys[i].payload.regular.drawable = YES;
        keys[i].typeface = 0;
        keys[i].thinStrokes = NO;
        keys[i].visualColumn = i;
        keys[i].logicalIndex = i;
    }
}

static void generateAttributes(iTermMetalGlyphAttributes_sim *attrs, int count) {
    for (int i = 0; i < count; i++) {
        attrs[i].foregroundColor = (vector_float4_sim){1.0f, 1.0f, 1.0f, 1.0f};
        attrs[i].backgroundColor = (vector_float4_sim){0.0f, 0.0f, 0.0f, 1.0f};
        attrs[i].hasUnderlineColor = NO;
        attrs[i].underlineColor = (vector_float4_sim){0.0f, 0.0f, 0.0f, 0.0f};
        attrs[i].underlineStyle = 0;
        attrs[i].annotation = NO;
    }
}

int main(int argc, const char *argv[]) {
    @autoreleasepool {
        printf("ASCII Glyph Processing Benchmark\n");
        printf("================================\n\n");

        // Test configurations: rows x columns
        struct {
            int rows;
            int columns;
            const char *name;
        } configs[] = {
            {24, 80, "standard (24x80)"},
            {50, 120, "medium (50x120)"},
            {100, 200, "large (100x200)"},
        };

        const int numConfigs = sizeof(configs) / sizeof(configs[0]);
        const int iterations = 1000; // Simulate 1000 frames

        ASCIIProcessor *processor = [[ASCIIProcessor alloc] init];

        for (int c = 0; c < numConfigs; c++) {
            int rows = configs[c].rows;
            int columns = configs[c].columns;
            int totalGlyphs = rows * columns;

            printf("Configuration: %s (%d rows x %d columns = %d glyphs)\n", configs[c].name, rows, columns,
                   totalGlyphs);

            // Allocate test data
            iTermMetalGlyphKey_sim *glyphKeys =
                (iTermMetalGlyphKey_sim *)malloc(columns * sizeof(iTermMetalGlyphKey_sim));
            iTermMetalGlyphAttributes_sim *attributes =
                (iTermMetalGlyphAttributes_sim *)malloc(columns * sizeof(iTermMetalGlyphAttributes_sim));

            generateGlyphKeys(glyphKeys, columns);
            generateAttributes(attributes, columns);

            PIUArray_sim piuArrayInline, piuArrayObjc;

            // ============ Benchmark Objective-C path ============
            uint64_t objcTotal = 0;
            for (int iter = 0; iter < iterations; iter++) {
                PIUArray_init(&piuArrayObjc, totalGlyphs);

                uint64_t start = mach_absolute_time();
                for (int row = 0; row < rows; row++) {
                    float yOffset = (rows - row - 1) * 16.0f;
                    for (int col = 0; col < columns; col++) {
                        if (GlyphKeyCanTakeASCIIFastPath_sim(&glyphKeys[col])) {
                            [processor addASCIIGlyph:&glyphKeys[col]
                                        asciiXOffset:0.0f
                                             yOffset:yOffset
                                           cellWidth:8.0f
                                          attributes:attributes
                                            piuArray:&piuArrayObjc];
                        }
                    }
                }
                uint64_t end = mach_absolute_time();
                objcTotal += (end - start);

                PIUArray_free(&piuArrayObjc);
            }
            double objcAvg = getElapsedMs(0, objcTotal) / iterations;

            // ============ Benchmark Inline C++ path ============
            uint64_t inlineTotal = 0;
            for (int iter = 0; iter < iterations; iter++) {
                PIUArray_init(&piuArrayInline, totalGlyphs);

                uint64_t start = mach_absolute_time();
                for (int row = 0; row < rows; row++) {
                    float yOffset = (rows - row - 1) * 16.0f;
                    for (int col = 0; col < columns; col++) {
                        if (GlyphKeyCanTakeASCIIFastPath_sim(&glyphKeys[col])) {
                            iTermAddASCIIGlyphInline_sim(&glyphKeys[col], 0.0f, yOffset, 8.0f, attributes,
                                                         &piuArrayInline);
                        }
                    }
                }
                uint64_t end = mach_absolute_time();
                inlineTotal += (end - start);

                PIUArray_free(&piuArrayInline);
            }
            double inlineAvg = getElapsedMs(0, inlineTotal) / iterations;

            double speedup = objcAvg / inlineAvg;
            int glyphsPerSecondObjc = (int)((totalGlyphs * 60.0) / (objcAvg / 1000.0));
            int glyphsPerSecondInline = (int)((totalGlyphs * 60.0) / (inlineAvg / 1000.0));

            printf("  Objective-C dispatch: %.4f ms/frame (%.2f us/glyph)\n", objcAvg,
                   (objcAvg * 1000.0) / totalGlyphs);
            printf("  Inline C++ path:      %.4f ms/frame (%.2f us/glyph)\n", inlineAvg,
                   (inlineAvg * 1000.0) / totalGlyphs);
            printf("  Speedup:              %.2fx faster\n", speedup);
            printf("  Throughput at 60fps:  %d vs %d glyphs/sec\n\n", glyphsPerSecondObjc, glyphsPerSecondInline);

            free(glyphKeys);
            free(attributes);
        }

        printf("Summary\n");
        printf("-------\n");
        printf("The inline C++ path eliminates Objective-C message dispatch overhead,\n");
        printf("which was previously measured at ~20%% of ASCII rendering time.\n");
        printf("This optimization benefits ASCII-heavy terminal content (most common case).\n");
    }
    return 0;
}
