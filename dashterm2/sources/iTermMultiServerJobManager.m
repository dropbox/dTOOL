//
//  iTermMultiServerJobManager.m
//  DashTerm2
//
//  Created by George Nachman on 11/25/19.
//

#import "iTermMultiServerJobManager.h"

#import "DebugLogging.h"
#import "iTermAdvancedSettingsModel.h"
#import "iTermFileDescriptorMultiClient.h"
#import "iTermMultiServerConnection.h"
#import "iTermNotificationCenter.h"
#import "iTermProcessCache.h"
#import "NSArray+iTerm.h"
#import "NSFileManager+iTerm.h"
#import "NSObject+iTerm.h"
#import "TaskNotifier.h"

NSString *const iTermMultiServerRestorationKeyType = @"Type";
NSString *const iTermMultiServerRestorationKeyVersion = @"Version";
NSString *const iTermMultiServerRestorationKeySocket = @"Socket";
NSString *const iTermMultiServerRestorationKeyChildPID = @"Child PID";

// Value for iTermMultiServerRestorationKeyType
static NSString *const iTermMultiServerRestorationType = @"multiserver";
static const int iTermMultiServerMaximumSupportedRestorationIdentifierVersion = 1;

@interface iTermMultiServerJobManagerPartialAttachment : NSObject <iTermJobManagerPartialResult>
@property (nonatomic) BOOL shouldRegister;
@property (nonatomic) BOOL attached;
@property (nonatomic) pid_t pid;
@property (nonatomic) BOOL brokenPipe;
@end

@implementation iTermMultiServerJobManagerPartialAttachment
@end

@class iTermMultiServerJobManagerState;
@interface iTermMultiServerJobManagerState : iTermSynchronizedState <iTermMultiServerJobManagerState *>
@property (nonatomic, strong) iTermMultiServerConnection *conn;
@property (nonatomic, strong) iTermFileDescriptorMultiClientChild *child;
@end

@implementation iTermMultiServerJobManagerState
@end

@interface iTermMultiServerJobManager ()
@property (atomic, strong, readwrite) iTermThread<iTermMultiServerJobManagerState *> *thread;
@end

@implementation iTermMultiServerJobManager

- (dispatch_queue_t)queue {
    return _thread.queue;
}

+ (BOOL)available {
    return [iTermAdvancedSettingsModel multiserver] && [iTermMultiServerConnection available];
}

+ (BOOL)getGeneralConnection:(iTermGeneralServerConnection *)generalConnection
    fromRestorationIdentifier:(NSDictionary *)dict {
    NSString *type = dict[iTermMultiServerRestorationKeyType];
    if (![type isEqual:iTermMultiServerRestorationType]) {
        return NO;
    }

    NSNumber *version = [NSNumber castFrom:dict[iTermMultiServerRestorationKeyVersion]];
    if (!version || version.intValue < 0 ||
        version.intValue > iTermMultiServerMaximumSupportedRestorationIdentifierVersion) {
        return NO;
    }

    NSNumber *socketNumber = [NSNumber castFrom:dict[iTermMultiServerRestorationKeySocket]];
    if (!socketNumber || socketNumber.intValue <= 0) {
        return NO;
    }
    NSNumber *childPidNumber = [NSNumber castFrom:dict[iTermMultiServerRestorationKeyChildPID]];
    if (!childPidNumber || childPidNumber.intValue < 0) {
        return NO;
    }
    memset(generalConnection, 0, sizeof(*generalConnection));
    generalConnection->type = iTermGeneralServerConnectionTypeMulti;
    generalConnection->multi.number = socketNumber.intValue;
    generalConnection->multi.pid = childPidNumber.intValue;
    return YES;
}

- (instancetype)initWithQueue:(dispatch_queue_t)queue {
    if (![iTermMultiServerConnection available]) {
        DLog(@"Not creating multiserver job manager because it isn't available. Long path to socket?");
        return nil;
    }
    self = [super init];
    if (self) {
        _thread =
            [[iTermThread alloc] initWithQueue:queue
                                  stateFactory:^iTermSynchronizedState *_Nullable(dispatch_queue_t _Nonnull queue) {
                                      return [[iTermMultiServerJobManagerState alloc] initWithQueue:queue];
                                  }];
    }
    return self;
}

- (NSString *)description {
    __block NSString *result = nil;
    [_thread dispatchRecursiveSync:^(iTermMultiServerJobManagerState *_Nullable state) {
        result = [NSString stringWithFormat:@"<%@: %p child=%@ connection=%@>", NSStringFromClass([self class]), self,
                                            state.child, state.conn];
    }];
    return result;
}

typedef struct {
    iTermTTYState ttyState;
    NSData *argpath;
    char **argv;
    NSData *initialPwd;
    char **environ;
} iTermMultiServerJobManagerForkRequest;

- (void)forkAndExecWithTtyState:(iTermTTYState)ttyState
                        argpath:(NSString *)argpath
                           argv:(NSArray<NSString *> *)argv
                     initialPwd:(NSString *)initialPwd
                     newEnviron:(NSArray<NSString *> *)newEnviron
                           task:(id<iTermTask>)task
                     completion:(void (^)(iTermJobManagerForkAndExecStatus, NSNumber *optionalErrorCode))completion {
    DLog(@"begin");
    iTermMultiServerJobManagerForkRequest forkRequest = {
        .ttyState = ttyState,
        .argpath = [NSData dataWithBytes:argpath.UTF8String length:strlen(argpath.UTF8String) + 1],
        .argv = [argv nullTerminatedCStringArray],
        .initialPwd = [NSData dataWithBytes:initialPwd.UTF8String length:strlen(initialPwd.UTF8String) + 1],
        .environ = [newEnviron nullTerminatedCStringArray]};
    iTermCallback *callback = [self.thread newCallbackWithBlock:^(iTermMultiServerJobManagerState *state,
                                                                  iTermMultiServerConnection *conn) {
        DLog(@"Callback for %@ run with connection %@", self, conn);
        if (!conn) {
            DLog(@"MultiServerJobManager: NO CONNECTION, returning ServerLaunchFailed");
            completion(iTermJobManagerForkAndExecStatusServerLaunchFailed, nil);
            return;
        }
        DLog(@"Set conn of %@ to %@", self, conn);
        state.conn = conn;
        [self queueForkAndExecWithForkRequest:forkRequest
                                   connection:conn
                                         task:task
                                        state:state
                                   completion:^(iTermJobManagerForkAndExecStatus status, NSNumber *optionalErrorCode) {
                                       iTermFreeeNullTerminatedCStringArray(forkRequest.argv);
                                       iTermFreeeNullTerminatedCStringArray(forkRequest.environ);
                                       if (completion) {
                                           completion(status, optionalErrorCode);
                                       }
                                   }];
    }];
    [iTermMultiServerConnection getOrCreatePrimaryConnectionWithCallback:callback];
}

- (void)queueForkAndExecWithForkRequest:(iTermMultiServerJobManagerForkRequest)forkRequest
                             connection:(iTermMultiServerConnection *)conn
                                   task:(id<iTermTask>)task
                                  state:(iTermMultiServerJobManagerState *)state
                             completion:(void (^)(iTermJobManagerForkAndExecStatus, NSNumber *code))completion {
    DLog(@"queueForkAndExecWithForkRequest: entered with conn=%@", conn);
    [state check];

    iTermCallback *callback =
        [self.thread newCallbackWithBlock:^(iTermMultiServerJobManagerState *state,
                                            iTermResult<iTermFileDescriptorMultiClientChild *> *result) {
            DLog(@"called back with result %@", result);
            [result
                handleObject:^(iTermFileDescriptorMultiClientChild *_Nonnull child) {
                    DLog(@"Forked and execed %@ OK. Set child to %@, pid=%d", self, child, child.pid);
                    state.child = child;
                    // Happy path
                    dispatch_async(dispatch_get_main_queue(), ^{
                        [[iTermProcessCache sharedInstance] registerTrackedPID:child.pid];
                        DLog(@"Register %@ after server successfully execs job", @(child.pid));
                        [[TaskNotifier sharedInstance] registerTask:task];
                        [[iTermProcessCache sharedInstance] setNeedsUpdate:YES];
                        completion(iTermJobManagerForkAndExecStatusSuccess, nil);
                    });
                }
                error:^(NSError *_Nonnull error) {
                    DLog(@"Fork and exec %@ failed with error=%@. Set child to nil", self, error);
                    state.child = nil;
                    // BUG-f1057: Replace assert with guard - wrong error domain should return generic error, not crash
                    if (![error.domain isEqualToString:iTermFileDescriptorMultiClientErrorDomain]) {
                        ELog(@"queueForkAndExec error has unexpected domain: %@ (expected %@)",
                             error.domain, iTermFileDescriptorMultiClientErrorDomain);
                        completion(iTermJobManagerForkAndExecStatusServerError, nil);
                        return;
                    }
                    const iTermFileDescriptorMultiClientErrorCode code =
                        (iTermFileDescriptorMultiClientErrorCode)error.code;
                    switch (code) {
                        case iTermFileDescriptorMultiClientErrorCodePreemptiveWaitResponse:
                        case iTermFileDescriptorMultiClientErrorCodeConnectionLost:
                        case iTermFileDescriptorMultiClientErrorCodeNoSuchChild:
                        case iTermFileDescriptorMultiClientErrorCodeCanNotWait:
                        case iTermFileDescriptorMultiClientErrorCodeUnknown:
                        case iTermFileDescriptorMultiClientErrorIO:
                        case iTermFileDescriptorMultiClientErrorCannotConnect:
                        case iTermFileDescriptorMultiClientErrorProtocolError:
                        case iTermFileDescriptorMultiClientErrorAlreadyWaited:
                            completion(iTermJobManagerForkAndExecStatusServerError, nil);
                            return;

                        case iTermFileDescriptorMultiClientErrorCodeForkFailed:
                            completion(iTermJobManagerForkAndExecStatusFailedToFork, error.userInfo[@"code"]);
                            return;
                    }
                    // BUG-404: Replace assert(NO) with ELog for unhandled error code
                    // If a new error code is added to the enum and not handled here,
                    // log it and return a generic server error instead of crashing
                    ELog(@"iTermMultiServerJobManager: Unhandled error code %@ in domain %@", @(code), error.domain);
                    completion(iTermJobManagerForkAndExecStatusServerError, nil);
                }];
        }];
    [conn launchWithTTYState:forkRequest.ttyState
                     argpath:forkRequest.argpath.bytes
                        argv:forkRequest.argv
                  initialPwd:forkRequest.initialPwd.bytes
                  newEnviron:forkRequest.environ
                    callback:callback];
}

- (int)fd {
    __block int result = -1;
    [self.thread dispatchRecursiveSync:^(iTermMultiServerJobManagerState *_Nullable state) {
        result = state.child ? state.child.fd : -1;
    }];
    return result;
}

- (BOOL)closeFileDescriptor {
    __block BOOL result = NO;
    [self.thread dispatchRecursiveSync:^(iTermMultiServerJobManagerState *_Nullable state) {
        DLog(@"closeFileDescriptor for %@, child=%@", self, state.child);
        if (state.child == nil) {
            result = NO;
            return;
        }
        [state.child closeFileDescriptor];
        DLog(@"Set child of %@ to nil", self);
        state.child = nil;
        result = YES;
    }];
    return result;
}


- (BOOL)ioAllowed {
    __block BOOL result = NO;
    [self.thread dispatchRecursiveSync:^(iTermMultiServerJobManagerState *_Nullable state) {
        result = (state.child != nil && state.child.fd >= 0);
    }];
    return result;
}

- (void)setFd:(int)fd {
    // BUG-f1058: Replace assert with guard - setFd should only accept -1
    if (fd != -1) {
        ELog(@"setFd called with %d but only -1 is supported", fd);
        return;
    }
}

- (NSString *)tty {
    __block NSString *result = nil;
    [self.thread dispatchRecursiveSync:^(iTermMultiServerJobManagerState *_Nullable state) {
        result = [state.child.tty copy];
    }];
    return result;
}

- (void)setTty:(NSString *)tty {
    [self.thread dispatchRecursiveSync:^(iTermMultiServerJobManagerState *_Nullable state) {
        ITAssertWithMessage([NSObject object:tty isEqualToObject:state.child.tty], @"setTty:%@ when _child.tty=%@", tty,
                            state.child.tty);
    }];
}

- (pid_t)externallyVisiblePid {
    __block pid_t result = 0;
    [self.thread dispatchRecursiveSync:^(iTermMultiServerJobManagerState *_Nullable state) {
        result = state.child.pid;
    }];
    return result;
}

- (BOOL)hasJob {
    __block BOOL result = NO;
    [self.thread dispatchRecursiveSync:^(iTermMultiServerJobManagerState *_Nullable state) {
        result = (state.child != nil);
    }];
    return result;
}

- (id)sessionRestorationIdentifier {
    __block id result = nil;
    [self.thread dispatchRecursiveSync:^(iTermMultiServerJobManagerState *_Nullable state) {
        if (!state.conn) {
            // This can happen while the connection is being set up.
            DLog(@"Report nil session restoration identifier because state has no connection");
            result = nil;
            return;
        }
        result = @{
            iTermMultiServerRestorationKeyType : iTermMultiServerRestorationType,
            iTermMultiServerRestorationKeyVersion : @(iTermMultiServerMaximumSupportedRestorationIdentifierVersion),
            iTermMultiServerRestorationKeySocket : @(state.conn.socketNumber),
            iTermMultiServerRestorationKeyChildPID : @(state.child.pid)
        };
    }];
    return result;
}

- (pid_t)pidToWaitOn {
    return -1;
}

- (BOOL)isSessionRestorationPossible {
    __block BOOL result = NO;
    [self.thread dispatchRecursiveSync:^(iTermMultiServerJobManagerState *_Nullable state) {
        iTermFileDescriptorMultiClientChild *child = state.child;
        DLog(@"child=%@", child);
        result = (child != nil);
    }];
    return result;
}

- (void)attachToServer:(iTermGeneralServerConnection)serverConnection
         withProcessID:(NSNumber *)thePid
                  task:(id<iTermTask>)task
            completion:(void (^)(iTermJobManagerAttachResults))completion {
    // BUG-f1059: Replace assert with guard - wrong connection type should return early
    if (serverConnection.type != iTermGeneralServerConnectionTypeMulti) {
        ELog(@"attachToServer called with non-multi connection type: %lu", (unsigned long)serverConnection.type);
        completion(0);
        return;
    }
    DLog(@"(async) Attach to server on socket number %@ for pid %@\n%@", @(serverConnection.multi.number),
         @(serverConnection.multi.pid), [NSThread callStackSymbols]);
    void (^finish)(iTermFileDescriptorMultiClientChild *) = ^(iTermFileDescriptorMultiClientChild *child) {
        [[iTermThread main] dispatchAsync:^(id _Nullable state) {
            if (child != nil && !child.hasTerminated) {
                [self didAttachToProcess:child.pid task:task state:state];
            }
            iTermJobManagerAttachResults results = 0;
            if (child != nil) {
                results |= iTermJobManagerAttachResultsAttached;
                if (!child.hasTerminated) {
                    results |= iTermJobManagerAttachResultsRegistered;
                }
            }
            completion(results);
        }];
    };
    [self beginAttachToServer:serverConnection
                withProcessID:thePid
                   brokenPipe:^{
                       [task brokenPipe];
                   }
                   completion:finish];
}

- (void)asyncPartialAttachToServer:(iTermGeneralServerConnection)serverConnection
                     withProcessID:(NSNumber *)thePid
                        completion:(void (^)(id<iTermJobManagerPartialResult>))completion {
    // BUG-f1060: Replace assert with guard - wrong connection type should return early
    if (serverConnection.type != iTermGeneralServerConnectionTypeMulti) {
        ELog(@"asyncPartialAttachToServer called with non-multi connection type: %lu", (unsigned long)serverConnection.type);
        completion(nil);
        return;
    }
    DLog(@"(sync) Attach to server on socket number %@ for pid %@\n%@", @(serverConnection.multi.number),
         @(serverConnection.multi.pid), [NSThread callStackSymbols]);

    iTermMultiServerJobManagerPartialAttachment *result = [[iTermMultiServerJobManagerPartialAttachment alloc] init];
    [self beginAttachToServer:serverConnection
        withProcessID:thePid
        brokenPipe:^{
            result.brokenPipe = YES;
        }
        completion:^(iTermFileDescriptorMultiClientChild *child) {
            result.shouldRegister = (child != nil && !child.hasTerminated);
            result.attached = (child != nil);
            result.pid = child.pid;
            completion(result);
        }];
}

- (iTermJobManagerAttachResults)attachToServer:(iTermGeneralServerConnection)serverConnection
                                 withProcessID:(NSNumber *)thePid
                                          task:(id<iTermTask>)task {
    __block iTermMultiServerJobManagerPartialAttachment *result;
    dispatch_group_t group = dispatch_group_create();
    dispatch_group_enter(group);
    [self asyncPartialAttachToServer:serverConnection
                       withProcessID:thePid
                          completion:^(iTermMultiServerJobManagerPartialAttachment *value) {
                              result = value;
                              dispatch_group_leave(group);
                          }];
    // Use 30 second timeout to prevent blocking forever during session restoration
    // This is generous but prevents indefinite hangs if the async operation never completes
    dispatch_time_t timeout = dispatch_time(DISPATCH_TIME_NOW, 30 * NSEC_PER_SEC);
    long waitResult = dispatch_group_wait(group, timeout);
    if (waitResult != 0) {
        DLog(@"iTermMultiServerJobManager: attachToServer timed out after 30 seconds for pid %@", thePid);
        // Return failure - couldn't attach in time
        return 0;
    }
    if (result.brokenPipe) {
        [task brokenPipe];
    }
    return [self finishAttaching:result task:task];
}

- (iTermJobManagerAttachResults)finishAttaching:(iTermMultiServerJobManagerPartialAttachment *)result
                                           task:(id<iTermTask>)task {
    if (result.shouldRegister) {
        // BUG-f1061: Replace assert with guard - invalid pid should skip registration
        if (result.pid <= 0) {
            ELog(@"finishAttaching: shouldRegister is YES but pid=%d is invalid - skipping registration", result.pid);
        } else {
            [self didAttachToProcess:result.pid task:task state:[iTermMainThreadState sharedInstance]];
        }
    }
    iTermJobManagerAttachResults results = 0;
    if (result.attached) {
        results |= iTermJobManagerAttachResultsAttached;
        if (result.shouldRegister) {
            results |= iTermJobManagerAttachResultsRegistered;
        }
    }
    if (result.brokenPipe) {
        [task brokenPipe];
    }
    return results;
}

- (void)beginAttachToServer:(iTermGeneralServerConnection)serverConnection
              withProcessID:(NSNumber *)thePid
                 brokenPipe:(void (^)(void))brokenPipe
                 completion:(void (^)(iTermFileDescriptorMultiClientChild *))completion {
    // BUG-f1062: Replace assert with guard - wrong connection type should return early
    if (serverConnection.type != iTermGeneralServerConnectionTypeMulti) {
        ELog(@"beginAttachToServer called with non-multi connection type: %lu", (unsigned long)serverConnection.type);
        completion(nil);
        return;
    }
    DLog(@"begin attaching to server on socket %@ with pid %@", @(serverConnection.multi.number),
         @(serverConnection.multi.pid));
    [self.thread dispatchAsync:^(iTermMultiServerJobManagerState *_Nullable state) {
        // BUG-f1063: Replace assert with guard - existing child should return early
        if (state.child != nil) {
            ELog(@"beginAttachToServer: state already has a child - returning existing");
            completion(state.child);
            return;
        }
        [self reallyAttachToServer:serverConnection
                     withProcessID:thePid
                        brokenPipe:brokenPipe
                             state:state
                          callback:[self.thread newCallbackWithBlock:^(iTermMultiServerJobManagerState *state,
                                                                       iTermFileDescriptorMultiClientChild *child) {
                              completion(child);
                          }]];
    }];
}

- (void)didAttachToProcess:(pid_t)pid task:(id<iTermTask>)task state:(iTermMainThreadState *)state {
    DLog(@"Did attach to process with pid %@", @(pid));
    [state check];

    [[iTermProcessCache sharedInstance] registerTrackedPID:pid];
    DLog(@"Register task %@ after attaching", @(pid));
    [[TaskNotifier sharedInstance] registerTask:task];
    [[iTermProcessCache sharedInstance] setNeedsUpdate:YES];
}

- (void)reallyAttachToServer:(iTermGeneralServerConnection)serverConnection
               withProcessID:(NSNumber *)thePid
                  brokenPipe:(void (^)(void))brokenPipe
                       state:(iTermMultiServerJobManagerState *)state
                    callback:(iTermCallback<id, iTermFileDescriptorMultiClientChild *> *)completionCallback {
    // BUG-f1064: Replace assert with guard - wrong connection type should return nil via callback
    if (serverConnection.type != iTermGeneralServerConnectionTypeMulti) {
        ELog(@"reallyAttachToServer called with non-multi connection type: %lu", (unsigned long)serverConnection.type);
        [completionCallback invokeWithObject:nil];
        return;
    }
    // BUG-f1065: Replace assert with guard - existing conn should log warning but proceed
    if (state.conn) {
        ELog(@"reallyAttachToServer: state already has a connection");
    }
    // BUG-f1066: Replace assert with guard - existing child should log warning but proceed
    if (state.child) {
        ELog(@"reallyAttachToServer: state already has a child");
    }
    DLog(@"Want to attach to socket %@, child %@", @(serverConnection.multi.number), @(serverConnection.multi.pid));
    iTermCallback *callback = [self.thread newCallbackWithBlock:^(iTermMultiServerJobManagerState *state,
                                                                  iTermResult<iTermMultiServerConnection *> *result) {
        [result
            handleObject:^(iTermMultiServerConnection *_Nonnull conn) {
                DLog(@"Have a connection to %@. Try to attach to child with pid %@.", @(serverConnection.multi.number),
                     @(serverConnection.multi.pid));
                DLog(@"Set conn of %@ to %@", self, conn);
                state.conn = conn;
                // BUG-f1067: Replace assert with warning log - pid mismatch should log, not crash
                if (thePid != nil && thePid.integerValue != serverConnection.multi.pid) {
                    ELog(@"reallyAttachToServer: pid mismatch - thePid=%@ but serverConnection.multi.pid=%d",
                         thePid, serverConnection.multi.pid);
                }
                [state.conn
                    attachToProcessID:serverConnection.multi.pid
                             callback:[self.thread newCallbackWithBlock:^(iTermMultiServerJobManagerState *state,
                                                                          iTermFileDescriptorMultiClientChild *child) {
                                 DLog(@"Attached OK. Set child of %@ to %@", self, child);
                                 state.child = child;
                                 if (!state.child) {
                                     [completionCallback invokeWithObject:nil];
                                     return;
                                 }
                                 if (state.child.hasTerminated) {
                                     DLog(@"Found child with pid %@, but it already terminated.",
                                          @(serverConnection.multi.pid));
                                     const pid_t pid = state.child.pid;
                                     [state.conn waitForChild:state.child
                                           removePreemptively:NO
                                                     callback:[self.thread newCallbackWithBlock:^(
                                                                               iTermMultiServerJobManagerState *state,
                                                                               iTermResult<NSNumber *> *waitResult) {
                                                         [waitResult
                                                             handleObject:^(NSNumber *_Nonnull statusNumber) {
                                                                 DLog(@"Child with pid %d terminated with status %d",
                                                                      pid, statusNumber.intValue);
                                                             }
                                                             error:^(NSError *_Nonnull error) {
                                                                 DLog(@"Failed to wait on child with pid %d: %@", pid,
                                                                      error);
                                                             }];
                                                         brokenPipe();
                                                         [completionCallback invokeWithObject:child];
                                                         return;
                                                     }]];
                                     return;
                                 }
                                 DLog(@"Found child with pid %@ and it looks to still be alive.",
                                      @(serverConnection.multi.pid));
                                 [completionCallback invokeWithObject:child];
                             }]];
            }
            error:^(NSError *_Nonnull error) {
                DLog(@"FAILED to connect to daemon %@, while aiming to attach to child with pid %@.",
                     @(serverConnection.multi.number), @(serverConnection.multi.pid));
                DLog(@"Set conn of %@ to nil", self);
                state.conn = nil;
                [completionCallback invokeWithObject:nil];
            }];
    }];
    [iTermMultiServerConnection getConnectionForSocketNumber:serverConnection.multi.number
                                            createIfPossible:NO
                                                    callback:callback];
}

- (void)sendSignal:(int)signo toServer:(BOOL)toServer state:(iTermMultiServerJobManagerState *)state {
    // Maybe this could be async. Needs testing.
    if (toServer) {
        if (state.conn.pid <= 0) {
            return;
        }
        DLog(@"Sending signal to server %@", @(state.conn.pid));
        kill(state.conn.pid, signo);
        return;
    }
    if (state.child.pid <= 0) {
        return;
    }
    [[iTermProcessCache sharedInstance] unregisterTrackedPID:state.child.pid];
    DLog(@"Send signal %@ to %@\n%@", @(signo), @(state.child.pid), [NSThread callStackSymbols]);
    killpg(state.child.pid, signo);
}

- (void)killWithMode:(iTermJobManagerKillingMode)mode {
    DLog(@"%@ killWithMode:%@", self, @(mode));
    [self.thread dispatchRecursiveSync:^(iTermMultiServerJobManagerState *_Nullable state) {
        [self killWithMode:mode state:state];
    }];
}

// Called on self.queue
- (void)killWithMode:(iTermJobManagerKillingMode)mode state:(iTermMultiServerJobManagerState *)state {
    switch (mode) {
        case iTermJobManagerKillingModeRegular:
            [self sendSignal:SIGHUP toServer:NO state:state];
            break;

        case iTermJobManagerKillingModeForce:
            [self sendSignal:SIGKILL toServer:NO state:state];
            break;

        case iTermJobManagerKillingModeForceUnrestorable:
            [self sendSignal:SIGKILL toServer:YES state:state];
            [self sendSignal:SIGHUP toServer:NO state:state];
            break;

        case iTermJobManagerKillingModeProcessGroup:
            if (state.child.pid > 0) {
                [[iTermProcessCache sharedInstance] unregisterTrackedPID:state.child.pid];
                // Kill a server-owned child.
                // TODO: Don't want to do this when Sparkle is upgrading.
                killpg(state.child.pid, SIGHUP);
            }
            break;

        case iTermJobManagerKillingModeBrokenPipe:
            // This is irrelevant for the multiserver. Monoserver needs to ensure the server
            // dies even when the child is persistent, but multiserver can survive
            // its children.
            break;
    }
    const pid_t pid = state.child.pid;
    if (pid) {
        [state.conn waitForChild:state.child
              removePreemptively:YES
                        callback:[self.thread newCallbackWithBlock:^(iTermMultiServerJobManagerState *state,
                                                                     iTermResult<NSNumber *> *result) {
                            // NOTE: killWithMode:state: must be idempotent. Be careful here.
                            DLog(@"Preemptive wait for %d finished with result %@", pid, result);
                        }]];
    }
}

- (BOOL)isReadOnly {
    return NO;
}

@end
