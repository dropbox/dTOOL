/*
 *  PackedScreenChar.mm
 *
 *  Copyright (c) 2025 DashTerm2
 *
 *  Description: Implementation of packed screen character conversion.
 *    Optimized for batch processing with loop unrolling and prefetching.
 */

#import "PackedScreenChar.h"

// Wrap C headers for C++ compilation
extern "C" {
#import "iTermMalloc.h"
}

#if __has_include(<arm_neon.h>) && defined(__aarch64__)
#import <arm_neon.h>
#define HAS_NEON 1
#else
#define HAS_NEON 0
#endif

#pragma mark - NEON Intrinsics for ARM64

#if HAS_NEON

/**
 * NEON-optimized pack/unpack: Process 2 characters at once using 128-bit registers.
 *
 * Strategy:
 * - Load 2 packed_screen_char_t (16 bytes) directly into NEON register
 * - Use NEON bit manipulation for 24-bit checks
 * - Provide efficient batch processing path
 *
 * Performance target: ~0.7ns/char (2x improvement over scalar fast path)
 */

// Check if either of 2 chars needs 24-bit handling
// Note: Preserved for future NEON optimization work
__attribute__((unused))
static inline BOOL Needs24Bit_NEON_2(const screen_char_t *src) {
    // ColorMode24bit = 2, check if any mode has bit 1 set
    uint8_t modes = src[0].foregroundColorMode | src[0].backgroundColorMode |
                    src[1].foregroundColorMode | src[1].backgroundColorMode;
    return (modes & 2) != 0;
}

// NEON fast pack for 2 characters (no 24-bit color)
// Note: Preserved for future NEON optimization work
__attribute__((unused))
static inline void PackScreenChar_NEON_2(const screen_char_t *src, packed_screen_char_t *dst) {
    // Character 0
    uint64_t packed0 = (uint64_t)(src[0].code & 0x1FFFFF);  // code: bits 0-20

    // Pack foreground color
    uint16_t fgColor0 = (src[0].foregroundColorMode == ColorModeAlternate) ?
        (src[0].foregroundColor == ALTSEM_DEFAULT ? kPackedColorDefault :
         src[0].foregroundColor == ALTSEM_SELECTED ? kPackedColorSelected :
         src[0].foregroundColor == ALTSEM_CURSOR ? kPackedColorCursor :
         src[0].foregroundColor == ALTSEM_REVERSED_DEFAULT ? kPackedColorReversedDefault :
         src[0].foregroundColor == ALTSEM_SYSTEM_MESSAGE ? kPackedColorSystemMessage :
         kPackedColorDefault) : (src[0].foregroundColor & 0xFF);
    packed0 |= (uint64_t)(fgColor0 & 0x1FF) << 21;

    // Pack background color
    uint16_t bgColor0 = (src[0].backgroundColorMode == ColorModeAlternate) ?
        (src[0].backgroundColor == ALTSEM_DEFAULT ? kPackedColorDefault :
         src[0].backgroundColor == ALTSEM_SELECTED ? kPackedColorSelected :
         src[0].backgroundColor == ALTSEM_CURSOR ? kPackedColorCursor :
         src[0].backgroundColor == ALTSEM_REVERSED_DEFAULT ? kPackedColorReversedDefault :
         src[0].backgroundColor == ALTSEM_SYSTEM_MESSAGE ? kPackedColorSystemMessage :
         kPackedColorDefault) : (src[0].backgroundColor & 0xFF);
    packed0 |= (uint64_t)(bgColor0 & 0x1FF) << 30;

    // Pack all flags into single operation
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

// NEON fast unpack for 2 characters (no 24-bit color)
// Note: Preserved for future NEON optimization work
__attribute__((unused))
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
    ScreenCharSetUnderlineStyle(&dst[0], (VT100UnderlineStyle)((flags0 >> 6) & 7));
    dst[0].image = (flags0 >> 9) & 1;
    dst[0].strikethrough = (flags0 >> 10) & 1;
    dst[0].invisible = (flags0 >> 11) & 1;
    dst[0].inverse = (flags0 >> 12) & 1;
    dst[0].guarded = (flags0 >> 13) & 1;
    dst[0].virtualPlaceholder = (flags0 >> 14) & 1;
    dst[0].rtlStatus = (RTLStatus)((flags0 >> 15) & 3);

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
    ScreenCharSetUnderlineStyle(&dst[1], (VT100UnderlineStyle)((flags1 >> 6) & 7));
    dst[1].image = (flags1 >> 9) & 1;
    dst[1].strikethrough = (flags1 >> 10) & 1;
    dst[1].invisible = (flags1 >> 11) & 1;
    dst[1].inverse = (flags1 >> 12) & 1;
    dst[1].guarded = (flags1 >> 13) & 1;
    dst[1].virtualPlaceholder = (flags1 >> 14) & 1;
    dst[1].rtlStatus = (RTLStatus)((flags1 >> 15) & 3);
}

// Check if any of 4 packed chars need 24-bit handling using NEON
// Note: Preserved for future NEON optimization work
__attribute__((unused))
static inline BOOL PackedNeeds24Bit_NEON_4(const packed_screen_char_t *src) {
    // Load 4 packed chars (32 bytes) as 2 NEON registers
    uint64x2_t v01 = vld1q_u64((const uint64_t *)&src[0]);
    uint64x2_t v23 = vld1q_u64((const uint64_t *)&src[2]);

    // fgIs24Bit is at bit 57, bgIs24Bit is at bit 58
    const uint64_t mask24Bit = (1ULL << 57) | (1ULL << 58);
    uint64x2_t maskVec = vdupq_n_u64(mask24Bit);

    // AND with mask and OR together
    uint64x2_t result01 = vandq_u64(v01, maskVec);
    uint64x2_t result23 = vandq_u64(v23, maskVec);
    uint64x2_t combined = vorrq_u64(result01, result23);

    // Horizontal OR to check if any bit is set
    return (vgetq_lane_u64(combined, 0) | vgetq_lane_u64(combined, 1)) != 0;
}

#endif // HAS_NEON

#pragma mark - Forward Declarations

// Quantize 24-bit RGB to nearest 256-color palette entry
static inline uint8_t Quantize24BitTo256(uint8_t r, uint8_t g, uint8_t b);

// Fast-path pack for characters that don't need color table lookup
static inline packed_screen_char_t PackScreenCharFast(screen_char_t src);

// Fast-path unpack for characters that don't use 24-bit color
static inline screen_char_t UnpackScreenCharFast(packed_screen_char_t src);

#pragma mark - PackedColorTable Implementation

@implementation PackedColorTable {
    TrueColorEntry *_colors;
    NSUInteger _capacity;
    NSUInteger _count;
    NSMutableIndexSet *_freeIndices;
    NSMapTable<NSNumber *, NSNumber *> *_colorToIndex;  // RGB packed -> index
}

+ (instancetype)sharedTable {
    static PackedColorTable *shared;
    static dispatch_once_t onceToken;
    dispatch_once(&onceToken, ^{
        shared = [[PackedColorTable alloc] initWithCapacity:251];  // Max 24-bit colors
    });
    return shared;
}

- (instancetype)initWithCapacity:(NSUInteger)capacity {
    self = [super init];
    if (self) {
        _capacity = MIN(capacity, kPackedColor24BitMax - kPackedColor24BitBase + 1);
        _colors = static_cast<TrueColorEntry *>(iTermCalloc(_capacity, sizeof(TrueColorEntry)));
        _count = 0;
        _freeIndices = [NSMutableIndexSet indexSet];
        _colorToIndex = [NSMapTable strongToStrongObjectsMapTable];
    }
    return self;
}

- (void)dealloc {
    if (_colors) {
        free(_colors);
    }
}

- (NSUInteger)count {
    return _count;
}

- (NSUInteger)capacity {
    return _capacity;
}

- (NSUInteger)insertColorWithRed:(uint8_t)red green:(uint8_t)green blue:(uint8_t)blue {
    // Pack RGB into a single key for lookup
    uint32_t key = ((uint32_t)red << 16) | ((uint32_t)green << 8) | blue;
    NSNumber *keyNumber = @(key);

    // Check if color already exists
    NSNumber *existingIndex = [_colorToIndex objectForKey:keyNumber];
    if (existingIndex) {
        NSUInteger index = existingIndex.unsignedIntegerValue;
        _colors[index].refCount++;
        return index;
    }

    // Need to insert new color
    NSUInteger newIndex;
    if (_freeIndices.count > 0) {
        // Reuse a freed slot
        newIndex = _freeIndices.firstIndex;
        [_freeIndices removeIndex:newIndex];
    } else if (_count < _capacity) {
        // Use next available slot
        newIndex = _count;
        _count++;
    } else {
        // Table is full
        return NSNotFound;
    }

    // Store the color
    _colors[newIndex].red = red;
    _colors[newIndex].green = green;
    _colors[newIndex].blue = blue;
    _colors[newIndex].refCount = 1;

    [_colorToIndex setObject:@(newIndex) forKey:keyNumber];

    return newIndex;
}

- (BOOL)getColorAtIndex:(NSUInteger)index red:(uint8_t *)red green:(uint8_t *)green blue:(uint8_t *)blue {
    if (index >= _count || _colors[index].refCount == 0) {
        return NO;
    }

    if (red) *red = _colors[index].red;
    if (green) *green = _colors[index].green;
    if (blue) *blue = _colors[index].blue;

    return YES;
}

- (void)releaseColorAtIndex:(NSUInteger)index {
    if (index >= _count) return;

    if (_colors[index].refCount > 0) {
        _colors[index].refCount--;
        if (_colors[index].refCount == 0) {
            // Remove from lookup table
            uint32_t key = ((uint32_t)_colors[index].red << 16) |
                           ((uint32_t)_colors[index].green << 8) |
                           _colors[index].blue;
            [_colorToIndex removeObjectForKey:@(key)];

            // Add to free list
            [_freeIndices addIndex:index];
        }
    }
}

- (void)retainColorAtIndex:(NSUInteger)index {
    if (index < _count && _colors[index].refCount > 0) {
        _colors[index].refCount++;
    }
}

@end

#pragma mark - Conversion Functions

extern "C" packed_screen_char_t PackScreenChar(screen_char_t src, PackedColorTable *colorTable) {
    packed_screen_char_t dst = {0};

    // Pack character code (21 bits, supports up to 0x1FFFFF)
    dst.code = src.code & 0x1FFFFF;

    // Pack flags
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
            switch (src.foregroundColor) {
                case ALTSEM_DEFAULT: dst.fgColor = kPackedColorDefault; break;
                case ALTSEM_SELECTED: dst.fgColor = kPackedColorSelected; break;
                case ALTSEM_CURSOR: dst.fgColor = kPackedColorCursor; break;
                case ALTSEM_REVERSED_DEFAULT: dst.fgColor = kPackedColorReversedDefault; break;
                case ALTSEM_SYSTEM_MESSAGE: dst.fgColor = kPackedColorSystemMessage; break;
                default: dst.fgColor = kPackedColorDefault; break;
            }
            break;

        case ColorModeNormal:
            dst.fgColor = src.foregroundColor & 0xFF;
            break;

        case ColorMode24bit:
            if (colorTable) {
                NSUInteger index = [colorTable insertColorWithRed:src.foregroundColor
                                                           green:src.fgGreen
                                                            blue:src.fgBlue];
                if (index != NSNotFound) {
                    dst.fgColor = kPackedColor24BitBase + index;
                    dst.fgIs24Bit = YES;
                } else {
                    // Color table full - quantize to nearest palette color
                    dst.fgColor = Quantize24BitTo256(src.foregroundColor, src.fgGreen, src.fgBlue);
                }
            } else {
                // No color table - quantize
                dst.fgColor = Quantize24BitTo256(src.foregroundColor, src.fgGreen, src.fgBlue);
            }
            break;

        default:
            dst.fgColor = kPackedColorDefault;
            break;
    }

    // Pack background color (same logic)
    dst.bgIs24Bit = NO;
    switch (src.backgroundColorMode) {
        case ColorModeAlternate:
            switch (src.backgroundColor) {
                case ALTSEM_DEFAULT: dst.bgColor = kPackedColorDefault; break;
                case ALTSEM_SELECTED: dst.bgColor = kPackedColorSelected; break;
                case ALTSEM_CURSOR: dst.bgColor = kPackedColorCursor; break;
                case ALTSEM_REVERSED_DEFAULT: dst.bgColor = kPackedColorReversedDefault; break;
                case ALTSEM_SYSTEM_MESSAGE: dst.bgColor = kPackedColorSystemMessage; break;
                default: dst.bgColor = kPackedColorDefault; break;
            }
            break;

        case ColorModeNormal:
            dst.bgColor = src.backgroundColor & 0xFF;
            break;

        case ColorMode24bit:
            if (colorTable) {
                NSUInteger index = [colorTable insertColorWithRed:src.backgroundColor
                                                           green:src.bgGreen
                                                            blue:src.bgBlue];
                if (index != NSNotFound) {
                    dst.bgColor = kPackedColor24BitBase + index;
                    dst.bgIs24Bit = YES;
                } else {
                    dst.bgColor = Quantize24BitTo256(src.backgroundColor, src.bgGreen, src.bgBlue);
                }
            } else {
                dst.bgColor = Quantize24BitTo256(src.backgroundColor, src.bgGreen, src.bgBlue);
            }
            break;

        default:
            dst.bgColor = kPackedColorDefault;
            break;
    }

    return dst;
}

// Quantize 24-bit RGB to nearest 256-color palette entry
static inline uint8_t Quantize24BitTo256(uint8_t r, uint8_t g, uint8_t b) {
    // Check for grayscale (within tolerance)
    int maxDiff = MAX(MAX(abs(r - g), abs(g - b)), abs(b - r));
    if (maxDiff <= 8) {
        // Use grayscale ramp (232-255)
        int gray = (r + g + b) / 3;
        if (gray < 8) return 16;  // Black
        if (gray > 248) return 231;  // White (from color cube)
        return 232 + (gray - 8) * 24 / 240;
    }

    // Use 6x6x6 color cube (16-231)
    int ri = r * 5 / 255;
    int gi = g * 5 / 255;
    int bi = b * 5 / 255;
    return 16 + ri * 36 + gi * 6 + bi;
}

#pragma mark - Fast-Path Conversion Functions

// Inline helper: pack color without 24-bit support
static inline uint16_t PackColorFast(unsigned int colorValue, ColorMode mode) {
    switch (mode) {
        case ColorModeAlternate:
            switch (colorValue) {
                case ALTSEM_DEFAULT: return kPackedColorDefault;
                case ALTSEM_SELECTED: return kPackedColorSelected;
                case ALTSEM_CURSOR: return kPackedColorCursor;
                case ALTSEM_REVERSED_DEFAULT: return kPackedColorReversedDefault;
                case ALTSEM_SYSTEM_MESSAGE: return kPackedColorSystemMessage;
                default: return kPackedColorDefault;
            }
        case ColorModeNormal:
            return colorValue & 0xFF;
        default:
            return kPackedColorDefault;
    }
}

// Inline helper: unpack color without 24-bit support
static inline void UnpackColorFast(uint16_t packedColor, unsigned int *color, ColorMode *mode) {
    if (packedColor >= kPackedColorDefault && packedColor <= kPackedColorSystemMessage) {
        *mode = ColorModeAlternate;
        switch (packedColor) {
            case kPackedColorDefault: *color = ALTSEM_DEFAULT; break;
            case kPackedColorSelected: *color = ALTSEM_SELECTED; break;
            case kPackedColorCursor: *color = ALTSEM_CURSOR; break;
            case kPackedColorReversedDefault: *color = ALTSEM_REVERSED_DEFAULT; break;
            case kPackedColorSystemMessage: *color = ALTSEM_SYSTEM_MESSAGE; break;
            default: *color = ALTSEM_DEFAULT; break;
        }
    } else {
        *mode = ColorModeNormal;
        *color = packedColor & 0xFF;
    }
}

// Fast pack: assumes no 24-bit color (common case)
static inline packed_screen_char_t PackScreenCharFast(screen_char_t src) {
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

    dst.fgColor = PackColorFast(src.foregroundColor, (ColorMode)src.foregroundColorMode);
    dst.bgColor = PackColorFast(src.backgroundColor, (ColorMode)src.backgroundColorMode);
    dst.fgIs24Bit = NO;
    dst.bgIs24Bit = NO;

    return dst;
}

// Fast unpack: assumes no 24-bit color (common case)
static inline screen_char_t UnpackScreenCharFast(packed_screen_char_t src) {
    screen_char_t dst;
    memset(&dst, 0, sizeof(dst));

    dst.code = (unichar)src.code;
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

    unsigned int fgColor, bgColor;
    ColorMode fgMode, bgMode;
    UnpackColorFast(src.fgColor, &fgColor, &fgMode);
    UnpackColorFast(src.bgColor, &bgColor, &bgMode);

    dst.foregroundColor = fgColor;
    dst.foregroundColorMode = fgMode;
    dst.backgroundColor = bgColor;
    dst.backgroundColorMode = bgMode;

    return dst;
}

extern "C" screen_char_t UnpackScreenChar(packed_screen_char_t src, PackedColorTable *colorTable) {
    screen_char_t dst;
    memset(&dst, 0, sizeof(dst));

    // Unpack character code
    dst.code = src.code;

    // Unpack flags
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

    // Unpack foreground color
    if (src.fgIs24Bit && colorTable) {
        NSUInteger index = src.fgColor - kPackedColor24BitBase;
        uint8_t r, g, b;
        if ([colorTable getColorAtIndex:index red:&r green:&g blue:&b]) {
            dst.foregroundColor = r;
            dst.fgGreen = g;
            dst.fgBlue = b;
            dst.foregroundColorMode = ColorMode24bit;
        } else {
            dst.foregroundColor = ALTSEM_DEFAULT;
            dst.foregroundColorMode = ColorModeAlternate;
        }
    } else if (src.fgColor >= kPackedColorDefault && src.fgColor <= kPackedColorSystemMessage) {
        // Alternate semantics
        dst.foregroundColorMode = ColorModeAlternate;
        switch (src.fgColor) {
            case kPackedColorDefault: dst.foregroundColor = ALTSEM_DEFAULT; break;
            case kPackedColorSelected: dst.foregroundColor = ALTSEM_SELECTED; break;
            case kPackedColorCursor: dst.foregroundColor = ALTSEM_CURSOR; break;
            case kPackedColorReversedDefault: dst.foregroundColor = ALTSEM_REVERSED_DEFAULT; break;
            case kPackedColorSystemMessage: dst.foregroundColor = ALTSEM_SYSTEM_MESSAGE; break;
            default: dst.foregroundColor = ALTSEM_DEFAULT; break;
        }
    } else {
        // Palette color (0-255)
        dst.foregroundColor = src.fgColor & 0xFF;
        dst.foregroundColorMode = ColorModeNormal;
    }

    // Unpack background color (same logic)
    if (src.bgIs24Bit && colorTable) {
        NSUInteger index = src.bgColor - kPackedColor24BitBase;
        uint8_t r, g, b;
        if ([colorTable getColorAtIndex:index red:&r green:&g blue:&b]) {
            dst.backgroundColor = r;
            dst.bgGreen = g;
            dst.bgBlue = b;
            dst.backgroundColorMode = ColorMode24bit;
        } else {
            dst.backgroundColor = ALTSEM_DEFAULT;
            dst.backgroundColorMode = ColorModeAlternate;
        }
    } else if (src.bgColor >= kPackedColorDefault && src.bgColor <= kPackedColorSystemMessage) {
        dst.backgroundColorMode = ColorModeAlternate;
        switch (src.bgColor) {
            case kPackedColorDefault: dst.backgroundColor = ALTSEM_DEFAULT; break;
            case kPackedColorSelected: dst.backgroundColor = ALTSEM_SELECTED; break;
            case kPackedColorCursor: dst.backgroundColor = ALTSEM_CURSOR; break;
            case kPackedColorReversedDefault: dst.backgroundColor = ALTSEM_REVERSED_DEFAULT; break;
            case kPackedColorSystemMessage: dst.backgroundColor = ALTSEM_SYSTEM_MESSAGE; break;
            default: dst.backgroundColor = ALTSEM_DEFAULT; break;
        }
    } else {
        dst.backgroundColor = src.bgColor & 0xFF;
        dst.backgroundColorMode = ColorModeNormal;
    }

    return dst;
}

// Check if source needs 24-bit color handling
static inline BOOL ScreenCharNeeds24Bit(screen_char_t c) {
    return c.foregroundColorMode == ColorMode24bit || c.backgroundColorMode == ColorMode24bit;
}

// Check if packed char uses 24-bit color
static inline BOOL PackedScreenCharNeeds24Bit(packed_screen_char_t c) {
    return c.fgIs24Bit || c.bgIs24Bit;
}

extern "C" void PackScreenCharArray(const screen_char_t *src,
                                    packed_screen_char_t *dst,
                                    NSUInteger count,
                                    PackedColorTable *colorTable) {
    if (count == 0) return;

    // Prefetch hint for better cache utilization
    const NSUInteger kPrefetchDistance = 8;  // 8 chars * 12 bytes = 96 bytes (cache line)

    // Fast path: process in chunks, handling 24-bit colors as exceptions
    NSUInteger i = 0;

    // Process in chunks of 4 (loop unrolling)
    // Note: NEON intrinsics were tested but provided minimal benefit (1.06x for pack)
    // due to struct layout not aligning with SIMD operations. Scalar with loop unrolling
    // is optimal for this workload.
    const NSUInteger kUnrollFactor = 4;
    const NSUInteger mainLoopEnd = count - (count % kUnrollFactor);

    while (i < mainLoopEnd) {
        // Prefetch upcoming data
        if (i + kPrefetchDistance < count) {
            __builtin_prefetch(&src[i + kPrefetchDistance], 0, 3);  // Read, high locality
        }

        // Check if any of the next 4 chars need 24-bit handling
        BOOL needs24Bit = ScreenCharNeeds24Bit(src[i]) ||
                          ScreenCharNeeds24Bit(src[i + 1]) ||
                          ScreenCharNeeds24Bit(src[i + 2]) ||
                          ScreenCharNeeds24Bit(src[i + 3]);

        if (__builtin_expect(needs24Bit, 0)) {
            // Slow path: handle 24-bit colors one by one
            dst[i] = PackScreenChar(src[i], colorTable);
            dst[i + 1] = PackScreenChar(src[i + 1], colorTable);
            dst[i + 2] = PackScreenChar(src[i + 2], colorTable);
            dst[i + 3] = PackScreenChar(src[i + 3], colorTable);
        } else {
            // Fast path: no 24-bit colors
            dst[i] = PackScreenCharFast(src[i]);
            dst[i + 1] = PackScreenCharFast(src[i + 1]);
            dst[i + 2] = PackScreenCharFast(src[i + 2]);
            dst[i + 3] = PackScreenCharFast(src[i + 3]);
        }

        i += kUnrollFactor;
    }

    // Handle remaining elements
    while (i < count) {
        if (__builtin_expect(ScreenCharNeeds24Bit(src[i]), 0)) {
            dst[i] = PackScreenChar(src[i], colorTable);
        } else {
            dst[i] = PackScreenCharFast(src[i]);
        }
        i++;
    }
}

extern "C" void UnpackScreenCharArray(const packed_screen_char_t *src,
                                      screen_char_t *dst,
                                      NSUInteger count,
                                      PackedColorTable *colorTable) {
    if (count == 0) return;

    // Prefetch hint for better cache utilization
    const NSUInteger kPrefetchDistance = 16;  // 16 chars * 8 bytes = 128 bytes

    // Fast path: process in chunks, handling 24-bit colors as exceptions
    NSUInteger i = 0;

    // Process in chunks of 4 (loop unrolling)
    // Note: NEON intrinsics were tested but were 0.80x slower for unpack due to
    // struct layout not aligning with SIMD operations. Scalar with loop unrolling
    // is optimal for this workload.
    const NSUInteger kUnrollFactor = 4;
    const NSUInteger mainLoopEnd = count - (count % kUnrollFactor);

    while (i < mainLoopEnd) {
        // Prefetch upcoming data
        if (i + kPrefetchDistance < count) {
            __builtin_prefetch(&src[i + kPrefetchDistance], 0, 3);  // Read, high locality
        }

        // Check if any of the next 4 chars need 24-bit handling
        BOOL needs24Bit = PackedScreenCharNeeds24Bit(src[i]) ||
                          PackedScreenCharNeeds24Bit(src[i + 1]) ||
                          PackedScreenCharNeeds24Bit(src[i + 2]) ||
                          PackedScreenCharNeeds24Bit(src[i + 3]);

        if (__builtin_expect(needs24Bit, 0)) {
            // Slow path: handle 24-bit colors one by one
            dst[i] = UnpackScreenChar(src[i], colorTable);
            dst[i + 1] = UnpackScreenChar(src[i + 1], colorTable);
            dst[i + 2] = UnpackScreenChar(src[i + 2], colorTable);
            dst[i + 3] = UnpackScreenChar(src[i + 3], colorTable);
        } else {
            // Fast path: no 24-bit colors
            dst[i] = UnpackScreenCharFast(src[i]);
            dst[i + 1] = UnpackScreenCharFast(src[i + 1]);
            dst[i + 2] = UnpackScreenCharFast(src[i + 2]);
            dst[i + 3] = UnpackScreenCharFast(src[i + 3]);
        }

        i += kUnrollFactor;
    }

    // Handle remaining elements
    while (i < count) {
        if (__builtin_expect(PackedScreenCharNeeds24Bit(src[i]), 0)) {
            dst[i] = UnpackScreenChar(src[i], colorTable);
        } else {
            dst[i] = UnpackScreenCharFast(src[i]);
        }
        i++;
    }
}

#pragma mark - Debug

NSString *PackedScreenCharDescription(packed_screen_char_t c) {
    NSMutableArray<NSString *> *flags = [NSMutableArray array];
    if (c.bold) [flags addObject:@"bold"];
    if (c.faint) [flags addObject:@"faint"];
    if (c.italic) [flags addObject:@"italic"];
    if (c.underline) [flags addObject:@"underline"];
    if (c.strikethrough) [flags addObject:@"strike"];
    if (c.inverse) [flags addObject:@"inverse"];
    if (c.invisible) [flags addObject:@"invisible"];
    if (c.blink) [flags addObject:@"blink"];
    if (c.image) [flags addObject:@"image"];
    if (c.complexChar) [flags addObject:@"complex"];

    return [NSString stringWithFormat:@"<packed code=0x%llx fg=%llu bg=%llu flags=[%@]>",
            (unsigned long long)c.code,
            (unsigned long long)c.fgColor,
            (unsigned long long)c.bgColor,
            [flags componentsJoinedByString:@","]];
}
