//
//  iTermTimestampsRenderer.m
//  DashTerm2
//
//  Created by George Nachman on 12/31/17.
//

#import "iTermTimestampsRenderer.h"

#import "DebugLogging.h"
#import "FutureMethods.h"
#import "NSImage+iTerm.h"
#import "iTermGraphicsUtilities.h"
#import "iTermSharedImageStore.h"
#import "iTermTexturePool.h"
#import "iTermTimestampDrawHelper.h"
#import "PTYFontInfo.h"

@interface iTermTimestampKey : NSObject<NSCopying>
@property (nonatomic) CGFloat width;
@property (nonatomic) vector_float4 textColor;
@property (nonatomic) vector_float4 backgroundColor;
@property (nonatomic) NSString *string;
@end

@implementation iTermTimestampKey

- (NSUInteger)hash {
    return [_string hash];
}

- (BOOL)isEqual:(id)other {
    if (![other isKindOfClass:[iTermTimestampKey class]]) {
        return NO;
    }
    iTermTimestampKey *otherKey = other;
    return (_width == otherKey->_width &&
            _textColor.x == otherKey->_textColor.x &&
            _textColor.y == otherKey->_textColor.y &&
            _textColor.z == otherKey->_textColor.z &&
            _backgroundColor.x == otherKey->_backgroundColor.x &&
            _backgroundColor.y == otherKey->_backgroundColor.y &&
            _backgroundColor.z == otherKey->_backgroundColor.z &&
            (_string == otherKey->_string || [_string isEqual:otherKey->_string]));
}

- (id)copyWithZone:(NSZone *)zone {
    iTermTimestampKey *copy = [[iTermTimestampKey allocWithZone:zone] init];
    copy.width = _width;
    copy.textColor = _textColor;
    copy.backgroundColor = _backgroundColor;
    copy.string = [_string copy];
    return copy;
}

@end

// Structure to hold per-row rendering info
typedef struct {
    int row;
    CGFloat yPosition;
    CGFloat xOverflow;
} iTermTimestampRowInfo;

@interface iTermTimestampsRendererTransientState()
- (void)enumerateRowsGroupedByKey:(void (^)(iTermTimestampKey *key,
                                            const iTermTimestampRowInfo *rowInfos,
                                            NSUInteger rowCount,
                                            NSRect baseFrame))block;
- (NSImage *)imageForRow:(int)row;
- (void)addPooledTexture:(iTermPooledTexture *)pooledTexture;
@end

@implementation iTermTimestampsRendererTransientState {
    iTermTimestampDrawHelper *_drawHelper;
    NSMutableArray<iTermPooledTexture *> *_pooledTextures;
}

- (void)writeDebugInfoToFolder:(NSURL *)folder {
    [super writeDebugInfoToFolder:folder];
    NSMutableString *s = [NSMutableString stringWithFormat:@"backgroundColor=%@\ntextColor=%@\n",
                          _backgroundColor, _textColor];
    [_timestamps enumerateObjectsUsingBlock:^(NSDate * _Nonnull obj, NSUInteger idx, BOOL * _Nonnull stop) {
        [s appendFormat:@"%@\n", obj];
    }];
    [s writeToURL:[folder URLByAppendingPathComponent:@"state.txt"]
       atomically:NO
         encoding:NSUTF8StringEncoding
            error:NULL];
}

- (void)addPooledTexture:(iTermPooledTexture *)pooledTexture {
    if (!_pooledTextures) {
        // Timestamps typically render 1-4 textures per frame for grouped rows
        _pooledTextures = [NSMutableArray arrayWithCapacity:4];
    }
    [_pooledTextures addObject:pooledTexture];
}

- (void)_initializeDrawHelperIfNeeded {
    if (_drawHelper) return;

    const CGFloat scale = self.cellConfiguration.scale;
    const CGFloat rowHeight = self.cellConfiguration.cellSize.height / scale;
    const CGFloat rowHeightWithoutSpacing = self.cellConfiguration.cellSizeWithoutSpacing.height / scale;

    _drawHelper = [[iTermTimestampDrawHelper alloc] initWithBackgroundColor:_backgroundColor
                                                                  textColor:_textColor
                                                                        now:[NSDate timeIntervalSinceReferenceDate]
                                                         useTestingTimezone:NO
                                                                  rowHeight:rowHeight
                                                  rowHeightWithoutSpacing:rowHeightWithoutSpacing
                                                                     retina:self.configuration.scale > 1
                                                                   fontInfo:self.fontInfo
                                                                   obscured:self.obscured];
    _drawHelper.timestampBaseline = self.timestampBaseline;
    [_timestamps enumerateObjectsUsingBlock:^(NSDate * _Nonnull date, NSUInteger idx, BOOL * _Nonnull stop) {
        [self->_drawHelper setDate:date forLine:idx];
    }];
}

// Groups rows by their timestamp key for batched rendering
// block receives: the key, C array of iTermTimestampRowInfo, count, and base frame
// Performance optimization: Uses C arrays instead of NSValue boxing to avoid
// ~50-200μs overhead per frame from Obj-C object allocation/deallocation.
- (void)enumerateRowsGroupedByKey:(void (^)(iTermTimestampKey *key,
                                            const iTermTimestampRowInfo *rowInfos,
                                            NSUInteger rowCount,
                                            NSRect baseFrame))block {
    // BUG-f1383: Convert assert to guard - return early if timestamps is nil
    if (!_timestamps) {
        DLog(@"WARNING BUG-f1383: enumerateRowsGroupedByKey called with nil _timestamps");
        return;
    }

    [self _initializeDrawHelperIfNeeded];

    const CGFloat scale = self.cellConfiguration.scale;
    const CGFloat rowHeight = self.cellConfiguration.cellSize.height / scale;
    const CGFloat visibleWidth = _drawHelper.suggestedWidth;
    const vector_float4 textColor = simd_make_float4(_textColor.redComponent,
                                                     _textColor.greenComponent,
                                                     _textColor.blueComponent,
                                                     _textColor.alphaComponent);
    const vector_float4 backgroundColor = simd_make_float4(_backgroundColor.redComponent,
                                                           _backgroundColor.greenComponent,
                                                           _backgroundColor.blueComponent,
                                                           _backgroundColor.alphaComponent);
    const CGFloat vmargin = self.margins.bottom / scale;
    const CGFloat gridWidth = self.cellConfiguration.gridSize.width * self.cellConfiguration.cellSize.width;
    const NSEdgeInsets margins = self.margins;
    const CGFloat rightGutterWidth = self.configuration.viewportSize.x - margins.left - margins.right - gridWidth - self.configuration.rightExtraPixels;

    // Group rows by key - use NSValue-wrapped C arrays for the values
    // to avoid per-row NSValue boxing in the inner loop
    // Pre-allocate with expected capacity based on timestamp count (typically 1-5 unique keys)
    const NSUInteger estimatedGroups = MIN(_timestamps.count, 8);
    NSMutableDictionary<iTermTimestampKey *, NSMutableData *> *groups = [NSMutableDictionary dictionaryWithCapacity:estimatedGroups];

    // Pre-allocate lookup key to avoid allocation in the loop
    iTermTimestampKey *lookupKey = [[iTermTimestampKey alloc] init];
    lookupKey.width = visibleWidth;
    lookupKey.textColor = textColor;
    lookupKey.backgroundColor = backgroundColor;

    const NSUInteger count = _timestamps.count;
    for (NSUInteger idx = 0; idx < count; idx++) {
        // Update lookup key string (reusing the key object avoids per-iteration alloc)
        lookupKey.string = [_drawHelper rowIsRepeat:idx] ? @"(repeat)" : [_drawHelper stringForRow:idx];

        CGFloat yPosition = self.configuration.viewportSize.y / scale - ((idx + 1) * rowHeight) - vmargin;

        iTermTimestampRowInfo info = {
            .row = (int)idx,
            .yPosition = yPosition,
            .xOverflow = 0  // Will be calculated during draw
        };

        NSMutableData *rowData = groups[lookupKey];
        if (!rowData) {
            // First row with this key - create data buffer and copy key
            rowData = [[NSMutableData alloc] initWithCapacity:sizeof(iTermTimestampRowInfo) * 4];
            groups[[lookupKey copy]] = rowData;
        }
        [rowData appendBytes:&info length:sizeof(iTermTimestampRowInfo)];
    }

    // Calculate base frame (at y=0, will be offset by PIU)
    CGFloat baseX = (self.configuration.viewportSize.x - rightGutterWidth) / scale - visibleWidth;
    NSRect baseFrame = NSMakeRect(baseX, 0, visibleWidth, rowHeight);

    // Call block for each group
    [groups enumerateKeysAndObjectsUsingBlock:^(iTermTimestampKey *key,
                                                NSMutableData *rowData,
                                                BOOL *stop) {
        const iTermTimestampRowInfo *rowInfos = (const iTermTimestampRowInfo *)rowData.bytes;
        NSUInteger rowCount = rowData.length / sizeof(iTermTimestampRowInfo);
        block(key, rowInfos, rowCount, baseFrame);
    }];
}

// Legacy single-row enumeration for compatibility
- (void)enumerateRows:(void (^)(int row, iTermTimestampKey *key, NSRect frame))block {
    // BUG-f1383: Convert assert to guard - return early if timestamps is nil
    if (!_timestamps) {
        DLog(@"WARNING BUG-f1383: enumerateRows called with nil _timestamps");
        return;
    }

    [self _initializeDrawHelperIfNeeded];

    const CGFloat scale = self.cellConfiguration.scale;
    const CGFloat rowHeight = self.cellConfiguration.cellSize.height / scale;
    const CGFloat visibleWidth = _drawHelper.suggestedWidth;
    const vector_float4 textColor = simd_make_float4(_textColor.redComponent,
                                                     _textColor.greenComponent,
                                                     _textColor.blueComponent,
                                                     _textColor.alphaComponent);
    const vector_float4 backgroundColor = simd_make_float4(_backgroundColor.redComponent,
                                                           _backgroundColor.greenComponent,
                                                           _backgroundColor.blueComponent,
                                                           _backgroundColor.alphaComponent);
    const CGFloat vmargin = self.margins.bottom / scale;

    const CGFloat gridWidth = self.cellConfiguration.gridSize.width * self.cellConfiguration.cellSize.width;
    const NSEdgeInsets margins = self.margins;
    const CGFloat rightGutterWidth = self.configuration.viewportSize.x - margins.left - margins.right - gridWidth - self.configuration.rightExtraPixels;

    // Phase 1 optimization: Pre-allocate key outside loop and reuse it.
    // The block receives the key which it can copy if needed for retention.
    iTermTimestampKey *reusableKey = [[iTermTimestampKey alloc] init];
    reusableKey.width = visibleWidth;
    reusableKey.textColor = textColor;
    reusableKey.backgroundColor = backgroundColor;

    const NSUInteger count = _timestamps.count;
    for (NSUInteger idx = 0; idx < count; idx++) {
        // Update only the string field each iteration (width/colors are constant)
        reusableKey.string = [_drawHelper rowIsRepeat:idx] ? @"(repeat)" : [_drawHelper stringForRow:idx];
        block((int)idx,
              reusableKey,
              NSMakeRect((self.configuration.viewportSize.x - rightGutterWidth) / scale - visibleWidth,
                         self.configuration.viewportSize.y / scale - ((idx + 1) * rowHeight) - vmargin,
                         visibleWidth,
                         rowHeight));
    }
}

- (NSImage *)imageForRow:(int)row {
    [self _initializeDrawHelperIfNeeded];

    NSSize size = NSMakeSize(_drawHelper.suggestedWidth,
                             self.cellConfiguration.cellSize.height / self.cellConfiguration.scale);
    // BUG-f1384: Convert assert to guard - return nil for zero-size image
    if (size.width * size.height <= 0) {
        DLog(@"WARNING BUG-f1384: imageForRow called with invalid size (%.2f x %.2f)", size.width, size.height);
        return nil;
    }
    NSImage *image = [[NSImage flippedImageOfSize:size drawBlock:^{
        NSGraphicsContext *context = [NSGraphicsContext currentContext];
        iTermSetSmoothing(context.CGContext,
                          NULL,
                          self.useThinStrokes,
                          self.antialiased);
        [self->_drawHelper drawRow:row
                   inContext:[NSGraphicsContext currentContext]
                       frame:NSMakeRect(0, 0, size.width, size.height)
               virtualOffset:0];
    }] it_verticallyFlippedImage];

    return image;
}

@end

@implementation iTermTimestampsRenderer {
    iTermMetalCellRenderer *_cellRenderer;

    // Configuration - if any change invalidate the cache
    NSColorSpace *_colorSpace;
    PTYFontInfo *_fontInfo;
    NSSize _cellSize;
    NSColor *_backgroundColor;
    NSColor *_textColor;
    CGFloat _scale;
    CGFloat _obscured;

    NSCache<iTermTimestampKey *, iTermPooledTexture *> *_cache;
    iTermTexturePool *_texturePool;
    iTermMetalMixedSizeBufferPool *_piuPool;
}

- (instancetype)initWithDevice:(id<MTLDevice>)device {
    self = [super init];
    if (self) {
        _texturePool = [[iTermTexturePool alloc] init];
        iTermMetalBlending *blending = [[iTermMetalBlending alloc] init];
#if ENABLE_TRANSPARENT_METAL_WINDOWS
        if (iTermTextIsMonochrome()) {
            blending = [iTermMetalBlending atop];  // IS THIS RIGHT EVERYWEHRE?
        }
#endif
        _cellRenderer = [[iTermMetalCellRenderer alloc] initWithDevice:device
                                                    vertexFunctionName:@"iTermTimestampsVertexShader"
                                                  fragmentFunctionName:@"iTermTimestampsFragmentShader"
                                                              blending:blending
                                                        piuElementSize:sizeof(iTermTimestampPIU)
                                                   transientStateClass:[iTermTimestampsRendererTransientState class]];
        _cache = [[NSCache alloc] init];
        _piuPool = [[iTermMetalMixedSizeBufferPool alloc] initWithDevice:device
                                                                capacity:iTermMetalDriverMaximumNumberOfFramesInFlight + 1
                                                                    name:@"timestamp PIU"];
    }
    return self;
}

- (BOOL)rendererDisabled {
    return NO;
}

- (iTermMetalFrameDataStat)createTransientStateStat {
    return iTermMetalFrameDataStatPqCreateTimestampsTS;
}

- (nullable __kindof iTermMetalRendererTransientState *)createTransientStateForCellConfiguration:(iTermCellRenderConfiguration *)configuration
                                                                                   commandBuffer:(id<MTLCommandBuffer>)commandBuffer {
    if (!_enabled) {
        return nil;
    }
    __kindof iTermMetalCellRendererTransientState * _Nonnull transientState =
        [_cellRenderer createTransientStateForCellConfiguration:configuration
                                                  commandBuffer:commandBuffer];
    [self initializeTransientState:transientState];
    return transientState;
}

- (void)initializeTransientState:(iTermTimestampsRendererTransientState *)tState {
}

- (BOOL)configurationChanged:(iTermTimestampsRendererTransientState *)tState {
    if (![NSObject object:tState.configuration.colorSpace isEqualToObject:_colorSpace]) {
        return YES;
    }
    if (![tState.fontInfo.font isEqualTo:_fontInfo.font]) {
        return YES;
    }
    if (!NSEqualSizes(tState.cellConfiguration.cellSize, _cellSize)) {
        return YES;
    }
    if (![tState.backgroundColor isEqual:_backgroundColor]) {
        return YES;
    }
    if (![tState.textColor isEqual:_textColor]) {
        return YES;
    }
    if (tState.cellConfiguration.scale != _scale) {
        return YES;
    }
    if (tState.obscured != _obscured) {
        return YES;
    }
    return NO;
}

- (void)drawWithFrameData:(iTermMetalFrameData *)frameData
           transientState:(__kindof iTermMetalCellRendererTransientState *)transientState {
    iTermTimestampsRendererTransientState *tState = transientState;
    _cache.countLimit = tState.cellConfiguration.gridSize.height * 4;
    const CGFloat scale = tState.configuration.scale;
    if ([self configurationChanged:tState]) {
        [_cache removeAllObjects];
        _colorSpace = tState.configuration.colorSpace;
        _fontInfo = tState.fontInfo;
        _cellSize = tState.cellConfiguration.cellSize;
        _backgroundColor = tState.backgroundColor;
        _textColor = tState.textColor;
        _scale = tState.cellConfiguration.scale;
        _obscured = tState.obscured;
    }

    // Use batched rendering: group rows by key and issue one draw call per unique texture
    // Performance optimization: Uses C arrays instead of NSArray<NSValue *> to avoid
    // boxing overhead in the render loop.
    [tState enumerateRowsGroupedByKey:^(iTermTimestampKey *key,
                                        const iTermTimestampRowInfo *rowInfos,
                                        NSUInteger rowCount,
                                        NSRect baseFrame) {
        // Get or create texture for this key
        iTermPooledTexture *pooledTexture = [self->_cache objectForKey:key];
        if (!pooledTexture) {
            // Need to render the timestamp - use first row in group
            if (rowCount == 0) {
                return;
            }
            const iTermTimestampRowInfo firstInfo = rowInfos[0];

            NSImage *image = [tState imageForRow:firstInfo.row];
            if (!image) {
                return;
            }
            iTermMetalBufferPoolContext *context = tState.poolContext;
            id<MTLTexture> texture = [self->_cellRenderer textureFromImage:[iTermImageWrapper withImage:image]
                                                                   context:context
                                                                      pool:self->_texturePool
                                                                colorSpace:tState.configuration.colorSpace];
            // BUG-1111: Replace assert with proper nil check that works in release builds
            if (!texture) {
                return; // Skip this group if texture creation failed
            }
            pooledTexture = [[iTermPooledTexture alloc] initWithTexture:texture
                                                                   pool:self->_texturePool];
            [self->_cache setObject:pooledTexture forKey:key];
        }
        [tState addPooledTexture:pooledTexture];

        // Calculate overflow for texture positioning
        CGFloat overflow;
        const CGFloat slop = iTermTimestampGradientWidth * scale;
        if (pooledTexture.texture.width < tState.configuration.viewportSize.x + slop) {
            overflow = 0;
        } else {
            overflow = pooledTexture.texture.width - tState.configuration.viewportSize.x - slop;
        }

        // Create base quad at y=0 (PIU will provide y offset per instance)
        CGRect quad = CGRectMake(baseFrame.origin.x * scale + overflow,
                                 0,  // Base at y=0
                                 baseFrame.size.width * scale,
                                 baseFrame.size.height * scale);
        const float minX = (float)CGRectGetMinX(quad);
        const float maxX = (float)CGRectGetMaxX(quad);
        const float minY = (float)CGRectGetMinY(quad);
        const float maxY = (float)CGRectGetMaxY(quad);
        const iTermVertex vertices[] = {
            // Pixel Positions (triangle 1)
            { { maxX, minY }, { 1, 1 } },
            { { minX, minY }, { 0, 1 } },
            { { minX, maxY }, { 0, 0 } },
            // Pixel Positions (triangle 2)
            { { maxX, minY }, { 1, 1 } },
            { { minX, maxY }, { 0, 0 } },
            { { maxX, maxY }, { 1, 0 } },
        };

        id<MTLBuffer> vertexBuffer = [self->_cellRenderer.verticesPool requestBufferFromContext:tState.poolContext
                                                                                      withBytes:vertices
                                                                                 checkIfChanged:YES];

        // Build PIU array for all rows in this group - direct C array access
        const size_t piuSize = sizeof(iTermTimestampPIU) * rowCount;
        id<MTLBuffer> piuBuffer = [self->_piuPool requestBufferFromContext:tState.poolContext
                                                                      size:piuSize];
        iTermTimestampPIU *pius = (iTermTimestampPIU *)piuBuffer.contents;

        for (NSUInteger i = 0; i < rowCount; i++) {
            pius[i] = (iTermTimestampPIU) {
                .yOffset = (float)(rowInfos[i].yPosition * scale),
                .xOffset = 0  // X is same for all instances in this group
            };
        }

        const iTermMetalBufferBinding vertexBindings[] = {
            iTermMetalBufferBindingMake(iTermVertexInputIndexVertices, vertexBuffer),
            iTermMetalBufferBindingMake(iTermVertexInputIndexPerInstanceUniforms, piuBuffer),
        };
        const iTermMetalTextureBinding textureBindings[] = {
            iTermMetalTextureBindingMake(iTermTextureIndexPrimary, pooledTexture.texture),
        };

        // Single draw call for all rows using this texture
        [self->_cellRenderer drawWithTransientState:tState
                                      renderEncoder:frameData.renderEncoder
                                   numberOfVertices:6
                                       numberOfPIUs:rowCount
                                     vertexBindings:vertexBindings
                                vertexBindingCount:sizeof(vertexBindings) / sizeof(vertexBindings[0])
                                   fragmentBindings:NULL
                              fragmentBindingCount:0
                                   textureBindings:textureBindings
                              textureBindingCount:sizeof(textureBindings) / sizeof(textureBindings[0])];
    }];
}

@end
