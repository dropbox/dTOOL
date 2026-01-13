//
//  iTermCache.h
//  DashTerm2
//
//  Created by George Nachman on 11/5/19.
//

#import <Foundation/Foundation.h>

NS_ASSUME_NONNULL_BEGIN

@interface iTermCache<KeyType, ValueType>: NSObject

- (instancetype)initWithCapacity:(NSInteger)capacity NS_DESIGNATED_INITIALIZER;
- (instancetype)init NS_UNAVAILABLE;

- (nullable id)objectForKeyedSubscript:(KeyType<NSCopying>)key;
- (void)setObject:(ValueType)obj forKeyedSubscript:(KeyType<NSCopying>)key;

/// Removes all entries from the cache.
- (void)removeAllObjects;

/// Trims the cache to the specified capacity, evicting least recently used entries.
/// @param capacity The target capacity. If current count is less than or equal to capacity, no action is taken.
- (void)trimToCapacity:(NSInteger)capacity;

@end

NS_ASSUME_NONNULL_END
