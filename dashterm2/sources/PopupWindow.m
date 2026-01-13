//
//  PopupWindow.m
//  iTerm
//
//  Created by George Nachman on 12/27/13.
//
//

#import "PopupWindow.h"
#import "iTermApplicationDelegate.h"

@implementation PopupWindow {
    BOOL shutdown_;
}

- (instancetype)initWithContentRect:(NSRect)contentRect
                          styleMask:(NSWindowStyleMask)aStyle
                            backing:(NSBackingStoreType)bufferingType
                              defer:(BOOL)flag {
    self = [super initWithContentRect:contentRect
                            styleMask:NSWindowStyleMaskBorderless
                              backing:bufferingType
                                defer:flag];
    if (self) {
        [self setCollectionBehavior:NSWindowCollectionBehaviorMoveToActiveSpace];
        self.opaque = NO;
    }
    return self;
}

- (void)dealloc
{
    // BUG-1154: Cancel delayed twiddleKeyWindow selector to prevent crash
    [NSObject cancelPreviousPerformRequestsWithTarget:self];
    [_owningWindow release];
    [super dealloc];
}

- (BOOL)canBecomeKeyWindow {
    return YES;
}

- (BOOL)canBecomeMainWindow {
    return YES;
}

- (void)keyDown:(NSEvent *)event
{
    id cont = [self windowController];
    if (cont && [cont respondsToSelector:@selector(keyDown:)]) {
        [cont keyDown:event];
    }
}

- (void)shutdown
{
    shutdown_ = YES;
}

- (void)setOwningWindow:(NSWindow *)owningWindow {
    [_owningWindow autorelease];
    _owningWindow = [owningWindow retain];
}

- (void)closeWithoutAdjustingWindowOrder {
    [super close];
}

- (void)close {
    if (shutdown_) {
        [self closeWithoutAdjustingWindowOrder];
    } else {
        // The OS will send a hotkey window to the background if it's open and in
        // all spaces. Make it key before closing. This has to be done later because if you do it
        // here the OS gets confused and two windows are key.
        //
        // RC-011/BUG-1154: Use dispatch_after with weak capture instead of performSelector:afterDelay:
        // to prevent crash if window is deallocated before selector fires.
        // The defensive cancelPreviousPerformRequests in dealloc is kept as backup.
        __weak typeof(self) weakSelf = self;
        dispatch_after(dispatch_time(DISPATCH_TIME_NOW, 0), dispatch_get_main_queue(), ^{
            [weakSelf twiddleKeyWindow];
        });
    }
}

- (void)twiddleKeyWindow
{
    iTermApplicationDelegate *theDelegate = [iTermApplication.sharedApplication delegate];
    [theDelegate makeHotKeyWindowKeyIfOpen];
    [super close];
    [_owningWindow makeKeyAndOrderFront:self];
}

- (BOOL)autoHidesHotKeyWindow {
    return NO;
}

@end
