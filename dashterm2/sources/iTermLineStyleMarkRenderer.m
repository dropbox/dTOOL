//
//  iTermLineStyleMarkRenderer.m
//  DashTerm2
//
//  Created by George Nachman on 4/8/23.
//

#import "iTermLineStyleMarkRenderer.h"

#import "DebugLogging.h"
#import "iTermAdvancedSettingsModel.h"
#import "iTermPreferences.h"
#import "iTermTextDrawingHelper.h"
#import "iTermMetalCellRenderer.h"
#import "NSColor+iTerm.h"
#import "NSObject+iTerm.h"

// Optimization: Cache NSNumber objects for common row indices (0-255) to avoid boxing.
// Metal rendering iterates over visible rows, which are typically 0-100 for most terminals.
// Caching 256 values covers all typical use cases with zero allocations.
static const int kCachedRowKeyCount = 256;
static NSNumber *sRowKeyCache[kCachedRowKeyCount];

NS_INLINE NSNumber *iTermRowKeyToNumber(int row) {
    if (row >= 0 && row < kCachedRowKeyCount) {
        return sRowKeyCache[row];
    }
    return @(row);
}

__attribute__((constructor)) static void iTermLineStyleMarkRendererInitializeRowKeyCache(void) {
    for (int i = 0; i < kCachedRowKeyCount; i++) {
        sRowKeyCache[i] = @(i);
    }
}

@interface iTermLineStyleMarkInfo : NSObject
@property (nonatomic) iTermMarkStyle style;
@property (nonatomic) int rightInset;
@end

@implementation iTermLineStyleMarkInfo
@end

@interface iTermLineStyleMarkRendererTransientState ()
@property (nonatomic, copy) NSDictionary<NSNumber *, iTermLineStyleMarkInfo *> *marks;
@end

@implementation iTermLineStyleMarkRendererTransientState {
    NSMutableDictionary<NSNumber *, iTermLineStyleMarkInfo *> *_marks;
}

- (void)writeDebugInfoToFolder:(NSURL *)folder {
    [super writeDebugInfoToFolder:folder];
    [[NSString stringWithFormat:@"lineStyleMarks=%@", _marks]
        writeToURL:[folder URLByAppendingPathComponent:@"state.txt"]
        atomically:NO
          encoding:NSUTF8StringEncoding
             error:NULL];
}

- (nonnull NSData *)newMarkPerInstanceUniforms {
    NSMutableData *data = [[NSMutableData alloc] initWithLength:sizeof(iTermLineStyleMarkPIU) * _marks.count];
    iTermLineStyleMarkPIU *pius = (iTermLineStyleMarkPIU *)data.mutableBytes;
    __block size_t i = 0;
    const CGFloat scale = self.configuration.scale;
    const CGFloat heightInPoints = 1.0;
    const CGFloat cellHeightInPoints = self.cellConfiguration.cellSize.height / scale;
    const CGFloat marginInPoints = self.margins.bottom / scale;
    const CGFloat viewportHeightInPoints = self.configuration.viewportSize.y / scale;

    [_marks enumerateKeysAndObjectsUsingBlock:^(NSNumber *rowNumber, iTermLineStyleMarkInfo *info, BOOL *stop) {
        const CGFloat offsetFromTopInPoints =
            round((((CGFloat)rowNumber.intValue) - 0.5) * cellHeightInPoints) + marginInPoints;
        const CGFloat yInPoints = viewportHeightInPoints - offsetFromTopInPoints - heightInPoints;
        pius[i].y = yInPoints * scale;
        pius[i].rightInset = info.rightInset * self.cellConfiguration.cellSize.width;
        switch (info.style) {
            case iTermMarkStyleRegularOther:
            case iTermMarkStyleFoldedOther:
                pius[i].color = self.colors.other;
                break;
            case iTermMarkStyleRegularSuccess:
            case iTermMarkStyleFoldedSuccess:
                pius[i].color = self.colors.success;
                break;
            case iTermMarkStyleRegularFailure:
            case iTermMarkStyleFoldedFailure:
                pius[i].color = self.colors.failure;
                break;
            default:
                pius[i].color = self.colors.other;
        }
        i++;
    }];
    return data;
}

- (void)setMarkStyle:(iTermMarkStyle)markStyle row:(int)row rightInset:(int)rightInset {
    if (!_marks) {
        _marks = [NSMutableDictionary dictionaryWithCapacity:64]; // Line style marks by row
    }
    NSNumber *rowKey = iTermRowKeyToNumber(row);
    if (markStyle == iTermMarkStyleNone) {
        [_marks removeObjectForKey:rowKey];
    } else {
        iTermLineStyleMarkInfo *info = [[iTermLineStyleMarkInfo alloc] init];
        info.style = markStyle;
        info.rightInset = rightInset;
        _marks[rowKey] = info;
    }
}

@end


@implementation iTermLineStyleMarkRenderer {
    iTermMetalCellRenderer *_cellRenderer;
    iTermMetalMixedSizeBufferPool *_piuPool;
}

- (instancetype)initWithDevice:(id<MTLDevice>)device {
    self = [super init];
    if (self) {
        _cellRenderer =
            [[iTermMetalCellRenderer alloc] initWithDevice:device
                                        vertexFunctionName:@"iTermLineStyleMarkVertexShader"
                                      fragmentFunctionName:@"iTermLineStyleMarkFragmentShader"
                                                  blending:[iTermMetalBlending compositeSourceOver]
                                            piuElementSize:sizeof(iTermLineStyleMarkPIU)
                                       transientStateClass:[iTermLineStyleMarkRendererTransientState class]];
        _piuPool =
            [[iTermMetalMixedSizeBufferPool alloc] initWithDevice:device
                                                         capacity:iTermMetalDriverMaximumNumberOfFramesInFlight + 1
                                                             name:@"Line-style mark PIU"];
    }
    return self;
}

- (BOOL)rendererDisabled {
    return NO;
}

- (iTermMetalFrameDataStat)createTransientStateStat {
    return iTermMetalFrameDataStatPqCreateMarkTS;
}

- (nullable __kindof iTermMetalRendererTransientState *)
    createTransientStateForCellConfiguration:(iTermCellRenderConfiguration *)configuration
                               commandBuffer:(id<MTLCommandBuffer>)commandBuffer {
    __kindof iTermMetalCellRendererTransientState *_Nonnull transientState =
        [_cellRenderer createTransientStateForCellConfiguration:configuration commandBuffer:commandBuffer];
    [self initializeTransientState:transientState];
    return transientState;
}

- (void)initializeTransientState:(iTermLineStyleMarkRendererTransientState *)tState {
    DLog(@"Initialize transient state");
    const CGFloat scale = tState.configuration.scale;

    DLog(@"Scale is %@, cell size is %@, cell size without spacing is %@", @(scale),
         NSStringFromSize(tState.cellConfiguration.cellSize),
         NSStringFromSize(tState.cellConfiguration.cellSizeWithoutSpacing));

    const NSSize size = NSMakeSize(
        tState.margins.left + tState.cellConfiguration.gridSize.width * tState.cellConfiguration.cellSize.width, scale);
    tState.vertexBuffer = [_cellRenderer newQuadOfSize:size poolContext:tState.poolContext];
}

- (void)drawWithFrameData:(iTermMetalFrameData *)frameData
           transientState:(__kindof iTermMetalCellRendererTransientState *)transientState {
    iTermLineStyleMarkRendererTransientState *tState = transientState;
    if (tState.marks.count == 0) {
        return;
    }

    NSData *data = [tState newMarkPerInstanceUniforms];
    tState.pius = [_piuPool requestBufferFromContext:tState.poolContext size:data.length];
    memcpy(tState.pius.contents, data.bytes, data.length);

    const iTermMetalBufferBinding vertexBindings[] = {
        iTermMetalBufferBindingMake(iTermVertexInputIndexVertices, tState.vertexBuffer),
        iTermMetalBufferBindingMake(iTermVertexInputIndexPerInstanceUniforms, tState.pius),
        iTermMetalBufferBindingMake(iTermVertexInputIndexOffset, tState.offsetBuffer),
    };
    [_cellRenderer drawWithTransientState:tState
                            renderEncoder:frameData.renderEncoder
                         numberOfVertices:6
                             numberOfPIUs:tState.marks.count
                           vertexBindings:vertexBindings
                       vertexBindingCount:sizeof(vertexBindings) / sizeof(vertexBindings[0])
                         fragmentBindings:NULL
                     fragmentBindingCount:0
                          textureBindings:NULL
                      textureBindingCount:0];
}

@end
