//
//  iTermOrderedDictionary.m
//  DashTerm2
//
//  Created by George Nachman on 7/29/20.
//

#import "iTermOrderedDictionary.h"

#import "NSArray+iTerm.h"
#import "NSDictionary+iTerm.h"

@implementation iTermOrderedDictionary {
    NSArray *_orderedKeys;
    NSDictionary *_dictionary;
}

+ (instancetype)byMapping:(NSArray<id> *)array block:(nullable id (^NS_NOESCAPE)(NSUInteger, id))block {
    return [self byMappingEnumerator:array.objectEnumerator block:block];
}

+ (instancetype)byMappingEnumerator:(NSEnumerator *)enumerator
                              block:(nullable id (^NS_NOESCAPE)(NSUInteger index, id object))block {
    // Start with reasonable capacity; will grow if needed
    NSMutableArray *keys = [NSMutableArray arrayWithCapacity:16];
    NSMutableDictionary *dictionary = [NSMutableDictionary dictionaryWithCapacity:16];

    NSInteger idx = 0;
    for (id obj = enumerator.nextObject; obj; obj = enumerator.nextObject) {
        id mapped = block(idx, obj);
        if (!mapped) {
            continue;
        }
        if (dictionary[mapped] != nil) {
            // Ignore duplicate keys.
            continue;
        }
        [keys addObject:mapped];
        dictionary[mapped] = obj;
        idx += 1;
    };
    return [[self alloc] initWithArray:keys dictionary:dictionary];
}

+ (instancetype)withTuples:(NSArray<iTermTuple *> *)tuples {
    NSArray *orderedKeys = [tuples mapWithBlock:^id(iTermTuple *tuple) {
        return tuple.firstObject;
    }];
    NSDictionary *dictionary = [tuples keyValuePairsWithBlock:^iTermTuple *(iTermTuple *object) {
        return object;
    }];
    return [[self alloc] initWithArray:orderedKeys dictionary:dictionary];
}

- (instancetype)initWithArray:(NSArray *)array dictionary:(NSDictionary *)dictionary {
    self = [super init];
    if (self) {
        _orderedKeys = array;
        _dictionary = dictionary;
    }
    return self;
}

- (NSString *)debugString {
    if ([[NSSet setWithArray:_orderedKeys] count] == _orderedKeys.count) {
        return @"ok";
    }
    NSCountedSet *countedSet = [[NSCountedSet alloc] initWithArray:_orderedKeys];
    // Duplicates are unusual, typically 0-3
    NSMutableArray<NSString *> *dups = [NSMutableArray arrayWithCapacity:4];
    for (id obj in countedSet) {
        if ([countedSet countForObject:obj] > 1) {
            [dups addObject:[obj description]];
        }
    }
    return [dups componentsJoinedByString:@", "];
}

- (NSArray *)keys {
    return _orderedKeys;
}

- (NSArray *)values {
    return [_orderedKeys mapWithBlock:^id(id anObject) {
        return _dictionary[anObject];
    }];
}

- (nullable id)objectForKeyedSubscript:(id)key {
    return _dictionary[key];
}

@end
