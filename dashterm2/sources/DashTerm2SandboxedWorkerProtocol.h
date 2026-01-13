//
//  DashTerm2SandboxedWorkerProtocol.h
//  DashTerm2SandboxedWorker
//
//  Created by Benedek Kozma on 2020. 12. 23..
//

#import <Foundation/Foundation.h>

@class iTermImage;

@protocol DashTerm2SandboxedWorkerProtocol

- (void)decodeImageFromData:(NSData * _Nonnull)imageData withReply:(void (^_Nonnull)(iTermImage * _Nullable))reply;
- (void)decodeImageFromSixelData:(NSData * _Nonnull)imageData withReply:(void (^_Nonnull)(iTermImage * _Nullable))reply;

@end
