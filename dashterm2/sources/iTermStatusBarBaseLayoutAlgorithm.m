//
//  iTermStatusBarBaseLayoutAlgorithm.m
//  DashTerm2
//
//  Created by George Nachman on 1/20/19.
//

#import "iTermStatusBarBaseLayoutAlgorithm.h"

#import <CoreFoundation/CoreFoundation.h>
#import <stdint.h>
#import <string.h>

#import "DebugLogging.h"
#import "iTermStatusBarBaseComponent.h"
#import "iTermStatusBarComponent.h"
#import "iTermStatusBarContainerView.h"
#import "NSArray+iTerm.h"

@interface iTermStatusBarBaseLayoutAlgorithm () {
    CFMutableDictionaryRef _containerOrderLookup;
    NSMapTable<id<iTermStatusBarComponent>, NSNumber *> *_minimumWidthCache;
    NSMapTable<id<iTermStatusBarComponent>, NSNumber *> *_maximumWidthCache;
}
@end

typedef struct {
    CGFloat minimumWidth;
    CGFloat contribution;
    BOOL hasNonzeroContent;
    BOOL hasMargins;
} iTermStatusBarWidthComputation;

static inline iTermStatusBarWidthComputation
iTermStatusBarWidthComputationMake(iTermStatusBarBaseLayoutAlgorithm *algorithm, iTermStatusBarContainerView *view) {
    iTermStatusBarWidthComputation computation;
    const CGFloat componentMinimumWidth = [algorithm minimumWidthForComponent:view.component];
    const BOOL hasIcon = view.component.statusBarComponentIcon != nil;
    computation.hasNonzeroContent = hasIcon || componentMinimumWidth > 0;

    CGFloat minimumWidth = componentMinimumWidth;
    if (hasIcon) {
        minimumWidth += iTermStatusBarViewControllerIconWidth;
    }
    computation.minimumWidth = minimumWidth;

    const BOOL hasMargins = view.component.statusBarComponentHasMargins;
    computation.hasMargins = hasMargins;

    CGFloat contribution = minimumWidth;
    if (hasMargins) {
        contribution += iTermStatusBarViewControllerMargin / 2 + 1;
        contribution += iTermStatusBarViewControllerMargin / 2;
    }
    computation.contribution = contribution;

    return computation;
}

static NSInteger iTermStatusBarLastMarginIndexInViews(NSArray<iTermStatusBarContainerView *> *views) {
    for (NSInteger index = (NSInteger)views.count - 1; index >= 0; index--) {
        if (views[index].component.statusBarComponentHasMargins) {
            return index;
        }
    }
    return NSNotFound;
}

static inline CGFloat iTermStatusBarTrailingMarginBonus(NSInteger lastMarginIndex) {
    return lastMarginIndex == NSNotFound ? 0 : iTermStatusBarViewControllerMargin / 2;
}

static const CFDictionaryValueCallBacks iTermStatusBarIndexValueCallbacks = {0, NULL, NULL, NULL, NULL};

static inline const void *iTermStatusBarIndexDictionaryValue(NSUInteger index) {
    return (const void *)(uintptr_t)(index + 1);
}

static inline NSUInteger iTermStatusBarIndexFromDictionaryValue(const void *value) {
    return (NSUInteger)((uintptr_t)value - 1);
}

static CFMutableDictionaryRef iTermStatusBarCreateContainerOrderLookup(NSArray<iTermStatusBarContainerView *> *views) {
    const NSUInteger count = views.count;
    if (count == 0) {
        return NULL;
    }
    CFMutableDictionaryRef dictionary = CFDictionaryCreateMutable(
        kCFAllocatorDefault, (CFIndex)count, &kCFTypeDictionaryKeyCallBacks, &iTermStatusBarIndexValueCallbacks);
    if (!dictionary) {
        return NULL;
    }
    for (NSUInteger idx = 0; idx < count; idx++) {
        iTermStatusBarContainerView *view = views[idx];
        CFDictionarySetValue(dictionary, (__bridge const void *)(view), iTermStatusBarIndexDictionaryValue(idx));
    }
    return dictionary;
}

static void iTermStatusBarRemoveViewFromPrioritized(iTermStatusBarContainerView *view,
                                                    NSMutableArray<iTermStatusBarContainerView *> *prioritized,
                                                    CFMutableDictionaryRef prioritizedIndexes,
                                                    NSMutableData *widthContributions, CGFloat *baseWidth,
                                                    NSInteger *trailingMarginIndex) {
    if (!prioritizedIndexes) {
        return;
    }
    const void *indexValue = CFDictionaryGetValue(prioritizedIndexes, (__bridge const void *)(view));
    if (!indexValue) {
        return;
    }
    const NSUInteger prioritizedIndex = iTermStatusBarIndexFromDictionaryValue(indexValue);
    const NSUInteger contributionCount = widthContributions.length / sizeof(CGFloat);
    // BUG-f856: Replace NSCAssert with graceful early return to prevent crash
    if (prioritized.count != contributionCount) {
        DLog(@"BUG-f856: Contribution buffer out of sync with prioritized views (%lu vs %lu)",
             (unsigned long)prioritized.count, (unsigned long)contributionCount);
        return;
    }
    CGFloat *contributions = widthContributions.mutableBytes;
    *baseWidth -= contributions[prioritizedIndex];

    const NSUInteger trailingCount = contributionCount - prioritizedIndex - 1;
    if (trailingCount > 0) {
        memmove(contributions + prioritizedIndex, contributions + prioritizedIndex + 1,
                trailingCount * sizeof(CGFloat));
    }
    widthContributions.length -= sizeof(CGFloat);

    // Update indexes for views that shift down
    for (NSUInteger i = prioritizedIndex + 1; i < contributionCount; i++) {
        iTermStatusBarContainerView *shiftedView = prioritized[i];
        CFDictionarySetValue(prioritizedIndexes, (__bridge const void *)(shiftedView),
                             iTermStatusBarIndexDictionaryValue(i - 1));
    }
    CFDictionaryRemoveValue(prioritizedIndexes, (__bridge const void *)(view));
    [prioritized removeObjectAtIndex:prioritizedIndex];

    if (*trailingMarginIndex == (NSInteger)prioritizedIndex) {
        *trailingMarginIndex = iTermStatusBarLastMarginIndexInViews(prioritized);
    } else if (*trailingMarginIndex != NSNotFound && *trailingMarginIndex > (NSInteger)prioritizedIndex) {
        *trailingMarginIndex -= 1;
    }
}

static void iTermStatusBarRemoveViewFromOrderedSubset(iTermStatusBarContainerView *view,
                                                      NSMutableArray<iTermStatusBarContainerView *> *orderedSubset,
                                                      CFMutableDictionaryRef orderedIndexes) {
    if (!view || !orderedSubset) {
        return;
    }

    if (!orderedIndexes) {
        const NSUInteger orderedIndex = [orderedSubset indexOfObjectIdenticalTo:view];
        // BUG-f857: Replace NSCAssert with graceful return to prevent crash
        if (orderedIndex == NSNotFound) {
            DLog(@"BUG-f857: View not found in ordered subset - may be out of sync");
            return;
        }
        [orderedSubset removeObjectAtIndex:orderedIndex];
        return;
    }

    const void *indexValue = CFDictionaryGetValue(orderedIndexes, (__bridge const void *)(view));
    if (!indexValue) {
        return;
    }
    const NSUInteger orderedIndex = iTermStatusBarIndexFromDictionaryValue(indexValue);
    const NSUInteger orderedCount = orderedSubset.count;
    // BUG-f858: Replace NSCAssert with graceful early return to prevent crash
    if (orderedIndex >= orderedCount) {
        DLog(@"BUG-f858: Ordered index %lu >= count %lu - subset out of sync", (unsigned long)orderedIndex,
             (unsigned long)orderedCount);
        CFDictionaryRemoveValue(orderedIndexes, (__bridge const void *)(view));
        return;
    }

    for (NSUInteger i = orderedIndex + 1; i < orderedCount; i++) {
        iTermStatusBarContainerView *shiftedView = orderedSubset[i];
        CFDictionarySetValue(orderedIndexes, (__bridge const void *)(shiftedView),
                             iTermStatusBarIndexDictionaryValue(i - 1));
    }
    CFDictionaryRemoveValue(orderedIndexes, (__bridge const void *)(view));
    [orderedSubset removeObjectAtIndex:orderedIndex];
}

static iTermStatusBarContainerView *
iTermStatusBarPopNextNonzeroCandidate(NSMutableArray<iTermStatusBarContainerView *> *candidateStack,
                                      CFMutableDictionaryRef prioritizedIndexes) {
    while (candidateStack.count > 0) {
        iTermStatusBarContainerView *candidate = candidateStack.lastObject;
        [candidateStack removeLastObject];
        if (!prioritizedIndexes) {
            return candidate;
        }
        // The prioritized index dictionary is the source of truth for whether the view still exists.
        const void *indexValue = CFDictionaryGetValue(prioritizedIndexes, (__bridge const void *)(candidate));
        if (indexValue) {
            return candidate;
        }
    }
    return nil;
}

@implementation iTermStatusBarBaseLayoutAlgorithm

- (instancetype)initWithContainerViews:(NSArray<iTermStatusBarContainerView *> *)containerViews
                         mandatoryView:(nonnull iTermStatusBarContainerView *)mandatoryView
                        statusBarWidth:(CGFloat)statusBarWidth
                 removeEmptyComponents:(BOOL)removeEmptyComponents {
    self = [super initWithContainerViews:containerViews
                           mandatoryView:mandatoryView
                          statusBarWidth:statusBarWidth
                   removeEmptyComponents:removeEmptyComponents];
    if (self) {
        _statusBarWidth = statusBarWidth;
        _containerViews = [containerViews copy];
        _containerOrderLookup = iTermStatusBarCreateContainerOrderLookup(_containerViews);
        _minimumWidthCache = [NSMapTable weakToStrongObjectsMapTable];
        _maximumWidthCache = [NSMapTable weakToStrongObjectsMapTable];
    }
    return self;
}

- (void)dealloc {
    if (_containerOrderLookup) {
        CFRelease(_containerOrderLookup);
        _containerOrderLookup = NULL;
    }
}

- (CGFloat)totalMarginWidthForViews:(NSArray<iTermStatusBarContainerView *> *)views {
    CGFloat totalMarginWidth = 0;
    for (iTermStatusBarContainerView *view in views) {
        totalMarginWidth += view.leftMargin + view.rightMargin;
    }
    return totalMarginWidth;
}

- (CGFloat)availableWidthAfterInitializingDesiredWidthForViews:(NSArray<iTermStatusBarContainerView *> *)views {
    const CGFloat totalMarginWidth = [self totalMarginWidthForViews:views];
    __block CGFloat availableWidth = _statusBarWidth - totalMarginWidth;
    DLog(@"availableWidthAfterInitializingDesiredWidthForViews available=%@", @(availableWidth));
    // Allocate minimum widths
    const NSUInteger count = views.count;
    for (NSUInteger idx = 0; idx < count; idx++) {
        iTermStatusBarContainerView *view = views[idx];
        id<iTermStatusBarComponent> component = view.component;
        CGFloat desiredWidth = [self minimumWidthForComponent:component];
        if (component.statusBarComponentIcon) {
            desiredWidth += iTermStatusBarViewControllerIconWidth;
        }
        view.desiredWidth = desiredWidth;
        availableWidth -= desiredWidth;
    }
    DLog(@"availableWidthAfterInitializingDesiredWidthForViews after assigning minimums: available=%@",
         @(availableWidth));
    return availableWidth;
}

- (NSArray<iTermStatusBarContainerView *> *)viewsThatCanGrowFromViews:(NSArray<iTermStatusBarContainerView *> *)views {
    const NSUInteger count = views.count;
    if (count == 0) {
        return @[];
    }

    NSMutableArray<iTermStatusBarContainerView *> *growable = nil;
    NSUInteger retainedPrefixCount = 0;
    BOOL removedAtLeastOne = NO;
    BOOL foundGrowableView = NO;

    for (NSUInteger idx = 0; idx < count; idx++) {
        iTermStatusBarContainerView *view = views[idx];
        double preferredWidth = view.component.statusBarComponentPreferredWidth;
        if (view.component.statusBarComponentIcon) {
            preferredWidth += iTermStatusBarViewControllerIconWidth;
        }
        const BOOL canGrow = ([view.component statusBarComponentCanStretch] &&
                              view.desiredWidth < [self maximumWidthForComponent:view.component] &&
                              floor(preferredWidth) > floor(view.desiredWidth));
        if (canGrow) {
            foundGrowableView = YES;
            if (removedAtLeastOne) {
                [growable addObject:view];
            } else {
                retainedPrefixCount++;
            }
            continue;
        }

        if (!removedAtLeastOne) {
            removedAtLeastOne = YES;
            growable = [NSMutableArray arrayWithCapacity:count - idx];
            if (retainedPrefixCount > 0) {
                for (NSUInteger prefix = 0; prefix < retainedPrefixCount; prefix++) {
                    [growable addObject:views[prefix]];
                }
            }
        }
    }

    if (!foundGrowableView) {
        return @[];
    }
    if (!removedAtLeastOne) {
        return views;
    }
    return growable;
}

- (double)sumOfSpringConstantsInViews:(NSArray<iTermStatusBarContainerView *> *)views {
    double sum = 0;
    for (iTermStatusBarContainerView *containerView in views) {
        if (![containerView.component statusBarComponentCanStretch]) {
            continue;
        }
        sum += containerView.component.statusBarComponentSpringConstant;
    }
    return sum;
}

- (NSArray<iTermStatusBarContainerView *> *)viewsByRemovingViewThatCannotGrow:
    (NSArray<iTermStatusBarContainerView *> *)views {
    const NSUInteger count = views.count;
    if (count == 0) {
        return @[];
    }

    NSMutableArray<iTermStatusBarContainerView *> *unsatisfiedViews = nil;
    NSUInteger retainedPrefixCount = 0;
    BOOL removedSatisfiedView = NO;
    BOOL hasUnsatisfiedView = NO;

    for (NSUInteger idx = 0; idx < count; idx++) {
        iTermStatusBarContainerView *view = views[idx];
        double preferredWidth = view.component.statusBarComponentPreferredWidth;
        if (view.component.statusBarComponentIcon) {
            preferredWidth += iTermStatusBarViewControllerIconWidth;
        }
        const BOOL unsatisfied = floor(preferredWidth) > ceil(view.desiredWidth);
        if (unsatisfied) {
            hasUnsatisfiedView = YES;
            DLog(@"%@ unsatisfied prefers=%@ allocated=%@", view.component.class,
                 @(view.component.statusBarComponentPreferredWidth), @(view.desiredWidth));
            if (removedSatisfiedView) {
                [unsatisfiedViews addObject:view];
            } else {
                retainedPrefixCount++;
            }
            continue;
        }

        if (!removedSatisfiedView) {
            removedSatisfiedView = YES;
            unsatisfiedViews = [NSMutableArray arrayWithCapacity:count - idx];
            if (retainedPrefixCount > 0) {
                for (NSUInteger prefix = 0; prefix < retainedPrefixCount; prefix++) {
                    [unsatisfiedViews addObject:views[prefix]];
                }
            }
        }
    }

    if (!hasUnsatisfiedView) {
        return @[];
    }
    if (!removedSatisfiedView) {
        return views;
    }
    return unsatisfiedViews;
}

- (double)totalGrowthAfterUpdatingDesiredWidthsForAvailableWidth:(CGFloat)availableWidth
                                            sumOfSpringConstants:(double)sumOfSpringConstants
                                                           views:(NSArray<iTermStatusBarContainerView *> *)views {
    return 0;
}

- (void)updateDesiredWidthsForViews:(NSArray<iTermStatusBarContainerView *> *)allViews {
    [self updateMargins:allViews];
    CGFloat availableWidth = [self availableWidthAfterInitializingDesiredWidthForViews:allViews];

    if (availableWidth < 1) {
        return;
    }

    // Find views that can grow
    NSArray<iTermStatusBarContainerView *> *views = [self viewsThatCanGrowFromViews:allViews];

    while (views.count) {
        const double sumOfSpringConstants = [self sumOfSpringConstantsInViews:views];

        DLog(@"updateDesiredWidths have %@ views that can grow: available=%@", @(views.count), @(availableWidth));

        const double growth = [self totalGrowthAfterUpdatingDesiredWidthsForAvailableWidth:availableWidth
                                                                      sumOfSpringConstants:sumOfSpringConstants
                                                                                     views:views];
        availableWidth -= growth;
        DLog(@"updateDesiredWidths after divvying: available = %@", @(availableWidth));

        if (availableWidth < 1) {
            return;
        }

        const NSInteger numberBefore = views.count;
        // Remove satisfied views.
        views = [self viewsByRemovingViewThatCannotGrow:views];

        if (growth < 1 && views.count == numberBefore) {
            DLog(@"Stopping. growth=%@ views %@->%@", @(growth), @(views.count), @(numberBefore));
            return;
        }
    }
}

- (NSArray<iTermStatusBarContainerView *> *)unhiddenContainerViews {
    if (!self.removeEmptyComponents) {
        return _containerViews;
    }

    const NSUInteger count = _containerViews.count;
    if (count == 0) {
        return @[];
    }

    NSMutableArray<iTermStatusBarContainerView *> *visibleViews = nil;
    NSUInteger retainedPrefixCount = 0;
    BOOL removedHiddenView = NO;

    for (NSUInteger idx = 0; idx < count; idx++) {
        iTermStatusBarContainerView *view = _containerViews[idx];
        if (!view.component.statusBarComponentIsEmpty) {
            if (removedHiddenView) {
                [visibleViews addObject:view];
            } else {
                retainedPrefixCount++;
            }
            continue;
        }

        if (!removedHiddenView) {
            removedHiddenView = YES;
            visibleViews = [NSMutableArray arrayWithCapacity:count - idx];
            if (retainedPrefixCount > 0) {
                for (NSUInteger prefix = 0; prefix < retainedPrefixCount; prefix++) {
                    [visibleViews addObject:_containerViews[prefix]];
                }
            }
        }
    }

    if (!removedHiddenView) {
        return _containerViews;
    }
    return visibleViews;
}

- (NSArray<iTermStatusBarContainerView *> *)containerViewsSortedByPriority:
    (NSArray<iTermStatusBarContainerView *> *)eligibleContainerViews {
    const NSUInteger count = eligibleContainerViews.count;
    if (count < 2) {
        return eligibleContainerViews;
    }

    iTermStatusBarContainerView *mandatoryView = self.mandatoryView;
    NSMutableArray<iTermStatusBarContainerView *> *prioritized = eligibleContainerViews.mutableCopy;

    CFDictionaryRef originalIndexes = _containerOrderLookup;

    [prioritized sortUsingComparator:^NSComparisonResult(iTermStatusBarContainerView *_Nonnull obj1,
                                                         iTermStatusBarContainerView *_Nonnull obj2) {
        if (obj1 == obj2) {
            return NSOrderedSame;
        }
        if (obj1 == mandatoryView) {
            return NSOrderedDescending;
        }
        if (obj2 == mandatoryView) {
            return NSOrderedAscending;
        }

        const double priority1 = obj1.component.statusBarComponentPriority;
        const double priority2 = obj2.component.statusBarComponentPriority;
        if (priority1 < priority2) {
            return NSOrderedAscending;
        }
        if (priority1 > priority2) {
            return NSOrderedDescending;
        }

        const void *indexValueOne =
            originalIndexes ? CFDictionaryGetValue(originalIndexes, (__bridge const void *)(obj1)) : NULL;
        const void *indexValueTwo =
            originalIndexes ? CFDictionaryGetValue(originalIndexes, (__bridge const void *)(obj2)) : NULL;
        const NSUInteger index1 = indexValueOne ? iTermStatusBarIndexFromDictionaryValue(indexValueOne) : NSNotFound;
        const NSUInteger index2 = indexValueTwo ? iTermStatusBarIndexFromDictionaryValue(indexValueTwo) : NSNotFound;
        if (index1 < index2) {
            return NSOrderedAscending;
        }
        if (index1 > index2) {
            return NSOrderedDescending;
        }
        return NSOrderedSame;
    }];
    return prioritized;
}

- (void)updateMargins:(NSArray<iTermStatusBarContainerView *> *)views {
    const NSUInteger count = views.count;
    BOOL foundMargin = NO;
    for (NSUInteger idx = 0; idx < count; idx++) {
        iTermStatusBarContainerView *view = views[idx];
        id<iTermStatusBarComponent> component = view.component;
        if (component.statusBarComponentHasMargins) {
            view.leftMargin = iTermStatusBarViewControllerMargin / 2 + 1;
        } else {
            view.leftMargin = 0;
        }
    }

    foundMargin = NO;
    for (NSInteger idx = (NSInteger)count - 1; idx >= 0; idx--) {
        iTermStatusBarContainerView *view = views[(NSUInteger)idx];
        id<iTermStatusBarComponent> component = view.component;
        const BOOL hasMargins = component.statusBarComponentHasMargins;
        if (hasMargins && !foundMargin) {
            view.rightMargin = iTermStatusBarViewControllerMargin;
            foundMargin = YES;
        } else if (hasMargins) {
            view.rightMargin = iTermStatusBarViewControllerMargin / 2;
        } else {
            view.rightMargin = 0;
        }
    }
}

- (CGFloat)minimumWidthOfContainerViews:(NSArray<iTermStatusBarContainerView *> *)views {
    CGFloat sumOfMinimumWidths = 0;
    NSInteger trailingMarginIndex = NSNotFound;
    for (NSUInteger index = 0; index < views.count; index++) {
        iTermStatusBarContainerView *containerView = views[index];
        const iTermStatusBarWidthComputation computation = iTermStatusBarWidthComputationMake(self, containerView);
        DLog(@"Minimum width of %@ is %@", containerView.component.class, @(computation.minimumWidth));
        sumOfMinimumWidths += computation.contribution;
        if (computation.hasMargins) {
            trailingMarginIndex = (NSInteger)index;
        }
    }
    return sumOfMinimumWidths + iTermStatusBarTrailingMarginBonus(trailingMarginIndex);
}

- (iTermStatusBarContainerView *)viewToRemoveAdjacentToViewBeingRemoved:(iTermStatusBarContainerView *)view
                                                              fromViews:
                                                                  (NSArray<iTermStatusBarContainerView *> *)views {
    return nil;
}

- (NSArray<iTermStatusBarContainerView *> *)viewsFrom:(NSArray<iTermStatusBarContainerView *> *)allowedViewsSubset
                                       keepingOrderIn:(NSArray<iTermStatusBarContainerView *> *)orderedViewsSuperset {
    if (allowedViewsSubset.count == 0 || orderedViewsSuperset.count == 0) {
        return @[];
    }
    if (allowedViewsSubset == orderedViewsSuperset) {
        return allowedViewsSubset;
    }
    NSMutableSet<iTermStatusBarContainerView *> *allowedSet = [NSMutableSet setWithCapacity:allowedViewsSubset.count];
    for (iTermStatusBarContainerView *view in allowedViewsSubset) {
        [allowedSet addObject:view];
    }
    if (allowedSet.count == 0) {
        return @[];
    }
    NSMutableArray<iTermStatusBarContainerView *> *orderedSubset =
        [NSMutableArray arrayWithCapacity:allowedViewsSubset.count];
    for (iTermStatusBarContainerView *view in orderedViewsSuperset) {
        if ([allowedSet containsObject:view]) {
            [orderedSubset addObject:view];
        }
    }
    return orderedSubset;
}

// Returns a subset of views by removing the lowest priority item until their minimum sizes all fit within the status
// bar's width.
- (NSArray<iTermStatusBarContainerView *> *)fittingSubsetOfContainerViewsFrom:
    (NSArray<iTermStatusBarContainerView *> *)views {
    const CGFloat allowedWidth = _statusBarWidth;
    if (allowedWidth < iTermStatusBarViewControllerMargin * 2) {
        return @[];
    }

    NSMutableArray<iTermStatusBarContainerView *> *prioritized =
        [self containerViewsSortedByPriority:views].mutableCopy;
    NSMutableArray<iTermStatusBarContainerView *> *prioritizedNonzeroRemovalStack =
        [NSMutableArray arrayWithCapacity:prioritized.count];

    NSMutableArray<iTermStatusBarContainerView *> *orderedPrioritized = [[self viewsFrom:prioritized
                                                                          keepingOrderIn:_containerViews] mutableCopy];
    if (!orderedPrioritized) {
        orderedPrioritized = [NSMutableArray arrayWithCapacity:prioritized.count];
    }
    CFMutableDictionaryRef orderedSubsetIndexes = NULL;
    const NSUInteger orderedSubsetCount = orderedPrioritized.count;
    if (orderedSubsetCount > 0) {
        orderedSubsetIndexes =
            CFDictionaryCreateMutable(kCFAllocatorDefault, (CFIndex)orderedSubsetCount, &kCFTypeDictionaryKeyCallBacks,
                                      &iTermStatusBarIndexValueCallbacks);
        for (NSUInteger index = 0; index < orderedSubsetCount; index++) {
            iTermStatusBarContainerView *orderedView = orderedPrioritized[index];
            CFDictionarySetValue(orderedSubsetIndexes, (__bridge const void *)(orderedView),
                                 iTermStatusBarIndexDictionaryValue(index));
        }
    }

    const NSUInteger prioritizedCount = prioritized.count;
    NSMutableData *widthContributions = [NSMutableData dataWithLength:prioritizedCount * sizeof(CGFloat)];
    CGFloat *widthContributionValues = widthContributions.mutableBytes;
    CFMutableDictionaryRef prioritizedIndexes = NULL;
    NSMutableData *nonzeroMinimumFlags =
        prioritizedCount > 0 ? [NSMutableData dataWithLength:prioritizedCount * sizeof(uint8_t)] : nil;
    uint8_t *nonzeroMinimumBytes = (uint8_t *)nonzeroMinimumFlags.mutableBytes;
    if (prioritizedCount > 0) {
        prioritizedIndexes =
            CFDictionaryCreateMutable(kCFAllocatorDefault, (CFIndex)prioritizedCount, &kCFTypeDictionaryKeyCallBacks,
                                      &iTermStatusBarIndexValueCallbacks);
    }
    CGFloat baseWidth = 0;
    NSInteger trailingMarginIndex = NSNotFound;
    for (NSUInteger index = 0; index < prioritizedCount; index++) {
        iTermStatusBarContainerView *containerView = prioritized[index];
        const iTermStatusBarWidthComputation computation = iTermStatusBarWidthComputationMake(self, containerView);
        widthContributionValues[index] = computation.contribution;
        if (prioritizedIndexes) {
            CFDictionarySetValue(prioritizedIndexes, (__bridge const void *)(containerView),
                                 iTermStatusBarIndexDictionaryValue(index));
        }
        if (nonzeroMinimumBytes) {
            nonzeroMinimumBytes[index] = computation.hasNonzeroContent;
        }
        baseWidth += computation.contribution;
        if (computation.hasMargins) {
            trailingMarginIndex = (NSInteger)index;
        }
    }

    if (prioritizedCount > 0 && nonzeroMinimumBytes) {
        for (NSInteger index = (NSInteger)prioritizedCount - 1; index >= 0; index--) {
            if (!nonzeroMinimumBytes[index]) {
                continue;
            }
            [prioritizedNonzeroRemovalStack addObject:prioritized[(NSUInteger)index]];
        }
    }

    CGFloat desiredWidth = baseWidth + iTermStatusBarTrailingMarginBonus(trailingMarginIndex);
    while (desiredWidth > allowedWidth && prioritizedNonzeroRemovalStack.count > 0) {
        iTermStatusBarContainerView *viewToRemove =
            iTermStatusBarPopNextNonzeroCandidate(prioritizedNonzeroRemovalStack, prioritizedIndexes);
        if (!viewToRemove) {
            break;
        }
        iTermStatusBarContainerView *adjacentViewToRemove =
            [self viewToRemoveAdjacentToViewBeingRemoved:viewToRemove fromViews:orderedPrioritized];

        iTermStatusBarRemoveViewFromPrioritized(viewToRemove, prioritized, prioritizedIndexes, widthContributions,
                                                &baseWidth, &trailingMarginIndex);
        iTermStatusBarRemoveViewFromOrderedSubset(viewToRemove, orderedPrioritized, orderedSubsetIndexes);
        desiredWidth = baseWidth + iTermStatusBarTrailingMarginBonus(trailingMarginIndex);

        if (adjacentViewToRemove) {
            iTermStatusBarRemoveViewFromPrioritized(adjacentViewToRemove, prioritized, prioritizedIndexes,
                                                    widthContributions, &baseWidth, &trailingMarginIndex);
            iTermStatusBarRemoveViewFromOrderedSubset(adjacentViewToRemove, orderedPrioritized, orderedSubsetIndexes);
            desiredWidth = baseWidth + iTermStatusBarTrailingMarginBonus(trailingMarginIndex);
        }
    }

    // Preserve original order
    NSArray<iTermStatusBarContainerView *> *orderedResult = [orderedPrioritized copy];
    if (prioritizedIndexes) {
        CFRelease(prioritizedIndexes);
    }
    if (orderedSubsetIndexes) {
        CFRelease(orderedSubsetIndexes);
    }
    return orderedResult;
}

- (void)makeWidthsAndOriginsIntegers:(NSArray<iTermStatusBarContainerView *> *)views {
    for (iTermStatusBarContainerView *view in views) {
        view.desiredOrigin = floor(view.desiredOrigin);
        view.desiredWidth = ceil(view.desiredWidth);
    }
}

- (CGFloat)minimumWidthForComponent:(id<iTermStatusBarComponent>)component {
    NSNumber *cachedWidth = [_minimumWidthCache objectForKey:component];
    if (cachedWidth) {
        return cachedWidth.doubleValue;
    }

    const CGFloat minPreferred = component.statusBarComponentMinimumWidth;
    NSDictionary *knobValues = component.configuration[iTermStatusBarComponentConfigurationKeyKnobValues];
    NSNumber *knobValue = knobValues[iTermStatusBarMinimumWidthKey];
    const CGFloat resolvedWidth = knobValue ? MAX(knobValue.doubleValue, minPreferred) : minPreferred;
    [_minimumWidthCache setObject:@(resolvedWidth) forKey:component];
    return resolvedWidth;
}

- (CGFloat)maximumWidthForComponent:(id<iTermStatusBarComponent>)component {
    NSNumber *cachedWidth = [_maximumWidthCache objectForKey:component];
    if (cachedWidth) {
        return cachedWidth.doubleValue;
    }

    const CGFloat maxPreferred = component.statusBarComponentMaximumWidth;
    NSDictionary *knobValues = component.configuration[iTermStatusBarComponentConfigurationKeyKnobValues];
    NSNumber *knobValue = knobValues[iTermStatusBarMaximumWidthKey];
    const CGFloat min = [self minimumWidthForComponent:component];
    const CGFloat resolvedWidth = MAX(min, knobValue ? MIN(knobValue.doubleValue, maxPreferred) : maxPreferred);
    [_maximumWidthCache setObject:@(resolvedWidth) forKey:component];
    return resolvedWidth;
}

// Returns non-hidden container views that all satisfy their minimum width requirement.
- (NSArray<iTermStatusBarContainerView *> *)visibleContainerViews {
    NSArray<iTermStatusBarContainerView *> *unhiddenViews = [self unhiddenContainerViews];
    NSArray<iTermStatusBarContainerView *> *visibleContainerViews =
        [self fittingSubsetOfContainerViewsFrom:unhiddenViews];
    [self updateDesiredWidthsForViews:visibleContainerViews];
    [self makeWidthsAndOriginsIntegers:visibleContainerViews];
    return visibleContainerViews;
}

@end
