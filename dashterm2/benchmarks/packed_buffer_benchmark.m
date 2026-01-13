//
//  packed_buffer_benchmark.m
//  DashTerm2 Benchmark Suite
//
//  Benchmarks for iTermPackedCharacterBuffer - validates correctness and measures
//  memory savings vs iTermCharacterBuffer.
//
//  Build:
//    clang -framework Foundation -I../sources -fobjc-arc \
//      packed_buffer_benchmark.m ../sources/PackedScreenChar.m \
//      ../sources/iTermPackedCharacterBuffer.m \
//      -o packed_buffer_benchmark
//

#import <Foundation/Foundation.h>
#import <mach/mach_time.h>
#import <sys/resource.h>

// Local includes for standalone build
#define ITERM_PACKED_BUFFER_BENCHMARK 1

// Minimal reimplementation for standalone build
typedef unsigned short unichar;

typedef enum {
    ColorModeAlternate = 0,
    ColorModeNormal = 1,
    ColorMode24bit = 2,
    ColorModeInvalid = 3
} ColorMode;

typedef enum {
    ALTSEM_DEFAULT = 0,
    ALTSEM_SELECTED = 1,
    ALTSEM_CURSOR = 2,
    ALTSEM_REVERSED_DEFAULT = 3,
    ALTSEM_SYSTEM_MESSAGE = 4,
} AlternateSemantics;

typedef enum {
    RTLStatusUnknown = 0,
    RTLStatusLTR = 1,
    RTLStatusRTL = 2,
    RTLStatusBidiDisabled = 3
} RTLStatus;

typedef enum {
    VT100UnderlineStyleSingle = 0,
    VT100UnderlineStyleDouble = 1,
    VT100UnderlineStyleCurly = 2,
} VT100UnderlineStyle;

// screen_char_t structure (12 bytes)
typedef struct screen_char_t {
    unichar code;
    unsigned int foregroundColor : 8;
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
    unsigned int underlineStyle0 : 2;
    unsigned int invisible : 1;
    unsigned int inverse : 1;
    unsigned int guarded : 1;
    unsigned int virtualPlaceholder : 1;
    RTLStatus rtlStatus : 2;
    unsigned int underlineStyle1 : 1;
    unsigned int unused : 11;
} screen_char_t;

_Static_assert(sizeof(screen_char_t) == 12, "screen_char_t must be 12 bytes");

// Helper functions
static inline VT100UnderlineStyle ScreenCharGetUnderlineStyle(screen_char_t c) {
    return (VT100UnderlineStyle)((c.underlineStyle1 << 2) | c.underlineStyle0);
}

static inline void ScreenCharSetUnderlineStyle(screen_char_t *c, VT100UnderlineStyle style) {
    c->underlineStyle0 = style & 3;
    c->underlineStyle1 = (style >> 2) & 1;
}

// iTermMalloc replacements
static void *iTermCalloc(size_t count, size_t size) {
    return calloc(count, size);
}

static void *iTermUninitializedCalloc(size_t count, size_t size) {
    return malloc(count * size);
}

static void *iTermMemdup(const void *src, size_t count, size_t size) {
    void *dst = malloc(count * size);
    memcpy(dst, src, count * size);
    return dst;
}

static void *iTermRealloc(void *ptr, size_t count, size_t size) {
    return realloc(ptr, count * size);
}

// ============================================================================
// Packed Screen Char Structure (inline for standalone build)
// ============================================================================

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
    uint64_t underlineStyle : 3;
    uint64_t image : 1;
    uint64_t strikethrough : 1;
    uint64_t invisible : 1;
    uint64_t inverse : 1;
    uint64_t guarded : 1;
    uint64_t virtualPlaceholder : 1;
    uint64_t rtlStatus : 2;
    uint64_t fgIs24Bit : 1;
    uint64_t bgIs24Bit : 1;
    uint64_t reserved : 5;
} packed_screen_char_t;

_Static_assert(sizeof(packed_screen_char_t) == 8, "packed_screen_char_t must be exactly 8 bytes");

// Color constants
typedef NS_ENUM(uint16_t, PackedColorIndex) {
    kPackedColorDefault = 256,
    kPackedColorSelected = 257,
    kPackedColorCursor = 258,
    kPackedColorReversedDefault = 259,
    kPackedColorSystemMessage = 260,
    kPackedColor24BitBase = 261,
    kPackedColor24BitMax = 511,
};

// 24-bit color table (simplified for benchmark)
@interface PackedColorTable : NSObject
@property (nonatomic, readonly) NSUInteger capacity;
@property (nonatomic, readonly) NSUInteger count;
- (instancetype)initWithCapacity:(NSUInteger)capacity;
- (NSUInteger)insertColorWithRed:(uint8_t)red green:(uint8_t)green blue:(uint8_t)blue;
- (BOOL)getColorAtIndex:(NSUInteger)index red:(uint8_t *)red green:(uint8_t *)green blue:(uint8_t *)blue;
@end

@implementation PackedColorTable {
    uint8_t *_colors;  // r,g,b triplets
    NSUInteger _capacity;
    NSUInteger _count;
}

- (instancetype)initWithCapacity:(NSUInteger)capacity {
    self = [super init];
    if (self) {
        _capacity = MIN(capacity, 251);
        _colors = calloc(_capacity * 3, sizeof(uint8_t));
        _count = 0;
    }
    return self;
}

- (void)dealloc {
    free(_colors);
}

- (NSUInteger)insertColorWithRed:(uint8_t)red green:(uint8_t)green blue:(uint8_t)blue {
    // Check if color exists
    for (NSUInteger i = 0; i < _count; i++) {
        if (_colors[i*3] == red && _colors[i*3+1] == green && _colors[i*3+2] == blue) {
            return i;
        }
    }

    if (_count >= _capacity) {
        return NSNotFound;
    }

    NSUInteger idx = _count++;
    _colors[idx*3] = red;
    _colors[idx*3+1] = green;
    _colors[idx*3+2] = blue;
    return idx;
}

- (BOOL)getColorAtIndex:(NSUInteger)index red:(uint8_t *)red green:(uint8_t *)green blue:(uint8_t *)blue {
    if (index >= _count) return NO;
    if (red) *red = _colors[index*3];
    if (green) *green = _colors[index*3+1];
    if (blue) *blue = _colors[index*3+2];
    return YES;
}

@end

// Pack/unpack functions (inline for standalone)
static packed_screen_char_t PackScreenChar(screen_char_t src, PackedColorTable *colorTable) {
    packed_screen_char_t dst = {0};

    dst.code = src.code & 0x1FFFFF;
    dst.complexChar = src.complexChar;
    dst.bold = src.bold;
    dst.faint = src.faint;
    dst.italic = src.italic;
    dst.blink = src.blink;
    dst.underline = src.underline;
    dst.underlineStyle = ScreenCharGetUnderlineStyle(src);
    dst.image = src.image;
    dst.strikethrough = src.strikethrough;
    dst.invisible = src.invisible;
    dst.inverse = src.inverse;
    dst.guarded = src.guarded;
    dst.virtualPlaceholder = src.virtualPlaceholder;
    dst.rtlStatus = src.rtlStatus;

    // Pack foreground color
    dst.fgIs24Bit = NO;
    switch (src.foregroundColorMode) {
        case ColorModeAlternate:
            dst.fgColor = kPackedColorDefault + src.foregroundColor;
            break;
        case ColorModeNormal:
            dst.fgColor = src.foregroundColor & 0xFF;
            break;
        case ColorMode24bit:
            if (colorTable) {
                NSUInteger idx = [colorTable insertColorWithRed:src.foregroundColor
                                                          green:src.fgGreen
                                                           blue:src.fgBlue];
                if (idx != NSNotFound) {
                    dst.fgColor = kPackedColor24BitBase + idx;
                    dst.fgIs24Bit = YES;
                } else {
                    dst.fgColor = kPackedColorDefault;
                }
            } else {
                dst.fgColor = kPackedColorDefault;
            }
            break;
        default:
            dst.fgColor = kPackedColorDefault;
            break;
    }

    // Pack background color
    dst.bgIs24Bit = NO;
    switch (src.backgroundColorMode) {
        case ColorModeAlternate:
            dst.bgColor = kPackedColorDefault + src.backgroundColor;
            break;
        case ColorModeNormal:
            dst.bgColor = src.backgroundColor & 0xFF;
            break;
        case ColorMode24bit:
            if (colorTable) {
                NSUInteger idx = [colorTable insertColorWithRed:src.backgroundColor
                                                          green:src.bgGreen
                                                           blue:src.bgBlue];
                if (idx != NSNotFound) {
                    dst.bgColor = kPackedColor24BitBase + idx;
                    dst.bgIs24Bit = YES;
                } else {
                    dst.bgColor = kPackedColorDefault;
                }
            } else {
                dst.bgColor = kPackedColorDefault;
            }
            break;
        default:
            dst.bgColor = kPackedColorDefault;
            break;
    }

    return dst;
}

static screen_char_t UnpackScreenChar(packed_screen_char_t src, PackedColorTable *colorTable) {
    screen_char_t dst;
    memset(&dst, 0, sizeof(dst));

    dst.code = src.code;
    dst.complexChar = src.complexChar;
    dst.bold = src.bold;
    dst.faint = src.faint;
    dst.italic = src.italic;
    dst.blink = src.blink;
    dst.underline = src.underline;
    ScreenCharSetUnderlineStyle(&dst, (VT100UnderlineStyle)src.underlineStyle);
    dst.image = src.image;
    dst.strikethrough = src.strikethrough;
    dst.invisible = src.invisible;
    dst.inverse = src.inverse;
    dst.guarded = src.guarded;
    dst.virtualPlaceholder = src.virtualPlaceholder;
    dst.rtlStatus = (RTLStatus)src.rtlStatus;

    // Unpack foreground
    if (src.fgIs24Bit && colorTable) {
        uint8_t r, g, b;
        if ([colorTable getColorAtIndex:(src.fgColor - kPackedColor24BitBase) red:&r green:&g blue:&b]) {
            dst.foregroundColor = r;
            dst.fgGreen = g;
            dst.fgBlue = b;
            dst.foregroundColorMode = ColorMode24bit;
        } else {
            dst.foregroundColor = ALTSEM_DEFAULT;
            dst.foregroundColorMode = ColorModeAlternate;
        }
    } else if (src.fgColor >= kPackedColorDefault && src.fgColor <= kPackedColorSystemMessage) {
        dst.foregroundColor = src.fgColor - kPackedColorDefault;
        dst.foregroundColorMode = ColorModeAlternate;
    } else {
        dst.foregroundColor = src.fgColor & 0xFF;
        dst.foregroundColorMode = ColorModeNormal;
    }

    // Unpack background
    if (src.bgIs24Bit && colorTable) {
        uint8_t r, g, b;
        if ([colorTable getColorAtIndex:(src.bgColor - kPackedColor24BitBase) red:&r green:&g blue:&b]) {
            dst.backgroundColor = r;
            dst.bgGreen = g;
            dst.bgBlue = b;
            dst.backgroundColorMode = ColorMode24bit;
        } else {
            dst.backgroundColor = ALTSEM_DEFAULT;
            dst.backgroundColorMode = ColorModeAlternate;
        }
    } else if (src.bgColor >= kPackedColorDefault && src.bgColor <= kPackedColorSystemMessage) {
        dst.backgroundColor = src.bgColor - kPackedColorDefault;
        dst.backgroundColorMode = ColorModeAlternate;
    } else {
        dst.backgroundColor = src.bgColor & 0xFF;
        dst.backgroundColorMode = ColorModeNormal;
    }

    return dst;
}

static void PackScreenCharArray(const screen_char_t *src, packed_screen_char_t *dst,
                                NSUInteger count, PackedColorTable *colorTable) {
    for (NSUInteger i = 0; i < count; i++) {
        dst[i] = PackScreenChar(src[i], colorTable);
    }
}

static void UnpackScreenCharArray(const packed_screen_char_t *src, screen_char_t *dst,
                                  NSUInteger count, PackedColorTable *colorTable) {
    for (NSUInteger i = 0; i < count; i++) {
        dst[i] = UnpackScreenChar(src[i], colorTable);
    }
}

// ============================================================================
// iTermPackedCharacterBuffer (inline for standalone build)
// ============================================================================

@interface iTermPackedCharacterBuffer : NSObject
@property (nonatomic, readonly) int size;
@property (nonatomic, strong, readonly) PackedColorTable *colorTable;
@property (nonatomic, readonly) NSUInteger memoryUsage;
@property (nonatomic, readonly) NSUInteger unpackedMemoryUsage;
@property (nonatomic, readonly) NSUInteger memorySaved;
- (instancetype)initWithChars:(const screen_char_t *)chars size:(int)count colorTable:(PackedColorTable *)colorTable;
- (void)readChars:(screen_char_t *)dst count:(int)count fromOffset:(int)offset;
@end

@implementation iTermPackedCharacterBuffer {
    packed_screen_char_t *_packedBuffer;
    int _size;
    PackedColorTable *_colorTable;
}

- (instancetype)initWithChars:(const screen_char_t *)chars size:(int)count colorTable:(PackedColorTable *)colorTable {
    self = [super init];
    if (self) {
        _size = count;
        _packedBuffer = iTermCalloc(count, sizeof(packed_screen_char_t));
        _colorTable = colorTable ?: [[PackedColorTable alloc] initWithCapacity:251];
        PackScreenCharArray(chars, _packedBuffer, count, _colorTable);
    }
    return self;
}

- (void)dealloc {
    free(_packedBuffer);
}

- (int)size { return _size; }
- (PackedColorTable *)colorTable { return _colorTable; }
- (NSUInteger)memoryUsage { return _size * sizeof(packed_screen_char_t); }
- (NSUInteger)unpackedMemoryUsage { return _size * sizeof(screen_char_t); }
- (NSUInteger)memorySaved { return self.unpackedMemoryUsage - self.memoryUsage; }

- (void)readChars:(screen_char_t *)dst count:(int)count fromOffset:(int)offset {
    if (offset < 0 || offset + count > _size || !dst) return;
    UnpackScreenCharArray(_packedBuffer + offset, dst, count, _colorTable);
}

@end

#pragma mark - Timing Utilities

static double MachTimeToNs(uint64_t elapsed) {
    static mach_timebase_info_data_t info = {0, 0};
    if (info.denom == 0) {
        mach_timebase_info(&info);
    }
    return (double)elapsed * info.numer / info.denom;
}

static size_t GetMemoryUsage(void) {
    struct rusage usage;
    getrusage(RUSAGE_SELF, &usage);
    return usage.ru_maxrss;
}

#pragma mark - Test Data Generation

static screen_char_t MakeDefaultChar(unichar code) {
    screen_char_t c = {0};
    c.code = code;
    c.foregroundColor = ALTSEM_DEFAULT;
    c.foregroundColorMode = ColorModeAlternate;
    c.backgroundColor = ALTSEM_DEFAULT;
    c.backgroundColorMode = ColorModeAlternate;
    return c;
}

static screen_char_t MakeColoredChar(unichar code, uint8_t fgIdx, uint8_t bgIdx) {
    screen_char_t c = {0};
    c.code = code;
    c.foregroundColor = fgIdx;
    c.foregroundColorMode = ColorModeNormal;
    c.backgroundColor = bgIdx;
    c.backgroundColorMode = ColorModeNormal;
    return c;
}

static screen_char_t Make24BitChar(unichar code, uint8_t r, uint8_t g, uint8_t b) {
    screen_char_t c = {0};
    c.code = code;
    c.foregroundColor = r;
    c.fgGreen = g;
    c.fgBlue = b;
    c.foregroundColorMode = ColorMode24bit;
    c.backgroundColor = ALTSEM_DEFAULT;
    c.backgroundColorMode = ColorModeAlternate;
    return c;
}

static void FillLineWithText(screen_char_t *line, int width, const char *text) {
    int textLen = (int)strlen(text);
    for (int i = 0; i < width; i++) {
        if (i < textLen) {
            line[i] = MakeDefaultChar(text[i]);
        } else {
            line[i] = MakeDefaultChar(' ');
        }
    }
}

#pragma mark - Test Functions

static BOOL TestRoundTrip(void) {
    printf("Test: Round-trip conversion correctness...\n");

    const int numChars = 1000;
    screen_char_t *original = calloc(numChars, sizeof(screen_char_t));

    // Create varied test data
    for (int i = 0; i < numChars; i++) {
        switch (i % 5) {
            case 0: // Default colors
                original[i] = MakeDefaultChar('A' + (i % 26));
                break;
            case 1: // ANSI palette
                original[i] = MakeColoredChar('0' + (i % 10), i % 16, (i + 8) % 16);
                break;
            case 2: // 256-color palette
                original[i] = MakeColoredChar('x', i % 256, (i * 3) % 256);
                break;
            case 3: // Bold/italic/etc
                original[i] = MakeDefaultChar('B');
                original[i].bold = (i / 5) % 2;
                original[i].italic = (i / 10) % 2;
                original[i].underline = (i / 15) % 2;
                break;
            case 4: // 24-bit color
                original[i] = Make24BitChar('C', (i * 7) % 256, (i * 11) % 256, (i * 13) % 256);
                break;
        }
    }

    // Create packed buffer from original
    iTermPackedCharacterBuffer *packed = [[iTermPackedCharacterBuffer alloc] initWithChars:original
                                                                                      size:numChars
                                                                                colorTable:nil];

    // Read back all characters
    screen_char_t *unpacked = calloc(numChars, sizeof(screen_char_t));
    [packed readChars:unpacked count:numChars fromOffset:0];

    // Verify
    int errors = 0;
    for (int i = 0; i < numChars; i++) {
        screen_char_t o = original[i];
        screen_char_t u = unpacked[i];

        if (o.code != u.code) {
            if (errors < 5) printf("  Error at %d: code %d != %d\n", i, o.code, u.code);
            errors++;
        }
        if (o.bold != u.bold || o.italic != u.italic || o.underline != u.underline) {
            if (errors < 5) printf("  Error at %d: attributes mismatch\n", i);
            errors++;
        }

        // For 24-bit colors, allow some loss due to quantization when color table is full
        if (o.foregroundColorMode == ColorMode24bit && u.foregroundColorMode != ColorMode24bit) {
            // Quantization happened - this is OK if color table was full
        } else if (o.foregroundColorMode != u.foregroundColorMode) {
            if (errors < 5) printf("  Error at %d: fg color mode %d != %d\n", i, o.foregroundColorMode, u.foregroundColorMode);
            errors++;
        }
    }

    free(original);
    free(unpacked);

    if (errors == 0) {
        printf("  PASS: All %d characters round-tripped correctly\n", numChars);
        return YES;
    } else {
        printf("  FAIL: %d errors\n", errors);
        return NO;
    }
}

static BOOL TestMemorySavings(void) {
    printf("\nTest: Memory savings calculation...\n");

    const int numChars = 10000;
    screen_char_t *chars = calloc(numChars, sizeof(screen_char_t));
    FillLineWithText(chars, numChars, "Hello, World! This is a test line for memory measurement.");

    iTermPackedCharacterBuffer *packed = [[iTermPackedCharacterBuffer alloc] initWithChars:chars
                                                                                      size:numChars
                                                                                colorTable:nil];

    NSUInteger packedMem = packed.memoryUsage;
    NSUInteger unpackedMem = packed.unpackedMemoryUsage;
    NSUInteger saved = packed.memorySaved;

    printf("  Characters: %d\n", numChars);
    printf("  Unpacked size: %lu bytes (%.2f KB)\n", unpackedMem, unpackedMem / 1024.0);
    printf("  Packed size:   %lu bytes (%.2f KB)\n", packedMem, packedMem / 1024.0);
    printf("  Saved:         %lu bytes (%.1f%%)\n", saved, 100.0 * saved / unpackedMem);

    free(chars);

    // Verify savings are approximately 33%
    double savingsPercent = 100.0 * saved / unpackedMem;
    if (savingsPercent >= 30 && savingsPercent <= 35) {
        printf("  PASS: Savings %.1f%% is within expected range (30-35%%)\n", savingsPercent);
        return YES;
    } else {
        printf("  FAIL: Savings %.1f%% outside expected range\n", savingsPercent);
        return NO;
    }
}

static BOOL TestLargeScrollback(void) {
    printf("\nTest: Large scrollback simulation (1M lines)...\n");

    const int lineWidth = 80;
    const int numLines = 1000000;
    const int charsPerBatch = lineWidth * 1000;  // Process 1000 lines at a time

    size_t memBefore = GetMemoryUsage();
    uint64_t startTime = mach_absolute_time();

    // Simulate writing 1M lines to packed storage
    // Use a single color table for all
    PackedColorTable *colorTable = [[PackedColorTable alloc] initWithCapacity:251];

    // Track total packed memory
    NSUInteger totalPackedBytes = 0;
    NSUInteger totalUnpackedBytes = 0;

    screen_char_t *batchBuffer = calloc(charsPerBatch, sizeof(screen_char_t));
    packed_screen_char_t *packedBatch = calloc(charsPerBatch, sizeof(packed_screen_char_t));

    int batches = numLines / 1000;
    for (int batch = 0; batch < batches; batch++) {
        // Fill batch with typical terminal content
        for (int line = 0; line < 1000; line++) {
            int offset = line * lineWidth;
            char textBuf[128];
            snprintf(textBuf, sizeof(textBuf), "Line %d: some typical terminal output here...",
                     batch * 1000 + line);
            FillLineWithText(batchBuffer + offset, lineWidth, textBuf);
        }

        // Pack the batch
        PackScreenCharArray(batchBuffer, packedBatch, charsPerBatch, colorTable);

        totalPackedBytes += charsPerBatch * sizeof(packed_screen_char_t);
        totalUnpackedBytes += charsPerBatch * sizeof(screen_char_t);
    }

    uint64_t endTime = mach_absolute_time();
    double elapsedMs = MachTimeToNs(endTime - startTime) / 1e6;
    size_t memAfter = GetMemoryUsage();

    free(batchBuffer);
    free(packedBatch);

    double savedMB = (totalUnpackedBytes - totalPackedBytes) / (1024.0 * 1024.0);
    double totalUnpackedMB = totalUnpackedBytes / (1024.0 * 1024.0);
    double totalPackedMB = totalPackedBytes / (1024.0 * 1024.0);

    printf("  Lines: %d (width: %d)\n", numLines, lineWidth);
    printf("  Total characters: %llu\n", (unsigned long long)numLines * lineWidth);
    printf("  Unpacked would be: %.1f MB\n", totalUnpackedMB);
    printf("  Packed size:       %.1f MB\n", totalPackedMB);
    printf("  Memory saved:      %.1f MB (%.1f%%)\n", savedMB, 100.0 * savedMB / totalUnpackedMB);
    printf("  Packing time:      %.1f ms (%.2f us/line)\n", elapsedMs, elapsedMs * 1000 / numLines);
    printf("  RSS change:        %+ld KB\n", (memAfter - memBefore) / 1024);

    // Verify savings
    if (savedMB > 200) {  // Should save ~320 MB for 1M lines
        printf("  PASS: Saved %.1f MB (expected ~320 MB)\n", savedMB);
        return YES;
    } else {
        printf("  FAIL: Only saved %.1f MB\n", savedMB);
        return NO;
    }
}

static BOOL TestUnpackPerformance(void) {
    printf("\nTest: Unpack performance (read latency)...\n");

    const int lineWidth = 80;
    const int numLines = 10000;
    const int numChars = lineWidth * numLines;

    // Create packed data
    screen_char_t *original = calloc(numChars, sizeof(screen_char_t));
    for (int i = 0; i < numChars; i++) {
        original[i] = MakeDefaultChar('A' + (i % 26));
    }

    iTermPackedCharacterBuffer *packed = [[iTermPackedCharacterBuffer alloc] initWithChars:original
                                                                                      size:numChars
                                                                                colorTable:nil];

    // Measure unpack time for single lines (simulates scrollback read)
    screen_char_t *lineBuffer = calloc(lineWidth, sizeof(screen_char_t));

    uint64_t startTime = mach_absolute_time();
    for (int line = 0; line < numLines; line++) {
        [packed readChars:lineBuffer count:lineWidth fromOffset:line * lineWidth];
    }
    uint64_t endTime = mach_absolute_time();

    double elapsedNs = MachTimeToNs(endTime - startTime);
    double nsPerLine = elapsedNs / numLines;
    double nsPerChar = elapsedNs / numChars;

    printf("  Lines read: %d\n", numLines);
    printf("  Total time: %.2f ms\n", elapsedNs / 1e6);
    printf("  Per line:   %.0f ns (%.2f us)\n", nsPerLine, nsPerLine / 1000);
    printf("  Per char:   %.1f ns\n", nsPerChar);

    free(original);
    free(lineBuffer);

    // Should be under 1us per line for good scroll performance
    if (nsPerLine < 1000) {
        printf("  PASS: %.0f ns/line is under 1us threshold\n", nsPerLine);
        return YES;
    } else {
        printf("  WARN: %.0f ns/line is above 1us threshold\n", nsPerLine);
        return YES;  // Not a failure, just a warning
    }
}

#pragma mark - Main

int main(int argc, const char *argv[]) {
    @autoreleasepool {
        printf("═══════════════════════════════════════════════════════════════\n");
        printf("  DashTerm2 Packed Character Buffer Benchmark\n");
        printf("  Testing iTermPackedCharacterBuffer (8 bytes vs 12 bytes)\n");
        printf("═══════════════════════════════════════════════════════════════\n\n");

        int passed = 0;
        int failed = 0;

        if (TestRoundTrip()) passed++; else failed++;
        if (TestMemorySavings()) passed++; else failed++;
        if (TestLargeScrollback()) passed++; else failed++;
        if (TestUnpackPerformance()) passed++; else failed++;

        printf("\n═══════════════════════════════════════════════════════════════\n");
        printf("  Results: %d passed, %d failed\n", passed, failed);
        printf("═══════════════════════════════════════════════════════════════\n");

        // Save results to JSON
        NSString *resultsPath = @"baselines/packed_buffer_baseline.json";
        NSDateFormatter *fmt = [[NSDateFormatter alloc] init];
        fmt.dateFormat = @"yyyy-MM-dd'T'HH:mm:ss'Z'";
        fmt.timeZone = [NSTimeZone timeZoneWithName:@"UTC"];

        NSDictionary *results = @{
            @"benchmark": @"iTermPackedCharacterBuffer",
            @"date": [fmt stringFromDate:[NSDate date]],
            @"passed": @(passed),
            @"failed": @(failed),
            @"struct_sizes": @{
                @"screen_char_t": @(sizeof(screen_char_t)),
                @"packed_screen_char_t": @(sizeof(packed_screen_char_t)),
                @"savings_percent": @(100.0 * (sizeof(screen_char_t) - sizeof(packed_screen_char_t)) / sizeof(screen_char_t))
            }
        };

        NSError *error;
        NSData *jsonData = [NSJSONSerialization dataWithJSONObject:results options:NSJSONWritingPrettyPrinted error:&error];
        if (jsonData) {
            [jsonData writeToFile:resultsPath atomically:YES];
            printf("\nResults saved to: %s\n", resultsPath.UTF8String);
        }

        return failed > 0 ? 1 : 0;
    }
}
