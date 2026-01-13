//
//  iTermMetalRowDataPool.h
//  DashTerm2
//
//  Created by AI on 12/28/24.
//
//  Object pool for iTermMetalRowData to reduce per-frame allocation overhead.
//  Instead of allocating new row data objects for every dirty row each frame,
//  this pool recycles objects, significantly reducing malloc pressure and
//  improving cache locality.
//

#import <Foundation/Foundation.h>

@class iTermMetalRowData;

NS_ASSUME_NONNULL_BEGIN

/// Thread-safe object pool for iTermMetalRowData.
///
/// Usage:
///   1. Acquire row data for dirty rows: `[pool acquireRowDataWithColumns:]`
///   2. After frame completes, return all to pool: `[pool returnRowData:]`
///
/// The pool pre-allocates data buffers sized for a given column count.
/// If the column count changes, pooled objects are resized as needed.
NS_CLASS_AVAILABLE(10_11, NA)
@interface iTermMetalRowDataPool : NSObject

/// Maximum number of row data objects to keep pooled.
/// Objects beyond this limit are released instead of pooled.
/// Default: 256 (supports 4 frames at 64 rows each)
@property (nonatomic) NSUInteger maxPoolSize;

/// Number of row data objects currently in the pool (available for reuse).
@property (nonatomic, readonly) NSUInteger pooledCount;

/// Number of row data objects currently in use (acquired but not returned).
@property (nonatomic, readonly) NSUInteger inUseCount;

/// Total allocations since pool creation (for debugging/metrics).
@property (nonatomic, readonly) NSUInteger totalAllocations;

/// Total reuses from pool (for debugging/metrics).
@property (nonatomic, readonly) NSUInteger totalReuses;

/// Create a pool with the default max size.
- (instancetype)init;

/// Create a pool with a specific max size.
- (instancetype)initWithMaxPoolSize:(NSUInteger)maxSize NS_DESIGNATED_INITIALIZER;

/// Acquire a row data object, either from pool or newly allocated.
/// The returned object has its data buffers pre-allocated for the given column count.
/// @param columns Number of columns (used to size internal buffers)
/// @return A ready-to-use iTermMetalRowData object
- (iTermMetalRowData *)acquireRowDataWithColumns:(int)columns;

/// Return a row data object to the pool for reuse.
/// Call this after the frame is done with the row data.
/// If pool is at capacity, the object is released instead.
/// @param rowData The row data to return (does nothing if nil)
- (void)returnRowData:(nullable iTermMetalRowData *)rowData;

/// Return multiple row data objects to the pool at once.
/// More efficient than calling returnRowData: repeatedly.
/// @param rowDataArray Array of iTermMetalRowData objects
- (void)returnRowDataArray:(NSArray<iTermMetalRowData *> *)rowDataArray;

/// Clear all pooled objects, releasing their memory.
/// In-use objects are not affected.
- (void)drain;

/// Reset pool statistics (totalAllocations, totalReuses).
- (void)resetStats;

@end

NS_ASSUME_NONNULL_END
