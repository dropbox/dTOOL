//
//  iTermExpect.m
//  DashTerm2
//
//  Created by George Nachman on 12/21/19.
//

#import "iTermExpect.h"

#import "DebugLogging.h"
#import "NSArray+iTerm.h"
#import "NSData+iTerm.h"

// The concurrency model is based on multi-version concurrency. The API is designed so that merge conflicts cannot happen.
// Clients on the main thread can add or cancel expectations.
// Periodically, the main-thread instance of iTermExpect will be copied into the mutation thread.
// This snapshot persists for a short time.
// The mutation thread performs matching and may call didMatchWithCaptureGroups:.
// Calls to didMatchWithCaptureGroups: on the mutation thread schedule matching main-thread calls.
// These main thread calls update the main thread's copy of iTermExpect (e.g., adding and removing
// expectations) and also run user callbacks.
// During a call to didMatchWithCaptureGroups: the mutation thread updates its internal state in the
// same way.
@interface iTermExpectation()
@property (nonatomic, readonly) void (^userWillExpectCallback)(void);
@property (nonatomic, readonly) void (^willExpect)(iTermExpectation *expectation);
@property (nullable, nonatomic, strong, readwrite) iTermExpectation *successor;
@property (nullable, nonatomic, weak, readwrite) iTermExpectation *predecessor;
@property (nonatomic, nullable, weak) iTermExpectation *original;
// Did a copy already match this and dispatch a call to didMatchWithCaptureGroups: on self?
@property (atomic) BOOL matchPending;
@property (atomic) BOOL userWillExpectCalled;
@end

@implementation iTermExpectation {
    void (^_completion)(iTermExpectation *, NSArray<NSString *> * _Nonnull);
}

- (instancetype)initWithOriginal:(iTermExpectation *)original
                      willExpect:(void (^)(iTermExpectation *))willExpect
                  userWillExpect:(void (^)(void))userWillExpect
                      completion:(void (^)(iTermExpectation *, NSArray<NSString *> * _Nonnull))completion {
    self = [self initWithRegularExpression:original.regex
                                  deadline:original.deadline
                                willExpect:willExpect
                            userWillExpect:userWillExpect
                                completion:completion];
    if (self) {
        _original = original;
        // BUG-f941: Guard against nil regex instead of crashing
        if (!_regex) {
            ELog(@"iTermExpect: copy init resulted in nil regex");
            return nil;
        }
    }
    return self;
}

- (instancetype)initWithRegularExpression:(NSString *)regex
                                 deadline:(NSDate *)deadline
                               willExpect:(void (^)(iTermExpectation *))willExpect
                           userWillExpect:(void (^)(void))userWillExpect
                               completion:(void (^)(iTermExpectation *, NSArray<NSString *> * _Nonnull))completion {
    // BUG-f942: Guard against nil regex early instead of crashing
    if (!regex) {
        ELog(@"iTermExpect: initWithRegularExpression called with nil regex");
        return nil;
    }
    self = [super init];
    if (self) {
        _deadline = deadline;
        _completion = [completion copy];
        _willExpect = [willExpect copy];
        _userWillExpectCallback = [userWillExpect copy];
        _regex = [regex copy];
    }
    return self;
}

- (NSString *)description {
    return [NSString stringWithFormat:@"<%@: %p regex=%@ hasCompleted=%@ successor=%p>",
            NSStringFromClass(self.class), self, _regex, @(self.hasCompleted), _successor];
}

- (iTermExpectation *)lastExpectation {
    return _successor ?: self;
    return self;
}

- (void)didMatchWithCaptureGroups:(NSArray<NSString *> *)captureGroups
                       dispatcher:(void (^)(void (^)(void)))dispatcher {
    if (self.original) {
        iTermExpectation *original = self.original;
        original.matchPending = YES;
        dispatcher(^{
            original.matchPending = NO;
            if (original.hasCompleted) {
                return;
            }
            [original didMatchWithCaptureGroups:captureGroups
                                     dispatcher:^(void (^closure)(void)) {
                closure();
            }];
        });
    }
    _hasCompleted = YES;
    if (_completion) {
        _completion(self, captureGroups);
    }
    _completion = nil;
    if (_successor) {
        [_successor expect];
    }
}

- (void)addSuccessor:(iTermExpectation *)successor {
    // BUG-f943: Guard against being called on background thread instead of crashing
    if (![NSThread isMainThread]) {
        ELog(@"iTermExpect: addSuccessor called on non-main thread");
        dispatch_async(dispatch_get_main_queue(), ^{
            [self addSuccessor:successor];
        });
        return;
    }
    iTermExpectation *current = self;
    while (current.successor) {
        current = current.successor;
    }
    current.successor = successor;
    successor.predecessor = self;
}

- (void)expect {
    // BUG-f944: Guard against expect on completed expectation instead of crashing
    if (_hasCompleted) {
        ELog(@"iTermExpect: expect called on already completed expectation");
        return;
    }
    _willExpect(self);
}

- (void)cancel {
    [_successor cancel];
    _predecessor.successor = nil;
    _hasCompleted = YES;
    _completion = nil;
    _willExpect = nil;
    // BUG-f945: Log warning if trying to cancel a copy instead of crashing
    if (self.original) {
        ELog(@"iTermExpect: cancel called on copy expectation - this is not supported");
    }
}

- (void)invokeUserWillExpectCallbackIfNeeded {
    if (self.userWillExpectCalled) {
        return;
    }
    self.userWillExpectCalled = YES;
    if (!self.userWillExpectCallback) {
        return;
    }
    self.userWillExpectCallback();
}

@end

@implementation iTermExpect {
    NSMutableArray<iTermExpectation *> *_expectations;
}

- (instancetype)initDry:(BOOL)dry {
    self = [super init];
    if (self) {
        _dry = dry;
        _expectations = [NSMutableArray arrayWithCapacity:8];  // Typical expectation chain
    }
    return self;
}

- (NSString *)description {
    return [NSString stringWithFormat:@"<%@: %p dry=%@ expectations:\n%@>", NSStringFromClass([self class]), self, @(_dry), _expectations];
}

- (iTermExpectation *)expectRegularExpression:(NSString *)regex
                                        after:(nullable iTermExpectation *)predecessor
                                     deadline:(nullable NSDate *)deadline
                                   willExpect:(void (^ _Nullable)(void))willExpect
                                   completion:(void (^ _Nullable)(NSArray<NSString *> * _Nonnull))completion {
    // BUG-f946: Guard against nil regex instead of crashing
    if (!regex) {
        ELog(@"iTermExpect: expectRegularExpression called with nil regex");
        return nil;
    }
    _dirty = YES;
    DLog(@"Add expectation, set dirty dry=%d", _dry);
    // BUG-f947: Guard against being called on background thread
    if (![NSThread isMainThread]) {
        ELog(@"iTermExpect: expectRegularExpression called on non-main thread");
        return nil;
    }
    __weak __typeof(self) weakSelf = self;
    void (^internalWillExpect)(iTermExpectation *) = ^(iTermExpectation *expectation){
        [weakSelf addExpectation:expectation];
    };
    void (^internalCompletion)(iTermExpectation *, NSArray<NSString *> *) = ^(iTermExpectation *expectation,
                                                                              NSArray<NSString *> *captureGroups) {
        [weakSelf removeExpectation:expectation];
        if (completion) {
            completion(captureGroups);
        }
    };
    iTermExpectation *expectation = [[iTermExpectation alloc] initWithRegularExpression:regex
                                                                               deadline:deadline
                                                                             willExpect:internalWillExpect
                                                                         userWillExpect:willExpect
                                                                             completion:internalCompletion];
    if (predecessor && !predecessor.hasCompleted) {
        [predecessor addSuccessor:expectation];
        return expectation;
    }
    [expectation expect];
    return expectation;
}

- (void)addExpectation:(iTermExpectation *)expectation {
    DLog(@"addExpectation:%@ dry=%d", expectation, _dry);
    if (!expectation) {
        DLog(@"  - fail, is nil. dry=%d", _dry);
        return;
    }
    [_expectations addObject:expectation];
    if (_dry) {
        return;
    }
    DLog(@"Wet add expectation");
    dispatch_async(dispatch_get_main_queue(), ^{
        [expectation.original invokeUserWillExpectCallbackIfNeeded];
    });
}

- (void)removeExpectation:(iTermExpectation *)expectation {
    if (expectation) {
        DLog(@"Remove expectation %@ from dry=%d", expectation, _dry);
    }
    [_expectations removeObject:expectation];
}

- (void)cancelExpectation:(iTermExpectation *)expectation {
    _dirty = YES;
    if (expectation != nil) {
        DLog(@"cancel %@ dry=%d", expectation, _dry);
    }
    [expectation cancel];
    [_expectations removeObject:expectation];
}

- (void)removeExpiredExpectations {
    if (_expectations.count == 0) {
        return;
    }
    const BOOL dry = _dry;
    [_expectations removeObjectsPassingTest:^BOOL(iTermExpectation *expectation) {
        const BOOL didExpire = expectation.userWillExpectCalled && expectation.deadline != nil && expectation.deadline.timeIntervalSinceNow < 0;
        if (didExpire) {
            DLog(@"Remove expired expectation %@ dry=%d", expectation, dry);
        }
        return didExpire;
    }];
}

- (BOOL)expectationsIsEmpty {
    if (_expectations.count == 0) {
        return YES;
    }
    [self removeExpiredExpectations];
    return _expectations.count == 0;
}

- (NSArray<iTermExpectation *> *)expectations {
    [self removeExpiredExpectations];
    return [_expectations copy];
}

- (BOOL)maybeHasExpectations {
    return _expectations.count > 0;
}

- (void)resetDirty {
    if (_dirty) {
        DLog(@"resetDirty dry=%d", _dry);
        _dirty = NO;
    }
}

#pragma mark - NSCopying

- (iTermExpectation *)copyOfExpectation:(iTermExpectation *)original copiedExpect:(iTermExpect *)copiedExpect {
    DLog(@"Copy over expectation %@", [((NSData *)original.userData) stringWithEncoding:NSUTF8StringEncoding]);
    if (original.matchPending) {
        if (!original.successor) {
            return nil;
        }
        return [self copyOfExpectation:original.successor copiedExpect:copiedExpect];
    }

    __weak __typeof(self) weakSelf = copiedExpect;
    void (^willExpect)(iTermExpectation *) = ^(iTermExpectation *obj) {
        [weakSelf addExpectation:obj];
    };
    void (^completion)(iTermExpectation *, NSArray<NSString *> *) = ^(iTermExpectation *obj,
                                                                      NSArray<NSString *> * _Nonnull captures) {
        [weakSelf removeExpectation:obj];
    };
    // BUG-f948: Guard against nil original instead of crashing
    if (!original) {
        ELog(@"iTermExpect: copyOfExpectation called with nil original");
        return nil;
    }
    // BUG-f949: Guard against nil regex instead of crashing
    if (!original.regex) {
        ELog(@"iTermExpect: copyOfExpectation called with original having nil regex");
        return nil;
    }
    iTermExpectation *theCopy = [[iTermExpectation alloc] initWithOriginal:original
                                                                willExpect:willExpect
                                                            userWillExpect:original.userWillExpectCallback
                                                                completion:completion];
    theCopy.userData = original.userData;
    if (original.successor) {
        theCopy.successor = [self copyOfExpectation:original.successor copiedExpect:copiedExpect];
    }
    return theCopy;
}

- (id)copyWithZone:(NSZone *)zone {
    iTermExpect *theCopy = [[iTermExpect alloc] initDry:NO];
    DLog(@"Making a copy of dry=%d with expectations %@", _dry, self.expectations);
    [self.expectations enumerateObjectsUsingBlock:^(iTermExpectation * _Nonnull original, NSUInteger idx, BOOL * _Nonnull stop) {
        [theCopy addExpectation:[self copyOfExpectation:original
                                           copiedExpect:theCopy]];
    }];
    return theCopy;
}

@end
