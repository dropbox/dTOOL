
//
//  PseudoTerminalRestorer.m
//  iTerm
//
//  Created by George Nachman on 10/24/11.
//

// This ifndef affects only the Leopard configuration.

#import "PseudoTerminalRestorer.h"
#import "DebugLogging.h"
#import "iTermAdvancedSettingsModel.h"
#import "iTermApplication.h"
#import "iTermApplicationDelegate.h"
#import "iTermController.h"
#import "iTermOrphanServerAdopter.h"
#import "iTermPreferences.h"
#import "iTermRestorableStateController.h"
#import "iTermUserDefaults.h"
#import "NSApplication+iTerm.h"
#import "NSObject+iTerm.h"
#import "PseudoTerminal.h"
#import "PseudoTerminal+Private.h"
#import "PseudoTerminal+WindowStyle.h"

static NSMutableArray *queuedBlocks;
typedef void (^VoidBlock)(void);
static BOOL gWaitingForFullScreen;
static void (^gPostRestorationCompletionBlock)(void);
static BOOL gRanQueuedBlocks;
static BOOL gExternalRestorationDidComplete;

NSString *const iTermWindowStateKeyGUID = @"guid";

@implementation PseudoTerminalState

- (instancetype)initWithCoder:(NSCoder *)coder {
    self = [super init];
    if (self) {
        // Use secure decoding with allowed property list classes
        NSSet<Class> *plistClasses = [NSSet setWithObjects:[NSDictionary class], [NSArray class], [NSString class], [NSNumber class], [NSData class], [NSDate class], nil];
        _arrangement = [[NSDictionary castFrom:[coder decodeObjectOfClasses:plistClasses forKey:kTerminalWindowStateRestorationWindowArrangementKey]] retain];
        _coder = [coder retain];
    }
    return self;
}

- (instancetype)initWithDictionary:(NSDictionary *)arrangement {
    self = [super init];
    if (self) {
        _arrangement = [arrangement retain];
    }
    return self;
}

- (void)dealloc {
    [_arrangement release];
    [_coder release];
    [super dealloc];
}

@end

@implementation PseudoTerminalRestorer

+ (BOOL)willOpenWindows {
    return queuedBlocks.count > 0;
}

+ (void)runQueuedBlocks {
    CFAbsoluteTime startTime = CFAbsoluteTimeGetCurrent();
    DLog(@"runQueuedBlocks (10.11+) starting");
    NSLog(@"[STARTUP] runQueuedBlocks starting with %lu blocks", (unsigned long)queuedBlocks.count);
    NSInteger blockIndex = 0;
    while (queuedBlocks.count) {
        if (gWaitingForFullScreen) {
            DLog(@"waiting for fullscreen");
            NSLog(@"[STARTUP] Pausing for fullscreen window");
            return;
        }
        DLog(@"Running queued block...");
        CFAbsoluteTime blockStart = CFAbsoluteTimeGetCurrent();
        VoidBlock block = [queuedBlocks firstObject];
        block();
        [queuedBlocks removeObjectAtIndex:0];
        NSLog(@"[STARTUP] Block %ld took %.3fs", (long)blockIndex, CFAbsoluteTimeGetCurrent() - blockStart);
        blockIndex++;
        DLog(@"Finished running queued block");
    }
    DLog(@"Ran all queued blocks");
    NSLog(@"[STARTUP] All %ld queued blocks done in %.3fs", (long)blockIndex, CFAbsoluteTimeGetCurrent() - startTime);
    [queuedBlocks release];
    queuedBlocks = nil;
    gRanQueuedBlocks = YES;
    [self runPostRestorationBlockIfNeeded];
}

+ (void)runPostRestorationBlockIfNeeded {
    if (gPostRestorationCompletionBlock && gExternalRestorationDidComplete) {
        DLog(@"run post-restoration block %p", gPostRestorationCompletionBlock);
        gPostRestorationCompletionBlock();
        [gPostRestorationCompletionBlock release];
        gPostRestorationCompletionBlock = nil;
    }
}

+ (void)externalRestorationDidComplete {
    DLog(@"external restoration completed");
    gExternalRestorationDidComplete = YES;
    [self runPostRestorationBlockIfNeeded];
}

+ (void)setPostRestorationCompletionBlock:(void (^)(void))postRestorationCompletionBlock {
    DLog(@"set post-restoration completion block");
    if (gRanQueuedBlocks && gExternalRestorationDidComplete) {
        postRestorationCompletionBlock();
    } else {
        if (gPostRestorationCompletionBlock) {
            // BUG-7280: The old code did retain+autorelease AND explicit release, causing
            // double-release. Just retain without autorelease, and the block will release
            // when it runs.
            void (^oldBlock)(void) = [gPostRestorationCompletionBlock retain];
            gPostRestorationCompletionBlock = [^{
                DLog(@"call older postrestoration block");
                oldBlock();
                [oldBlock release];
                postRestorationCompletionBlock();
            } copy];
            DLog(@"replace postretoration block %p with new one %p", oldBlock, gPostRestorationCompletionBlock);
        } else {
            gPostRestorationCompletionBlock = [postRestorationCompletionBlock copy];
            DLog(@"postrestoration block is now %p", gPostRestorationCompletionBlock);
        }
    }
}

+ (void (^)(void))postRestorationCompletionBlock {
    return gPostRestorationCompletionBlock;
}

+ (void)restoreWindowWithIdentifier:(NSString *)identifier
                              state:(NSCoder *)state
                  completionHandler:(void (^)(NSWindow *, NSError *))completionHandler {
    [self restoreWindowWithIdentifier:identifier
                  pseudoTerminalState:[[[PseudoTerminalState alloc] initWithCoder:state] autorelease]
                               system:YES
                    completionHandler:completionHandler];
}

+ (void)restoreWindowWithIdentifier:(NSString *)identifier
                pseudoTerminalState:(PseudoTerminalState *)state
                             system:(BOOL)system
                  completionHandler:(void (^)(NSWindow *, NSError *))completionHandler {
    DLog(@"restoreWindowWithIdentifier:%@", identifier);
    if (system && [iTermUserDefaults ignoreSystemWindowRestoration]) {
        DLog(@"Ignore system window restoration because we're using our own restorable state controller.");
        // Use secure decoding for the GUID string
        NSString *guid = [state.coder decodeObjectOfClass:[NSString class] forKey:iTermWindowStateKeyGUID];
        if (!guid) {
            DLog(@"GUID missing.");
            iTermRestorableStateController.shouldIgnoreOpenUntitledFile = YES;
            completionHandler(nil, nil);
            iTermRestorableStateController.shouldIgnoreOpenUntitledFile = NO;
        } else {
            DLog(@"Save completion handler in restorable state controller for window %@", guid);
            [[iTermRestorableStateController sharedInstance] setSystemRestorationCallback:completionHandler
                                                                         windowIdentifier:guid];
        }
        DLog(@"return");
        return;
    }
    if ([[NSApplication sharedApplication] isRunningUnitTests]) {
        completionHandler(nil, nil);
        return;
    }
    if ([iTermAdvancedSettingsModel startDebugLoggingAutomatically]) {
        TurnOnDebugLoggingAutomatically();
    }

    DLog(@"Restore window with identifier %@", identifier);
    if ([[[NSBundle mainBundle] bundleIdentifier] containsString:@"applescript"]) {
        // Disable window restoration for DashTerm2ForAppleScriptTesting
        DLog(@"Abort because bundle ID contains applescript");
        completionHandler(nil, nil);
        return;
    }
    if ([[NSApplication sharedApplication] isRunningUnitTests]) {
        DLog(@"Abort because this is a unit test.");
        completionHandler(nil, nil);
        return;
    }
    [[[iTermApplication sharedApplication] delegate] willRestoreWindow];

    if ([iTermPreferences boolForKey:kPreferenceKeyOpenArrangementAtStartup]) {
        DLog(@"Abort because opening arrangement at startup");
        NSDictionary *arrangement = state.arrangement;
        if (arrangement) {
            [PseudoTerminal registerSessionsInArrangement:arrangement];
        }
        completionHandler(nil, nil);
        return;
    } else if ([iTermPreferences boolForKey:kPreferenceKeyOpenNoWindowsAtStartup]) {
        DLog(@"Abort because opening no windows at startup");
        completionHandler(nil, nil);
        return;
    }

    if (!queuedBlocks) {
        DLog(@"This is the first run of PseudoTerminalRestorer");
        queuedBlocks = [[NSMutableArray alloc] initWithCapacity:8];  // Queued restoration blocks
    }
    NSDictionary *arrangement = [state.arrangement retain];
    if (arrangement) {
        DLog(@"Have an arrangement");
        VoidBlock theBlock = ^{
            CFAbsoluteTime blockInnerStart = CFAbsoluteTimeGetCurrent();
            DLog(@"PseudoTerminalRestorer block running for id %@", identifier);
            DLog(@"Creating term");
            NSLog(@"[STARTUP] Creating PseudoTerminal from arrangement...");
            PseudoTerminal *term = [PseudoTerminal bareTerminalWithArrangement:arrangement
                                                      forceOpeningHotKeyWindow:NO];
            [arrangement autorelease];
            NSLog(@"[STARTUP] bareTerminalWithArrangement took %.3fs", CFAbsoluteTimeGetCurrent() - blockInnerStart);
            DLog(@"Create a new terminal %@", term);
            if (!term) {
                DLog(@"Failed to create term");
                completionHandler(nil, nil);
                return;
            }
            // We have to set the frame for fullscreen windows because the OS tries
            // to move it up 22 pixels for no good reason. Fullscreen, top, and
            // bottom windows will also end up broken if the screen resolution
            // has changed.
            // We MUST NOT set it for lion fullscreen because the OS knows what
            // to do with those, and we'd set it to some crazy wrong size.
            // Normal, top, and bottom windows take care of themselves.
            switch ([term windowType]) {
                case WINDOW_TYPE_TRADITIONAL_FULL_SCREEN:
                case WINDOW_TYPE_TOP_PERCENTAGE:
                case WINDOW_TYPE_CENTERED:
                case WINDOW_TYPE_TOP_CELLS:
                case WINDOW_TYPE_BOTTOM_PERCENTAGE:
                case WINDOW_TYPE_BOTTOM_CELLS:
                case WINDOW_TYPE_MAXIMIZED:
                case WINDOW_TYPE_COMPACT_MAXIMIZED:
                    DLog(@"Canonicalizing window frame");
                    [term performSelector:@selector(canonicalizeWindowFrame)
                               withObject:nil
                               afterDelay:0];
                    break;

                case WINDOW_TYPE_LEFT_PERCENTAGE:
                case WINDOW_TYPE_RIGHT_PERCENTAGE:
                case WINDOW_TYPE_NORMAL:
                case WINDOW_TYPE_LEFT_CELLS:
                case WINDOW_TYPE_NO_TITLE_BAR:
                case WINDOW_TYPE_COMPACT:
                case WINDOW_TYPE_RIGHT_CELLS:
                case WINDOW_TYPE_LION_FULL_SCREEN:
                case WINDOW_TYPE_ACCESSORY:
                    break;
            }

            DLog(@"Invoking completion handler");
            if (!term.togglingLionFullScreen) {
                DLog(@"In 10.10 or earlier, or 10.11 and a nonfullscreen window");
                term.restoringWindow = YES;
                completionHandler([term window], nil);
                term.restoringWindow = NO;
                DLog(@"Registering terminal window");
                [[iTermController sharedInstance] addTerminalWindow:term];
            } else {
                DLog(@"10.11 and this is a fullscreen window.");
                // Keep any more blocks from running until this window finishes entering fullscreen.
                gWaitingForFullScreen = YES;
                DLog(@"Set gWaitingForFullScreen=YES and set callback on %@", term);

                [completionHandler retain];
                term.didEnterLionFullscreen = ^(PseudoTerminal *theTerm) {
                    // Finished entering fullscreen. Run the completion handler
                    // and open more windows.
                    DLog(@"%@ finished entering fullscreen, running completion handler", theTerm);
                    term.restoringWindow = YES;
                    completionHandler([theTerm window], nil);
                    term.restoringWindow = NO;
                    [completionHandler release];
                    DLog(@"Registering terminal window");
                    [[iTermController sharedInstance] addTerminalWindow:term];
                    gWaitingForFullScreen = NO;
                    [PseudoTerminalRestorer runQueuedBlocks];
                };
            }
            DLog(@"Done running block for id %@", identifier);
        };
        DLog(@"Queueing block to run");
        [queuedBlocks addObject:[[theBlock copy] autorelease]];
        DLog(@"Returning");
    } else {
        DLog(@"Abort because no arrangement");
        completionHandler(nil, nil);
    }
}

+ (void)setRestorationCompletionBlock:(void(^)(void))completion {
    if (queuedBlocks) {
        [queuedBlocks addObject:[[completion copy] autorelease]];
    } else {
        completion();
    }
}

@end
