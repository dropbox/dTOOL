//
//  iTermURLStore.m
//  DashTerm2
//
//  Created by George Nachman on 3/19/17.
//
//

#import "iTermURLStore.h"

#import "DebugLogging.h"
#import "NSObject+iTerm.h"
#import "iTermChangeTrackingDictionary.h"
#import "iTermGraphEncoder.h"
#import "iTermTuple.h"

#import <Cocoa/Cocoa.h>
#import <os/lock.h>

// Optimization: Cache NSNumber objects for common URL codes (0-4095).
// URL codes are unsigned ints starting at 1 that rarely exceed a few hundred in typical use.
// Caching 4096 values covers most applications with many hyperlinks.
static const unsigned int kCachedURLCodeCount = 4096;
static NSNumber *sURLCodeCache[kCachedURLCodeCount];

NS_INLINE NSNumber *iTermURLCodeToNumber(unsigned int code) {
    if (code < kCachedURLCodeCount) {
        return sURLCodeCache[code];
    }
    return @(code);
}

__attribute__((constructor)) static void iTermURLStoreInitializeCodeCache(void) {
    for (unsigned int i = 0; i < kCachedURLCodeCount; i++) {
        sURLCodeCache[i] = @(i);
    }
}

@implementation iTermURLStore {
    // { "url": NSURL.absoluteString, "params": NSString } -> @(NSInteger)
    iTermChangeTrackingDictionary<iTermTuple<NSString *, NSString *> *, NSNumber *> *_store;

    // @(unsigned int) -> { "url": NSURL, "params": NSString }
    NSMutableDictionary<NSNumber *, iTermTuple<NSURL *, NSString *> *> *_reverseStore;

    iTermChangeTrackingDictionary<NSNumber *, NSNumber *> *_referenceCounts;

    // Will never be zero.
    NSInteger _nextCode;

    // Lock for thread-safe access to store data structures
    os_unfair_lock _lock;
}

+ (instancetype)sharedInstance {
    static dispatch_once_t onceToken;
    static id instance;
    dispatch_once(&onceToken, ^{
        instance = [[self alloc] init];
    });
    return instance;
}

+ (unsigned int)successor:(unsigned int)n {
    if (n >= UINT_MAX - 1) {
        return 1;
    }
    return n + 1;
}

- (instancetype)init {
    self = [super init];
    if (self) {
        _store = [[iTermChangeTrackingDictionary alloc] init];
        _reverseStore = [NSMutableDictionary dictionaryWithCapacity:64];
        _referenceCounts = [[iTermChangeTrackingDictionary alloc] init];
        _nextCode = 1;
        _lock = OS_UNFAIR_LOCK_INIT;
    }
    return self;
}

- (void)retainCode:(unsigned int)code {
    os_unfair_lock_lock(&_lock);
    _generation++;
    dispatch_async(dispatch_get_main_queue(), ^{
        [NSApp invalidateRestorableState];
    });
    NSNumber *codeKey = iTermURLCodeToNumber(code);
    DLog(@"Retain %@. New rc=%@", [_reverseStore[codeKey] firstObject], @(_referenceCounts[codeKey].integerValue + 1));
    _referenceCounts[codeKey] = @(_referenceCounts[codeKey].integerValue + 1);
    os_unfair_lock_unlock(&_lock);
}

- (void)releaseCode:(unsigned int)code {
    os_unfair_lock_lock(&_lock);
    _generation++;
    dispatch_async(dispatch_get_main_queue(), ^{
        [NSApp invalidateRestorableState];
    });
    NSNumber *codeKey = iTermURLCodeToNumber(code);
    DLog(@"Release %@. New rc=%@", [_reverseStore[codeKey] firstObject], @(_referenceCounts[codeKey].integerValue - 1));
    NSNumber *count = _referenceCounts[codeKey];
    if (count.integerValue <= 1) {
        [_referenceCounts removeObjectForKey:codeKey];
        iTermTuple<NSURL *, NSString *> *tuple = _reverseStore[codeKey];
        [_reverseStore removeObjectForKey:codeKey];
        NSString *url = [tuple.firstObject absoluteString];
        NSString *params = tuple.secondObject;
        if (url) {
            [_store removeObjectForKey:[iTermTuple tupleWithObject:url andObject:params]];
        }
    } else {
        _referenceCounts[codeKey] = @(count.integerValue - 1);
    }
    os_unfair_lock_unlock(&_lock);
}

- (unsigned int)codeForURL:(NSURL *)url withParams:(NSString *)params {
    if (!url.absoluteString || !params) {
        DLog(@"codeForURL:%@ withParams:%@ returning 0 because of nil value", url.absoluteString, params);
        return 0;
    }
    os_unfair_lock_lock(&_lock);
    if (_nextCode < 1) {
        ELog(@"URL store next code invalid (%@). Resetting to 1.", @(_nextCode));
        _nextCode = 1;
    }
    iTermTuple<NSString *, NSString *> *key = [iTermTuple tupleWithObject:url.absoluteString andObject:params];
    NSNumber *number = _store[key];
    if (number) {
        os_unfair_lock_unlock(&_lock);
        return number.unsignedIntValue;
    }
    if (_reverseStore.count == USHRT_MAX - 1) {
        DLog(@"Ran out of URL storage. Refusing to allocate a code.");
        os_unfair_lock_unlock(&_lock);
        return 0;
    }
    // Advance _nextCode to the next unused code. This will not normally happen - only on wraparound.
    while (_reverseStore[iTermURLCodeToNumber(_nextCode)]) {
        _nextCode = [iTermURLStore successor:_nextCode];
    }

    // Save it and advance.
    number = iTermURLCodeToNumber(_nextCode);
    _nextCode = [iTermURLStore successor:_nextCode];

    // Record the code/URL+params relation.
    _store[key] = number;
    _reverseStore[number] = [iTermTuple tupleWithObject:url andObject:params];

    dispatch_async(dispatch_get_main_queue(), ^{
        [NSApp invalidateRestorableState];
    });
    _generation++;
    os_unfair_lock_unlock(&_lock);
    return number.unsignedIntValue;
}

- (NSURL *)urlForCode:(unsigned int)code {
    if (code == 0) {
        // Safety valve in case something goes awry. There should never be an entry at 0.
        return nil;
    }
    os_unfair_lock_lock(&_lock);
    NSURL *result = _reverseStore[iTermURLCodeToNumber(code)].firstObject;
    os_unfair_lock_unlock(&_lock);
    return result;
}

- (NSString *)paramsForCode:(unsigned int)code {
    if (code == 0) {
        // Safety valve in case something goes awry. There should never be an entry at 0.
        return nil;
    }
    os_unfair_lock_lock(&_lock);
    NSString *result = _reverseStore[iTermURLCodeToNumber(code)].secondObject;
    os_unfair_lock_unlock(&_lock);
    return result;
}

- (NSString *)paramWithKey:(NSString *)key forCode:(unsigned int)code {
    NSString *params = [self paramsForCode:code];
    if (!params) {
        return nil;
    }
    return iTermURLStoreGetParamForKey(params, key);
}

static NSString *iTermURLStoreGetParamForKey(NSString *params, NSString *key) {
    NSArray<NSString *> *parts = [params componentsSeparatedByString:@":"];
    for (NSString *part in parts) {
        NSInteger i = [part rangeOfString:@"="].location;
        if (i != NSNotFound) {
            NSString *partKey = [part substringToIndex:i];
            if ([partKey isEqualToString:key]) {
                return [part substringFromIndex:i + 1];
            }
        }
    }
    return nil;
}

- (NSDictionary *)dictionaryValue {
    os_unfair_lock_lock(&_lock);
    NSMutableArray<NSNumber *> *encodedRefcounts =
        [NSMutableArray arrayWithCapacity:_referenceCounts.allKeys.count * 2];
    for (NSNumber *obj in _referenceCounts.allKeys) {
        [encodedRefcounts addObject:obj];
        [encodedRefcounts addObject:_referenceCounts[obj]];
    };

    NSDictionary *result = @{@"store" : _store.dictionary, @"refcounts3" : encodedRefcounts};
    os_unfair_lock_unlock(&_lock);
    return result;
}

- (BOOL)encodeGraphWithEncoder:(iTermGraphEncoder *)encoder {
    return YES;
}

- (void)loadFromGraphRecord:(iTermEncoderGraphRecord *)record {
    [_store loadFromRecord:[record childRecordWithKey:@"store" identifier:@""]
                  keyClass:[iTermTuple class]
                valueClass:[NSNumber class]];
    NSMutableArray<iTermTuple<NSString *, NSString *> *> *keysToRemove = [NSMutableArray arrayWithCapacity:4];
    [_store enumerateKeysAndObjectsUsingBlock:^(iTermTuple<NSString *, NSString *> *key, NSNumber *obj, BOOL *stop) {
        if (obj.unsignedIntValue == 0) {
            ELog(@"Skipping URL store entry with reserved code 0 for %@", key);
            [keysToRemove addObject:key];
            return;
        }
        _reverseStore[obj] = [iTermTuple tupleWithObject:[NSURL URLWithString:key.firstObject]
                                               andObject:key.secondObject];
        _nextCode = [iTermURLStore successor:MAX(obj.unsignedIntValue, _nextCode)];
    }];
    for (iTermTuple<NSString *, NSString *> *key in keysToRemove) {
        [_store removeObjectForKey:key];
    }
    [_referenceCounts loadFromRecord:[record childRecordWithKey:@"referenceCounts" identifier:@""]
                            keyClass:[NSNumber class]
                          valueClass:[NSNumber class]];
    [_referenceCounts removeObjectForKey:iTermURLCodeToNumber(0)];
    if (_nextCode < 1) {
        ELog(@"URL store next code invalid (%@) after graph load. Resetting to 1.", @(_nextCode));
        _nextCode = 1;
    }
}

- (iTermTuple<NSString *, NSString *> *)migratedKey:(id)unknownKey {
    NSDictionary *dict = [NSDictionary castFrom:unknownKey];
    if (dict) {
        return [iTermTuple tupleWithObject:dict[@"url"] andObject:dict[@"param"]];
    }
    return [iTermTuple castFrom:unknownKey];
}

- (void)loadFromDictionary:(NSDictionary *)dictionary {
    NSDictionary *store = dictionary[@"store"];
    NSData *refcounts = dictionary[@"refcounts"];   // deprecated
    NSData *refcounts2 = dictionary[@"refcounts2"]; // deprecated
    NSArray<NSNumber *> *refcounts3 = dictionary[@"refcounts3"];

    if (!store || (!refcounts && !refcounts2 && !refcounts3)) {
        DLog(@"URLStore restoration dictionary missing value");
        DLog(@"store=%@", store);
        DLog(@"refcounts=%@", refcounts);
        DLog(@"refcounts2=%@", refcounts2);
        DLog(@"refcounts3=%@", refcounts3);
        return;
    }

    os_unfair_lock_lock(&_lock);
    [store enumerateKeysAndObjectsUsingBlock:^(id unknownKey, NSNumber *_Nonnull obj, BOOL *_Nonnull stop) {
        iTermTuple<NSString *, NSString *> *key = [self migratedKey:unknownKey];

        if (!key || ![obj isKindOfClass:[NSNumber class]]) {
            ELog(@"Unexpected types when loading dictionary: %@ -> %@", key.class, obj.class);
            return;
        }
        NSURL *url = [NSURL URLWithString:key.firstObject];
        if (url == nil) {
            XLog(@"Bogus key not a URL: %@", url);
            return;
        }
        if (obj.unsignedIntValue == 0) {
            ELog(@"Skipping URL store entry with reserved code 0 for %@", key);
            return;
        }
        self->_store[key] = obj;

        self->_reverseStore[obj] = [iTermTuple tupleWithObject:url andObject:key.secondObject ?: @""];
        self->_nextCode = [iTermURLStore successor:MAX(obj.unsignedIntValue, self->_nextCode)];
    }];

    NSError *error = nil;
    if (refcounts2) {
        NSCountedSet *countedSet =
            [NSKeyedUnarchiver
                unarchivedObjectOfClasses:[NSSet setWithArray:@[ [NSCountedSet class], [NSNumber class] ]]
                                 fromData:refcounts2
                                    error:&error]
                ?: [[NSCountedSet alloc] init];
        if (error) {
            countedSet = [self legacyDecodedRefcounts:dictionary];
            DLog(@"Failed to decode refcounts from data %@", refcounts2);
        }
        if (countedSet) {
            [countedSet enumerateObjectsUsingBlock:^(id _Nonnull obj, BOOL *_Nonnull stop) {
                _referenceCounts[obj] = @([countedSet countForObject:obj]);
            }];
        }
        os_unfair_lock_unlock(&_lock);
        return;
    }
    if (refcounts3) {
        const NSInteger count = refcounts3.count;
        for (NSInteger i = 0; i + 1 < count; i += 2) {
            NSNumber *obj = refcounts3[i];
            if (obj.unsignedIntValue == 0) {
                ELog(@"Skipping URL store refcount with reserved code 0");
                continue;
            }
            _referenceCounts[obj] = refcounts3[i + 1];
        }
    }
    if (_nextCode < 1) {
        ELog(@"URL store next code invalid (%@) after dictionary load. Resetting to 1.", @(_nextCode));
        _nextCode = 1;
    }
    os_unfair_lock_unlock(&_lock);
}

- (NSCountedSet *)legacyDecodedRefcounts:(NSDictionary *)dictionary {
    NSData *refcounts = dictionary[@"refcounts"];
    if (!refcounts) {
        return [[NSCountedSet alloc] init];
    }
    // TODO: Remove this after the old format is no longer around. Probably safe to do around mid 2023.
    NSError *error = nil;
    NSKeyedUnarchiver *decoder = [[NSKeyedUnarchiver alloc] initForReadingFromData:refcounts error:&error];
    if (error) {
        DLog(@"Failed to decode refcounts from data %@", refcounts);
        return [[NSCountedSet alloc] init];
    }
    return [[NSCountedSet alloc] initWithCoder:decoder] ?: [[NSCountedSet alloc] init];
}

@end
