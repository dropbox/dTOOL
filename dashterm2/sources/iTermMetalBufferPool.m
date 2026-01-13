//
//  iTermMetalBufferPool.m
//  DashTerm2SharedARC
//
//  Created by George Nachman on 12/14/17.
//

#import "iTermMetalBufferPool.h"

#import "DebugLogging.h"
#import "iTermHistogram.h"
#import "iTermMalloc.h"
#import <Metal/Metal.h>
#import <os/lock.h>

NS_ASSUME_NONNULL_BEGIN

static NSString *const iTermMetalBufferPoolContextStackKey = @"iTermMetalBufferPoolContextStackKey";

@protocol iTermMetalBufferPool <NSObject>
- (void)returnBuffer:(id<MTLBuffer>)buffer;
@end

@interface iTermMetalBufferPool () <iTermMetalBufferPool>
@end

@interface iTermMetalMixedSizeBufferPool () <iTermMetalBufferPool>
@end

// Optimization: Use a C struct instead of an Objective-C object for buffer entries.
// This eliminates per-entry object allocation overhead during frame rendering.
// Buffers are requested dozens of times per frame, so avoiding alloc/dealloc is significant.
typedef struct {
    __unsafe_unretained id<MTLBuffer> buffer;
    __unsafe_unretained id<iTermMetalBufferPool> pool;
} iTermMetalBufferPoolContextEntry;

// Initial capacity for buffer entries. Will grow if needed.
// Most frames use 50-100 buffers, so 128 is a reasonable starting point.
static const NSUInteger kInitialBufferEntryCapacity = 128;

@interface iTermMetalBufferPoolContext ()
- (void)addBuffer:(id<MTLBuffer>)buffer pool:(id<iTermMetalBufferPool>)pool;
@end

@implementation iTermMetalBufferPoolContext {
    // Struct-based storage for buffer entries to avoid per-entry allocations.
    iTermMetalBufferPoolContextEntry *_entries;
    NSUInteger _entryCount;
    NSUInteger _entryCapacity;
    // Strong references to buffers and pools to keep them alive.
    // We use CFArray for minimal overhead vs NSMutableArray.
    CFMutableArrayRef _retainedBuffers;
    CFMutableArrayRef _retainedPools;
}

- (instancetype)init {
    self = [super init];
    if (self) {
        _entryCapacity = kInitialBufferEntryCapacity;
        _entryCount = 0;
        // BUG-2158: Use iTermCalloc for safe allocation with overflow protection
        _entries =
            (iTermMetalBufferPoolContextEntry *)iTermCalloc(_entryCapacity, sizeof(iTermMetalBufferPoolContextEntry));
        _retainedBuffers = CFArrayCreateMutable(kCFAllocatorDefault, 0, &kCFTypeArrayCallBacks);
        _retainedPools = CFArrayCreateMutable(kCFAllocatorDefault, 0, &kCFTypeArrayCallBacks);
        _histogram = [[iTermHistogram alloc] init];
        _textureHistogram = [[iTermHistogram alloc] init];
        _wasteHistogram = [[iTermHistogram alloc] init];
    }
    return self;
}

- (void)dealloc {
    // Return all buffers to their pools.
    for (NSUInteger i = 0; i < _entryCount; i++) {
        [_entries[i].pool returnBuffer:_entries[i].buffer];
    }
    free(_entries);
    if (_retainedBuffers) {
        CFRelease(_retainedBuffers);
    }
    if (_retainedPools) {
        CFRelease(_retainedPools);
    }
}

- (void)didAddTextureOfSize:(double)size {
    [_textureHistogram addValue:size];
}

- (void)addBuffer:(id<MTLBuffer>)buffer pool:(id<iTermMetalBufferPool>)pool {
    [_histogram addValue:buffer.length];

    // Grow the entry array if needed.
    if (_entryCount >= _entryCapacity) {
        // BUG-2159: Use iTermRealloc for safe reallocation with overflow protection
        _entryCapacity *= 2;
        _entries = (iTermMetalBufferPoolContextEntry *)iTermRealloc(_entries, _entryCapacity,
                                                                    sizeof(iTermMetalBufferPoolContextEntry));
    }

    // Store the entry (unsafe_unretained pointers for fast access).
    _entries[_entryCount].buffer = buffer;
    _entries[_entryCount].pool = pool;
    _entryCount++;

    // Retain the objects to keep them alive.
    CFArrayAppendValue(_retainedBuffers, (__bridge const void *)buffer);
    CFArrayAppendValue(_retainedPools, (__bridge const void *)pool);
}

- (void)relinquishOwnershipOfBuffer:(id<MTLBuffer>)buffer {
    // Find and remove the entry for this buffer.
    for (NSUInteger i = 0; i < _entryCount; i++) {
        if (_entries[i].buffer == buffer) {
            // Remove from retained arrays.
            CFIndex bufferIndex = CFArrayGetFirstIndexOfValue(
                _retainedBuffers, CFRangeMake(0, CFArrayGetCount(_retainedBuffers)), (__bridge const void *)buffer);
            if (bufferIndex != kCFNotFound) {
                CFArrayRemoveValueAtIndex(_retainedBuffers, bufferIndex);
                CFArrayRemoveValueAtIndex(_retainedPools, bufferIndex);
            }
            // Shift remaining entries down.
            if (i < _entryCount - 1) {
                memmove(&_entries[i], &_entries[i + 1],
                        (_entryCount - i - 1) * sizeof(iTermMetalBufferPoolContextEntry));
            }
            _entryCount--;
            break;
        }
    }
}

- (void)addWastedSpace:(double)wastedSpace {
    [_wasteHistogram addValue:wastedSpace];
}

- (NSString *)summaryStatisticsWithName:(NSString *)name {
    NSMutableString *string = [NSMutableString stringWithFormat:@"%@\n", name];
    if (_histogram.count) {
        [string appendFormat:@"  Buffer sizes: %@\n", [_histogram sparklines]];
    }
    if (_wasteHistogram.count) {
        [string appendFormat:@"  Wasted space: %@\n", [_wasteHistogram sparklines]];
    }
    if (_textureHistogram.count) {
        [string appendFormat:@"  New textures: %@\n", [_textureHistogram sparklines]];
    }
    return string;
}

@end

@implementation iTermMetalMixedSizeBufferPool {
    id<MTLDevice> _device;
    NSMutableArray<id<MTLBuffer>> *_buffers;
    NSUInteger _capacity;
    NSInteger _numberOutstanding;
    os_unfair_lock _lock; // Performance optimization: os_unfair_lock has ~10x lower overhead than @synchronized
    dispatch_source_t _memoryPressureSource;
}

- (instancetype)initWithDevice:(id<MTLDevice>)device capacity:(NSUInteger)capacity name:(nonnull NSString *)name {
    self = [super init];
    if (self) {
        _name = [name copy];
        _device = device;
        _capacity = capacity;
        _buffers = [NSMutableArray arrayWithCapacity:capacity];
        _lock = OS_UNFAIR_LOCK_INIT;
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
            DLog(@"Critical memory pressure detected, clearing iTermMetalMixedSizeBufferPool %@", strongSelf.name);
            [strongSelf drain];
        } else if (flags & DISPATCH_MEMORYPRESSURE_WARN) {
            DLog(@"Memory pressure warning detected, trimming iTermMetalMixedSizeBufferPool %@ to 50%%", strongSelf.name);
            [strongSelf trimToCapacity:strongSelf->_capacity / 2];
        }
    });
    dispatch_resume(_memoryPressureSource);
}

- (void)drain {
    os_unfair_lock_lock(&_lock);
    NSUInteger count = _buffers.count;
    [_buffers removeAllObjects];
    os_unfair_lock_unlock(&_lock);
    DLog(@"iTermMetalMixedSizeBufferPool %@ drained %lu buffers", _name, (unsigned long)count);
}

- (void)trimToCapacity:(NSUInteger)targetCapacity {
    os_unfair_lock_lock(&_lock);
    NSUInteger removed = 0;
    // Remove smallest buffers first (they're at the beginning)
    while (_buffers.count > targetCapacity) {
        [_buffers removeObjectAtIndex:0];
        removed++;
    }
    os_unfair_lock_unlock(&_lock);
    if (removed > 0) {
        DLog(@"iTermMetalMixedSizeBufferPool %@ trimmed %lu buffers", _name, (unsigned long)removed);
    }
}

- (id<MTLBuffer>)requestBufferFromContext:(iTermMetalBufferPoolContext *)context size:(size_t)size {
    // BUG-f969: Use guard instead of assert - nil context should return nil with warning
    if (context == nil) {
        ELog(@"BUG-f969: requestBufferFromContext:size: called with nil context - returning nil");
        return nil;
    }
    os_unfair_lock_lock(&_lock);
    id<MTLBuffer> buffer;
    NSInteger index = [self indexOfFirstBufferWithLengthAtLeast:size];
    if (index != NSNotFound) {
        buffer = _buffers[(NSUInteger)index];
        [_buffers removeObjectAtIndex:index];
    } else {
        buffer = [_device newBufferWithLength:size options:MTLResourceStorageModeShared];
    }
    [context addBuffer:buffer pool:self];
    [context addWastedSpace:buffer.length - size];
    os_unfair_lock_unlock(&_lock);
    return buffer;
}

- (NSInteger)indexOfFirstBufferWithLengthAtLeast:(size_t)length {
    NSInteger index = [self lowerBoundIndexForBufferLength:length];
    return (index < (NSInteger)_buffers.count) ? index : NSNotFound;
}

- (NSInteger)lowerBoundIndexForBufferLength:(size_t)length {
    NSInteger low = 0;
    NSInteger high = (NSInteger)_buffers.count;
    while (low < high) {
        NSInteger mid = (low + high) >> 1;
        id<MTLBuffer> candidate = _buffers[(NSUInteger)mid];
        const size_t candidateLength = candidate.length;
        if (candidateLength < length) {
            low = mid + 1;
        } else {
            high = mid;
        }
    }
    return low;
}

- (id<MTLBuffer>)requestBufferFromContext:(iTermMetalBufferPoolContext *)context
                                     size:(size_t)size
                                    bytes:(nonnull const void *)bytes {
    // BUG-f969: Use guard instead of assert - nil context should return nil with warning
    if (context == nil) {
        ELog(@"BUG-f969: requestBufferFromContext:size:bytes: called with nil context - returning nil");
        return nil;
    }
    os_unfair_lock_lock(&_lock);
    id<MTLBuffer> buffer;
    NSInteger index = [self indexOfFirstBufferWithLengthAtLeast:size];
    if (index != NSNotFound) {
        id<MTLBuffer> bestMatch = _buffers[(NSUInteger)index];
        [_buffers removeObjectAtIndex:index];
        buffer = bestMatch;
        memcpy(buffer.contents, bytes, size);
    } else {
        // size was larger than the largest item
        DLog(@"%@ allocating a new buffer of size %d (%d outstanding)", _name, (int)size, (int)_numberOutstanding);
        buffer = [_device newBufferWithBytes:bytes length:size options:MTLResourceStorageModeShared];
    }
    [context addBuffer:buffer pool:self];
    [context addWastedSpace:buffer.length - size];
    _numberOutstanding++;
    os_unfair_lock_unlock(&_lock);
    return buffer;
}

- (void)returnBuffer:(id<MTLBuffer>)buffer {
    os_unfair_lock_lock(&_lock);
    _numberOutstanding--;
    if (_buffers.count == _capacity) {
        [_buffers removeObjectAtIndex:0];
    }
    NSInteger index = [self lowerBoundIndexForBufferLength:buffer.length];
    [_buffers insertObject:buffer atIndex:index];
    os_unfair_lock_unlock(&_lock);
}

@end

@implementation iTermMetalBufferPool {
    id<MTLDevice> _device;
    size_t _bufferSize;
    NSMutableArray<id<MTLBuffer>> *_buffers;
    os_unfair_lock _lock; // Performance optimization: os_unfair_lock has ~10x lower overhead than @synchronized
    dispatch_source_t _memoryPressureSource;
}

- (instancetype)initWithDevice:(id<MTLDevice>)device bufferSize:(size_t)bufferSize {
    self = [super init];
    if (self) {
        _device = device;
        _bufferSize = bufferSize;
        // Pre-allocate for typical buffer pool size - pools hold 4-8 buffers for reuse
        _buffers = [NSMutableArray arrayWithCapacity:8];
        _lock = OS_UNFAIR_LOCK_INIT;
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
            DLog(@"Critical memory pressure detected, clearing iTermMetalBufferPool");
            [strongSelf drain];
        } else if (flags & DISPATCH_MEMORYPRESSURE_WARN) {
            DLog(@"Memory pressure warning detected, trimming iTermMetalBufferPool to 50%%");
            [strongSelf trimToCount:4];
        }
    });
    dispatch_resume(_memoryPressureSource);
}

- (void)drain {
    os_unfair_lock_lock(&_lock);
    NSUInteger count = _buffers.count;
    [_buffers removeAllObjects];
    os_unfair_lock_unlock(&_lock);
    DLog(@"iTermMetalBufferPool drained %lu buffers", (unsigned long)count);
}

- (void)trimToCount:(NSUInteger)targetCount {
    os_unfair_lock_lock(&_lock);
    NSUInteger removed = 0;
    while (_buffers.count > targetCount) {
        [_buffers removeLastObject];
        removed++;
    }
    os_unfair_lock_unlock(&_lock);
    if (removed > 0) {
        DLog(@"iTermMetalBufferPool trimmed %lu buffers", (unsigned long)removed);
    }
}

- (void)setBufferSize:(size_t)bufferSize {
    os_unfair_lock_lock(&_lock);
    if (bufferSize != _bufferSize) {
        _bufferSize = bufferSize;
        [_buffers removeAllObjects];
    }
    os_unfair_lock_unlock(&_lock);
}

- (id<MTLBuffer>)requestBufferFromContext:(iTermMetalBufferPoolContext *)context {
    // BUG-f969: Use guard instead of assert - nil context should return nil with warning
    if (context == nil) {
        ELog(@"BUG-f969: requestBufferFromContext: called with nil context - returning nil");
        return nil;
    }
    os_unfair_lock_lock(&_lock);
    id<MTLBuffer> buffer;
    if (_buffers.count) {
        buffer = _buffers.lastObject;
        [_buffers removeLastObject];
    } else {
        buffer = [_device newBufferWithLength:_bufferSize options:MTLResourceStorageModeShared];
    }
    [context addBuffer:buffer pool:self];
    os_unfair_lock_unlock(&_lock);
    return buffer;
}

- (id<MTLBuffer>)requestBufferFromContext:(iTermMetalBufferPoolContext *)context
                                withBytes:(const void *)bytes
                           checkIfChanged:(BOOL)checkIfChanged {
    // BUG-f969: Use guard instead of assert - nil context should return nil with warning
    if (context == nil) {
        ELog(@"BUG-f969: requestBufferFromContext:withBytes:checkIfChanged: called with nil context - returning nil");
        return nil;
    }
    os_unfair_lock_lock(&_lock);
    id<MTLBuffer> buffer;
    if (_buffers.count) {
        buffer = _buffers.lastObject;
        [_buffers removeLastObject];
        if (checkIfChanged) {
            if (memcmp(bytes, buffer.contents, _bufferSize)) {
                memcpy(buffer.contents, bytes, _bufferSize);
            }
        } else {
            memcpy(buffer.contents, bytes, _bufferSize);
        }
    } else {
        buffer = [_device newBufferWithBytes:bytes length:_bufferSize options:MTLResourceStorageModeShared];
    }
    [context addBuffer:buffer pool:self];
    ITAssertWithMessage(buffer != nil, @"Failed to allocate buffer of size %@", @(_bufferSize));
    os_unfair_lock_unlock(&_lock);
    return buffer;
}

// High-water mark for buffer pool. When pool exceeds this size on return,
// trim to half capacity to prevent unbounded memory growth.
// Value chosen based on typical frame rendering patterns: most frames use 4-8 buffers,
// so 16 provides headroom while bounding memory.
static const NSUInteger kMaxPooledBuffers = 16;

- (void)returnBuffer:(id<MTLBuffer>)buffer {
    os_unfair_lock_lock(&_lock);
    [_buffers addObject:buffer];
    // Section 4.3: High-water mark pruning to prevent unbounded growth
    if (_buffers.count > kMaxPooledBuffers) {
        NSUInteger targetCount = kMaxPooledBuffers / 2;
        while (_buffers.count > targetCount) {
            [_buffers removeLastObject];
        }
        DLog(@"iTermMetalBufferPool pruned to %lu buffers (high-water mark exceeded)",
             (unsigned long)_buffers.count);
    }
    os_unfair_lock_unlock(&_lock);
}

@end

NS_ASSUME_NONNULL_END
