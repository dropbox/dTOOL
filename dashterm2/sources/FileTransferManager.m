//
//  DownloadManager.m
//  iTerm
//
//  Created by George Nachman on 12/21/13.
//
//

#import "FileTransferManager.h"
#import "iTermApplicationDelegate.h"
#import "iTermPasswordManagerWindowController.h"
#import "NSArray+iTerm.h"
#import "TransferrableFileMenuItemViewController.h"

// Finished downloads will be automatically removed from the downloads menu after this number of
// seconds.
static const NSTimeInterval kMaximumTimeToKeepFinishedDownload = 24 * 60 * 60;

@interface FileTransferManager () <iTermPasswordManagerDelegate>
@property (nonatomic, retain) NSMutableArray *files;
@end

@implementation FileTransferManager {
    NSMutableArray *_viewControllers;
    NSTimer *_timer; // cleanUpMenus timer. weak reference.
    iTermPasswordManagerWindowController *_passwordManagerWindowController;
    void (^_passwordCompletion)(NSString *password);
    // BUG-3087: Hold animating windows until fade completes instead of using performSelector:@selector(release)
    NSMutableArray<NSWindow *> *_animatingWindows;
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
    self = [super init];
    if (self) {
        _files = [[NSMutableArray alloc] initWithCapacity:8];  // Typical concurrent file transfers
        _viewControllers = [[NSMutableArray alloc] initWithCapacity:8];  // One per file
        _animatingWindows = [[NSMutableArray alloc] initWithCapacity:4];  // Windows in fade animation
        _timer = [NSTimer scheduledTimerWithTimeInterval:60 * 10
                                                  target:self
                                                selector:@selector(cleanUpMenus)
                                                userInfo:nil
                                                 repeats:YES];
    }
    return self;
}

- (void)dealloc {
    // BUG-3132: Cancel scheduled timer to prevent crash if manager is deallocated
    [_timer invalidate];
    _timer = nil;
    // BUG-3133: Cancel delayed selectors (fadeWindowOut:, finishWindowFadeOut:) targeting self
    [NSObject cancelPreviousPerformRequestsWithTarget:self];
    [[NSNotificationCenter defaultCenter] removeObserver:self];
    [_files release];
    [_viewControllers release];
    [_animatingWindows release];
    [super dealloc];
}

- (void)cleanUpMenus {
    // Typically 0-5 controllers to remove at a time
    NSMutableArray *controllersToRemove = [NSMutableArray arrayWithCapacity:4];
    for (TransferrableFileMenuItemViewController *controller in _viewControllers) {
        if ([controller timeSinceLastStatusChange] > kMaximumTimeToKeepFinishedDownload &&
            controller.transferrableFile.status != kTransferrableFileStatusStarting &&
            controller.transferrableFile.status != kTransferrableFileStatusTransferring &&
            controller.transferrableFile.status != kTransferrableFileStatusCancelling) {
            [controller.view.enclosingMenuItem.menu removeItem:controller.view.enclosingMenuItem];
            [controllersToRemove addObject:controller];
        }
    }

    for (TransferrableFileMenuItemViewController *controller in controllersToRemove) {
        [_viewControllers removeObject:controller];
    }
}

- (NSMenu *)downloadsMenu {
    iTermApplicationDelegate *ad = [iTermApplication.sharedApplication delegate];
    return [ad downloadsMenu];
}

- (NSMenu *)uploadsMenu {
    iTermApplicationDelegate *ad = [iTermApplication.sharedApplication delegate];
    return [ad uploadsMenu];
}

- (void)animateImage:(NSImage *)image
            intoMenu:(AXUIElementRef)menuElement
           fromPoint:(NSPoint)point
            onScreen:(NSScreen *)screen {
    if (!menuElement) {
        return;
    }
    CFTypeRef temp = NULL;
    AXUIElementCopyAttributeValue(menuElement, kAXPositionAttribute, (CFTypeRef *)&temp);
    if (!temp) {
        return;
    }

    CGPoint position;
    AXValueGetValue(temp, kAXValueCGPointType, &position);
    CFRelease(temp);
    CFRelease(menuElement);

    NSWindow *window =
        [[NSWindow alloc] initWithContentRect:NSMakeRect(point.x, point.y, image.size.width, image.size.height)
                                    styleMask:NSWindowStyleMaskBorderless
                                      backing:NSBackingStoreBuffered
                                        defer:NO];
    NSImageView *imageView =
        [[[NSImageView alloc] initWithFrame:NSMakeRect(0, 0, image.size.width, image.size.height)] autorelease];
    imageView.image = image;
    window.contentView = imageView;
    [window makeKeyAndOrderFront:nil];
    [window setLevel:NSMainMenuWindowLevel];

    // Todo: deal with multiple screens, mavericks
    const CGFloat menuBarHeight = [[[NSApplication sharedApplication] mainMenu] menuBarHeight];
    position.y = screen.frame.size.height - position.y - menuBarHeight;

    [window.animator setFrame:NSMakeRect(position.x, position.y, window.frame.size.width, window.frame.size.height)
                      display:YES];
    // BUG-3087: Hold reference in array instead of relying on performSelector:@selector(release)
    [_animatingWindows addObject:window];
    [window release]; // Transfer ownership to the array
    [self performSelector:@selector(fadeWindowOut:)
               withObject:window
               afterDelay:[[NSAnimationContext currentContext] duration]];
}

- (void)animateImage:(NSImage *)image intoDownloadsMenuFromPoint:(NSPoint)point onScreen:(NSScreen *)screen {
    AXUIElementRef menuElement = [self downloadsMenuElement];
    [self animateImage:image intoMenu:menuElement fromPoint:point onScreen:screen];
}

- (void)fadeWindowOut:(NSWindow *)window {
    [window.animator setAlphaValue:0];
    // BUG-3087: Use proper memory management - schedule removal from array after animation
    [self performSelector:@selector(finishWindowFadeOut:)
               withObject:window
               afterDelay:[[NSAnimationContext currentContext] duration]];
}

// BUG-3087: Remove window from array after fade animation completes, releasing it properly
- (void)finishWindowFadeOut:(NSWindow *)window {
    [window orderOut:nil];
    [_animatingWindows removeObject:window];
}

- (AXUIElementRef)downloadsMenuElement {
    return [self menuElementNamed:@"Downloads"];
}

- (AXUIElementRef)uploadsMenuElement {
    return [self menuElementNamed:@"Uploads"];
}

- (AXUIElementRef)menuElementNamed:(NSString *)menuName {
    AXUIElementRef appElement = AXUIElementCreateApplication(getpid());
    AXUIElementRef menuBar;
    AXError error = AXUIElementCopyAttributeValue(appElement, kAXMenuBarAttribute, (CFTypeRef *)&menuBar);
    CFRelease(appElement);
    if (error) {
        return NULL;
    }

    CFIndex count = -1;
    error = AXUIElementGetAttributeValueCount(menuBar, kAXChildrenAttribute, &count);
    if (error) {
        CFRelease(menuBar);
        return NULL;
    }

    CFArrayRef children = NULL;
    // BUG-3037: AXUIElementCopyAttributeValues uses "Copy" naming, so we own the array
    error = AXUIElementCopyAttributeValues(menuBar, kAXChildrenAttribute, 0, count, &children);
    if (error) {
        CFRelease(menuBar);
        return NULL;
    }

    AXUIElementRef result = NULL;
    for (CFIndex i = 0; i < CFArrayGetCount(children); i++) {
        AXUIElementRef element = (AXUIElementRef)CFArrayGetValueAtIndex(children, i);
        CFTypeRef title = NULL;
        error = AXUIElementCopyAttributeValue(element, kAXTitleAttribute, &title);
        if (error) {
            continue;
        }
        BOOL found = [(__bridge NSString *)title isEqualToString:menuName];
        CFRelease(title);
        if (found) {
            // Retain the element since we're going to release children
            result = (AXUIElementRef)CFRetain(element);
            break;
        }
    }

    // BUG-3037: Release the children array we own
    CFRelease(children);
    // BUG-3038: Release menuBar in all code paths
    CFRelease(menuBar);
    return result;
}

- (void)openDownloadsMenu {
    AXUIElementRef menuElement = [self downloadsMenuElement];
    if (menuElement) {
        AXUIElementPerformAction(menuElement, kAXPressAction);
        CFRelease(menuElement);
    }
}

- (void)openUploadsMenu {
    AXUIElementRef menuElement = [self uploadsMenuElement];
    if (menuElement) {
        AXUIElementPerformAction(menuElement, kAXPressAction);
        CFRelease(menuElement);
    }
}

- (void)removeAllDownloads {
    NSArray *removableStatuses = @[
        @(kTransferrableFileStatusFinishedSuccessfully), @(kTransferrableFileStatusFinishedWithError),
        @(kTransferrableFileStatusCancelled)
    ];

    NSArray *downloads =
        [_viewControllers filteredArrayUsingBlock:^BOOL(TransferrableFileMenuItemViewController *anObject) {
            return anObject.transferrableFile.isDownloading &&
                   [removableStatuses containsObject:@(anObject.transferrableFile.status)];
        }];
    for (TransferrableFileMenuItemViewController *controller in downloads) {
        [self removeItem:controller];
    }
}

- (void)removeAllUploads {
    NSArray *removableStatuses = @[
        @(kTransferrableFileStatusFinishedSuccessfully), @(kTransferrableFileStatusFinishedWithError),
        @(kTransferrableFileStatusCancelled)
    ];
    NSArray *downloads =
        [_viewControllers filteredArrayUsingBlock:^BOOL(TransferrableFileMenuItemViewController *anObject) {
            return !anObject.transferrableFile.isDownloading &&
                   [removableStatuses containsObject:@(anObject.transferrableFile.status)];
        }];
    for (TransferrableFileMenuItemViewController *controller in downloads) {
        [self removeItem:controller];
    }
}

- (TransferrableFileMenuItemViewController *)viewControllerForTransferrableFile:(TransferrableFile *)transferrableFile {
    for (TransferrableFileMenuItemViewController *controller in _viewControllers) {
        if (controller.transferrableFile == transferrableFile) {
            return controller;
        }
    }
    return nil;
}

- (NSMenuItem *)menuItemForTransferrableFile:(TransferrableFile *)transferrableFile {
    NSMenuItem *item = [[NSMenuItem alloc] init];
    TransferrableFileMenuItemViewController *controller =
        [[[TransferrableFileMenuItemViewController alloc] initWithTransferrableFile:transferrableFile] autorelease];
    [_viewControllers addObject:controller];
    item.view = [controller view];
    [item setEnabled:YES];
    [item setTarget:controller];
    [item setAction:@selector(itemSelected:)];

    NSMenu *submenu = [[[NSMenu alloc] init] autorelease];
    NSMenuItem *subItem = [[[NSMenuItem alloc] initWithTitle:@"Stop" action:@selector(stop:)
                                               keyEquivalent:@""] autorelease];
    [subItem setTarget:controller];
    [submenu addItem:subItem];
    controller.stopSubItem = subItem;

    if (transferrableFile.isDownloading) {
        subItem = [[[NSMenuItem alloc] initWithTitle:@"Show in Finder"
                                              action:@selector(showInFinder:)
                                       keyEquivalent:@""] autorelease];
        [subItem setTarget:controller];
        [submenu addItem:subItem];
        controller.showInFinderSubItem = subItem;
    }

    subItem = [[[NSMenuItem alloc] initWithTitle:@"Remove from List"
                                          action:@selector(removeFromList:)
                                   keyEquivalent:@""] autorelease];
    [subItem setTarget:controller];
    [submenu addItem:subItem];
    controller.removeFromListSubItem = subItem;

    if (transferrableFile.isDownloading) {
        subItem = [[[NSMenuItem alloc] initWithTitle:@"Open" action:@selector(open:) keyEquivalent:@""] autorelease];
        [subItem setTarget:controller];
        [submenu addItem:subItem];
        controller.openSubItem = subItem;
    }

    subItem = [[[NSMenuItem alloc] initWithTitle:@"Get Info" action:@selector(getInfo:) keyEquivalent:@""] autorelease];
    [subItem setTarget:controller];
    [submenu addItem:subItem];
    controller.openSubItem = subItem;

    item.submenu = submenu;

    [controller update];
    return item;
}

- (NSMenu *)menuForFile:(TransferrableFile *)transferrableFile {
    if (transferrableFile.isDownloading) {
        return [self downloadsMenu];
    } else {
        return [self uploadsMenu];
    }
}

- (void)transferrableFileDidStartTransfer:(TransferrableFile *)transferrableFile {
    DLog(@"Transfer started");
    [[self menuForFile:transferrableFile] addItem:[self menuItemForTransferrableFile:transferrableFile]];
    TransferrableFileMenuItemViewController *controller = [self viewControllerForTransferrableFile:transferrableFile];
    [controller update];
}

// Number of bytes transferred has changed or total size has been discovered.
- (void)transferrableFileProgressDidChange:(TransferrableFile *)transferrableFile {
    TransferrableFileMenuItemViewController *controller = [self viewControllerForTransferrableFile:transferrableFile];
    [controller update];
}

// |error| is nil on success
- (void)transferrableFile:(TransferrableFile *)transferrableFile didFinishTransmissionWithError:(NSError *)error {
    if (error) {
        [transferrableFile
            didFailWithError:error.localizedDescription ?: @"File transfer failed with an unknown error"];
    } else {
        transferrableFile.status = kTransferrableFileStatusFinishedSuccessfully;
    }
    DLog(@"Transfer finished. error=%@", error);

    TransferrableFileMenuItemViewController *controller = [self viewControllerForTransferrableFile:transferrableFile];
    [controller update];
}

- (void)transferrableFileWillStop:(TransferrableFile *)transferrableFile {
    DLog(@"file transfer stop requested");
    transferrableFile.status = kTransferrableFileStatusCancelling;
    TransferrableFileMenuItemViewController *controller = [self viewControllerForTransferrableFile:transferrableFile];
    [controller update];
}

- (void)transferrableFileDidStopTransfer:(TransferrableFile *)transferrableFile {
    DLog(@"file transfer stopped");
    transferrableFile.status = kTransferrableFileStatusCancelled;
    TransferrableFileMenuItemViewController *controller = [self viewControllerForTransferrableFile:transferrableFile];
    [controller update];
}


// Shows a modal alert with the text in |prompt| and a freeform keyboard input. Returns the
// value entered.
- (void)transferrableFile:(TransferrableFile *)transferrableFile
        interactivePrompt:(NSString *)prompt
               completion:(void (^)(NSString *password))completion {
    NSString *text = [NSString stringWithFormat:@"Authenticate %@", transferrableFile.authRequestor];
    NSAlert *alert = [[[NSAlert alloc] init] autorelease];
    alert.messageText = text;
    alert.informativeText = [NSString stringWithFormat:@"Please enter the %@ for %@ to begin %@.", prompt,
                                                       transferrableFile.authRequestor, transferrableFile.protocolName];
    [alert addButtonWithTitle:@"OK"];
    [alert addButtonWithTitle:@"Cancel"];
    [alert addButtonWithTitle:@"Password Manager…"];

    NSSecureTextField *input = [[[NSSecureTextField alloc] initWithFrame:NSMakeRect(0, 0, 200, 24)] autorelease];
    [input setStringValue:@""];
    [alert setAccessoryView:input];
    [alert layout];
    [[alert window] makeFirstResponder:input];
    NSInteger button = [alert runModal];
    if (button == NSAlertFirstButtonReturn) {
        [input validateEditing];
        completion([input stringValue]);
    } else if (button == NSAlertThirdButtonReturn) {
        [self asynchronouslySelectPasswordFromPasswordManager:completion];
    } else {
        completion(nil);
    }
}

- (void)asynchronouslySelectPasswordFromPasswordManager:(void (^)(NSString *password))completion {
    if (_passwordManagerWindowController) {
        completion(nil);
        return;
    }
    _passwordManagerWindowController = [[iTermPasswordManagerWindowController alloc] init];
    _passwordManagerWindowController.delegate = self;
    [[_passwordManagerWindowController window] makeKeyAndOrderFront:nil];
    _passwordCompletion = [completion copy];
}

// Shows message, returns YES if OK, NO if Cancel
- (BOOL)transferrableFile:(TransferrableFile *)transferrableFile
                    title:(NSString *)title
           confirmMessage:(NSString *)message {
    NSAlert *alert = [[[NSAlert alloc] init] autorelease];
    alert.messageText = title;
    alert.informativeText = message;
    [alert addButtonWithTitle:@"OK"];
    [alert addButtonWithTitle:@"Cancel"];

    [alert layout];
    NSInteger button = [alert runModal];
    return (button == NSAlertFirstButtonReturn);
}

- (void)removeItem:(TransferrableFileMenuItemViewController *)viewController {
    NSMenuItem *item = [viewController.view enclosingMenuItem];
    [[item menu] removeItem:item];
    [_viewControllers removeObject:viewController];
}

#pragma mark - iTermPasswordManagerDelegate

- (BOOL)iTermPasswordManagerCanEnterPassword {
    return YES;
}

- (void)iTermPasswordManagerEnterPassword:(NSString *)password broadcast:(BOOL)broadcast {
    if (_passwordCompletion) {
        _passwordCompletion(password);
    }
    _passwordCompletion = nil;
    _passwordManagerWindowController.delegate = nil;
    _passwordManagerWindowController = nil;
}

- (void)iTermPasswordManagerEnterUserName:(NSString *)username broadcast:(BOOL)broadcast {
    // BUG-407: FileTransferManager doesn't support username entry - return without action
    // This method is part of the iTermPasswordManagerDelegate protocol but
    // FileTransferManager only needs password, not username support
}

- (BOOL)iTermPasswordManagerCanEnterUserName {
    return NO;
}

- (BOOL)iTermPasswordManagerCanBroadcast {
    return NO;
}

- (void)iTermPasswordManagerDidClose {
    _passwordCompletion = nil;
    _passwordManagerWindowController.delegate = nil;
    _passwordManagerWindowController = nil;
}

@end
