#import "iTermMarkRenderer.h"

#import "DebugLogging.h"
#import "iTermAdvancedSettingsModel.h"
#import "iTermPreferences.h"
#import "iTermTextDrawingHelper.h"
#import "iTermTextureArray.h"
#import "iTermMetalCellRenderer.h"
#import "NSImage+iTerm.h"
#import "NSObject+iTerm.h"

// Optimization: Cache NSNumber objects for common row indices (0-255) to avoid boxing.
// Metal rendering iterates over visible rows, which are typically 0-100 for most terminals.
// Caching 256 values covers all typical use cases with zero allocations.
static const int kCachedRowKeyCount = 256;
static NSNumber *sRowKeyCache[kCachedRowKeyCount];

// Optimization: Cache NSNumber objects for mark style values (0-5).
// iTermMarkStyleNone (-1) triggers removal and is never stored.
// The 6 valid styles (RegularSuccess=0 through FoldedOther=5) are cached.
static const int kCachedMarkStyleCount = 6;
static NSNumber *sMarkStyleCache[kCachedMarkStyleCount];

NS_INLINE NSNumber *iTermMarkRowKeyToNumber(int row) {
    if (row >= 0 && row < kCachedRowKeyCount) {
        return sRowKeyCache[row];
    }
    return @(row);
}

NS_INLINE NSNumber *iTermMarkStyleToNumber(iTermMarkStyle style) {
    if (style >= 0 && style < kCachedMarkStyleCount) {
        return sMarkStyleCache[style];
    }
    return @(style);
}

__attribute__((constructor))
static void iTermMarkRendererInitializeCaches(void) {
    for (int i = 0; i < kCachedRowKeyCount; i++) {
        sRowKeyCache[i] = @(i);
    }
    for (int i = 0; i < kCachedMarkStyleCount; i++) {
        sMarkStyleCache[i] = @(i);
    }
}

@interface iTermMarkRendererTransientState()
@property (nonatomic, strong) iTermTextureArray *marksArrayTexture;
@property (nonatomic) CGSize markSize;
@property (nonatomic) CGPoint markOffset;
@property (nonatomic, copy) NSDictionary<NSNumber *, NSNumber *> *marks;
- (size_t)piuSize;
- (void)fillPIUBuffer:(iTermMarkPIU *)pius;
@end

@implementation iTermMarkRendererTransientState {
    NSMutableDictionary<NSNumber *, NSNumber *> *_marks;
}

- (void)writeDebugInfoToFolder:(NSURL *)folder {
    [super writeDebugInfoToFolder:folder];
    [[NSString stringWithFormat:@"marks=%@", _marks] writeToURL:[folder URLByAppendingPathComponent:@"state.txt"]
                                                     atomically:NO
                                                       encoding:NSUTF8StringEncoding
                                                          error:NULL];
}

- (size_t)piuSize {
    return sizeof(iTermMarkPIU) * _marks.count;
}

// Fill PIU buffer directly - avoids intermediate NSMutableData allocation
- (void)fillPIUBuffer:(iTermMarkPIU *)pius {
    __block size_t i = 0;
    [_marks enumerateKeysAndObjectsUsingBlock:^(NSNumber * _Nonnull rowNumber, NSNumber * _Nonnull styleNumber, BOOL * _Nonnull stop) {
        MTLOrigin origin = [self->_marksArrayTexture offsetForIndex:styleNumber.integerValue];
        pius[i] = (iTermMarkPIU) {
            .offset = {
                0,
                (self.cellConfiguration.gridSize.height - rowNumber.intValue - 1) * self.cellConfiguration.cellSize.height + self.margins.top + self.cellConfiguration.cellSize.height - self.markSize.height - self.markOffset.y
            },
            .textureOffset = { origin.x, origin.y }
        };
        i++;
    }];
}

- (void)setMarkStyle:(iTermMarkStyle)markStyle row:(int)row {
    if (!_marks) {
        // Marks dictionary typically holds 1-10 entries per visible screen
        _marks = [NSMutableDictionary dictionaryWithCapacity:8];
    }
    NSNumber *rowKey = iTermMarkRowKeyToNumber(row);
    if (markStyle == iTermMarkStyleNone) {
        [_marks removeObjectForKey:rowKey];
    } else {
        _marks[rowKey] = iTermMarkStyleToNumber(markStyle);
    }
}

@end

@implementation iTermMarkRenderer {
    iTermMetalCellRenderer *_cellRenderer;
    iTermTextureArray *_marksArrayTexture;
    NSColorSpace *_colorSpace;
    CGSize _markSize;
    iTermMetalMixedSizeBufferPool *_piuPool;
}

- (instancetype)initWithDevice:(id<MTLDevice>)device {
    self = [super init];
    if (self) {
        _cellRenderer = [[iTermMetalCellRenderer alloc] initWithDevice:device
                                                    vertexFunctionName:@"iTermMarkVertexShader"
                                                  fragmentFunctionName:@"iTermMarkFragmentShader"
                                                              blending:[iTermMetalBlending compositeSourceOver]
                                                        piuElementSize:sizeof(iTermMarkPIU)
                                                   transientStateClass:[iTermMarkRendererTransientState class]];
        _piuPool = [[iTermMetalMixedSizeBufferPool alloc] initWithDevice:device
                                                                capacity:iTermMetalDriverMaximumNumberOfFramesInFlight + 1
                                                                    name:@"mark PIU"];
    }
    return self;
}

- (BOOL)rendererDisabled {
    return NO;
}

- (iTermMetalFrameDataStat)createTransientStateStat {
    return iTermMetalFrameDataStatPqCreateMarkTS;
}

- (nullable  __kindof iTermMetalRendererTransientState *)createTransientStateForCellConfiguration:(iTermCellRenderConfiguration *)configuration
                                                                                    commandBuffer:(id<MTLCommandBuffer>)commandBuffer {
    __kindof iTermMetalCellRendererTransientState * _Nonnull transientState =
        [_cellRenderer createTransientStateForCellConfiguration:configuration
                                              commandBuffer:commandBuffer];
    [self initializeTransientState:transientState];
    return transientState;
}

- (void)initializeTransientState:(iTermMarkRendererTransientState *)tState {
    DLog(@"Initialize transient state");
    const CGFloat scale = tState.configuration.scale;

    DLog(@"Side margin size is %@, scale is %@, cell size is %@, cell size without spacing is %@",
         @([iTermPreferences intForKey:kPreferenceKeySideMargins]),
         @(scale),
         NSStringFromSize(tState.cellConfiguration.cellSize),
         NSStringFromSize(tState.cellConfiguration.cellSizeWithoutSpacing));
    CGRect leftMarginRect = CGRectMake(1,
                                       0,
                                       ([iTermPreferences intForKey:kPreferenceKeySideMargins] - 1) * scale,
                                       tState.cellConfiguration.cellSize.height);
    DLog(@"leftMarginRect=%@", NSStringFromRect(leftMarginRect));
    CGRect markRect = [iTermTextDrawingHelper frameForMarkContainedInRect:leftMarginRect
                                                                 cellSize:tState.cellConfiguration.cellSize
                                                   cellSizeWithoutSpacing:tState.cellConfiguration.cellSizeWithoutSpacing
                                                                    scale:scale];
    DLog(@"markRect=%@, _markSize=%@", NSStringFromRect(markRect), NSStringFromSize(_markSize));

    if (!CGSizeEqualToSize(markRect.size, _markSize) || ![NSObject object:tState.configuration.colorSpace isEqualToObject:_colorSpace]) {
        DLog(@"Mark size or colorspace has changed");
        _markSize = markRect.size;
        _colorSpace = tState.configuration.colorSpace;
        if (_markSize.width > 0 && _markSize.height > 0) {
            DLog(@"Size is positive, make images of size %@", NSStringFromSize(_markSize));
            NSColor *successColor = [iTermTextDrawingHelper successMarkColor];
            NSColor *otherColor = [iTermTextDrawingHelper otherMarkColor];
            NSColor *failureColor = [iTermTextDrawingHelper errorMarkColor];

            NSImage *regularSuccessImage = [self newImageWithMarkOfColor:successColor size:_markSize folded:NO];
            NSImage *regularFailureImage = [self newImageWithMarkOfColor:failureColor size:_markSize folded:NO];
            NSImage *regularOtherImage = [self newImageWithMarkOfColor:otherColor size:_markSize folded:NO];

            NSImage *foldedSuccessImage = [self newImageWithMarkOfColor:successColor size:_markSize folded:YES];
            NSImage *foldedFailureImage = [self newImageWithMarkOfColor:failureColor size:_markSize folded:YES];
            NSImage *foldedOtherImage = [self newImageWithMarkOfColor:otherColor size:_markSize folded:YES];
            _marksArrayTexture = [[iTermTextureArray alloc] initWithImages:@[regularSuccessImage,
                                                                             regularFailureImage,
                                                                             regularOtherImage,
                                                                             foldedSuccessImage,
                                                                             foldedFailureImage,
                                                                             foldedOtherImage]
                                                                    device:_cellRenderer.device];
        }
    }

    tState.markOffset = markRect.origin;
    tState.marksArrayTexture = _marksArrayTexture;
    tState.markSize = _markSize;
    tState.vertexBuffer = [_cellRenderer newQuadOfSize:_markSize poolContext:tState.poolContext];
}

- (void)drawWithFrameData:(iTermMetalFrameData *)frameData
           transientState:(__kindof iTermMetalCellRendererTransientState *)transientState {
    iTermMarkRendererTransientState *tState = transientState;
    if (tState.marks.count == 0) {
        return;
    }
    if (tState.marksArrayTexture == nil) {
        return;
    }

    const CGFloat scale = tState.configuration.scale;
    const CGRect quad = CGRectMake(round(tState.markOffset.x / scale) * scale,
                                   0,
                                   tState.markSize.width,
                                   tState.markSize.height);
    const CGRect textureFrame = CGRectMake(0,
                                           0,
                                           tState.markSize.width,
                                           tState.markSize.height);
    DLog(@"Using texture frame of %@", NSStringFromRect(textureFrame));
    const iTermVertex vertices[] = {
        // Pixel Positions                              Texture Coordinates
        { { CGRectGetMaxX(quad), CGRectGetMinY(quad) }, { CGRectGetMaxX(textureFrame), CGRectGetMaxY(textureFrame) } },
        { { CGRectGetMinX(quad), CGRectGetMinY(quad) }, { CGRectGetMinX(textureFrame), CGRectGetMaxY(textureFrame) } },
        { { CGRectGetMinX(quad), CGRectGetMaxY(quad) }, { CGRectGetMinX(textureFrame), CGRectGetMinY(textureFrame) } },

        { { CGRectGetMaxX(quad), CGRectGetMinY(quad) }, { CGRectGetMaxX(textureFrame), CGRectGetMaxY(textureFrame) } },
        { { CGRectGetMinX(quad), CGRectGetMaxY(quad) }, { CGRectGetMinX(textureFrame), CGRectGetMinY(textureFrame) } },
        { { CGRectGetMaxX(quad), CGRectGetMaxY(quad) }, { CGRectGetMaxX(textureFrame), CGRectGetMinY(textureFrame) } },
    };
    tState.vertexBuffer = [_cellRenderer.verticesPool requestBufferFromContext:tState.poolContext
                                                                      withBytes:vertices
                                                                 checkIfChanged:YES];

    // Create PIU buffer and fill directly (avoids intermediate NSMutableData allocation)
    const size_t piuSize = [tState piuSize];
    tState.pius = [_piuPool requestBufferFromContext:tState.poolContext
                                                size:piuSize];
    [tState fillPIUBuffer:(iTermMarkPIU *)tState.pius.contents];

    const iTermMetalBufferBinding vertexBindings[] = {
        iTermMetalBufferBindingMake(iTermVertexInputIndexVertices, tState.vertexBuffer),
        iTermMetalBufferBindingMake(iTermVertexInputIndexPerInstanceUniforms, tState.pius),
        iTermMetalBufferBindingMake(iTermVertexInputIndexOffset, tState.offsetBuffer),
    };
    const iTermMetalTextureBinding textureBindings[] = {
        iTermMetalTextureBindingMake(iTermTextureIndexPrimary, tState.marksArrayTexture.texture),
    };
    [_cellRenderer drawWithTransientState:tState
                            renderEncoder:frameData.renderEncoder
                         numberOfVertices:6
                             numberOfPIUs:tState.marks.count
                           vertexBindings:vertexBindings
                      vertexBindingCount:sizeof(vertexBindings) / sizeof(vertexBindings[0])
                          fragmentBindings:NULL
                     fragmentBindingCount:0
                              textureBindings:textureBindings
                         textureBindingCount:sizeof(textureBindings) / sizeof(textureBindings[0])];
}

#pragma mark - Private

- (NSImage *)newImageWithMarkOfColor:(NSColor *)color size:(CGSize)pixelSize folded:(BOOL)folded {
    return [iTermTextDrawingHelper newImageWithMarkOfColor:color
                                                 pixelSize:pixelSize
                                                    folded:folded];
}

@end
