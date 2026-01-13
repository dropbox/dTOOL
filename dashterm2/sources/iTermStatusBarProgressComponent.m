//
//  iTermStatusBarProgressComponent.m
//  DashTerm2
//
//  Created by George Nachman on 7/26/18.
//

#import "iTermStatusBarProgressComponent.h"

#import "DebugLogging.h"
#import "NSImage+iTerm.h"
#import "PasteViewController.h"

NS_ASSUME_NONNULL_BEGIN

@interface iTermStatusBarProgressComponent()<PasteViewControllerDelegate>
@end

@implementation iTermStatusBarProgressComponent {
    PasteViewController *_viewController;
}

- (CGFloat)statusBarComponentMinimumWidth {
    return 125;
}

- (void)statusBarComponentSizeView:(NSView *)view toFitWidth:(CGFloat)width {
    // BUG-f1273: Replace assert() with guard - unexpected view should log warning, not crash
    if (view != _viewController.view) {
        DLog(@"WARNING: BUG-f1273 statusBarComponentSizeView called with unexpected view %@ != %@", view, _viewController.view);
        return;
    }
    NSRect rect = view.frame;
    rect.size.width = width;
    rect.size.height = 18;
    view.frame = rect;
}

- (CGFloat)statusBarComponentPreferredWidth {
    return 200;
}

- (BOOL)statusBarComponentCanStretch {
    return YES;
}

#pragma mark - iTermStatusBarComponent

- (nullable NSImage *)statusBarComponentIcon {
    return [NSImage it_cacheableImageNamed:@"StatusBarIconPaste" forClass:[self class]];
}


- (NSString *)statusBarComponentShortDescription {
    return @"Progress Indicator";
}

- (NSString *)statusBarComponentDetailedDescription {
    [self doesNotRecognizeSelector:_cmd];
    return @"Generic progress indicator";
}

- (NSArray<iTermStatusBarComponentKnob *> *)statusBarComponentKnobs {
    return @[];
}

- (id)statusBarComponentExemplarWithBackgroundColor:(NSColor *)backgroundColor
                                          textColor:(NSColor *)textColor {
    [self doesNotRecognizeSelector:_cmd];
    return @"[=== ]";
}

- (NSView *)statusBarComponentView {
    if (!_viewController) {
        _viewController = [[PasteViewController alloc] initWithContext:self.pasteContext
                                                                length:self.bufferLength
                                                                  mini:YES];
        _viewController.delegate = self;
    }
    return _viewController.view;
}

- (void)statusBarDefaultTextColorDidChange {
    [_viewController updateLabelColor];
}

- (void)setRemainingLength:(int)remainingLength {
    _viewController.remainingLength = remainingLength;
}

- (int)remainingLength {
    return _viewController.remainingLength;
}

#pragma mark - PasteViewControllerDelegate

- (void)pasteViewControllerDidCancel {
    [self.progressDelegate statusBarProgressComponentDidCancel];
}

@end

NS_ASSUME_NONNULL_END
