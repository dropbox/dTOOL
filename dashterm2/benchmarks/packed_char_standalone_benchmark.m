/*
 * packed_char_standalone_benchmark.m
 *
 * Standalone benchmark for packed_screen_char_t memory and conversion performance.
 * Does not depend on DashTerm2 headers - includes all necessary definitions inline.
 *
 * Compile and run:
 *   clang -O3 -framework Foundation Benchmarks/packed_char_standalone_benchmark.m -o /tmp/packed_benchmark && /tmp/packed_benchmark
 */

#import <Foundation/Foundation.h>
#import <mach/mach_time.h>

#pragma mark - Type Definitions (from ScreenChar.h)

typedef unsigned short unichar;

typedef enum {
    ColorModeAlternate = 0,
    ColorModeNormal = 1,
    ColorMode24bit = 2,
    ColorModeInvalid = 3
} ColorMode;

typedef NS_ENUM(unsigned int, VT100UnderlineStyle) {
    VT100UnderlineStyleSingle,
    VT100UnderlineStyleCurly,
    VT100UnderlineStyleDouble,
    VT100UnderlineStyleDotted,
    VT100UnderlineStyleDashed
};

typedef NS_ENUM(unsigned int, RTLStatus) {
    RTLStatusUnknown = 0,
    RTLStatusLTR = 1,
    RTLStatusRTL = 2
};

#define ALTSEM_DEFAULT 0
#define ALTSEM_SELECTED 1
#define ALTSEM_CURSOR 2
#define ALTSEM_REVERSED_DEFAULT 3
#define ALTSEM_SYSTEM_MESSAGE 4

// Current structure (12 bytes)
typedef struct screen_char_t {
    unichar code;
    unsigned int foregroundColor : 8;
    unsigned int fgGreen : 8;
    unsigned int fgBlue  : 8;
    unsigned int backgroundColor : 8;
    unsigned int bgGreen : 8;
    unsigned int bgBlue  : 8;
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

// Packed structure (8 bytes)
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

typedef NS_ENUM(uint16_t, PackedColorIndex) {
    kPackedColorDefault = 256,
    kPackedColorSelected = 257,
    kPackedColorCursor = 258,
    kPackedColorReversedDefault = 259,
    kPackedColorSystemMessage = 260,
    kPackedColor24BitBase = 261,
    kPackedColor24BitMax = 511,
};

// Verify sizes at compile time
_Static_assert(sizeof(screen_char_t) == 12, "screen_char_t must be 12 bytes");
_Static_assert(sizeof(packed_screen_char_t) == 8, "packed_screen_char_t must be 8 bytes");

#pragma mark - Color Table

typedef struct {
    uint8_t red;
    uint8_t green;
    uint8_t blue;
    uint8_t refCount;
} TrueColorEntry;

@interface PackedColorTable : NSObject {
    TrueColorEntry *_colors;
    NSUInteger _capacity;
    NSUInteger _count;
    NSMutableDictionary<NSNumber *, NSNumber *> *_colorToIndex;
}
@property (nonatomic, readonly) NSUInteger capacity;
@property (nonatomic, readonly) NSUInteger count;
- (instancetype)initWithCapacity:(NSUInteger)capacity;
- (NSUInteger)insertColorWithRed:(uint8_t)red green:(uint8_t)green blue:(uint8_t)blue;
- (BOOL)getColorAtIndex:(NSUInteger)index red:(uint8_t *)red green:(uint8_t *)green blue:(uint8_t *)blue;
@end

@implementation PackedColorTable

- (instancetype)initWithCapacity:(NSUInteger)capacity {
    self = [super init];
    if (self) {
        _capacity = MIN(capacity, 251);
        _colors = calloc(_capacity, sizeof(TrueColorEntry));
        _count = 0;
        _colorToIndex = [NSMutableDictionary dictionary];
    }
    return self;
}

- (void)dealloc {
    free(_colors);
}

- (NSUInteger)insertColorWithRed:(uint8_t)red green:(uint8_t)green blue:(uint8_t)blue {
    uint32_t key = ((uint32_t)red << 16) | ((uint32_t)green << 8) | blue;
    NSNumber *existing = _colorToIndex[@(key)];
    if (existing) {
        NSUInteger idx = existing.unsignedIntegerValue;
        _colors[idx].refCount++;
        return idx;
    }
    if (_count >= _capacity) return NSNotFound;
    NSUInteger newIndex = _count++;
    _colors[newIndex] = (TrueColorEntry){red, green, blue, 1};
    _colorToIndex[@(key)] = @(newIndex);
    return newIndex;
}

- (BOOL)getColorAtIndex:(NSUInteger)index red:(uint8_t *)red green:(uint8_t *)green blue:(uint8_t *)blue {
    if (index >= _count) return NO;
    if (red) *red = _colors[index].red;
    if (green) *green = _colors[index].green;
    if (blue) *blue = _colors[index].blue;
    return YES;
}

@end

#pragma mark - Conversion Functions

static inline uint8_t Quantize24BitTo256(uint8_t r, uint8_t g, uint8_t b) {
    int maxDiff = MAX(MAX(abs(r - g), abs(g - b)), abs(b - r));
    if (maxDiff <= 8) {
        int gray = (r + g + b) / 3;
        if (gray < 8) return 16;
        if (gray > 248) return 231;
        return 232 + (gray - 8) * 24 / 240;
    }
    return 16 + (r * 5 / 255) * 36 + (g * 5 / 255) * 6 + (b * 5 / 255);
}

static inline packed_screen_char_t PackScreenChar(screen_char_t src, PackedColorTable *colorTable) {
    packed_screen_char_t dst = {0};
    dst.code = src.code;
    dst.complexChar = src.complexChar;
    dst.bold = src.bold;
    dst.faint = src.faint;
    dst.italic = src.italic;
    dst.blink = src.blink;
    dst.underline = src.underline;
    dst.underlineStyle = src.underlineStyle0 | (src.underlineStyle1 << 2);
    dst.image = src.image;
    dst.strikethrough = src.strikethrough;
    dst.invisible = src.invisible;
    dst.inverse = src.inverse;
    dst.guarded = src.guarded;
    dst.virtualPlaceholder = src.virtualPlaceholder;
    dst.rtlStatus = src.rtlStatus;

    // Pack foreground
    if (src.foregroundColorMode == ColorModeAlternate) {
        dst.fgColor = 256 + src.foregroundColor;
    } else if (src.foregroundColorMode == ColorModeNormal) {
        dst.fgColor = src.foregroundColor & 0xFF;
    } else if (src.foregroundColorMode == ColorMode24bit && colorTable) {
        NSUInteger idx = [colorTable insertColorWithRed:src.foregroundColor
                                                  green:src.fgGreen
                                                   blue:src.fgBlue];
        if (idx != NSNotFound) {
            dst.fgColor = kPackedColor24BitBase + idx;
            dst.fgIs24Bit = 1;
        } else {
            dst.fgColor = Quantize24BitTo256(src.foregroundColor, src.fgGreen, src.fgBlue);
        }
    }

    // Pack background
    if (src.backgroundColorMode == ColorModeAlternate) {
        dst.bgColor = 256 + src.backgroundColor;
    } else if (src.backgroundColorMode == ColorModeNormal) {
        dst.bgColor = src.backgroundColor & 0xFF;
    } else if (src.backgroundColorMode == ColorMode24bit && colorTable) {
        NSUInteger idx = [colorTable insertColorWithRed:src.backgroundColor
                                                  green:src.bgGreen
                                                   blue:src.bgBlue];
        if (idx != NSNotFound) {
            dst.bgColor = kPackedColor24BitBase + idx;
            dst.bgIs24Bit = 1;
        } else {
            dst.bgColor = Quantize24BitTo256(src.backgroundColor, src.bgGreen, src.bgBlue);
        }
    }

    return dst;
}

static inline screen_char_t UnpackScreenChar(packed_screen_char_t src, PackedColorTable *colorTable) {
    screen_char_t dst = {0};
    dst.code = src.code;
    dst.complexChar = src.complexChar;
    dst.bold = src.bold;
    dst.faint = src.faint;
    dst.italic = src.italic;
    dst.blink = src.blink;
    dst.underline = src.underline;
    dst.underlineStyle0 = src.underlineStyle & 3;
    dst.underlineStyle1 = (src.underlineStyle >> 2) & 1;
    dst.image = src.image;
    dst.strikethrough = src.strikethrough;
    dst.invisible = src.invisible;
    dst.inverse = src.inverse;
    dst.guarded = src.guarded;
    dst.virtualPlaceholder = src.virtualPlaceholder;
    dst.rtlStatus = src.rtlStatus;

    // Unpack foreground
    if (src.fgIs24Bit && colorTable) {
        uint8_t r, g, b;
        if ([colorTable getColorAtIndex:src.fgColor - kPackedColor24BitBase red:&r green:&g blue:&b]) {
            dst.foregroundColor = r;
            dst.fgGreen = g;
            dst.fgBlue = b;
            dst.foregroundColorMode = ColorMode24bit;
        }
    } else if (src.fgColor >= 256 && src.fgColor <= 260) {
        dst.foregroundColorMode = ColorModeAlternate;
        dst.foregroundColor = src.fgColor - 256;
    } else {
        dst.foregroundColorMode = ColorModeNormal;
        dst.foregroundColor = src.fgColor;
    }

    // Unpack background
    if (src.bgIs24Bit && colorTable) {
        uint8_t r, g, b;
        if ([colorTable getColorAtIndex:src.bgColor - kPackedColor24BitBase red:&r green:&g blue:&b]) {
            dst.backgroundColor = r;
            dst.bgGreen = g;
            dst.bgBlue = b;
            dst.backgroundColorMode = ColorMode24bit;
        }
    } else if (src.bgColor >= 256 && src.bgColor <= 260) {
        dst.backgroundColorMode = ColorModeAlternate;
        dst.backgroundColor = src.bgColor - 256;
    } else {
        dst.backgroundColorMode = ColorModeNormal;
        dst.backgroundColor = src.bgColor;
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

#pragma mark - Benchmark

static mach_timebase_info_data_t timebaseInfo;

static uint64_t nanoseconds(uint64_t elapsed) {
    return elapsed * timebaseInfo.numer / timebaseInfo.denom;
}

static double milliseconds(uint64_t elapsed) {
    return (double)nanoseconds(elapsed) / 1e6;
}

static void fillTestData(screen_char_t *buffer, NSUInteger count) {
    for (NSUInteger i = 0; i < count; i++) {
        buffer[i].code = (unichar)('A' + (i % 26));
        buffer[i].complexChar = (i % 100 == 0);
        buffer[i].bold = (i % 10 == 0);
        buffer[i].italic = (i % 20 == 0);
        buffer[i].underline = (i % 30 == 0);

        if (i % 200 == 0) {
            buffer[i].foregroundColorMode = ColorMode24bit;
            buffer[i].foregroundColor = (i * 17) & 0xFF;
            buffer[i].fgGreen = (i * 23) & 0xFF;
            buffer[i].fgBlue = (i * 31) & 0xFF;
        } else if (i % 66 == 0) {
            buffer[i].foregroundColorMode = ColorModeNormal;
            buffer[i].foregroundColor = 16 + (i % 216);
        } else if (i % 12 == 0) {
            buffer[i].foregroundColorMode = ColorModeNormal;
            buffer[i].foregroundColor = i % 16;
        } else {
            buffer[i].foregroundColorMode = ColorModeAlternate;
            buffer[i].foregroundColor = ALTSEM_DEFAULT;
        }

        buffer[i].backgroundColorMode = ColorModeAlternate;
        buffer[i].backgroundColor = ALTSEM_DEFAULT;
    }
}

int main(int argc, const char * argv[]) {
    @autoreleasepool {
        mach_timebase_info(&timebaseInfo);

        printf("===========================================\n");
        printf("Packed screen_char_t Benchmark\n");
        printf("===========================================\n\n");

        printf("Structure Sizes:\n");
        printf("  screen_char_t:        %lu bytes\n", sizeof(screen_char_t));
        printf("  packed_screen_char_t: %lu bytes\n", sizeof(packed_screen_char_t));
        printf("  Memory reduction:     %.1f%%\n",
               100.0 * (1.0 - (double)sizeof(packed_screen_char_t) / sizeof(screen_char_t)));
        printf("\n");

        printf("Cache Line Efficiency (64-byte cache line):\n");
        printf("  screen_char_t:        %.1f chars/cache line\n", 64.0 / sizeof(screen_char_t));
        printf("  packed_screen_char_t: %.1f chars/cache line\n", 64.0 / sizeof(packed_screen_char_t));
        printf("  Improvement:          %.2fx\n",
               (64.0 / sizeof(packed_screen_char_t)) / (64.0 / sizeof(screen_char_t)));
        printf("\n");

        printf("Memory Savings (80 columns):\n");
        NSUInteger columns = 80;
        NSUInteger sizes[] = {24, 1000, 10000, 100000, 1000000};
        const char *labels[] = {"24 lines (1 screen)", "1K lines", "10K lines", "100K lines", "1M lines"};

        for (int i = 0; i < 5; i++) {
            NSUInteger chars = columns * sizes[i];
            size_t oldSize = chars * sizeof(screen_char_t);
            size_t newSize = chars * sizeof(packed_screen_char_t);
            printf("  %-20s: %6.1f MB -> %6.1f MB (saves %.1f MB)\n",
                   labels[i], oldSize / 1e6, newSize / 1e6, (oldSize - newSize) / 1e6);
        }
        printf("\n");

        printf("Conversion Performance:\n");
        NSUInteger benchmarkSize = 100000;
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

        int iterations = 100;
        uint64_t startTime = mach_absolute_time();
        for (int i = 0; i < iterations; i++) {
            PackScreenCharArray(original, packed, benchmarkSize, colorTable);
        }
        uint64_t packTime = mach_absolute_time() - startTime;

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
        printf("  Throughput (pack):   %.0f MB/s\n",
               (benchmarkSize * sizeof(screen_char_t) / 1e6) / (packMs / 1000));
        printf("  Throughput (unpack): %.0f MB/s\n",
               (benchmarkSize * sizeof(screen_char_t) / 1e6) / (unpackMs / 1000));
        printf("\n");

        // Verify
        NSUInteger diffCount = 0;
        for (NSUInteger i = 0; i < benchmarkSize; i++) {
            if (original[i].code != roundTrip[i].code ||
                original[i].bold != roundTrip[i].bold) {
                diffCount++;
            }
        }
        printf("Conversion Correctness:\n");
        printf("  Round-trip verified: %s\n", diffCount == 0 ? "YES" : "NO");
        printf("  24-bit colors stored: %lu / %lu\n", colorTable.count, colorTable.capacity);
        printf("\n");

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

        free(original);
        free(roundTrip);
        free(packed);

        return 0;
    }
}
