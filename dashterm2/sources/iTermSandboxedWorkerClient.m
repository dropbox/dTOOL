//
//  iTermSandboxedWorkerClient.m
//  DashTerm2
//
//  Created by Benedek Kozma on 2020. 12. 23..
//

#import "iTermSandboxedWorkerClient.h"
#import "DashTerm2SandboxedWorkerProtocol.h"
#import <os/lock.h>

// Performance optimization: os_unfair_lock has ~10x lower overhead than @synchronized

static os_unfair_lock sConnectionLock = OS_UNFAIR_LOCK_INIT;

@implementation iTermSandboxedWorkerClient

+ (NSXPCConnection *)connection {
    os_unfair_lock_lock(&sConnectionLock);
    static NSXPCConnection *sSandboxedWorkerConnection;
    if (sSandboxedWorkerConnection) {
        os_unfair_lock_unlock(&sConnectionLock);
        return sSandboxedWorkerConnection;
    }
    sSandboxedWorkerConnection =
        [[NSXPCConnection alloc] initWithServiceName:@"com.dashterm.dashterm2.sandboxed-worker"];
    if (!sSandboxedWorkerConnection) {
        os_unfair_lock_unlock(&sConnectionLock);
        return nil;
    }
    sSandboxedWorkerConnection.remoteObjectInterface =
        [NSXPCInterface interfaceWithProtocol:@protocol(DashTerm2SandboxedWorkerProtocol)];
    sSandboxedWorkerConnection.invalidationHandler = ^{
        os_unfair_lock_lock(&sConnectionLock);
        sSandboxedWorkerConnection = nil;
        os_unfair_lock_unlock(&sConnectionLock);
    };
    [sSandboxedWorkerConnection resume];
    os_unfair_lock_unlock(&sConnectionLock);
    return sSandboxedWorkerConnection;
}

+ (iTermImage *)performSynchronously:(void (^NS_NOESCAPE)(NSXPCConnection *connection,
                                                          void (^completion)(iTermImage *)))block {
    NSXPCConnection *connectionToService = [self connection];

    dispatch_group_t group = dispatch_group_create();
    dispatch_group_enter(group);

    __block iTermImage *result = nil;
    __block BOOL completed = NO;
    block(connectionToService, ^(iTermImage *image) {
        if (!completed) {
            completed = YES;
            result = image;
            dispatch_group_leave(group);
        }
    });

    // Use timeout to prevent blocking forever if XPC fails silently
    // 30 second timeout is generous for image decoding
    dispatch_time_t timeout = dispatch_time(DISPATCH_TIME_NOW, 30 * NSEC_PER_SEC);
    long waitResult = dispatch_group_wait(group, timeout);
    if (waitResult != 0) {
        // Timeout occurred - mark as completed to prevent late callback from crashing
        completed = YES;
        NSLog(@"iTermSandboxedWorkerClient: XPC call timed out after 30 seconds");
        return nil;
    }

    return result;
}

+ (iTermImage *)imageFromData:(NSData *)data {
    return [self performSynchronously:^(NSXPCConnection *connectionToService, void (^completion)(iTermImage *)) {
        [[connectionToService remoteObjectProxyWithErrorHandler:^(NSError *_Nonnull error) {
            NSLog(@"Failed to connect to service: %@", error);
            completion(nil);
        }] decodeImageFromData:data
                     withReply:^(iTermImage *_Nullable image) {
                         completion(image);
                     }];
    }];
}

+ (iTermImage *)imageFromSixelData:(NSData *)data {
    return [self performSynchronously:^(NSXPCConnection *connectionToService, void (^completion)(iTermImage *)) {
        [[connectionToService remoteObjectProxyWithErrorHandler:^(NSError *_Nonnull error) {
            NSLog(@"Failed to connect to service: %@", error);
            completion(nil);
        }] decodeImageFromSixelData:data
                          withReply:^(iTermImage *_Nullable image) {
                              completion(image);
                          }];
    }];
}

@end
