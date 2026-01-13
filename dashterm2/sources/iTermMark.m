//
//  iTermMark.m
//  DashTerm2
//
//  Created by George Nachman on 10/18/15.
//
//

#import "iTermMark.h"
#import "CapturedOutput.h"
#import "DebugLogging.h"
#import "NSDictionary+iTerm.h"
#import <os/lock.h>

// Global lock for thread-safe doppelganger/progenitor access across all iTermMark instances
static os_unfair_lock sMarkLock = OS_UNFAIR_LOCK_INIT;

@implementation iTermMark {
    iTermMark *_doppelganger;
    __weak iTermMark *_progenitor;
    BOOL _isDoppelganger;
}

@synthesize entry;
@synthesize cachedLocation;

#pragma mark - IntervalTreeObject

- (instancetype)initWithDictionary:(NSDictionary *)dict {
    return [super init];
}

- (NSDictionary *)dictionaryValue {
    return @{};
}

- (NSDictionary *)dictionaryValueWithTypeInformation {
    return @{ @"class": NSStringFromClass(self.class),
              @"value": [self dictionaryValue] };
}

+ (id<IntervalTreeObject>)intervalTreeObjectWithDictionaryWithTypeInformation:(NSDictionary *)dict {
    NSString *className = dict[@"class"];
    if (!className) {
        return nil;
    }
    NSDictionary *value = dict[@"value"];
    if (!value) {
        return nil;
    }
    Class c = NSClassFromString(className);
    if (!c) {
        return nil;
    }
    if (![c conformsToProtocol:@protocol(IntervalTreeObject)] ||
        ![c instancesRespondToSelector:@selector(initWithDictionary:)]) {
        return nil;
    }
    return [[c alloc] initWithDictionary:value];
}

- (instancetype)copyOfIntervalTreeObject {
    return [[self.class alloc] init];
}

- (BOOL)isDoppelganger {
    os_unfair_lock_lock(&sMarkLock);
    BOOL result = _isDoppelganger;
    os_unfair_lock_unlock(&sMarkLock);
    return result;
}

// BUG-1202: Version that assumes lock is already held. Use from copyWithZone: which is called
// from within doppelganger method that holds the lock. os_unfair_lock is not recursive,
// so we cannot re-acquire it.
- (BOOL)isDoppelgangerLocked {
    return _isDoppelganger;
}

- (id<iTermMark>)doppelganger {
    os_unfair_lock_lock(&sMarkLock);
    // BUG-f1374: Replace assert with guard - doppelganger of doppelganger should return nil, not crash
    if (_isDoppelganger) {
        DLog(@"WARNING: Attempted to get doppelganger of a doppelganger (iTermMark)");
        os_unfair_lock_unlock(&sMarkLock);
        return nil;
    }
    if (!_doppelganger) {
        _doppelganger = [self copy];
        [_doppelganger becomeDoppelgangerWithProgenitor:self];
    }
    id<iTermMark> result = _doppelganger;
    os_unfair_lock_unlock(&sMarkLock);
    return result;
}

- (void)becomeDoppelgangerWithProgenitor:(iTermMark *)progenitor {
    _isDoppelganger = YES;
    _progenitor = progenitor;
}

- (NSString *)shortDebugDescription {
    return [NSString stringWithFormat:@"[Mark %@]", NSStringFromClass(self.class)];
}

- (id<iTermMark>)progenitor {
    os_unfair_lock_lock(&sMarkLock);
    id<iTermMark> result = _progenitor;
    os_unfair_lock_unlock(&sMarkLock);
    return result;
}

#pragma mark - NSObject

- (NSString *)description {
    return [NSString stringWithFormat:@"<%@: %p interval=%@ %@>",
            NSStringFromClass(self.class),
            self,
            self.entry.interval,
            _isDoppelganger ? @"IsDop" : @"NotDop"];
}

#pragma mark - APIs

- (BOOL)isVisible {
    return YES;
}

#pragma mark - NSCopying

- (id)copyWithZone:(NSZone *)zone {
    return [[self.class alloc] initWithDictionary:self.dictionaryValue];
}

- (iTermMark *)copy {
    return [self copyWithZone:nil];
}

@end
