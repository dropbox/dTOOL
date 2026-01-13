//
//  iTermStartupPlaceholderWindow.m
//  DashTerm2
//
//  Lightweight placeholder window shown immediately at app launch.
//

#import "iTermStartupPlaceholderWindow.h"

@interface iTermStartupPlaceholderCursorView : NSView
@property (nonatomic, strong) NSTimer *blinkTimer;
@property (nonatomic, assign) BOOL cursorVisible;
@end

@implementation iTermStartupPlaceholderCursorView

- (instancetype)initWithFrame:(NSRect)frameRect {
    self = [super initWithFrame:frameRect];
    if (self) {
        _cursorVisible = YES;
        _blinkTimer = [NSTimer scheduledTimerWithTimeInterval:0.5
                                                       target:self
                                                     selector:@selector(toggleCursor)
                                                     userInfo:nil
                                                      repeats:YES];
    }
    return self;
}

- (void)dealloc {
    [_blinkTimer invalidate];
}

- (void)toggleCursor {
    _cursorVisible = !_cursorVisible;
    [self setNeedsDisplay:YES];
}

- (void)drawRect:(NSRect)dirtyRect {
    if (_cursorVisible) {
        [[NSColor colorWithWhite:0.9 alpha:1.0] setFill];
        NSRectFill(self.bounds);
    }
}

@end

// ---

static iTermStartupPlaceholderWindow *sSharedInstance = nil;

@interface iTermStartupPlaceholderWindow ()
@property (nonatomic, strong) iTermStartupPlaceholderCursorView *cursorView;
@property (nonatomic, strong) NSTextField *promptLabel;
@end

@implementation iTermStartupPlaceholderWindow

+ (iTermStartupPlaceholderWindow *)sharedInstance {
    return sSharedInstance;
}

+ (void)setSharedInstance:(iTermStartupPlaceholderWindow *)instance {
    sSharedInstance = instance;
}

+ (void)showPlaceholder {
    if (sSharedInstance) {
        return;
    }

    // Use saved window frame if available, otherwise use sensible defaults
    NSRect frame = NSMakeRect(100, 100, 720, 480);
    NSArray *savedWindows = [[NSUserDefaults standardUserDefaults] arrayForKey:@"NSWindow Frame TerminalWindow"];
    if (savedWindows.count > 0) {
        // Parse saved frame string: "x y w h ..."
        NSString *frameString = savedWindows.firstObject;
        if ([frameString isKindOfClass:[NSString class]]) {
            frame = NSRectFromString(frameString);
            if (NSWidth(frame) < 200 || NSHeight(frame) < 100) {
                frame = NSMakeRect(100, 100, 720, 480);
            }
        }
    }

    // Center on main screen if we're using defaults
    NSScreen *mainScreen = [NSScreen mainScreen];
    if (mainScreen && NSEqualRects(frame, NSMakeRect(100, 100, 720, 480))) {
        NSRect screenFrame = mainScreen.visibleFrame;
        frame.origin.x = NSMidX(screenFrame) - NSWidth(frame) / 2;
        frame.origin.y = NSMidY(screenFrame) - NSHeight(frame) / 2;
    }

    sSharedInstance = [[iTermStartupPlaceholderWindow alloc] initWithFrame:frame];
    [sSharedInstance makeKeyAndOrderFront:nil];
}

+ (void)dismissPlaceholder {
    if (!sSharedInstance) {
        return;
    }

    [NSAnimationContext runAnimationGroup:^(NSAnimationContext *context) {
        context.duration = 0.15;
        [[sSharedInstance animator] setAlphaValue:0.0];
    } completionHandler:^{
        [sSharedInstance orderOut:nil];
        sSharedInstance = nil;
    }];
}

- (instancetype)initWithFrame:(NSRect)frame {
    self = [super initWithContentRect:frame
                            styleMask:(NSWindowStyleMaskTitled |
                                      NSWindowStyleMaskClosable |
                                      NSWindowStyleMaskMiniaturizable |
                                      NSWindowStyleMaskResizable)
                              backing:NSBackingStoreBuffered
                                defer:NO];
    if (self) {
        [self setupWindow];
    }
    return self;
}

- (void)setupWindow {
    // Match terminal appearance
    self.title = @"DashTerm2";
    self.backgroundColor = [NSColor colorWithRed:0.05 green:0.05 blue:0.07 alpha:1.0];
    self.titlebarAppearsTransparent = YES;
    self.titleVisibility = NSWindowTitleHidden;

    // Dark appearance
    self.appearance = [NSAppearance appearanceNamed:NSAppearanceNameDarkAqua];

    // Create content
    NSView *contentView = self.contentView;
    contentView.wantsLayer = YES;
    contentView.layer.backgroundColor = [NSColor colorWithRed:0.05 green:0.05 blue:0.07 alpha:1.0].CGColor;

    // Prompt label (like "user@host:~ $")
    _promptLabel = [[NSTextField alloc] initWithFrame:NSMakeRect(12, NSHeight(contentView.bounds) - 32, 200, 18)];
    _promptLabel.stringValue = @"";  // Empty - just show cursor
    _promptLabel.textColor = [NSColor colorWithWhite:0.8 alpha:1.0];
    _promptLabel.font = [NSFont fontWithName:@"Menlo" size:13] ?: [NSFont monospacedSystemFontOfSize:13 weight:NSFontWeightRegular];
    _promptLabel.bezeled = NO;
    _promptLabel.editable = NO;
    _promptLabel.selectable = NO;
    _promptLabel.drawsBackground = NO;
    _promptLabel.autoresizingMask = NSViewMinYMargin;
    [contentView addSubview:_promptLabel];

    // Blinking cursor
    CGFloat cursorX = 12;
    CGFloat cursorY = NSHeight(contentView.bounds) - 32;
    _cursorView = [[iTermStartupPlaceholderCursorView alloc] initWithFrame:NSMakeRect(cursorX, cursorY, 8, 16)];
    _cursorView.autoresizingMask = NSViewMinYMargin;
    [contentView addSubview:_cursorView];
}

- (void)transitionToWindow:(NSWindow *)realWindow animated:(BOOL)animated {
    if (!animated) {
        [self orderOut:nil];
        sSharedInstance = nil;
        return;
    }

    // Match frames
    [realWindow setFrame:self.frame display:NO];

    // Fade out placeholder, show real window
    realWindow.alphaValue = 0.0;
    [realWindow makeKeyAndOrderFront:nil];

    [NSAnimationContext runAnimationGroup:^(NSAnimationContext *context) {
        context.duration = 0.1;
        [[self animator] setAlphaValue:0.0];
        [[realWindow animator] setAlphaValue:1.0];
    } completionHandler:^{
        [self orderOut:nil];
        sSharedInstance = nil;
    }];
}

@end
