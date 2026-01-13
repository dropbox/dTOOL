//
//  VT100WorkingDirectory.m
//  iTerm
//
//  Created by George Nachman on 12/20/13.
//
//

#import "VT100WorkingDirectory.h"
#import "DebugLogging.h"
#import "NSObject+iTerm.h"

#import <os/lock.h>

static NSString *const kWorkingDirectoryStateWorkingDirectoryKey = @"Working Directory";

// Class-level lock for doppelganger creation (replaces @synchronized([VT100WorkingDirectory class]))
static os_unfair_lock gVT100WorkingDirectoryLock = OS_UNFAIR_LOCK_INIT;

@implementation VT100WorkingDirectory {
    VT100WorkingDirectory *_doppelganger;
    __weak VT100WorkingDirectory *_progenitor;
    BOOL _isDoppelganger;
}

@synthesize entry;

- (instancetype)initWithDictionary:(NSDictionary *)dict {
    return [self initWithDirectory:dict[kWorkingDirectoryStateWorkingDirectoryKey]];
}

- (instancetype)initWithDirectory:(NSString *)directory {
    self = [super init];
    if (self) {
        _workingDirectory = [directory copy];
    }
    return self;
}

- (NSString *)description {
    return
        [NSString stringWithFormat:@"<%@: %p workingDirectory=%@ interval=%@ %@>", self.class, self,
                                   self.workingDirectory, self.entry.interval, _isDoppelganger ? @"IsDop" : @"NotDop"];
}

#pragma mark - IntervalTreeObject

- (NSDictionary *)dictionaryValue {
    if (self.workingDirectory) {
        return @{kWorkingDirectoryStateWorkingDirectoryKey : self.workingDirectory};
    } else {
        return @{};
    }
}

- (nonnull NSDictionary *)dictionaryValueWithTypeInformation {
    return @{@"class" : NSStringFromClass(self.class), @"value" : [self dictionaryValue]};
}

- (NSString *)shortDebugDescription {
    return [NSString stringWithFormat:@"[Dir %@]", self.workingDirectory];
}

- (nonnull id<IntervalTreeObject>)doppelganger {
    os_unfair_lock_lock(&gVT100WorkingDirectoryLock);
    // BUG-f1378: Replace assert with guard - doppelganger of doppelganger should return nil, not crash
    if (_isDoppelganger) {
        os_unfair_lock_unlock(&gVT100WorkingDirectoryLock);
        DLog(@"WARNING: Attempted to get doppelganger of a doppelganger (VT100WorkingDirectory)");
        return nil;
    }
    if (!_doppelganger) {
        _doppelganger = [self copyOfIntervalTreeObject];
        _doppelganger->_progenitor = self;
        _doppelganger->_isDoppelganger = YES;
    }
    id<IntervalTreeObject> result = _doppelganger;
    os_unfair_lock_unlock(&gVT100WorkingDirectoryLock);
    return result;
}

- (id<IntervalTreeObject>)progenitor {
    os_unfair_lock_lock(&gVT100WorkingDirectoryLock);
    id<IntervalTreeObject> result = _progenitor;
    os_unfair_lock_unlock(&gVT100WorkingDirectoryLock);
    return result;
}

- (instancetype)copyOfIntervalTreeObject {
    return [[VT100WorkingDirectory alloc] initWithDirectory:self.workingDirectory];
}

@end
