//
//  iTermMetalPerFrameStateRowPool.m
//  DashTerm2
//
//  Created by AI Worker on 12/17/25.
//

#import "iTermMetalPerFrameStateRowPool.h"
#import "iTermMetalPerFrameStateRow.h"

// Maximum number of row objects to keep in the pool.
// This should accommodate typical terminal sizes (24-80 visible rows).
// Excess objects are released when returned.
static const NSUInteger kMaxPoolSize = 128;

// Initial capacity for the internal storage array.
static const NSUInteger kInitialCapacity = 32;

@implementation iTermMetalPerFrameStateRowPool {
    // Stack-based pool for O(1) acquire/return.
    // Using CFMutableArray for better performance than NSMutableArray.
    CFMutableArrayRef _pool;
    NSUInteger _poolCount;
    dispatch_source_t _memoryPressureSource;
}

+ (instancetype)sharedPool {
    static iTermMetalPerFrameStateRowPool *instance;
    static dispatch_once_t onceToken;
    dispatch_once(&onceToken, ^{
        instance = [[iTermMetalPerFrameStateRowPool alloc] init];
    });
    return instance;
}

- (instancetype)init {
    self = [super init];
    if (self) {
        _pool = CFArrayCreateMutable(kCFAllocatorDefault, kInitialCapacity, &kCFTypeArrayCallBacks);
        _poolCount = 0;
        _totalAcquisitions = 0;
        _totalReturns = 0;
        [self setupMemoryPressureHandler];
    }
    return self;
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
        if (flags & (DISPATCH_MEMORYPRESSURE_WARN | DISPATCH_MEMORYPRESSURE_CRITICAL)) {
            NSLog(@"Memory pressure detected, draining iTermMetalPerFrameStateRowPool");
            [strongSelf drain];
        }
    });
    dispatch_resume(_memoryPressureSource);
}

- (void)dealloc {
    if (_memoryPressureSource) {
        dispatch_source_cancel(_memoryPressureSource);
    }
    if (_pool) {
        CFRelease(_pool);
    }
}

- (NSUInteger)pooledCount {
    return _poolCount;
}

- (iTermMetalPerFrameStateRow *)acquireRow {
    _totalAcquisitions++;

    if (_poolCount > 0) {
        // Pop from the pool (LIFO for cache-friendliness).
        _poolCount--;
        iTermMetalPerFrameStateRow *row =
            (__bridge_transfer iTermMetalPerFrameStateRow *)CFArrayGetValueAtIndex(_pool, _poolCount);
        CFArrayRemoveValueAtIndex(_pool, _poolCount);
        return row;
    }

    // Pool empty - caller should allocate a new row.
    return nil;
}

- (void)returnRow:(iTermMetalPerFrameStateRow *)row {
    if (!row) {
        return;
    }

    _totalReturns++;

    if (_poolCount < kMaxPoolSize) {
        // Clear object references to allow memory to be freed.
        [self clearRowReferences:row];

        CFArrayAppendValue(_pool, (__bridge const void *)row);
        _poolCount++;
    }
    // If pool is full, object is simply released (ARC handles it).
}

- (void)returnRows:(NSArray<iTermMetalPerFrameStateRow *> *)rows {
    for (iTermMetalPerFrameStateRow *row in rows) {
        [self returnRow:row];
    }
}

- (void)drain {
    if (_pool) {
        CFArrayRemoveAllValues(_pool);
    }
    _poolCount = 0;
}

#pragma mark - Private

- (void)clearRowReferences:(iTermMetalPerFrameStateRow *)row {
    // Clear object references to allow them to be deallocated.
    // The row's primitive fields will be overwritten on next use.
    row->_screenCharLine = nil;
    row->_selectedIndexSet = nil;
    row->_date = nil;
    row->_matches = nil;
    row->_eaIndex = nil;
}

@end
