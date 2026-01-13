//
//  iTermRecordingCodec.h
//  DashTerm2
//
//  Created by George Nachman on 8/8/18.
//

#import <Cocoa/Cocoa.h>

@class PTYSession;

@interface iTermRecordingCodec : NSObject

+ (void)loadRecording;
+ (void)loadRecording:(NSURL *)url;
+ (void)exportRecording:(PTYSession *)session window:(NSWindow *)window;
+ (void)exportRecording:(PTYSession *)session from:(long long)from to:(long long)to window:(NSWindow *)window;

@end
