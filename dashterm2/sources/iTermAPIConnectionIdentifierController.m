//
//  iTermAPIConnectionIdentifierController.m
//  DashTerm2SharedARC
//
//  Created by George Nachman on 4/20/18.
//

#import "iTermAPIConnectionIdentifierController.h"

#import <os/lock.h>

@implementation iTermAPIConnectionIdentifierController {
    NSMutableDictionary<NSString *, NSString *> *_map;
    NSInteger _nextIdentifier;
    // Performance optimization: os_unfair_lock has ~10x lower overhead than @synchronized
    os_unfair_lock _lock;
}

+ (instancetype)sharedInstance {
    static id instance;
    static dispatch_once_t onceToken;
    dispatch_once(&onceToken, ^{
        instance = [[self alloc] init];
    });
    return instance;
}

- (instancetype)init {
    self = [super init];
    if (self) {
        _map = [[NSMutableDictionary alloc] initWithCapacity:16];
        _lock = OS_UNFAIR_LOCK_INIT;
    }
    return self;
}

- (id)identifierForKey:(NSString *)key {
    // BUG-1288: Synchronize to prevent race condition when called from multiple threads
    os_unfair_lock_lock(&_lock);
    id identifier = _map[key];
    if (!identifier) {
        identifier = [@(_nextIdentifier) stringValue];
        _map[key] = identifier;
        _nextIdentifier++;
    }
    os_unfair_lock_unlock(&_lock);
    return identifier;
}

@end
