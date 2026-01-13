//
//  NSView+RecursiveDescription.m
//  iTerm
//
//  Created by George Nachman on 11/18/13.
//
//

#import "NSView+RecursiveDescription.h"
#import "NSObject+iTerm.h"

@implementation NSView (RecursiveDescription)

- (NSString *)recursiveDescriptionWithPrefix:(NSString *)prefix {
    // Estimate: ~256 chars for self + 256 per subview
    NSMutableString *s = [NSMutableString stringWithCapacity:256 * (1 + self.subviews.count)];

    [s appendString:prefix];
    [s appendString:[self it_description]];
    [s appendString:@"\n"];
    for (NSView *view in [self subviews]) {
        [s appendString:[view recursiveDescriptionWithPrefix:[prefix stringByAppendingString:@"|   "]]];
    }
    return s;
}

- (NSString *)iterm_recursiveDescription {
    return [self recursiveDescriptionWithPrefix:@""];
}

- (NSString *)it_description {
    // Typical view description is ~200 chars
    NSMutableString *s = [NSMutableString stringWithCapacity:256];
    NSMutableArray *arm = [NSMutableArray arrayWithCapacity:6]; // Autoresizing mask flags
    if (self.autoresizingMask & NSViewHeightSizable) {
        [arm addObject:@"h"];
    }
    if (self.autoresizingMask & NSViewWidthSizable) {
        [arm addObject:@"w"];
    }
    if (self.autoresizingMask & NSViewMinXMargin) {
        [arm addObject:@"minX"];
    }
    if (self.autoresizingMask & NSViewMaxXMargin) {
        [arm addObject:@"maxX"];
    }
    if (self.autoresizingMask & NSViewMinYMargin) {
        [arm addObject:@"minY"];
    }
    if (self.autoresizingMask & NSViewMaxYMargin) {
        [arm addObject:@"maxY"];
    }
    if (self.autoresizesSubviews) {
        [arm addObject:@"subviews"];
    }
    NSString *detail = @"";
    NSTextField *textField = [NSTextField castFrom:self];
    if (textField) {
        detail = [NSString stringWithFormat:@"stringValue=“%@” ", textField.stringValue];
    }

    NSButton *button = [NSButton castFrom:self];
    if (button) {
        detail = [NSString stringWithFormat:@"title=“%@” ", button.title];
    }

    NSPopUpButton *popup = [NSPopUpButton castFrom:self];
    if (popup) {
        detail = [NSString stringWithFormat:@"selectedTitle=“%@” ", popup.selectedItem.title];
    }

    if (self.identifier.length) {
        detail = [detail stringByAppendingFormat:@"id=%@ ", self.identifier];
    }

    [s appendFormat:@"%@ frame=%@ hidden=%@ alphaValue=%0.2f autoresizing=%@ autolayout=%@ %@tracking_areas=%@", self,
                    [NSValue valueWithRect:self.frame], self.isHidden ? @"YES" : @"no", self.alphaValue,
                    [arm componentsJoinedByString:@","],
                    self.translatesAutoresizingMaskIntoConstraints
                        ? @"No"
                        : [NSString stringWithFormat:@"*AUTO LAYOUT IN EFFECT* intrinsicContentSize=%@",
                                                     NSStringFromSize(self.intrinsicContentSize)],
                    detail, self.trackingAreas.count ? self.trackingAreas : @"none"];
    return s;
}

@end
