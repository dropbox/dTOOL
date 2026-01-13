//
//  iTermMetalPerFrameStateRow.m
//  DashTerm2
//
//  Created by George Nachman on 11/19/18.
//

#import "iTermMetalPerFrameStateRow.h"
#import "iTermMetalPerFrameStateRowPool.h"

#import "DebugLogging.h"
#import "DashTerm2SharedARC-Swift.h"
#import "iTermAdvancedSettingsModel.h"
#import "iTermColorMap.h"
#import "iTermData.h"
#import "iTermMarkRenderer.h"
#import "iTermMetalPerFrameStateConfiguration.h"
#import "iTermSelection.h"
#import "iTermTextDrawingHelper.h"
#import "PTYTextView.h"
#import "ScreenCharArray.h"
#import "VT100GridTypes.h"
#import "VT100Screen.h"
#import "VT100ScreenMark.h"

NS_ASSUME_NONNULL_BEGIN

@implementation iTermMetalPerFrameStateRow

- (instancetype)init {
    self = [super init];
    if (self) {
        _markStyle = iTermMarkStyleNone;
        _hoverState = NO;
        _lineStyleMark = NO;
        _lineStyleMarkRightInset = 0;
        _belongsToBlock = NO;
        _x_inDeselectedRegion = NO;
        _underlinedRange = NSMakeRange(0, 0);
    }
    return self;
}

- (instancetype)initEmptyFrom:(iTermMetalPerFrameStateRow *)source {
    self = [self init];
    if (self) {
        _date = source->_date;
        _belongsToBlock = source->_belongsToBlock;
        _screenCharLine = [ScreenCharArray emptyLineOfLength:source->_screenCharLine.length];
        _selectedIndexSet = nil;
    }
    return self;
}

- (instancetype)initWithDrawingHelper:(iTermTextDrawingHelper *)drawingHelper
                             textView:(PTYTextView *)textView
                               screen:(VT100Screen *)screen
                                width:(size_t)width
                  allowOtherMarkStyle:(BOOL)allowOtherMarkStyle
                    timestampsEnabled:(BOOL)timestampsEnabled
                                  row:(int)i
              totalScrollbackOverflow:(long long)totalScrollbackOverflow
                       hasFindMatches:(BOOL)hasFindMatches {
    self = [self init];
    if (self) {
        if (timestampsEnabled) {
            _date = [textView drawingHelperTimestampForLine:i];
        }
        _screenCharLine = [[screen screenCharArrayForLine:i] paddedOrTruncatedToLength:width];
        // BUG-f1077: Replace assert with guard - nil screenCharLine should create empty line, not crash
        if (!_screenCharLine) {
            DLog(@"WARNING: screenCharArrayForLine returned nil for line %d", i);
            _screenCharLine = [[[ScreenCharArray alloc] init] paddedOrTruncatedToLength:width];
        }
        // BUG-f1078: Replace assert with guard - nil line should create safe fallback, not crash
        if (_screenCharLine.line == nil) {
            DLog(@"WARNING: screenCharLine.line is nil for line %d, creating safe fallback", i);
            _screenCharLine = [[[ScreenCharArray alloc] init] paddedOrTruncatedToLength:width];
        }
        [_screenCharLine makeSafe];

        if (hasFindMatches) {
            NSData *findMatches = [drawingHelper.delegate drawingHelperMatchesOnLine:i];
            if (findMatches) {
                _matches = findMatches;
            }
        }
        _eaIndex = [[screen externalAttributeIndexForLine:i] copy];
        _belongsToBlock = _eaIndex.attributes[@0].blockIDList != nil;

        _underlinedRange = NSMakeRange(0, 0);
        _x_inDeselectedRegion = drawingHelper.selectedCommandRegion.length > 0 &&
                                !NSLocationInRange(i, drawingHelper.selectedCommandRegion);
        _markStyle = [self markStyleForLine:i
                                    enabled:drawingHelper.drawMarkIndicators
                                   textView:textView
                        allowOtherMarkStyle:allowOtherMarkStyle
                                    hasFold:[drawingHelper.folds containsIndex:i]
                              lineStyleMark:&_lineStyleMark
                    lineStyleMarkRightInset:&_lineStyleMarkRightInset];
        _hoverState = NSLocationInRange(i, drawingHelper.highlightedBlockLineRange);
    }
    return self;
}

- (void)repopulateWithDrawingHelper:(iTermTextDrawingHelper *)drawingHelper
                           textView:(PTYTextView *)textView
                             screen:(VT100Screen *)screen
                              width:(size_t)width
                allowOtherMarkStyle:(BOOL)allowOtherMarkStyle
                  timestampsEnabled:(BOOL)timestampsEnabled
                                row:(int)i
            totalScrollbackOverflow:(long long)totalScrollbackOverflow
                     hasFindMatches:(BOOL)hasFindMatches {
    // Clear previous references to allow deallocation
    _selectedIndexSet = nil;
    _matches = nil;
    _eaIndex = nil;

    if (timestampsEnabled) {
        _date = [textView drawingHelperTimestampForLine:i];
    } else {
        _date = nil;
    }

    ScreenCharArray *sourceLine = nil;
    if ([iTermAdvancedSettingsModel dtermCoreGridEnabled] && textView.dtermGridAdapter != nil) {
        sourceLine = [textView.dtermGridAdapter screenCharArrayForLine:i width:(int)width];
    }
    if (sourceLine == nil) {
        sourceLine = [screen screenCharArrayForLine:i];
    }
    _screenCharLine = [sourceLine paddedOrTruncatedToLength:width];
    // BUG-f1077: Replace assert with guard - nil screenCharLine should create empty line, not crash
    if (!_screenCharLine) {
        DLog(@"WARNING: repopulate screenCharArrayForLine returned nil for line %d", i);
        _screenCharLine = [[[ScreenCharArray alloc] init] paddedOrTruncatedToLength:width];
    }
    // BUG-f1078: Replace assert with guard - nil line should create safe fallback, not crash
    if (_screenCharLine.line == nil) {
        DLog(@"WARNING: repopulate screenCharLine.line is nil for line %d, creating safe fallback", i);
        _screenCharLine = [[[ScreenCharArray alloc] init] paddedOrTruncatedToLength:width];
    }
    [_screenCharLine makeSafe];

    if (hasFindMatches) {
        NSData *findMatches = [drawingHelper.delegate drawingHelperMatchesOnLine:i];
        if (findMatches) {
            _matches = findMatches;
        }
    }

    _eaIndex = [[screen externalAttributeIndexForLine:i] copy];
    _belongsToBlock = _eaIndex.attributes[@0].blockIDList != nil;

    _underlinedRange = NSMakeRange(0, 0);
    _x_inDeselectedRegion =
        drawingHelper.selectedCommandRegion.length > 0 && !NSLocationInRange(i, drawingHelper.selectedCommandRegion);
    _markStyle = [self markStyleForLine:i
                                enabled:drawingHelper.drawMarkIndicators
                               textView:textView
                    allowOtherMarkStyle:allowOtherMarkStyle
                                hasFold:[drawingHelper.folds containsIndex:i]
                          lineStyleMark:&_lineStyleMark
                lineStyleMarkRightInset:&_lineStyleMarkRightInset];
    _hoverState = NSLocationInRange(i, drawingHelper.highlightedBlockLineRange);
}

- (iTermMarkStyle)markStyleForLine:(int)i
                           enabled:(BOOL)enabled
                          textView:(PTYTextView *)textView
               allowOtherMarkStyle:(BOOL)allowOtherMarkStyle
                           hasFold:(BOOL)folded
                     lineStyleMark:(out BOOL *)lineStyleMark
           lineStyleMarkRightInset:(out int *)lineStyleMarkRightInset {
    id<iTermMark> genericMark = [textView.dataSource drawableMarkOnLine:i];
    id<VT100ScreenMarkReading> mark = (id<VT100ScreenMarkReading>)genericMark;
    *lineStyleMarkRightInset = 0;
    *lineStyleMark = NO;
    if (mark != nil && enabled) {
        if (mark.lineStyle) {
            // Don't draw line-style mark in selected command region or immediately after selected command region.
            // Note: that logic is in populateLineStyleMarkRendererTransientStateWithFrameData.
            *lineStyleMark = YES;
            if (mark.command.length) {
                *lineStyleMarkRightInset = iTermTextDrawingHelperLineStyleMarkRightInsetCells;
            }
        }
    }
    if (!mark) {
        if (folded) {
            // Folds without a mark should draw as folded success.
            return iTermMarkStyleFoldedSuccess;
        } else {
            return iTermMarkStyleNone;
        }
    }
    if (mark.name.length == 0) {
        if (!enabled && !folded) {
            return iTermMarkStyleNone;
        }
    }
    if (mark.code == 0) {
        return folded ? iTermMarkStyleFoldedSuccess : iTermMarkStyleRegularSuccess;
    }
    if (allowOtherMarkStyle && mark.code >= 128 && mark.code <= 128 + 32) {
        return folded ? iTermMarkStyleFoldedOther : iTermMarkStyleRegularOther;
    }
    return folded ? iTermMarkStyleFoldedFailure : iTermMarkStyleRegularFailure;
}

- (iTermMetalPerFrameStateRow *)emptyCopy {
    return [[iTermMetalPerFrameStateRow alloc] initEmptyFrom:self];
}

@end

@implementation iTermMetalPerFrameStateRowFactory {
    iTermTextDrawingHelper *_drawingHelper;
    PTYTextView *_textView;
    VT100Screen *_screen;
    int _width;
    long long _totalScrollbackOverflow;
    BOOL _allowOtherMarkStyle;
    BOOL _timestampsEnabled;
    BOOL _hasFindMatches;
    iTermSelection *_selection;
    BOOL _selectionHasSelection;
    long long _selectionMinAbsLine;
    long long _selectionMaxAbsLine;
    BOOL _hasUnderlinedRange;
    long long _underlinedRangeMinAbsLine;
    long long _underlinedRangeMaxAbsLine;
}

- (instancetype)initWithDrawingHelper:(iTermTextDrawingHelper *)drawingHelper
                             textView:(PTYTextView *)textView
                               screen:(VT100Screen *)screen
                        configuration:(iTermMetalPerFrameStateConfiguration *)configuration
                                width:(int)width {
    self = [super init];
    if (self) {
        _drawingHelper = drawingHelper;
        _textView = textView;
        _screen = screen;
        _width = width;
        _totalScrollbackOverflow = [screen totalScrollbackOverflow];
        _allowOtherMarkStyle = [iTermAdvancedSettingsModel showYellowMarkForJobStoppedBySignal];
        _timestampsEnabled = configuration->_timestampsEnabled;
        _hasFindMatches = [textView hasFindOnPageMatches];
        _selection = textView.selection;
        _selectionHasSelection = _selection.hasSelection;
        if (_selectionHasSelection) {
            const VT100GridAbsCoordRange span = _selection.spanningAbsRange;
            const long long startLine = span.start.y;
            const long long endLine = span.end.y;
            if (startLine <= endLine) {
                _selectionMinAbsLine = startLine;
                _selectionMaxAbsLine = endLine;
            } else {
                _selectionMinAbsLine = endLine;
                _selectionMaxAbsLine = startLine;
            }
        } else {
            _selectionMinAbsLine = 1;
            _selectionMaxAbsLine = 0;
        }

        const VT100GridAbsWindowedRange underlinedRange = drawingHelper.underlinedRange;
        if (underlinedRange.coordRange.start.x >= 0) {
            _hasUnderlinedRange = YES;
            const long long startLine = underlinedRange.coordRange.start.y;
            const long long endLine = underlinedRange.coordRange.end.y;
            if (startLine <= endLine) {
                _underlinedRangeMinAbsLine = startLine;
                _underlinedRangeMaxAbsLine = endLine;
            } else {
                _underlinedRangeMinAbsLine = endLine;
                _underlinedRangeMaxAbsLine = startLine;
            }
        } else {
            _hasUnderlinedRange = NO;
            _underlinedRangeMinAbsLine = 1;
            _underlinedRangeMaxAbsLine = 0;
        }
    }
    return self;
}

- (void)applySelectionToRow:(iTermMetalPerFrameStateRow *)row absoluteLine:(long long)absoluteLine {
    if (!_selectionHasSelection || !_selection) {
        return;
    }
    if (absoluteLine < _selectionMinAbsLine || absoluteLine > _selectionMaxAbsLine) {
        return;
    }

    NSIndexSet *selectedIndexes = [_selection selectedIndexesIncludingTabFillersInAbsoluteLine:absoluteLine];
    if (selectedIndexes.count == 0) {
        return;
    }
    row->_selectedIndexSet = selectedIndexes;
}

- (void)applyUnderlinedRangeToRow:(iTermMetalPerFrameStateRow *)row absoluteLine:(long long)absoluteLine {
    if (!_hasUnderlinedRange) {
        return;
    }
    if (absoluteLine < _underlinedRangeMinAbsLine || absoluteLine > _underlinedRangeMaxAbsLine) {
        return;
    }

    row->_underlinedRange = [_drawingHelper underlinedRangeOnLine:absoluteLine];
}

- (iTermMetalPerFrameStateRow *)newRowForLine:(int)line {
    // Note: Object pooling for iTermMetalPerFrameStateRow was evaluated and found to add overhead
    // rather than save time. The synthetic benchmark showed ~30% slowdown from CFArray pool operations.
    // This is different from iTermMetalRowDataPool which pools large byte buffers.
    // The expensive work here (screenCharArrayForLine:, externalAttributeIndexForLine:) must happen
    // each frame regardless, so pooling only saves trivial NSObject allocation overhead.
    // See benchmarks/perframe_state_row_benchmark.m for details.
    iTermMetalPerFrameStateRow *row = [[iTermMetalPerFrameStateRow alloc] initWithDrawingHelper:_drawingHelper
                                                                                       textView:_textView
                                                                                         screen:_screen
                                                                                          width:_width
                                                                            allowOtherMarkStyle:_allowOtherMarkStyle
                                                                              timestampsEnabled:_timestampsEnabled
                                                                                            row:line
                                                                        totalScrollbackOverflow:_totalScrollbackOverflow
                                                                                 hasFindMatches:_hasFindMatches];
    const long long absoluteLine = _totalScrollbackOverflow + line;
    [self applySelectionToRow:row absoluteLine:absoluteLine];
    [self applyUnderlinedRangeToRow:row absoluteLine:absoluteLine];
    return row;
}

@end

NS_ASSUME_NONNULL_END
