#import "RemotePreferencesTestHelpers.h"

#import "iTermRemotePreferences.h"
#import "iTermScriptingWindow.h"

NSData *ITRemotePreferencesLoadURL(iTermRemotePreferences *_Nonnull prefs,
                                   NSURL *_Nonnull url,
                                   BOOL respectingTimeout,
                                   NSError * _Nullable * _Nullable error) {
    return [prefs loadFromURL:url
     respectingTimeoutSetting:respectingTimeout
                        error:error];
}

iTermScriptingWindow *ITCreateScriptingWindow(NSWindow * _Nullable window) {
    return [iTermScriptingWindow scriptingWindowWithWindow:window];
}
