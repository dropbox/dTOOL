//
//  VT100TokenPool.m
//  DashTerm2
//
//  Created by DashTerm2 Performance Audit on 2025-12-17.
//  Part of DashTerm2 Performance Optimization - Iteration 5
//
//  Token pooling eliminates 15-25% CPU overhead from token allocation.
//  See reports/main/PERFORMANCE_AUDIT_003.md for design details.
//

#import "VT100TokenPool.h"
#import "VT100Token.h"
#import <os/lock.h>

// Initial pool size - preallocate to avoid cold-start latency
static const NSUInteger kInitialPoolSize = 64;

// Maximum pool size - prevent unbounded memory growth
static const NSUInteger kMaxPoolSize = 256;

@implementation VT100TokenPool {
    NSMutableArray<VT100Token *> *_freeList;
    os_unfair_lock _lock;
    dispatch_source_t _memoryPressureSource;

    // Statistics
    NSUInteger _activeCount;
    NSUInteger _recycledCount;
    NSUInteger _acquiredCount;
    NSUInteger _poolMissCount;
}

+ (instancetype)sharedPool {
    static VT100TokenPool *pool;
    static dispatch_once_t onceToken;
    dispatch_once(&onceToken, ^{
        pool = [[VT100TokenPool alloc] init];
    });
    return pool;
}

+ (VT100Token *)unpooledToken {
    return [VT100Token token];
}

- (instancetype)init {
    self = [super init];
    if (self) {
        _freeList = [[NSMutableArray alloc] initWithCapacity:kMaxPoolSize];
        _lock = OS_UNFAIR_LOCK_INIT;

        // Pre-allocate tokens to avoid cold-start latency
        for (NSUInteger i = 0; i < kInitialPoolSize; i++) {
            VT100Token *token = [[VT100Token alloc] init];
            token.pooled = YES;
            [_freeList addObject:token];
        }

        [self setupMemoryPressureHandler];
    }
    return self;
}

- (void)setupMemoryPressureHandler {
    _memoryPressureSource = dispatch_source_create(DISPATCH_SOURCE_TYPE_MEMORYPRESSURE, 0,
                                                   DISPATCH_MEMORYPRESSURE_WARN | DISPATCH_MEMORYPRESSURE_CRITICAL,
                                                   dispatch_get_main_queue());

    __weak __typeof(self) weakSelf = self;
    dispatch_source_t source = _memoryPressureSource;
    dispatch_source_set_event_handler(_memoryPressureSource, ^{
        __strong __typeof(weakSelf) strongSelf = weakSelf;
        if (!strongSelf) {
            return;
        }
        dispatch_source_memorypressure_flags_t flags = dispatch_source_get_data(source);
        if (flags & DISPATCH_MEMORYPRESSURE_CRITICAL) {
            NSLog(@"Critical memory pressure detected, draining VT100TokenPool");
            [strongSelf drain];
        } else if (flags & DISPATCH_MEMORYPRESSURE_WARN) {
            NSLog(@"Memory pressure warning detected, trimming VT100TokenPool to 50%%");
            [strongSelf trimToSize:kMaxPoolSize / 2];
        }
    });
    dispatch_resume(_memoryPressureSource);
}

- (void)dealloc {
    // Cancel memory pressure source
    if (_memoryPressureSource) {
        dispatch_source_cancel(_memoryPressureSource);
    }
    // Release all pooled tokens
    for (VT100Token *token in _freeList) {
        [token release];
    }
    [_freeList release];
    [super dealloc];
}

- (VT100Token *)acquireToken {
    __block VT100Token *token = nil;

    os_unfair_lock_lock(&_lock);
    _acquiredCount++;

    if (_freeList.count > 0) {
        // Fast path: reuse from pool
        token = [[_freeList lastObject] retain];
        [_freeList removeLastObject];
    } else {
        // Slow path: allocate new token
        _poolMissCount++;
        token = [[VT100Token alloc] init];
        token.pooled = YES;
    }
    _activeCount++;
    os_unfair_lock_unlock(&_lock);

    return token;
}

- (void)recycleToken:(VT100Token *)token {
    if (!token) {
        return;
    }

    if (!token.pooled) {
        // Not from pool, let normal memory management handle it
        return;
    }

    // Reset token state before returning to pool
    [token resetForPoolReuse];

    BOOL shouldRelease = NO;
    os_unfair_lock_lock(&_lock);
    _activeCount--;
    _recycledCount++;

    if (_freeList.count < kMaxPoolSize) {
        // Return to pool for reuse
        [_freeList addObject:token];
    } else {
        // Pool is full, release the token after unlocking
        shouldRelease = YES;
    }
    os_unfair_lock_unlock(&_lock);

    if (shouldRelease) {
        [token release];
    }
}

#pragma mark - Statistics

- (NSUInteger)poolSize {
    os_unfair_lock_lock(&_lock);
    const NSUInteger size = _freeList.count;
    os_unfair_lock_unlock(&_lock);
    return size;
}

- (NSUInteger)activeTokens {
    os_unfair_lock_lock(&_lock);
    const NSUInteger count = _activeCount;
    os_unfair_lock_unlock(&_lock);
    return count;
}

- (NSUInteger)recycledCount {
    os_unfair_lock_lock(&_lock);
    const NSUInteger count = _recycledCount;
    os_unfair_lock_unlock(&_lock);
    return count;
}

- (NSUInteger)acquiredCount {
    os_unfair_lock_lock(&_lock);
    const NSUInteger count = _acquiredCount;
    os_unfair_lock_unlock(&_lock);
    return count;
}

- (NSUInteger)poolMissCount {
    os_unfair_lock_lock(&_lock);
    const NSUInteger count = _poolMissCount;
    os_unfair_lock_unlock(&_lock);
    return count;
}

- (void)resetStatistics {
    os_unfair_lock_lock(&_lock);
    _recycledCount = 0;
    _acquiredCount = 0;
    _poolMissCount = 0;
    os_unfair_lock_unlock(&_lock);
}

- (NSString *)statisticsDescription {
    os_unfair_lock_lock(&_lock);
    const NSUInteger poolSize = _freeList.count;
    const NSUInteger active = _activeCount;
    const NSUInteger acquired = _acquiredCount;
    const NSUInteger recycled = _recycledCount;
    const NSUInteger misses = _poolMissCount;
    os_unfair_lock_unlock(&_lock);

    const double hitRate = acquired > 0 ? (1.0 - (double)misses / (double)acquired) * 100.0 : 0.0;
    return [NSString stringWithFormat:
            @"VT100TokenPool: poolSize=%lu active=%lu acquired=%lu recycled=%lu misses=%lu hitRate=%.1f%%",
            (unsigned long)poolSize,
            (unsigned long)active,
            (unsigned long)acquired,
            (unsigned long)recycled,
            (unsigned long)misses,
            hitRate];
}

- (void)drain {
    os_unfair_lock_lock(&_lock);
    NSUInteger count = _freeList.count;
    for (VT100Token *token in _freeList) {
        [token release];
    }
    [_freeList removeAllObjects];
    os_unfair_lock_unlock(&_lock);

    NSLog(@"VT100TokenPool drained %lu tokens", (unsigned long)count);
}

- (void)trimToSize:(NSUInteger)size {
    os_unfair_lock_lock(&_lock);
    NSUInteger removed = 0;
    while (_freeList.count > size) {
        VT100Token *token = [_freeList lastObject];
        [_freeList removeLastObject];
        [token release];
        removed++;
    }
    os_unfair_lock_unlock(&_lock);

    if (removed > 0) {
        NSLog(@"VT100TokenPool trimmed %lu tokens to size %lu", (unsigned long)removed, (unsigned long)size);
    }
}

@end
