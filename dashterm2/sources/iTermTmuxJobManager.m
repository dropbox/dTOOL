//
//  iTermTmuxJobManager.m
//  DashTerm2
//
//  Created by George Nachman on 5/28/20.
//

#import "iTermTmuxJobManager.h"

#import "DebugLogging.h"
#import <os/lock.h>

// Performance optimization: os_unfair_lock has ~10x lower overhead than @synchronized

@implementation iTermTmuxJobManager {
    os_unfair_lock _lock;
}

+ (void)initialize {
    if (self == [iTermTmuxJobManager class]) {
        // Lock is initialized inline below
    }
}

- (instancetype)init {
    self = [super init];
    if (self) {
        _lock = OS_UNFAIR_LOCK_INIT;
    }
    return self;
}

@synthesize fd = _fd;
@synthesize tty = _tty;
@synthesize queue = _queue;

+ (BOOL)available {
    return YES;
}

- (instancetype)initWithQueue:(dispatch_queue_t)queue {
    self = [self init];
    if (self) {
        _fd = -1;
    }
    return self;
}

- (NSString *)description {
    return [NSString
        stringWithFormat:@"<%@: %p read-only-fd=%d tty=%@>", NSStringFromClass([self class]), self, _fd, _tty];
}

- (pid_t)serverPid {
    return -1;
}

- (void)setServerPid:(pid_t)serverPid {
    // BUG-456: Replace assert(NO) with DLog - tmux job manager doesn't support servers
    // This method exists for protocol conformance but should not be called
    DLog(@"setServerPid called on tmux job manager - this is not supported");
}

- (int)socketFd {
    return -1;
}

- (void)setSocketFd:(int)socketFd {
    // BUG-457: Replace assert(NO) with DLog - tmux job manager doesn't support socket FD
    // This method exists for protocol conformance but should not be called
    DLog(@"setSocketFd called on tmux job manager - this is not supported");
}

- (BOOL)closeFileDescriptor {
    os_unfair_lock_lock(&_lock);
    BOOL result;
    if (self.fd == -1) {
        result = NO;
    } else {
        close(self.fd);
        self.fd = -1;
        result = YES;
    }
    os_unfair_lock_unlock(&_lock);
    return result;
}

- (void)forkAndExecWithTtyState:(iTermTTYState)ttyState
                        argpath:(NSString *)argpath
                           argv:(NSArray<NSString *> *)argv
                     initialPwd:(NSString *)initialPwd
                     newEnviron:(NSArray<NSString *> *)newEnviron
                           task:(id<iTermTask>)task
                     completion:(void (^)(iTermJobManagerForkAndExecStatus, NSNumber *))completion {
    // BUG-458: Replace assert(NO) with ELog and error callback
    // tmux job manager doesn't fork processes - call error completion
    ELog(@"forkAndExecWithTtyState called on tmux job manager - not supported");
    if (completion) {
        completion(iTermJobManagerForkAndExecStatusServerError, nil);
    }
}

- (void)attachToServer:(iTermGeneralServerConnection)serverConnection
         withProcessID:(NSNumber *)thePid
                  task:(id<iTermTask>)task
            completion:(void (^)(iTermJobManagerAttachResults results))completion {
    // BUG-459: Replace assert(NO) with ELog and error callback
    // tmux job manager doesn't support server attachment
    ELog(@"attachToServer called on tmux job manager - not supported");
    if (completion) {
        // iTermJobManagerAttachResults is a bitmask - 0 means failure (no flags set)
        completion(0);
    }
}

- (iTermJobManagerAttachResults)attachToServer:(iTermGeneralServerConnection)serverConnection
                                 withProcessID:(NSNumber *)thePid
                                          task:(id<iTermTask>)task {
    // BUG-460: Replace assert(NO) with ELog and error return
    // tmux job manager doesn't support synchronous server attachment
    ELog(@"attachToServer (sync) called on tmux job manager - not supported");
    // iTermJobManagerAttachResults is a bitmask - 0 means failure (no flags set)
    return 0;
}

- (void)killWithMode:(iTermJobManagerKillingMode)mode {
}

- (pid_t)externallyVisiblePid {
    return 0;
}

- (BOOL)hasJob {
    return YES;
}

- (BOOL)ioAllowed {
    os_unfair_lock_lock(&_lock);
    BOOL result = self.fd >= 0;
    os_unfair_lock_unlock(&_lock);
    return result;
}

- (BOOL)isSessionRestorationPossible {
    return NO;
}

- (pid_t)pidToWaitOn {
    return 0;
}

- (id)sessionRestorationIdentifier {
    return nil;
}

- (BOOL)isReadOnly {
    return YES;
}

@end
