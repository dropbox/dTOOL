//
//  iTermStatusBarLayoutAlgorithm.m
//  DashTerm2SharedARC
//
//  Created by George Nachman on 1/19/19.
//

#import "iTermStatusBarLayoutAlgorithm.h"

#import "DebugLogging.h"
#import "iTermStatusBarContainerView.h"
#import "NSArray+iTerm.h"
#import "NSObject+iTerm.h"
#import "iTermStatusBarTightlyPackedLayoutAlgorithm.h"
#import "iTermStatusBarStableLayoutAlgorithm.h"

const CGFloat iTermStatusBarViewControllerMargin = 10;

@implementation iTermStatusBarLayoutAlgorithm

+ (instancetype)alloc {
    if ([self class] == [iTermStatusBarLayoutAlgorithm class]) {
        // BUG-412: Replace assert(NO) with ELog - abstract class instantiation protection
        // This should never be called directly on the base class, but if it is,
        // log and return nil instead of crashing
        ELog(@"iTermStatusBarLayoutAlgorithm is an abstract class - use a subclass");
        return nil;
    }
    return [super alloc];
}

+ (instancetype)layoutAlgorithmWithContainerViews:(NSArray<iTermStatusBarContainerView *> *)containerViews
                                    mandatoryView:(nullable iTermStatusBarContainerView *)mandatoryView
                                   statusBarWidth:(CGFloat)statusBarWidth
                                          setting:(iTermStatusBarLayoutAlgorithmSetting)setting
                            removeEmptyComponents:(BOOL)removeEmptyComponents {
    switch (setting) {
        case iTermStatusBarLayoutAlgorithmSettingStable:
            return [[iTermStatusBarStableLayoutAlgorithm alloc] initWithContainerViews:containerViews
                                                                         mandatoryView:mandatoryView
                                                                        statusBarWidth:statusBarWidth
                                                                 removeEmptyComponents:removeEmptyComponents];
        case iTermStatusBarLayoutAlgorithmSettingTightlyPacked:
            return [[iTermStatusBarTightlyPackedLayoutAlgorithm alloc] initWithContainerViews:containerViews
                                                                                mandatoryView:mandatoryView
                                                                               statusBarWidth:statusBarWidth
                                                                        removeEmptyComponents:removeEmptyComponents];
    }
    return nil;
}

- (instancetype)initWithContainerViews:(NSArray<iTermStatusBarContainerView *> *)containerViews
                         mandatoryView:(nonnull iTermStatusBarContainerView *)mandatoryView
                        statusBarWidth:(CGFloat)statusBarWidth
                 removeEmptyComponents:(BOOL)removeEmptyComponents {
    self = [super init];
    if (self) {
        _mandatoryView = mandatoryView;
        _removeEmptyComponents = removeEmptyComponents;
    }
    return self;
}

- (NSArray<iTermStatusBarContainerView *> *)visibleContainerViews {
    // BUG-413: Replace assert(NO) with ELog - abstract method protection
    // Subclasses must override this method; if called on base class, log and return empty
    ELog(@"visibleContainerViews must be overridden in a subclass of iTermStatusBarLayoutAlgorithm");
    return @[];
}

@end
