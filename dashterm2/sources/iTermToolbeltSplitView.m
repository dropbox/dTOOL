//
//  iTermToolbeltSplitView.m
//  DashTerm2
//
//  Created by George Nachman on 7/5/15.
//
//

#import "iTermToolbeltSplitView.h"

@implementation iTermToolbeltSplitView {
    NSColor *_dividerColor;
}

// ARC handles dealloc - no manual memory management needed

- (void)setDividerColor:(NSColor *)dividerColor {
    _dividerColor = [dividerColor copy];
    [self setNeedsDisplay:YES];
}

- (NSColor *)dividerColor {
    return _dividerColor;
}

@end

