//
//  iTermStartupPlaceholderWindow.h
//  DashTerm2
//
//  Lightweight placeholder window shown immediately at app launch
//  before heavy initialization occurs. Provides instant visual feedback.
//

#import <Cocoa/Cocoa.h>

NS_ASSUME_NONNULL_BEGIN

/// A lightweight window that appears instantly at app launch.
/// Shows a terminal-like dark background with a blinking cursor
/// while the actual terminal initializes in the background.
@interface iTermStartupPlaceholderWindow : NSWindow

/// Shared instance created at startup.
@property (class, nonatomic, strong, nullable) iTermStartupPlaceholderWindow *sharedInstance;

/// Show the placeholder window immediately. Call from main() before NSApplicationMain.
+ (void)showPlaceholder;

/// Dismiss the placeholder window. Call when real terminal window is ready.
+ (void)dismissPlaceholder;

/// Transition from placeholder to real terminal window with optional animation.
- (void)transitionToWindow:(NSWindow *)realWindow animated:(BOOL)animated;

@end

NS_ASSUME_NONNULL_END
