//
//  iTermMetalPerFrameStateRow.h
//  DashTerm2SharedARC
//
//  Created by George Nachman on 11/19/18.
//

#import <Foundation/Foundation.h>

#import "iTermMarkRenderer.h"

NS_ASSUME_NONNULL_BEGIN

@class iTermData;
@class iTermTextDrawingHelper;
@class PTYTextView;
@class ScreenCharArray;
@class VT100Screen;
@protocol iTermExternalAttributeIndexReading;
@class iTermMetalPerFrameStateConfiguration;

@interface iTermMetalPerFrameStateRow : NSObject {
  @public
    iTermMarkStyle _markStyle;
    BOOL _hoverState;
    BOOL _lineStyleMark;
    int _lineStyleMarkRightInset;
    ScreenCharArray *_screenCharLine;
    NSIndexSet *_selectedIndexSet;
    NSDate *_date;
    BOOL _belongsToBlock;
    NSData *_matches;
    NSRange _underlinedRange; // Underline for semantic history
    BOOL _x_inDeselectedRegion;
    id<iTermExternalAttributeIndexReading> _eaIndex;
}

- (instancetype)init NS_DESIGNATED_INITIALIZER;
- (iTermMetalPerFrameStateRow *)emptyCopy;

/// Repopulates an existing row object with new data. Used by the pool to reuse allocations.
- (void)repopulateWithDrawingHelper:(iTermTextDrawingHelper *)drawingHelper
                           textView:(PTYTextView *)textView
                             screen:(VT100Screen *)screen
                              width:(size_t)width
                allowOtherMarkStyle:(BOOL)allowOtherMarkStyle
                  timestampsEnabled:(BOOL)timestampsEnabled
                                row:(int)i
            totalScrollbackOverflow:(long long)totalScrollbackOverflow
                     hasFindMatches:(BOOL)hasFindMatches;

@end


@interface iTermMetalPerFrameStateRowFactory : NSObject

- (instancetype)initWithDrawingHelper:(iTermTextDrawingHelper *)drawingHelper
                             textView:(PTYTextView *)textView
                               screen:(VT100Screen *)screen
                        configuration:(iTermMetalPerFrameStateConfiguration *)configuration
                                width:(int)width NS_DESIGNATED_INITIALIZER;
- (instancetype)init NS_UNAVAILABLE;

- (iTermMetalPerFrameStateRow *)newRowForLine:(int)line;

@end

NS_ASSUME_NONNULL_END
