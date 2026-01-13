//
//  iTermAlphaBlendingHelper.m
//  DashTerm2
//
//  Created by George Nachman on 2/24/20.
//

#import "iTermAlphaBlendingHelper.h"
#import "DebugLogging.h"

// Threshold for preventing division by 0. 10^-3 is enough that the discontinuity is imperceptible.
const CGFloat iTermAlphaBlendingHelperEpsilon = 0.001;

// Formula for `q`
// BUG-f866, BUG-f867: Replace assertions with value clamping for robustness
static CGFloat Q(CGFloat l) {
    // Clamp l to valid range [0, 1) instead of asserting
    if (l < 0) {
        DLog(@"BUG-f866: Q() called with negative l=%f, clamping to 0", l);
        l = 0;
    }
    if (l >= 1) {
        DLog(@"BUG-f867: Q() called with l=%f >= 1, clamping to 0.999", l);
        l = 1.0 - iTermAlphaBlendingHelperEpsilon;
    }
    return 1.0 / (1.0 - l) - 1.0;
}

// Background color, aka b
CGFloat iTermAlphaValueForTopView(CGFloat t, CGFloat l) {
    if (l > (1 - iTermAlphaBlendingHelperEpsilon)) {
        // Problem (2)
        return 0;
    }

    // Formula for `b`
    const CGFloat alpha = (1.0 - t) / (Q(l) + 1);
    return alpha;
}

// Image, aka a
CGFloat iTermAlphaValueForBottomView(CGFloat t, CGFloat l) {
    if (l < iTermAlphaBlendingHelperEpsilon) {
        // Problem (3)
        return 0;
    }
    if (l > (1 - iTermAlphaBlendingHelperEpsilon)) {
        // Problem (1)
        return 1 - t;
    }
    const CGFloat q = Q(l);

    // Formula for `a`
    const CGFloat alpha = q * (1.0 - t) / (q + t);
    return alpha;
}
