//
//  iTermHighlightRowRenderer.m
//  DashTerm2SharedARC
//
//  Created by George Nachman on 1/9/18.
//

#import "iTermHighlightRowRenderer.h"
#include <vector>

namespace DashTerm2 {
struct Highlight {
    // premultiplied
    vector_float4 color;
    int row;
};
} // namespace DashTerm2

@interface iTermHighlightRowRendererTransientState ()
- (NSUInteger)highlightCount;
- (size_t)piuSize;
- (void)fillPIUBuffer:(iTermHighlightRowPIU *)pius;
@end

@implementation iTermHighlightRowRendererTransientState {
    std::vector<DashTerm2::Highlight> _highlights;
}

- (void)writeDebugInfoToFolder:(NSURL *)folder {
    [super writeDebugInfoToFolder:folder];
    NSMutableString *s = [[NSMutableString alloc] initWithCapacity:_highlights.size() * 64];
    // Use const reference to avoid copying the struct on each iteration
    for (const auto &h : _highlights) {
        [s appendFormat:@"color=(%@, %@, %@, %@) row=%@\n", @(h.color.x), @(h.color.y), @(h.color.z), @(h.color.w),
                        @(h.row)];
    }
    [s writeToURL:[folder URLByAppendingPathComponent:@"state.txt"]
        atomically:NO
          encoding:NSUTF8StringEncoding
             error:NULL];
}

- (void)setOpacity:(CGFloat)opacity color:(vector_float3)color row:(int)row {
    DashTerm2::Highlight h = {
        .color = simd_make_float4(color.x * opacity, color.y * opacity, color.z * opacity, opacity), .row = row};
    _highlights.push_back(h);
}

- (NSUInteger)highlightCount {
    return _highlights.size();
}

- (size_t)piuSize {
    return sizeof(iTermHighlightRowPIU) * _highlights.size();
}

// Fill PIU buffer directly - avoids intermediate NSMutableData allocation
- (void)fillPIUBuffer:(iTermHighlightRowPIU *)pius {
    const VT100GridSize gridSize = self.cellConfiguration.gridSize;
    const CGSize cellSize = self.cellConfiguration.cellSize;
    const CGFloat top = self.margins.top;

    for (size_t i = 0; i < _highlights.size(); i++) {
        const auto &h = _highlights[i];
        pius[i] = (iTermHighlightRowPIU){
            .yOffset = static_cast<float>((gridSize.height - h.row - 1) * cellSize.height + top), .color = h.color};
    }
}

@end

@implementation iTermHighlightRowRenderer {
    iTermMetalCellRenderer *_cellRenderer;
    iTermMetalMixedSizeBufferPool *_piuPool;
}

- (instancetype)initWithDevice:(id<MTLDevice>)device {
    self = [super init];
    if (self) {
        _cellRenderer = [[iTermMetalCellRenderer alloc] initWithDevice:device
                                                    vertexFunctionName:@"iTermHighlightRowVertexShader"
                                                  fragmentFunctionName:@"iTermHighlightRowFragmentShader"
                                                              blending:[iTermMetalBlending compositeSourceOver]
                                                        piuElementSize:sizeof(iTermHighlightRowPIU)
                                                   transientStateClass:[iTermHighlightRowRendererTransientState class]];
        _piuPool =
            [[iTermMetalMixedSizeBufferPool alloc] initWithDevice:device
                                                         capacity:iTermMetalDriverMaximumNumberOfFramesInFlight + 1
                                                             name:@"highlight row PIU"];
    }
    return self;
}

- (BOOL)rendererDisabled {
    return NO;
}

- (iTermMetalFrameDataStat)createTransientStateStat {
    return iTermMetalFrameDataStatPqCreateCursorGuideTS;
}

- (nullable __kindof iTermMetalRendererTransientState *)
    createTransientStateForCellConfiguration:(iTermCellRenderConfiguration *)configuration
                               commandBuffer:(id<MTLCommandBuffer>)commandBuffer {
    __kindof iTermMetalCellRendererTransientState *_Nonnull transientState =
        [_cellRenderer createTransientStateForCellConfiguration:configuration commandBuffer:commandBuffer];
    [self initializeTransientState:transientState];
    return transientState;
}

- (void)initializeTransientState:(iTermHighlightRowRendererTransientState *)tState {
}

- (void)drawWithFrameData:(iTermMetalFrameData *)frameData
           transientState:(__kindof iTermMetalCellRendererTransientState *)transientState {
    iTermHighlightRowRendererTransientState *tState = transientState;
    const NSUInteger count = [tState highlightCount];
    if (count == 0) {
        return;
    }

    const VT100GridSize gridSize = tState.cellConfiguration.gridSize;
    const CGSize cellSize = tState.cellConfiguration.cellSize;
    const CGFloat left = tState.margins.left;
    const CGFloat right = tState.margins.right;

    // Create a single quad for one row at y=0. The PIU will provide the y offset per instance.
    const CGRect quad = CGRectMake(0, 0, cellSize.width * gridSize.width + left + right, cellSize.height);
    const float minX = static_cast<float>(CGRectGetMinX(quad));
    const float maxX = static_cast<float>(CGRectGetMaxX(quad));
    const float minY = static_cast<float>(CGRectGetMinY(quad));
    const float maxY = static_cast<float>(CGRectGetMaxY(quad));
    const iTermVertex vertices[] = {
        // Pixel Positions (triangle 1)
        {{maxX, minY}, {0, 0}},
        {{minX, minY}, {0, 0}},
        {{minX, maxY}, {0, 0}},
        // Pixel Positions (triangle 2)
        {{maxX, minY}, {0, 0}},
        {{minX, maxY}, {0, 0}},
        {{maxX, maxY}, {0, 0}},
    };

    id<MTLBuffer> vertexBuffer = [_cellRenderer.verticesPool requestBufferFromContext:tState.poolContext
                                                                            withBytes:vertices
                                                                       checkIfChanged:YES];

    // Create PIU buffer and fill directly (avoids intermediate NSMutableData allocation)
    const size_t piuSize = [tState piuSize];
    id<MTLBuffer> piuBuffer = [_piuPool requestBufferFromContext:tState.poolContext size:piuSize];
    [tState fillPIUBuffer:(iTermHighlightRowPIU *)piuBuffer.contents];

    const iTermMetalBufferBinding vertexBindings[] = {
        iTermMetalBufferBindingMake(iTermVertexInputIndexVertices, vertexBuffer),
        iTermMetalBufferBindingMake(iTermVertexInputIndexPerInstanceUniforms, piuBuffer),
    };

    // Single draw call for all highlighted rows
    [_cellRenderer drawWithTransientState:tState
                            renderEncoder:frameData.renderEncoder
                         numberOfVertices:6
                             numberOfPIUs:count
                           vertexBindings:vertexBindings
                       vertexBindingCount:sizeof(vertexBindings) / sizeof(vertexBindings[0])
                         fragmentBindings:NULL
                     fragmentBindingCount:0
                          textureBindings:NULL
                      textureBindingCount:0];
}

@end
