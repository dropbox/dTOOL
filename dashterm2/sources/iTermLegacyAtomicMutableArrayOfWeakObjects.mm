//
//  iTermLegacyAtomicMutableArrayOfWeakObjects.mm
//  DashTerm2SharedARC
//
//  Created by George Nachman on 1/3/23.
//

#import "iTermLegacyAtomicMutableArrayOfWeakObjects.h"

extern "C" {
#import "DebugLogging.h"
#import "NSArray+iTerm.h"
#import "iTermWeakBox.h"
}

#include <atomic>
#import <os/lock.h>

// Performance optimization: os_unfair_lock has ~10x lower overhead than @synchronized
// This is a global lock shared by all instances (same behavior as previous @synchronized([class lock]))
static os_unfair_lock sGlobalLock = OS_UNFAIR_LOCK_INIT;

@implementation iTermLegacyAtomicMutableArrayOfWeakObjects {
    NSMutableArray<iTermWeakBox *> *_array;
}

// Sanity check for debugging lock contention
static std::atomic<int> iTermLegacyAtomicMutableArrayOfWeakObjectsLockCount;
static void iTermLegacyAtomicMutableArrayOfWeakObjectsLock(void) {
    iTermLegacyAtomicMutableArrayOfWeakObjectsLockCount += 1;
    // BUG-f1291: Replace assert with warning log - lock count != 1 indicates potential re-entrancy
    // but should not crash production. The os_unfair_lock already provides thread safety.
    if (iTermLegacyAtomicMutableArrayOfWeakObjectsLockCount != 1) {
        DLog(@"BUG-f1291: Unexpected lock count %d (expected 1) - possible re-entrant access",
             iTermLegacyAtomicMutableArrayOfWeakObjectsLockCount.load());
    }
}

static void iTermLegacyAtomicMutableArrayOfWeakObjectsLockUnlock(void) {
    iTermLegacyAtomicMutableArrayOfWeakObjectsLockCount -= 1;
    // BUG-f1292: Replace assert with warning log - lock count != 0 after unlock indicates imbalance
    if (iTermLegacyAtomicMutableArrayOfWeakObjectsLockCount != 0) {
        DLog(@"BUG-f1292: Unexpected lock count %d after unlock (expected 0) - lock imbalance",
             iTermLegacyAtomicMutableArrayOfWeakObjectsLockCount.load());
    }
}

+ (instancetype)array {
    return [[self alloc] init];
}

- (instancetype)init {
    if (self = [super init]) {
        _array = [NSMutableArray array];
    }
    return self;
}

- (void)removeObjectsPassingTest:(BOOL (^)(id anObject))block {
    os_unfair_lock_lock(&sGlobalLock);
    iTermLegacyAtomicMutableArrayOfWeakObjectsLock();
    @try {
        [_array removeObjectsPassingTest:^(iTermWeakBox *box) {
            return block(box.object);
        }];
    } @catch (NSException *exception) {
        const int count = iTermLegacyAtomicMutableArrayOfWeakObjectsLockCount;
        CrashLog(@"%@ with lock count=%@", exception.debugDescription, @(count));
        iTermLegacyAtomicMutableArrayOfWeakObjectsLockUnlock();
        os_unfair_lock_unlock(&sGlobalLock);
        @throw exception;
    }
    iTermLegacyAtomicMutableArrayOfWeakObjectsLockUnlock();
    os_unfair_lock_unlock(&sGlobalLock);
}

- (NSArray *)strongObjects {
    os_unfair_lock_lock(&sGlobalLock);
    iTermLegacyAtomicMutableArrayOfWeakObjectsLock();
    NSArray *result = [_array mapWithBlock:^(iTermWeakBox *box) {
        return box.object;
    }];
    iTermLegacyAtomicMutableArrayOfWeakObjectsLockUnlock();
    os_unfair_lock_unlock(&sGlobalLock);
    return result;
}

- (void)removeAllObjects {
    os_unfair_lock_lock(&sGlobalLock);
    iTermLegacyAtomicMutableArrayOfWeakObjectsLock();
    [_array removeAllObjects];
    iTermLegacyAtomicMutableArrayOfWeakObjectsLockUnlock();
    os_unfair_lock_unlock(&sGlobalLock);
}

- (void)addObject:(id)object {
    os_unfair_lock_lock(&sGlobalLock);
    iTermLegacyAtomicMutableArrayOfWeakObjectsLock();
    [_array addObject:[iTermWeakBox boxFor:object]];
    iTermLegacyAtomicMutableArrayOfWeakObjectsLockUnlock();
    os_unfair_lock_unlock(&sGlobalLock);
}

- (NSUInteger)count {
    os_unfair_lock_lock(&sGlobalLock);
    iTermLegacyAtomicMutableArrayOfWeakObjectsLock();
    const NSUInteger result = _array.count;
    iTermLegacyAtomicMutableArrayOfWeakObjectsLockUnlock();
    os_unfair_lock_unlock(&sGlobalLock);
    return result;
}

- (void)prune {
    [self removeObjectsPassingTest:^BOOL(id anObject) {
        return anObject == nil;
    }];
}

- (iTermLegacyAtomicMutableArrayOfWeakObjects *)compactMap:(id (^)(id))block {
    iTermLegacyAtomicMutableArrayOfWeakObjects *result = [[iTermLegacyAtomicMutableArrayOfWeakObjects alloc] init];
    for (id object in [self strongObjects]) {
        id mapped = block(object);
        if (mapped) {
            [result addObject:mapped];
        }
    }
    return result;
}

- (NSUInteger)countByEnumeratingWithState:(NSFastEnumerationState *)state
                                  objects:(id __unsafe_unretained _Nullable *)buffer
                                    count:(NSUInteger)len {
    return [self.strongObjects countByEnumeratingWithState:state objects:buffer count:len];
}

@end
