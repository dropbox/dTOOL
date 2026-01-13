//
//  LineBufferSorting.h
//  DashTerm2
//
//  Created by George Nachman on 4/12/22.
//

#import <Foundation/Foundation.h>
#import "LineBufferHelpers.h"
#import "VT100GridTypes.h"

typedef struct {
    int position;
    NSUInteger rangeIndex;
    BOOL isEnd;
} LineBufferSortedPosition;

#if __cplusplus
extern "C" {
#endif
LineBufferSortedPosition *SortedPositionsFromResultRanges(NSArray<ResultRange *> *ranges,
                                                          BOOL includeEnds,
                                                          NSUInteger *countOut);
#if __cplusplus
}
#endif
