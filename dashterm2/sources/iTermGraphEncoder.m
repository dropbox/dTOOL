//
//  iTermGraphEncoder.m
//  DashTerm2SharedARC
//
//  Created by George Nachman on 7/26/20.
//

#import "iTermGraphEncoder.h"

#import "DebugLogging.h"
#import "NSArray+iTerm.h"
#import "NSData+iTerm.h"
#import "NSDictionary+iTerm.h"
#import "NSObject+iTerm.h"
#import "iTermTuple.h"

NSInteger iTermGenerationAlwaysEncode = NSIntegerMax;

@implementation iTermGraphEncoder {
    NSMutableDictionary<NSString *, id> *_pod;
    NSString *_identifier;
    NSInteger _generation;
    NSString *_key;
    // This is append-only, otherwise rolling back a transaction breaks.
    NSMutableArray<iTermEncoderGraphRecord *> *_children;
    iTermEncoderGraphRecord *_record;
}

- (instancetype)initWithKey:(NSString *)key identifier:(NSString *)identifier generation:(NSInteger)generation {
    // BUG-f1028: Replace assert(identifier) with guard - nil identifier should return nil, not crash
    if (!identifier) {
        ELog(@"iTermGraphEncoder initialized with nil identifier for key: %@", key);
        return nil;
    }
    self = [super init];
    if (self) {
        _key = key;
        _identifier = identifier;
        if (generation != iTermGenerationAlwaysEncode) {
            _generation = generation;
        } else {
            _generation = 0;
        }
        // Phase 1 optimization: Pre-size for typical graph encoder data
        _pod = [[NSMutableDictionary alloc] initWithCapacity:16];
        _children = [[NSMutableArray alloc] initWithCapacity:8];
        _state = iTermGraphEncoderStateLive;
    }
    return self;
}

- (instancetype)initWithRecord:(iTermEncoderGraphRecord *)record {
    iTermGraphEncoder *encoder = [self initWithKey:record.key
                                        identifier:record.identifier
                                        generation:record.generation];
    if (!encoder) {
        return nil;
    }
    encoder->_pod = [record.pod mutableCopy];
    encoder->_children = [record.graphRecords mutableCopy];
    return encoder;
}

- (void)encodeString:(NSString *)string forKey:(NSString *)key {
    // BUG-f1029: Replace assert with guard - encoding on non-live state should be ignored, not crash
    if (_state != iTermGraphEncoderStateLive) {
        ELog(@"encodeString called on non-live encoder (state=%d) for key: %@", (int)_state, key);
        return;
    }
    _pod[key] = string.copy;
}

- (void)encodeNumber:(NSNumber *)number forKey:(NSString *)key {
    // BUG-f1030: Replace assert with guard - encoding on non-live state should be ignored, not crash
    if (_state != iTermGraphEncoderStateLive) {
        ELog(@"encodeNumber called on non-live encoder (state=%d) for key: %@", (int)_state, key);
        return;
    }
    _pod[key] = number;
}

- (void)encodeData:(NSData *)data forKey:(NSString *)key {
    // BUG-f1031: Replace assert with guard - encoding on non-live state should be ignored, not crash
    if (_state != iTermGraphEncoderStateLive) {
        ELog(@"encodeData called on non-live encoder (state=%d) for key: %@", (int)_state, key);
        return;
    }
    _pod[key] = data.copy;
}

- (BOOL)encodePropertyList:(id)plist withKey:(NSString *)key {
    // BUG-f1032: Replace assert with guard - encoding on non-live state should return NO, not crash
    if (_state != iTermGraphEncoderStateLive) {
        ELog(@"encodePropertyList called on non-live encoder (state=%d) for key: %@", (int)_state, key);
        return NO;
    }
    NSError *error;
    NSData *data = [NSData it_dataWithSecurelyArchivedObject:plist error:&error];
    if (error) {
        DLog(@"Failed to serialize property list %@: %@", plist, error);
        return NO;
    }
    _pod[key] = data;
    return YES;
}

- (void)encodeDate:(NSDate *)date forKey:(NSString *)key {
    // BUG-f1033: Replace assert with guard - encoding on non-live state should be ignored, not crash
    if (_state != iTermGraphEncoderStateLive) {
        ELog(@"encodeDate called on non-live encoder (state=%d) for key: %@", (int)_state, key);
        return;
    }
    _pod[key] = date;
}

- (void)encodeNullForKey:(NSString *)key {
    // BUG-f1034: Replace assert with guard - encoding on non-live state should be ignored, not crash
    if (_state != iTermGraphEncoderStateLive) {
        ELog(@"encodeNullForKey called on non-live encoder (state=%d) for key: %@", (int)_state, key);
        return;
    }
    _pod[key] = [NSNull null];
}

- (BOOL)encodeObject:(id)obj key:(NSString *)key {
    if ([obj conformsToProtocol:@protocol(iTermGraphEncodable)] &&
        [(id<iTermGraphEncodable>)obj graphEncoderShouldIgnore]) {
        return NO;
    }
    if ([obj isKindOfClass:[NSString class]]) {
        [self encodeString:obj forKey:key];
        return YES;
    }
    if ([obj isKindOfClass:[NSData class]]) {
        [self encodeData:obj forKey:key];
        return YES;
    }
    if ([obj isKindOfClass:[NSDate class]]) {
        [self encodeData:obj forKey:key];
        return YES;
    }
    if ([obj isKindOfClass:[NSNumber class]]) {
        [self encodeNumber:obj forKey:key];
        return YES;
    }
    if ([obj isKindOfClass:[NSNull class]]) {
        [self encodeNullForKey:key];
        return YES;
    }
    NSError *error = nil;
    [NSData it_dataWithSecurelyArchivedObject:obj error:&error];
    if (!error) {
        _pod[key] = obj;
        return YES;
    }
    if ([obj isKindOfClass:[NSArray class]]) {
        NSArray *array = obj;
        [self encodeArrayWithKey:key
                      generation:_generation
                     identifiers:[NSArray stringSequenceWithRange:NSMakeRange(0, array.count)]
                         options:0
                           block:^BOOL(NSString *_Nonnull identifier, NSInteger index,
                                       iTermGraphEncoder *_Nonnull subencoder, BOOL *stop) {
                               [subencoder encodeObject:array[index] key:@"__arrayValue"];
                               return YES;
                           }];
        return YES;
    }
    if ([obj isKindOfClass:[NSDictionary class]]) {
        NSDictionary *dict = obj;
        [self encodeDictionary:dict withKey:key generation:_generation];
        return YES;
    }
    // BUG-13008: Replace assert(NO) with ELog and return NO for unsupported object types
    // Unknown object types should be logged and skipped, not crash the encoder
    ELog(@"Cannot encode object of unsupported type %@ for key %@", NSStringFromClass([obj class]), key);
    return NO;
}

- (void)encodeDictionary:(NSDictionary *)dict withKey:(NSString *)key generation:(NSInteger)generation {
    [self encodeChildWithKey:@"__dict"
                  identifier:key
                  generation:generation
                       block:^BOOL(iTermGraphEncoder *_Nonnull subencoder) {
                           [dict enumerateKeysAndObjectsUsingBlock:^(NSString *_Nonnull key, id _Nonnull obj,
                                                                     BOOL *_Nonnull stop) {
                               [subencoder encodeObject:obj key:key];
                           }];
                           return YES;
                       }];
}

- (void)encodeGraph:(iTermEncoderGraphRecord *)record {
    // BUG-f1035: Replace assert with guard - encoding on non-live state should be ignored, not crash
    if (_state != iTermGraphEncoderStateLive) {
        ELog(@"encodeGraph called on non-live encoder (state=%d)", (int)_state);
        return;
    }
    // BUG-f1036: Guard against nil record to prevent adding nil to array
    if (!record) {
        ELog(@"encodeGraph called with nil record");
        return;
    }
    [_children addObject:record];
}

- (void)mergeDictionary:(NSDictionary *)dictionary {
    [_pod it_mergeFrom:dictionary];
}

- (BOOL)encodeChildWithKey:(NSString *)key
                identifier:(NSString *)identifier
                generation:(NSInteger)generation
                     block:(BOOL (^NS_NOESCAPE)(iTermGraphEncoder *subencoder))block {
    // BUG-f1037: Replace assert with guard - encoding on non-live state should return NO, not crash
    if (_state != iTermGraphEncoderStateLive) {
        ELog(@"encodeChildWithKey called on non-live encoder (state=%d) for key: %@", (int)_state, key);
        return NO;
    }
    iTermGraphEncoder *encoder = [[iTermGraphEncoder alloc] initWithKey:key
                                                             identifier:identifier
                                                             generation:generation];
    if (!block(encoder)) {
        return NO;
    }
    [self encodeGraph:encoder.record];
    return YES;
}

- (void)encodeChildrenWithKey:(NSString *)key
                  identifiers:(NSArray<NSString *> *)identifiers
                   generation:(NSInteger)generation
                        block:(BOOL (^)(NSString *identifier, NSUInteger idx, iTermGraphEncoder *subencoder,
                                        BOOL *stop))block {
    if (identifiers.count > 16 && _children.count == 0) {
        _children = [[NSMutableArray alloc] initWithCapacity:identifiers.count];
    }
    [identifiers enumerateObjectsUsingBlock:^(NSString *_Nonnull identifier, NSUInteger idx, BOOL *_Nonnull stop) {
        // transaction is slow because it makes a copy in case of rollback.
        // Do I need a transactio nfor each identifier?
        [self transaction:^BOOL {
            return [self encodeChildWithKey:key
                                 identifier:identifier
                                 generation:generation
                                      block:^BOOL(iTermGraphEncoder *_Nonnull subencoder) {
                                          return block(identifier, idx, subencoder, stop);
                                      }];
        }];
    }];
}

- (void)encodeArrayWithKey:(NSString *)key
                generation:(NSInteger)generation
               identifiers:(NSArray<NSString *> *)identifiers
                   options:(iTermGraphEncoderArrayOptions)options
                     block:(BOOL (^NS_NOESCAPE)(NSString *identifier, NSInteger index, iTermGraphEncoder *subencoder,
                                                BOOL *stop))block {
    if (identifiers.count != [NSSet setWithArray:identifiers].count) {
        ITBetaAssert(NO, @"Identifiers for %@ contains a duplicate: %@", key, identifiers);
    }
    [self encodeChildWithKey:@"__array"
                  identifier:key
                  generation:generation
                       block:^BOOL(iTermGraphEncoder *_Nonnull subencoder) {
                           // Phase 1 optimization: Pre-size for identifier count
                           NSMutableArray<NSString *> *savedIdentifiers =
                               [[NSMutableArray alloc] initWithCapacity:identifiers.count];
                           [subencoder encodeChildrenWithKey:@""
                                                 identifiers:identifiers
                                                  generation:iTermGenerationAlwaysEncode
                                                       block:^BOOL(NSString *_Nonnull identifier, NSUInteger idx,
                                                                   iTermGraphEncoder *_Nonnull subencoder,
                                                                   BOOL *_Nonnull stop) {
                                                           const BOOL result = block(identifier, idx, subencoder, stop);
                                                           if (result) {
                                                               [savedIdentifiers addObject:identifier];
                                                           }
                                                           return result;
                                                       }];
                           NSArray<NSString *> *orderedIdentifiers = savedIdentifiers;
                           if (options & iTermGraphEncoderArrayOptionsReverse) {
                               orderedIdentifiers = orderedIdentifiers.reversed;
                           }
                           orderedIdentifiers = [orderedIdentifiers arrayByRemovingDuplicatesStably];
                           [subencoder encodeString:[orderedIdentifiers componentsJoinedByString:@"\t"]
                                             forKey:@"__order"];
                           return YES;
                       }];
}

- (iTermEncoderGraphRecord *)record {
    switch (_state) {
        case iTermGraphEncoderStateLive:
            _record = [iTermEncoderGraphRecord withPODs:_pod
                                                 graphs:_children
                                             generation:_generation
                                                    key:_key
                                             identifier:_identifier
                                                  rowid:nil];
            _state = iTermGraphEncoderStateCommitted;
            return _record;

        case iTermGraphEncoderStateCommitted:
            return _record;

        case iTermGraphEncoderStateRolledBack:
            return nil;
    }
}

- (void)transaction:(BOOL (^)(void))block {
    NSMutableDictionary<NSString *, id> *savedPOD = [_pod mutableCopy];
    const NSUInteger savedCount = _children.count;
    const BOOL commit = block();
    if (commit) {
        return;
    }
    _pod = savedPOD;
    if (savedCount < _children.count) {
        DLog(@"Roll back from %@ to %@", @(_children.count), @(savedCount));
        [_children removeObjectsInRange:NSMakeRange(savedCount, _children.count - savedCount)];
    }
}

@end
