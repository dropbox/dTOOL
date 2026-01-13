//
//  LineBufferSorting.mm
//  DashTerm2SharedARC
//
//  Created by George Nachman on 4/12/22.
//

#include "LineBufferSorting.h"

#include <algorithm>
#include <vector>

extern "C" {
#import "iTermMalloc.h"
}

namespace {
struct LineBufferSortedPositionLess {
    bool operator()(const LineBufferSortedPosition &lhs,
                    const LineBufferSortedPosition &rhs) const {
        if (lhs.position == rhs.position) {
            // Ensure starts are processed before ends for identical positions.
            return lhs.isEnd < rhs.isEnd;
        }
        return lhs.position < rhs.position;
    }
};
}  // namespace

extern "C" LineBufferSortedPosition *SortedPositionsFromResultRanges(
    NSArray<ResultRange *> *ranges,
    BOOL includeEnds,
    NSUInteger *countOut) {
    const NSUInteger rangeCount = ranges.count;
    const NSUInteger resultCount = includeEnds ? rangeCount * 2 : rangeCount;
    if (countOut) {
        *countOut = resultCount;
    }
    if (resultCount == 0) {
        return nullptr;
    }

    std::vector<LineBufferSortedPosition> entries;
    entries.reserve(resultCount);

    NSUInteger rangeIndex = 0;
    for (ResultRange *range in ranges) {
        LineBufferSortedPosition start;
        start.position = range->position;
        start.rangeIndex = rangeIndex;
        start.isEnd = NO;
        entries.push_back(start);

        if (includeEnds) {
            LineBufferSortedPosition end = start;
            end.position = range->position + range->length - 1;
            end.isEnd = YES;
            entries.push_back(end);
        }
        rangeIndex++;
    }

    std::sort(entries.begin(), entries.end(), LineBufferSortedPositionLess());

    LineBufferSortedPosition *result =
        static_cast<LineBufferSortedPosition *>(
            iTermMalloc(entries.size() * sizeof(LineBufferSortedPosition)));
    std::copy(entries.begin(), entries.end(), result);
    return result;
}
