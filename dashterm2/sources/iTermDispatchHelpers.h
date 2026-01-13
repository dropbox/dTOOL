//
//  iTermDispatchHelpers.h
//  DashTerm2
//
//  Created by DashTerm2 Polishing Work on 2025-12-28.
//
//  Safe dispatch helpers to prevent common threading issues:
//  - dispatch_sync to main queue from main queue (deadlock)
//  - dispatch_sync to serial queue from same queue (deadlock)
//
//  Usage:
//    iTermDispatchSyncMain(^{
//        // Code to run on main thread
//    });
//
//  These helpers detect the current queue context and either execute
//  the block directly (if already on the target queue) or dispatch_sync.
//

#import <Foundation/Foundation.h>

NS_ASSUME_NONNULL_BEGIN

#pragma mark - Main Queue Helpers

/// Safely execute a block on the main queue.
/// If already on the main queue, executes directly (avoids deadlock).
/// If on a background queue, uses dispatch_sync.
///
/// @param block The block to execute on the main queue.
NS_INLINE void iTermDispatchSyncMain(dispatch_block_t block) {
    if ([NSThread isMainThread]) {
        block();
    } else {
        dispatch_sync(dispatch_get_main_queue(), block);
    }
}

/// Safely execute a block on the main queue, returning a value.
/// If already on the main queue, executes directly (avoids deadlock).
///
/// @param block The block to execute on the main queue.
/// @return The value returned by the block.
#define iTermDispatchSyncMainReturning(type, block) \
    ({ \
        __block type _result; \
        if ([NSThread isMainThread]) { \
            _result = (block)(); \
        } else { \
            dispatch_sync(dispatch_get_main_queue(), ^{ \
                _result = (block)(); \
            }); \
        } \
        _result; \
    })

/// Asynchronously execute a block on the main queue.
/// If already on the main queue, dispatches async to avoid re-entrancy issues.
///
/// @param block The block to execute on the main queue.
NS_INLINE void iTermDispatchAsyncMain(dispatch_block_t block) {
    dispatch_async(dispatch_get_main_queue(), block);
}

#pragma mark - Serial Queue Helpers

/// Safely execute a block on a serial queue.
/// Uses a queue-specific key to detect if already on the target queue.
/// If already on the queue, executes directly (avoids deadlock).
///
/// @param queue The target serial queue. Must have been created with iTermCreateSerialQueue.
/// @param block The block to execute on the queue.
///
/// @note The queue must have been created with iTermCreateSerialQueue() for
///       the re-entrancy check to work. Standard dispatch_queue_create queues
///       will not have the context key set.
NS_INLINE void iTermDispatchSyncSerial(dispatch_queue_t queue, dispatch_block_t block) {
    // Check if we're already on this queue by looking for our context marker
    void *context = dispatch_get_specific((__bridge const void *)queue);
    if (context == (__bridge void *)queue) {
        // Already on this queue - execute directly
        block();
    } else {
        dispatch_sync(queue, block);
    }
}

/// Create a serial queue with re-entrancy detection support.
/// Queues created with this function can be used with iTermDispatchSyncSerial()
/// for deadlock-safe synchronous dispatch.
///
/// @param label The label for the dispatch queue (for debugging).
/// @return A new serial dispatch queue with context key set.
NS_INLINE dispatch_queue_t iTermCreateSerialQueue(const char *label) {
    dispatch_queue_t queue = dispatch_queue_create(label, DISPATCH_QUEUE_SERIAL);
    // Use the queue pointer itself as both the key and context value.
    // This allows iTermDispatchSyncSerial to detect if we're already on this queue.
    dispatch_queue_set_specific(queue, (__bridge const void *)queue, (__bridge void *)queue, NULL);
    return queue;
}

#pragma mark - Background Queue Helpers

/// Execute a block on a global background queue with default priority.
///
/// @param block The block to execute on the background queue.
NS_INLINE void iTermDispatchAsyncBackground(dispatch_block_t block) {
    dispatch_async(dispatch_get_global_queue(DISPATCH_QUEUE_PRIORITY_DEFAULT, 0), block);
}

/// Execute a block on a global background queue with specified priority.
///
/// @param priority The queue priority (DISPATCH_QUEUE_PRIORITY_*).
/// @param block The block to execute on the background queue.
NS_INLINE void iTermDispatchAsyncBackgroundWithPriority(long priority, dispatch_block_t block) {
    dispatch_async(dispatch_get_global_queue(priority, 0), block);
}

#pragma mark - Delayed Dispatch

/// Execute a block on the main queue after a delay.
///
/// @param seconds The delay in seconds.
/// @param block The block to execute.
NS_INLINE void iTermDispatchAfterMain(NSTimeInterval seconds, dispatch_block_t block) {
    dispatch_after(dispatch_time(DISPATCH_TIME_NOW, (int64_t)(seconds * NSEC_PER_SEC)),
                   dispatch_get_main_queue(),
                   block);
}

/// Execute a block on a specific queue after a delay.
///
/// @param seconds The delay in seconds.
/// @param queue The target queue.
/// @param block The block to execute.
NS_INLINE void iTermDispatchAfter(NSTimeInterval seconds, dispatch_queue_t queue, dispatch_block_t block) {
    dispatch_after(dispatch_time(DISPATCH_TIME_NOW, (int64_t)(seconds * NSEC_PER_SEC)),
                   queue,
                   block);
}

#pragma mark - Debug Helpers

/// Assert that the current code is running on the main thread.
/// In debug builds, this will crash if called from a background thread.
/// In release builds, this logs a warning but does not crash.
#ifdef DEBUG
#define iTermAssertMainThread() \
    do { \
        if (![NSThread isMainThread]) { \
            NSLog(@"FATAL: Expected main thread but on %@ at %s:%d", \
                  [NSThread currentThread], __FILE__, __LINE__); \
            __builtin_trap(); \
        } \
    } while (0)
#else
#define iTermAssertMainThread() \
    do { \
        if (![NSThread isMainThread]) { \
            NSLog(@"WARNING: Expected main thread but on %@ at %s:%d", \
                  [NSThread currentThread], __FILE__, __LINE__); \
        } \
    } while (0)
#endif

/// Assert that the current code is NOT running on the main thread.
#ifdef DEBUG
#define iTermAssertBackgroundThread() \
    do { \
        if ([NSThread isMainThread]) { \
            NSLog(@"FATAL: Expected background thread but on main at %s:%d", \
                  __FILE__, __LINE__); \
            __builtin_trap(); \
        } \
    } while (0)
#else
#define iTermAssertBackgroundThread() \
    do { \
        if ([NSThread isMainThread]) { \
            NSLog(@"WARNING: Expected background thread but on main at %s:%d", \
                  __FILE__, __LINE__); \
        } \
    } while (0)
#endif

NS_ASSUME_NONNULL_END
