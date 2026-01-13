//
//  iTermLegacyJobManager.m
//  DashTerm2SharedARC
//
//  Created by George Nachman on 11/16/19.
//

#import "iTermLegacyJobManager.h"

#import "DebugLogging.h"
#import "iTermProcessCache.h"
#import "NSArray+iTerm.h"
#import "PTYTask+MRR.h"
#import "TaskNotifier.h"

#import <os/lock.h>

@interface iTermLegacyJobManager ()
@property (atomic) pid_t childPid;
@end

@implementation iTermLegacyJobManager {
    // Performance optimization: os_unfair_lock has ~10x lower overhead than @synchronized
    os_unfair_lock _fdLock;
}

@synthesize fd = _fd;
@synthesize tty = _tty;
@synthesize queue = _queue;

+ (BOOL)available {
    return YES;
}

- (instancetype)initWithQueue:(dispatch_queue_t)queue {
    self = [super init];
    if (self) {
        _queue = queue;
        _fd = -1;
        self.childPid = -1;
        _fdLock = OS_UNFAIR_LOCK_INIT;
    }
    return self;
}

- (NSString *)description {
    return [NSString stringWithFormat:@"<%@: %p fd=%d tty=%@ childPid=%@>", NSStringFromClass([self class]), self, _fd,
                                      _tty, @(self.childPid)];
}

- (pid_t)serverPid {
    return -1;
}

- (void)setServerPid:(pid_t)serverPid {
    // BUG-448: Replace assert(NO) with DLog - legacy job manager doesn't support servers
    // This method exists for protocol conformance but should not be called
    DLog(@"setServerPid called on legacy job manager - this is not supported");
}

- (int)socketFd {
    return -1;
}

- (void)setSocketFd:(int)socketFd {
    // BUG-449: Replace assert(NO) with DLog - legacy job manager doesn't support socket FD
    // This method exists for protocol conformance but should not be called
    DLog(@"setSocketFd called on legacy job manager - this is not supported");
}

- (BOOL)closeFileDescriptor {
    os_unfair_lock_lock(&_fdLock);
    if (self.fd == -1) {
        os_unfair_lock_unlock(&_fdLock);
        return NO;
    }
    close(self.fd);
    self.fd = -1;
    os_unfair_lock_unlock(&_fdLock);
    return YES;
}

- (void)forkAndExecWithTtyState:(iTermTTYState)ttyState
                        argpath:(NSString *)argpath
                           argv:(NSArray<NSString *> *)argv
                     initialPwd:(NSString *)initialPwd
                     newEnviron:(NSArray<NSString *> *)newEnviron
                           task:(id<iTermTask>)task
                     completion:(void (^)(iTermJobManagerForkAndExecStatus, NSNumber *))completion {
    __block iTermJobManagerForkAndExecStatus status = iTermJobManagerForkAndExecStatusSuccess;
    dispatch_sync(self.queue, ^{
        status = [self queueForkAndExecWithTtyState:ttyState
                                            argpath:argpath
                                               argv:argv
                                         initialPwd:initialPwd
                                         newEnviron:newEnviron
                                               task:task];
    });
    if (status == iTermJobManagerForkAndExecStatusSuccess) {
        DLog(@"Register task for pid %@", @(self.childPid));
        [[TaskNotifier sharedInstance] registerTask:task];
    }
    if (completion) {
        completion(status, nil);
    }
}

- (iTermJobManagerForkAndExecStatus)queueForkAndExecWithTtyState:(iTermTTYState)ttyState
                                                         argpath:(NSString *)argpath
                                                            argv:(NSArray<NSString *> *)argv
                                                      initialPwd:(NSString *)initialPwd
                                                      newEnviron:(NSArray<NSString *> *)newEnviron
                                                            task:(id<iTermTask>)task {
    iTermForkState forkState = {
        .connectionFd = -1,
        .deadMansPipe = {0, 0},
    };
    char **cArgv = [argv nullTerminatedCStringArray];
    char **cEnviron = [newEnviron nullTerminatedCStringArray];
    self.fd = iTermForkAndExecToRunJobDirectly(&forkState, &ttyState, argpath.UTF8String, cArgv, YES,
                                               initialPwd.UTF8String, cEnviron);
    iTermFreeeNullTerminatedCStringArray(cArgv);
    iTermFreeeNullTerminatedCStringArray(cEnviron);

    // If you get here you're the parent.
    self.childPid = forkState.pid;
    if (self.childPid > 0) {
        [[iTermProcessCache sharedInstance] registerTrackedPID:self.childPid];
    }
    if (forkState.pid < (pid_t)0) {
        return iTermJobManagerForkAndExecStatusFailedToFork;
    }

    // Make sure the master side of the pty is closed on future exec() calls.
    DLog(@"fcntl");
    fcntl(self.fd, F_SETFD, fcntl(self.fd, F_GETFD) | FD_CLOEXEC);

    self.tty = [NSString stringWithUTF8String:ttyState.tty];
    fcntl(self.fd, F_SETFL, O_NONBLOCK);
    return iTermJobManagerForkAndExecStatusSuccess;
}

- (void)attachToServer:(iTermGeneralServerConnection)serverConnection
         withProcessID:(NSNumber *)thePid
                  task:(id<iTermTask>)task
            completion:(void (^)(iTermJobManagerAttachResults))completion {
    completion(iTermJobManagerAttachResultsAttached | iTermJobManagerAttachResultsRegistered);
}

- (iTermJobManagerAttachResults)attachToServer:(iTermGeneralServerConnection)serverConnection
                                 withProcessID:(NSNumber *)thePid
                                          task:(id<iTermTask>)task {
    return iTermJobManagerAttachResultsAttached | iTermJobManagerAttachResultsRegistered;
}

- (void)sendSignal:(int)signo toProcessGroup:(BOOL)toProcessGroup {
    dispatch_async(self.queue, ^{
        [self queueSendSignal:signo toProcessGroup:toProcessGroup];
    });
}

- (void)queueSendSignal:(int)signo toProcessGroup:(BOOL)toProcessGroup {
    if (self.childPid <= 0) {
        return;
    }
    [[iTermProcessCache sharedInstance] unregisterTrackedPID:self.childPid];
    if (toProcessGroup) {
        DLog(@"Kill process group %@ with signal %@", @(self.childPid), @(signo));
        killpg(self.childPid, signo);
    } else {
        DLog(@"Kill process %@ with signal %@", @(self.childPid), @(signo));
        kill(self.childPid, signo);
    }
    DLog(@"%@", [NSThread callStackSymbols]);
}

- (void)killWithMode:(iTermJobManagerKillingMode)mode {
    DLog(@"%@ killWithMode:%@", self, @(mode));
    switch (mode) {
        case iTermJobManagerKillingModeRegular:
            [self sendSignal:SIGHUP toProcessGroup:NO];
            break;

        case iTermJobManagerKillingModeForce:
            [self sendSignal:SIGKILL toProcessGroup:NO];
            break;

        case iTermJobManagerKillingModeForceUnrestorable:
            // SIGHUP is correct here per PTYTask.h: "SIGHUP to child always" for this mode.
            // Server-based managers send SIGKILL to server + SIGHUP to child.
            // Legacy manager has no server, so we only send SIGHUP to the child.
            [self sendSignal:SIGHUP toProcessGroup:NO];
            break;

        case iTermJobManagerKillingModeProcessGroup:
            [self sendSignal:SIGHUP toProcessGroup:YES];
            break;

        case iTermJobManagerKillingModeBrokenPipe:
            break;
    }
}

- (pid_t)pidToWaitOn {
    return self.childPid;
}

- (BOOL)isSessionRestorationPossible {
    return NO;
}

- (pid_t)externallyVisiblePid {
    return self.childPid;
}

- (BOOL)hasJob {
    return self.childPid != -1;
}

- (id)sessionRestorationIdentifier {
    return nil;
}

- (BOOL)ioAllowed {
    return self.fd >= 0;
}

- (BOOL)isReadOnly {
    return NO;
}

@end
