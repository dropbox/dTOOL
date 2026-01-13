//
//  VT100TokenPool.h
//  DashTerm2
//
//  Created by DashTerm2 Performance Audit on 2025-12-17.
//  Part of DashTerm2 Performance Optimization - Iteration 5
//
//  Token pooling eliminates 15-25% CPU overhead from token allocation.
//  See reports/main/PERFORMANCE_AUDIT_003.md for design details.
//

#import <Foundation/Foundation.h>

@class VT100Token;

NS_ASSUME_NONNULL_BEGIN

/// Thread-safe pool for VT100Token objects to reduce allocation overhead.
///
/// Under heavy parsing workloads (e.g., `yes | head -1M`), token allocation
/// becomes a significant CPU bottleneck (15-25% of CPU time). This pool
/// reuses token objects to minimize allocation and autorelease pressure.
///
/// Usage:
/// ```objc
/// VT100Token *token = [[VT100TokenPool sharedPool] acquireToken];
/// // ... use token ...
/// [[VT100TokenPool sharedPool] recycleToken:token];
/// ```
@interface VT100TokenPool : NSObject

/// Returns the shared token pool instance.
+ (instancetype)sharedPool;

/// Acquires a token from the pool.
///
/// Fast path: Returns a recycled token from the free list.
/// Slow path: Allocates a new pooled token if the free list is empty.
///
/// @return A token ready for use. The token is NOT autoreleased.
- (VT100Token *)acquireToken;

/// Returns a token to the pool for reuse.
///
/// The token is reset to its initial state before being added to the free list.
/// If the token is not pooled (created via `+[VT100Token token]`), this is a no-op.
///
/// @param token The token to recycle. May be nil.
- (void)recycleToken:(nullable VT100Token *)token;

/// Returns a new token that is NOT managed by the pool.
///
/// Use this when you need a token with a longer lifetime that shouldn't
/// be recycled (e.g., tokens stored in data structures).
///
/// @return An autoreleased token created via the traditional allocation path.
+ (VT100Token *)unpooledToken;

#pragma mark - Statistics (for debugging and profiling)

/// Current number of tokens in the free list.
@property (nonatomic, readonly) NSUInteger poolSize;

/// Number of tokens currently in use (acquired but not recycled).
@property (nonatomic, readonly) NSUInteger activeTokens;

/// Total number of tokens that have been recycled.
@property (nonatomic, readonly) NSUInteger recycledCount;

/// Total number of tokens acquired from the pool.
@property (nonatomic, readonly) NSUInteger acquiredCount;

/// Number of times a new token had to be allocated (pool was empty).
@property (nonatomic, readonly) NSUInteger poolMissCount;

/// Resets all statistics counters to zero.
- (void)resetStatistics;

/// Returns a string describing pool statistics.
- (NSString *)statisticsDescription;

/// Drains the pool, releasing all pooled tokens.
/// Call this when memory pressure is high.
- (void)drain;

/// Trims the pool to the specified size, releasing excess tokens.
/// @param size The target pool size.
- (void)trimToSize:(NSUInteger)size;

@end

NS_ASSUME_NONNULL_END
