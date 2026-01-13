//
//  nonascii_glyph_benchmark.m
//  DashTerm2
//
//  Benchmark for measuring non-ASCII glyph processing performance.
//  Compares the inline C++ path vs simulated Objective-C dispatch overhead.
//
//  Build and run:
//    clang -framework Foundation -fobjc-arc -O3 \
//      benchmarks/nonascii_glyph_benchmark.m -o benchmarks/nonascii_glyph_benchmark
//    ./benchmarks/nonascii_glyph_benchmark
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

// Simulated texture page lookup (simplified)
typedef struct {
    int part;
    int page_id;
    float origin_x;
    float origin_y;
} GlyphEntry_sim;

typedef struct {
    GlyphEntry_sim entries[4]; // Simulate up to 4 parts per glyph
    int entry_count;
} GlyphEntryList_sim;

// Simulate a texture page collection with a simple hash table
typedef struct {
    GlyphEntryList_sim *entries;
    size_t capacity;
} TexturePageCollection_sim;

static void TexturePageCollection_init(TexturePageCollection_sim *col, size_t capacity) {
    col->entries = (GlyphEntryList_sim *)calloc(capacity, sizeof(GlyphEntryList_sim));
    col->capacity = capacity;
}

static void TexturePageCollection_free(TexturePageCollection_sim *col) {
    free(col->entries);
}

static GlyphEntryList_sim *TexturePageCollection_find(TexturePageCollection_sim *col, int code) {
    size_t index = code % col->capacity;
    GlyphEntryList_sim *entry = &col->entries[index];
    if (entry->entry_count == 0) {
        // Simulate creation - typically 1-4 entries for complex glyphs
        entry->entry_count = 1 + (code % 3); // 1-3 entries
        for (int i = 0; i < entry->entry_count; i++) {
            entry->entries[i].part = i;
            entry->entries[i].page_id = code / 100;
            entry->entries[i].origin_x = (code % 100) * 0.01f;
            entry->entries[i].origin_y = (code / 100) * 0.01f;
        }
    }
    return entry;
}

// Inline non-ASCII processing (simulates the optimized path)
static inline void iTermAddNonASCIIGlyphInline_sim(const iTermMetalGlyphKey_sim *theGlyphKey, float yOffset,
                                                   float cellWidth, float cellHeight, float glyphWidth,
                                                   float glyphHeight, const iTermMetalGlyphAttributes_sim *attributes,
                                                   BOOL inMarkedRange, BOOL allowUnderline,
                                                   TexturePageCollection_sim *textures, PIUArray_sim *piuArray) {
    const int visualIndex = theGlyphKey->visualColumn;

    // Simulate texture lookup
    GlyphEntryList_sim *entries = TexturePageCollection_find(textures, theGlyphKey->payload.regular.code);
    if (!entries || entries->entry_count == 0) {
        return;
    }

    for (int i = 0; i < entries->entry_count; i++) {
        GlyphEntry_sim *entry = &entries->entries[i];

        iTermTextPIU_sim *piu = PIUArray_get_next(piuArray);

        // Build the PIU (similar to actual implementation)
        const int part = entry->part;
        const int dx = part % 3; // Simplified iTermImagePartDX
        const int dy = part / 3; // Simplified iTermImagePartDY

        piu->offset.x = theGlyphKey->visualColumn * cellWidth + dx * glyphWidth;
        piu->offset.y = -dy * glyphHeight + yOffset;
        piu->textureOffset.x = entry->origin_x;
        piu->textureOffset.y = entry->origin_y;
        piu->textColor = attributes[visualIndex].foregroundColor;

        // Handle underline
        if (attributes[visualIndex].annotation) {
            piu->underlineStyle = 1; // Single underline
            piu->underlineColor = (vector_float4_sim){1.0f, 1.0f, 0.0f, 1.0f};
        } else if (inMarkedRange) {
            piu->underlineStyle = 1;
            piu->underlineColor = attributes[visualIndex].foregroundColor;
        } else {
            piu->underlineStyle = attributes[visualIndex].underlineStyle;
            if (attributes[visualIndex].hasUnderlineColor) {
                piu->underlineColor = attributes[visualIndex].underlineColor;
            } else {
                piu->underlineColor = attributes[visualIndex].foregroundColor;
            }
        }

        // Don't underline non-center parts
        if (part != 0 && part != 1) {
            piu->underlineStyle = 0;
        }
    }
}

// Simulated Objective-C method (adds dispatch overhead)
@interface NonASCIIProcessor : NSObject {
    TexturePageCollection_sim *_textures;
}
- (instancetype)initWithTextures:(TexturePageCollection_sim *)textures;
- (void)addNonASCIIGlyph:(const iTermMetalGlyphKey_sim *)theGlyphKey
                 yOffset:(float)yOffset
               cellWidth:(float)cellWidth
              cellHeight:(float)cellHeight
              glyphWidth:(float)glyphWidth
             glyphHeight:(float)glyphHeight
              attributes:(const iTermMetalGlyphAttributes_sim *)attributes
           inMarkedRange:(BOOL)inMarkedRange
          allowUnderline:(BOOL)allowUnderline
                piuArray:(PIUArray_sim *)piuArray;
@end

@implementation NonASCIIProcessor

- (instancetype)initWithTextures:(TexturePageCollection_sim *)textures {
    self = [super init];
    if (self) {
        _textures = textures;
    }
    return self;
}

- (void)addNonASCIIGlyph:(const iTermMetalGlyphKey_sim *)theGlyphKey
                 yOffset:(float)yOffset
               cellWidth:(float)cellWidth
              cellHeight:(float)cellHeight
              glyphWidth:(float)glyphWidth
             glyphHeight:(float)glyphHeight
              attributes:(const iTermMetalGlyphAttributes_sim *)attributes
           inMarkedRange:(BOOL)inMarkedRange
          allowUnderline:(BOOL)allowUnderline
                piuArray:(PIUArray_sim *)piuArray {
    const int visualIndex = theGlyphKey->visualColumn;

    // Simulate texture lookup
    GlyphEntryList_sim *entries = TexturePageCollection_find(_textures, theGlyphKey->payload.regular.code);
    if (!entries || entries->entry_count == 0) {
        return;
    }

    for (int i = 0; i < entries->entry_count; i++) {
        GlyphEntry_sim *entry = &entries->entries[i];

        iTermTextPIU_sim *piu = PIUArray_get_next(piuArray);

        const int part = entry->part;
        const int dx = part % 3;
        const int dy = part / 3;

        piu->offset.x = theGlyphKey->visualColumn * cellWidth + dx * glyphWidth;
        piu->offset.y = -dy * glyphHeight + yOffset;
        piu->textureOffset.x = entry->origin_x;
        piu->textureOffset.y = entry->origin_y;
        piu->textColor = attributes[visualIndex].foregroundColor;

        if (attributes[visualIndex].annotation) {
            piu->underlineStyle = 1;
            piu->underlineColor = (vector_float4_sim){1.0f, 1.0f, 0.0f, 1.0f};
        } else if (inMarkedRange) {
            piu->underlineStyle = 1;
            piu->underlineColor = attributes[visualIndex].foregroundColor;
        } else {
            piu->underlineStyle = attributes[visualIndex].underlineStyle;
            if (attributes[visualIndex].hasUnderlineColor) {
                piu->underlineColor = attributes[visualIndex].underlineColor;
            } else {
                piu->underlineColor = attributes[visualIndex].foregroundColor;
            }
        }

        if (part != 0 && part != 1) {
            piu->underlineStyle = 0;
        }
    }
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

// Generate non-ASCII test data (Unicode characters)
static void generateNonASCIIGlyphKeys(iTermMetalGlyphKey_sim *keys, int count) {
    for (int i = 0; i < count; i++) {
        keys[i].type = iTermMetalGlyphTypeRegular_sim;
        // Use Unicode characters (Chinese, Japanese, emoji, etc.)
        // Start from 0x4E00 (CJK Unified Ideographs)
        keys[i].payload.regular.code = 0x4E00 + (i % 1000);
        keys[i].payload.regular.combiningSuccessor = 0;
        keys[i].payload.regular.isComplex = YES; // Non-ASCII are complex
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
        printf("Non-ASCII Glyph Processing Benchmark\n");
        printf("=====================================\n\n");

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

        TexturePageCollection_sim textures;
        TexturePageCollection_init(&textures, 4096);

        NonASCIIProcessor *processor = [[NonASCIIProcessor alloc] initWithTextures:&textures];

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

            generateNonASCIIGlyphKeys(glyphKeys, columns);
            generateAttributes(attributes, columns);

            PIUArray_sim piuArrayInline, piuArrayObjc;

            // ============ Benchmark Objective-C path ============
            uint64_t objcTotal = 0;
            for (int iter = 0; iter < iterations; iter++) {
                PIUArray_init(&piuArrayObjc, totalGlyphs * 3); // Allow for multi-part glyphs

                uint64_t start = mach_absolute_time();
                for (int row = 0; row < rows; row++) {
                    float yOffset = (rows - row - 1) * 16.0f;
                    for (int col = 0; col < columns; col++) {
                        [processor addNonASCIIGlyph:&glyphKeys[col]
                                            yOffset:yOffset
                                          cellWidth:8.0f
                                         cellHeight:16.0f
                                         glyphWidth:10.0f
                                        glyphHeight:18.0f
                                         attributes:attributes
                                      inMarkedRange:NO
                                     allowUnderline:YES
                                           piuArray:&piuArrayObjc];
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
                PIUArray_init(&piuArrayInline, totalGlyphs * 3);

                uint64_t start = mach_absolute_time();
                for (int row = 0; row < rows; row++) {
                    float yOffset = (rows - row - 1) * 16.0f;
                    for (int col = 0; col < columns; col++) {
                        iTermAddNonASCIIGlyphInline_sim(&glyphKeys[col], yOffset, 8.0f, 16.0f, 10.0f, 18.0f, attributes,
                                                        NO, YES, &textures, &piuArrayInline);
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

        TexturePageCollection_free(&textures);

        printf("Summary\n");
        printf("-------\n");
        printf("The inline C++ path eliminates Objective-C message dispatch overhead for\n");
        printf("non-ASCII glyphs (CJK characters, emoji, etc.). This is particularly\n");
        printf("important for international terminal content where non-ASCII characters\n");
        printf("are common. The optimization follows the same pattern as #132 for ASCII.\n");
    }
    return 0;
}
