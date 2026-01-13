//
//  TaskNotifier.m
//  iTerm
//
//  Created by George Nachman on 12/27/13.
//
//  DashTerm2: Refactored from select() to kqueue() for improved I/O polling
//  performance. kqueue provides O(1) event retrieval vs O(n) for select(),
//  and removes the FD_SETSIZE (1024) limitation.
//

#import "TaskNotifier.h"
#import "Coprocess.h"
#import "DebugLogging.h"
#import "iTermLSOF.h"
#import "iTermAdvancedSettingsModel.h"

#include <sys/time.h>
#include <sys/event.h>

#define PtyTaskDebugLog(args...)

NSString *const kCoprocessStatusChangeNotification = @"kCoprocessStatusChangeNotification";

static int unblockPipeR;
static int unblockPipeW;
static const NSInteger kDeadpoolMaxRetries = 5;

// Event filter types for kqueue registration
typedef NS_ENUM(NSInteger, KQEventType) {
    KQEventTypeTaskRead = 1,
    KQEventTypeTaskWrite = 2,
    KQEventTypeCoprocessRead = 3,
    KQEventTypeCoprocessWrite = 4,
};

@implementation TaskNotifier {
    NSMutableArray<id<iTermTask>> *_tasks;
    // Set to true when an element of '_tasks' was modified
    BOOL tasksChanged;
    // Protects '_tasks' and 'tasksChanged'.
    NSRecursiveLock *tasksLock;

    // A set of NSNumber*s holding pids of tasks that need to be wait()ed on
    NSMutableSet *deadpool;
    NSMutableDictionary<NSNumber *, NSNumber *> *_deadpoolErrorCounts;

    // kqueue file descriptor for event notification
    int _kqueueFd;

    // Track registered fds to avoid duplicate registrations
    // Maps fd -> NSMutableSet of KQEventType values
    NSMutableDictionary<NSNumber *, NSMutableSet<NSNumber *> *> *_registeredFds;
}


+ (instancetype)sharedInstance {
    static id instance;
    static dispatch_once_t once;
    dispatch_once(&once, ^{
        instance = [[self alloc] init];
        [NSThread detachNewThreadSelector:@selector(run) toTarget:instance withObject:nil];
    });
    return instance;
}

- (instancetype)init {
    self = [super init];
    if (self) {
        deadpool = [[NSMutableSet alloc] initWithCapacity:16];  // Dead tasks awaiting cleanup
        _tasks = [[NSMutableArray alloc] initWithCapacity:32];  // Active tasks
        tasksLock = [[NSRecursiveLock alloc] init];
        tasksChanged = NO;
        _registeredFds = [[NSMutableDictionary alloc] initWithCapacity:64];  // FD to task mapping
        _deadpoolErrorCounts = [[NSMutableDictionary alloc] initWithCapacity:16];  // Error counts per deadpool task

        // Create kqueue for event notification
        _kqueueFd = kqueue();
        if (_kqueueFd < 0) {
            [self release];
            return nil;
        }

        int unblockPipe[2];
        if (pipe(unblockPipe) != 0) {
            close(_kqueueFd);
            [self release];
            return nil;
        }
        // Set close-on-exec and non-blocking on both sides of the pipe.
        for (int i = 0; i < 2; i++) {
            int flags;
            flags = fcntl(unblockPipe[0], F_GETFD);
            fcntl(unblockPipe[i], F_SETFD, flags | FD_CLOEXEC);
            fcntl(unblockPipe[i], F_SETFL, O_NONBLOCK);
        }
        unblockPipeR = unblockPipe[0];
        unblockPipeW = unblockPipe[1];

        // Register the unblock pipe with kqueue
        struct kevent ev;
        EV_SET(&ev, unblockPipeR, EVFILT_READ, EV_ADD | EV_CLEAR, 0, 0, NULL);
        kevent(_kqueueFd, &ev, 1, NULL, 0, NULL);
    }
    return self;
}

- (void)dealloc {
    [_tasks release];
    [tasksLock release];
    [deadpool release];
    [_deadpoolErrorCounts release];
    [_registeredFds release];
    close(unblockPipeR);
    close(unblockPipeW);
    if (_kqueueFd >= 0) {
        close(_kqueueFd);
    }
    [super dealloc];
}

- (void)registerTask:(id<iTermTask>)task {
    PtyTaskDebugLog(@"registerTask: lock\n");
    [tasksLock lock];
    PtyTaskDebugLog(@"Add task at %p\n", (void *)task);
    [_tasks addObject:task];
    PtyTaskDebugLog(@"There are now %lu tasks\n", (unsigned long)_tasks.count);
    tasksChanged = YES;
    PtyTaskDebugLog(@"registerTask: unlock\n");
    [tasksLock unlock];
    [self unblock];

    __weak __typeof(task) weakTask = task;
    dispatch_async(dispatch_get_main_queue(), ^{
        [weakTask didRegister];
    });
}

- (void)deregisterTask:(id<iTermTask>)task {
    PtyTaskDebugLog(@"deregisterTask: lock\n");
    [tasksLock lock];
    PtyTaskDebugLog(@"Begin remove task %p\n", (void *)task);
    PtyTaskDebugLog(@"Add %d to deadpool", [task pid]);
    pid_t pidToWaitOn = task.pidToWaitOn;
    if (pidToWaitOn > 0) {
        NSNumber *pidNumber = [iTermLSOF cachedNumberForPid:pidToWaitOn];
        [deadpool addObject:pidNumber];
    }
    if ([task hasCoprocess]) {
        pid_t coprocessPID = [[task coprocess] pid];
        NSNumber *coprocessNumber = [iTermLSOF cachedNumberForPid:coprocessPID];
        [deadpool addObject:coprocessNumber];
    }
    [_tasks removeObject:task];
    tasksChanged = YES;
    PtyTaskDebugLog(@"End remove task %p. There are now %lu tasks.\n", (void *)task, (unsigned long)[_tasks count]);
    PtyTaskDebugLog(@"deregisterTask: unlock\n");
    [tasksLock unlock];
    [self unblock];
}

// NB: This is currently used for coprocesses.
- (void)waitForPid:(pid_t)pid {
    [tasksLock lock];
    NSNumber *pidNumber = [iTermLSOF cachedNumberForPid:pid];
    [deadpool addObject:pidNumber];
    [tasksLock unlock];
    [self unblock];
}

- (void)unblock {
    UnblockTaskNotifier();
}

void UnblockTaskNotifier(void) {
    // This is called in a signal handler and must only call functions listed
    // as safe in sigaction(2)'s man page.
    char dummy = 0;
    write(unblockPipeW, &dummy, 1);
}

- (id<iTermTask>)taskForFileDescriptorLocked:(int)fd {
    for (id<iTermTask> task in _tasks) {
        if ([task fd] == fd) {
            return task;
        }
        __block BOOL found = NO;
        [task withCoprocessLocked:^(Coprocess *coprocess) {
            if (coprocess) {
                const int readFd = [coprocess readFileDescriptor];
                const int writeFd = [coprocess writeFileDescriptor];
                if (fd == readFd || fd == writeFd) {
                    found = YES;
                }
            }
        }];
        if (found) {
            return task;
        }
    }
    return nil;
}

#pragma mark - kqueue registration helpers

// Register a file descriptor for read events with kqueue
- (void)kqueueRegisterReadFd:(int)fd forTask:(id<iTermTask>)task type:(KQEventType)type {
    if (fd < 0)
        return;

    NSNumber *fdKey = @(fd);
    NSNumber *typeKey = @(type);

    NSMutableSet *types = _registeredFds[fdKey];
    if (!types) {
        types = [NSMutableSet setWithCapacity:2]; // Typically read/write events
        _registeredFds[fdKey] = types;
    }

    if ([types containsObject:typeKey]) {
        return; // Already registered
    }

    struct kevent ev;
    EV_SET(&ev, fd, EVFILT_READ, EV_ADD | EV_CLEAR, 0, 0, NULL);
    if (kevent(_kqueueFd, &ev, 1, NULL, 0, NULL) == 0) {
        [types addObject:typeKey];
    }
}

// Register a file descriptor for write events with kqueue
- (void)kqueueRegisterWriteFd:(int)fd forTask:(id<iTermTask>)task type:(KQEventType)type {
    if (fd < 0)
        return;

    NSNumber *fdKey = @(fd);
    NSNumber *typeKey = @(type);

    NSMutableSet *types = _registeredFds[fdKey];
    if (!types) {
        types = [NSMutableSet setWithCapacity:2]; // Typically read/write events
        _registeredFds[fdKey] = types;
    }

    if ([types containsObject:typeKey]) {
        return; // Already registered
    }

    struct kevent ev;
    EV_SET(&ev, fd, EVFILT_WRITE, EV_ADD | EV_CLEAR, 0, 0, NULL);
    if (kevent(_kqueueFd, &ev, 1, NULL, 0, NULL) == 0) {
        [types addObject:typeKey];
    }
}

// Unregister a file descriptor from kqueue
- (void)kqueueUnregisterFd:(int)fd filter:(int16_t)filter type:(KQEventType)type {
    if (fd < 0)
        return;

    NSNumber *fdKey = @(fd);
    NSNumber *typeKey = @(type);

    NSMutableSet *types = _registeredFds[fdKey];
    if (!types || ![types containsObject:typeKey]) {
        return; // Not registered
    }

    struct kevent ev;
    EV_SET(&ev, fd, filter, EV_DELETE, 0, 0, NULL);
    kevent(_kqueueFd, &ev, 1, NULL, 0, NULL);

    [types removeObject:typeKey];
    if (types.count == 0) {
        [_registeredFds removeObjectForKey:fdKey];
    }
}

// Unregister all events for a task
- (void)kqueueUnregisterTask:(id<iTermTask>)task {
    int fd = [task fd];
    if (fd >= 0) {
        [self kqueueUnregisterFd:fd filter:EVFILT_READ type:KQEventTypeTaskRead];
        [self kqueueUnregisterFd:fd filter:EVFILT_WRITE type:KQEventTypeTaskWrite];
    }

    [task withCoprocessLocked:^(Coprocess *coprocess) {
        if (coprocess) {
            int rfd = [coprocess readFileDescriptor];
            int wfd = [coprocess writeFileDescriptor];
            [self kqueueUnregisterFd:rfd filter:EVFILT_READ type:KQEventTypeCoprocessRead];
            [self kqueueUnregisterFd:wfd filter:EVFILT_WRITE type:KQEventTypeCoprocessWrite];
        }
    }];
}

#pragma mark - Event handlers (kqueue-based)

- (void)run {
    // Maximum number of events to retrieve per kevent call
    static const int kMaxEvents = 64;
    struct kevent events[kMaxEvents];

    NSAutoreleasePool *autoreleasePool = [[NSAutoreleasePool alloc] init];

    // DashTerm2: Using kqueue instead of select() for O(1) event retrieval
    for (;;) {
        // Clean out dead tasks and update kqueue registrations
        PtyTaskDebugLog(@"run1: lock");
        [tasksLock lock];

        // Remove tasks with closed file descriptors
        PtyTaskDebugLog(@"Begin cleaning out dead tasks");
        // Pre-allocate with small capacity - typically few tasks are dead at once
        NSMutableArray<id<iTermTask>> *tasksToDeregister = [NSMutableArray arrayWithCapacity:4];
        for (id<iTermTask> theTask in _tasks) {
            if ([theTask fd] < 0) {
                PtyTaskDebugLog(@"Deregister dead task %@\n", theTask);
                [tasksToDeregister addObject:theTask];
            }
        }
        for (id<iTermTask> theTask in tasksToDeregister) {
            [self kqueueUnregisterTask:theTask];
            [self deregisterTask:theTask];
        }

        // waitpid() on pids that we think are dead or will be dead soon
        if ([deadpool count] > 0) {
            NSMutableSet *newDeadpool = [NSMutableSet setWithCapacity:[deadpool count]];
            for (NSNumber *pid in deadpool) {
                if ([pid intValue] < 0) {
                    continue;
                }
                int statLoc;
                PtyTaskDebugLog(@"wait on %d", [pid intValue]);
                pid_t waitresult = waitpid([pid intValue], &statLoc, WNOHANG);
                if (waitresult == 0) {
                    // the process is not yet dead, so put it back in the pool
                    [_deadpoolErrorCounts removeObjectForKey:pid];
                    [newDeadpool addObject:pid];
                } else if (waitresult < 0) {
                    if (errno != ECHILD) {
                        NSNumber *attemptCount = _deadpoolErrorCounts[pid] ?: @0;
                        NSInteger nextAttempt = attemptCount.integerValue + 1;
                        if (nextAttempt >= kDeadpoolMaxRetries) {
                            PtyTaskDebugLog(@"  wait failed repeatedly (%d: %s), dropping pid %@ from deadpool", errno,
                                            strerror(errno), pid);
                            [_deadpoolErrorCounts removeObjectForKey:pid];
                        } else {
                            PtyTaskDebugLog(@"  wait failed with %d (%s), retry %ld/%ld", errno, strerror(errno),
                                            (long)nextAttempt, (long)kDeadpoolMaxRetries);
                            _deadpoolErrorCounts[pid] = @(nextAttempt);
                            [newDeadpool addObject:pid];
                        }
                    } else {
                        PtyTaskDebugLog(@"  wait failed with ECHILD, I guess we already waited on it.");
                        [_deadpoolErrorCounts removeObjectForKey:pid];
                    }
                } else {
                    [_deadpoolErrorCounts removeObjectForKey:pid];
                }
            }
            [deadpool release];
            deadpool = [newDeadpool retain];
        }

        // Register/update kqueue events for all tasks
        PtyTaskDebugLog(@"Begin enumeration over %lu tasks\n", (unsigned long)[_tasks count]);
        for (id<iTermTask> task in _tasks) {
            PtyTaskDebugLog(@"Got task %@\n", task);
            int fd = [task fd];
            if (fd < 0) {
                PtyTaskDebugLog(@"Task has fd of %d\n", fd);
                continue;
            }

            // Register for read events if task wants to read
            if ([task wantsRead]) {
                [self kqueueRegisterReadFd:fd forTask:task type:KQEventTypeTaskRead];
            } else {
                [self kqueueUnregisterFd:fd filter:EVFILT_READ type:KQEventTypeTaskRead];
            }

            // Register for write events if task wants to write
            if ([task wantsWrite]) {
                [self kqueueRegisterWriteFd:fd forTask:task type:KQEventTypeTaskWrite];
            } else {
                [self kqueueUnregisterFd:fd filter:EVFILT_WRITE type:KQEventTypeTaskWrite];
            }

            // Handle coprocess registrations
            [task withCoprocessLocked:^(Coprocess *coprocess) {
                if (coprocess) {
                    if ([coprocess wantToRead] && [task writeBufferHasRoom]) {
                        int rfd = [coprocess readFileDescriptor];
                        [self kqueueRegisterReadFd:rfd forTask:task type:KQEventTypeCoprocessRead];
                    }
                    if ([coprocess wantToWrite]) {
                        int wfd = [coprocess writeFileDescriptor];
                        [self kqueueRegisterWriteFd:wfd forTask:task type:KQEventTypeCoprocessWrite];
                    }
                }
            }];
        }

        PtyTaskDebugLog(@"run1: unlock");
        [tasksLock unlock];

        [autoreleasePool drain];
        autoreleasePool = [[NSAutoreleasePool alloc] init];

        // Wait for events using kqueue (blocks until events are available)
        int numEvents = kevent(_kqueueFd, NULL, 0, events, kMaxEvents, NULL);
        if (numEvents < 0) {
            if (errno == EINTR || errno == EAGAIN) {
                continue;
            }
            // Other errors - continue the loop
            continue;
        }

        // Process received events
        __block BOOL notifyOfCoprocessChange = NO;
        NSMutableSet *handledTasks = [NSMutableSet setWithCapacity:numEvents];

        PtyTaskDebugLog(@"run2: lock");
        [tasksLock lock];

        for (int i = 0; i < numEvents; i++) {
            struct kevent *ev = &events[i];

            // Handle unblock pipe
            if ((int)ev->ident == unblockPipeR) {
                char dummy[32];
                while (read(unblockPipeR, dummy, sizeof(dummy)) > 0) {
                }
                continue;
            }

            // Get the task associated with this event
            id<iTermTask> task = [self taskForFileDescriptorLocked:(int)ev->ident];
            if (!task) {
                continue; // Task was removed or fd no longer tracked
            }

            // Avoid processing same task multiple times per iteration
            if ([handledTasks containsObject:task]) {
                continue;
            }
            [handledTasks addObject:task];

            [[task retain] autorelease];
            int fd = [task fd];
            if (fd < 0) {
                continue;
            }

            // Handle EOF/error on the fd
            if (ev->flags & (EV_EOF | EV_ERROR)) {
                if ((int)ev->ident == fd) {
                    PtyTaskDebugLog(@"run/brokenPipe: unlock");
                    [tasksLock unlock];
                    [task brokenPipe];
                    PtyTaskDebugLog(@"run/brokenPipe: lock");
                    [tasksLock lock];
                    if (tasksChanged) {
                        tasksChanged = NO;
                        break; // Restart from the beginning
                    }
                    continue;
                }
            }

            // Handle read events
            if (ev->filter == EVFILT_READ) {
                if ((int)ev->ident == fd) {
                    // Main task read
                    PtyTaskDebugLog(@"run/processRead: unlock");
                    [tasksLock unlock];
                    [task processRead];
                    PtyTaskDebugLog(@"run/processRead: lock");
                    [tasksLock lock];
                    if (tasksChanged) {
                        tasksChanged = NO;
                        break;
                    }
                } else {
                    // Coprocess read
                    int evIdent = (int)ev->ident;
                    [task withCoprocessLocked:^(Coprocess *coprocess) {
                        if (coprocess && ![coprocess eof] && evIdent == [coprocess readFileDescriptor]) {
                            PtyTaskDebugLog(@"Reading from coprocess");
                            [coprocess read];
                            [task writeTask:coprocess.inputBuffer coprocess:YES];
                            [coprocess.inputBuffer setLength:0];
                        }
                    }];
                }
            }

            // Handle write events
            if (ev->filter == EVFILT_WRITE) {
                if ((int)ev->ident == fd) {
                    // Main task write
                    PtyTaskDebugLog(@"run/processWrite: unlock");
                    [tasksLock unlock];
                    [task processWrite];
                    PtyTaskDebugLog(@"run/processWrite: lock");
                    [tasksLock lock];
                    if (tasksChanged) {
                        tasksChanged = NO;
                        break;
                    }
                } else {
                    // Coprocess write
                    int evIdent = (int)ev->ident;
                    [task withCoprocessLocked:^(Coprocess *coprocess) {
                        if (coprocess && ![coprocess eof] && evIdent == [coprocess writeFileDescriptor]) {
                            PtyTaskDebugLog(@"Write to coprocess %@", coprocess);
                            [coprocess write];
                        }
                    }];
                }
            }

            // Check for coprocess EOF
            if ([task fd] >= 0 && ![task hasBrokenPipe]) {
                [task withCoprocessLocked:^(Coprocess *coprocess) {
                    if (coprocess && [coprocess eof]) {
                        NSNumber *pidNumber = [iTermLSOF cachedNumberForPid:[coprocess pid]];
                        [deadpool addObject:pidNumber];
                        [coprocess terminate];
                        [task setCoprocess:nil];
                        notifyOfCoprocessChange = YES;
                    }
                }];
            }
        }

        PtyTaskDebugLog(@"run3: unlock");
        [tasksLock unlock];

        if (notifyOfCoprocessChange) {
            // Use waitUntilDone:NO to avoid blocking the task notifier thread
            // while waiting for the main thread. The notification just triggers
            // a UI refresh which doesn't need to be synchronous.
            [self performSelectorOnMainThread:@selector(notifyCoprocessChange) withObject:nil waitUntilDone:NO];
        }

        [autoreleasePool drain];
        autoreleasePool = [[NSAutoreleasePool alloc] init];
    }
    // BUG-f639: Replace assert(false) with ELog - this should never be reached, but if it is, log it instead of
    // crashing
    ELog(@"BUG-f639: Task notifier loop terminated unexpectedly - autorelease pool may leak");
    [autoreleasePool drain]; // Attempt to drain the pool as a fallback
}

// This is run in the main thread.
- (void)notifyCoprocessChange {
    [[NSNotificationCenter defaultCenter] postNotificationName:kCoprocessStatusChangeNotification object:nil];
}

- (void)lock {
    [tasksLock lock];
}

- (void)unlock {
    [tasksLock unlock];
}

@end
