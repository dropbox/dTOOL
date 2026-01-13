
//
//  iTermTextRendererTransientStatePrivate.h
//  DashTerm2
//
//  Created by George Nachman on 12/22/17.
//

#import "iTermASCIITexture.h"
#import "iTermTexturePageCollection.h"

@interface iTermTextRendererTransientState ()

@property (nonatomic, readonly) NSData *piuData;
@property (nonatomic, strong) id<MTLDevice> device;
@property (nonatomic, strong) iTermASCIITextureGroup *asciiTextureGroup;
@property (nonatomic) iTermTexturePageCollectionSharedPointer *texturePageCollectionSharedPointer;
@property (nonatomic) NSInteger numberOfCells;
@property (nonatomic) CGSize asciiOffset;

// Phase 1C optimization: Pre-allocate PIU arrays based on expected number of cells.
// Call this after setting numberOfCells to avoid segment allocations during rendering.
- (void)preallocatePIUArraysForCellCount:(NSInteger)cellCount;

+ (NSString *)formatTextPIU:(iTermTextPIU)a;

- (void)enumerateDraws:(void (^)(const iTermTextPIU *,
                                 NSInteger,
                                 id<MTLTexture>,
                                 vector_uint2,
                                 vector_uint2,
                                 iTermMetalUnderlineDescriptor,
                                 iTermMetalUnderlineDescriptor,
                                 BOOL underlined,
                                 BOOL emoji))block
             copyBlock:(void (^)(void))copyBlock;

@end

