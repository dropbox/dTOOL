//
//  iTermStatusBarStableLayoutAlgorithm.m
//  DashTerm2SharedARC
//
//  Created by George Nachman on 1/20/19.
//

#import "iTermStatusBarStableLayoutAlgorithm.h"

#import <CoreFoundation/CoreFoundation.h>
#import <stdint.h>
#import <stdlib.h>
#import <string.h>

#import "DebugLogging.h"
#import "iTermStatusBarComponent.h"
#import "iTermStatusBarContainerView.h"
#import "iTermStatusBarFixedSpacerComponent.h"
#import "iTermStatusBarSpringComponent.h"
#import "NSArray+iTerm.h"

// Tracks the minimum-width state so the removal loop can update it incrementally.
typedef struct {
    CGFloat largestMinimumSize;
    NSUInteger numberOfViewsAtLargestMinimum;
    NSUInteger numberOfNonFixedViews;
    CGFloat widthOfAllFixedSpacers;
} iTermStatusBarStableLayoutMinimumWidthMetrics;

typedef struct {
    NSInteger leftIndex;
    NSInteger rightIndex;
} iTermStatusBarStableLayoutNeighborPair;

typedef struct {
    NSInteger *previous;
    NSInteger *next;
    NSUInteger count;
    BOOL ownsStorage;
} iTermStatusBarStableLayoutNeighborTable;

typedef struct {
    uint8_t *states;
    NSUInteger count;
    BOOL ownsStorage;
} iTermStatusBarStableLayoutRemovalMask;

typedef struct {
    __unsafe_unretained iTermStatusBarContainerView *view;
    NSUInteger originalIndex;
    double priority;
    CGFloat minimumWidth;
} iTermStatusBarStableLayoutRemovalCandidate;

typedef struct {
    double springConstant;
    CGFloat minSize;
    CGFloat maxSize;
    BOOL isFixedSpacer;
    BOOL canStillGrow;
} iTermStatusBarStableLayoutDistributionInfo;

typedef struct {
    uint8_t *removalMaskStorage;
    NSUInteger removalMaskCapacity;
    NSInteger *neighborStorage;
    NSUInteger neighborCapacity;
    iTermStatusBarStableLayoutRemovalCandidate *candidateStorage;
    NSUInteger candidateCapacity;
    iTermStatusBarStableLayoutDistributionInfo *distributionInfoStorage;
    NSUInteger distributionInfoCapacity;
    struct iTermStatusBarStableLayoutMinimumWidthBucket *minimumWidthBucketStorage;
    NSUInteger minimumWidthBucketCapacity;
} iTermStatusBarStableLayoutScratchBuffers;

typedef struct {
    iTermStatusBarStableLayoutRemovalCandidate *orderedCandidates;
    NSUInteger count;
    BOOL usesScratchStorage;
} iTermStatusBarStableLayoutRemovalPlan;

typedef struct {
    __unsafe_unretained iTermStatusBarContainerView **items;
    NSUInteger count;
} iTermStatusBarStableLayoutViewSlice;

typedef struct iTermStatusBarStableLayoutMinimumWidthBucket {
    CGFloat width;
    NSUInteger count;
} iTermStatusBarStableLayoutMinimumWidthBucket;

typedef struct {
    iTermStatusBarStableLayoutMinimumWidthBucket *buckets;
    NSUInteger bucketCount;
    NSUInteger currentIndex;
    BOOL usesScratchStorage;
} iTermStatusBarStableLayoutMinimumWidthHistogram;

@interface iTermStatusBarStableLayoutAlgorithm () {
    iTermStatusBarStableLayoutScratchBuffers _scratchBuffers;
}
@end

NS_INLINE BOOL iTermStatusBarStableLayoutViewIsFixedSpacer(iTermStatusBarContainerView *view) {
    return [view.component isKindOfClass:[iTermStatusBarFixedSpacerComponent class]];
}

static NSUInteger iTermStatusBarStableLayoutIndexForView(iTermStatusBarContainerView *view,
                                                         NSArray<iTermStatusBarContainerView *> *views) {
    if (!view) {
        return NSNotFound;
    }
    return [views indexOfObjectIdenticalTo:view];
}

static BOOL iTermStatusBarStableLayoutRemovalMaskInit(iTermStatusBarStableLayoutRemovalMask *mask,
                                                      NSUInteger count,
                                                      iTermStatusBarStableLayoutScratchBuffers *scratch) {
    if (!mask || count == 0) {
        if (mask) {
            mask->states = NULL;
            mask->count = 0;
            mask->ownsStorage = NO;
        }
        return NO;
    }

    const size_t bytes = sizeof(uint8_t) * count;
    if (scratch) {
        if (scratch->removalMaskCapacity < count) {
            uint8_t *newStorage = realloc(scratch->removalMaskStorage, bytes);
            if (newStorage) {
                scratch->removalMaskStorage = newStorage;
                scratch->removalMaskCapacity = count;
            }
        }
        if (scratch->removalMaskCapacity >= count && scratch->removalMaskStorage) {
            memset(scratch->removalMaskStorage, 0, bytes);
            mask->states = scratch->removalMaskStorage;
            mask->count = count;
            mask->ownsStorage = NO;
            return YES;
        }
    }

    mask->states = calloc(count, sizeof(uint8_t));
    if (!mask->states) {
        mask->count = 0;
        mask->ownsStorage = NO;
        return NO;
    }
    memset(mask->states, 0, bytes);
    mask->count = count;
    mask->ownsStorage = YES;
    return YES;
}

static void iTermStatusBarStableLayoutRemovalMaskDestroy(iTermStatusBarStableLayoutRemovalMask *mask) {
    if (!mask) {
        return;
    }
    if (mask->ownsStorage) {
        free(mask->states);
    }
    mask->states = NULL;
    mask->count = 0;
    mask->ownsStorage = NO;
}

static inline BOOL iTermStatusBarStableLayoutRemovalMaskIndexIsMarked(const iTermStatusBarStableLayoutRemovalMask *mask,
                                                                     NSUInteger index) {
    return mask && mask->states && index < mask->count && mask->states[index];
}

static inline void iTermStatusBarStableLayoutRemovalMaskMarkIndex(iTermStatusBarStableLayoutRemovalMask *mask,
                                                                  NSUInteger index) {
    if (!mask || !mask->states || index >= mask->count) {
        return;
    }
    mask->states[index] = 1;
}

static BOOL iTermStatusBarStableLayoutRemovalMaskViewIsMarked(const iTermStatusBarStableLayoutRemovalMask *mask,
                                                              CFSetRef fallbackSet,
                                                              NSUInteger index,
                                                              iTermStatusBarContainerView *view) {
    if (index != NSNotFound && iTermStatusBarStableLayoutRemovalMaskIndexIsMarked(mask, index)) {
        return YES;
    }
    if (fallbackSet && view) {
        return CFSetContainsValue(fallbackSet, (__bridge const void *)(view));
    }
    return NO;
}

static void iTermStatusBarStableLayoutMarkRemoval(iTermStatusBarStableLayoutRemovalMask *mask,
                                                  CFMutableSetRef fallbackSet,
                                                  NSUInteger index,
                                                  iTermStatusBarContainerView *view) {
    if (index != NSNotFound) {
        iTermStatusBarStableLayoutRemovalMaskMarkIndex(mask, index);
    }
    if (fallbackSet && view) {
        CFSetAddValue(fallbackSet, (__bridge const void *)(view));
    }
}

static BOOL iTermStatusBarStableLayoutNeighborTableInit(iTermStatusBarStableLayoutNeighborTable *table,
                                                        NSUInteger count,
                                                        iTermStatusBarStableLayoutScratchBuffers *scratch) {
    if (!table || count == 0) {
        if (table) {
            table->previous = NULL;
            table->next = NULL;
            table->count = 0;
            table->ownsStorage = NO;
        }
        return NO;
    }

    NSInteger *previous = NULL;
    NSInteger *next = NULL;
    BOOL ownsStorage = YES;
    BOOL usingScratch = NO;
    if (scratch) {
        if (scratch->neighborCapacity < count) {
            const size_t bytes = sizeof(NSInteger) * count * 2;
            NSInteger *newStorage = realloc(scratch->neighborStorage, bytes);
            if (newStorage) {
                scratch->neighborStorage = newStorage;
                scratch->neighborCapacity = count;
            }
        }
        if (scratch->neighborCapacity >= count && scratch->neighborStorage) {
            previous = scratch->neighborStorage;
            next = scratch->neighborStorage + count;
            ownsStorage = NO;
            usingScratch = YES;
        }
    }

    if (!usingScratch) {
        previous = calloc(count, sizeof(NSInteger));
        next = calloc(count, sizeof(NSInteger));
        if (!previous || !next) {
            free(previous);
            free(next);
            if (table) {
                table->previous = NULL;
                table->next = NULL;
                table->count = 0;
                table->ownsStorage = NO;
            }
            return NO;
        }
        ownsStorage = YES;
    }

    table->previous = previous;
    table->next = next;
    table->count = count;
    table->ownsStorage = ownsStorage;

    for (NSUInteger index = 0; index < count; index++) {
        previous[index] = (index == 0) ? -1 : (NSInteger)index - 1;
        next[index] = (index + 1 < count) ? (NSInteger)index + 1 : -1;
    }
    return YES;
}

static void iTermStatusBarStableLayoutNeighborTableDestroy(iTermStatusBarStableLayoutNeighborTable *table) {
    if (!table) {
        return;
    }
    if (table->ownsStorage) {
        free(table->previous);
        free(table->next);
    }
    table->previous = NULL;
    table->next = NULL;
    table->count = 0;
    table->ownsStorage = NO;
}

static iTermStatusBarStableLayoutNeighborPair iTermStatusBarStableLayoutNeighborTableRemoveIndex(iTermStatusBarStableLayoutNeighborTable *table,
                                                                                                 NSUInteger index) {
    iTermStatusBarStableLayoutNeighborPair pair = { .leftIndex = -1, .rightIndex = -1 };
    if (!table || index >= table->count) {
        return pair;
    }
    const NSInteger left = table->previous[index];
    const NSInteger right = table->next[index];
    if (left >= 0 && (NSUInteger)left < table->count) {
        table->next[left] = right;
    }
    if (right >= 0 && (NSUInteger)right < table->count) {
        table->previous[right] = left;
    }
    table->previous[index] = -1;
    table->next[index] = -1;
    pair.leftIndex = left;
    pair.rightIndex = right;
    return pair;
}

NS_INLINE iTermStatusBarContainerView *iTermStatusBarStableLayoutNeighborView(NSArray<iTermStatusBarContainerView *> *views,
                                                                             NSInteger index) {
    if (index < 0) {
        return nil;
    }
    const NSUInteger unsignedIndex = (NSUInteger)index;
    if (unsignedIndex >= views.count) {
        return nil;
    }
    return views[unsignedIndex];
}

static iTermStatusBarStableLayoutNeighborPair iTermStatusBarStableLayoutNeighborPairByScanning(NSUInteger index,
                                                                                               NSArray<iTermStatusBarContainerView *> *views,
                                                                                               const iTermStatusBarStableLayoutRemovalMask *removalMask,
                                                                                               CFSetRef markedForRemoval) {
    iTermStatusBarStableLayoutNeighborPair pair = { .leftIndex = -1, .rightIndex = -1 };
    const NSUInteger count = views.count;
    if (index == NSNotFound || index >= count) {
        return pair;
    }

    NSInteger left = (NSInteger)index - 1;
    while (left >= 0) {
        iTermStatusBarContainerView *candidate = views[(NSUInteger)left];
        if (!iTermStatusBarStableLayoutRemovalMaskViewIsMarked(removalMask,
                                                               markedForRemoval,
                                                               (NSUInteger)left,
                                                               candidate)) {
            break;
        }
        left -= 1;
    }
    pair.leftIndex = left;

    NSInteger right = (NSInteger)index + 1;
    while ((NSUInteger)right < count) {
        iTermStatusBarContainerView *candidate = views[(NSUInteger)right];
        if (!iTermStatusBarStableLayoutRemovalMaskViewIsMarked(removalMask,
                                                               markedForRemoval,
                                                               (NSUInteger)right,
                                                               candidate)) {
            pair.rightIndex = right;
            break;
        }
        right += 1;
    }

    if ((NSUInteger)right >= count) {
        pair.rightIndex = -1;
    }

    return pair;
}

static iTermStatusBarStableLayoutNeighborPair iTermStatusBarStableLayoutNeighborPairForRemoval(NSUInteger index,
                                                                                               NSArray<iTermStatusBarContainerView *> *views,
                                                                                               CFSetRef markedForRemoval,
                                                                                               const iTermStatusBarStableLayoutRemovalMask *removalMask,
                                                                                               iTermStatusBarStableLayoutNeighborTable *neighborTable,
                                                                                               BOOL hasNeighborTable) {
    if (hasNeighborTable && neighborTable) {
        return iTermStatusBarStableLayoutNeighborTableRemoveIndex(neighborTable, index);
    }
    return iTermStatusBarStableLayoutNeighborPairByScanning(index, views, removalMask, markedForRemoval);
}

static inline NSComparisonResult iTermStatusBarStableLayoutComparePointers(iTermStatusBarContainerView *lhs,
                                                                          iTermStatusBarContainerView *rhs) {
    const uintptr_t lhsAddress = (uintptr_t)(__bridge const void *)(lhs);
    const uintptr_t rhsAddress = (uintptr_t)(__bridge const void *)(rhs);
    if (lhsAddress < rhsAddress) {
        return NSOrderedAscending;
    }
    if (lhsAddress > rhsAddress) {
        return NSOrderedDescending;
    }
    return NSOrderedSame;
}

typedef NSComparisonResult (*iTermStatusBarStableLayoutCandidateComparator)(const iTermStatusBarStableLayoutRemovalCandidate *lhs,
                                                                            const iTermStatusBarStableLayoutRemovalCandidate *rhs);

static NSComparisonResult iTermStatusBarStableLayoutCompareSpringCandidates(const iTermStatusBarStableLayoutRemovalCandidate *lhs,
                                                                            const iTermStatusBarStableLayoutRemovalCandidate *rhs) {
    if (lhs == rhs) {
        return NSOrderedSame;
    }
    if (lhs->priority < rhs->priority) {
        return NSOrderedAscending;
    }
    if (lhs->priority > rhs->priority) {
        return NSOrderedDescending;
    }
    if (lhs->originalIndex < rhs->originalIndex) {
        return NSOrderedAscending;
    }
    if (lhs->originalIndex > rhs->originalIndex) {
        return NSOrderedDescending;
    }
    return iTermStatusBarStableLayoutComparePointers(lhs->view, rhs->view);
}

static NSComparisonResult iTermStatusBarStableLayoutCompareNonSpacerMetadata(const iTermStatusBarStableLayoutRemovalCandidate *lhs,
                                                                            const iTermStatusBarStableLayoutRemovalCandidate *rhs) {
    if (lhs == rhs) {
        return NSOrderedSame;
    }
    if (lhs->priority < rhs->priority) {
        return NSOrderedAscending;
    }
    if (lhs->priority > rhs->priority) {
        return NSOrderedDescending;
    }
    if (lhs->minimumWidth > rhs->minimumWidth) {
        return NSOrderedAscending;
    }
    if (lhs->minimumWidth < rhs->minimumWidth) {
        return NSOrderedDescending;
    }
    if (lhs->originalIndex > rhs->originalIndex) {
        return NSOrderedAscending;
    }
    if (lhs->originalIndex < rhs->originalIndex) {
        return NSOrderedDescending;
    }
    return iTermStatusBarStableLayoutComparePointers(lhs->view, rhs->view);
}

static void iTermStatusBarStableLayoutSortCandidateRange(iTermStatusBarStableLayoutRemovalCandidate *candidates,
                                                         NSUInteger count,
                                                         iTermStatusBarStableLayoutCandidateComparator comparator) {
    if (count < 2 || !candidates || !comparator) {
        return;
    }
    for (NSUInteger index = 1; index < count; index++) {
        iTermStatusBarStableLayoutRemovalCandidate key = candidates[index];
        NSInteger previous = (NSInteger)index - 1;
        while (previous >= 0 && comparator(&candidates[previous], &key) == NSOrderedDescending) {
            candidates[previous + 1] = candidates[previous];
            previous -= 1;
        }
        candidates[previous + 1] = key;
    }
}

static iTermStatusBarStableLayoutRemovalCandidate *iTermStatusBarStableLayoutBorrowCandidateBuffer(NSUInteger count,
                                                                                                   iTermStatusBarStableLayoutScratchBuffers *scratch,
                                                                                                   BOOL *usingScratchOut) {
    if (usingScratchOut) {
        *usingScratchOut = NO;
    }
    if (count == 0) {
        return NULL;
    }
    if (scratch) {
        if (scratch->candidateCapacity < count) {
            const size_t bytes = sizeof(iTermStatusBarStableLayoutRemovalCandidate) * count;
            iTermStatusBarStableLayoutRemovalCandidate *newStorage = realloc(scratch->candidateStorage, bytes);
            if (newStorage) {
                scratch->candidateStorage = newStorage;
                scratch->candidateCapacity = count;
            }
        }
        if (scratch->candidateCapacity >= count && scratch->candidateStorage) {
            if (usingScratchOut) {
                *usingScratchOut = YES;
            }
            return scratch->candidateStorage;
        }
    }
    return malloc(sizeof(iTermStatusBarStableLayoutRemovalCandidate) * count);
}

static void iTermStatusBarStableLayoutSortWidthsDescending(CGFloat *values, NSUInteger count) {
    if (!values || count < 2) {
        return;
    }
    for (NSUInteger index = 1; index < count; index++) {
        const CGFloat value = values[index];
        NSInteger cursor = (NSInteger)index - 1;
        while (cursor >= 0 && values[cursor] < value) {
            values[cursor + 1] = values[cursor];
            cursor--;
        }
        values[cursor + 1] = value;
    }
}

static iTermStatusBarStableLayoutMinimumWidthBucket *iTermStatusBarStableLayoutBorrowMinimumWidthBuckets(NSUInteger count,
                                                                                                       iTermStatusBarStableLayoutScratchBuffers *scratch,
                                                                                                       BOOL *usesScratchStorageOut) {
    if (usesScratchStorageOut) {
        *usesScratchStorageOut = NO;
    }
    if (count == 0) {
        return NULL;
    }

    if (scratch) {
        if (scratch->minimumWidthBucketCapacity < count) {
            const size_t bytes = sizeof(iTermStatusBarStableLayoutMinimumWidthBucket) * count;
            iTermStatusBarStableLayoutMinimumWidthBucket *newStorage = realloc(scratch->minimumWidthBucketStorage, bytes);
            if (newStorage) {
                scratch->minimumWidthBucketStorage = newStorage;
                scratch->minimumWidthBucketCapacity = count;
            }
        }
        if (scratch->minimumWidthBucketCapacity >= count && scratch->minimumWidthBucketStorage) {
            if (usesScratchStorageOut) {
                *usesScratchStorageOut = YES;
            }
            return scratch->minimumWidthBucketStorage;
        }
    }

    return malloc(sizeof(iTermStatusBarStableLayoutMinimumWidthBucket) * count);
}

static void iTermStatusBarStableLayoutDestroyMinimumWidthHistogram(iTermStatusBarStableLayoutMinimumWidthHistogram *histogram) {
    if (!histogram) {
        return;
    }
    if (!histogram->usesScratchStorage) {
        free(histogram->buckets);
    }
    histogram->buckets = NULL;
    histogram->bucketCount = 0;
    histogram->currentIndex = 0;
    histogram->usesScratchStorage = NO;
}

static NSInteger iTermStatusBarStableLayoutBucketIndexForWidth(const iTermStatusBarStableLayoutMinimumWidthHistogram *histogram,
                                                               CGFloat width) {
    if (!histogram || !histogram->buckets || histogram->bucketCount == 0) {
        return -1;
    }
    NSUInteger low = 0;
    NSUInteger high = histogram->bucketCount;
    while (low < high) {
        const NSUInteger mid = low + (high - low) / 2;
        const CGFloat candidate = histogram->buckets[mid].width;
        if (width > candidate) {
            high = mid;
        } else if (width < candidate) {
            low = mid + 1;
        } else {
            return (NSInteger)mid;
        }
    }
    return -1;
}

static void iTermStatusBarStableLayoutMinimumWidthHistogramConsumeBucket(iTermStatusBarStableLayoutMinimumWidthHistogram *histogram,
                                                                         NSUInteger index) {
    if (!histogram || !histogram->buckets || index >= histogram->bucketCount) {
        return;
    }
    iTermStatusBarStableLayoutMinimumWidthBucket *bucket = &histogram->buckets[index];
    if (bucket->count > 0) {
        bucket->count -= 1;
    }
    if (index == histogram->currentIndex) {
        while (histogram->currentIndex < histogram->bucketCount &&
               histogram->buckets[histogram->currentIndex].count == 0) {
            histogram->currentIndex += 1;
        }
    }
}

static void iTermStatusBarStableLayoutMinimumWidthHistogramSyncMetrics(const iTermStatusBarStableLayoutMinimumWidthHistogram *histogram,
                                                                       iTermStatusBarStableLayoutMinimumWidthMetrics *metrics) {
    if (!histogram || !metrics) {
        return;
    }
    if (histogram->currentIndex >= histogram->bucketCount) {
        metrics->largestMinimumSize = 0;
        metrics->numberOfViewsAtLargestMinimum = 0;
        return;
    }
    const iTermStatusBarStableLayoutMinimumWidthBucket bucket = histogram->buckets[histogram->currentIndex];
    metrics->largestMinimumSize = bucket.width;
    metrics->numberOfViewsAtLargestMinimum = bucket.count;
}

static iTermStatusBarStableLayoutDistributionInfo *
iTermStatusBarStableLayoutPrepareDistributionInfo(iTermStatusBarStableLayoutViewSlice viewSlice,
                                                  iTermStatusBarStableLayoutAlgorithm *algorithm,
                                                  iTermStatusBarStableLayoutScratchBuffers *scratch,
                                                  NSUInteger *initialGrowableCount) {
    if (initialGrowableCount) {
        *initialGrowableCount = 0;
    }
    if (!algorithm || !scratch) {
        return NULL;
    }
    const NSUInteger count = viewSlice.count;
    if (count == 0) {
        return NULL;
    }
    if (scratch->distributionInfoCapacity < count) {
        const size_t bytes = sizeof(iTermStatusBarStableLayoutDistributionInfo) * count;
        iTermStatusBarStableLayoutDistributionInfo *newStorage = realloc(scratch->distributionInfoStorage, bytes);
        if (newStorage) {
            scratch->distributionInfoStorage = newStorage;
            scratch->distributionInfoCapacity = count;
        }
    }
    if (scratch->distributionInfoCapacity < count || !scratch->distributionInfoStorage) {
        return NULL;
    }
    iTermStatusBarStableLayoutDistributionInfo *info = scratch->distributionInfoStorage;
    NSUInteger growableCount = 0;
    for (NSUInteger index = 0; index < count; index++) {
        iTermStatusBarContainerView *view = viewSlice.items[index];
        id<iTermStatusBarComponent> component = view.component;
        const BOOL isFixedSpacer = [component isKindOfClass:[iTermStatusBarFixedSpacerComponent class]];
        info[index].isFixedSpacer = isFixedSpacer;
        info[index].springConstant = isFixedSpacer ? 0 : component.statusBarComponentSpringConstant;
        info[index].minSize = [algorithm minimumWidthForComponent:component];
        info[index].maxSize = [algorithm maximumWidthForComponent:component];
        info[index].canStillGrow = !isFixedSpacer;
        if (info[index].canStillGrow) {
            growableCount += 1;
        }
    }
    if (initialGrowableCount) {
        *initialGrowableCount = growableCount;
    }
    return info;
}

static void iTermStatusBarStableLayoutDestroyRemovalPlan(iTermStatusBarStableLayoutRemovalPlan *plan) {
    if (!plan) {
        return;
    }
    if (plan->orderedCandidates && !plan->usesScratchStorage) {
        free(plan->orderedCandidates);
    }
    plan->orderedCandidates = NULL;
    plan->count = 0;
    plan->usesScratchStorage = NO;
}

static BOOL iTermStatusBarStableLayoutBuildRemovalPlan(NSArray<iTermStatusBarContainerView *> *visibleViews,
                                                       iTermStatusBarStableLayoutRemovalPlan *plan,
                                                       iTermStatusBarStableLayoutScratchBuffers *scratch) {
    if (!plan) {
        return NO;
    }
    plan->orderedCandidates = NULL;
    plan->count = 0;
    plan->usesScratchStorage = NO;

    const NSUInteger count = visibleViews.count;
    if (count == 0) {
        return NO;
    }

    BOOL usingScratchCandidates = NO;
    iTermStatusBarStableLayoutRemovalCandidate *candidates = iTermStatusBarStableLayoutBorrowCandidateBuffer(count,
                                                                                                            scratch,
                                                                                                            &usingScratchCandidates);
    if (!candidates) {
        return NO;
    }

    NSUInteger springCount = 0;
    NSUInteger fixedSpacerCount = 0;
    for (iTermStatusBarContainerView *view in visibleViews) {
        if ([view.component isKindOfClass:[iTermStatusBarSpringComponent class]]) {
            springCount += 1;
            continue;
        }
        if (iTermStatusBarStableLayoutViewIsFixedSpacer(view)) {
            fixedSpacerCount += 1;
        }
    }
    const NSUInteger nonSpacerCount = count - springCount - fixedSpacerCount;

    iTermStatusBarStableLayoutRemovalCandidate *springCursor = candidates;
    iTermStatusBarStableLayoutRemovalCandidate *fixedCursor = candidates + springCount;
    iTermStatusBarStableLayoutRemovalCandidate *nonSpacerCursor = fixedCursor + fixedSpacerCount;

    NSUInteger index = 0;
    for (iTermStatusBarContainerView *view in visibleViews) {
        iTermStatusBarStableLayoutRemovalCandidate *slot = NULL;
        if ([view.component isKindOfClass:[iTermStatusBarSpringComponent class]]) {
            slot = springCursor++;
        } else if (iTermStatusBarStableLayoutViewIsFixedSpacer(view)) {
            slot = fixedCursor++;
        } else {
            slot = nonSpacerCursor++;
        }
        slot->view = view;
        slot->originalIndex = index;
        slot->priority = view.component.statusBarComponentPriority;
        slot->minimumWidth = view.minimumWidthIncludingIcon;
        index += 1;
    }

    iTermStatusBarStableLayoutSortCandidateRange(candidates, springCount, iTermStatusBarStableLayoutCompareSpringCandidates);
    iTermStatusBarStableLayoutSortCandidateRange(candidates + springCount,
                                                fixedSpacerCount,
                                                iTermStatusBarStableLayoutCompareSpringCandidates);
    iTermStatusBarStableLayoutSortCandidateRange(candidates + springCount + fixedSpacerCount,
                                                nonSpacerCount,
                                                iTermStatusBarStableLayoutCompareNonSpacerMetadata);

    plan->orderedCandidates = candidates;
    plan->count = count;
    plan->usesScratchStorage = usingScratchCandidates;
    return YES;
}

static iTermStatusBarContainerView *iTermStatusBarStableLayoutNextRemovalCandidate(const iTermStatusBarStableLayoutRemovalCandidate *orderedCandidates,
                                                                                  NSUInteger candidateCount,
                                                                                  NSUInteger *cursor,
                                                                                  CFMutableSetRef removedViews,
                                                                                  const iTermStatusBarStableLayoutRemovalMask *removalMask,
                                                                                  NSArray<iTermStatusBarContainerView *> *visibleContainerViews,
                                                                                  iTermStatusBarContainerView *viewToSkip,
                                                                                  NSUInteger *outIndex) {
    if (outIndex) {
        *outIndex = NSNotFound;
    }
    if (!orderedCandidates || candidateCount == 0) {
        return nil;
    }
    while (*cursor < candidateCount) {
        const NSUInteger position = *cursor;
        const iTermStatusBarStableLayoutRemovalCandidate candidateMetadata = orderedCandidates[position];
        iTermStatusBarContainerView *candidate = candidateMetadata.view;
        *cursor += 1;
        if (!candidate || candidate == viewToSkip) {
            continue;
        }
        NSUInteger candidateIndex = candidateMetadata.originalIndex;
        if (candidateIndex == NSNotFound || candidateIndex >= visibleContainerViews.count) {
            candidateIndex = iTermStatusBarStableLayoutIndexForView(candidate, visibleContainerViews);
        }
        if (iTermStatusBarStableLayoutRemovalMaskViewIsMarked(removalMask,
                                                              removedViews,
                                                              candidateIndex,
                                                              candidate)) {
            continue;
        }
        if (outIndex) {
            *outIndex = candidateIndex;
        }
        return candidate;
    }
    return nil;
}

NS_INLINE CGFloat iTermStatusBarStableLayoutRequiredWidth(iTermStatusBarStableLayoutMinimumWidthMetrics metrics) {
    return metrics.largestMinimumSize * metrics.numberOfNonFixedViews + metrics.widthOfAllFixedSpacers;
}

static void iTermStatusBarStableLayoutRecomputeLargestMinimum(NSArray<iTermStatusBarContainerView *> *views,
                                                              iTermStatusBarStableLayoutMinimumWidthMetrics *metrics) {
    if (metrics->numberOfNonFixedViews == 0) {
        metrics->largestMinimumSize = 0;
        metrics->numberOfViewsAtLargestMinimum = 0;
        return;
    }

    metrics->largestMinimumSize = 0;
    metrics->numberOfViewsAtLargestMinimum = 0;
    for (iTermStatusBarContainerView *candidate in views) {
        if (iTermStatusBarStableLayoutViewIsFixedSpacer(candidate)) {
            continue;
        }
        const CGFloat minimumWidth = candidate.minimumWidthIncludingIcon;
        if (metrics->numberOfViewsAtLargestMinimum == 0 || minimumWidth > metrics->largestMinimumSize) {
            metrics->largestMinimumSize = minimumWidth;
            metrics->numberOfViewsAtLargestMinimum = 1;
        } else if (minimumWidth == metrics->largestMinimumSize) {
            metrics->numberOfViewsAtLargestMinimum += 1;
        }
    }
}

static BOOL iTermStatusBarStableLayoutComputeMinimumWidthState(NSArray<iTermStatusBarContainerView *> *views,
                                                              iTermStatusBarStableLayoutMinimumWidthMetrics *metrics,
                                                              iTermStatusBarStableLayoutMinimumWidthHistogram *histogram,
                                                              iTermStatusBarStableLayoutScratchBuffers *scratch) {
    if (!metrics || !histogram) {
        return NO;
    }
    *metrics = (iTermStatusBarStableLayoutMinimumWidthMetrics){0, 0, 0, 0};
    *histogram = (iTermStatusBarStableLayoutMinimumWidthHistogram){0};

    const NSUInteger count = views.count;
    if (count == 0) {
        return YES;
    }

    CGFloat *minimumWidths = count > 0 ? (CGFloat *)alloca(sizeof(CGFloat) * count) : NULL;
    if (count > 0 && !minimumWidths) {
        return NO;
    }

    NSUInteger nonFixedCount = 0;
    for (iTermStatusBarContainerView *view in views) {
        const CGFloat minimumWidth = view.minimumWidthIncludingIcon;
        if (iTermStatusBarStableLayoutViewIsFixedSpacer(view)) {
            metrics->widthOfAllFixedSpacers += minimumWidth;
            continue;
        }
        minimumWidths[nonFixedCount++] = minimumWidth;
    }
    metrics->numberOfNonFixedViews = nonFixedCount;

    if (nonFixedCount == 0) {
        return YES;
    }

    iTermStatusBarStableLayoutSortWidthsDescending(minimumWidths, nonFixedCount);

    BOOL usingScratchBuckets = NO;
    iTermStatusBarStableLayoutMinimumWidthBucket *buckets = iTermStatusBarStableLayoutBorrowMinimumWidthBuckets(nonFixedCount,
                                                                                                               scratch,
                                                                                                               &usingScratchBuckets);
    if (!buckets) {
        return NO;
    }

    NSUInteger bucketIndex = 0;
    CGFloat currentWidth = minimumWidths[0];
    NSUInteger currentCount = 1;
    for (NSUInteger index = 1; index < nonFixedCount; index++) {
        const CGFloat width = minimumWidths[index];
        if (width == currentWidth) {
            currentCount += 1;
            continue;
        }
        buckets[bucketIndex].width = currentWidth;
        buckets[bucketIndex].count = currentCount;
        bucketIndex += 1;
        currentWidth = width;
        currentCount = 1;
    }
    buckets[bucketIndex].width = currentWidth;
    buckets[bucketIndex].count = currentCount;
    bucketIndex += 1;

    histogram->buckets = buckets;
    histogram->bucketCount = bucketIndex;
    histogram->currentIndex = 0;
    histogram->usesScratchStorage = usingScratchBuckets;

    metrics->largestMinimumSize = buckets[0].width;
    metrics->numberOfViewsAtLargestMinimum = buckets[0].count;
    return YES;
}

// Legacy function - kept for reference but replaced by iTermStatusBarStableLayoutUpdateMetricsForRemovalWithCount
__attribute__((unused))
static void iTermStatusBarStableLayoutUpdateMetricsForRemoval(iTermStatusBarContainerView *view,
                                                              NSArray<iTermStatusBarContainerView *> *currentViews,
                                                              iTermStatusBarStableLayoutMinimumWidthMetrics *metrics) {
    if (!view) {
        return;
    }
    if (iTermStatusBarStableLayoutViewIsFixedSpacer(view)) {
        metrics->widthOfAllFixedSpacers -= view.minimumWidthIncludingIcon;
        return;
    }
    if (metrics->numberOfNonFixedViews == 0) {
        return;
    }
    metrics->numberOfNonFixedViews -= 1;
    if (metrics->numberOfNonFixedViews == 0) {
        metrics->largestMinimumSize = 0;
        metrics->numberOfViewsAtLargestMinimum = 0;
        return;
    }

    const CGFloat minimumWidth = view.minimumWidthIncludingIcon;
    if (minimumWidth == metrics->largestMinimumSize) {
        if (metrics->numberOfViewsAtLargestMinimum > 0) {
            metrics->numberOfViewsAtLargestMinimum -= 1;
        }
        if (metrics->numberOfViewsAtLargestMinimum == 0) {
            iTermStatusBarStableLayoutRecomputeLargestMinimum(currentViews, metrics);
        }
    }
}

// Recompute the largest minimum width, skipping views that are marked for removal.
static void iTermStatusBarStableLayoutRecomputeLargestMinimumWithMarked(NSArray<iTermStatusBarContainerView *> *views,
                                                                         CFSetRef markedForRemoval,
                                                                         const iTermStatusBarStableLayoutRemovalMask *removalMask,
                                                                         iTermStatusBarStableLayoutMinimumWidthMetrics *metrics) {
    if (metrics->numberOfNonFixedViews == 0) {
        metrics->largestMinimumSize = 0;
        metrics->numberOfViewsAtLargestMinimum = 0;
        return;
    }

    metrics->largestMinimumSize = 0;
    metrics->numberOfViewsAtLargestMinimum = 0;
    const NSUInteger count = views.count;
    for (NSUInteger index = 0; index < count; index++) {
        iTermStatusBarContainerView *candidate = views[index];
        if (iTermStatusBarStableLayoutRemovalMaskViewIsMarked(removalMask,
                                                              markedForRemoval,
                                                              index,
                                                              candidate)) {
            continue;
        }
        if (iTermStatusBarStableLayoutViewIsFixedSpacer(candidate)) {
            continue;
        }
        const CGFloat minimumWidth = candidate.minimumWidthIncludingIcon;
        if (metrics->numberOfViewsAtLargestMinimum == 0 || minimumWidth > metrics->largestMinimumSize) {
            metrics->largestMinimumSize = minimumWidth;
            metrics->numberOfViewsAtLargestMinimum = 1;
        } else if (minimumWidth == metrics->largestMinimumSize) {
            metrics->numberOfViewsAtLargestMinimum += 1;
        }
    }
}

// Update metrics for view removal using the mark-and-sweep approach.
// Instead of iterating over a modified array, we skip marked views.
static void iTermStatusBarStableLayoutUpdateMetricsForRemovalWithCount(iTermStatusBarContainerView *view,
                                                                        NSArray<iTermStatusBarContainerView *> *allViews,
                                                                        CFSetRef markedForRemoval,
                                                                        const iTermStatusBarStableLayoutRemovalMask *removalMask,
                                                                        iTermStatusBarStableLayoutMinimumWidthHistogram *histogram,
                                                                        iTermStatusBarStableLayoutMinimumWidthMetrics *metrics) {
    if (!view) {
        return;
    }
    if (iTermStatusBarStableLayoutViewIsFixedSpacer(view)) {
        metrics->widthOfAllFixedSpacers -= view.minimumWidthIncludingIcon;
        return;
    }
    if (metrics->numberOfNonFixedViews == 0) {
        return;
    }
    metrics->numberOfNonFixedViews -= 1;
    if (metrics->numberOfNonFixedViews == 0) {
        metrics->largestMinimumSize = 0;
        metrics->numberOfViewsAtLargestMinimum = 0;
        if (histogram) {
            histogram->currentIndex = histogram->bucketCount;
        }
        return;
    }

    const CGFloat minimumWidth = view.minimumWidthIncludingIcon;

    if (histogram && histogram->bucketCount > 0) {
        const NSInteger bucketIndex = iTermStatusBarStableLayoutBucketIndexForWidth(histogram, minimumWidth);
        if (bucketIndex >= 0) {
            iTermStatusBarStableLayoutMinimumWidthHistogramConsumeBucket(histogram, (NSUInteger)bucketIndex);
            iTermStatusBarStableLayoutMinimumWidthHistogramSyncMetrics(histogram, metrics);
            return;
        }
        histogram->currentIndex = histogram->bucketCount;
    }

    if (minimumWidth == metrics->largestMinimumSize) {
        if (metrics->numberOfViewsAtLargestMinimum > 0) {
            metrics->numberOfViewsAtLargestMinimum -= 1;
        }
        if (metrics->numberOfViewsAtLargestMinimum == 0) {
            iTermStatusBarStableLayoutRecomputeLargestMinimumWithMarked(allViews,
                                                                        markedForRemoval,
                                                                        removalMask,
                                                                        metrics);
        }
    }
}

@implementation iTermStatusBarStableLayoutAlgorithm

- (void)dealloc {
    free(_scratchBuffers.removalMaskStorage);
    free(_scratchBuffers.neighborStorage);
    free(_scratchBuffers.candidateStorage);
    free(_scratchBuffers.distributionInfoStorage);
    free(_scratchBuffers.minimumWidthBucketStorage);
}

- (NSArray<iTermStatusBarContainerView *> *)allPossibleCandidateViews {
    return [self unhiddenContainerViews];
}

- (BOOL)componentIsSpacer:(id<iTermStatusBarComponent>)component {
    return ([component isKindOfClass:[iTermStatusBarSpringComponent class]] ||
            [component isKindOfClass:[iTermStatusBarFixedSpacerComponent class]]);
}

- (BOOL)views:(NSArray<iTermStatusBarContainerView *> *)views
haveSpacersOnBothSidesOfIndex:(NSInteger)index
         left:(out id<iTermStatusBarComponent>*)leftOut
        right:(out id<iTermStatusBarComponent>*)rightOut {
    if (index == 0) {
        return NO;
    }
    if (index + 1 == views.count) {
        return NO;
    }
    id<iTermStatusBarComponent> left = views[index - 1].component;
    id<iTermStatusBarComponent> right = views[index + 1].component;
    if (![self componentIsSpacer:left] || ![self componentIsSpacer:right]) {
        return NO;
    }
    *leftOut = left;
    *rightOut = right;
    return YES;
}

- (CGFloat)minimumWidthOfContainerViews:(NSArray<iTermStatusBarContainerView *> *)views {
    const NSUInteger count = views.count;
    if (count == 0) {
        return 0;
    }

    CGFloat largestMinimumSize = 0;
    CGFloat widthOfAllFixedSpacers = 0;
    NSUInteger numberOfNonFixedViews = 0;

    for (NSUInteger index = 0; index < count; index++) {
        iTermStatusBarContainerView *view = views[index];
        const CGFloat minimumWidth = view.minimumWidthIncludingIcon;
        if (minimumWidth > largestMinimumSize) {
            largestMinimumSize = minimumWidth;
        }
        if ([view.component isKindOfClass:[iTermStatusBarFixedSpacerComponent class]]) {
            widthOfAllFixedSpacers += minimumWidth;
            continue;
        }
        numberOfNonFixedViews += 1;
    }

    return largestMinimumSize * numberOfNonFixedViews + widthOfAllFixedSpacers;
}

- (NSArray<iTermStatusBarContainerView *> *)visibleContainerViewsAllowingEqualSpacingFromViews:(NSArray<iTermStatusBarContainerView *> *)visibleContainerViews {
    const NSUInteger viewCount = visibleContainerViews.count;
    if (viewCount == 0) {
        return @[];
    }

    // Compute the minimum-width metrics once and reuse them for the early exit and removal loop.
    iTermStatusBarStableLayoutMinimumWidthMetrics minimumWidthMetrics = {0, 0, 0, 0};
    iTermStatusBarStableLayoutMinimumWidthHistogram minimumWidthHistogram = {0};
    if (!iTermStatusBarStableLayoutComputeMinimumWidthState(visibleContainerViews,
                                                           &minimumWidthMetrics,
                                                           &minimumWidthHistogram,
                                                           &_scratchBuffers)) {
        return visibleContainerViews;
    }
    CGFloat requiredWidth = iTermStatusBarStableLayoutRequiredWidth(minimumWidthMetrics);
    if (_statusBarWidth >= requiredWidth) {
        iTermStatusBarStableLayoutDestroyMinimumWidthHistogram(&minimumWidthHistogram);
        return visibleContainerViews;
    }

    iTermStatusBarStableLayoutRemovalPlan removalPlan = {0};
    if (!iTermStatusBarStableLayoutBuildRemovalPlan(visibleContainerViews,
                                                    &removalPlan,
                                                    &_scratchBuffers)) {
        iTermStatusBarStableLayoutDestroyMinimumWidthHistogram(&minimumWidthHistogram);
        return visibleContainerViews;
    }

    // Use a "mark and sweep" approach with a compact removal mask. Fall back to a CFSet only if
    // allocation fails, mirroring the previous behavior without paying hashing costs in the hot path.
    iTermStatusBarStableLayoutRemovalMask removalMask = {0};
    const BOOL hasRemovalMask = iTermStatusBarStableLayoutRemovalMaskInit(&removalMask,
                                                                         viewCount,
                                                                         &_scratchBuffers);
    CFMutableSetRef markedForRemoval = NULL;
    if (!hasRemovalMask) {
        markedForRemoval = CFSetCreateMutable(kCFAllocatorDefault,
                                              (CFIndex)viewCount,
                                              &kCFTypeSetCallBacks);
        if (!markedForRemoval) {
            iTermStatusBarStableLayoutDestroyRemovalPlan(&removalPlan);
            iTermStatusBarStableLayoutDestroyMinimumWidthHistogram(&minimumWidthHistogram);
            return visibleContainerViews;
        }
    }
    iTermStatusBarStableLayoutNeighborTable neighborTable = {0};
    BOOL hasNeighborTable = iTermStatusBarStableLayoutNeighborTableInit(&neighborTable,
                                                                       viewCount,
                                                                       &_scratchBuffers);
    NSUInteger removalCursor = 0;
    NSUInteger activeViewCount = viewCount;

    iTermStatusBarContainerView *mandatoryView = self.mandatoryView;

    while (activeViewCount > 0) {
        if (_statusBarWidth >= requiredWidth) {
            break;
        }

        NSUInteger removalIndex = NSNotFound;
        iTermStatusBarContainerView *viewToRemove = iTermStatusBarStableLayoutNextRemovalCandidate(removalPlan.orderedCandidates,
                                                                                                 removalPlan.count,
                                                                                                 &removalCursor,
                                                                                                 markedForRemoval,
                                                                                                 hasRemovalMask ? &removalMask : NULL,
                                                                                                 visibleContainerViews,
                                                                                                 mandatoryView,
                                                                                                 &removalIndex);
        if (!viewToRemove) {
            break;
        }

        if ((removalIndex == NSNotFound || removalIndex >= viewCount) && hasNeighborTable) {
            iTermStatusBarStableLayoutNeighborTableDestroy(&neighborTable);
            hasNeighborTable = NO;
        }

        iTermStatusBarStableLayoutMarkRemoval(hasRemovalMask ? &removalMask : NULL,
                                              markedForRemoval,
                                              removalIndex,
                                              viewToRemove);
        activeViewCount--;

        iTermStatusBarStableLayoutNeighborPair neighborPair = { .leftIndex = -1, .rightIndex = -1 };
        if (removalIndex != NSNotFound && removalIndex < viewCount) {
            neighborPair = iTermStatusBarStableLayoutNeighborPairForRemoval(removalIndex,
                                                                            visibleContainerViews,
                                                                            markedForRemoval,
                                                                            hasRemovalMask ? &removalMask : NULL,
                                                                            hasNeighborTable ? &neighborTable : NULL,
                                                                            hasNeighborTable);
        }

        // Check for adjacent spacer to remove using the cached neighbor map when available.
        iTermStatusBarContainerView *adjacentViewToRemove = nil;
        NSUInteger adjacentIndex = NSNotFound;
        if (removalIndex != NSNotFound) {
            adjacentViewToRemove = [self viewToRemoveAdjacentToViewAtIndex:removalIndex
                                                               neighborPair:neighborPair
                                                                     inViews:visibleContainerViews
                                                              adjacentIndex:&adjacentIndex];
        }
        if (adjacentViewToRemove) {
            if (adjacentIndex == NSNotFound) {
                adjacentIndex = iTermStatusBarStableLayoutIndexForView(adjacentViewToRemove,
                                                                       visibleContainerViews);
            }
            iTermStatusBarStableLayoutMarkRemoval(hasRemovalMask ? &removalMask : NULL,
                                                  markedForRemoval,
                                                  adjacentIndex,
                                                  adjacentViewToRemove);
            activeViewCount--;
            if (hasNeighborTable && adjacentIndex != NSNotFound && adjacentIndex < viewCount) {
                (void)iTermStatusBarStableLayoutNeighborTableRemoveIndex(&neighborTable, adjacentIndex);
            }
            iTermStatusBarStableLayoutUpdateMetricsForRemovalWithCount(adjacentViewToRemove,
                                                                        visibleContainerViews,
                                                                        markedForRemoval,
                                                                        hasRemovalMask ? &removalMask : NULL,
                                                                        &minimumWidthHistogram,
                                                                        &minimumWidthMetrics);
        }

        // Update metrics for the removed view
        iTermStatusBarStableLayoutUpdateMetricsForRemovalWithCount(viewToRemove,
                                                                    visibleContainerViews,
                                                                    markedForRemoval,
                                                                    hasRemovalMask ? &removalMask : NULL,
                                                                    &minimumWidthHistogram,
                                                                    &minimumWidthMetrics);
        requiredWidth = iTermStatusBarStableLayoutRequiredWidth(minimumWidthMetrics);
    }

    // Build the result array in a single pass
    NSMutableArray<iTermStatusBarContainerView *> *result = [NSMutableArray arrayWithCapacity:activeViewCount];
    for (NSUInteger index = 0; index < viewCount; index++) {
        iTermStatusBarContainerView *view = visibleContainerViews[index];
        if (iTermStatusBarStableLayoutRemovalMaskViewIsMarked(hasRemovalMask ? &removalMask : NULL,
                                                              markedForRemoval,
                                                              index,
                                                              view)) {
            continue;
        }
        [result addObject:view];
    }

    iTermStatusBarStableLayoutDestroyRemovalPlan(&removalPlan);
    if (hasNeighborTable) {
        iTermStatusBarStableLayoutNeighborTableDestroy(&neighborTable);
    }
    if (hasRemovalMask) {
        iTermStatusBarStableLayoutRemovalMaskDestroy(&removalMask);
    }
    if (markedForRemoval) {
        CFRelease(markedForRemoval);
    }
    iTermStatusBarStableLayoutDestroyMinimumWidthHistogram(&minimumWidthHistogram);
    return result;
}

// views are sorted ascending by priority. First remove spacers regardless of priority, then remove
// views from lowest to highest priority.
- (iTermStatusBarContainerView *)bestViewToRemoveFrom:(NSArray<iTermStatusBarContainerView *> *)views
                                         skippingView:(nullable iTermStatusBarContainerView *)viewToSkip {
    const NSUInteger count = views.count;
    if (count == 0) {
        return nil;
    }

    iTermStatusBarContainerView *bestView = nil;
    NSInteger bestScore = NSIntegerMin;
    double bestPriority = 0;
    CGFloat bestWidth = 0;
    NSInteger bestIndex = -1;

    for (NSUInteger index = 0; index < count; index++) {
        iTermStatusBarContainerView *view = views[index];
        if (view == viewToSkip) {
            continue;
        }

        NSInteger score;
        if ([view.component isKindOfClass:[iTermStatusBarSpringComponent class]]) {
            score = 2;
        } else if ([view.component isKindOfClass:[iTermStatusBarFixedSpacerComponent class]]) {
            score = 1;
        } else {
            score = 0;
        }

        if (!bestView || score > bestScore) {
            bestView = view;
            bestScore = score;
            bestPriority = view.component.statusBarComponentPriority;
            bestWidth = view.minimumWidthIncludingIcon;
            bestIndex = (NSInteger)index;
            continue;
        }

        if (score < bestScore) {
            continue;
        }

        if (score != 0) {
            continue;
        }

        // Non-spacers tie-break by priority, then minimum width, then index.
        const double priority = view.component.statusBarComponentPriority;
        if (priority < bestPriority) {
            bestView = view;
            bestPriority = priority;
            bestWidth = view.minimumWidthIncludingIcon;
            bestIndex = (NSInteger)index;
            continue;
        }
        if (priority > bestPriority) {
            continue;
        }

        const CGFloat width = view.minimumWidthIncludingIcon;
        if (width > bestWidth) {
            bestView = view;
            bestWidth = width;
            bestIndex = (NSInteger)index;
            continue;
        }
        if (width < bestWidth) {
            continue;
        }

        if ((NSInteger)index > bestIndex) {
            bestView = view;
            bestIndex = (NSInteger)index;
        }
    }

    return bestView;
}

- (NSArray<iTermStatusBarContainerView *> *)visibleContainerViewsAllowingEqualSpacing {
    if (_statusBarWidth <= 0) {
        return @[];
    }
    return [self visibleContainerViewsAllowingEqualSpacingFromViews:[self allPossibleCandidateViews]];
}

// Determine if a view removal can also trim one of its adjacent spacers.
- (iTermStatusBarContainerView *)viewToRemoveAdjacentToViewAtIndex:(NSUInteger)index
                                                     neighborPair:(iTermStatusBarStableLayoutNeighborPair)neighborPair
                                                           inViews:(NSArray<iTermStatusBarContainerView *> *)views
                                                    adjacentIndex:(NSUInteger *)adjacentIndexOut {
    if (adjacentIndexOut) {
        *adjacentIndexOut = NSNotFound;
    }
    iTermStatusBarContainerView *leftNeighbor = iTermStatusBarStableLayoutNeighborView(views, neighborPair.leftIndex);
    iTermStatusBarContainerView *rightNeighbor = iTermStatusBarStableLayoutNeighborView(views, neighborPair.rightIndex);
    if (!leftNeighbor || !rightNeighbor) {
        return nil;
    }

    if (![self componentIsSpacer:leftNeighbor.component] || ![self componentIsSpacer:rightNeighbor.component]) {
        return nil;
    }

    const double leftSpring = leftNeighbor.component.statusBarComponentSpringConstant;
    const double rightSpring = rightNeighbor.component.statusBarComponentSpringConstant;
    if (leftSpring > rightSpring) {
        if (adjacentIndexOut && neighborPair.leftIndex >= 0) {
            *adjacentIndexOut = (NSUInteger)neighborPair.leftIndex;
        }
        return leftNeighbor;
    }
    if (leftSpring < rightSpring) {
        if (adjacentIndexOut && neighborPair.rightIndex >= 0) {
            *adjacentIndexOut = (NSUInteger)neighborPair.rightIndex;
        }
        return rightNeighbor;
    }
    if (index < views.count / 2) {
        if (adjacentIndexOut && neighborPair.rightIndex >= 0) {
            *adjacentIndexOut = (NSUInteger)neighborPair.rightIndex;
        }
        return rightNeighbor;
    }
    if (adjacentIndexOut && neighborPair.leftIndex >= 0) {
        *adjacentIndexOut = (NSUInteger)neighborPair.leftIndex;
    }
    return leftNeighbor;
}

- (CGFloat)preallocatedWidthInViewSlice:(iTermStatusBarStableLayoutViewSlice)nonFixedViews
                     preallocatedSlice:(iTermStatusBarStableLayoutViewSlice)preallocatedViews
                             fromWidth:(CGFloat)totalWidth {
    if (preallocatedViews.count == 0 || nonFixedViews.count == 0) {
        return 0;
    }
    const CGFloat singleUnitWidth = floor(totalWidth / nonFixedViews.count);
    CGFloat sumOfPreferredSizesOfPreallocatedViews = 0;
    for (NSUInteger index = 0; index < nonFixedViews.count; index++) {
        iTermStatusBarContainerView *view = nonFixedViews.items[index];
        const CGFloat clampedWidth = MAX(MIN([self maximumWidthForComponent:view.component],
                                             singleUnitWidth),
                                         view.minimumWidthIncludingIcon);
        sumOfPreferredSizesOfPreallocatedViews += clampedWidth;
    }
    sumOfPreferredSizesOfPreallocatedViews = round(sumOfPreferredSizesOfPreallocatedViews);
    if (sumOfPreferredSizesOfPreallocatedViews <= totalWidth) {
        CGFloat preallocation = 0;
        for (NSUInteger index = 0; index < preallocatedViews.count; index++) {
            iTermStatusBarContainerView *view = preallocatedViews.items[index];
            const CGFloat width = MAX(singleUnitWidth, view.minimumWidthIncludingIcon);
            view.desiredWidth = width;
            preallocation += width;
        }
        return preallocation;
    }

    for (NSUInteger index = 0; index < preallocatedViews.count; index++) {
        preallocatedViews.items[index].desiredWidth = 0;
    }

    CGFloat available = totalWidth;
    CGFloat numberOfGrowableViews = preallocatedViews.count;
    CGFloat preallocation = 0;
    while (round(available) > 0 && numberOfGrowableViews > 0 && available >= numberOfGrowableViews) {
        const CGFloat apportionment = floor(available / numberOfGrowableViews);
        numberOfGrowableViews = 0;
        for (NSUInteger index = 0; index < preallocatedViews.count; index++) {
            iTermStatusBarContainerView *view = preallocatedViews.items[index];
            const CGFloat maxWidth = MAX([self minimumWidthForComponent:view.component],
                                         [self maximumWidthForComponent:view.component]);
            const CGFloat oldWidth = view.desiredWidth;
            const CGFloat newWidth = MIN(maxWidth, oldWidth + apportionment);
            if (round(newWidth) == round(oldWidth)) {
                continue;
            }
            view.desiredWidth = newWidth;
            const CGFloat growth = newWidth - oldWidth;
            available -= growth;
            preallocation += growth;
            if (round(newWidth) < maxWidth) {
                numberOfGrowableViews += 1;
            }
        }
    }
    return preallocation;
}

- (void)updateDesiredWidthsForViews:(NSArray<iTermStatusBarContainerView *> *)views {
    [self updateMargins:views];
    const NSUInteger viewCount = views.count;
    __unsafe_unretained iTermStatusBarContainerView **nonFixedViews = viewCount > 0 ? (__unsafe_unretained iTermStatusBarContainerView **)alloca(sizeof(iTermStatusBarContainerView *) * viewCount) : NULL;
    __unsafe_unretained iTermStatusBarContainerView **nonPreallocatedViews = viewCount > 0 ? (__unsafe_unretained iTermStatusBarContainerView **)alloca(sizeof(iTermStatusBarContainerView *) * viewCount) : NULL;
    __unsafe_unretained iTermStatusBarContainerView **preallocatedViews = viewCount > 0 ? (__unsafe_unretained iTermStatusBarContainerView **)alloca(sizeof(iTermStatusBarContainerView *) * viewCount) : NULL;
    NSUInteger nonFixedCount = 0;
    NSUInteger nonPreallocatedCount = 0;
    NSUInteger preallocatedCount = 0;
    CGFloat widthOfAllFixedSpacers = 0;

    for (NSUInteger index = 0; index < viewCount; index++) {
        iTermStatusBarContainerView *view = views[index];
        const BOOL isFixedSpacer = [view.component isKindOfClass:[iTermStatusBarFixedSpacerComponent class]];
        if (isFixedSpacer) {
            widthOfAllFixedSpacers += view.minimumWidthIncludingIcon;
        } else if (nonFixedViews) {
            nonFixedViews[nonFixedCount++] = view;
        }

        const BOOL isPreallocated = (view.component.statusBarComponentPriority == INFINITY);
        if (!isPreallocated && nonPreallocatedViews) {
            nonPreallocatedViews[nonPreallocatedCount++] = view;
        } else if (!isFixedSpacer && preallocatedViews && isPreallocated) {
            preallocatedViews[preallocatedCount++] = view;
        }
    }

    iTermStatusBarStableLayoutViewSlice nonFixedSlice = { nonFixedViews, nonFixedCount };
    iTermStatusBarStableLayoutViewSlice nonPreallocatedSlice = { nonPreallocatedViews, nonPreallocatedCount };
    iTermStatusBarStableLayoutViewSlice preallocatedSlice = { preallocatedViews, preallocatedCount };

    const CGFloat totalMarginWidth = [self totalMarginWidthForViews:views];
    const CGFloat availableWidthBeforePreallocation = _statusBarWidth - totalMarginWidth - widthOfAllFixedSpacers;
    const CGFloat preallocatedWidth = [self preallocatedWidthInViewSlice:nonFixedSlice
                                                     preallocatedSlice:preallocatedSlice
                                                             fromWidth:availableWidthBeforePreallocation];
    CGFloat availableWidth = availableWidthBeforePreallocation - preallocatedWidth;

    for (NSUInteger index = 0; index < nonPreallocatedSlice.count; index++) {
        nonPreallocatedSlice.items[index].desiredWidth = 0;
    }

    NSUInteger growableCount = nonPreallocatedSlice.count;
    iTermStatusBarStableLayoutDistributionInfo *distributionInfo = iTermStatusBarStableLayoutPrepareDistributionInfo(nonPreallocatedSlice,
                                                                                                                     self,
                                                                                                                     &_scratchBuffers,
                                                                                                                     &growableCount);

    if (!distributionInfo && nonPreallocatedSlice.count > 0) {
        while (round(availableWidth) > 0 && nonPreallocatedSlice.count > 0) {
            BOOL changed = NO;
            availableWidth = [self distributeNonPreallocatedAvailableWidth:availableWidth
                                                                 viewSlice:&nonPreallocatedSlice
                                                                   changed:&changed];
            if (!changed || nonPreallocatedSlice.count == 0) {
                break;
            }
        }
        return;
    }

    BOOL hasProcessedAtLeastOnce = NO;
    while (round(availableWidth) > 0 && (growableCount > 0 || !hasProcessedAtLeastOnce)) {
        BOOL changed = NO;
        availableWidth = [self distributeNonPreallocatedAvailableWidth:availableWidth
                                                            viewSlice:nonPreallocatedSlice
                                                          viewMetadata:distributionInfo
                                                               changed:&changed
                                                remainingGrowableCount:&growableCount];
        hasProcessedAtLeastOnce = YES;
        if (!changed) {
            break;
        }
    }
}

- (CGFloat)distributeNonPreallocatedAvailableWidth:(CGFloat)availableWidth
                                         viewSlice:(iTermStatusBarStableLayoutViewSlice *)viewSlice
                                           changed:(out BOOL *)changed {
    if (changed) {
        *changed = NO;
    }
    if (!viewSlice || viewSlice->count == 0 || !viewSlice->items) {
        return availableWidth;
    }

    const NSUInteger count = viewSlice->count;
    typedef struct {
        double springConstant;
        CGFloat minSize;
        CGFloat maxSize;
        BOOL isFixedSpacer;
        BOOL canStillGrow;
    } ViewDistributionInfo;

    ViewDistributionInfo *infos = (ViewDistributionInfo *)alloca(count * sizeof(ViewDistributionInfo));
    double sumOfSpringConstants = 0;
    for (NSUInteger index = 0; index < count; index++) {
        iTermStatusBarContainerView *view = viewSlice->items[index];
        id<iTermStatusBarComponent> component = view.component;
        const BOOL isFixedSpacer = [component isKindOfClass:[iTermStatusBarFixedSpacerComponent class]];
        infos[index].isFixedSpacer = isFixedSpacer;
        infos[index].canStillGrow = !isFixedSpacer;
        if (isFixedSpacer) {
            infos[index].springConstant = 0;
            infos[index].minSize = view.minimumWidthIncludingIcon;
            infos[index].maxSize = infos[index].minSize;
        } else {
            const double springConstant = component.statusBarComponentSpringConstant;
            infos[index].springConstant = springConstant;
            infos[index].minSize = [self minimumWidthForComponent:component];
            infos[index].maxSize = [self maximumWidthForComponent:component];
            sumOfSpringConstants += springConstant;
        }
    }

    const CGFloat apportionment = sumOfSpringConstants > 0 ? availableWidth / sumOfSpringConstants : 0;
    CGFloat remainingWidth = availableWidth;
    DLog(@"updateDesiredWidthsForViews available=%@ apportionment=%@", @(availableWidth), @(apportionment));

    for (NSUInteger index = 0; index < count; index++) {
        iTermStatusBarContainerView *view = viewSlice->items[index];
        const CGFloat oldWidth = view.desiredWidth;
        CGFloat newWidth;
        if (infos[index].isFixedSpacer) {
            newWidth = infos[index].minSize;
            infos[index].canStillGrow = NO;
        } else {
            const CGFloat maxSize = infos[index].maxSize;
            const CGFloat minSize = infos[index].minSize;
            const CGFloat clampedMaximum = MAX(minSize, maxSize);
            newWidth = MIN(clampedMaximum, oldWidth + apportionment * infos[index].springConstant);
            if (oldWidth == 0) {
                newWidth = MAX(newWidth, minSize);
            }
            infos[index].canStillGrow = (newWidth < maxSize);
        }
        if (round(oldWidth) != round(newWidth) && changed) {
            *changed = YES;
        }
        view.desiredWidth = newWidth;
        remainingWidth -= (newWidth - oldWidth);
    }

    NSUInteger growableCount = 0;
    for (NSUInteger index = 0; index < count; index++) {
        if (infos[index].canStillGrow) {
            viewSlice->items[growableCount++] = viewSlice->items[index];
        }
    }
    viewSlice->count = growableCount;
    return remainingWidth;
}

- (CGFloat)distributeNonPreallocatedAvailableWidth:(CGFloat)availableWidth
                                         viewSlice:(iTermStatusBarStableLayoutViewSlice)viewSlice
                                      viewMetadata:(iTermStatusBarStableLayoutDistributionInfo *)metadata
                                           changed:(out BOOL *)changed
                            remainingGrowableCount:(NSUInteger *)remainingGrowableCountOut {
    if (changed) {
        *changed = NO;
    }
    if (remainingGrowableCountOut) {
        *remainingGrowableCountOut = 0;
    }
    const NSUInteger count = viewSlice.count;
    if (count == 0 || !metadata || !viewSlice.items) {
        return availableWidth;
    }

    double sumOfSpringConstants = 0;
    for (NSUInteger index = 0; index < count; index++) {
        const iTermStatusBarStableLayoutDistributionInfo info = metadata[index];
        if (info.isFixedSpacer || !info.canStillGrow) {
            continue;
        }
        sumOfSpringConstants += info.springConstant;
    }

    const CGFloat apportionment = sumOfSpringConstants > 0 ? availableWidth / sumOfSpringConstants : 0;
    CGFloat remainingWidth = availableWidth;
    NSUInteger growableCount = 0;

    for (NSUInteger index = 0; index < count; index++) {
        iTermStatusBarStableLayoutDistributionInfo *info = &metadata[index];
        iTermStatusBarContainerView *view = viewSlice.items[index];
        const CGFloat oldWidth = view.desiredWidth;
        CGFloat newWidth = oldWidth;

        if (info->isFixedSpacer) {
            newWidth = info->minSize;
            info->canStillGrow = NO;
        } else if (!info->canStillGrow) {
            newWidth = MAX(oldWidth, info->minSize);
        } else {
            const CGFloat minSize = info->minSize;
            const CGFloat maxSize = MAX(minSize, info->maxSize);
            newWidth = MIN(maxSize, oldWidth + (CGFloat)(apportionment * info->springConstant));
            if (oldWidth == 0) {
                newWidth = MAX(newWidth, minSize);
            }
            const BOOL stillGrowable = (newWidth < maxSize);
            info->canStillGrow = stillGrowable;
            if (stillGrowable) {
                growableCount += 1;
            }
        }

        if (round(oldWidth) != round(newWidth) && changed) {
            *changed = YES;
        }
        view.desiredWidth = newWidth;
        remainingWidth -= (newWidth - oldWidth);
    }

    if (remainingGrowableCountOut) {
        *remainingGrowableCountOut = growableCount;
    }
    return remainingWidth;
}

- (NSArray<iTermStatusBarContainerView *> *)visibleContainerViews {
    NSArray<iTermStatusBarContainerView *> *visibleContainerViews = [self visibleContainerViewsAllowingEqualSpacing];

    [self updateDesiredWidthsForViews:visibleContainerViews];
    [self makeWidthsAndOriginsIntegers:visibleContainerViews];
    return visibleContainerViews;
}

@end
