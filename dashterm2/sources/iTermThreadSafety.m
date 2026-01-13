//
//  iTermThreadSafety.m
//  DashTerm2
//
//  Created by George Nachman on 3/14/20.
//

#import "iTermThreadSafety.h"

#import "DebugLogging.h"
#import "NSArray+iTerm.h"
#import "NSObject+iTerm.h"
#import <os/lock.h>

#define CHECK_DOUBLE_INVOKES BETA

@interface iTermSynchronizedState()
@property (atomic) BOOL ready;
@end

@implementation iTermSynchronizedState {
    const char *_queueLabel;
}

- (instancetype)initWithQueue:(dispatch_queue_t)queue {
    self = [super init];
    if (self) {
        _queue = queue;
        _queueLabel = dispatch_queue_get_label(queue);
        // BUG-f1186: Replace assert with guard - nil queue label should fail init not crash
        if (!_queueLabel) {
            DLog(@"ERROR: iTermSynchronizedState initWithQueue: queue has nil label");
            [self release];
            return nil;
        }
    }
    return self;
}

- (void)dealloc {
    [super dealloc];
}

static void Check(iTermSynchronizedState *self) {
    // BUG-f1187: Replace assert with warning log - wrong queue should warn not crash
    if (dispatch_queue_get_label(DISPATCH_CURRENT_QUEUE_LABEL) != self->_queueLabel) {
        DLog(@"WARNING: iTermSynchronizedState Check: called from wrong queue. Expected %s, got %s",
             self->_queueLabel ?: "(null)",
             dispatch_queue_get_label(DISPATCH_CURRENT_QUEUE_LABEL) ?: "(null)");
    }
}

- (instancetype)retain {
    id result = [super retain];
    [self check];
    return result;
}

// Can't check on release because autorelease pools can be drained on a different thread.

- (instancetype)autorelease {
    [self check];
    return [super autorelease];
}

- (id)state {
    [self check];
    return self;
}

- (void)check {
    if (self.ready) {
        Check(self);
    }
}

@end

@implementation iTermMainThreadState
+ (instancetype)uncheckedSharedInstance {
    static iTermMainThreadState *instance;
    static dispatch_once_t onceToken;
    dispatch_once(&onceToken, ^{
        instance = [[iTermMainThreadState alloc] initWithQueue:dispatch_get_main_queue()];
    });
    return instance;
}

+ (instancetype)sharedInstance {
    iTermMainThreadState *instance = [self uncheckedSharedInstance];
    [instance check];
    return instance;
}
@end

#if CHECK_DOUBLE_INVOKES
// Weak references to all iTermThread objects.
NSPointerArray *gThreads;
static os_unfair_lock gThreadsLock = OS_UNFAIR_LOCK_INIT;
#endif

@implementation iTermThread {
    iTermSynchronizedState *_state;
    NSMutableArray *_deferred;
    os_unfair_lock _deferredLock;
#if CHECK_DOUBLE_INVOKES
    NSArray<NSString *> *_stacks;
#endif
}

+ (instancetype)main {
    static iTermThread *instance;
    static dispatch_once_t onceToken;
    dispatch_once(&onceToken, ^{
        instance = [[self alloc] initWithQueue:dispatch_get_main_queue()
                              stateFactory:
                ^iTermSynchronizedState * _Nullable(dispatch_queue_t  _Nonnull queue) {
            return [iTermMainThreadState uncheckedSharedInstance];
        }];
    });
    return instance;
}

+ (instancetype)withLabel:(NSString *)label
             stateFactory:(iTermThreadStateFactoryBlockType)stateFactory {
    return [[self alloc] initWithLabel:label stateFactory:stateFactory];
}

#if CHECK_DOUBLE_INVOKES
+ (iTermThread *)currentThread {
    const char *currentLabel = dispatch_queue_get_label(DISPATCH_CURRENT_QUEUE_LABEL);
    iTermThread *result = nil;
    os_unfair_lock_lock(&gThreadsLock);
    for (iTermThread *thread in gThreads) {
        if ([[thread retain] autorelease] == nil) {
            continue;
        }
        if (dispatch_queue_get_label(thread->_queue) == currentLabel) {
            result = thread;
            break;
        }
    }
    os_unfair_lock_unlock(&gThreadsLock);
    return result;
}
#endif

- (instancetype)initWithQueue:(dispatch_queue_t)queue
                 stateFactory:(iTermThreadStateFactoryBlockType)stateFactory {
    self = [super init];
    if (self) {
        _queue = queue;
        dispatch_retain(_queue);
        _state = [stateFactory(_queue) retain];
        _state.ready = YES;
        _deferredLock = OS_UNFAIR_LOCK_INIT;
#if CHECK_DOUBLE_INVOKES
        _stacks = [@[] retain];
        static dispatch_once_t onceToken;
        dispatch_once(&onceToken, ^{
            gThreads = [[NSPointerArray weakObjectsPointerArray] retain];
        });
        os_unfair_lock_lock(&gThreadsLock);
        [gThreads addPointer:self];
        os_unfair_lock_unlock(&gThreadsLock);
#endif
    }
    return self;
}

+ (NSString *)uniqueQueueLabelWithName:(NSString *)label {
    static _Atomic int threadNumber;
    int i = threadNumber++;
    return [NSString stringWithFormat:@"%@.%d", label, i];
}

- (instancetype)initWithLabel:(NSString *)label
                 stateFactory:(iTermThreadStateFactoryBlockType)stateFactory {
    const char *cstr = [iTermThread uniqueQueueLabelWithName:label].UTF8String;
    return [self initWithQueue:dispatch_queue_create(cstr, DISPATCH_QUEUE_SERIAL)
                  stateFactory:stateFactory];
}

- (void)dealloc {
    dispatch_release(_queue);
    // BUG-f1188: Replace assert with warning - deferred still set at dealloc is a leak not a crash
    if (_deferred) {
        DLog(@"WARNING: iTermThread dealloc: _deferred still set - %lu pending blocks will be lost",
             (unsigned long)_deferred.count);
        [_deferred release];
        _deferred = nil;
    }
    [_state release];
#if CHECK_DOUBLE_INVOKES
    [_stacks release];
#endif
    [super dealloc];
}

- (NSString *)description {
    return [NSString stringWithFormat:@"<%@: %p queue=%@>",
            NSStringFromClass(self.class), self, _queue];
}

- (NSString *)label {
    return [NSString stringWithUTF8String:dispatch_queue_get_label(_queue)];
}

#if CHECK_DOUBLE_INVOKES
- (NSString *)stack {
    return [_stacks componentsJoinedByString:@"\n\n"];
}

- (NSArray<NSString *> *)currentStacks {
    NSArray *frames = [[NSThread callStackSymbols] subarrayFromIndex:1];
    frames = [@[ self.label ] arrayByAddingObjectsFromArray:frames];
    NSString *stack = [frames componentsJoinedByString:@"\n"];
    return [@[stack] arrayByAddingObjectsFromArray:_stacks];
}

- (NSString *)currentStack {
    return [self.currentStacks componentsJoinedByString:@"\n\n"];
}
#endif

- (void)performDeferredBlocksAfter:(void (^ NS_NOESCAPE)(void))block {
    os_unfair_lock_lock(&_deferredLock);
    // BUG-f1189: Replace assert with guard - nested performDeferredBlocksAfter should no-op not crash
    if (_deferred) {
        os_unfair_lock_unlock(&_deferredLock);
        DLog(@"WARNING: iTermThread performDeferredBlocksAfter: called while already deferring - nested call ignored");
        block();
        return;
    }
    _deferred = [NSMutableArray arrayWithCapacity:16];  // Typical deferred block count
    os_unfair_lock_unlock(&_deferredLock);
    block();
    while (YES) {
        NSArray *blocks = nil;
        os_unfair_lock_lock(&_deferredLock);
        blocks = [[_deferred copy] autorelease];
        [_deferred removeAllObjects];
        if (blocks.count == 0) {
            _deferred = nil;
            os_unfair_lock_unlock(&_deferredLock);
            break;
        }
        os_unfair_lock_unlock(&_deferredLock);
        for (void (^block)(id) in blocks) {
            [self dispatchSync:^(id  _Nullable state) {
                block(state);
            }];
        }
    }
}

- (void)dispatchAsync:(void (^)(id))block {
    [self retain];
    os_unfair_lock_lock(&_deferredLock);
    if (_deferred) {
        [_deferred addObject:[[block copy] autorelease]];
        os_unfair_lock_unlock(&_deferredLock);
        return;
    }
    os_unfair_lock_unlock(&_deferredLock);
#if CHECK_DOUBLE_INVOKES
    NSArray *stacks = [[iTermThread currentThread] currentStacks] ?: @[ [[NSThread callStackSymbols]  componentsJoinedByString:@"\n"] ];
    [stacks retain];
#endif
    dispatch_async(_queue, ^{
#if CHECK_DOUBLE_INVOKES
        NSArray *saved = [_stacks retain];
        _stacks = stacks;
#endif
        block(self->_state);
#if CHECK_DOUBLE_INVOKES
        _stacks = saved;
        [stacks release];
        [saved release];
#endif
        [self release];
    });
}

- (void)dispatchSync:(void (^ NS_NOESCAPE)(id))block {
    [self retain];
    // BUG-f1190: Replace assert with recursive dispatch - same queue should use recursive sync not crash
    if (dispatch_queue_get_label(DISPATCH_CURRENT_QUEUE_LABEL) == dispatch_queue_get_label(_queue)) {
        DLog(@"WARNING: iTermThread dispatchSync: called on same queue - using recursive dispatch");
        block(self->_state);
        [self release];
        return;
    }
    dispatch_sync(_queue, ^{
        block(self->_state);
        [self release];
    });
}

- (void)dispatchRecursiveSync:(void (^ NS_NOESCAPE)(id))block {
    if (dispatch_queue_get_label(DISPATCH_CURRENT_QUEUE_LABEL) == dispatch_queue_get_label(_queue)) {
        block(self->_state);
    } else {
        [self dispatchSync:block];
    }
}

- (iTermCallback *)newCallbackWithBlock:(void (^)(id, id))callback {
    return [[iTermCallback onThread:self block:callback] retain];
}

- (iTermCallback *)newCallbackWithWeakTarget:(id)target selector:(SEL)selector userInfo:(id)userInfo {
    __weak id weakTarget = target;
    return [self newCallbackWithBlock:^(id  _Nonnull state, id  _Nullable value) {
        [weakTarget it_performNonObjectReturningSelector:selector
                                              withObject:state
                                                  object:value
                                                  object:userInfo];
    }];
}

- (void)check {
    [_state check];
}

@end

@implementation iTermCallback {
    void (^_block)(id, id);
    dispatch_group_t _group;
#if CHECK_DOUBLE_INVOKES
    NSString *_invokeStack;
    NSString *_creationStack;
    int _magic;
    NSMutableString *_debugInfo;
#endif
}

+ (instancetype)onThread:(iTermThread *)thread block:(void (^)(id, id))block {
    return [[[self alloc] initWithThread:thread block:block] autorelease];
}

- (instancetype)initWithThread:(iTermThread *)thread block:(void (^)(id, id))block {
    self = [super init];
    if (self) {
        _thread = [thread retain];
        _block = [block copy];
        _group = dispatch_group_create();
        dispatch_group_enter(_group);
#if CHECK_DOUBLE_INVOKES
        _debugInfo = [[NSMutableString alloc] init];
        _magic = 0xdeadbeef;
        _creationStack = [[[NSThread callStackSymbols] componentsJoinedByString:@"\n"] copy];
#endif
    }
    return self;
}

- (void)dealloc {
    [_thread release];
    [_block release];
#if CHECK_DOUBLE_INVOKES
    // BUG-f1191: Replace assert with warning - double-free detection should warn not crash
    if (_magic != 0xdeadbeef) {
        DLog(@"ERROR: iTermCallback dealloc: magic value corrupted (possible double-free). Expected 0xdeadbeef, got 0x%x", _magic);
    }
    [_invokeStack release];
    [_creationStack release];
    [_debugInfo release];
    _magic = 0;
#endif
    dispatch_release(_group);
    _block = nil;
    _group = nil;
    [super dealloc];
}

- (void)invokeWithObject:(id)object {
#if DEBUG
    BOOL trace = self.trace;
#endif
    void (^block)(id, id) = [_block retain];
    [self retain];
#if CHECK_DOUBLE_INVOKES
    NSString *stack = [[[iTermThread currentThread] currentStack] retain];
#endif
    [_thread dispatchAsync:^(iTermSynchronizedState *state) {
#if CHECK_DOUBLE_INVOKES
        ITAssertWithMessage(!self->_invokeStack, @"Previously invoked from:\n%@\n\nNow invoked from:\n%@\n\nCreated from:\n%@\n%@",
                     _invokeStack, stack, _creationStack, _debugInfo);
        _invokeStack = [stack copy];
        [stack release];
#endif
#if DEBUG
        if (trace) {
            NSLog(@"%@", stack);
        }
#endif
        block(state, object);
        [block release];
        dispatch_group_leave(_group);
        [self release];
    }];
}

- (void)invokeMaybeImmediatelyWithObject:(id)object {
    void (^block)(id, id) = [_block retain];
    [self retain];
#if CHECK_DOUBLE_INVOKES
    NSString *stack = [[_thread currentStack] retain];
#endif
    [_thread dispatchRecursiveSync:^(iTermSynchronizedState *state) {
#if CHECK_DOUBLE_INVOKES
        ITAssertWithMessage(!self->_invokeStack, @"Previously invoked from:\n%@\n\nNow invoked from:\n%@\n\nCreated from:\n%@\n%@",
                     _invokeStack, stack, _creationStack, _debugInfo);

        _invokeStack = [stack copy];
        [stack release];
#endif
        block(state, object);
        [block release];
        dispatch_group_leave(_group);
        [self release];
    }];
}

// Returns YES if invoked, NO if timed out
- (BOOL)waitUntilInvoked {
    // BUG-11879: Add 30 second timeout to prevent indefinite hang
    dispatch_time_t timeout = dispatch_time(DISPATCH_TIME_NOW, (int64_t)(30 * NSEC_PER_SEC));
    return dispatch_group_wait(_group, timeout) == 0;
}

- (void)addDebugInfo:(NSString *)debugInfo {
#if CHECK_DOUBLE_INVOKES
    [_debugInfo appendString:debugInfo];
    [_debugInfo appendString:@"\n"];
#endif
}

@end

@implementation iTermThreadChecker {
    __weak iTermThread *_thread;
}

- (instancetype)initWithThread:(iTermThread *)thread {
    self = [super init];
    if (self) {
        _thread = thread;
    }
    return self;
}

- (void)check {
    [_thread check];
}

- (instancetype)retain {
    id result = [super retain];
    [self check];
    return result;
}

- (oneway void)release {
    [self check];
    [super release];
}

- (instancetype)autorelease {
    [self check];
    return [super autorelease];
}

@end
