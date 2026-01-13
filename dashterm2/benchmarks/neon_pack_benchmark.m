/*
 *  neon_pack_benchmark.m
 *
 *  Benchmark to measure ARM NEON optimization impact on pack/unpack routines.
 *  Compares NEON-optimized path with scalar fallback.
 *
 *  Build:
 *    clang -O3 -framework Foundation -o neon_pack_benchmark neon_pack_benchmark.m
 *
 *  Run:
 *    ./neon_pack_benchmark
 */

#import <Foundation/Foundation.h>
#import <mach/mach_time.h>

#if __has_include(<arm_neon.h>) && defined(__aarch64__)
#import <arm_neon.h>
#define HAS_NEON 1
#else
#define HAS_NEON 0
#endif

#pragma mark - Type Definitions

// Color modes from DashTerm2
typedef NS_ENUM(int, ColorMode) {
    ColorModeAlternate = 0,
    ColorModeNormal = 1,
    ColorMode24bit = 2,
};

// RTL Status
typedef NS_ENUM(int, RTLStatus) {
    RTLStatusUnknown = 0,
    RTLStatusLTR = 1,
    RTLStatusRTL = 2,
};

// Underline style
typedef NS_ENUM(int, VT100UnderlineStyle) {
    VT100UnderlineStyleSingle = 0,
    VT100UnderlineStyleDouble = 1,
    VT100UnderlineStyleCurly = 2,
    VT100UnderlineStyleDotted = 3,
    VT100UnderlineStyleDashed = 4,
};

// Alternate semantics
enum {
    ALTSEM_DEFAULT = 0,
    ALTSEM_SELECTED = 1,
    ALTSEM_CURSOR = 2,
    ALTSEM_REVERSED_DEFAULT = 3,
    ALTSEM_SYSTEM_MESSAGE = 4,
};

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

// Source structure (12 bytes)
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
    unsigned int rtlStatus : 2;
    unsigned int underlineStyle1 : 1;
    unsigned int unused : 5;
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

_Static_assert(sizeof(screen_char_t) == 12, "screen_char_t must be 12 bytes");
_Static_assert(sizeof(packed_screen_char_t) == 8, "packed_screen_char_t must be 8 bytes");

#pragma mark - Helper Functions

static inline VT100UnderlineStyle ScreenCharGetUnderlineStyle(screen_char_t c) {
    return (VT100UnderlineStyle)(c.underlineStyle0 | (c.underlineStyle1 << 2));
}

#pragma mark - Scalar Implementation (Baseline)

static inline packed_screen_char_t PackScreenCharScalar(screen_char_t src) {
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
    if (src.foregroundColorMode == ColorModeAlternate) {
        switch (src.foregroundColor) {
            case ALTSEM_DEFAULT: dst.fgColor = kPackedColorDefault; break;
            case ALTSEM_SELECTED: dst.fgColor = kPackedColorSelected; break;
            case ALTSEM_CURSOR: dst.fgColor = kPackedColorCursor; break;
            case ALTSEM_REVERSED_DEFAULT: dst.fgColor = kPackedColorReversedDefault; break;
            case ALTSEM_SYSTEM_MESSAGE: dst.fgColor = kPackedColorSystemMessage; break;
            default: dst.fgColor = kPackedColorDefault; break;
        }
    } else {
        dst.fgColor = src.foregroundColor & 0xFF;
    }

    // Pack background color
    if (src.backgroundColorMode == ColorModeAlternate) {
        switch (src.backgroundColor) {
            case ALTSEM_DEFAULT: dst.bgColor = kPackedColorDefault; break;
            case ALTSEM_SELECTED: dst.bgColor = kPackedColorSelected; break;
            case ALTSEM_CURSOR: dst.bgColor = kPackedColorCursor; break;
            case ALTSEM_REVERSED_DEFAULT: dst.bgColor = kPackedColorReversedDefault; break;
            case ALTSEM_SYSTEM_MESSAGE: dst.bgColor = kPackedColorSystemMessage; break;
            default: dst.bgColor = kPackedColorDefault; break;
        }
    } else {
        dst.bgColor = src.backgroundColor & 0xFF;
    }

    return dst;
}

static inline screen_char_t UnpackScreenCharScalar(packed_screen_char_t src) {
    screen_char_t dst;
    memset(&dst, 0, sizeof(dst));

    dst.code = (unichar)(src.code);
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

    uint16_t fgColor = src.fgColor;
    if (fgColor >= kPackedColorDefault && fgColor <= kPackedColorSystemMessage) {
        dst.foregroundColorMode = ColorModeAlternate;
        switch (fgColor) {
            case kPackedColorDefault: dst.foregroundColor = ALTSEM_DEFAULT; break;
            case kPackedColorSelected: dst.foregroundColor = ALTSEM_SELECTED; break;
            case kPackedColorCursor: dst.foregroundColor = ALTSEM_CURSOR; break;
            case kPackedColorReversedDefault: dst.foregroundColor = ALTSEM_REVERSED_DEFAULT; break;
            case kPackedColorSystemMessage: dst.foregroundColor = ALTSEM_SYSTEM_MESSAGE; break;
            default: dst.foregroundColor = ALTSEM_DEFAULT; break;
        }
    } else {
        dst.foregroundColorMode = ColorModeNormal;
        dst.foregroundColor = fgColor & 0xFF;
    }

    uint16_t bgColor = src.bgColor;
    if (bgColor >= kPackedColorDefault && bgColor <= kPackedColorSystemMessage) {
        dst.backgroundColorMode = ColorModeAlternate;
        switch (bgColor) {
            case kPackedColorDefault: dst.backgroundColor = ALTSEM_DEFAULT; break;
            case kPackedColorSelected: dst.backgroundColor = ALTSEM_SELECTED; break;
            case kPackedColorCursor: dst.backgroundColor = ALTSEM_CURSOR; break;
            case kPackedColorReversedDefault: dst.backgroundColor = ALTSEM_REVERSED_DEFAULT; break;
            case kPackedColorSystemMessage: dst.backgroundColor = ALTSEM_SYSTEM_MESSAGE; break;
            default: dst.backgroundColor = ALTSEM_DEFAULT; break;
        }
    } else {
        dst.backgroundColorMode = ColorModeNormal;
        dst.backgroundColor = bgColor & 0xFF;
    }

    return dst;
}

#pragma mark - NEON Implementation

#if HAS_NEON

static inline void PackScreenChar_NEON_2(const screen_char_t *src, packed_screen_char_t *dst) {
    // Character 0
    uint64_t packed0 = (uint64_t)(src[0].code & 0x1FFFFF);

    uint16_t fgColor0 = (src[0].foregroundColorMode == ColorModeAlternate) ?
        (src[0].foregroundColor == ALTSEM_DEFAULT ? kPackedColorDefault :
         src[0].foregroundColor == ALTSEM_SELECTED ? kPackedColorSelected :
         src[0].foregroundColor == ALTSEM_CURSOR ? kPackedColorCursor :
         src[0].foregroundColor == ALTSEM_REVERSED_DEFAULT ? kPackedColorReversedDefault :
         src[0].foregroundColor == ALTSEM_SYSTEM_MESSAGE ? kPackedColorSystemMessage :
         kPackedColorDefault) : (src[0].foregroundColor & 0xFF);
    packed0 |= (uint64_t)(fgColor0 & 0x1FF) << 21;

    uint16_t bgColor0 = (src[0].backgroundColorMode == ColorModeAlternate) ?
        (src[0].backgroundColor == ALTSEM_DEFAULT ? kPackedColorDefault :
         src[0].backgroundColor == ALTSEM_SELECTED ? kPackedColorSelected :
         src[0].backgroundColor == ALTSEM_CURSOR ? kPackedColorCursor :
         src[0].backgroundColor == ALTSEM_REVERSED_DEFAULT ? kPackedColorReversedDefault :
         src[0].backgroundColor == ALTSEM_SYSTEM_MESSAGE ? kPackedColorSystemMessage :
         kPackedColorDefault) : (src[0].backgroundColor & 0xFF);
    packed0 |= (uint64_t)(bgColor0 & 0x1FF) << 30;

    packed0 |= ((uint64_t)src[0].complexChar |
               ((uint64_t)src[0].bold << 1) |
               ((uint64_t)src[0].faint << 2) |
               ((uint64_t)src[0].italic << 3) |
               ((uint64_t)src[0].blink << 4) |
               ((uint64_t)src[0].underline << 5) |
               ((uint64_t)ScreenCharGetUnderlineStyle(src[0]) << 6) |
               ((uint64_t)src[0].image << 9) |
               ((uint64_t)src[0].strikethrough << 10) |
               ((uint64_t)src[0].invisible << 11) |
               ((uint64_t)src[0].inverse << 12) |
               ((uint64_t)src[0].guarded << 13) |
               ((uint64_t)src[0].virtualPlaceholder << 14) |
               ((uint64_t)src[0].rtlStatus << 15)) << 39;

    // Character 1
    uint64_t packed1 = (uint64_t)(src[1].code & 0x1FFFFF);

    uint16_t fgColor1 = (src[1].foregroundColorMode == ColorModeAlternate) ?
        (src[1].foregroundColor == ALTSEM_DEFAULT ? kPackedColorDefault :
         src[1].foregroundColor == ALTSEM_SELECTED ? kPackedColorSelected :
         src[1].foregroundColor == ALTSEM_CURSOR ? kPackedColorCursor :
         src[1].foregroundColor == ALTSEM_REVERSED_DEFAULT ? kPackedColorReversedDefault :
         src[1].foregroundColor == ALTSEM_SYSTEM_MESSAGE ? kPackedColorSystemMessage :
         kPackedColorDefault) : (src[1].foregroundColor & 0xFF);
    packed1 |= (uint64_t)(fgColor1 & 0x1FF) << 21;

    uint16_t bgColor1 = (src[1].backgroundColorMode == ColorModeAlternate) ?
        (src[1].backgroundColor == ALTSEM_DEFAULT ? kPackedColorDefault :
         src[1].backgroundColor == ALTSEM_SELECTED ? kPackedColorSelected :
         src[1].backgroundColor == ALTSEM_CURSOR ? kPackedColorCursor :
         src[1].backgroundColor == ALTSEM_REVERSED_DEFAULT ? kPackedColorReversedDefault :
         src[1].backgroundColor == ALTSEM_SYSTEM_MESSAGE ? kPackedColorSystemMessage :
         kPackedColorDefault) : (src[1].backgroundColor & 0xFF);
    packed1 |= (uint64_t)(bgColor1 & 0x1FF) << 30;

    packed1 |= ((uint64_t)src[1].complexChar |
               ((uint64_t)src[1].bold << 1) |
               ((uint64_t)src[1].faint << 2) |
               ((uint64_t)src[1].italic << 3) |
               ((uint64_t)src[1].blink << 4) |
               ((uint64_t)src[1].underline << 5) |
               ((uint64_t)ScreenCharGetUnderlineStyle(src[1]) << 6) |
               ((uint64_t)src[1].image << 9) |
               ((uint64_t)src[1].strikethrough << 10) |
               ((uint64_t)src[1].invisible << 11) |
               ((uint64_t)src[1].inverse << 12) |
               ((uint64_t)src[1].guarded << 13) |
               ((uint64_t)src[1].virtualPlaceholder << 14) |
               ((uint64_t)src[1].rtlStatus << 15)) << 39;

    // Use NEON store for 2 packed values (16 bytes)
    uint64x2_t result = {packed0, packed1};
    vst1q_u64((uint64_t *)dst, result);
}

static inline void UnpackScreenChar_NEON_2(const packed_screen_char_t *src, screen_char_t *dst) {
    // Load 2 packed chars using NEON
    uint64x2_t packed = vld1q_u64((const uint64_t *)src);
    uint64_t packed0 = vgetq_lane_u64(packed, 0);
    uint64_t packed1 = vgetq_lane_u64(packed, 1);

    // Character 0
    memset(&dst[0], 0, sizeof(screen_char_t));
    dst[0].code = (unichar)(packed0 & 0x1FFFFF);

    uint16_t fgColor0 = (packed0 >> 21) & 0x1FF;
    uint16_t bgColor0 = (packed0 >> 30) & 0x1FF;

    if (fgColor0 >= kPackedColorDefault && fgColor0 <= kPackedColorSystemMessage) {
        dst[0].foregroundColorMode = ColorModeAlternate;
        dst[0].foregroundColor = (fgColor0 == kPackedColorDefault) ? ALTSEM_DEFAULT :
                                 (fgColor0 == kPackedColorSelected) ? ALTSEM_SELECTED :
                                 (fgColor0 == kPackedColorCursor) ? ALTSEM_CURSOR :
                                 (fgColor0 == kPackedColorReversedDefault) ? ALTSEM_REVERSED_DEFAULT :
                                 ALTSEM_SYSTEM_MESSAGE;
    } else {
        dst[0].foregroundColorMode = ColorModeNormal;
        dst[0].foregroundColor = fgColor0 & 0xFF;
    }

    if (bgColor0 >= kPackedColorDefault && bgColor0 <= kPackedColorSystemMessage) {
        dst[0].backgroundColorMode = ColorModeAlternate;
        dst[0].backgroundColor = (bgColor0 == kPackedColorDefault) ? ALTSEM_DEFAULT :
                                 (bgColor0 == kPackedColorSelected) ? ALTSEM_SELECTED :
                                 (bgColor0 == kPackedColorCursor) ? ALTSEM_CURSOR :
                                 (bgColor0 == kPackedColorReversedDefault) ? ALTSEM_REVERSED_DEFAULT :
                                 ALTSEM_SYSTEM_MESSAGE;
    } else {
        dst[0].backgroundColorMode = ColorModeNormal;
        dst[0].backgroundColor = bgColor0 & 0xFF;
    }

    uint64_t flags0 = packed0 >> 39;
    dst[0].complexChar = (flags0 >> 0) & 1;
    dst[0].bold = (flags0 >> 1) & 1;
    dst[0].faint = (flags0 >> 2) & 1;
    dst[0].italic = (flags0 >> 3) & 1;
    dst[0].blink = (flags0 >> 4) & 1;
    dst[0].underline = (flags0 >> 5) & 1;
    dst[0].underlineStyle0 = (flags0 >> 6) & 3;
    dst[0].underlineStyle1 = (flags0 >> 8) & 1;
    dst[0].image = (flags0 >> 9) & 1;
    dst[0].strikethrough = (flags0 >> 10) & 1;
    dst[0].invisible = (flags0 >> 11) & 1;
    dst[0].inverse = (flags0 >> 12) & 1;
    dst[0].guarded = (flags0 >> 13) & 1;
    dst[0].virtualPlaceholder = (flags0 >> 14) & 1;
    dst[0].rtlStatus = (flags0 >> 15) & 3;

    // Character 1
    memset(&dst[1], 0, sizeof(screen_char_t));
    dst[1].code = (unichar)(packed1 & 0x1FFFFF);

    uint16_t fgColor1 = (packed1 >> 21) & 0x1FF;
    uint16_t bgColor1 = (packed1 >> 30) & 0x1FF;

    if (fgColor1 >= kPackedColorDefault && fgColor1 <= kPackedColorSystemMessage) {
        dst[1].foregroundColorMode = ColorModeAlternate;
        dst[1].foregroundColor = (fgColor1 == kPackedColorDefault) ? ALTSEM_DEFAULT :
                                 (fgColor1 == kPackedColorSelected) ? ALTSEM_SELECTED :
                                 (fgColor1 == kPackedColorCursor) ? ALTSEM_CURSOR :
                                 (fgColor1 == kPackedColorReversedDefault) ? ALTSEM_REVERSED_DEFAULT :
                                 ALTSEM_SYSTEM_MESSAGE;
    } else {
        dst[1].foregroundColorMode = ColorModeNormal;
        dst[1].foregroundColor = fgColor1 & 0xFF;
    }

    if (bgColor1 >= kPackedColorDefault && bgColor1 <= kPackedColorSystemMessage) {
        dst[1].backgroundColorMode = ColorModeAlternate;
        dst[1].backgroundColor = (bgColor1 == kPackedColorDefault) ? ALTSEM_DEFAULT :
                                 (bgColor1 == kPackedColorSelected) ? ALTSEM_SELECTED :
                                 (bgColor1 == kPackedColorCursor) ? ALTSEM_CURSOR :
                                 (bgColor1 == kPackedColorReversedDefault) ? ALTSEM_REVERSED_DEFAULT :
                                 ALTSEM_SYSTEM_MESSAGE;
    } else {
        dst[1].backgroundColorMode = ColorModeNormal;
        dst[1].backgroundColor = bgColor1 & 0xFF;
    }

    uint64_t flags1 = packed1 >> 39;
    dst[1].complexChar = (flags1 >> 0) & 1;
    dst[1].bold = (flags1 >> 1) & 1;
    dst[1].faint = (flags1 >> 2) & 1;
    dst[1].italic = (flags1 >> 3) & 1;
    dst[1].blink = (flags1 >> 4) & 1;
    dst[1].underline = (flags1 >> 5) & 1;
    dst[1].underlineStyle0 = (flags1 >> 6) & 3;
    dst[1].underlineStyle1 = (flags1 >> 8) & 1;
    dst[1].image = (flags1 >> 9) & 1;
    dst[1].strikethrough = (flags1 >> 10) & 1;
    dst[1].invisible = (flags1 >> 11) & 1;
    dst[1].inverse = (flags1 >> 12) & 1;
    dst[1].guarded = (flags1 >> 13) & 1;
    dst[1].virtualPlaceholder = (flags1 >> 14) & 1;
    dst[1].rtlStatus = (flags1 >> 15) & 3;
}

#endif // HAS_NEON

#pragma mark - Benchmark Functions

static void PackArrayScalar(const screen_char_t *src, packed_screen_char_t *dst, NSUInteger count) {
    for (NSUInteger i = 0; i < count; i++) {
        dst[i] = PackScreenCharScalar(src[i]);
    }
}

static void UnpackArrayScalar(const packed_screen_char_t *src, screen_char_t *dst, NSUInteger count) {
    for (NSUInteger i = 0; i < count; i++) {
        dst[i] = UnpackScreenCharScalar(src[i]);
    }
}

#if HAS_NEON
static void PackArrayNEON(const screen_char_t *src, packed_screen_char_t *dst, NSUInteger count) {
    NSUInteger i = 0;

    // Process pairs with NEON
    while (i + 1 < count) {
        PackScreenChar_NEON_2(&src[i], &dst[i]);
        i += 2;
    }

    // Handle odd element
    if (i < count) {
        dst[i] = PackScreenCharScalar(src[i]);
    }
}

static void UnpackArrayNEON(const packed_screen_char_t *src, screen_char_t *dst, NSUInteger count) {
    NSUInteger i = 0;

    // Process pairs with NEON
    while (i + 1 < count) {
        UnpackScreenChar_NEON_2(&src[i], &dst[i]);
        i += 2;
    }

    // Handle odd element
    if (i < count) {
        dst[i] = UnpackScreenCharScalar(src[i]);
    }
}
#endif

#pragma mark - Timing

static double GetTimeNanos(void) {
    static mach_timebase_info_data_t timebase;
    if (timebase.denom == 0) {
        mach_timebase_info(&timebase);
    }
    return (double)mach_absolute_time() * timebase.numer / timebase.denom;
}

#pragma mark - Main

int main(int argc, const char * argv[]) {
    @autoreleasepool {
        printf("=================================================\n");
        printf("DashTerm2 NEON Pack/Unpack Benchmark\n");
        printf("=================================================\n\n");

#if HAS_NEON
        printf("Platform: ARM64 with NEON intrinsics available\n\n");
#else
        printf("Platform: x86_64 or ARM without NEON (scalar only)\n\n");
#endif

        // Test parameters
        const NSUInteger kNumChars = 8 * 1024 * 1024;  // 8M chars (100K lines x 80 cols)
        const int kIterations = 5;

        printf("Test size: %lu characters (%.1f MB unpacked, %.1f MB packed)\n\n",
               (unsigned long)kNumChars,
               (double)kNumChars * sizeof(screen_char_t) / (1024 * 1024),
               (double)kNumChars * sizeof(packed_screen_char_t) / (1024 * 1024));

        // Allocate buffers
        screen_char_t *srcChars = calloc(kNumChars, sizeof(screen_char_t));
        packed_screen_char_t *packedChars = calloc(kNumChars, sizeof(packed_screen_char_t));
        screen_char_t *dstChars = calloc(kNumChars, sizeof(screen_char_t));

        // Initialize test data with typical terminal content
        for (NSUInteger i = 0; i < kNumChars; i++) {
            srcChars[i].code = 'A' + (i % 26);
            srcChars[i].foregroundColor = (i % 8);  // ANSI color
            srcChars[i].backgroundColor = 0;
            srcChars[i].foregroundColorMode = ColorModeNormal;
            srcChars[i].backgroundColorMode = ColorModeAlternate;  // Default
            srcChars[i].bold = (i % 100 == 0);
            srcChars[i].italic = (i % 200 == 0);
        }

        // Warm up
        PackArrayScalar(srcChars, packedChars, kNumChars);
        UnpackArrayScalar(packedChars, dstChars, kNumChars);

        // Benchmark scalar pack
        double scalarPackTotal = 0;
        for (int iter = 0; iter < kIterations; iter++) {
            double start = GetTimeNanos();
            PackArrayScalar(srcChars, packedChars, kNumChars);
            double end = GetTimeNanos();
            scalarPackTotal += (end - start);
        }
        double scalarPackAvg = scalarPackTotal / kIterations;
        double scalarPackPerChar = scalarPackAvg / kNumChars;

        // Benchmark scalar unpack
        double scalarUnpackTotal = 0;
        for (int iter = 0; iter < kIterations; iter++) {
            double start = GetTimeNanos();
            UnpackArrayScalar(packedChars, dstChars, kNumChars);
            double end = GetTimeNanos();
            scalarUnpackTotal += (end - start);
        }
        double scalarUnpackAvg = scalarUnpackTotal / kIterations;
        double scalarUnpackPerChar = scalarUnpackAvg / kNumChars;

        printf("Scalar Implementation:\n");
        printf("  Pack:   %.2f ns/char (%.2f ms total, %.2f GB/s)\n",
               scalarPackPerChar, scalarPackAvg / 1e6,
               (double)kNumChars * sizeof(screen_char_t) / scalarPackAvg);
        printf("  Unpack: %.2f ns/char (%.2f ms total, %.2f GB/s)\n\n",
               scalarUnpackPerChar, scalarUnpackAvg / 1e6,
               (double)kNumChars * sizeof(packed_screen_char_t) / scalarUnpackAvg);

#if HAS_NEON
        // Warm up NEON
        PackArrayNEON(srcChars, packedChars, kNumChars);
        UnpackArrayNEON(packedChars, dstChars, kNumChars);

        // Benchmark NEON pack
        double neonPackTotal = 0;
        for (int iter = 0; iter < kIterations; iter++) {
            double start = GetTimeNanos();
            PackArrayNEON(srcChars, packedChars, kNumChars);
            double end = GetTimeNanos();
            neonPackTotal += (end - start);
        }
        double neonPackAvg = neonPackTotal / kIterations;
        double neonPackPerChar = neonPackAvg / kNumChars;

        // Benchmark NEON unpack
        double neonUnpackTotal = 0;
        for (int iter = 0; iter < kIterations; iter++) {
            double start = GetTimeNanos();
            UnpackArrayNEON(packedChars, dstChars, kNumChars);
            double end = GetTimeNanos();
            neonUnpackTotal += (end - start);
        }
        double neonUnpackAvg = neonUnpackTotal / kIterations;
        double neonUnpackPerChar = neonUnpackAvg / kNumChars;

        printf("NEON Implementation:\n");
        printf("  Pack:   %.2f ns/char (%.2f ms total, %.2f GB/s)\n",
               neonPackPerChar, neonPackAvg / 1e6,
               (double)kNumChars * sizeof(screen_char_t) / neonPackAvg);
        printf("  Unpack: %.2f ns/char (%.2f ms total, %.2f GB/s)\n\n",
               neonUnpackPerChar, neonUnpackAvg / 1e6,
               (double)kNumChars * sizeof(packed_screen_char_t) / neonUnpackAvg);

        // Speedup
        double packSpeedup = scalarPackPerChar / neonPackPerChar;
        double unpackSpeedup = scalarUnpackPerChar / neonUnpackPerChar;

        printf("=================================================\n");
        printf("NEON Speedup:\n");
        printf("  Pack:   %.2fx faster\n", packSpeedup);
        printf("  Unpack: %.2fx faster\n", unpackSpeedup);
        printf("=================================================\n\n");

        // Real-world impact
        double scrollback1M_scalar = (1000000.0 * 80 * scalarPackPerChar) / 1e6;  // ms
        double scrollback1M_neon = (1000000.0 * 80 * neonPackPerChar) / 1e6;

        printf("Real-World Impact (1M lines scrollback @ 80 cols):\n");
        printf("  Scalar pack: %.1f ms\n", scrollback1M_scalar);
        printf("  NEON pack:   %.1f ms\n", scrollback1M_neon);
        printf("  Savings:     %.1f ms (%.0f%% reduction)\n\n",
               scrollback1M_scalar - scrollback1M_neon,
               (1 - scrollback1M_neon / scrollback1M_scalar) * 100);
#else
        printf("NEON not available on this platform.\n\n");
#endif

        // Verify correctness
        printf("Verifying correctness...\n");
        PackArrayScalar(srcChars, packedChars, kNumChars);
        UnpackArrayScalar(packedChars, dstChars, kNumChars);

        BOOL correct = YES;
        for (NSUInteger i = 0; i < MIN(kNumChars, 1000); i++) {
            if (srcChars[i].code != dstChars[i].code ||
                srcChars[i].foregroundColor != dstChars[i].foregroundColor ||
                srcChars[i].bold != dstChars[i].bold) {
                correct = NO;
                printf("  Mismatch at index %lu\n", (unsigned long)i);
                break;
            }
        }
        printf("  Scalar: %s\n", correct ? "PASS" : "FAIL");

#if HAS_NEON
        PackArrayNEON(srcChars, packedChars, kNumChars);
        UnpackArrayNEON(packedChars, dstChars, kNumChars);

        correct = YES;
        for (NSUInteger i = 0; i < MIN(kNumChars, 1000); i++) {
            if (srcChars[i].code != dstChars[i].code ||
                srcChars[i].foregroundColor != dstChars[i].foregroundColor ||
                srcChars[i].bold != dstChars[i].bold) {
                correct = NO;
                printf("  Mismatch at index %lu\n", (unsigned long)i);
                break;
            }
        }
        printf("  NEON:   %s\n", correct ? "PASS" : "FAIL");
#endif

        free(srcChars);
        free(packedChars);
        free(dstChars);

        printf("\nBenchmark complete.\n");
    }
    return 0;
}
