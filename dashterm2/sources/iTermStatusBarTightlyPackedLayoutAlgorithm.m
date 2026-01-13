//
//  iTermStatusBarTightlyPackedLayoutAlgorithm.m
//  DashTerm2
//
//  Created by George Nachman on 1/20/19.
//

#import "iTermStatusBarTightlyPackedLayoutAlgorithm.h"

#import "DebugLogging.h"
#import "iTermStatusBarComponent.h"
#import "iTermStatusBarContainerView.h"
#import "NSArray+iTerm.h"

@implementation iTermStatusBarTightlyPackedLayoutAlgorithm

- (double)totalGrowthAfterUpdatingDesiredWidthsForAvailableWidth:(CGFloat)availableWidth
                                            sumOfSpringConstants:(double)sumOfSpringConstants
                                                           views:(NSArray<iTermStatusBarContainerView *> *)views {
    const NSUInteger count = views.count;
    if (count == 0 || sumOfSpringConstants <= 0) {
        return 0;
    }

    double growth = 0;
    const double widthPerSpring = availableWidth / sumOfSpringConstants;
    for (NSUInteger idx = 0; idx < count; idx++) {
        iTermStatusBarContainerView *view = views[idx];
        id<iTermStatusBarComponent> component = view.component;
        const double springConstant = component.statusBarComponentSpringConstant;
        double delta = floor(springConstant * widthPerSpring);
        const CGFloat currentWidth = view.desiredWidth;
        const CGFloat maxWidth = [self maximumWidthForComponent:component];
        if (currentWidth + delta > maxWidth) {
            delta = maxWidth - currentWidth;
        }

        const BOOL hasIcon = component.statusBarComponentIcon != nil;
        const double preferredWidth = component.statusBarComponentPreferredWidth;
        const double maximum = preferredWidth + (hasIcon ? iTermStatusBarViewControllerIconWidth : 0);
        const double proposed = currentWidth + delta;
        if (proposed > maximum) {
            const double overage = floor(proposed - maximum);
            delta -= overage;
        }

        view.desiredWidth = currentWidth + delta;
        growth += delta;
        DLog(@"  grow %@ by %@ to %@. Its preferred width is %@",
             component,
             @(delta),
             @(view.desiredWidth),
             @(preferredWidth));
    }

    return growth;
}


@end
