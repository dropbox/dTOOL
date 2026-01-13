#import <Metal/Metal.h>

#import "iTermCharacterBitmap.h"
#import "iTermCharacterParts.h"

NS_CLASS_AVAILABLE(10_11, NA)
@interface iTermTextureArray : NSObject {
@public
    uint32_t _width;
    uint32_t _height;
    NSInteger _cellsPerRow;
}

@property (nonatomic, readonly) id <MTLTexture> texture;
@property (nonatomic, readonly) uint32_t width;
@property (nonatomic, readonly) uint32_t height;
@property (nonatomic, readonly) NSUInteger count;
@property (nonatomic, readonly) CGSize atlasSize;

// Number of glyphs staged but not yet uploaded to GPU
@property (nonatomic, readonly) NSUInteger pendingUploadCount;

+ (CGSize)atlasSizeForUnitSize:(CGSize)unitSize
                   arrayLength:(NSUInteger)length
                   cellsPerRow:(out NSInteger *)cellsPerRowOut;

- (instancetype)initWithTextureWidth:(uint32_t)width
                       textureHeight:(uint32_t)height
                         arrayLength:(NSUInteger)length
                         pixelFormat:(MTLPixelFormat)pixelFormat
                              device:(id <MTLDevice>)device;
- (instancetype)initWithImages:(NSArray<NSImage *> *)images device:(id <MTLDevice>)device;

- (BOOL)addSliceWithContentsOfFile:(NSString *)path;
- (void)addSliceWithImage:(NSImage *)image;
- (BOOL)setSlice:(NSUInteger)slice withImage:(NSImage *)nsimage;
- (void)setSlice:(NSUInteger)slice withBitmap:(iTermCharacterBitmap *)bitmap;

// Batched upload API - stages glyph bitmap in CPU buffer, uploads on flush.
// This reduces GPU overhead by combining multiple per-glyph uploads into fewer operations.

// Stage a bitmap for batch upload. Does NOT upload to GPU yet.
// The slice can be used immediately after staging (data is copied).
- (void)stageBitmapForSlice:(NSUInteger)slice withBitmap:(iTermCharacterBitmap *)bitmap;

// Upload all staged bitmaps to GPU in optimized batches.
// Call this before rendering to ensure all staged data is available.
// Automatically called when the array is full or on dealloc.
- (void)flushStagedBitmaps;

- (void)copyTextureAtIndex:(NSInteger)index
                   toArray:(iTermTextureArray *)destination
                     index:(NSInteger)destinationIndex
                   blitter:(id<MTLBlitCommandEncoder>)blitter;
- (MTLOrigin)offsetForIndex:(NSInteger)index;

@end

NS_CLASS_AVAILABLE(10_11, NA)
NS_INLINE MTLOrigin iTermTextureArrayOffsetForIndex(iTermTextureArray *self, const NSInteger index) {
    return MTLOriginMake(self->_width * (index % self->_cellsPerRow),
                         self->_height * (index / self->_cellsPerRow),
                         0);
}
