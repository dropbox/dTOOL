#import "iTermCursorGuideRenderer.h"
#import "iTermSharedImageStore.h"
#import "NSObject+iTerm.h"

@interface iTermCursorGuideRendererTransientState()
@property (nonatomic, strong) id<MTLTexture> texture;
@property (nonatomic) int row;
@end

@implementation iTermCursorGuideRendererTransientState {
    int _row;
}

- (void)setRow:(int)row {
    _row = row;
}

- (void)initializeVerticesWithPool:(iTermMetalBufferPool *)verticesPool {
    CGSize cellSize = self.cellConfiguration.cellSize;
    VT100GridSize gridSize = self.cellConfiguration.gridSize;

    const CGRect quad = CGRectMake(0,
                                   self.margins.top + (gridSize.height - self.row - 1) * cellSize.height,
                                   self.configuration.viewportSize.x,
                                   cellSize.height);
    const CGRect textureFrame = CGRectMake(0, 0, 1, 1);
    const iTermVertex vertices[] = {
        // Pixel Positions                              Texture Coordinates
        { { CGRectGetMaxX(quad), CGRectGetMinY(quad) }, { CGRectGetMaxX(textureFrame), CGRectGetMinY(textureFrame) } },
        { { CGRectGetMinX(quad), CGRectGetMinY(quad) }, { CGRectGetMinX(textureFrame), CGRectGetMinY(textureFrame) } },
        { { CGRectGetMinX(quad), CGRectGetMaxY(quad) }, { CGRectGetMinX(textureFrame), CGRectGetMaxY(textureFrame) } },

        { { CGRectGetMaxX(quad), CGRectGetMinY(quad) }, { CGRectGetMaxX(textureFrame), CGRectGetMinY(textureFrame) } },
        { { CGRectGetMinX(quad), CGRectGetMaxY(quad) }, { CGRectGetMinX(textureFrame), CGRectGetMaxY(textureFrame) } },
        { { CGRectGetMaxX(quad), CGRectGetMaxY(quad) }, { CGRectGetMaxX(textureFrame), CGRectGetMaxY(textureFrame) } },
    };
    self.vertexBuffer = [verticesPool requestBufferFromContext:self.poolContext
                                                     withBytes:vertices
                                                checkIfChanged:YES];
}

- (void)writeDebugInfoToFolder:(NSURL *)folder {
    [super writeDebugInfoToFolder:folder];
    [[NSString stringWithFormat:@"row=%@", @(_row)] writeToURL:[folder URLByAppendingPathComponent:@"state.txt"]
                                                    atomically:NO
                                                      encoding:NSUTF8StringEncoding
                                                         error:NULL];
}

@end

@implementation iTermCursorGuideRenderer {
    iTermMetalCellRenderer *_cellRenderer;
    id<MTLTexture> _texture;
    NSColor *_color;
    CGSize _lastCellSize;
    NSColorSpace *_colorSpace;
}

- (instancetype)initWithDevice:(id<MTLDevice>)device {
    self = [super init];
    if (self) {
        _color = [[NSColor blueColor] colorWithAlphaComponent:0.7];
        _cellRenderer = [[iTermMetalCellRenderer alloc] initWithDevice:device
                                                    vertexFunctionName:@"iTermCursorGuideVertexShader"
                                                  fragmentFunctionName:@"iTermCursorGuideFragmentShader"
                                                              blending:[iTermMetalBlending compositeSourceOver]
                                                        piuElementSize:0
                                                   transientStateClass:[iTermCursorGuideRendererTransientState class]];
    }
    return self;
}

- (BOOL)rendererDisabled {
    return NO;
}

- (iTermMetalFrameDataStat)createTransientStateStat {
    return iTermMetalFrameDataStatPqCreateCursorGuideTS;
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

- (void)initializeTransientState:(iTermCursorGuideRendererTransientState *)tState {
    if (!CGSizeEqualToSize(tState.cellConfiguration.cellSize, _lastCellSize) ||
        ![NSObject object:tState.configuration.colorSpace isEqualToObject:_colorSpace]) {
        _texture = [self newCursorGuideTextureWithTransientState:tState];
        _lastCellSize = tState.cellConfiguration.cellSize;
        _colorSpace = tState.configuration.colorSpace;
    }
    tState.texture = _texture;
}

- (void)setColor:(NSColor *)color {
    if (color == _color || [color isEqual:_color]) {
        return;
    }
    _color = color;

    // Invalidate cell size so the texture gets created again
    _lastCellSize = CGSizeZero;
}

- (void)drawWithFrameData:(iTermMetalFrameData *)frameData
           transientState:(__kindof iTermMetalCellRendererTransientState *)transientState {
    iTermCursorGuideRendererTransientState *tState = transientState;
    if (tState.row < 0) {
        // Cursor is offscreen. We set it to -1 to signal this.
        return;
    }

    [tState initializeVerticesWithPool:_cellRenderer.verticesPool];
    const iTermMetalBufferBinding vertexBindings[] = {
        iTermMetalBufferBindingMake(iTermVertexInputIndexVertices, tState.vertexBuffer),
    };
    const iTermMetalTextureBinding textureBindings[] = {
        iTermMetalTextureBindingMake(iTermTextureIndexPrimary, tState.texture),
    };
    [_cellRenderer drawWithTransientState:tState
                            renderEncoder:frameData.renderEncoder
                         numberOfVertices:6
                             numberOfPIUs:0
                           vertexBindings:vertexBindings
                      vertexBindingCount:sizeof(vertexBindings) / sizeof(vertexBindings[0])
                          fragmentBindings:NULL
                     fragmentBindingCount:0
                              textureBindings:textureBindings
                         textureBindingCount:sizeof(textureBindings) / sizeof(textureBindings[0])];
}

#pragma mark - Private

// Use bitmap context instead of lockFocus for thread safety (BUG-3215)
- (id<MTLTexture>)newCursorGuideTextureWithTransientState:(iTermCursorGuideRendererTransientState *)tState {
    NSSize size = tState.cellConfiguration.cellSize;
    if (size.width <= 0 || size.height <= 0) {
        return nil;
    }

    // Create bitmap representation - thread-safe alternative to lockFocus
    NSBitmapImageRep *bitmapRep = [[NSBitmapImageRep alloc]
        initWithBitmapDataPlanes:NULL
                      pixelsWide:(NSInteger)size.width
                      pixelsHigh:(NSInteger)size.height
                   bitsPerSample:8
                 samplesPerPixel:4
                        hasAlpha:YES
                        isPlanar:NO
                  colorSpaceName:NSCalibratedRGBColorSpace
                     bytesPerRow:0
                    bitsPerPixel:0];

    if (!bitmapRep) {
        return nil;
    }

    // Draw into bitmap context
    NSGraphicsContext *ctx = [NSGraphicsContext graphicsContextWithBitmapImageRep:bitmapRep];
    [NSGraphicsContext saveGraphicsState];
    [NSGraphicsContext setCurrentContext:ctx];
    {
        [_color set];
        NSRect rect = NSMakeRect(0, 0, size.width, size.height);
        NSRectFillUsingOperation(rect, NSCompositingOperationSourceOver);

        rect.size.height = tState.cellConfiguration.scale;
        NSRectFillUsingOperation(rect, NSCompositingOperationSourceOver);

        rect.origin.y += size.height - tState.cellConfiguration.scale;
        NSRectFillUsingOperation(rect, NSCompositingOperationSourceOver);
    }
    [NSGraphicsContext restoreGraphicsState];

    // Create NSImage from bitmap
    NSImage *image = [[NSImage alloc] initWithSize:size];
    [image addRepresentation:bitmapRep];

    return [_cellRenderer textureFromImage:[iTermImageWrapper withImage:image]
                                   context:tState.poolContext
                                colorSpace:tState.configuration.colorSpace];
}

@end
