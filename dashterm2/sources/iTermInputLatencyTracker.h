//
//  iTermInputLatencyTracker.h
//  DashTerm2SharedARC
//
//  Created by DashTerm2 Worker on 12/19/24.
//
//  Tracks input latency from keypress to frame presentation.

#import <Foundation/Foundation.h>

NS_ASSUME_NONNULL_BEGIN

/// Tracks and measures input latency from keypress to screen update.
/// This is a singleton that coordinates between PTYTextView (input) and
/// iTermMetalDriver (output).
@interface iTermInputLatencyTracker : NSObject

/// Shared singleton instance.
+ (instancetype)sharedInstance;

/// Call this when a key is pressed. Records the timestamp for latency measurement.
/// @param timestamp The timestamp from the NSEvent (event.timestamp) or 0 to use current time.
- (void)recordKeypressWithTimestamp:(NSTimeInterval)timestamp;

/// Call this when a frame is presented (GPU completion).
/// Updates the moving average latency if there's a pending keypress.
- (void)recordFramePresented;

/// Returns the current smoothed input latency in milliseconds.
/// Returns 0 if no measurements have been taken.
@property (nonatomic, readonly) double latencyMilliseconds;

/// Returns a formatted string for display (e.g., "12ms input")
@property (nonatomic, readonly) NSString *latencyDisplayString;

/// Returns YES if there's valid latency data to display.
@property (nonatomic, readonly) BOOL hasValidData;

/// Reset all measurements (useful for testing).
- (void)reset;

@end

NS_ASSUME_NONNULL_END
