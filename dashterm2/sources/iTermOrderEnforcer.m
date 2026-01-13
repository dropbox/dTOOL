//
//  iTermOrderEnforcer.m
//  DashTerm2
//
//  Created by George Nachman on 1/14/20.
//

#import "iTermOrderEnforcer.h"

#import "DebugLogging.h"
#include <os/lock.h>

@interface iTermOrderEnforcer()
- (BOOL)commit:(NSInteger)generation;
- (BOOL)peek:(NSInteger)generation;
@end

@interface iTermOrderedToken: NSObject<iTermOrderedToken>
@property (nonatomic, readonly, weak) iTermOrderEnforcer *enforcer;
@end

@implementation iTermOrderedToken {
    NSInteger _generation;
    BOOL _committed;
    os_unfair_lock _lock;
}

- (instancetype)initWithGeneration:(NSInteger)generation
                          enforcer:(iTermOrderEnforcer *)enforcer {
    self = [super init];
    if (self) {
        _generation = generation;
        _enforcer = enforcer;
        _lock = OS_UNFAIR_LOCK_INIT;
    }
    return self;
}

- (NSString *)description {
    return [@(_generation) stringValue];
}

#pragma mark - iTermOrderedToken

// BUG-f1074: Replace assert() with guard to prevent crash on double commit
- (BOOL)commit {
    os_unfair_lock_lock(&_lock);
    if (_committed) {
        os_unfair_lock_unlock(&_lock);
        DLog(@"BUG-f1074: iTermOrderedToken double commit detected for generation %@", @(_generation));
        return NO;  // Already committed, reject silently
    }
    _committed = YES;
    NSInteger generation = _generation;
    iTermOrderEnforcer *enforcer = _enforcer;
    os_unfair_lock_unlock(&_lock);
    return [enforcer commit:generation];
}

- (BOOL)peek {
    os_unfair_lock_lock(&_lock);
    NSInteger generation = _generation;
    iTermOrderEnforcer *enforcer = _enforcer;
    os_unfair_lock_unlock(&_lock);
    return [enforcer peek:generation];
}

@end

@implementation iTermOrderEnforcer {
    NSInteger _generation;
    NSInteger _lastCommitted;
    os_unfair_lock _lock;
}

- (instancetype)init {
    self = [super init];
    if (self) {
        _lastCommitted = -1;
        _lock = OS_UNFAIR_LOCK_INIT;
    }
    return self;
}

- (id<iTermOrderedToken>)newToken {
    os_unfair_lock_lock(&_lock);
    NSInteger generation = _generation++;
    os_unfair_lock_unlock(&_lock);
    return [[iTermOrderedToken alloc] initWithGeneration:generation
                                                enforcer:self];
}

- (BOOL)commit:(NSInteger)generation {
    os_unfair_lock_lock(&_lock);
    const BOOL accepted = (generation > _lastCommitted);
    if (accepted) {
        _lastCommitted = generation;
    }
    os_unfair_lock_unlock(&_lock);
    if (!accepted) {
        DLog(@"Reject out of order token with generation %@", @(generation));
    }
    return accepted;
}

- (BOOL)peek:(NSInteger)generation {
    os_unfair_lock_lock(&_lock);
    BOOL result = (generation > _lastCommitted);
    os_unfair_lock_unlock(&_lock);
    return result;
}
@end
