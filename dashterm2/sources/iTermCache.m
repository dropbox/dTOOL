//
//  iTermCache.m
//  DashTerm2SharedARC
//
//  Created by George Nachman on 11/5/19.
//

#import "iTermCache.h"

#import "DebugLogging.h"
#import "iTermDoublyLinkedList.h"

@interface iTermCacheEntry : NSObject
@property (nonatomic, strong) id key;
@property (nonatomic, strong) id object;
@end

@implementation iTermCacheEntry
@end

@implementation iTermCache {
    // All ivars should be accessed only on _queue.
    NSInteger _capacity;
    NSMutableDictionary<id, iTermDoublyLinkedListEntry<iTermCacheEntry *> *> *_dict;
    iTermDoublyLinkedList *_mru;
    dispatch_queue_t _queue;
    dispatch_source_t _memoryPressureSource;
}

- (instancetype)initWithCapacity:(NSInteger)capacity {
    self = [super init];
    if (self) {
        _capacity = capacity;
        _dict = [NSMutableDictionary dictionaryWithCapacity:capacity];
        _mru = [[iTermDoublyLinkedList alloc] init];
        _queue = dispatch_queue_create("com.dashterm.dashterm2.cache", DISPATCH_QUEUE_SERIAL);
        [self setupMemoryPressureHandler];
    }
    return self;
}

- (void)dealloc {
    if (_memoryPressureSource) {
        dispatch_source_cancel(_memoryPressureSource);
    }
}

- (void)setupMemoryPressureHandler {
    _memoryPressureSource = dispatch_source_create(DISPATCH_SOURCE_TYPE_MEMORYPRESSURE, 0,
                                                   DISPATCH_MEMORYPRESSURE_WARN | DISPATCH_MEMORYPRESSURE_CRITICAL,
                                                   dispatch_get_main_queue());

    __weak __typeof(self) weakSelf = self;
    dispatch_source_t source = _memoryPressureSource;
    dispatch_source_set_event_handler(_memoryPressureSource, ^{
        __strong __typeof(weakSelf) strongSelf = weakSelf;
        if (!strongSelf) {
            return;
        }
        dispatch_source_memorypressure_flags_t flags = dispatch_source_get_data(source);
        if (flags & DISPATCH_MEMORYPRESSURE_CRITICAL) {
            DLog(@"Critical memory pressure detected, clearing iTermCache");
            [strongSelf removeAllObjects];
        } else if (flags & DISPATCH_MEMORYPRESSURE_WARN) {
            DLog(@"Memory pressure warning detected, trimming iTermCache to 50%%");
            [strongSelf trimToCapacity:strongSelf->_capacity / 2];
        }
    });
    dispatch_resume(_memoryPressureSource);
}

- (id)objectForKeyedSubscript:(id)key {
    __block id result = nil;
    dispatch_sync(_queue, ^{
        iTermDoublyLinkedListEntry<iTermCacheEntry *> *entry = self->_dict[key];
        if (!entry) {
            return;
        }
        [self->_mru remove:entry];
        [self->_mru prepend:entry];
        result = entry.object.object;
    });
    return result;
}

- (void)setObject:(id)obj forKeyedSubscript:(id)key {
    dispatch_sync(_queue, ^{
        iTermDoublyLinkedListEntry<iTermCacheEntry *> *dllEntry = self->_dict[key];
        if (dllEntry) {
            [self->_mru remove:dllEntry];
        }
        iTermCacheEntry *cacheEntry = [[iTermCacheEntry alloc] init];
        cacheEntry.key = key;
        cacheEntry.object = obj;
        dllEntry = [[iTermDoublyLinkedListEntry alloc] initWithObject:cacheEntry];
        self->_dict[key] = dllEntry;
        [self->_mru prepend:dllEntry];
        DLog(@"%@ Insert object %@ with key %@", self, obj, key);
        // BUG-f910: Replace assert with log - cache consistency check should not crash
        if (self->_dict.count != self->_mru.count) {
            ELog(@"BUG-f910: iTermCache inconsistency after insert: dict.count=%lu mru.count=%lu",
                 (unsigned long)self->_dict.count, (unsigned long)self->_mru.count);
        }

        while (self->_mru.count > self->_capacity) {
            iTermDoublyLinkedListEntry<iTermCacheEntry *> *lru = self->_mru.last;
            DLog(@"%@ Evict object %@ with key %@", self, lru.object.object, lru.object.key);
            // BUG-f911: Replace assert with guard - missing LRU entry should not crash
            if (!self->_dict[lru.object.key]) {
                ELog(@"BUG-f911: iTermCache LRU entry not found in dict for key: %@", lru.object.key);
                [self->_mru remove:lru];
                continue;
            }
            [self->_dict removeObjectForKey:lru.object.key];
            [self->_mru remove:lru];
            // BUG-f912: Replace assert with log - cache consistency check should not crash
            if (self->_dict.count != self->_mru.count) {
                ELog(@"BUG-f912: iTermCache inconsistency after evict: dict.count=%lu mru.count=%lu",
                     (unsigned long)self->_dict.count, (unsigned long)self->_mru.count);
            }
        }
    });
}

- (void)removeAllObjects {
    dispatch_sync(_queue, ^{
        DLog(@"iTermCache removing all %lu objects", (unsigned long)self->_dict.count);
        [self->_dict removeAllObjects];
        while (self->_mru.count > 0) {
            [self->_mru remove:self->_mru.last];
        }
    });
}

- (void)trimToCapacity:(NSInteger)capacity {
    if (capacity < 0) {
        capacity = 0;
    }
    dispatch_sync(_queue, ^{
        while (self->_mru.count > (NSUInteger)capacity) {
            iTermDoublyLinkedListEntry<iTermCacheEntry *> *lru = self->_mru.last;
            if (!lru) {
                break;
            }
            DLog(@"iTermCache evicting object with key %@ (trimming to capacity %ld)", lru.object.key, (long)capacity);
            [self->_dict removeObjectForKey:lru.object.key];
            [self->_mru remove:lru];
        }
    });
}

@end
