//
//  iTermResult.m
//  DashTerm2
//
//  Created by George Nachman on 3/14/20.
//

#import "iTermResult.h"

#import "DebugLogging.h"

@implementation iTermResult {
    id _object;
    NSError *_error;
}

+ (instancetype)withError:(NSError *)error {
    // BUG-f1024: Guard against nil error instead of crashing - create a generic error
    if (!error) {
        DLog(@"BUG-f1024: withError: called with nil error - creating generic error");
        error =
            [NSError errorWithDomain:@"com.dashterm2.iTermResult"
                                code:-1
                            userInfo:@{NSLocalizedDescriptionKey : @"Unknown error (nil error passed to withError:)"}];
    }
    return [[self alloc] initWithObject:nil error:error];
}

+ (instancetype)withObject:(id)object {
    // BUG-f1025: Guard against nil object instead of crashing - return nil
    if (!object) {
        DLog(@"BUG-f1025: withObject: called with nil object - returning nil");
        return nil;
    }
    return [[self alloc] initWithObject:object error:nil];
}

- (instancetype)initWithObject:(id)object error:(NSError *)error {
    self = [super init];
    if (self) {
        _object = object;
        _error = error;
    }
    return self;
}

- (NSString *)description {
    return [NSString stringWithFormat:@"<%@: %p %@=%@>", NSStringFromClass(self.class), self,
                                      _object ? @"object" : @"error", _object ?: _error];
}

- (void)handleObject:(void (^)(id _Nonnull))object error:(void (^)(NSError *_Nonnull))error {
    if (_object) {
        object(_object);
        return;
    }
    error(_error);
}

@end
