//
//  iTermStatusBarPlaceholderComponent.m
//  DashTerm2
//
//  Created by George Nachman on 09/03/19.
//

#import "iTermStatusBarPlaceholderComponent.h"

#import "DebugLogging.h"

NS_ASSUME_NONNULL_BEGIN

@implementation iTermStatusBarPlaceholderComponent

- (NSString *)statusBarComponentShortDescription {
    return @"Placeholder";
}

- (NSString *)statusBarComponentDetailedDescription {
    return @"Placeholder";
}

- (id)statusBarComponentExemplarWithBackgroundColor:(NSColor *)backgroundColor textColor:(NSColor *)textColor {
    // BUG-414: Replace assert(NO) with DLog - exemplar not needed for placeholder
    // This method shouldn't be called for the placeholder component, but if it is,
    // return the placeholder text instead of crashing
    DLog(@"iTermStatusBarPlaceholderComponent: statusBarComponentExemplarWithBackgroundColor called (unexpected)");
    return self.stringValue ?: @"Click here to configure status bar";
}

- (BOOL)statusBarComponentCanStretch {
    return YES;
}

- (BOOL)statusBarComponentIsInternal {
    return YES;
}

- (nullable NSString *)stringValue {
    return @"Click here to configure status bar";
}

- (nullable NSString *)stringValueForCurrentWidth {
    return self.stringValue;
}

- (nullable NSArray<NSString *> *)stringVariants {
    return @[ self.stringValue ?: @"" ];
}

- (BOOL)statusBarComponentHandlesClicks {
    return YES;
}

- (void)statusBarComponentDidClickWithView:(NSView *)view {
    [self.delegate statusBarComponentOpenStatusBarPreferences:self];
}

- (BOOL)statusBarComponentIsEmpty {
    // This is used to ensure there is at least one component, so it mustn't be hidden due to emptiness.
    return NO;
}

@end

NS_ASSUME_NONNULL_END
