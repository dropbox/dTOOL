#import <Foundation/Foundation.h>
#import "iTermRemotePreferences.h"
#import "iTermScriptingWindow.h"

@interface iTermRemotePreferences (Testing)
- (nullable NSData *)loadFromURL:(NSURL *)url
        respectingTimeoutSetting:(BOOL)respectingTimeoutSetting
                               error:(NSError * _Nullable * _Nullable)error;
@end

FOUNDATION_EXTERN NSData * _Nullable ITRemotePreferencesLoadURL(iTermRemotePreferences *_Nonnull prefs,
                                                               NSURL *_Nonnull url,
                                                               BOOL respectingTimeout,
                                                               NSError * _Nullable * _Nullable error);

FOUNDATION_EXTERN iTermScriptingWindow * _Nullable ITCreateScriptingWindow(NSWindow * _Nullable window);
