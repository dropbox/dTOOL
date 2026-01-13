//
//  iTermPromise.m
//  DashTerm2SharedARC
//
//  Created by George Nachman on 2/10/20.
//

#import "iTermPromise.h"

#import "DebugLogging.h"
#import "NSArray+iTerm.h"
#import "NSObject+iTerm.h"
#include <os/lock.h>

@implementation iTermOr {
    id _first;
    id _second;
}

+ (instancetype)first:(id)object {
    // BUG-f1122: Replace assert with guard - return nil for nil input
    if (!object) {
        DLog(@"Warning: iTermOr first: called with nil object");
        return nil;
    }
    return [[self alloc] initWithFirst:object second:nil];
}

+ (instancetype)second:(id)object {
    // BUG-f1123: Replace assert with guard - return nil for nil input
    if (!object) {
        DLog(@"Warning: iTermOr second: called with nil object");
        return nil;
    }
    return [[self alloc] initWithFirst:nil second:object];
}

- (instancetype)initWithFirst:(id)first second:(id)second {
    self = [super init];
    if (self) {
        _first = first;
        _second = second;
    }
    return self;
}

- (void)whenFirst:(void (^NS_NOESCAPE)(id))firstBlock second:(void (^NS_NOESCAPE)(id))secondBlock {
    if (_first && firstBlock) {
        firstBlock(_first);
    } else if (_second && secondBlock) {
        secondBlock(_second);
    }
}

- (BOOL)hasFirst {
    return _first != nil;
}

- (BOOL)hasSecond {
    return _second != nil;
}

- (id)maybeFirst {
    return _first;
}

- (id)maybeSecond {
    return _second;
}

- (NSString *)description {
    __block NSString *value;
    [self
        whenFirst:^(id _Nonnull object) {
            value = [NSString stringWithFormat:@"first=%@", object];
        }
        second:^(id _Nonnull object) {
            value = [NSString stringWithFormat:@"second=%@", object];
        }];
    return [NSString stringWithFormat:@"<%@: %p %@>", NSStringFromClass(self.class), self, value];
}

- (BOOL)isEqual:(id)object {
    if (object == self) {
        return YES;
    }
    iTermOr *other = [iTermOr castFrom:object];
    if (!other) {
        return NO;
    }
    return [NSObject object:_first isEqualToObject:other->_first] && [NSObject object:_second
                                                                         isEqualToObject:other->_second];
}

- (NSUInteger)hash {
    return [_first hash] | [_second hash];
}

@end

@interface iTermPromiseSeal : NSObject <iTermPromiseSeal>

@property (nonatomic, readonly) iTermOr<id, NSError *> *value;
@property (nonatomic, readonly) void (^observer)(iTermOr<id, NSError *> *value);

+ (instancetype)new NS_UNAVAILABLE;
- (instancetype)init NS_UNAVAILABLE;
- (instancetype)initWithLock:(os_unfair_lock *)lock
                     promise:(id)promise
                    observer:(void (^)(iTermOr<id, NSError *> *))observer NS_DESIGNATED_INITIALIZER;

@end

@implementation iTermPromiseSeal {
    os_unfair_lock *_lock;
    // The seal keeps the promise from getting dealloced. This gets nilled out after fulfill/reject.
    // This works because the provider must eventually either fulfill or reject and it has to keep
    // the seal around until that happens.
    id _promise;
}

- (instancetype)initWithLock:(os_unfair_lock *)lock
                     promise:(id)promise
                    observer:(void (^)(iTermOr<id, NSError *> *))observer {
    self = [super init];
    if (self) {
        _observer = [observer copy];
        _promise = promise;
        _lock = lock;
    }
    return self;
}

- (void)dealloc {
    // BUG-f1124: Replace assert with warning - log if seal dealloc'ed without fulfill/reject
    if (_promise != nil) {
        DLog(@"Warning: iTermPromiseSeal dealloc'ed without fulfill or reject being called");
    }
}

- (void)fulfill:(id)value {
    // BUG-f1125: Replace assert with guard - return early for nil value
    if (!value) {
        DLog(@"Warning: iTermPromiseSeal fulfill: called with nil value");
        return;
    }
    os_unfair_lock_lock(_lock);
    // BUG-f1126: Replace assert with guard - prevent double fulfill
    if (_value != nil) {
        os_unfair_lock_unlock(_lock);
        DLog(@"Warning: iTermPromiseSeal fulfill: called when already fulfilled");
        return;
    }
    _value = [iTermOr first:value];
    void (^observer)(iTermOr<id, NSError *> *) = self.observer;
    iTermOr<id, NSError *> *valueToNotify = self.value;
    _promise = nil;
    os_unfair_lock_unlock(_lock);
    observer(valueToNotify);
}

- (void)reject:(NSError *)error {
    // BUG-f1127: Replace assert with guard - create generic error if none provided
    NSError *actualError = error;
    if (!actualError) {
        DLog(@"Warning: iTermPromiseSeal reject: called with nil error, using default");
        actualError = [NSError errorWithDomain:@"com.dashterm.dashterm2.promise"
                                          code:iTermPromiseErrorCodeGeneric
                                      userInfo:nil];
    }
    os_unfair_lock_lock(_lock);
    // BUG-f1128: Replace assert with guard - prevent double reject
    if (_value != nil) {
        os_unfair_lock_unlock(_lock);
        DLog(@"Warning: iTermPromiseSeal reject: called when already fulfilled/rejected");
        return;
    }
    _value = [iTermOr second:actualError];
    void (^observer)(iTermOr<id, NSError *> *) = self.observer;
    iTermOr<id, NSError *> *valueToNotify = self.value;
    _promise = nil;
    os_unfair_lock_unlock(_lock);
    observer(valueToNotify);
}

- (void)rejectWithDefaultError {
    [self reject:[NSError errorWithDomain:@"com.dashterm.dashterm2.promise"
                                     code:iTermPromiseErrorCodeGeneric
                                 userInfo:nil]];
}

@end

typedef void (^iTermPromiseCallback)(iTermOr<id, NSError *> *);

@interface iTermPromise ()
@property (nonatomic, strong) iTermOr<id, NSError *> *value;
@property (nonatomic, copy) id<iTermPromiseSeal> seal;
@property (nonatomic, strong) NSMutableArray<iTermPromiseCallback> *callbacks;
@end

@implementation iTermPromise {
  @protected
    os_unfair_lock _lock;
    BOOL _waited;
}

+ (instancetype)promise:(void (^NS_NOESCAPE)(id<iTermPromiseSeal>))block {
    return [[self alloc] initPrivate:block];
}

+ (instancetype)promiseValue:(id)value {
    return [self promise:^(id<iTermPromiseSeal> _Nonnull seal) {
        if (value) {
            [seal fulfill:value];
        } else {
            [seal rejectWithDefaultError];
        }
    }];
}

+ (instancetype)promiseError:(NSError *)value {
    return [self promise:^(id<iTermPromiseSeal> _Nonnull seal) {
        [seal reject:value];
    }];
}

+ (instancetype)promiseDefaultError {
    return [self promise:^(id<iTermPromiseSeal> _Nonnull seal) {
        [seal rejectWithDefaultError];
    }];
}

+ (void)gather:(NSArray<iTermPromise<id> *> *)promises
         queue:(dispatch_queue_t)queue
    completion:(void (^)(NSArray<iTermOr<id, NSError *> *> *values))completion {
    dispatch_group_t group = dispatch_group_create();
    [promises enumerateObjectsUsingBlock:^(iTermPromise<id> *_Nonnull promise, NSUInteger idx, BOOL *_Nonnull stop) {
        dispatch_group_enter(group);
        [[promise then:^(id _Nonnull value) {
            dispatch_group_leave(group);
        }] catchError:^(NSError *_Nonnull error) {
            dispatch_group_leave(group);
        }];
    }];
    dispatch_group_notify(group, queue, ^{
        NSArray<iTermOr<id, NSError *> *> *ors = [promises mapWithBlock:^id(iTermPromise<id> *promise) {
            return promise.value;
        }];
        completion(ors);
    });
}

- (instancetype)initPrivate:(void (^NS_NOESCAPE)(id<iTermPromiseSeal>))block {
    self = [super init];
    if (self) {
        // Pre-allocate for typical number of callbacks (usually 1-4)
        _callbacks = [NSMutableArray arrayWithCapacity:4];
        _lock = OS_UNFAIR_LOCK_INIT;
        __weak __typeof(self) weakSelf = self;
        iTermPromiseSeal *seal = [[iTermPromiseSeal alloc] initWithLock:&_lock
                                                                promise:self
                                                               observer:^(iTermOr<id, NSError *> * or) {
                                                                   [or
                                                                       whenFirst:^(id object) {
                                                                           [weakSelf didFulfill:object];
                                                                       }
                                                                       second:^(NSError *object) {
                                                                           [weakSelf didReject:object];
                                                                       }];
                                                               }];
        if (self) {
            block(seal);
        }
    }
    DLog(@"create %@", self);
    return self;
}

- (BOOL)isEqual:(id)object {
    return self == object;
}

- (NSUInteger)hash {
    NSUInteger result;
    void *selfPtr = (__bridge void *)self;
    // BUG-f1379: Replace assert with _Static_assert - compile-time check is safer than runtime crash
    // On 64-bit macOS, sizeof(NSUInteger) == sizeof(void*) == 8
    _Static_assert(sizeof(NSUInteger) == sizeof(void *), "NSUInteger must be pointer-sized");
    memmove(&result, &selfPtr, sizeof(result));
    return result;
}

- (void)didFulfill:(id)object {
    DLog(@"fulfill %@", self);
    os_unfair_lock_lock(&_lock);
    // BUG-f1180: Replace assert() with guard - double fulfill should be no-op, not crash
    if (_value) {
        os_unfair_lock_unlock(&_lock);
        DLog(@"WARNING: didFulfill called but promise already resolved, ignoring");
        return;
    }
    // BUG-1201: Set _value directly and call notifyLocked to avoid deadlock.
    // Previously called self.value = ... which tried to re-acquire the lock.
    _value = [iTermOr first:object];
    [self notifyLocked];
    os_unfair_lock_unlock(&_lock);
}

- (void)didReject:(NSError *)error {
    os_unfair_lock_lock(&_lock);
    // BUG-f1181: Replace assert() with guard - double reject should be no-op, not crash
    if (_value) {
        os_unfair_lock_unlock(&_lock);
        DLog(@"WARNING: didReject called but promise already resolved, ignoring");
        return;
    }
    // BUG-1201: Set _value directly and call notifyLocked to avoid deadlock.
    // Previously called self.value = ... which tried to re-acquire the lock.
    _value = [iTermOr second:error];
    [self notifyLocked];
    os_unfair_lock_unlock(&_lock);
}

- (void)setValue:(iTermOr<id, NSError *> *)value {
    os_unfair_lock_lock(&_lock);
    // BUG-f1182: Replace assert() with guard - double setValue should be no-op, not crash
    if (_value) {
        os_unfair_lock_unlock(&_lock);
        DLog(@"WARNING: setValue called but promise already has value, ignoring");
        return;
    }
    // BUG-f1183: Replace assert() with guard - nil value should be no-op, not crash
    if (!value) {
        os_unfair_lock_unlock(&_lock);
        DLog(@"WARNING: setValue called with nil value, ignoring");
        return;
    }

    _value = value;
    [self notifyLocked];
    os_unfair_lock_unlock(&_lock);
}

- (void)addCallback:(iTermPromiseCallback)callback {
    os_unfair_lock_lock(&_lock);
    // BUG-f1184: Replace assert() with guard - nil callback should be no-op, not crash
    if (!callback) {
        os_unfair_lock_unlock(&_lock);
        DLog(@"WARNING: addCallback called with nil callback, ignoring");
        return;
    }
    // BUG-f1185: Replace assert() with guard - nil _callbacks should be no-op, not crash
    if (!_callbacks) {
        os_unfair_lock_unlock(&_lock);
        DLog(@"WARNING: addCallback called but _callbacks is nil, ignoring");
        return;
    }
    [_callbacks addObject:[callback copy]];

    [self notifyLocked];
    os_unfair_lock_unlock(&_lock);
}

// Must be called with _lock held
- (void)addCallbackLocked:(iTermPromiseCallback)callback {
    // BUG-f1184: Replace assert() with guard - nil callback should be no-op, not crash
    if (!callback) {
        DLog(@"WARNING: addCallbackLocked called with nil callback, ignoring");
        return;
    }
    // BUG-f1185: Replace assert() with guard - nil _callbacks should be no-op, not crash
    if (!_callbacks) {
        DLog(@"WARNING: addCallbackLocked called but _callbacks is nil, ignoring");
        return;
    }
    [_callbacks addObject:[callback copy]];
    [self notifyLocked];
}

// Must be called with _lock held.
// WARNING: Callbacks are executed OUTSIDE the lock to prevent deadlocks.
// BUG-1202: Fixed deadlock - callbacks (especially from waitWithTimeout) may try to
// acquire the lock, causing deadlock if we invoke them while holding the lock.
- (void)notifyLocked {
    iTermOr<id, NSError *> *value = _value;
    if (!value) {
        return;
    }
    NSArray<iTermPromiseCallback> *callbacks = [_callbacks copy];
    [_callbacks removeAllObjects];

    // Release the lock before invoking callbacks to prevent deadlock.
    // Callbacks may call back into the promise (e.g., to check timedOut flag).
    os_unfair_lock_unlock(&_lock);

    [callbacks
        enumerateObjectsUsingBlock:^(iTermPromiseCallback _Nonnull callback, NSUInteger idx, BOOL *_Nonnull stop) {
            callback(value);
        }];

    // Re-acquire the lock since caller expects it to still be held.
    os_unfair_lock_lock(&_lock);
}

- (iTermPromise *)then:(void (^)(id))block {
    os_unfair_lock_lock(&_lock);
    iTermPromise *next = [iTermPromise promise:^(id<iTermPromiseSeal> seal) {
        [self addCallbackLocked:^(iTermOr<id, NSError *> *value) {
            // _lock is held at this point since this is called from -notifyLocked.
            [value
                whenFirst:^(id object) {
                    block(object);
                    [seal fulfill:object];
                }
                second:^(NSError *object) {
                    [seal reject:object];
                }];
        }];
    }];
    os_unfair_lock_unlock(&_lock);
    return next;
}

- (iTermPromise *)catchError:(void (^)(NSError *error))block {
    os_unfair_lock_lock(&_lock);
    iTermPromise *next = [iTermPromise promise:^(id<iTermPromiseSeal> seal) {
        [self addCallbackLocked:^(iTermOr<id, NSError *> *value) {
            // _lock is held at this point since this is called from -notifyLocked.
            [value
                whenFirst:^(id object) {
                    [seal fulfill:object];
                }
                second:^(NSError *object) {
                    block(object);
                    [seal reject:object];
                }];
        }];
    }];
    os_unfair_lock_unlock(&_lock);
    return next;
}

static void iTermPromiseRunBlockOnQueue(dispatch_queue_t queue, id parameter, void (^block)(id)) {
    if (dispatch_queue_get_label(DISPATCH_CURRENT_QUEUE_LABEL) == dispatch_queue_get_label(queue)) {
        block(parameter);
        return;
    }
    dispatch_async(queue, ^{
        block(parameter);
    });
}

- (iTermPromise *)onQueue:(dispatch_queue_t)queue then:(void (^)(id value))block {
    return [self then:^(id _Nonnull value) {
        iTermPromiseRunBlockOnQueue(queue, value, block);
    }];
}

- (iTermPromise *)onQueue:(dispatch_queue_t)queue catchError:(void (^)(NSError *error))block {
    return [self catchError:^(NSError *_Nonnull error) {
        iTermPromiseRunBlockOnQueue(queue, error, block);
    }];
}

- (BOOL)hasValue {
    os_unfair_lock_lock(&_lock);
    BOOL result = self.value != nil;
    os_unfair_lock_unlock(&_lock);
    return result;
}

- (id)maybeValue {
    os_unfair_lock_lock(&_lock);
    iTermOr<id, NSError *> *valueSnapshot = self.value;
    os_unfair_lock_unlock(&_lock);
    __block id result = nil;
    [valueSnapshot
        whenFirst:^(id _Nonnull object) {
            result = object;
        }
        second:^(NSError *_Nonnull object) {
            result = nil;
        }];
    return result;
}

- (id)maybeError {
    os_unfair_lock_lock(&_lock);
    iTermOr<id, NSError *> *valueSnapshot = self.value;
    os_unfair_lock_unlock(&_lock);
    __block NSError *result = nil;
    [valueSnapshot
        whenFirst:^(id _Nonnull object) {
            result = nil;
        }
        second:^(NSError *_Nonnull error) {
            result = error;
        }];
    return result;
}


- (iTermOr<id, NSError *> *)wait {
    return [self waitWithTimeout:INFINITY];
}


- (iTermOr *)waitWithTimeout:(NSTimeInterval)timeout {
    // BUG-f1639: Use dispatch_semaphore instead of dispatch_group.
    // dispatch_group has a strict enter/leave balance requirement that caused crashes
    // ("Unbalanced call to dispatch_group_leave()") when promises were reused across
    // multiple waitWithTimeout calls (as in iTermMetalView.fetchDrawable).
    // dispatch_semaphore is simpler: signal() can be called multiple times without error,
    // and there's no "unbalanced" concept to worry about.
    dispatch_semaphore_t semaphore = dispatch_semaphore_create(0);
    __block BOOL signaled = NO;
    os_unfair_lock_lock(&_lock);
    if (self.value != nil) {
        iTermOr<id, NSError *> *result = self.value;
        os_unfair_lock_unlock(&_lock);
        return result;
    }
    _waited = YES;
    // Capture lock pointer directly to avoid retain cycle warning.
    // This is safe because the callback is always invoked synchronously within this method
    // (we wait on the semaphore), so self cannot be deallocated while the lock is in use.
    os_unfair_lock *lockPtr = &_lock;
    [self addCallbackLocked:^(iTermOr<id, NSError *> *result) {
        // Signal the semaphore when the promise is fulfilled/rejected.
        // Use lock + flag to ensure we only signal once (for efficiency, not correctness -
        // signaling multiple times is safe but wasteful).
        os_unfair_lock_lock(lockPtr);
        if (!signaled) {
            signaled = YES;
            dispatch_semaphore_signal(semaphore);
        }
        os_unfair_lock_unlock(lockPtr);
    }];
    os_unfair_lock_unlock(&_lock);
    if (timeout == INFINITY) {
        dispatch_semaphore_wait(semaphore, DISPATCH_TIME_FOREVER);
    } else {
        if (dispatch_semaphore_wait(semaphore, dispatch_time(DISPATCH_TIME_NOW, (int64_t)(timeout * NSEC_PER_SEC)))) {
            // Timed out. Unlike dispatch_group, we don't need to "leave" or balance anything.
            // The semaphore simply wasn't signaled in time. If the callback signals later,
            // that's fine - the semaphore count just goes from 0 to 1, which is harmless
            // since we're already returning.
            return [iTermOr second:[NSError errorWithDomain:@"com.dashterm.dashterm2.promise"
                                                       code:iTermPromiseErrorCodeTimeout
                                                   userInfo:nil]];
        }
    }
    os_unfair_lock_lock(&_lock);
    iTermOr<id, NSError *> *finalValue = self.value;
    os_unfair_lock_unlock(&_lock);
    return finalValue;
}

@end

@implementation iTermRenegablePromise

+ (instancetype)promise:(void (^NS_NOESCAPE)(id<iTermPromiseSeal> seal))block renege:(void (^)(void))renege {
    iTermRenegablePromise *promise = [super promise:block];
    if (promise) {
        promise->_renegeBlock = [renege copy];
    }
    return promise;
}

- (void)dealloc {
    DLog(@"dealloc %@", self);
    [self renege];
}

- (void)renege {
    os_unfair_lock_lock(&_lock);
    if (self.value || _waited) {
        os_unfair_lock_unlock(&_lock);
        return;
    }
    void (^block)(void) = nil;
    if (_renegeBlock) {
        block = _renegeBlock;
        _renegeBlock = nil;
    }
    os_unfair_lock_unlock(&_lock);
    if (block) {
        block();
    }
}

@end
