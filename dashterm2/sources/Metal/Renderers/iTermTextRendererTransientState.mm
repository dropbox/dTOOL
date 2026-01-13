//
//  iTermTextRendererTransientState.m
//  DashTerm2SharedARC
//
//  Created by George Nachman on 12/22/17.
//

#import "FutureMethods.h"
#import "iTermTextRendererTransientState.h"
#import "iTermTextRendererTransientState+Private.h"
#import "iTermPIUArray.h"
#import "iTermSubpixelModelBuilder.h"
#import "iTermTexturePage.h"
#import "iTermTexturePageCollection.h"
#import "NSMutableData+iTerm.h"

#include <unordered_map>

const vector_float4 iTermIMEColor = simd_make_float4(1, 1, 0, 1);
const vector_float4 iTermAnnotationUnderlineColor = simd_make_float4(1, 1, 0, 1);

// Pre-compute alpha vector for text color on CPU to avoid per-vertex GPU computation.
// This mirrors the logic in iTermAlphaVectorForTextColor() in iTermText.metal.
// Used by monochrome rendering path on macOS 10.14+.
static inline vector_float4 iTermComputeAlphaVectorForTextColor(vector_float4 textColor) {
    static const vector_float4 blackVector = {0, 0, 1, 0};
    static const vector_float4 redVector = {0, 1, 0, 0};
    static const vector_float4 greenVector = {1, 0, 0, 0};
    static const vector_float4 yellowVector = {0, 0, 0, 1};

    // Low thresholds bias toward heavier text for mid-tones.
    const float threshold = 0.6f;

    // https://gitlab.com/gnachman/iterm2/wikis/macOS-Mojave-Regression-Challenge
    if (textColor.x + textColor.y > threshold * 2) {
        return yellowVector;
    } else if (textColor.y > threshold) {
        return greenVector;
    } else if (textColor.x > threshold) {
        return redVector;
    } else {
        return blackVector;
    }
}

namespace DashTerm2 {
class TexturePage;
}

static vector_uint2 CGSizeToVectorUInt2(const CGSize &size) {
    return simd_make_uint2(size.width, size.height);
}

static const size_t iTermPIUArrayIndexNotUnderlined = 0;
static const size_t iTermPIUArrayIndexUnderlined = 1;         // This can be used as the Underlined bit in this bitmask
static const size_t iTermPIUArrayIndexNotUnderlinedEmoji = 2; // This can be used as the Emoji bit in this bitmask
static const size_t iTermPIUArrayIndexUnderlinedEmoji = 3;
static const size_t iTermPIUArraySize = 4;

static const size_t iTermNumberOfPIUArrays = iTermASCIITextureAttributesMax * 2;

@implementation iTermTextRendererTransientState {
    DashTerm2::PIUArray<iTermTextPIU> _asciiPIUArrays[iTermPIUArraySize][iTermNumberOfPIUArrays];
    DashTerm2::PIUArray<iTermTextPIU> _asciiOverflowArrays[iTermPIUArraySize][iTermNumberOfPIUArrays];

    // Array of PIUs for each texture page.
    // Use unordered_map for O(1) lookup instead of std::map's O(log n).
    // Key is a pointer which is trivially hashable.
    std::unordered_map<DashTerm2::TexturePage *, DashTerm2::PIUArray<iTermTextPIU> *> _pius[iTermPIUArraySize];

    iTermPreciseTimerStats _stats[iTermTextRendererStatCount];
}

- (void)dealloc {
    for (size_t i = 0; i < iTermPIUArraySize; i++) {
        for (auto it = _pius[i].begin(); it != _pius[i].end(); it++) {
            delete it->second;
        }
    }
}

+ (NSString *)formatTextPIU:(iTermTextPIU)a {
    return [NSString stringWithFormat:@"offset=(%@, %@) "
                                      @"textureOffset=(%@, %@) "
                                      @"textColor=(%@, %@, %@, %@) "
                                      @"underlineStyle=%@ "
                                      @"underlineColor=(%@, %@, %@, %@)\n",
                                      @(a.offset.x), @(a.offset.y), @(a.textureOffset.x), @(a.textureOffset.y),
                                      @(a.textColor.x), @(a.textColor.y), @(a.textColor.z), @(a.textColor.w),
                                      @(a.underlineStyle), @(a.underlineColor.x), @(a.underlineColor.y),
                                      @(a.underlineColor.z), @(a.underlineColor.w)];
}

- (void)writeDebugInfoToFolder:(NSURL *)folder {
    [super writeDebugInfoToFolder:folder];

    [_modelData writeToURL:[folder URLByAppendingPathComponent:@"model.bin"] atomically:NO];

    @autoreleasepool {
        for (int k = 0; k < iTermPIUArraySize; k++) {
            for (int i = 0; i < sizeof(_asciiPIUArrays[k]) / sizeof(**_asciiPIUArrays); i++) {
                const int size = _asciiPIUArrays[k][i].size();
                NSMutableString *s = [[NSMutableString alloc] initWithCapacity:size * 128];
                for (int j = 0; j < size; j++) {
                    const iTermTextPIU &a = _asciiPIUArrays[k][i].get(j);
                    [s appendString:[self.class formatTextPIU:a]];
                }
                NSMutableString *name = [NSMutableString stringWithFormat:@"asciiPIUs.CenterPart."];
                if (i & iTermASCIITextureAttributesBold) {
                    [name appendString:@"B"];
                }
                if (i & iTermASCIITextureAttributesItalic) {
                    [name appendString:@"I"];
                }
                if (i & iTermASCIITextureAttributesThinStrokes) {
                    [name appendString:@"T"];
                }
                if (k == iTermPIUArrayIndexUnderlined || k == iTermPIUArrayIndexUnderlinedEmoji) {
                    [name appendString:@"U"];
                }
                if (k == iTermPIUArrayIndexNotUnderlinedEmoji || k == iTermPIUArrayIndexUnderlinedEmoji) {
                    [name appendString:@"E"];
                }
                [name appendString:@".txt"];
                [s writeToURL:[folder URLByAppendingPathComponent:name]
                    atomically:NO
                      encoding:NSUTF8StringEncoding
                         error:nil];
            }
        }
    }

    @autoreleasepool {
        for (int k = 0; k < iTermPIUArraySize; k++) {
            for (int i = 0; i < sizeof(_asciiOverflowArrays[k]) / sizeof(**_asciiOverflowArrays); i++) {
                const int size = _asciiOverflowArrays[k][i].size();
                NSMutableString *s = [[NSMutableString alloc] initWithCapacity:size * 128];
                for (int j = 0; j < size; j++) {
                    const iTermTextPIU &a = _asciiOverflowArrays[k][i].get(j);
                    [s appendString:[self.class formatTextPIU:a]];
                }
                NSMutableString *name = [NSMutableString stringWithFormat:@"asciiPIUs.Overflow."];
                if (i & iTermASCIITextureAttributesBold) {
                    [name appendString:@"B"];
                }
                if (i & iTermASCIITextureAttributesItalic) {
                    [name appendString:@"I"];
                }
                if (i & iTermASCIITextureAttributesThinStrokes) {
                    [name appendString:@"T"];
                }
                [name appendString:@".txt"];
                [s writeToURL:[folder URLByAppendingPathComponent:name]
                    atomically:NO
                      encoding:NSUTF8StringEncoding
                         error:nil];
            }
        }
    }

    @autoreleasepool {
        for (int k = 0; k < iTermPIUArraySize; k++) {
            NSMutableString *s = [[NSMutableString alloc] initWithCapacity:_pius[k].size() * 256];
            // Use const reference to avoid copying the map pair on each iteration
            for (const auto &entry : _pius[k]) {
                const DashTerm2::TexturePage *texturePage = entry.first;
                DashTerm2::PIUArray<iTermTextPIU> *piuArray = entry.second;
                [s appendFormat:@"Texture Page with texture %@:\n", texturePage->get_texture().label];
                if (piuArray) {
                    for (int j = 0; j < piuArray->size(); j++) {
                        iTermTextPIU &piu = piuArray->get(j);
                        [s appendString:[self.class formatTextPIU:piu]];
                    }
                }
            }
            NSString *name = @"non-ascii-pius.txt";
            if (k & iTermPIUArrayIndexUnderlined) {
                name = [@"underlined-" stringByAppendingString:name];
            }
            if (k & iTermPIUArrayIndexNotUnderlinedEmoji) {
                name = [@"emoji-" stringByAppendingString:name];
            }
            [s writeToURL:[folder URLByAppendingPathComponent:name]
                atomically:NO
                  encoding:NSUTF8StringEncoding
                     error:nil];
        }
    }

    NSString *s = [NSString
        stringWithFormat:@"backgroundTexture=%@\nasciiUnderlineDescriptor=%@\nnonAsciiUnderlineDescriptor=%@"
                         @"\nstrikethroughUnderlineDescriptor=%@",
                         _backgroundTexture, iTermMetalUnderlineDescriptorDescription(&_asciiUnderlineDescriptor),
                         iTermMetalUnderlineDescriptorDescription(&_nonAsciiUnderlineDescriptor),
                         iTermMetalUnderlineDescriptorDescription(&_strikethroughUnderlineDescriptor)];
    [s writeToURL:[folder URLByAppendingPathComponent:@"state.txt"]
        atomically:NO
          encoding:NSUTF8StringEncoding
             error:NULL];
}

- (iTermPreciseTimerStats *)stats {
    return _stats;
}

- (int)numberOfStats {
    return iTermTextRendererStatCount;
}

- (NSString *)nameForStat:(int)i {
    return [@[ @"text.newQuad", @"text.newPIU", @"text.newDims", @"text.info", @"text.subpixel", @"text.draw" ]
        objectAtIndex:i];
}

- (BOOL)haveAsciiOverflow {
    for (int k = 0; k < iTermPIUArraySize; k++) {
        for (int i = 0; i < iTermNumberOfPIUArrays; i++) {
            const int n = _asciiOverflowArrays[k][i].get_number_of_segments();
            if (n > 0) {
                for (int j = 0; j < n; j++) {
                    if (_asciiOverflowArrays[k][i].size_of_segment(j) > 0) {
                        return YES;
                    }
                }
            }
        }
    }
    return NO;
}

// Phase 1C optimization: Pre-allocate PIU arrays to avoid segment allocations during rendering.
// For typical terminal content, ASCII characters dominate, so we reserve space in ASCII arrays.
// For CJK-heavy content, the non-ASCII path uses dynamic allocation with numberOfCells capacity,
// which is already optimal since those arrays are created on-demand with the right size.
- (void)preallocatePIUArraysForCellCount:(NSInteger)cellCount {
    if (cellCount <= 0) {
        return;
    }
    // Distribute expected capacity across the ASCII arrays.
    // Most terminals are 80-200 columns by 24-50 rows = 2000-10000 cells.
    // For a typical frame, ASCII characters are split across:
    // - 4 underline/emoji combinations (iTermPIUArraySize)
    // - Multiple ASCII attribute combinations (bold, italic, etc.)
    // We use a conservative estimate: reserve enough for all cells to be in one array,
    // which handles the worst case while avoiding over-allocation.
    const size_t reservePerArray = static_cast<size_t>(cellCount);

    for (int k = 0; k < iTermPIUArraySize; k++) {
        for (int i = 0; i < iTermNumberOfPIUArrays; i++) {
            _asciiPIUArrays[k][i].reserve(reservePerArray);
        }
    }
}

- (void)enumerateASCIIDrawsFromArrays:(DashTerm2::PIUArray<iTermTextPIU> *)piuArrays
                           underlined:(BOOL)underlined
                                emoji:(BOOL)emoji
                                block:(void (^)(const iTermTextPIU *, NSInteger, id<MTLTexture>, vector_uint2,
                                                vector_uint2, iTermMetalUnderlineDescriptor,
                                                iTermMetalUnderlineDescriptor, BOOL underlined, BOOL emoji))block {
    // ASCII glyphs are shifted by self.asciiOffset.height pixels relative to non-ascii glyphs.
    // The underlines would come along with them so we adjust them here. Issue 10168.
    iTermMetalUnderlineDescriptor adjustedASCIIUnderlineDescriptor = _asciiUnderlineDescriptor;
    adjustedASCIIUnderlineDescriptor.offset -= self.asciiOffset.height / self.configuration.scale;
    vector_uint2 augmentedGlyphSize = CGSizeToVectorUInt2(_asciiTextureGroup.glyphSize);
    if (iTermTextIsMonochrome()) {
        // There is only a center part for ASCII on Mojave because the glyph size is increased to contain the largest
        // ASCII glyph, notwithstanding the ascii offset. If we don't account for ascii offset here than glyphs that
        // spill into the right (such as Italic often does) can be truncated.
        augmentedGlyphSize.x += self.asciiOffset.width;
        augmentedGlyphSize.y += self.asciiOffset.height;
    }
    for (int i = 0; i < iTermNumberOfPIUArrays; i++) {
        const int n = piuArrays[i].get_number_of_segments();
        iTermASCIITexture *asciiTexture = [_asciiTextureGroup asciiTextureForAttributes:(iTermASCIITextureAttributes)i];
        ITBetaAssert(asciiTexture, @"nil ascii texture for attributes %d", i);
        for (int j = 0; j < n; j++) {
            if (piuArrays[i].size_of_segment(j) > 0) {
                block(piuArrays[i].start_of_segment(j), piuArrays[i].size_of_segment(j),
                      asciiTexture.textureArray.texture, CGSizeToVectorUInt2(asciiTexture.textureArray.atlasSize),
                      augmentedGlyphSize, adjustedASCIIUnderlineDescriptor, _strikethroughUnderlineDescriptor,
                      underlined, emoji);
            }
        }
    }
}

- (size_t)enumerateNonASCIIDraws:(void (^)(const iTermTextPIU *, NSInteger, id<MTLTexture>, vector_uint2, vector_uint2,
                                           iTermMetalUnderlineDescriptor, iTermMetalUnderlineDescriptor,
                                           BOOL underlined, BOOL emoji))block {
    size_t sum = 0;
    for (size_t k = 0; k < iTermPIUArraySize; k++) {
        for (auto const &mapPair : _pius[k]) {
            const DashTerm2::TexturePage *const &texturePage = mapPair.first;
            const DashTerm2::PIUArray<iTermTextPIU> *const &piuArray = mapPair.second;

            for (size_t i = 0; i < piuArray->get_number_of_segments(); i++) {
                const size_t count = piuArray->size_of_segment(i);
                if (count > 0) {
                    sum += count;
                    block(piuArray->start_of_segment(i), count, texturePage->get_texture(),
                          texturePage->get_atlas_size(), texturePage->get_cell_size(), _nonAsciiUnderlineDescriptor,
                          _strikethroughUnderlineDescriptor, !!(k & iTermPIUArrayIndexUnderlined),
                          !!(k & iTermPIUArrayIndexNotUnderlinedEmoji));
                }
            }
        }
    }
    return sum;
}

- (void)enumerateASCIIDrawsFromArrays:
            (DashTerm2::PIUArray<iTermTextPIU>[iTermPIUArraySize][iTermNumberOfPIUArrays])piuArrays
                                block:(void (^)(const iTermTextPIU *, NSInteger, id<MTLTexture>, vector_uint2,
                                                vector_uint2, iTermMetalUnderlineDescriptor,
                                                iTermMetalUnderlineDescriptor, BOOL, BOOL))block {
    [self enumerateASCIIDrawsFromArrays:piuArrays[iTermPIUArrayIndexUnderlined]
                             underlined:true
                                  emoji:false
                                  block:block];
    [self enumerateASCIIDrawsFromArrays:piuArrays[iTermPIUArrayIndexUnderlinedEmoji]
                             underlined:true
                                  emoji:true
                                  block:block];
    [self enumerateASCIIDrawsFromArrays:piuArrays[iTermPIUArrayIndexNotUnderlined]
                             underlined:false
                                  emoji:false
                                  block:block];
    [self enumerateASCIIDrawsFromArrays:piuArrays[iTermPIUArrayIndexNotUnderlinedEmoji]
                             underlined:false
                                  emoji:true
                                  block:block];
}

- (void)enumerateDraws:(void (^)(const iTermTextPIU *, NSInteger, id<MTLTexture>, vector_uint2, vector_uint2,
                                 iTermMetalUnderlineDescriptor, iTermMetalUnderlineDescriptor, BOOL underlined,
                                 BOOL emoji))block
             copyBlock:(void (^)(void))copyBlock {
    [self enumerateNonASCIIDraws:block];
    [self enumerateASCIIDrawsFromArrays:_asciiPIUArrays block:block];
    if ([self haveAsciiOverflow]) {
        copyBlock();
        [self enumerateASCIIDrawsFromArrays:_asciiOverflowArrays block:block];
    }
}

- (void)willDraw {
    DLog(@"WILL DRAW %@", self);

    // Flush any staged texture uploads before rendering.
    // This ensures all glyph bitmaps are uploaded to GPU.
    _texturePageCollectionSharedPointer.object->flush_all_staged_uploads();

    for (int k = 0; k < iTermPIUArraySize; k++) {
        // Use const reference to avoid copying the map pair on each iteration
        for (const auto &pair : _pius[k]) {
            DashTerm2::TexturePage *page = pair.first;
            page->record_use();
        }
    }
    DLog(@"END WILL DRAW");
}

NS_INLINE iTermTextPIU *iTermTextRendererTransientStateAddASCIIPart(
    iTermTextPIU *piu, char code, float w, float h, iTermASCIITexture *texture, float cellWidth, int visualColumn,
    CGSize asciiOffset, iTermASCIITextureOffset offset, vector_float4 textColor,
    iTermMetalGlyphAttributesUnderline underlineStyle, vector_float4 underlineColor) {
    piu->offset = simd_make_float2(visualColumn * cellWidth + asciiOffset.width, asciiOffset.height);
    const int index = iTermASCIITextureIndexOfCode(code, offset);
    MTLOrigin origin = iTermTextureArrayOffsetForIndex(texture.textureArray, index);
    piu->textureOffset = (vector_float2){origin.x * w, origin.y * h};
    piu->textColor = textColor;
    piu->underlineStyle = underlineStyle;
    piu->underlineColor = underlineColor;
    piu->alphaVector = iTermComputeAlphaVectorForTextColor(textColor);
    return piu;
}

static inline int iTermOuterPIUIndex(const bool &annotation, const bool &underlined, const bool &emoji) {
    const int underlineBit = (annotation || underlined) ? iTermPIUArrayIndexUnderlined : 0;
    const int emojiBit = emoji ? iTermPIUArrayIndexNotUnderlinedEmoji : 0;
    return underlineBit | emojiBit;
}

- (void)addASCIICellToPIUsForCode:(char)code
                     logicalIndex:(int)logicalIndex
                     visualColumn:(int)visualColumn
                           offset:(CGSize)asciiOffset
                                w:(float)w
                                h:(float)h
                        cellWidth:(float)cellWidth
                       asciiAttrs:(iTermASCIITextureAttributes)asciiAttrs
                       attributes:(const iTermMetalGlyphAttributes *)attributes
                    inMarkedRange:(BOOL)inMarkedRange {
    // When profiling, objc_retain and objc_release took 20% of the time in this function, which
    // is called super frequently. It should be safe not to retain the texture because the texture
    // group retains it. Once a texture is set in the group it won't be removed or replaced.
    __unsafe_unretained iTermASCIITexture *texture = _asciiTextureGroup->_textures[asciiAttrs];
    if (!texture) {
        texture = [_asciiTextureGroup asciiTextureForAttributes:asciiAttrs];
    }

    // BUG-1109: Validate code is in valid range [0, 127] before accessing parts array
    const size_t safeCode = (size_t)((unsigned char)code);
    if (safeCode >= 128) {
        return; // Invalid code, skip rendering
    }
    iTermASCIITextureParts parts = texture.parts[safeCode];
    vector_float4 underlineColor = {0, 0, 0, 0};

    const bool &hasAnnotation = attributes[visualColumn].annotation;
    const bool hasUnderline = attributes[visualColumn].underlineStyle != iTermMetalGlyphAttributesUnderlineNone;
    const int outerPIUIndex = iTermOuterPIUIndex(hasAnnotation, hasUnderline, false);
    if (hasAnnotation) {
        underlineColor = iTermAnnotationUnderlineColor;
    } else if (hasUnderline) {
        if (attributes[visualColumn].hasUnderlineColor) {
            underlineColor = attributes[visualColumn].underlineColor;
        } else {
            underlineColor = _asciiUnderlineDescriptor.color.w > 0 ? _asciiUnderlineDescriptor.color
                                                                   : attributes[visualColumn].foregroundColor;
        }
    }

    iTermMetalGlyphAttributesUnderline underlineStyle = attributes[visualColumn].underlineStyle;
    vector_float4 textColor = attributes[visualColumn].foregroundColor;
    if (inMarkedRange) {
        // Marked range gets a yellow underline.
        underlineStyle = iTermMetalGlyphAttributesUnderlineSingle;
    }

    if (iTermTextIsMonochrome()) {
        // There is only a center part for ASCII on Mojave because the glyph size is increased to contain the largest
        // ASCII glyph.
        iTermTextRendererTransientStateAddASCIIPart(
            _asciiPIUArrays[outerPIUIndex][asciiAttrs].get_next(), code, w, h, texture, cellWidth, visualColumn,
            asciiOffset, iTermASCIITextureOffsetCenter, textColor, underlineStyle, underlineColor);
        return;
    }
    // Pre-10.14, ASCII glyphs can get chopped up into multiple parts. This is necessary so subpixel AA will work right.

    // Add PIU for left overflow
    if (parts & iTermASCIITexturePartsLeft) {
        iTermTextRendererTransientStateAddASCIIPart(_asciiOverflowArrays[outerPIUIndex][asciiAttrs].get_next(), code, w,
                                                    h, texture, cellWidth, visualColumn - 1, asciiOffset,
                                                    iTermASCIITextureOffsetLeft, textColor,
                                                    iTermMetalGlyphAttributesUnderlineNone, underlineColor);
    }

    // Add PIU for center part, which is always present
    iTermTextRendererTransientStateAddASCIIPart(
        _asciiPIUArrays[outerPIUIndex][asciiAttrs].get_next(), code, w, h, texture, cellWidth, visualColumn,
        asciiOffset, iTermASCIITextureOffsetCenter, textColor, underlineStyle, underlineColor);
    // Add PIU for right overflow
    if (parts & iTermASCIITexturePartsRight) {

        iTermTextRendererTransientStateAddASCIIPart(
            _asciiOverflowArrays[outerPIUIndex][asciiAttrs].get_next(), code, w, h, texture, cellWidth,
            visualColumn + 1, asciiOffset, iTermASCIITextureOffsetRight, attributes[visualColumn].foregroundColor,
            iTermMetalGlyphAttributesUnderlineNone, underlineColor);
    }
}

static inline BOOL GlyphKeyCanTakeASCIIFastPath(const iTermMetalGlyphKey &glyphKey) {
    return (glyphKey.type == iTermMetalGlyphTypeRegular &&
            glyphKey.payload.regular.code <= iTermASCIITextureMaximumCharacter &&
            glyphKey.payload.regular.code >= iTermASCIITextureMinimumCharacter && !glyphKey.payload.regular.isComplex &&
            !glyphKey.payload.regular.boxDrawing);
}

// Forward declaration for inline ASCII processing
struct iTermASCIIFastPathContext;

// Inline C++ function to process non-ASCII glyphs without Objective-C dispatch.
// Using inline C++ avoids objc_msgSend overhead for every non-ASCII glyph.
static inline void
iTermAddNonASCIIGlyphInline(const iTermMetalGlyphKey &theGlyphKey, int stateIndex, float yOffset, CGSize cellSize,
                            CGSize glyphSize, const iTermMetalGlyphAttributes *attributes, BOOL inMarkedRange,
                            BOOL allowUnderline, iTermMetalBufferPoolContext *context,
                            NSDictionary<NSNumber *, iTermCharacterBitmap *> * (^creation)(int x, BOOL *emoji),
                            iTermTexturePageCollectionSharedPointer *texturePageCollectionSharedPointer,
                            std::unordered_map<DashTerm2::TexturePage *, DashTerm2::PIUArray<iTermTextPIU> *> *pius,
                            NSInteger numberOfCells, const iTermMetalUnderlineDescriptor *nonAsciiUnderlineDescriptor) {
    const int visualIndex = theGlyphKey.visualColumn;

    const DashTerm2::GlyphKey glyphKey(&theGlyphKey);
    std::vector<const DashTerm2::GlyphEntry *> *entries = texturePageCollectionSharedPointer.object->find(glyphKey);
    if (!entries) {
        entries = texturePageCollectionSharedPointer.object->add(stateIndex, glyphKey, context, creation);
        if (!entries) {
            return;
        }
    } else if (entries->empty()) {
        return;
    }

    const bool &hasAnnotation = attributes[visualIndex].annotation;
    const bool hasUnderline = attributes[visualIndex].underlineStyle != iTermMetalGlyphAttributesUnderlineNone;
    const DashTerm2::GlyphEntry *firstGlyphEntry = (*entries)[0];
    const int outerPIUIndex = iTermOuterPIUIndex(hasAnnotation, hasUnderline, firstGlyphEntry->_is_emoji);
    // Use const reference to avoid copying the pointer on each iteration
    for (const auto &entry : *entries) {
        auto it = pius[outerPIUIndex].find(entry->_page);
        DashTerm2::PIUArray<iTermTextPIU> *array;
        if (it == pius[outerPIUIndex].end()) {
            array = pius[outerPIUIndex][entry->_page] = new DashTerm2::PIUArray<iTermTextPIU>(numberOfCells);
        } else {
            array = it->second;
        }
        iTermTextPIU *piu = array->get_next();
        // Build the PIU
        const int &part = entry->_part;
        const int dx = iTermImagePartDX(part);
        const int dy = iTermImagePartDY(part);
        piu->offset = simd_make_float2(theGlyphKey.visualColumn * cellSize.width + dx * glyphSize.width,
                                       -dy * glyphSize.height + yOffset);
        MTLOrigin origin = entry->get_origin();
        vector_float2 reciprocal_atlas_size = entry->_page->get_reciprocal_atlas_size();
        piu->textureOffset = simd_make_float2(origin.x * reciprocal_atlas_size.x, origin.y * reciprocal_atlas_size.y);
        piu->textColor = attributes[visualIndex].foregroundColor;
        if (attributes[visualIndex].annotation) {
            piu->underlineStyle = iTermMetalGlyphAttributesUnderlineSingle;
            piu->underlineColor = iTermAnnotationUnderlineColor;
        } else if (inMarkedRange) {
            piu->underlineStyle = iTermMetalGlyphAttributesUnderlineSingle;
            piu->underlineColor =
                nonAsciiUnderlineDescriptor->color.w > 1 ? nonAsciiUnderlineDescriptor->color : piu->textColor;
        } else {
            piu->underlineStyle = attributes[visualIndex].underlineStyle;
            if (attributes[visualIndex].hasUnderlineColor) {
                piu->underlineColor = attributes[visualIndex].underlineColor;
            } else {
                piu->underlineColor =
                    nonAsciiUnderlineDescriptor->color.w > 1 ? nonAsciiUnderlineDescriptor->color : piu->textColor;
            }
        }
        if (part != iTermTextureMapMiddleCharacterPart && part != iTermTextureMapMiddleCharacterPart + 1) {
            // Only underline center part and its right neighbor of the character. There are weird artifacts otherwise,
            // such as floating underlines (for parts above and below) or doubly drawn
            // underlines.
            piu->underlineStyle = iTermMetalGlyphAttributesUnderlineNone;
        }
        // Pre-compute alpha vector for monochrome rendering path
        piu->alphaVector = iTermComputeAlphaVectorForTextColor(piu->textColor);
    }
}

// Inline C++ function to process ASCII glyphs without Objective-C dispatch.
// Using inline C++ avoids objc_msgSend overhead (~20% of rendering time).
static inline void
iTermAddASCIIGlyphInline(const iTermMetalGlyphKey &theGlyphKey, float asciiXOffset, float yOffset, float asciiYOffset,
                         vector_float2 reciprocalAsciiAtlasSize, CGSize cellSize,
                         const iTermMetalGlyphAttributes *attributes, BOOL inMarkedRange,
                         iTermASCIITextureGroup *__unsafe_unretained asciiTextureGroup,
                         DashTerm2::PIUArray<iTermTextPIU> (*asciiPIUArrays)[iTermNumberOfPIUArrays],
                         DashTerm2::PIUArray<iTermTextPIU> (*asciiOverflowArrays)[iTermNumberOfPIUArrays],
                         const iTermMetalUnderlineDescriptor *asciiUnderlineDescriptor) {

    const char code = theGlyphKey.payload.regular.code;
    const int visualColumn = theGlyphKey.visualColumn;

    iTermASCIITextureAttributes asciiAttrs =
        iTermASCIITextureAttributesFromGlyphKeyTypeface(theGlyphKey.typeface, theGlyphKey.thinStrokes);

    // Get texture without retain (safe because texture group retains it)
    __unsafe_unretained iTermASCIITexture *texture = asciiTextureGroup->_textures[asciiAttrs];
    if (!texture) {
        texture = [asciiTextureGroup asciiTextureForAttributes:asciiAttrs];
    }

    const float w = reciprocalAsciiAtlasSize.x;
    const float h = reciprocalAsciiAtlasSize.y;
    const float cellWidth = cellSize.width;
    CGSize asciiOffset = CGSizeMake(asciiXOffset, yOffset + asciiYOffset);

    // Compute underline color
    vector_float4 underlineColor = {0, 0, 0, 0};
    const bool &hasAnnotation = attributes[visualColumn].annotation;
    const bool hasUnderline = attributes[visualColumn].underlineStyle != iTermMetalGlyphAttributesUnderlineNone;
    const int outerPIUIndex = iTermOuterPIUIndex(hasAnnotation, hasUnderline, false);

    if (hasAnnotation) {
        underlineColor = iTermAnnotationUnderlineColor;
    } else if (hasUnderline) {
        if (attributes[visualColumn].hasUnderlineColor) {
            underlineColor = attributes[visualColumn].underlineColor;
        } else {
            underlineColor = asciiUnderlineDescriptor->color.w > 0 ? asciiUnderlineDescriptor->color
                                                                   : attributes[visualColumn].foregroundColor;
        }
    }

    iTermMetalGlyphAttributesUnderline underlineStyle = attributes[visualColumn].underlineStyle;
    vector_float4 textColor = attributes[visualColumn].foregroundColor;
    if (inMarkedRange) {
        underlineStyle = iTermMetalGlyphAttributesUnderlineSingle;
    }

    // On macOS 10.14+ (Mojave), always use monochrome path (only center part)
    if (iTermTextIsMonochrome()) {
        iTermTextRendererTransientStateAddASCIIPart(
            asciiPIUArrays[outerPIUIndex][asciiAttrs].get_next(), code, w, h, texture, cellWidth, visualColumn,
            asciiOffset, iTermASCIITextureOffsetCenter, textColor, underlineStyle, underlineColor);
        return;
    }

    // Pre-10.14 path: may need left/center/right parts for subpixel AA
    // BUG-1109: Validate code is in valid range [0, 127] before accessing parts array
    const size_t safeCode = (size_t)((unsigned char)code);
    if (safeCode >= 128) {
        return; // Invalid code, skip rendering
    }
    iTermASCIITextureParts parts = texture.parts[safeCode];

    if (parts & iTermASCIITexturePartsLeft) {
        iTermTextRendererTransientStateAddASCIIPart(asciiOverflowArrays[outerPIUIndex][asciiAttrs].get_next(), code, w,
                                                    h, texture, cellWidth, visualColumn - 1, asciiOffset,
                                                    iTermASCIITextureOffsetLeft, textColor,
                                                    iTermMetalGlyphAttributesUnderlineNone, underlineColor);
    }

    iTermTextRendererTransientStateAddASCIIPart(
        asciiPIUArrays[outerPIUIndex][asciiAttrs].get_next(), code, w, h, texture, cellWidth, visualColumn, asciiOffset,
        iTermASCIITextureOffsetCenter, textColor, underlineStyle, underlineColor);

    if (parts & iTermASCIITexturePartsRight) {
        iTermTextRendererTransientStateAddASCIIPart(
            asciiOverflowArrays[outerPIUIndex][asciiAttrs].get_next(), code, w, h, texture, cellWidth, visualColumn + 1,
            asciiOffset, iTermASCIITextureOffsetRight, attributes[visualColumn].foregroundColor,
            iTermMetalGlyphAttributesUnderlineNone, underlineColor);
    }
}

typedef struct {
    const iTermMetalGlyphKey *glyphKeys;
    int i;
    float asciiXOffset;
    float asciiYOffset;
    float yOffset;
    vector_float2 reciprocalAsciiAtlasSize;
    CGSize cellSize;
    CGSize glyphSize;
    const iTermMetalGlyphAttributes *attributes;
    BOOL inMarkedRange;
    iTermMetalBufferPoolContext *context;
    NSDictionary<NSNumber *, iTermCharacterBitmap *> * (^creation)(int x, BOOL *emoji);
} iTermTextRendererGlyphState;

- (void)setGlyphKeysData:(iTermGlyphKeyData *)glyphKeysData
             glyphKeyCount:(NSUInteger)glyphKeyCount
                     count:(int)count
            attributesData:(iTermAttributesData *)attributesData
                       row:(int)row
    backgroundColorRLEData:(nonnull iTermData *)backgroundColorRLEData
         markedRangeOnLine:(NSRange)markedRangeOnLine
                   context:(iTermMetalBufferPoolContext *)context
                  creation:(NSDictionary<NSNumber *, iTermCharacterBitmap *> *(NS_NOESCAPE ^)(int x, BOOL *emoji))
                               creation {
    // DLog(@"BEGIN setGlyphKeysData for %@", self);
    const iTermMetalGlyphKey *glyphKeys = (iTermMetalGlyphKey *)glyphKeysData.bytes;
    const iTermMetalGlyphAttributes *attributes = (iTermMetalGlyphAttributes *)attributesData.bytes;
    vector_float2 reciprocalAsciiAtlasSize = 1.0 / _asciiTextureGroup.atlasSize;
    CGSize glyphSize = self.cellConfiguration.glyphSize;
    const CGSize cellSize = self.cellConfiguration.cellSize;
    const float cellHeight = self.cellConfiguration.cellSize.height;
    // NOTE: This must match logic in -[iTermCharacterSource drawBoxAtOffset:iteration:]
    const float verticalShift =
        round((cellHeight - self.cellConfiguration.cellSizeWithoutSpacing.height) / (2 * self.configuration.scale)) *
        self.configuration.scale;
    const float yOffset = (self.cellConfiguration.gridSize.height - row - 1) * cellHeight + verticalShift;
    const float asciiYOffset = -self.asciiOffset.height;
    const float asciiXOffset = -self.asciiOffset.width;

    iTermTextRendererGlyphState state = {.glyphKeys = glyphKeys,
                                         .asciiXOffset = asciiXOffset,
                                         .asciiYOffset = asciiYOffset,
                                         .yOffset = yOffset,
                                         .reciprocalAsciiAtlasSize = reciprocalAsciiAtlasSize,
                                         .cellSize = cellSize,
                                         .glyphSize = glyphSize,
                                         .attributes = attributes,
                                         .context = context,
                                         .creation = creation};
    int previousLogicalIndex = -1;

    // Cache instance variables for inline ASCII path to avoid repeated ivar access
    iTermASCIITextureGroup *__unsafe_unretained asciiTextureGroup = _asciiTextureGroup;
    DashTerm2::PIUArray<iTermTextPIU>(*asciiPIUArrays)[iTermNumberOfPIUArrays] = _asciiPIUArrays;
    DashTerm2::PIUArray<iTermTextPIU>(*asciiOverflowArrays)[iTermNumberOfPIUArrays] = _asciiOverflowArrays;
    const iTermMetalUnderlineDescriptor *asciiUnderlineDescriptor = &_asciiUnderlineDescriptor;

    // Cache instance variables for inline non-ASCII path to avoid repeated ivar access
    iTermTexturePageCollectionSharedPointer *texturePageCollectionSharedPointer = _texturePageCollectionSharedPointer;
    std::unordered_map<DashTerm2::TexturePage *, DashTerm2::PIUArray<iTermTextPIU> *> *pius = _pius;
    NSInteger numberOfCells = _numberOfCells;
    const iTermMetalUnderlineDescriptor *nonAsciiUnderlineDescriptor = &_nonAsciiUnderlineDescriptor;

    for (state.i = 0; state.i < glyphKeyCount; state.i++) {
        const int logicalIndex = glyphKeys[state.i].logicalIndex;
        const int visualIndex = glyphKeys[state.i].visualColumn;
        state.inMarkedRange = NSLocationInRange(visualIndex, markedRangeOnLine);

        switch (glyphKeys[state.i].type) {
            case iTermMetalGlyphTypeRegular:
                if (!glyphKeys[state.i].payload.regular.drawable) {
                    break;
                }
                if (GlyphKeyCanTakeASCIIFastPath(glyphKeys[state.i])) {
                    // Use inline C++ function to avoid Objective-C dispatch overhead.
                    // This was measured to save ~20% of rendering time on ASCII-heavy content.
                    iTermAddASCIIGlyphInline(glyphKeys[state.i], asciiXOffset, yOffset, asciiYOffset,
                                             reciprocalAsciiAtlasSize, cellSize, attributes, state.inMarkedRange,
                                             asciiTextureGroup, asciiPIUArrays, asciiOverflowArrays,
                                             asciiUnderlineDescriptor);
                } else {
                    // Use inline C++ function to avoid Objective-C dispatch overhead.
                    // Similar to ASCII path optimization.
                    iTermAddNonASCIIGlyphInline(glyphKeys[state.i], state.i, yOffset, cellSize, glyphSize, attributes,
                                                state.inMarkedRange, YES, context, creation,
                                                texturePageCollectionSharedPointer, pius, numberOfCells,
                                                nonAsciiUnderlineDescriptor);
                }
                break;
            case iTermMetalGlyphTypeDecomposed:
                // Use inline C++ function to avoid Objective-C dispatch overhead.
                iTermAddNonASCIIGlyphInline(glyphKeys[state.i], state.i, yOffset, cellSize, glyphSize, attributes,
                                            state.inMarkedRange, logicalIndex != previousLogicalIndex, context,
                                            creation, texturePageCollectionSharedPointer, pius, numberOfCells,
                                            nonAsciiUnderlineDescriptor);
                break;
        }
        previousLogicalIndex = logicalIndex;
    }
}

- (void)didComplete {
    DLog(@"BEGIN didComplete for %@", self);
    _texturePageCollectionSharedPointer.object
        ->prune_if_needed(); // The static analyzer wrongly says this is a use-after-free.
    DLog(@"END didComplete");
}

- (nonnull NSMutableData *)modelData {
    if (_modelData == nil) {
        _modelData = [[NSMutableData alloc] initWithUninitializedLength:sizeof(iTermTextPIU) *
                                                                        self.cellConfiguration.gridSize.width *
                                                                        self.cellConfiguration.gridSize.height];
    }
    return _modelData;
}

- (void)expireNonASCIIGlyphs {
    _texturePageCollectionSharedPointer.object->remove_all();
}

@end
