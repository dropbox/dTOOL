//
//  iTermMetalRowDataPool.m
//  DashTerm2
//
//  Created by AI on 12/28/24.
//

#import "iTermMetalRowDataPool.h"

#import "iTermData.h"
#import "iTermMetalGlyphKey.h"
#import "iTermMetalRowData.h"
#import "iTermTextRendererCommon.h"
#import <os/lock.h>

// Default max pool size: 4 frames worth at 64 rows each
static const NSUInteger kDefaultMaxPoolSize = 256;

@implementation iTermMetalRowDataPool {
    NSMutableArray<iTermMetalRowData *> *_pool;
    os_unfair_lock _lock;
    NSUInteger _inUseCount;
    NSUInteger _totalAllocations;
    NSUInteger _totalReuses;
    dispatch_source_t _memoryPressureSource;
}

- (instancetype)init {
    return [self initWithMaxPoolSize:kDefaultMaxPoolSize];
}

- (instancetype)initWithMaxPoolSize:(NSUInteger)maxSize {
    self = [super init];
    if (self) {
        _maxPoolSize = maxSize;
        _pool = [[NSMutableArray alloc] initWithCapacity:maxSize];
        _lock = OS_UNFAIR_LOCK_INIT;

        // Register for memory pressure notifications to drain pool when needed
        _memoryPressureSource = dispatch_source_create(
            DISPATCH_SOURCE_TYPE_MEMORYPRESSURE,
            0,
            DISPATCH_MEMORYPRESSURE_WARN | DISPATCH_MEMORYPRESSURE_CRITICAL,
            dispatch_get_main_queue());

        __weak __typeof(self) weakSelf = self;
        dispatch_source_set_event_handler(_memoryPressureSource, ^{
            [weakSelf handleMemoryPressure];
        });
        dispatch_resume(_memoryPressureSource);
    }
    return self;
}

- (void)dealloc {
    if (_memoryPressureSource) {
        dispatch_source_cancel(_memoryPressureSource);
    }
}

- (void)handleMemoryPressure {
    // On memory pressure, drain the pool to free memory
    // In-use objects will naturally be returned when frames complete
    [self drain];
}

- (NSUInteger)pooledCount {
    os_unfair_lock_lock(&_lock);
    NSUInteger count = _pool.count;
    os_unfair_lock_unlock(&_lock);
    return count;
}

- (NSUInteger)inUseCount {
    os_unfair_lock_lock(&_lock);
    NSUInteger count = _inUseCount;
    os_unfair_lock_unlock(&_lock);
    return count;
}

- (NSUInteger)totalAllocations {
    os_unfair_lock_lock(&_lock);
    NSUInteger count = _totalAllocations;
    os_unfair_lock_unlock(&_lock);
    return count;
}

- (NSUInteger)totalReuses {
    os_unfair_lock_lock(&_lock);
    NSUInteger count = _totalReuses;
    os_unfair_lock_unlock(&_lock);
    return count;
}

- (iTermMetalRowData *)acquireRowDataWithColumns:(int)columns {
    iTermMetalRowData *rowData = nil;

    os_unfair_lock_lock(&_lock);
    if (_pool.count > 0) {
        // Reuse from pool
        rowData = _pool.lastObject;
        [_pool removeLastObject];
        _totalReuses++;
    }
    _inUseCount++;
    os_unfair_lock_unlock(&_lock);

    if (rowData) {
        // Reset reused object for new frame
        [self resetRowData:rowData forColumns:columns];
    } else {
        // Allocate new
        rowData = [self createRowDataWithColumns:columns];
        os_unfair_lock_lock(&_lock);
        _totalAllocations++;
        os_unfair_lock_unlock(&_lock);
    }

    return rowData;
}

- (void)returnRowData:(iTermMetalRowData *)rowData {
    if (!rowData) {
        return;
    }

    os_unfair_lock_lock(&_lock);
    _inUseCount--;
    if (_pool.count < _maxPoolSize) {
        [_pool addObject:rowData];
    }
    // else: over capacity, let rowData be released
    os_unfair_lock_unlock(&_lock);
}

- (void)returnRowDataArray:(NSArray<iTermMetalRowData *> *)rowDataArray {
    if (rowDataArray.count == 0) {
        return;
    }

    os_unfair_lock_lock(&_lock);
    _inUseCount -= rowDataArray.count;

    NSUInteger spaceAvailable = _maxPoolSize - _pool.count;
    NSUInteger toAdd = MIN(spaceAvailable, rowDataArray.count);
    if (toAdd > 0) {
        // Add as many as we have space for
        [_pool addObjectsFromArray:[rowDataArray subarrayWithRange:NSMakeRange(0, toAdd)]];
    }
    os_unfair_lock_unlock(&_lock);
}

- (void)drain {
    os_unfair_lock_lock(&_lock);
    [_pool removeAllObjects];
    os_unfair_lock_unlock(&_lock);
}

- (void)resetStats {
    os_unfair_lock_lock(&_lock);
    _totalAllocations = 0;
    _totalReuses = 0;
    os_unfair_lock_unlock(&_lock);
}

#pragma mark - Private

- (iTermMetalRowData *)createRowDataWithColumns:(int)columns {
    iTermMetalRowData *rowData = [[iTermMetalRowData alloc] init];
    [self allocateBuffersForRowData:rowData columns:columns];
    return rowData;
}

- (void)allocateBuffersForRowData:(iTermMetalRowData *)rowData columns:(int)columns {
    const NSUInteger glyphKeySize = sizeof(iTermMetalGlyphKey) * columns;
    const NSUInteger attributesSize = sizeof(iTermMetalGlyphAttributes) * columns;
    const NSUInteger rleSize = sizeof(iTermMetalBackgroundColorRLE) * columns;

    rowData.keysData = [iTermGlyphKeyData dataOfLength:glyphKeySize];
    rowData.attributesData = [iTermAttributesData dataOfLength:attributesSize];
    rowData.backgroundColorRLEData = [iTermBackgroundColorRLEsData dataOfLength:rleSize];
}

- (void)resetRowData:(iTermMetalRowData *)rowData forColumns:(int)columns {
    // Check if buffers need resizing
    const NSUInteger glyphKeySize = sizeof(iTermMetalGlyphKey) * columns;
    const NSUInteger attributesSize = sizeof(iTermMetalGlyphAttributes) * columns;
    const NSUInteger rleSize = sizeof(iTermMetalBackgroundColorRLE) * columns;

    // Resize buffers if needed (iTermData handles this efficiently if size hasn't changed)
    if (rowData.keysData.length < glyphKeySize) {
        rowData.keysData = [iTermGlyphKeyData dataOfLength:glyphKeySize];
    } else {
        rowData.keysData.length = glyphKeySize;
    }

    if (rowData.attributesData.length < attributesSize) {
        rowData.attributesData = [iTermAttributesData dataOfLength:attributesSize];
    } else {
        rowData.attributesData.length = attributesSize;
    }

    if (rowData.backgroundColorRLEData.length < rleSize) {
        rowData.backgroundColorRLEData = [iTermBackgroundColorRLEsData dataOfLength:rleSize];
    } else {
        rowData.backgroundColorRLEData.length = rleSize;
    }

    // Clear other properties
    rowData.y = 0;
    rowData.absLine = 0;
    rowData.screenCharArray = nil;
    rowData.numberOfBackgroundRLEs = 0;
    rowData.numberOfDrawableGlyphs = 0;
    rowData.markStyle = iTermMarkStyleNone;
    rowData.hoverState = NO;
    rowData.lineStyleMark = NO;
    rowData.lineStyleMarkRightInset = 0;
    rowData.date = nil;
    rowData.imageRuns = nil;
    rowData.kittyImageRuns = nil;
    rowData.belongsToBlock = NO;
    rowData.glyphKeyCount = 0;
}

@end
