#define MAXRW 1024

#import "PTYTask.h"
#import "PTYTask+Private.h"

#import "Coprocess.h"
#import "DebugLogging.h"
#import "iTermMalloc.h"
#import "iTermNotificationController.h"
#import "iTermPosixTTYReplacements.h"
#import "iTermProcessCache.h"
#import "NSWorkspace+iTerm.h"
#import "PreferencePanel.h"
#import "PTYTask.h"
#import "PTYTask+MRR.h"
#import "TaskNotifier.h"
#import "iTermAdvancedSettingsModel.h"
#import "iTermLSOF.h"
#import "iTermLegacyJobManager.h"
#import "iTermMonoServerJobManager.h"
#import "iTermMultiServerJobManager.h"
#import "iTermOpenDirectory.h"
#import "iTermOrphanServerAdopter.h"
#import "iTermThreadSafety.h"
#import "iTermTmuxJobManager.h"
#import "NSDictionary+iTerm.h"

#import "DashTerm2SharedARC-Swift.h"
#include "iTermFileDescriptorClient.h"
#include "iTermFileDescriptorServer.h"
#include "iTermFileDescriptorSocketPath.h"
#include "legacy_server.h"
#include <dlfcn.h>
#include <libproc.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/mount.h>
#include <sys/msg.h>
#include <sys/select.h>
#include <sys/time.h>
#include <sys/user.h>
#include <unistd.h>
#include <util.h>
#import <os/lock.h>

@interface PTYTask (WinSizeControllerDelegate) <iTermWinSizeControllerDelegate>
@end

static void HandleSigChld(int n) {
    // This is safe to do because write(2) is listed in the sigaction(2) man page
    // as allowed in a signal handler. Calling a method is *NOT* safe since something might
    // be fiddling with the runtime. I saw a lot of crashes where CoreData got interrupted by
    // a sigchild while doing class_addMethod and that caused a crash because of a method call.
    UnblockTaskNotifier();
}

@implementation PTYTask {
    int status;
    NSString *path;
    BOOL hasOutput;

    NSLock *writeLock; // protects writeBuffer
    NSMutableData *writeBuffer;

    // Performance optimization: os_unfair_lock has ~10x lower overhead than @synchronized
    os_unfair_lock _lock;

    Coprocess *coprocess_; // protected by _lock
    BOOL brokenPipe_;      // protected by _lock
    NSString *command_;    // Command that was run if launchWithPath:arguments:etc was called

    BOOL _paused; // protected by _lock

    dispatch_queue_t _jobManagerQueue;
    BOOL _isTmuxTask;
}

- (instancetype)init {
    self = [super init];
    if (self) {
        _lock = OS_UNFAIR_LOCK_INIT;
        const char *label = [iTermThread uniqueQueueLabelWithName:@"com.dashterm.dashterm2.job-manager"].UTF8String;
        _jobManagerQueue = dispatch_queue_create(label, DISPATCH_QUEUE_SERIAL);
        _winSizeController = [[iTermWinSizeController alloc] init];
        _winSizeController.delegate = self;
        writeBuffer = [[NSMutableData alloc] init];
        writeLock = [[NSLock alloc] init];
        if ([iTermAdvancedSettingsModel runJobsInServers]) {
            if ([iTermMultiServerJobManager available]) {
                self.jobManager = [[iTermMultiServerJobManager alloc] initWithQueue:_jobManagerQueue];
            } else {
                self.jobManager = [[iTermMonoServerJobManager alloc] initWithQueue:_jobManagerQueue];
            }
        } else {
            self.jobManager = [[iTermLegacyJobManager alloc] initWithQueue:_jobManagerQueue];
        }
        self.fd = -1;
    }
    return self;
}

- (void)dealloc {
    DLog(@"Dealloc PTYTask %p", self);
    // TODO: The use of killpg seems pretty sketchy. It takes a pgid_t, not a
    // pid_t. Are they guaranteed to always be the same for process group
    // leaders? It is not clear from git history why killpg is used here and
    // not in other places. I suspect it's what we ought to use everywhere.
    [self.jobManager killWithMode:iTermJobManagerKillingModeProcessGroup];
    if (_tmuxClientProcessID) {
        [[iTermProcessCache sharedInstance] unregisterTrackedPID:_tmuxClientProcessID.intValue];
    }
    [self.ioBuffer invalidate];

    [self closeFileDescriptorAndDeregisterIfPossible];

    os_unfair_lock_lock(&_lock);
    Coprocess *coprocess = coprocess_;
    os_unfair_lock_unlock(&_lock);
    [coprocess mainProcessDidTerminate];
}

- (NSString *)description {
    return [NSString stringWithFormat:@"<%@: %p jobManager=%@ pid=%@ fd=%@ tmuxClientProcessID=%@>",
                                      NSStringFromClass([self class]), self, self.jobManager, @(self.pid), @(self.fd),
                                      _tmuxClientProcessID];
}

#pragma mark - APIs

- (BOOL)paused {
    os_unfair_lock_lock(&_lock);
    BOOL paused = _paused;
    os_unfair_lock_unlock(&_lock);
    return paused;
}

- (void)setPaused:(BOOL)paused {
    os_unfair_lock_lock(&_lock);
    _paused = paused;
    os_unfair_lock_unlock(&_lock);
    // Start/stop selecting on our FD
    [[TaskNotifier sharedInstance] unblock];
    [_delegate taskDidChangePaused:self paused:paused];
}

- (pid_t)pidToWaitOn {
    return self.jobManager.pidToWaitOn;
}

- (BOOL)isSessionRestorationPossible {
    return self.jobManager.isSessionRestorationPossible;
}

- (id)sessionRestorationIdentifier {
    return self.jobManager.sessionRestorationIdentifier;
}

- (int)fd {
    // BUG-f887: Replace assert with graceful handling - return -1 if no jobManager
    if (!self.jobManager) {
        DLog(@"BUG-f887: fd getter called but jobManager is nil - returning -1");
        return -1;
    }
    return self.jobManager.fd;
}

- (void)setFd:(int)fd {
    // BUG-f888: Replace assert with graceful handling - ignore if no jobManager
    if (!self.jobManager) {
        DLog(@"BUG-f888: setFd called but jobManager is nil - ignoring");
        return;
    }
    self.jobManager.fd = fd;
}

- (pid_t)pid {
    return self.jobManager.externallyVisiblePid;
}

- (int)status {
    return status;
}

- (NSString *)path {
    return path;
}

- (NSString *)getWorkingDirectory {
    DLog(@"Want working directory of %@ - SYNCHRONOUS", @(self.pid));
    if (self.pid == -1) {
        DLog(@"Want to use the kernel to get the working directory but pid = -1");
        return nil;
    }
    return [iTermLSOF workingDirectoryOfProcess:self.pid];
}

- (void)getWorkingDirectoryWithCompletion:(void (^)(NSString *pwd))completion {
    DLog(@"Want working directory of %@ - async", @(self.pid));
    if (self.pid == -1) {
        DLog(@"Want to use the kernel to get the working directory but pid = -1");
        completion(nil);
        return;
    }
    [iTermLSOF asyncWorkingDirectoryOfProcess:self.pid queue:dispatch_get_main_queue() block:completion];
}

- (Coprocess *)coprocess {
    os_unfair_lock_lock(&_lock);
    Coprocess *coprocess = coprocess_;
    os_unfair_lock_unlock(&_lock);
    return coprocess;
}

// This runs on the task notifier thread
- (void)setCoprocess:(Coprocess *)coprocess {
    DLog(@"Set coprocess of %@ to %@", self, coprocess);
    os_unfair_lock_lock(&_lock);
    coprocess_ = coprocess;
    BOOL hasMuteCoprocess = coprocess_.mute;
    os_unfair_lock_unlock(&_lock);
    self.hasMuteCoprocess = hasMuteCoprocess;
    __weak __typeof(self) weakSelf = self;
    dispatch_async(dispatch_get_main_queue(), ^{
        [weakSelf.delegate taskMuteCoprocessDidChange:self hasMuteCoprocess:self.hasMuteCoprocess];
    });
    [[TaskNotifier sharedInstance] unblock];
}

- (BOOL)writeBufferHasRoom {
    const int kMaxWriteBufferSize = 1024 * 10;
    [writeLock lock];
    BOOL hasRoom = [writeBuffer length] < kMaxWriteBufferSize;
    [writeLock unlock];
    return hasRoom;
}

- (BOOL)hasCoprocess {
    os_unfair_lock_lock(&_lock);
    BOOL hasCoprocess = (coprocess_ != nil);
    os_unfair_lock_unlock(&_lock);
    return hasCoprocess;
}

- (void)withCoprocessLocked:(void (^)(Coprocess *coprocess))block {
    if (!block) {
        return;
    }
    os_unfair_lock_lock(&_lock);
    Coprocess *coprocess = coprocess_; // ARC retains automatically via strong local variable
    os_unfair_lock_unlock(&_lock);
    block(coprocess);
    // ARC releases automatically when coprocess goes out of scope
}

- (BOOL)passwordInput {
    struct termios termAttributes;
    if ([iTermAdvancedSettingsModel detectPasswordInput] && self.fd > 0 && isatty(self.fd) &&
        tcgetattr(self.fd, &termAttributes) == 0) {
        return !(termAttributes.c_lflag & ECHO) && (termAttributes.c_lflag & ICANON);
    } else {
        return NO;
    }
}

- (BOOL)hasBrokenPipe {
    os_unfair_lock_lock(&_lock);
    BOOL hasBrokenPipe = brokenPipe_;
    os_unfair_lock_unlock(&_lock);
    return hasBrokenPipe;
}

- (NSString *)originalCommand {
    return command_;
}

- (void)launchWithPath:(NSString *)progpath
             arguments:(NSArray *)args
           environment:(NSDictionary *)env
           customShell:(NSString *)customShell
              gridSize:(VT100GridSize)gridSize
              viewSize:(NSSize)viewSize
      maybeScaleFactor:(CGFloat)maybeScaleFactor
                isUTF8:(BOOL)isUTF8
            completion:(void (^)(void))completion {
    DLog(@"launchWithPath: entered with progpath=%@", progpath);
    DLog(@"launchWithPath: runJobsInServers=%@, multiServerAvailable=%@",
         @([iTermAdvancedSettingsModel runJobsInServers]), @([iTermMultiServerJobManager available]));
    DLog(@"launchWithPath:%@ args:%@ env:%@ grisSize:%@ isUTF8:%@", progpath, args, env,
         VT100GridSizeDescription(gridSize), @(isUTF8));

    if ([iTermAdvancedSettingsModel runJobsInServers] && ![iTermMultiServerJobManager available]) {
        // We want to run
        //   DashTerm2 --server progpath args
        NSArray *updatedArgs = [@[ @"--server", progpath ] arrayByAddingObjectsFromArray:args];
        if (![iTermAdvancedSettingsModel bootstrapDaemon]) {
            env = [env dictionaryBySettingObject:@"1" forKey:@"DASHTERM2_DISABLE_BOOTSTRAP"];
        }
        [self reallyLaunchWithPath:[[NSBundle mainBundle] executablePath]
                         arguments:updatedArgs
                       environment:env
                       customShell:customShell
                          gridSize:gridSize
                          viewSize:viewSize
                  maybeScaleFactor:maybeScaleFactor
                            isUTF8:isUTF8
                        completion:completion];
    } else {
        [self reallyLaunchWithPath:progpath
                         arguments:args
                       environment:env
                       customShell:customShell
                          gridSize:gridSize
                          viewSize:viewSize
                  maybeScaleFactor:maybeScaleFactor
                            isUTF8:isUTF8
                        completion:completion];
    }
}

- (void)setTmuxClientProcessID:(NSNumber *)tmuxClientProcessID {
    if ([NSObject object:tmuxClientProcessID isEqualToObject:_tmuxClientProcessID]) {
        return;
    }
    DLog(@"Set tmux client process ID for %@ to %@", self, tmuxClientProcessID);
    if (_tmuxClientProcessID) {
        [[iTermProcessCache sharedInstance] unregisterTrackedPID:_tmuxClientProcessID.intValue];
    }
    if (tmuxClientProcessID) {
        [[iTermProcessCache sharedInstance] registerTrackedPID:tmuxClientProcessID.intValue];
    }
    _tmuxClientProcessID = tmuxClientProcessID;
}

- (void)setReadOnlyFileDescriptor:(int)readOnlyFileDescriptor {
    iTermTmuxJobManager *jobManager = [[iTermTmuxJobManager alloc] initWithQueue:self->_jobManagerQueue];
    jobManager.fd = readOnlyFileDescriptor;
    DLog(@"Configure %@ as tmux task", self);
    _jobManager = jobManager;
    [[TaskNotifier sharedInstance] registerTask:self];
}

- (void)setIoBuffer:(iTermIOBuffer *)ioBuffer {
    iTermChannelJobManager *jobManager = [[iTermChannelJobManager alloc] initWithQueue:_jobManagerQueue];
    jobManager.ioBuffer = ioBuffer;
    _jobManager = jobManager;
}

- (iTermIOBuffer *)ioBuffer {
    return [[iTermChannelJobManager castFrom:_jobManager] ioBuffer];
}

- (int)readOnlyFileDescriptor {
    if (![_jobManager isKindOfClass:[iTermTmuxJobManager class]]) {
        return -1;
    }
    return _jobManager.fd;
}

// Send keyboard input, coprocess output, tmux commands, etc.
- (void)writeTask:(NSData *)data {
    [self writeTask:data coprocess:NO];
}

- (void)writeTask:(NSData *)data coprocess:(BOOL)fromCoprocessOutput {
    if (_isTmuxTask) {
        // Send keypresses to tmux.
        NSData *copyOfData = [data copy];
        dispatch_async(dispatch_get_main_queue(), ^{
            [self.delegate tmuxClientWrite:copyOfData];
        });
        return;
    }
    if (self.sshIntegrationActive && fromCoprocessOutput) {
        NSData *copyOfData = [data copy];
        DLog(@"Direct data from coprocess to session to route to conductor: %@", data);
        __weak __typeof(self) weakSelf = self;
        dispatch_async(dispatch_get_main_queue(), ^{
            [weakSelf.delegate taskDidReadFromCoprocessWhileSSHIntegrationInUse:copyOfData];
        });
        return;
    }
    iTermIOBuffer *ioBuffer = self.ioBuffer;
    if (ioBuffer) {
        [ioBuffer write:data];
        return;
    }
    // Write as much as we can now through the non-blocking pipe
    // Lock to protect the writeBuffer from the IO thread
    id<iTermJobManager> jobManager = self.jobManager;
    // BUG-f889: Replace assert with graceful handling - skip write if read-only
    if (jobManager && self.jobManager.isReadOnly) {
        DLog(@"BUG-f889: writeTask called but jobManager is read-only - ignoring write");
        return;
    }
    [writeLock lock];
    [writeBuffer appendData:data];
    [[TaskNotifier sharedInstance] unblock];
    [writeLock unlock];
}

- (void)killWithMode:(iTermJobManagerKillingMode)mode {
    [self.jobManager killWithMode:mode];
    if (_tmuxClientProcessID) {
        [[iTermProcessCache sharedInstance] unregisterTrackedPID:_tmuxClientProcessID.intValue];
    }
}

- (void)stop {
    DLog(@"stop %@", self);
    self.paused = NO;
    [self.loggingHelper stop];
    [self killWithMode:iTermJobManagerKillingModeRegular];

    // Ensure the server is broken out of accept()ing for future connections
    // in case the child doesn't die right away.
    [self killWithMode:iTermJobManagerKillingModeBrokenPipe];

    [self closeFileDescriptorAndDeregisterIfPossible];
}

- (void)brokenPipe {
    DLog(@"brokenPipe %@", self);
    os_unfair_lock_lock(&_lock);
    brokenPipe_ = YES;
    os_unfair_lock_unlock(&_lock);
    [[TaskNotifier sharedInstance] deregisterTask:self];
    [self.delegate threadedTaskBrokenPipe];
}

// Main queue
- (void)didRegister {
    DLog(@"didRegister %@", self);
    [self.delegate taskDidRegister:self];
}

// I did extensive benchmarking in May of 2025 when using the VT100_GANG optimization fully.
// I saw that this function almost never produces more than 1024 bytes. I think what's
// happening is that the TTY driver has an internal buffer of 1024 bytes. Because token
// execution is slower than reading and parsing, we enter a backpressure situation. At that
// point, we are in a situation where each read gives 1024 bytes and allows the PTY to fill
// with the next 1024 bytes. That becomes the stead state. Consequently, the semaphore that
// defines the depth of our queue also determines (in the steady state) how much data can be
// buffered and it's 1024 bytes * initial semaphore count.
- (void)processRead {
    int iterations = 4;
    int bytesRead = 0;

    char buffer[MAXRW * iterations];
    for (int i = 0; i < iterations; ++i) {
        // Only read up to MAXRW*iterations bytes, then release control
        ssize_t n = read(self.fd, buffer + bytesRead, MAXRW);
        if (n < 0) {
            // There was a read error.
            if (errno != EAGAIN && errno != EINTR) {
                // It was a serious error.
                [self brokenPipe];
                return;
            } else {
                // We could read again in the case of EINTR but it would
                // complicate the code with little advantage. Just bail out.
                n = 0;
            }
        }
        bytesRead += n;
        if (n < MAXRW) {
            // If we read fewer bytes than expected, return. For some apparently
            // undocumented reason, read() never returns more than 1024 bytes
            // (at least on OS 10.6), so that's what MAXRW is set to. If that
            // ever goes down this'll break.
            break;
        }
    }

    hasOutput = YES;

    // Send data to the terminal
    [self readTask:buffer length:bytesRead];
}

- (void)processWrite {
    // Retain to prevent the object from being released during this method
    // Lock to protect the writeBuffer from the main thread
    [writeLock lock];

    // Only write up to MAXRW bytes, then release control
    char *ptr = [writeBuffer mutableBytes];
    unsigned int length = [writeBuffer length];
    if (length > MAXRW) {
        length = MAXRW;
    }
    ssize_t written = write(self.fd, [writeBuffer mutableBytes], length);

    // No data?
    if ((written < 0) && (!(errno == EAGAIN || errno == EINTR))) {
        [self brokenPipe];
    } else if (written > 0) {
        // Shrink the writeBuffer
        length = [writeBuffer length] - written;
        memmove(ptr, ptr + written, length);
        [writeBuffer setLength:length];
    }

    // Clean up locks
    [writeLock unlock];
}

- (void)stopCoprocess {
    pid_t thePid = 0;
    os_unfair_lock_lock(&_lock);
    if (coprocess_.pid > 0) {
        thePid = coprocess_.pid;
    }
    [coprocess_ terminate];
    coprocess_ = nil;
    os_unfair_lock_unlock(&_lock);
    self.hasMuteCoprocess = NO;
    [self.delegate taskMuteCoprocessDidChange:self hasMuteCoprocess:self.hasMuteCoprocess];

    if (thePid) {
        [[TaskNotifier sharedInstance] waitForPid:thePid];
    }
    [[TaskNotifier sharedInstance] performSelectorOnMainThread:@selector(notifyCoprocessChange)
                                                    withObject:nil
                                                 waitUntilDone:NO];
}

- (void)setJobManagerType:(iTermGeneralServerConnectionType)type {
    // BUG-f890: Replace assert with graceful handling
    if (![self canAttach]) {
        DLog(@"BUG-f890: setJobManagerType called but canAttach is NO - ignoring");
        return;
    }
    // BUG-f891: Replace assert with graceful handling
    if (![NSThread isMainThread]) {
        DLog(@"BUG-f891: setJobManagerType called off main thread - dispatching to main");
        dispatch_async(dispatch_get_main_queue(), ^{
            [self setJobManagerType:type];
        });
        return;
    }
    switch (type) {
        case iTermGeneralServerConnectionTypeMono:
            if ([self.jobManager isKindOfClass:[iTermMonoServerJobManager class]]) {
                return;
            }
            DLog(@"Replace jobmanager %@ with monoserver instance", self.jobManager);
            self.jobManager = [[iTermMonoServerJobManager alloc] initWithQueue:self->_jobManagerQueue];
            return;

        case iTermGeneralServerConnectionTypeMulti:
            if ([self.jobManager isKindOfClass:[iTermMultiServerJobManager class]]) {
                return;
            }
            DLog(@"Replace jobmanager %@ with multiserver instance", self.jobManager);
            self.jobManager = [[iTermMultiServerJobManager alloc] initWithQueue:self->_jobManagerQueue];
            return;
    }
    ITAssertWithMessage(NO, @"Unrecognized job type %@", @(type));
}

// This works for any kind of connection. It finishes the process of attaching a PTYTask to a child
// that we know is in a server, either newly launched or an orphan.
- (void)attachToServer:(iTermGeneralServerConnection)serverConnection
            completion:(void (^)(iTermJobManagerAttachResults))completion {
    // BUG-f892: Replace assert with graceful early return
    if (![self canAttach]) {
        DLog(@"BUG-f892: attachToServer:completion: called but canAttach is NO");
        if (completion) {
            completion(0); // No flags set - not attached, not registered
        }
        return;
    }
    [self setJobManagerType:serverConnection.type];
    [_jobManager attachToServer:serverConnection withProcessID:nil task:self completion:completion];
}

- (iTermJobManagerAttachResults)attachToServer:(iTermGeneralServerConnection)serverConnection {
    // BUG-f893: Replace assert with graceful early return
    if (![self canAttach]) {
        DLog(@"BUG-f893: attachToServer: called but canAttach is NO");
        return 0; // No flags set - not attached, not registered
    }
    [self setJobManagerType:serverConnection.type];
    if (serverConnection.type == iTermGeneralServerConnectionTypeMulti) {
        DLog(@"PTYTask: attach to multiserver %@", @(serverConnection.multi.number));
    }
    return [_jobManager attachToServer:serverConnection withProcessID:nil task:self];
}

- (BOOL)canAttach {
    if (![iTermAdvancedSettingsModel runJobsInServers]) {
        return NO;
    }
    if (self.jobManager.hasJob) {
        return NO;
    }
    return YES;
}

// Monoserver only. Used when restoring a non-ophan session. May block while connecting to the
// server. Deletes the socket after connecting.
- (BOOL)tryToAttachToServerWithProcessId:(pid_t)thePid tty:(NSString *)tty {
    if (![self canAttach]) {
        return NO;
    }

    DLog(@"tryToAttachToServerWithProcessId: Attempt to connect to server for pid %d, tty %@", (int)thePid, tty);
    iTermFileDescriptorServerConnection serverConnection = iTermFileDescriptorClientRun(thePid);
    if (!serverConnection.ok) {
        DLog(@"Failed with error %s", serverConnection.error);
        return NO;
    }
    DLog(@"Succeeded.");
    iTermGeneralServerConnection general = {.type = iTermGeneralServerConnectionTypeMono, .mono = serverConnection};
    [self setJobManagerType:general.type];
    // This assumes the monoserver finishes synchronously and can't fail.
    [self.jobManager attachToServer:general
                      withProcessID:@(thePid)
                               task:self
                         completion:^(iTermJobManagerAttachResults results){
                         }];
    [self setTty:tty];
    return YES;
}

// Multiserver only. Used when restoring a non-orphan session. May block while connecting to the
// server.
- (iTermJobManagerAttachResults)tryToAttachToMultiserverWithRestorationIdentifier:
    (NSDictionary *)restorationIdentifier {
    if (![self canAttach]) {
        return 0;
    }
    iTermGeneralServerConnection generalConnection;
    if (![iTermMultiServerJobManager getGeneralConnection:&generalConnection
                                fromRestorationIdentifier:restorationIdentifier]) {
        return 0;
    }

    DLog(@"tryToAttachToMultiserverWithRestorationIdentifier:%@", restorationIdentifier);
    return [self attachToServer:generalConnection];
}

- (void)partiallyAttachToMultiserverWithRestorationIdentifier:(NSDictionary *)restorationIdentifier
                                                   completion:(void (^)(id<iTermJobManagerPartialResult>))completion {
    if (!self.canAttach) {
        completion(0);
        return;
    }
    iTermGeneralServerConnection generalConnection;
    if (![iTermMultiServerJobManager getGeneralConnection:&generalConnection
                                fromRestorationIdentifier:restorationIdentifier]) {
        completion(0);
        return;
    }
    if (generalConnection.type != iTermGeneralServerConnectionTypeMulti) {
        // BUG-12004: Return early instead of assert(NO) for unexpected connection type
        DLog(@"BUG-12004: Expected multi-server connection type but got %d", (int)generalConnection.type);
        completion(0);
        return;
    }
    [_jobManager asyncPartialAttachToServer:generalConnection
                              withProcessID:@(generalConnection.multi.pid)
                                 completion:completion];
}

- (iTermJobManagerAttachResults)finishAttachingToMultiserver:(id<iTermJobManagerPartialResult>)partialResult
                                                  jobManager:(id<iTermJobManager>)jobManager
                                                       queue:(dispatch_queue_t)queue {
    // BUG-f894: Replace assert with graceful handling
    if (![NSThread isMainThread]) {
        DLog(@"BUG-f894: finishAttachingToMultiserver called off main thread");
        __block iTermJobManagerAttachResults result;
        dispatch_sync(dispatch_get_main_queue(), ^{
            result = [self finishAttachingToMultiserver:partialResult jobManager:jobManager queue:queue];
        });
        return result;
    }
    self.jobManager = jobManager;
    _jobManagerQueue = queue;
    return [_jobManager finishAttaching:partialResult task:self];
}

- (void)registerTmuxTask {
    _isTmuxTask = YES;
    DLog(@"Register pid %@ as coprocess-only task", @(self.pid));
    [[TaskNotifier sharedInstance] registerTask:self];
}

#pragma mark - Private

#pragma mark Task Launching Helpers

+ (NSMutableDictionary *)mutableEnvironmentDictionary {
    // Typical environment has ~50-100 variables
    NSMutableDictionary *result = [NSMutableDictionary dictionaryWithCapacity:64];
    extern char **environ;
    if (environ != NULL) {
        NSSet<NSString *> *forbiddenKeys = [NSSet setWithArray:@[ @"NSZombieEnabled", @"MallocStackLogging" ]];
        for (int i = 0; environ[i]; i++) {
            NSString *kvp = [NSString stringWithUTF8String:environ[i]];
            NSRange equalsRange = [kvp rangeOfString:@"="];
            if (equalsRange.location != NSNotFound) {
                NSString *key = [kvp substringToIndex:equalsRange.location];
                NSString *value = [kvp substringFromIndex:equalsRange.location + 1];
                if (![forbiddenKeys containsObject:key]) {
                    result[key] = value;
                }
            } else {
                result[kvp] = @"";
            }
        }
    }
    return result;
}

// Returns a NSMutableDictionary containing the key-value pairs defined in the
// global "environ" variable.
- (NSMutableDictionary *)mutableEnvironmentDictionary {
    return [PTYTask mutableEnvironmentDictionary];
}

- (NSArray<NSString *> *)environWithOverrides:(NSDictionary *)env {
    NSMutableDictionary *environmentDict = [self mutableEnvironmentDictionary];
    for (NSString *k in env) {
        environmentDict[k] = env[k];
    }
    [environmentDict removeObjectForKey:@"SHLVL"]; // Issue 9756
    // Each environment entry becomes a KEY=VALUE string
    NSMutableArray<NSString *> *environment = [NSMutableArray arrayWithCapacity:environmentDict.count];
    for (NSString *k in environmentDict) {
        NSString *temp = [NSString stringWithFormat:@"%@=%@", k, environmentDict[k]];
        [environment addObject:temp];
    }
    return environment;
}

- (NSDictionary *)environmentBySettingShell:(NSDictionary *)originalEnvironment {
    NSString *shell = [iTermOpenDirectory userShell];
    if (!shell) {
        return originalEnvironment;
    }
    NSMutableDictionary *newEnvironment = [originalEnvironment mutableCopy];
    newEnvironment[@"SHELL"] = [shell copy];
    return newEnvironment;
}

- (void)setCommand:(NSString *)command {
    command_ = [command copy];
}

- (NSString *)tty {
    return self.jobManager.tty;
}

- (void)setTty:(NSString *)tty {
    self.jobManager.tty = tty;
    if ([NSThread isMainThread]) {
        [self.delegate taskDidChangeTTY:self];
    } else {
        __weak id<PTYTaskDelegate> delegate = self.delegate;
        __weak __typeof(self) weakSelf = self;
        dispatch_async(dispatch_get_main_queue(), ^{
            __strong __typeof(self) strongSelf = weakSelf;
            if (strongSelf) {
                [delegate taskDidChangeTTY:strongSelf];
            }
        });
    }
}

- (void)reallyLaunchWithPath:(NSString *)progpath
                   arguments:(NSArray *)args
                 environment:(NSDictionary *)env
                 customShell:(NSString *)customShell
                    gridSize:(VT100GridSize)gridSize
                    viewSize:(NSSize)pointSize
            maybeScaleFactor:(CGFloat)maybeScaleFactor
                      isUTF8:(BOOL)isUTF8
                  completion:(void (^)(void))completion {
    DLog(@"reallyLaunchWithPath: entered with progpath=%@", progpath);
    NSSize viewSize = pointSize;
    if (maybeScaleFactor > 0) {
        viewSize.width *= maybeScaleFactor;
        viewSize.height *= maybeScaleFactor;
    }
    DLog(@"reallyLaunchWithPath:%@ args:%@ env:%@ gridSize:%@ viewSize:%@ isUTF8:%@", progpath, args, env,
         VT100GridSizeDescription(gridSize), NSStringFromSize(viewSize), @(isUTF8));

    __block iTermTTYState ttyState;
    PTYTaskSize newSize = {.cellSize = iTermTTYCellSizeMake(gridSize.width, gridSize.height),
                           .pixelSize = iTermTTYPixelSizeMake(viewSize.width, viewSize.height)};
    DLog(@"Initialize tty with cell size %d x %d, pixel size %d x %d", newSize.cellSize.width, newSize.cellSize.height,
         newSize.pixelSize.width, newSize.pixelSize.height);
    iTermTTYStateInit(&ttyState, newSize.cellSize, newSize.pixelSize, isUTF8);
    [_winSizeController setInitialSize:gridSize viewSize:pointSize scaleFactor:maybeScaleFactor];

    [self setCommand:progpath];
    if (customShell) {
        DLog(@"Use custom shell");
        env = [env dictionaryBySettingObject:customShell forKey:@"SHELL"];
    } else {
        env = [self environmentBySettingShell:env];
    }

    DLog(@"After setting shell environment is %@", env);
    path = [progpath copy];
    NSString *commandToExec = [progpath stringByStandardizingPath];

    // Register a handler for the child death signal. There is some history here.
    // Originally, a do-nothing handler was registered with the following comment:
    //   We cannot ignore SIGCHLD because Sparkle (the software updater) opens a
    //   Safari control which uses some buggy Netscape code that calls wait()
    //   until it succeeds. If we wait() on its pid, that process locks because
    //   it doesn't check if wait()'s failure is ECHLD. Instead of wait()ing here,
    //   we reap our children when our select() loop sees that a pipes is broken.
    // In response to bug 2903, wherein select() fails to return despite the file
    // descriptor having EOF status, I changed the handler to unblock the task
    // notifier.
    signal(SIGCHLD, HandleSigChld);

    // argv includes progpath + user args, typically 1-10 entries
    NSMutableArray<NSString *> *argv = [NSMutableArray arrayWithCapacity:1 + args.count];
    [argv addObject:[progpath stringByStandardizingPath]];
    [argv addObjectsFromArray:args];

    DLog(@"Preparing to launch a job. Command is %@ and args are %@", commandToExec, args);
    DLog(@"Environment is\n%@", env);
    NSArray<NSString *> *newEnviron = [self environWithOverrides:env];

    // Note: stringByStandardizingPath will automatically call stringByExpandingTildeInPath.
    NSString *initialPwd = [[env objectForKey:@"PWD"] stringByStandardizingPath];
    DLog(@"initialPwd=%@, jobManager=%@", initialPwd, self.jobManager);
    DLog(@"reallyLaunchWithPath: calling forkAndExecWithTtyState, jobManager=%@, commandToExec=%@", self.jobManager,
         commandToExec);
    [self.jobManager
        forkAndExecWithTtyState:ttyState
                        argpath:commandToExec
                           argv:argv
                     initialPwd:initialPwd ?: NSHomeDirectory()
                     newEnviron:newEnviron
                           task:self
                     completion:^(iTermJobManagerForkAndExecStatus status, NSNumber *optionalErrorCode) {
                         DLog(@"forkAndExec completion: status=%d, errorCode=%@", (int)status, optionalErrorCode);
                         dispatch_async(dispatch_get_main_queue(), ^{
                             [self didForkAndExec:progpath withStatus:status optionalErrorCode:optionalErrorCode];
                             if (completion) {
                                 completion();
                             }
                         });
                     }];
}

// Main queue
- (void)didForkAndExec:(NSString *)progpath
            withStatus:(iTermJobManagerForkAndExecStatus)status
     optionalErrorCode:(NSNumber *)optionalErrorCode {
    switch (status) {
        case iTermJobManagerForkAndExecStatusSuccess:
            // Parent
            [self setTty:self.jobManager.tty];
            DLog(@"finished succesfully");
            break;

        case iTermJobManagerForkAndExecStatusTempFileError:
            [self showFailedToCreateTempSocketError];
            break;

        case iTermJobManagerForkAndExecStatusFailedToFork: {
            DLog(@"Unable to fork %@: %s", progpath, strerror(optionalErrorCode.intValue));
            NSString *error = @"Unable to fork child process: you may have too many processes already running.";
            if (optionalErrorCode) {
                error = [NSString
                    stringWithFormat:@"%@ The system error was: %s", error, strerror(optionalErrorCode.intValue)];
            }
            [[iTermNotificationController sharedInstance] notify:@"Unable to fork!" withDescription:error];
            [self.delegate taskDiedWithError:error];
            break;
        }

        case iTermJobManagerForkAndExecStatusTaskDiedImmediately:
        case iTermJobManagerForkAndExecStatusServerError:
        case iTermJobManagerForkAndExecStatusServerLaunchFailed:
            [self.delegate taskDiedImmediately];
            break;
    }
}

- (void)showFailedToCreateTempSocketError {
    NSAlert *alert = [[NSAlert alloc] init];
    alert.messageText = @"Error";
    alert.informativeText = [NSString stringWithFormat:@"An error was encountered while creating a temporary file with "
                                                       @"mkstemps. Verify that %@ exists and is writable.",
                                                       NSTemporaryDirectory()];
    [alert addButtonWithTitle:@"OK"];
    [alert runModal];
}

#pragma mark I/O

- (BOOL)wantsRead {
    if (self.paused) {
        return NO;
    }
    return self.jobManager.ioAllowed;
}

- (BOOL)wantsWrite {
    if (self.paused) {
        return NO;
    }
    if (self.jobManager.isReadOnly) {
        return NO;
    }
    [writeLock lock];
    const BOOL wantsWrite = [writeBuffer length] > 0;
    [writeLock unlock];
    if (!wantsWrite) {
        return NO;
    }
    return self.jobManager.ioAllowed;
}

- (BOOL)hasOutput {
    return hasOutput;
}

// BUG-2771: Add maximum output buffer size to prevent memory exhaustion
// Coprocess.m defines kMaxOutputBufferSize = 1024, but we use a larger limit here
// since terminal output can be bursty. 64KB allows reasonable buffering while
// preventing unbounded growth.
static const NSUInteger kPTYTaskMaxCoprocessOutputBufferSize = 64 * 1024;

// Internal method for use when _lock is already held
- (void)writeToCoprocess_locked:(NSData *)data {
    // BUG-2771: Check buffer size before appending to prevent memory exhaustion
    NSUInteger currentSize = coprocess_.outputBuffer.length;
    NSUInteger remainingCapacity =
        (currentSize < kPTYTaskMaxCoprocessOutputBufferSize) ? (kPTYTaskMaxCoprocessOutputBufferSize - currentSize) : 0;
    if (remainingCapacity == 0) {
        // Buffer full - drop data to prevent memory exhaustion
        return;
    }
    NSUInteger bytesToAppend = MIN(data.length, remainingCapacity);
    if (bytesToAppend < data.length) {
        // Truncate data to fit in remaining capacity
        [coprocess_.outputBuffer appendData:[data subdataWithRange:NSMakeRange(0, bytesToAppend)]];
    } else {
        [coprocess_.outputBuffer appendData:data];
    }
}

- (void)writeToCoprocess:(NSData *)data {
    os_unfair_lock_lock(&_lock);
    [self writeToCoprocess_locked:data];
    os_unfair_lock_unlock(&_lock);
}

// The bytes in data were just read from the fd.
- (void)readTask:(char *)buffer length:(int)length {
    if (self.loggingHelper) {
        [self.loggingHelper logData:[NSData dataWithBytes:buffer length:length]];
    }

    // The delegate is responsible for parsing VT100 tokens here and sending them off to the
    // main thread for execution. If its queues get too large, it can block.
    [self.delegate threadedReadTask:buffer length:length];

    os_unfair_lock_lock(&_lock);
    if (coprocess_ && !self.sshIntegrationActive) {
        [self writeToCoprocess_locked:[NSData dataWithBytes:buffer length:length]];
    }
    os_unfair_lock_unlock(&_lock);
}

- (void)closeFileDescriptorAndDeregisterIfPossible {
    // BUG-f895: Replace assert with graceful early return
    if (!self.jobManager) {
        DLog(@"BUG-f895: closeFileDescriptorAndDeregisterIfPossible called but jobManager is nil");
        return;
    }
    const int fd = self.fd;
    if ([self.jobManager closeFileDescriptor]) {
        DLog(@"Deregister file descriptor %d for process %@ after closing it", fd, @(self.pid));
        [[TaskNotifier sharedInstance] deregisterTask:self];
    }
}

#pragma mark - iTermLoggingHelper

// NOTE: This can be called before the task is launched. It is not used when logging plain text.
- (void)loggingHelperStart:(iTermLoggingHelper *)loggingHelper {
    self.loggingHelper = loggingHelper;
}

- (void)loggingHelperStop:(iTermLoggingHelper *)loggingHelper {
    self.loggingHelper = nil;
}

@end

@implementation PTYTask (WinSizeControllerDelegate)

- (BOOL)winSizeControllerIsReady {
    return self.fd != -1;
}

- (void)winSizeControllerSetGridSize:(VT100GridSize)gridSize
                            viewSize:(NSSize)pointSize
                         scaleFactor:(CGFloat)scaleFactor {
    PTYTaskSize desiredSize = {
        .cellSize = iTermTTYCellSizeMake(gridSize.width, gridSize.height),
        .pixelSize = iTermTTYPixelSizeMake(pointSize.width * scaleFactor, pointSize.height * scaleFactor)};
    iTermSetTerminalSize(self.fd, desiredSize);
    [self.delegate taskDidResizeToGridSize:gridSize
                                 pixelSize:NSMakeSize(desiredSize.pixelSize.width, desiredSize.pixelSize.height)];
}

@end
