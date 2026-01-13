//
//  iTermImageMark.m
//  DashTerm2
//
//  Created by George Nachman on 10/18/15.
//
//

#import "iTermImageMark.h"

#import "DebugLogging.h"
#import "ScreenChar.h"
#import <os/lock.h>

// Performance optimization: os_unfair_lock has ~10x lower overhead than @synchronized
// Class-level lock for doppelganger operations
static os_unfair_lock gImageMarkLock = OS_UNFAIR_LOCK_INIT;

@implementation iTermImageMark {
    iTermImageMark *_doppelganger;
    __weak iTermImageMark *_progenitor;
    BOOL _isDoppelganger;
}

- (instancetype)initWithImageCode:(NSNumber *)imageCode {
    self = [super init];
    if (self) {
        _imageCode = imageCode;
    }
    DLog(@"New mage mark %@ created", self);
    return self;
}

- (void)setImageCode:(NSNumber *)imageCode {
    _imageCode = imageCode;
    DLog(@"Update image code %@", self);
}

- (NSString *)description {
    return [NSString stringWithFormat:@"<%@: %p imageCode=%@ %@>", NSStringFromClass(self.class), self, self.imageCode,
                                      _isDoppelganger ? @"IsDop" : @"NotDop"];
}

- (instancetype)initWithDictionary:(NSDictionary *)dict {
    NSNumber *imageCode = dict[@"imageCode"];
    if (!imageCode) {
        return nil;
    }
    return [self initWithImageCode:imageCode];
}

- (NSDictionary *)dictionaryValue {
    if (_imageCode) {
        return @{@"imageCode" : _imageCode};
    } else {
        return @{};
    }
}

- (void)dealloc {
    DLog(@"Deallocing %@", self);
    // BUG-5067: Only the progenitor (original) should release the image.
    // Doppelgangers share the imageCode but don't own it.
    if (_imageCode && !_isDoppelganger) {
        ReleaseImage(_imageCode.integerValue);
    }
}

- (id<IntervalTreeObject>)doppelganger {
    os_unfair_lock_lock(&gImageMarkLock);
    // BUG-f1372: Replace assert with guard - doppelganger of doppelganger should return nil, not crash
    if (_isDoppelganger) {
        os_unfair_lock_unlock(&gImageMarkLock);
        DLog(@"WARNING: Attempted to get doppelganger of a doppelganger (iTermImageMark)");
        return nil;
    }
    if (!_doppelganger) {
        _doppelganger = [[iTermImageMark alloc] init];
        _doppelganger->_imageCode = _imageCode;
        _doppelganger->_isDoppelganger = YES;
        _doppelganger->_progenitor = self;
    }
    // BUG-f1373: Replace assert with nil check - alloc failure should return nil, not crash
    iTermImageMark *result = _doppelganger;
    os_unfair_lock_unlock(&gImageMarkLock);
    return result;
}

- (NSString *)shortDebugDescription {
    return [NSString stringWithFormat:@"[Image %@]", _imageCode];
}

- (id<iTermMark>)progenitor {
    os_unfair_lock_lock(&gImageMarkLock);
    id<iTermMark> result = _progenitor;
    os_unfair_lock_unlock(&gImageMarkLock);
    return result;
}

@end
