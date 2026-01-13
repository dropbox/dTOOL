//
//  iTermMetalPerFrameStateRowPool.h
//  DashTerm2SharedARC
//
//  Created by AI Worker on 12/17/25.
//
//  Object pool for iTermMetalPerFrameStateRow to reduce per-frame allocations.
//  In Metal rendering, metalDriverWillBeginDrawingFrame creates row state objects
//  for every visible row every frame. This pool recycles these objects to avoid
//  allocation overhead, similar to iTermMetalRowDataPool but for the per-frame state.
//

#import <Foundation/Foundation.h>

@class iTermMetalPerFrameStateRow;

NS_ASSUME_NONNULL_BEGIN

/// Pool for reusing per-frame state row objects across frames.
/// Thread-safety: Must be accessed only from the main thread (where metalDriverWillBeginDrawingFrame runs).
@interface iTermMetalPerFrameStateRowPool : NSObject

/// Returns a shared pool instance. The pool is lazily created on first access.
+ (instancetype)sharedPool;

/// Acquires a row object from the pool, or returns nil if the pool is empty.
/// When nil is returned, the caller should allocate a new row via the normal initializer.
/// @return A row object ready for reuse, or nil if the pool is empty.
- (nullable iTermMetalPerFrameStateRow *)acquireRow;

/// Returns a row object to the pool for reuse.
/// @param row The row object to return. Must not be nil.
- (void)returnRow:(iTermMetalPerFrameStateRow *)row;

/// Returns multiple row objects to the pool.
/// @param rows Array of row objects to return.
- (void)returnRows:(NSArray<iTermMetalPerFrameStateRow *> *)rows;

/// Current number of pooled objects available for reuse.
@property (nonatomic, readonly) NSUInteger pooledCount;

/// Total number of acquisitions from this pool (for diagnostics).
@property (nonatomic, readonly) NSUInteger totalAcquisitions;

/// Total number of objects returned to this pool (for diagnostics).
@property (nonatomic, readonly) NSUInteger totalReturns;

/// Clears all pooled objects. Use when memory pressure is high.
- (void)drain;

@end

NS_ASSUME_NONNULL_END
