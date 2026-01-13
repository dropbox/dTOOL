//
//  iTermImageRenderer.m
//  DashTerm2
//
//  Created by George Nachman on 1/7/18.
//

#import <limits.h>

#import "iTermImageRenderer.h"

#import "iTermImageInfo.h"
#import "iTermSharedImageStore.h"
#import "iTermTexture.h"
#import "NSArray+iTerm.h"
#import "NSImage+iTerm.h"

@class iTermImageRenderer;

static NSString *const iTermImageRendererTextureMetadataKeyImageMissing =
    @"iTermImageRendererTextureMetadataKeyImageMissing";

static inline uint64_t iTermImageRendererPackTextureKey(uint32_t colorSpaceIdentifier, uint16_t code, uint16_t frame) {
    return (((uint64_t)colorSpaceIdentifier) << 32) | (((uint64_t)code) << 16) | (uint64_t)frame;
}

static inline uint32_t iTermImageRendererColorSpaceIdentifierFromKey(NSNumber *key) {
    return (uint32_t)(key.unsignedLongLongValue >> 32);
}

@interface iTermImageRenderer (TextureKeys)
- (NSNumber *)textureKeyForCode:(int32_t)code frame:(uint16_t)frame colorSpaceName:(NSString *)colorSpaceName;
- (uint32_t)identifierForColorSpaceName:(NSString *)colorSpaceName;
@end

@interface iTermImageRenderer ()
@property (nonatomic, assign) uint32_t activeColorSpaceIdentifier;
@end

@implementation iTermMetalImageRun

- (NSString *)debugDescription {
    NSString *info = [NSString
        stringWithFormat:@"startCoordInImage=%@ startCoordOnScreen=%@ length=%@ code=%@ size=%@ uniqueIdentifier=%@",
                         VT100GridCoordDescription(self.startingCoordInImage),
                         VT100GridCoordDescription(self.startingCoordOnScreen), @(self.length), @(self.code),
                         NSStringFromSize(_imageInfo.size), _imageInfo.uniqueIdentifier];
    return [NSString stringWithFormat:@"<%@: %p %@>", NSStringFromClass([self class]), self, info];
}

@end

@interface iTermImageRendererTransientState ()
@property (nonatomic) iTermMetalCellRenderer *cellRenderer;
@property (nonatomic) NSTimeInterval timestamp;
// Counts the number of times each texture key is in use. Shared by all transient states.
@property (nonatomic, strong) NSCountedSet<NSNumber *> *counts;
@property (nonatomic, strong) NSMutableDictionary<NSNumber *, id<MTLTexture>> *textures;
@property (nonatomic, weak) iTermImageRenderer *renderer;
@end

@implementation iTermImageRendererTransientState {
    NSMutableArray<iTermMetalImageRun *> *_runs;
    // Images that weren't available
    NSMutableSet<NSString *> *_missingImageUniqueIdentifiers;
    // Images that were available
    NSMutableSet<NSString *> *_foundImageUniqueIdentifiers;
    // Absolute line numbers that contained animation
    NSMutableIndexSet *_animatedLines;
    // Cache the last color space pointer + identifier to avoid per-run lookups.
    NSColorSpace *_cachedColorSpace;
    uint32_t _cachedColorSpaceIdentifier;
    BOOL _hasCachedColorSpaceIdentifier;
}

- (instancetype)initWithConfiguration:(__kindof iTermRenderConfiguration *)configuration {
    self = [super initWithConfiguration:configuration];
    if (self) {
        _missingImageUniqueIdentifiers = [[NSMutableSet alloc] initWithCapacity:8];
        _foundImageUniqueIdentifiers = [[NSMutableSet alloc] initWithCapacity:8];
        _animatedLines = [NSMutableIndexSet indexSet];
    }
    return self;
}

- (void)writeDebugInfoToFolder:(NSURL *)folder {
    [super writeDebugInfoToFolder:folder];
    NSMutableString *s = [[NSMutableString alloc] initWithCapacity:_runs.count * 128 + 256];

    NSString *runsString = [[_runs mapWithBlock:^id(iTermMetalImageRun *run) {
        return [run debugDescription];
    }] componentsJoinedByString:@"\n"];
    [s appendFormat:@"runs:\n%@\n", runsString];

    [s appendFormat:@"missingImageUniqueIDs: %@\n", _missingImageUniqueIdentifiers];
    [s appendFormat:@"foundImageUniqueIDs: %@\n", _foundImageUniqueIdentifiers];
    [s appendFormat:@"animated lines: %@\n", _animatedLines];

    [s writeToURL:[folder URLByAppendingPathComponent:@"state.txt"]
        atomically:NO
          encoding:NSUTF8StringEncoding
             error:NULL];
}

- (void)removeTexturesExceptForColorSpace:(NSColorSpace *)colorSpaceTokeep {
    iTermImageRenderer *renderer = self.renderer;

    if (!colorSpaceTokeep) {
        [_textures removeAllObjects];
        renderer.activeColorSpaceIdentifier = 0;
        return;
    }

    if (!renderer) {
        [_textures removeAllObjects];
        return;
    }

    const uint32_t targetIdentifier = [renderer identifierForColorSpaceName:colorSpaceTokeep.localizedName];
    if (_textures.count == 0) {
        renderer.activeColorSpaceIdentifier = targetIdentifier;
        return;
    }
    if (renderer.activeColorSpaceIdentifier == targetIdentifier) {
        return;
    }
    renderer.activeColorSpaceIdentifier = targetIdentifier;

    __block NSMutableArray<NSNumber *> *keys = nil;
    [_textures enumerateKeysAndObjectsUsingBlock:^(NSNumber *key, id obj, BOOL *stop) {
        if (iTermImageRendererColorSpaceIdentifierFromKey(key) != targetIdentifier) {
            if (!keys) {
                // Typically a few textures are removed during color space change
                keys = [NSMutableArray arrayWithCapacity:4];
            }
            [keys addObject:key];
        }
    }];
    if (keys.count) {
        [_textures removeObjectsForKeys:keys];
    }
}

- (void)addRun:(iTermMetalImageRun *)imageRun {
    // Remove any textures for this run but different color spaces.
    [self removeTexturesExceptForColorSpace:self.configuration.colorSpace];
    if (!_runs) {
        // Images typically have 1-4 runs per visible screen (inline images, sixel, etc.)
        _runs = [NSMutableArray arrayWithCapacity:4];
    }
    [_runs addObject:imageRun];
    id key = [self keyForRun:imageRun colorSpace:self.configuration.colorSpace];
    id<MTLTexture> texture = _textures[key];

    // Check if the image got loaded asynchronously. This happens when decoding an image takes a while.
    if ([iTermTexture metadataForTexture:texture][iTermImageRendererTextureMetadataKeyImageMissing] &&
        imageRun.imageInfo.ready) {
        [_textures removeObjectForKey:key];
    }

    if (_textures[key] == nil) {
        _textures[key] = [self newTextureForImageRun:imageRun];
    } else if (imageRun.imageInfo) {
        [_foundImageUniqueIdentifiers addObject:imageRun.imageInfo.uniqueIdentifier];
    }
    if (imageRun.imageInfo.animated) {
        const long long absoluteLine = imageRun.startingCoordOnScreen.y + _firstVisibleAbsoluteLineNumber;
        if (absoluteLine >= 0) {
            // Absolute line numbers should always be non-negative; guard to avoid NSUInteger underflow.
            [_animatedLines addIndex:(NSUInteger)absoluteLine];
        }
    }
    [_counts addObject:key];
}

- (NSNumber *)keyForRun:(iTermMetalImageRun *)run colorSpace:(NSColorSpace *)colorSpace {
    const uint16_t frame = (uint16_t)([run.imageInfo frameForTimestamp:_timestamp] & 0xffff);
    const uint16_t sanitizedCode = (uint16_t)(run.code & 0xffff);
    const uint32_t identifier = [self identifierForColorSpace:colorSpace];
    return @(iTermImageRendererPackTextureKey(identifier, sanitizedCode, frame));
}

- (uint32_t)identifierForColorSpace:(NSColorSpace *)colorSpace {
    if (_hasCachedColorSpaceIdentifier && _cachedColorSpace == colorSpace) {
        return _cachedColorSpaceIdentifier;
    }
    iTermImageRenderer *renderer = self.renderer;
    if (!renderer) {
        _cachedColorSpace = colorSpace;
        _cachedColorSpaceIdentifier = 0;
        _hasCachedColorSpaceIdentifier = YES;
        return 0;
    }
    NSString *name = colorSpace.localizedName;
    const uint32_t identifier = [renderer identifierForColorSpaceName:name];
    _cachedColorSpace = colorSpace;
    _cachedColorSpaceIdentifier = identifier;
    _hasCachedColorSpaceIdentifier = YES;
    return identifier;
}

- (id<MTLTexture>)newTextureForImageRun:(iTermMetalImageRun *)run {
    CGSize cellSize = self.cellConfiguration.cellSize;
    const CGFloat scale = self.configuration.scale;
    cellSize.width /= scale;
    cellSize.height /= scale;
    NSImage *image = [run.imageInfo imageWithCellSize:cellSize timestamp:_timestamp scale:scale];
    BOOL missing = NO;
    if (!image) {
        DLog(@"Failed to get image. Use placeholder");
        if (!run.imageInfo) {
            image = [NSImage imageOfSize:CGSizeMake(1, 1) color:[NSColor brownColor]];
        } else {
            image = [NSImage imageOfSize:CGSizeMake(1, 1) color:[NSColor grayColor]];
            missing = YES;
        }
    }
    if (run.imageInfo) {
        if (missing) {
            DLog(@"record missing");
            [_missingImageUniqueIdentifiers addObject:run.imageInfo.uniqueIdentifier];
        } else {
            DLog(@"record found");
            [_foundImageUniqueIdentifiers addObject:run.imageInfo.uniqueIdentifier];
        }
    }
    NSImage *flipped = [image it_verticallyFlippedImage];
    DLog(@"Make texture from %@ (original) -> %@ (flipped)", image, flipped);
    id<MTLTexture> texture = [_cellRenderer textureFromImage:[iTermImageWrapper withImage:flipped]
                                                     context:self.poolContext
                                                  colorSpace:self.configuration.colorSpace];
    if (missing) {
        [iTermTexture setMetadataObject:@YES forKey:iTermImageRendererTextureMetadataKeyImageMissing onTexture:texture];
    }
    return texture;
}

- (void)enumerateDraws:(void (^)(NSNumber *, id<MTLBuffer>, id<MTLTexture>))block {
    const CGSize cellSize = self.cellConfiguration.cellSize;
    const CGPoint offset = CGPointMake(self.margins.left, self.margins.bottom);
    const CGFloat height = self.configuration.viewportSize.y;
    const CGFloat scale = self.configuration.scale;

    [_runs enumerateObjectsUsingBlock:^(iTermMetalImageRun *_Nonnull run, NSUInteger idx, BOOL *_Nonnull stop) {
        id key = [self keyForRun:run colorSpace:self.configuration.colorSpace];
        id<MTLTexture> texture = self->_textures[key];
        // BUG-1128: Check for zero size to prevent division by zero
        if (!texture || run.imageInfo.size.width <= 0 || run.imageInfo.size.height <= 0) {
            return; // Skip this run
        }
        const CGSize textureSize = CGSizeMake(texture.width, texture.height);
        NSSize chunkSize =
            NSMakeSize(textureSize.width / run.imageInfo.size.width, textureSize.height / run.imageInfo.size.height);
        const CGRect textureFrame =
            NSMakeRect((chunkSize.width * run.startingCoordInImage.x) / textureSize.width,
                       (textureSize.height - chunkSize.height * (run.startingCoordInImage.y + 1)) / textureSize.height,
                       (chunkSize.width * run.length) / textureSize.width, (chunkSize.height) / textureSize.height);

        // This is done to match the point-based calculation in the legacy renderer.
        const CGFloat spacing =
            round((self.cellConfiguration.cellSizeWithoutSpacing.height - cellSize.height) / (2.0 * scale)) * scale;
        const CGRect destinationFrame =
            CGRectMake(run.startingCoordOnScreen.x * cellSize.width + offset.x,
                       height - (run.startingCoordOnScreen.y + 1) * cellSize.height - offset.y - spacing,
                       run.length * cellSize.width, cellSize.height);

        id<MTLBuffer> vertexBuffer = [self->_cellRenderer newQuadWithFrame:destinationFrame
                                                              textureFrame:textureFrame
                                                               poolContext:self.poolContext];

        block(key, vertexBuffer, texture);
    }];
}

@end

// Number of frames a texture must be unused before it gets removed.
// This helps animated GIFs avoid texture churning by keeping textures around
// for a few frames in case they cycle back quickly.
static const NSUInteger kTextureRetentionFrames = 4;

@implementation iTermImageRenderer {
    iTermMetalCellRenderer *_cellRenderer;
    NSMutableDictionary<NSNumber *, id<MTLTexture>> *_textures;
    NSCountedSet<NSNumber *> *_counts;

    // Track how many frames each texture has been unused.
    // When count goes to 0, add to this dict with value 0.
    // Each frame, increment unused count. When it reaches kTextureRetentionFrames, remove texture.
    // If texture is used again, remove from this dict.
    NSMutableDictionary<NSNumber *, NSNumber *> *_unusedFrameCounts;

    NSMutableDictionary<NSString *, NSNumber *> *_colorSpaceIdentifiers;
    uint32_t _nextColorSpaceIdentifier;

    // Phase 1 optimization: Reuse these containers across frames to avoid per-frame allocations.
    // These are cleared at the start of each frame rather than reallocated.
    NSMutableSet<NSNumber *> *_usedThisFrame;
    NSMutableArray<NSNumber *> *_keysToRemove;
}

- (instancetype)initWithDevice:(id<MTLDevice>)device {
    self = [super init];
    if (self) {
        _cellRenderer = [[iTermMetalCellRenderer alloc] initWithDevice:device
                                                    vertexFunctionName:@"iTermImageVertexShader"
                                                  fragmentFunctionName:@"iTermImageFragmentShader"
                                                              blending:[iTermMetalBlending compositeSourceOver]
                                                        piuElementSize:0
                                                   transientStateClass:[iTermImageRendererTransientState class]];
        // Phase 1 optimization: Pre-size for typical image caching
        _textures = [[NSMutableDictionary alloc] initWithCapacity:32];
        _counts = [[NSCountedSet alloc] init];
        _unusedFrameCounts = [[NSMutableDictionary alloc] initWithCapacity:32];
        _colorSpaceIdentifiers = [[NSMutableDictionary alloc] initWithCapacity:8];
        _nextColorSpaceIdentifier = 1;
        self.activeColorSpaceIdentifier = 0;

        // Phase 1 optimization: Pre-allocate containers for frame rendering
        _usedThisFrame = [NSMutableSet setWithCapacity:16];
        _keysToRemove = [[NSMutableArray alloc] initWithCapacity:16];
    }
    return self;
}

- (BOOL)rendererDisabled {
    return NO;
}

- (iTermMetalFrameDataStat)createTransientStateStat {
    return iTermMetalFrameDataStatPqCreateImageTS;
}

- (nullable __kindof iTermMetalRendererTransientState *)
    createTransientStateForCellConfiguration:(iTermCellRenderConfiguration *)configuration
                               commandBuffer:(id<MTLCommandBuffer>)commandBuffer {
    __kindof iTermMetalCellRendererTransientState *_Nonnull transientState =
        [_cellRenderer createTransientStateForCellConfiguration:configuration commandBuffer:commandBuffer];
    [self initializeTransientState:transientState];
    return transientState;
}

- (void)initializeTransientState:(iTermImageRendererTransientState *)tState {
    tState.cellRenderer = _cellRenderer;
    tState.timestamp = [NSDate timeIntervalSinceReferenceDate];
    tState.textures = _textures;
    tState.counts = _counts;
    tState.renderer = self;
}

- (void)drawWithFrameData:(iTermMetalFrameData *)frameData
           transientState:(__kindof iTermMetalCellRendererTransientState *)transientState {
    iTermImageRendererTransientState *tState = transientState;

    // Phase 1 optimization: Reuse pre-allocated containers instead of allocating each frame.
    // Clear them at the start of each frame.
    [_usedThisFrame removeAllObjects];

    [tState enumerateDraws:^(id key, id<MTLBuffer> vertexBuffer, id<MTLTexture> texture) {
        // Texture is being used - remove from unused tracking
        [self->_unusedFrameCounts removeObjectForKey:key];
        [self->_usedThisFrame addObject:key];

        const iTermMetalBufferBinding vertexBindings[] = {
            iTermMetalBufferBindingMake(iTermVertexInputIndexVertices, vertexBuffer),
        };
        const iTermMetalTextureBinding textureBindings[] = {
            iTermMetalTextureBindingMake(iTermTextureIndexPrimary, texture),
        };
        [self->_cellRenderer drawWithTransientState:tState
                                      renderEncoder:frameData.renderEncoder
                                   numberOfVertices:6
                                       numberOfPIUs:0
                                     vertexBindings:vertexBindings
                                 vertexBindingCount:sizeof(vertexBindings) / sizeof(vertexBindings[0])
                                   fragmentBindings:NULL
                               fragmentBindingCount:0
                                    textureBindings:textureBindings
                                textureBindingCount:sizeof(textureBindings) / sizeof(textureBindings[0])];
        [self->_counts removeObject:key];
        if ([self->_counts countForObject:key] == 0) {
            // Reference count went to 0 - start tracking as unused
            self->_unusedFrameCounts[key] = @0;
        }
    }];

    // Update unused frame counts and remove textures that have been unused too long
    // Phase 1 optimization: Reuse pre-allocated array instead of allocating each frame
    [_keysToRemove removeAllObjects];
    for (NSNumber *key in [_unusedFrameCounts allKeys]) {
        if ([_usedThisFrame containsObject:key]) {
            // Was used this frame - already removed from tracking above
            continue;
        }
        NSUInteger unusedFrames = _unusedFrameCounts[key].unsignedIntegerValue + 1;
        if (unusedFrames >= kTextureRetentionFrames) {
            // Texture has been unused for too many frames - mark for removal
            [_keysToRemove addObject:key];
        } else {
            // Increment unused frame count
            _unusedFrameCounts[key] = @(unusedFrames);
        }
    }

    // Remove stale textures
    for (NSNumber *key in _keysToRemove) {
        [_unusedFrameCounts removeObjectForKey:key];
        [_textures removeObjectForKey:key];
    }
}

- (NSNumber *)textureKeyForCode:(int32_t)code frame:(uint16_t)frame colorSpaceName:(NSString *)colorSpaceName {
    const uint32_t identifier = [self identifierForColorSpaceName:colorSpaceName];
    const uint16_t sanitizedCode = (uint16_t)(code & 0xffff);
    return @(iTermImageRendererPackTextureKey(identifier, sanitizedCode, frame));
}

- (uint32_t)identifierForColorSpaceName:(NSString *)colorSpaceName {
    NSString *name = colorSpaceName ?: @"(null)";
    NSNumber *existing = _colorSpaceIdentifiers[name];
    if (existing) {
        return existing.unsignedIntValue;
    }
    // BUG-f554: Log error instead of crashing when color space budget is exceeded
    if (_nextColorSpaceIdentifier == UINT32_MAX) {
        DLog(@"ERROR: Exceeded color space identifier budget for image textures");
        return UINT32_MAX;
    }
    const uint32_t identifier = _nextColorSpaceIdentifier++;
    _colorSpaceIdentifiers[name] = @(identifier);
    return identifier;
}

@end
