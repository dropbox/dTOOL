//
//  PTYAnnotation.m
//  DashTerm2
//
//  Created by George Nachman on 12/26/21.
//

#import "PTYAnnotation.h"
#import "DebugLogging.h"
#import <os/lock.h>

// Performance optimization: os_unfair_lock has ~10x lower overhead than @synchronized
// Class-level lock for doppelganger operations
static os_unfair_lock gPTYAnnotationLock = OS_UNFAIR_LOCK_INIT;

static NSString *const PTYAnnotationDictionaryKeyText = @"Text";
static NSString *const PTYAnnotationDictionaryKeyUniqueID = @"UniqueID";

@implementation PTYAnnotation {
    BOOL _deferHide;
    PTYAnnotation *_doppelganger;
    BOOL _isDoppelganger;
}

@synthesize progenitor = _progenitor;
@synthesize entry = _entry;
@synthesize delegate = _delegate;
@synthesize uniqueID = _uniqueID;

+ (NSString *)textForAnnotationForNamedMarkWithName:(NSString *)name {
    return [@"Named Mark " stringByAppendingString:name];
}

- (instancetype)init {
    self = [super init];
    if (self) {
        _stringValue = @"";
        _uniqueID = [[NSUUID UUID] UUIDString];
    }
    return self;
}

- (instancetype)initWithDictionary:(NSDictionary *)dict {
    self = [self init];
    if (self) {
        _stringValue = [dict[PTYAnnotationDictionaryKeyText] copy] ?: @"";
        _uniqueID = [dict[PTYAnnotationDictionaryKeyUniqueID] copy] ?: @"";
    }
    return self;
}

- (NSString *)shortDebugDescription {
    return [NSString stringWithFormat:@"[Annotation %@]", _stringValue];
}

- (NSString *)description {
    return [NSString stringWithFormat:@"<%@: %p string=%@ %@>", NSStringFromClass([self class]), self, _stringValue,
                                      _isDoppelganger ? @"IsDop" : @"NotDop"];
}

- (NSDictionary *)dictionaryValue {
    return @{PTYAnnotationDictionaryKeyText : _stringValue.copy, PTYAnnotationDictionaryKeyUniqueID : _uniqueID};
}

- (instancetype)copyOfIntervalTreeObject {
    PTYAnnotation *copy = [[PTYAnnotation alloc] initWithDictionary:self.dictionaryValue];
    copy.stringValue = _stringValue;
    return copy;
}

- (NSDictionary *)dictionaryValueWithTypeInformation {
    return @{@"class" : NSStringFromClass(self.class), @"value" : [self dictionaryValue]};
}

- (void)hide {
    if (!self.delegate) {
        _deferHide = YES;
        return;
    }
    [self.delegate annotationDidRequestHide:self];
}

- (void)setDelegate:(id<PTYAnnotationDelegate>)delegate {
    if (delegate == _delegate) {
        return;
    }
    _delegate = delegate;
    if (delegate && _deferHide) {
        _deferHide = NO;
        [delegate annotationDidRequestHide:self];
    }
}

- (void)setStringValue:(NSString *)stringValue {
    _stringValue = [stringValue copy];
    [_delegate annotationStringDidChange:self];
}

- (void)setStringValueWithoutSideEffects:(NSString *)value {
    _stringValue = [value copy];
}

- (void)willRemove {
    [_delegate annotationWillBeRemoved:self];
}

- (id<IntervalTreeObject>)doppelganger {
    os_unfair_lock_lock(&gPTYAnnotationLock);
    // BUG-f1375: Replace assert with guard - doppelganger of doppelganger should return nil, not crash
    if (_isDoppelganger) {
        os_unfair_lock_unlock(&gPTYAnnotationLock);
        DLog(@"WARNING: Attempted to get doppelganger of a doppelganger (PTYAnnotation)");
        return nil;
    }
    if (!_doppelganger) {
        _doppelganger = [self copyOfIntervalTreeObject];
        _doppelganger->_progenitor = self;
        _doppelganger->_isDoppelganger = YES;
    }
    PTYAnnotation *result = _doppelganger;
    os_unfair_lock_unlock(&gPTYAnnotationLock);
    return result;
}

- (id<PTYAnnotationReading>)progenitor {
    os_unfair_lock_lock(&gPTYAnnotationLock);
    id<PTYAnnotationReading> result = _progenitor;
    os_unfair_lock_unlock(&gPTYAnnotationLock);
    return result;
}

@end
