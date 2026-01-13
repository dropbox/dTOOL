//
//  iTermProcessCache.m
//  DashTerm2SharedARC
//
//  Created by George Nachman on 7/18/18.
//

#import <Cocoa/Cocoa.h>

#import "DebugLogging.h"
#import "DashTerm2SharedARC-Swift.h"
#import "iTermLSOF.h"
#import "iTermProcessCache.h"
#import "iTermProcessMonitor.h"
#import "iTermRateLimitedUpdate.h"
#import "NSArray+iTerm.h"
#import <stdatomic.h>

@interface iTermProcessCache ()

// Maps process id to deepest foreground job. _lockQueue
@property (nonatomic) NSDictionary<NSNumber *, iTermProcessInfo *> *cachedDeepestForegroundJobLQ;
@property (atomic) BOOL forcingLQ;
@end

@implementation iTermProcessCache {
    dispatch_queue_t _lockQueue;
    dispatch_queue_t _workQueue;
    iTermProcessCollection *_collectionLQ;                                  // _lockQueue
    NSMutableDictionary<NSNumber *, iTermProcessMonitor *> *_trackedPidsLQ; // _lockQueue
    NSMutableArray<void (^)(void)> *_blocksLQ;                              // _lockQueue
    BOOL _needsUpdateFlagLQ;                                                // _lockQueue
    iTermRateLimitedUpdate *_rateLimit; // Main queue. keeps updateIfNeeded from eating all the CPU
    NSMutableIndexSet *_dirtyPIDsLQ;    // _lockQueue
}

+ (instancetype)sharedInstance {
    static dispatch_once_t onceToken;
    static id instance;
    dispatch_once(&onceToken, ^{
        instance = [[self alloc] init];
    });
    return instance;
}

- (instancetype)init {
    self = [super init];
    if (self) {
        _lockQueue = dispatch_queue_create("com.dashterm.dashterm2.process-cache-lock", DISPATCH_QUEUE_SERIAL);
        _workQueue = dispatch_queue_create("com.dashterm.dashterm2.process-cache-work", DISPATCH_QUEUE_SERIAL);
        // Pre-allocate for typical number of tracked processes
        _trackedPidsLQ = [NSMutableDictionary dictionaryWithCapacity:32];
        _dirtyPIDsLQ = [NSMutableIndexSet indexSet];
        // Pre-allocate for queued blocks (usually 4-16)
        _blocksLQ = [NSMutableArray arrayWithCapacity:8];

        // I'm not fond of this pattern (code that sometimes is synchronous and sometimes not) but
        // I don't want to break -setNeedsUpdate when called on the main queue and that requires
        // synchronous initialization. Job managers use the process cache on their own queues and
        // sometimes they win a race and call init before anyone else, so it has to work in this
        // case.
        if (dispatch_queue_get_label(DISPATCH_CURRENT_QUEUE_LABEL) !=
            dispatch_queue_get_label(dispatch_get_main_queue())) {
            dispatch_async(dispatch_get_main_queue(), ^{
                [self finishInitialization];
            });
        } else {
            [self finishInitialization];
        }
    }
    return self;
}

- (void)finishInitialization {
    // Perform main-thread-only initialization.
    _rateLimit = [[iTermRateLimitedUpdate alloc] initWithName:@"Process cache" minimumInterval:0.5];
    [self setNeedsUpdate:YES];
    [[NSNotificationCenter defaultCenter] addObserver:self
                                             selector:@selector(applicationDidBecomeActive:)
                                                 name:NSApplicationDidBecomeActiveNotification
                                               object:nil];
    [[NSNotificationCenter defaultCenter] addObserver:self
                                             selector:@selector(applicationDidResignActive:)
                                                 name:NSApplicationDidResignActiveNotification
                                               object:nil];
}

#pragma mark - APIs

// Main queue
- (void)setNeedsUpdate:(BOOL)needsUpdate {
    if (dispatch_queue_get_label(DISPATCH_CURRENT_QUEUE_LABEL) != dispatch_queue_get_label(dispatch_get_main_queue())) {
        DLog(@"Try again on main queue");
        dispatch_async(dispatch_get_main_queue(), ^{
            DLog(@"Trying again on main queue");
            [self setNeedsUpdate:needsUpdate];
        });
        return;
    }

    DLog(@"setNeedsUpdate:%@", @(needsUpdate));
    dispatch_sync(_lockQueue, ^{
        self->_needsUpdateFlagLQ = needsUpdate;
    });
    if (needsUpdate) {
        [_rateLimit performRateLimitedSelector:@selector(updateIfNeeded) onTarget:self withObject:nil];
    }
}

// main queue
- (void)requestImmediateUpdateWithCompletionBlock:(void (^)(void))completion {
    [self requestImmediateUpdateWithCompletionQueue:dispatch_get_main_queue() block:completion];
}

// main queue
- (void)requestImmediateUpdateWithCompletionQueue:(dispatch_queue_t)queue block:(void (^)(void))completion {
    __block BOOL needsUpdate;
    dispatch_sync(_lockQueue, ^{
        void (^wrapper)(void) = ^{
            dispatch_async(queue, completion);
        };
        [self->_blocksLQ addObject:[wrapper copy]];
        needsUpdate = self->_blocksLQ.count == 1;
    });
    if (!needsUpdate) {
        DLog(@"request immediate update just added block to queue");
        return;
    }
    DLog(@"request immediate update scheduling update");
    __weak __typeof(self) weakSelf = self;
    dispatch_async(_workQueue, ^{
        [weakSelf collectBlocksAndUpdate];
    });
}


// lockQueue
- (void)queueRequestUpdateWithCompletionQueue:(dispatch_queue_t)queue block:(void (^)(void))completion {
    __block BOOL needsUpdate;
    void (^wrapper)(void) = ^{
        dispatch_async(queue, completion);
    };
    [self->_blocksLQ addObject:[wrapper copy]];
    needsUpdate = self->_blocksLQ.count == 1;
    if (!needsUpdate) {
        DLog(@"request immediate update just added block to queue");
        return;
    }
    DLog(@"request immediate update scheduling update");
    __weak __typeof(self) weakSelf = self;
    dispatch_async(_workQueue, ^{
        [weakSelf collectBlocksAndUpdate];
    });
}

// main queue
- (void)updateSynchronously {
    dispatch_group_t group = dispatch_group_create();
    dispatch_group_enter(group);
    [self requestImmediateUpdateWithCompletionQueue:_workQueue
                                              block:^{
                                                  dispatch_group_leave(group);
                                              }];
    // BUG-11876: Add 5 second timeout to prevent indefinite hang if work queue is blocked
    dispatch_time_t timeout = dispatch_time(DISPATCH_TIME_NOW, (int64_t)(5 * NSEC_PER_SEC));
    if (dispatch_group_wait(group, timeout) != 0) {
        DLog(@"updateSynchronously timed out after 5 seconds");
    }
}

// _workQueue
- (void)collectBlocksAndUpdate {
    __block NSArray<void (^)(void)> *blocks;
    dispatch_sync(_lockQueue, ^{
        blocks = self->_blocksLQ.copy;
        [self->_blocksLQ removeAllObjects];
    });
    // BUG-f1121: Replace assert with guard - return early if no blocks to process
    if (blocks.count == 0) {
        DLog(@"Warning: collectBlocksAndUpdate called with no blocks");
        return;
    }
    DLog(@"collecting blocks and updating");
    [self reallyUpdate];

    // NOTE: blocks are called on the work queue, but they should have been wrapped with a
    // dispatch_async to the queue the caller really wants.
    for (void (^block)(void) in blocks) {
        block();
    }
}

// Any queue
- (iTermProcessInfo *)processInfoForPid:(pid_t)pid {
    __block iTermProcessInfo *info = nil;
    dispatch_sync(_lockQueue, ^{
        info = [self->_collectionLQ infoForProcessID:pid];
    });
    return info;
}

// Any queue
- (iTermProcessInfo *)deepestForegroundJobForPid:(pid_t)pid {
    __block iTermProcessInfo *result;
    NSNumber *pidNumber = [iTermLSOF cachedNumberForPid:pid];
    dispatch_sync(_lockQueue, ^{
        result = self.cachedDeepestForegroundJobLQ[pidNumber];
    });
    return result;
}

// Any queue
- (void)registerTrackedPID:(pid_t)pid {
    NSNumber *pidNumber = [iTermLSOF cachedNumberForPid:pid];
    dispatch_async(_lockQueue, ^{
        __weak __typeof(self) weakSelf = self;
        iTermProcessMonitor *monitor = [[iTermProcessMonitor alloc]
            initWithQueue:self->_lockQueue
                 callback:^(iTermProcessMonitor *monitor, dispatch_source_proc_flags_t flags) {
                     [weakSelf processMonitor:monitor didChangeFlags:flags];
                 }];
        iTermProcessInfo *info = [self->_collectionLQ infoForProcessID:pid];
        if (!info) {
            DLog(@"Request update for %@", pidNumber);
            [self queueRequestUpdateWithCompletionQueue:self->_lockQueue
                                                  block:^{
                                                      DLog(@"Got update for %@", pidNumber);
                                                      [weakSelf didUpdateForPid:pid];
                                                  }];
        } else {
            monitor.processInfo = info;
        }
        self->_trackedPidsLQ[pidNumber] = monitor;
    });
}

// lockQueue
- (void)didUpdateForPid:(pid_t)pid {
    NSNumber *pidNumber = [iTermLSOF cachedNumberForPid:pid];
    iTermProcessInfo *info = [self->_collectionLQ infoForProcessID:pid];
    if (!info) {
        DLog(@":( no info for %@", pidNumber);
        return;
    }
    iTermProcessMonitor *monitor = self->_trackedPidsLQ[pidNumber];
    if (!monitor || monitor.processInfo != nil) {
        DLog(@":( no monitor for %@", pidNumber);
        return;
    }
    DLog(@"Set info in monitor to %@", info);
    monitor.processInfo = info;
}

// lockQueue
- (void)processMonitor:(iTermProcessMonitor *)monitor didChangeFlags:(dispatch_source_proc_flags_t)flags {
    DLog(@"Flags changed for %@.", @(monitor.processInfo.processID));
    _needsUpdateFlagLQ = YES;
    const BOOL wasForced = self.forcingLQ;
    self.forcingLQ = YES;
    if (!wasForced) {
        dispatch_async(dispatch_get_main_queue(), ^{
            DLog(@"Forcing update");
            [self->_rateLimit performRateLimitedSelector:@selector(updateIfNeeded) onTarget:self withObject:nil];
            [self->_rateLimit performWithinDuration:0.0167];
            self.forcingLQ = NO;
        });
    }
}

// Main queue
- (BOOL)processIsDirty:(pid_t)pid {
    __block BOOL result;
    NSNumber *pidNumber = [iTermLSOF cachedNumberForPid:pid];
    dispatch_sync(_lockQueue, ^{
        result = [_dirtyPIDsLQ containsIndex:pid];
        if (result) {
            DLog(@"Found dirty process %@", pidNumber);
            [_dirtyPIDsLQ removeIndex:pid];
        }
    });
    return result;
}

// Any queue
- (void)unregisterTrackedPID:(pid_t)pid {
    NSNumber *pidNumber = [iTermLSOF cachedNumberForPid:pid];
    dispatch_async(_lockQueue, ^{
        [self->_trackedPidsLQ removeObjectForKey:pidNumber];
    });
}

- (void)sendSignal:(int32_t)signal toPID:(int32_t)pid {
    kill(pid, signal);
}

#pragma mark - Private

// Any queue
- (void)updateIfNeeded {
    DLog(@"updateIfNeeded");
    __block BOOL needsUpdate;
    dispatch_sync(_lockQueue, ^{
        needsUpdate = self->_needsUpdateFlagLQ;
    });
    if (!needsUpdate) {
        DLog(@"** Returning early!");
        return;
    }
    __weak __typeof(self) weakSelf = self;
    dispatch_async(_workQueue, ^{
        [weakSelf reallyUpdate];
    });
}

+ (iTermProcessCollection *)newProcessCollection {
    NSArray<NSNumber *> *allPids = [iTermLSOF allPids];
    // pid -> ppid
    NSMutableDictionary<NSNumber *, NSNumber *> *parentmap =
        [[NSMutableDictionary alloc] initWithCapacity:allPids.count];
    iTermProcessCollection *collection =
        [[iTermProcessCollection alloc] initWithDataSource:[iTermLSOF processDataSource]];
    for (NSNumber *pidNumber in allPids) {
        pid_t pid = pidNumber.intValue;

        pid_t ppid = [iTermLSOF ppidForPid:pid];
        if (!ppid) {
            continue;
        }

        NSNumber *parentPIDNumber = [iTermLSOF cachedNumberForPid:ppid];
        parentmap[pidNumber] = parentPIDNumber;
        [collection addProcessWithProcessID:pid parentProcessID:ppid];
    }
    [collection commit];
    return collection;
}

- (NSDictionary<NSNumber *, iTermProcessInfo *> *)newDeepestForegroundJobCacheWithCollection:
    (iTermProcessCollection *)collection {
    NSMutableDictionary<NSNumber *, iTermProcessInfo *> *cache = [[NSMutableDictionary alloc] initWithCapacity:64];
    __block NSSet<NSNumber *> *trackedPIDs;
    dispatch_sync(_lockQueue, ^{
        trackedPIDs = [self->_trackedPidsLQ.allKeys copy];
    });
    for (NSNumber *root in trackedPIDs) {
        iTermProcessInfo *info = [collection infoForProcessID:root.integerValue].deepestForegroundJob;
        DLog(@"iTermProcessCache: deepest fg job for %@ is %@", @(root.integerValue), @(info.processID));
        if (info) {
            cache[root] = info;
        }
    }
    return cache;
}

// _workQueue
- (void)reallyUpdate {
    DLog(@"* DOING THE EXPENSIVE THING * Process cache reallyUpdate starting");

    @autoreleasepool {
        // Do expensive stuff
        iTermProcessCollection *collection = [self.class newProcessCollection];

        // Save the tracked PIDs in the cache
        NSDictionary<NSNumber *, iTermProcessInfo *> *cachedDeepestForegroundJob =
            [self newDeepestForegroundJobCacheWithCollection:collection];

        // Flip to the new state.
        dispatch_sync(_lockQueue, ^{
            self->_cachedDeepestForegroundJobLQ = cachedDeepestForegroundJob;
            self->_collectionLQ = collection;
            self->_needsUpdateFlagLQ = NO;
            [_trackedPidsLQ enumerateKeysAndObjectsUsingBlock:^(
                                NSNumber *_Nonnull key, iTermProcessMonitor *_Nonnull monitor, BOOL *_Nonnull stop) {
                iTermProcessInfo *info = [collection infoForProcessID:key.intValue];
                if ([monitor setProcessInfo:info]) {
                    DLog(@"%@ changed! Set dirty", @(info.processID));
                    [_dirtyPIDsLQ addIndex:key.intValue];
                }
            }];
        });
    }
}

#pragma mark - Notifications

// Main queue
- (void)applicationDidResignActive:(NSNotification *)notification {
    _rateLimit.minimumInterval = 5;
}

// Main queue
- (void)applicationDidBecomeActive:(NSNotification *)notification {
    DLog(@"Application did become active (process cache)");
    _rateLimit.minimumInterval = 0.5;
}

@end
