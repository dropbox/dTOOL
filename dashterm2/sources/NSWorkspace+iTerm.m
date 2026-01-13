//
//  NSWorkspace+iTerm.m
//  DashTerm2
//
//  Created by George Nachman on 5/11/15.
//
//

#import "NSWorkspace+iTerm.h"

#import "DebugLogging.h"
#import "DashTerm2SharedARC-Swift.h"
#import "iTermAdvancedSettingsModel.h"
#import "iTermMalloc.h"
#import "iTermWarning.h"
#import <UniformTypeIdentifiers/UniformTypeIdentifiers.h>
#include <string.h>

@implementation NSWorkspace (iTerm)

- (NSString *)temporaryFileNameWithPrefix:(NSString *)prefix suffix:(NSString *)suffix {
    NSString *template = [NSString stringWithFormat:@"%@XXXXXX%@", prefix ?: @"", suffix ?: @""];
    NSString *tempFileTemplate = [NSTemporaryDirectory() stringByAppendingPathComponent:template];
    const char *tempFileTemplateCString = [tempFileTemplate fileSystemRepresentation];
    const size_t templateLength = strlen(tempFileTemplateCString) + 1;
    char *tempFileNameCString = (char *)iTermMalloc(templateLength);
    memcpy(tempFileNameCString, tempFileTemplateCString, templateLength);
    int fileDescriptor = mkstemps(tempFileNameCString, suffix.length);

    if (fileDescriptor == -1) {
        XLog(@"mkstemps failed with template %s: %s", tempFileNameCString, strerror(errno));
        free(tempFileNameCString);
        return nil;
    }
    close(fileDescriptor);
    NSString *filename =
        [[NSFileManager defaultManager] stringWithFileSystemRepresentation:tempFileNameCString
                                                                    length:strlen(tempFileNameCString)];
    free(tempFileNameCString);
    return filename;
}

- (BOOL)it_securityAgentIsActive {
    NSRunningApplication *activeApplication = [[NSWorkspace sharedWorkspace] frontmostApplication];
    NSString *bundleIdentifier = activeApplication.bundleIdentifier;
    return [bundleIdentifier isEqualToString:@"com.apple.SecurityAgent"];
}

- (void)it_openURL:(NSURL *)url target:target style:(iTermOpenStyle)style window:(NSWindow *)window {
    [self it_openURL:url
               target:target
        configuration:[NSWorkspaceOpenConfiguration configuration]
                style:style
               window:window];
}

- (BOOL)it_urlIsWeb:(NSURL *)url {
    if (!url) {
        return NO;
    }
    if (![@[ @"http", @"https", @"ftp", @"file" ] containsObject:url.scheme]) {
        // The browser configured in advanced settings and the built-in browser don't handle this scheme.
        return NO;
    }
    return YES;
}

// A very weak check of whether the URL is openable by the built-in browser. This can be used to
// check if it's worth nagging the user to install the plugin to open this URL.
- (BOOL)it_localBrowserCouldHypotheticallyHandleURL:(NSURL *)url {
    if (![iTermAdvancedSettingsModel browserProfiles]) {
        return NO;
    }
    if ([url.scheme isEqualToString:@"file"] && [self it_localBrowserIsCompatibleWithFileURL:url]) {
        return YES;
    }
    if (![iTermBrowserMetadata.supportedSchemes containsObject:url.scheme]) {
        return NO;
    }
    return YES;
}

// Is this URL one that would open locally, or would request consent to open locally?
- (BOOL)it_urlIsConditionallyLocallyOpenable:(NSURL *)url {
    DLog(@"%@", url);
    if (![self it_urlIsWeb:url]) {
        return NO;
    }
    if (![self it_localBrowserCouldHypotheticallyHandleURL:url]) {
        return NO;
    }
    if ([self it_isDefaultBrowserForWebURL:url]) {
        return YES;
    }
    return [self it_tryToOpenURLLocallyDespiteNotBeingDefaultBrowser:url
                                                              target:nil
                                                       configuration:nil
                                                               style:iTermOpenStyleTab
                                                            testOnly:YES
                                                              window:nil];
}

- (BOOL)it_urlIsLocallyOpenableWithUpsell:(NSURL *)url {
    DLog(@"%@", url);
    if (![self it_urlIsWeb:url]) {
        return NO;
    }
    if (![self it_localBrowserCouldHypotheticallyHandleURL:url]) {
        return NO;
    }
    return [self it_tryToOpenURLLocallyDespiteNotBeingDefaultBrowser:url
                                                              target:nil
                                                       configuration:nil
                                                               style:iTermOpenStyleTab
                                                            testOnly:YES
                                                              window:nil];
}

// A high-confidence check of whether we'd open this URL ourselves.
// Assumes a web URL (see it_urlIsWeb:).
- (BOOL)it_isDefaultBrowserForWebURL:(NSURL *)url {
    if (![iTermBrowserGateway browserAllowedCheckingIfNot:YES]) {
        return NO;
    }
    NSString *bundleID = [iTermAdvancedSettingsModel browserBundleID];
    return ([bundleID isEqual:NSBundle.mainBundle.bundleIdentifier] || [self it_isDefaultAppForURL:url]);
}

- (void)it_openURL:(NSURL *)url
            target:(NSString *)target
     configuration:(NSWorkspaceOpenConfiguration *)configuration
             style:(iTermOpenStyle)style
            window:(NSWindow *)window {
    [self it_openURL:url target:target configuration:configuration style:style upsell:YES window:window];
}


- (BOOL)it_localBrowserIsCompatibleWithFileURL:(NSURL *)url {
    NSString *ext = url.pathExtension;
    if (ext.length == 0) {
        return NO;
    }

    UTType *type = [UTType typeWithFilenameExtension:ext];
    if (!type) {
        return NO;
    }

    // Core web formats
    return ([type conformsToType:UTTypeHTML] || [type conformsToType:UTTypeXML] ||
            [type conformsToType:[UTType typeWithIdentifier:@"public.svg-image"]] ||
            [type conformsToType:[UTType typeWithIdentifier:@"public.css"]] ||
            [type conformsToType:[UTType typeWithIdentifier:@"com.netscape.javascript-source"]] ||
            [type conformsToType:UTTypePDF] ||

            // Images
            [type conformsToType:UTTypePNG] || [type conformsToType:UTTypeJPEG] || [type conformsToType:UTTypeGIF] ||
            [type conformsToType:[UTType typeWithIdentifier:@"org.webmproject.webp"]] ||
            [type conformsToType:[UTType typeWithIdentifier:@"public.heic"]]);
}

- (BOOL)it_tryToOpenFileURLLocally:(NSURL *)url
                     configuration:(NSWorkspaceOpenConfiguration *)configuration
                             style:(iTermOpenStyle)style
                            upsell:(BOOL)upsell
                            window:(NSWindow *)window
                        completion:(void (^)(NSRunningApplication *app, NSError *error))completion {
    if (![self it_localBrowserIsCompatibleWithFileURL:url]) {
        return NO;
    }

    return [self it_tryToOpenURLLocally:url
                                 target:nil
                          configuration:configuration
                                  style:style
                                 upsell:upsell
                                 window:window];
}

- (BOOL)it_openIfNonWebURL:(NSURL *)url
             configuration:(NSWorkspaceOpenConfiguration *)configuration
                     style:(iTermOpenStyle)style
                    upsell:(BOOL)upsell
                    window:(NSWindow *)window
                completion:(void (^)(NSRunningApplication *app, NSError *error))completion {
    if ([@[ @"http", @"https", @"ftp" ] containsObject:url.scheme]) {
        return NO;
    }
    if ([url.scheme isEqualToString:@"file"]) {
        // Some files could usefully be opened locally, like PDFs.
        if ([self it_tryToOpenFileURLLocally:url
                               configuration:configuration
                                       style:style
                                      upsell:upsell
                                      window:window
                                  completion:completion]) {
            return YES;
        }
    }
    DLog(@"Non-web scheme");
    [self openURL:url
            configuration:configuration
        completionHandler:^(NSRunningApplication *app, NSError *error) {
            if (completion) {
                dispatch_async(dispatch_get_main_queue(), ^{
                    completion(app, error);
                });
            }
        }];
    return YES;
}

- (void)it_openURL:(NSURL *)url
            target:(NSString *)target
     configuration:(NSWorkspaceOpenConfiguration *)configuration
             style:(iTermOpenStyle)style
            upsell:(BOOL)upsell
            window:(NSWindow *)window {
    DLog(@"%@", url);
    if (!url) {
        return;
    }
    // BUG-1163/1175: Block dangerous URL schemes
    if ([self it_isSchemeBlocked:url]) {
        DLog(@"Blocked URL with dangerous scheme: %@", url);
        return;
    }
    if ([self it_openIfNonWebURL:url
                   configuration:configuration
                           style:style
                          upsell:upsell
                          window:window
                      completion:nil]) {
        return;
    }

    if ([self it_tryToOpenURLLocally:url
                              target:target
                       configuration:configuration
                               style:style
                              upsell:upsell
                              window:window]) {
        return;
    }

    [self it_openURLWithDefaultBrowser:url
                         configuration:configuration
                            completion:^(NSRunningApplication *app, NSError *error){
                            }];
}

- (BOOL)it_tryToOpenURLLocally:(NSURL *)url
                        target:(NSString *)target
                 configuration:(NSWorkspaceOpenConfiguration *)configuration
                         style:(iTermOpenStyle)style
                        upsell:(BOOL)upsell
                        window:(NSWindow *)window {
    if (!upsell && ![iTermBrowserGateway browserAllowedCheckingIfNot:YES]) {
        return NO;
    }
    if (![self it_localBrowserCouldHypotheticallyHandleURL:url]) {
        return NO;
    }
    if ([self it_isDefaultBrowserForWebURL:url]) {
        // We are the default app. Skip all the machinery and open it directly.
        if ([self it_openURLLocally:url target:target configuration:configuration openStyle:style]) {
            return YES;
        }
    }
    if (upsell) {
        // This feature is new and this is the main way people will discover it. Sorry for the annoyance :(
        if ([self it_tryToOpenURLLocallyDespiteNotBeingDefaultBrowser:url
                                                               target:target
                                                        configuration:configuration
                                                                style:style
                                                             testOnly:NO
                                                               window:window]) {
            return YES;
        }
    }
    return NO;
}

- (void)it_openURLWithDefaultBrowser:(NSURL *)url
                       configuration:(NSWorkspaceOpenConfiguration *)configuration
                          completion:(void (^)(NSRunningApplication *app, NSError *error))completion {
    NSString *bundleID = [iTermAdvancedSettingsModel browserBundleID];
    if ([bundleID stringByTrimmingCharactersInSet:[NSCharacterSet whitespaceCharacterSet]].length == 0) {
        // No custom app configured in advanced settings so use the systemwide default.
        DLog(@"Empty custom bundle ID “%@”", bundleID);
        [self openURL:url
                configuration:configuration
            completionHandler:^(NSRunningApplication *app, NSError *error) {
                dispatch_async(dispatch_get_main_queue(), ^{
                    completion(app, error);
                });
            }];
        return;
    }
    NSURL *appURL = [self URLForApplicationWithBundleIdentifier:bundleID];
    if (!appURL) {
        // The custom app configured in advanced settings isn't installed. Use the sytemwide default.
        DLog(@"No url for bundle ID %@", bundleID);
        [self openURL:url
                configuration:configuration
            completionHandler:^(NSRunningApplication *app, NSError *error) {
                dispatch_async(dispatch_get_main_queue(), ^{
                    completion(app, error);
                });
            }];
        return;
    }

    // Open with the advanced-settings-configured default browser.
    DLog(@"Open %@ with %@", url, appURL);
    [self openURLs:@[ url ]
        withApplicationAtURL:appURL
               configuration:configuration
           completionHandler:^(NSRunningApplication *app, NSError *error) {
               if (error) {
                   // That didn't work so just use the default browser
                   return [self openURL:url configuration:configuration completionHandler:completion];
               } else {
                   dispatch_async(dispatch_get_main_queue(), ^{
                       completion(app, error);
                   });
               }
           }];
}

- (void)it_asyncOpenURL:(NSURL *)url
                 target:(NSString *)target
          configuration:(NSWorkspaceOpenConfiguration *)configuration
                  style:(iTermOpenStyle)style
                 upsell:(BOOL)upsell
                 window:(NSWindow *)window
             completion:(void (^)(NSRunningApplication *app, NSError *error))completion {
    DLog(@"%@", url);
    if (!url) {
        return;
    }
    // BUG-1163/1175: Block dangerous URL schemes
    if ([self it_isSchemeBlocked:url]) {
        DLog(@"Blocked URL with dangerous scheme: %@", url);
        if (completion) {
            NSError *error = [NSError errorWithDomain:@"NSWorkspace+iTerm"
                                                 code:1
                                             userInfo:@{NSLocalizedDescriptionKey : @"Blocked dangerous URL scheme"}];
            completion(nil, error);
        }
        return;
    }
    if ([self it_openIfNonWebURL:url
                   configuration:configuration
                           style:style
                          upsell:upsell
                          window:window
                      completion:completion]) {
        return;
    }
    if ([self it_tryToOpenURLLocally:url
                              target:target
                       configuration:configuration
                               style:style
                              upsell:upsell
                              window:window]) {
        completion([NSRunningApplication currentApplication], nil);
        return;
    }
    [self it_openURLWithDefaultBrowser:url configuration:configuration completion:completion];
}

- (BOOL)it_isDefaultAppForURL:(NSURL *)url {
    if (!url) {
        return NO;
    }

    // Ask NSWorkspace for the app that would open it
    NSURL *appURL = [[NSWorkspace sharedWorkspace] URLForApplicationToOpenURL:url];

    // Extract its bundle ID
    if (appURL != nil) {
        NSBundle *bundle = [NSBundle bundleWithURL:appURL];
        NSString *bundleID = bundle.bundleIdentifier;
        return [bundleID isEqual:NSBundle.mainBundle.bundleIdentifier];
    }
    return NO;
}

// In test-only mode, returns whether the URL could be opened locally if the
// user were hypothetically to consent should consent be needed.
- (BOOL)it_tryToOpenURLLocallyDespiteNotBeingDefaultBrowser:(NSURL *)url
                                                     target:(NSString *)target
                                              configuration:(NSWorkspaceOpenConfiguration *)configuration
                                                      style:(iTermOpenStyle)style
                                                   testOnly:(BOOL)testOnly
                                                     window:(NSWindow *)window {
    if (![iTermBrowserGateway browserAllowedCheckingIfNot:YES]) {
        if ([iTermBrowserGateway shouldOfferPlugin]) {
            if (testOnly) {
                return [iTermBrowserGateway wouldUpsell];
            }
            switch ([iTermBrowserGateway upsell]) {
                case iTermTriStateTrue:
                    // User is downloading plugin. Return yes and you'll have to try again.
                    return YES;
                case iTermTriStateFalse:
                    // Use system browser.
                    return NO;
                case iTermTriStateOther:
                    // Cancel.
                    return YES;
            }
        } else {
            // Plugin not available, just use system browser.
            return NO;
        }
    }
    NSString *identifier;
    const BOOL isFileURL = [url.scheme isEqualToString:@"file"];
    if (isFileURL) {
        identifier = @"NoSyncOpenLinksInAppForFileURL";
    } else {
        identifier = @"NoSyncOpenLinksInApp";
    }
    if (testOnly) {
        NSNumber *n = [iTermWarning conditionalSavedSelectionForIdentifier:identifier];
        if (n) {
            return n.intValue == kiTermWarningSelection1;
        }
        return YES;
    }
    BOOL consent = NO;
    switch (style) {
        case iTermOpenStyleWindow:
        case iTermOpenStyleTab:
            if (isFileURL) {
                consent =
                    ([iTermWarning showWarningWithTitle:@"DashTerm2 can display files like this in its built-in web "
                                                        @"browser! Would you like to open this link in DashTerm2?"
                                                actions:@[ @"Use Default App", @"Open in DashTerm2" ]
                                              accessory:nil
                                             identifier:identifier
                                            silenceable:kiTermWarningTypePermanentlySilenceable
                                                heading:@"Open in DashTerm2?"
                                                 window:window] == kiTermWarningSelection1);
            } else {
                consent = ([iTermWarning
                               showWarningWithTitle:
                                   @"DashTerm2 can display web pages! Would you like to open this link in DashTerm2?"
                                            actions:@[ @"Use Default Browser", @"Open in DashTerm2" ]
                                          accessory:nil
                                         identifier:identifier
                                        silenceable:kiTermWarningTypePermanentlySilenceable
                                            heading:@"Open in DashTerm2?"
                                             window:window] == kiTermWarningSelection1);
            }
            break;
        case iTermOpenStyleVerticalSplit:
        case iTermOpenStyleHorizontalSplit:
            // Implied consent - no way to open in a split otherwise!
            consent = YES;
            break;
    }
    if (!consent) {
        return NO;
    }
    return [self it_openURLLocally:url target:target configuration:configuration openStyle:style];
}

- (BOOL)it_openURLLocally:(NSURL *)url
                   target:(NSString *)target
            configuration:(NSWorkspaceOpenConfiguration *)configuration
                openStyle:(iTermOpenStyle)openStyle {
    return [[iTermController sharedInstance] openURL:url
                                              target:target
                                           openStyle:openStyle
                                              select:configuration.activates];
}

// BUG-1169: Use a serial queue for thread-safe token operations
static NSMutableSet<NSString *> *urlTokens;
static dispatch_queue_t urlTokensQueue;

- (NSString *)it_newToken {
    static dispatch_once_t onceToken;
    dispatch_once(&onceToken, ^{
        // URL tokens are typically limited to a few per session
        urlTokens = [NSMutableSet setWithCapacity:16];
        urlTokensQueue = dispatch_queue_create("com.dashterm2.urlTokens", DISPATCH_QUEUE_SERIAL);
    });
    NSString *token = [[NSUUID UUID] UUIDString];
    dispatch_sync(urlTokensQueue, ^{
        [urlTokens addObject:token];
    });
    return token;
}

- (BOOL)it_checkToken:(NSString *)token {
    __block BOOL result = NO;
    dispatch_sync(urlTokensQueue, ^{
        if ([urlTokens containsObject:token]) {
            [urlTokens removeObject:token];
            result = YES;
        }
    });
    return result;
}

// BUG-1163/1175: Block dangerous URL schemes that could execute code or access sensitive data
- (BOOL)it_isSchemeBlocked:(NSURL *)url {
    if (!url || !url.scheme) {
        return YES;
    }
    NSString *scheme = [url.scheme lowercaseString];
    // Blocklist of dangerous schemes that should not be opened automatically
    static NSSet<NSString *> *blockedSchemes;
    static dispatch_once_t onceToken;
    dispatch_once(&onceToken, ^{
        blockedSchemes = [NSSet setWithArray:@[
            // Script execution
            @"javascript", @"applescript", @"osascript",

            // Data URIs can contain executable content
            @"data",

            // Shell-related
            @"shell", @"bash", @"sh", @"zsh",

            // Potential code execution
            @"vbscript", @"jscript",

            // Telephony/messaging (could cause unexpected charges)
            @"tel", @"sms", @"facetime", @"facetime-audio",

            // System app handlers that could be exploited
            @"itms-apps", @"itms-appss", @"macappstore", @"macappstores",

            // VNC/remote control
            @"vnc", @"rdp",
            @"ssh", // We handle ssh:// specially elsewhere with proper validation

            // Blob URLs (can reference local sensitive data)
            @"blob"
        ]];
    });

    if ([blockedSchemes containsObject:scheme]) {
        ELog(@"Blocked dangerous URL scheme: %@", scheme);
        return YES;
    }
    return NO;
}

- (void)it_revealInFinder:(NSString *)path {
    NSURL *finderURL = [[NSWorkspace sharedWorkspace] URLForApplicationWithBundleIdentifier:@"com.apple.finder"];
    if (!finderURL) {
        DLog(@"Can't find Finder");
        return;
    }
    [[NSWorkspace sharedWorkspace] openURLs:@[ [NSURL fileURLWithPath:path] ]
                       withApplicationAtURL:finderURL
                              configuration:[NSWorkspaceOpenConfiguration configuration]
                          completionHandler:nil];
}

@end
