//
//  iTermAboutWindowController.m
//  DashTerm2
//
//  DashTerm2 created by Andrew Yates
//  Based on DashTerm2 by George Nachman
//
//  Original DashTerm2 file created by George Nachman on 9/21/14.
//
// BUG-273: About window patron acknowledgment updated to DashTerm2
// BUG-289: About window whitebox URL updated to DashTerm2

#import "iTermAboutWindowController.h"

#import "DashTerm2SharedARC-Swift.h"
#import "iTermLaunchExperienceController.h"
#import "NSArray+iTerm.h"
#import "NSMutableAttributedString+iTerm.h"
#import "NSObject+iTerm.h"
#import "NSStringITerm.h"

static NSString *iTermAboutWindowControllerWhatsNewURLString = @"dashterm2://whats-new/";

@interface iTermAboutWindowContentView : NSVisualEffectView
@end

@interface iTermSponsor: NSObject
@property (nonatomic) NSTextField *textField;
@property (nonatomic) NSTrackingArea *trackingArea;
@property (nonatomic) NSView *view;
@property (nonatomic, copy) NSString *url;

+ (instancetype)sponsorWithView:(NSView *)view textField:(NSTextField *)textField container:(NSView *)container url:(NSString *)url;
@end

@implementation iTermSponsor
+ (instancetype)sponsorWithView:(NSView *)view textField:(NSTextField *)textField container:(NSView *)container url:(NSString *)url {
    iTermSponsor *sponsor = [[iTermSponsor alloc] init];
    sponsor.view = view;
    sponsor.textField = textField;
    sponsor.url = url;

    // Create a tracking area for the sponsor's view
    sponsor.trackingArea = [[NSTrackingArea alloc] initWithRect:view.frame
                                                        options:NSTrackingMouseEnteredAndExited | NSTrackingActiveAlways
                                                          owner:container
                                                       userInfo:nil];
    [view addTrackingArea:sponsor.trackingArea];
    if (textField) {
        NSDictionary *underlineAttribute = @{NSUnderlineStyleAttributeName: @(NSUnderlineStyleSingle)};
        NSAttributedString *attributedString = [[NSAttributedString alloc] initWithString:[textField stringValue] attributes:underlineAttribute];
        [textField setAttributedStringValue:attributedString];
    }
    return sponsor;
}

- (void)updateTrackingAreaForContainer:(NSView *)container {
    [container removeTrackingArea:self.trackingArea];
    self.trackingArea = [[NSTrackingArea alloc] initWithRect:self.view.frame
                                                    options:NSTrackingMouseEnteredAndExited | NSTrackingActiveAlways
                                                      owner:container
                                                   userInfo:nil];
    [container addTrackingArea:self.trackingArea];
}
@end

@implementation iTermAboutWindowContentView {
    IBOutlet NSScrollView *_bottomAlignedScrollView;
    IBOutlet NSTextView *_sponsorsHeading;

    IBOutlet NSView *_whitebox;
    IBOutlet NSTextField *_whiteboxText;

    IBOutlet NSView *_codeRabbit;

    NSArray<iTermSponsor *> *_sponsors;
}

- (void)resizeSubviewsWithOldSize:(NSSize)oldSize {
    NSRect frame = _bottomAlignedScrollView.frame;
    [super resizeSubviewsWithOldSize:oldSize];
    CGFloat topMargin = oldSize.height - NSMaxY(frame);
    frame.origin.y = self.frame.size.height - topMargin - frame.size.height;
    _bottomAlignedScrollView.frame = frame;
}

- (void)awakeFromNib {
    [super awakeFromNib];
    NSMutableParagraphStyle *paragraphStyle = [[NSMutableParagraphStyle alloc] init];
    paragraphStyle.alignment = NSTextAlignmentCenter;
    _sponsorsHeading.selectable = YES;
    _sponsorsHeading.editable = NO;
    [_sponsorsHeading.textStorage setAttributedString:[NSAttributedString attributedStringWithHTML:_sponsorsHeading.textStorage.string
                                                                                              font:_sponsorsHeading.font
                                                                                    paragraphStyle:paragraphStyle]];

    _sponsors = @[ [iTermSponsor sponsorWithView:_whitebox
                                       textField:_whiteboxText
                                       container:self
                                             url:@"https://whitebox.so/?utm_source=DashTerm2"],
                   [iTermSponsor sponsorWithView:_codeRabbit
                                       textField:nil
                                       container:self
                                             url:@"https://coderabbit.ai/"]];
}


- (void)mouseEntered:(NSEvent *)theEvent {
    [NSCursor.pointingHandCursor set];
}

- (void)mouseExited:(NSEvent *)theEvent {
    [NSCursor.arrowCursor set];
}

- (void)mouseUp:(NSEvent *)theEvent {
    if (theEvent.clickCount == 1) {
        NSPoint locationInView = [self convertPoint:theEvent.locationInWindow fromView:nil];
        [_sponsors enumerateObjectsUsingBlock:^(iTermSponsor * _Nonnull sponsor, NSUInteger idx, BOOL * _Nonnull stop) {
            if (NSPointInRect(locationInView, sponsor.view.frame)) {
                // Open the link
                [[NSWorkspace sharedWorkspace] openURL:[NSURL URLWithString:sponsor.url]];
            }
        }];
    }
}

// Don't forget to update the tracking area when the view resizes
- (void)updateTrackingAreas {
    [super updateTrackingAreas];
    [_sponsors enumerateObjectsUsingBlock:^(iTermSponsor * _Nonnull sponsor, NSUInteger idx, BOOL * _Nonnull stop) {
        [sponsor updateTrackingAreaForContainer:self];
    }];
}

@end

@interface iTermAboutWindowController()<NSTextViewDelegate>
@end

@implementation iTermAboutWindowController {
    IBOutlet NSTextView *_dynamicText;
    IBOutlet NSTextView *_patronsTextView;
}

+ (instancetype)sharedInstance {
    static id instance;
    static dispatch_once_t once;
    dispatch_once(&once, ^{
        instance = [[self alloc] init];
    });
    return instance;
}

- (instancetype)init {
    self = [super initWithWindowNibName:@"AboutWindow"];
    if (self) {
        NSDictionary *myDict = [[NSBundle bundleForClass:[self class]] infoDictionary];
        NSString *const versionNumber = myDict[(NSString *)kCFBundleVersionKey];
        NSString *versionString = [NSString stringWithFormat: @"Build %@\n\n", versionNumber];
        NSAttributedString *whatsNew = nil;
        if ([versionNumber hasPrefix:@"3.6."] || [versionString isEqualToString:@"unknown"]) {
            whatsNew = [self attributedStringWithLinkToURL:iTermAboutWindowControllerWhatsNewURLString
                                                     title:@"What's New in 3.6?\n"];
        }

        NSAttributedString *webAString = [self attributedStringWithLinkToURL:@"https://github.com/ayates_dbx/dashterm2"
                                                                       title:@"Home Page"];
        NSAttributedString *bugsAString =
                [self attributedStringWithLinkToURL:@"https://github.com/ayates_dbx/dashterm2/issues"
                                              title:@"Report a bug"];
        NSAttributedString *creditsAString =
                [self attributedStringWithLinkToURL:@"https://github.com/ayates_dbx/dashterm2#credits"
                                              title:@"Credits"];

        // Force IBOutlets to be bound by creating window.
        [self window];

        NSDictionary *versionAttributes = @{ NSForegroundColorAttributeName: [NSColor controlTextColor] };
        NSDictionary *creatorAttributes = @{ NSForegroundColorAttributeName: [NSColor controlTextColor],
                                             NSFontAttributeName: [NSFont boldSystemFontOfSize:12] };
        NSAttributedString *bullet = [[NSAttributedString alloc] initWithString:@" ∙ "
                                                                     attributes:versionAttributes];
        [_dynamicText setLinkTextAttributes:self.linkTextViewAttributes];
        [[_dynamicText textStorage] deleteCharactersInRange:NSMakeRange(0, [[_dynamicText textStorage] length])];

        // DashTerm2 creator attribution
        [[_dynamicText textStorage] appendAttributedString:[[NSAttributedString alloc] initWithString:@"Created by Andrew Yates\n"
                                                                                            attributes:creatorAttributes]];
        NSAttributedString *emailLink = [self attributedStringWithLinkToURL:@"mailto:ayates@dropbox.com"
                                                                      title:@"ayates@dropbox.com"];
        [[_dynamicText textStorage] appendAttributedString:emailLink];
        [[_dynamicText textStorage] appendAttributedString:[[NSAttributedString alloc] initWithString:@"\n\n"
                                                                                            attributes:versionAttributes]];
        // DashTerm2 attribution
        [[_dynamicText textStorage] appendAttributedString:[[NSAttributedString alloc] initWithString:@"Based on iTerm2 by "
                                                                                            attributes:versionAttributes]];
        NSAttributedString *gnachmanLink = [self attributedStringWithLinkToURL:@"https://github.com/gnachman"
                                                                         title:@"George Nachman"];
        [[_dynamicText textStorage] appendAttributedString:gnachmanLink];
        [[_dynamicText textStorage] appendAttributedString:[[NSAttributedString alloc] initWithString:@"\n\n"
                                                                                            attributes:versionAttributes]];

        [[_dynamicText textStorage] appendAttributedString:[[NSAttributedString alloc] initWithString:versionString
                                                                                            attributes:versionAttributes]];
        if (whatsNew) {
            [[_dynamicText textStorage] appendAttributedString:whatsNew];
        }
        [[_dynamicText textStorage] appendAttributedString:webAString];
        [[_dynamicText textStorage] appendAttributedString:bullet];
        [[_dynamicText textStorage] appendAttributedString:bugsAString];
        [[_dynamicText textStorage] appendAttributedString:bullet];
        [[_dynamicText textStorage] appendAttributedString:creditsAString];
        [_dynamicText setAlignment:NSTextAlignmentCenter
                             range:NSMakeRange(0, [[_dynamicText textStorage] length])];

        // Clear the patrons view - DashTerm2 is by Andrew Yates
        [[_patronsTextView textStorage] deleteCharactersInRange:NSMakeRange(0, [[_patronsTextView textStorage] length])];
    }
    return self;
}

- (NSDictionary *)linkTextViewAttributes {
    return @{ NSUnderlineStyleAttributeName: @(NSUnderlineStyleSingle),
              NSForegroundColorAttributeName: [NSColor linkColor],
              NSCursorAttributeName: [NSCursor pointingHandCursor] };
}

- (void)setPatronsString:(NSAttributedString *)patronsAttributedString animate:(BOOL)animate {
    NSSize minSize = _patronsTextView.minSize;
    minSize.height = 1;
    _patronsTextView.minSize = minSize;

    [_patronsTextView setLinkTextAttributes:self.linkTextViewAttributes];
    [[_patronsTextView textStorage] deleteCharactersInRange:NSMakeRange(0, [[_patronsTextView textStorage] length])];
    [[_patronsTextView textStorage] appendAttributedString:patronsAttributedString];
    [_patronsTextView setAlignment:NSTextAlignmentLeft
                         range:NSMakeRange(0, [[_patronsTextView textStorage] length])];
    _patronsTextView.horizontallyResizable = NO;

    NSRect rect = _patronsTextView.enclosingScrollView.frame;
    [_patronsTextView sizeToFit];
    const CGFloat desiredHeight = [_patronsTextView.textStorage heightForWidth:rect.size.width];
    CGFloat diff = desiredHeight - rect.size.height;
    rect.size.height = desiredHeight;
    rect.origin.y -= diff;
    _patronsTextView.enclosingScrollView.frame = rect;
    
    rect = self.window.frame;
    rect.size.height += diff;
    rect.origin.y -= diff;
    [self.window setFrame:rect display:YES animate:animate];
}

- (NSAttributedString *)defaultPatronsString {
    NSString *string = [NSString stringWithFormat:@"Loading supporters…"];
    NSMutableAttributedString *attributedString =
        [[NSMutableAttributedString alloc] initWithString:string
                                               attributes:self.attributes];
    return attributedString;
}

- (NSDictionary *)attributes {
    NSMutableParagraphStyle *style = [[NSMutableParagraphStyle alloc] init];
    [style setMinimumLineHeight:18];
    [style setMaximumLineHeight:18];
    [style setLineSpacing:3];
    return @{ NSForegroundColorAttributeName: [NSColor controlTextColor],
              NSParagraphStyleAttributeName: style
    };
}

- (void)setPatrons:(NSArray *)patronNames {
    if (!patronNames.count) {
        [self setPatronsString:[[NSAttributedString alloc] initWithString:@"Error loading patrons :("
                                                                attributes:[self attributes]]
                       animate:NO];
        return;
    }

    NSArray *sortedNames = [patronNames sortedArrayUsingSelector:@selector(localizedCaseInsensitiveCompare:)];
    NSString *string = [sortedNames componentsJoinedWithOxfordComma];
    NSDictionary *attributes = [self attributes];
    NSMutableAttributedString *attributedString =
        [[NSMutableAttributedString alloc] initWithString:string
                                               attributes:attributes];
    NSAttributedString *period = [[NSAttributedString alloc] initWithString:@"."];
    [attributedString appendAttributedString:period];

    [self setPatronsString:attributedString animate:YES];
}

- (NSAttributedString *)attributedStringWithLinkToURL:(NSString *)urlString title:(NSString *)title {
    NSDictionary *linkAttributes = @{ NSLinkAttributeName: [NSURL URLWithString:urlString] };
    NSString *localizedTitle = title;
    return [[NSAttributedString alloc] initWithString:localizedTitle
                                            attributes:linkAttributes];
}

#pragma mark - NSTextViewDelegate

- (BOOL)textView:(NSTextView *)textView clickedOnLink:(id)link atIndex:(NSUInteger)charIndex {
    NSURL *url = [NSURL castFrom:link];
    if ([url.absoluteString isEqualToString:iTermAboutWindowControllerWhatsNewURLString]) {
        [iTermLaunchExperienceController forceShowWhatsNew];
        return YES;
    }
    return NO;
}

@end
