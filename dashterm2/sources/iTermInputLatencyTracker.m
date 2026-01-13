//
//  iTermInputLatencyTracker.m
//  DashTerm2
//
//  Created by DashTerm2 Worker on 12/19/24.
//
//  Tracks input latency from keypress to frame presentation.

#import "iTermInputLatencyTracker.h"
#import "MovingAverage.h"
#import <mach/mach_time.h>

@implementation iTermInputLatencyTracker {
    // Mach absolute time of the most recent keypress awaiting frame presentation
    uint64_t _pendingKeypressMachTime;

    // Moving average of latency measurements (in seconds)
    MovingAverage *_latencyMovingAverage;

    // Conversion factor from mach_absolute_time to nanoseconds
    mach_timebase_info_data_t _timebaseInfo;

    // Thread safety
    dispatch_queue_t _queue;

    // Count of measurements for validity check
    NSInteger _measurementCount;
}

+ (instancetype)sharedInstance {
    static iTermInputLatencyTracker *instance;
    static dispatch_once_t onceToken;
    dispatch_once(&onceToken, ^{
        instance = [[iTermInputLatencyTracker alloc] init];
    });
    return instance;
}

- (instancetype)init {
    self = [super init];
    if (self) {
        _latencyMovingAverage = [[MovingAverage alloc] init];
        // Use alpha of 0.25 for smoother display (slower to respond to changes)
        // Lower alpha = more smoothing (old values weighted more heavily)
        _latencyMovingAverage.alpha = 0.25;

        _queue = dispatch_queue_create("com.dashterm2.inputLatencyTracker", DISPATCH_QUEUE_SERIAL);

        // Get the timebase info for converting mach_absolute_time to real time
        mach_timebase_info(&_timebaseInfo);

        _pendingKeypressMachTime = 0;
        _measurementCount = 0;
    }
    return self;
}

- (void)recordKeypressWithTimestamp:(NSTimeInterval)timestamp {
    dispatch_async(_queue, ^{
        // Always use mach_absolute_time for maximum precision
        // The NSEvent timestamp is relative to system boot and not as precise
        self->_pendingKeypressMachTime = mach_absolute_time();
    });
}

- (void)recordFramePresented {
    dispatch_async(_queue, ^{
        if (self->_pendingKeypressMachTime == 0) {
            // No pending keypress to measure
            return;
        }

        uint64_t now = mach_absolute_time();
        uint64_t elapsed = now - self->_pendingKeypressMachTime;

        // Convert to seconds using timebase
        // elapsed * numer / denom gives nanoseconds
        double elapsedNanoseconds = (double)elapsed * (double)self->_timebaseInfo.numer / (double)self->_timebaseInfo.denom;
        double elapsedSeconds = elapsedNanoseconds / 1e9;

        // Sanity check: ignore if > 1 second (probably stale keypress)
        if (elapsedSeconds < 1.0) {
            [self->_latencyMovingAverage addValue:elapsedSeconds];
            self->_measurementCount++;
        }

        // Clear the pending keypress
        self->_pendingKeypressMachTime = 0;
    });
}

- (double)latencyMilliseconds {
    __block double result;
    dispatch_sync(_queue, ^{
        result = [self->_latencyMovingAverage value] * 1000.0;
    });
    return result;
}

- (NSString *)latencyDisplayString {
    double latency = self.latencyMilliseconds;
    if (latency < 1.0) {
        return @"<1ms input";
    }
    return [NSString stringWithFormat:@"%.0fms input", latency];
}

- (BOOL)hasValidData {
    __block BOOL result;
    dispatch_sync(_queue, ^{
        // Need at least 3 measurements for stable data
        result = self->_measurementCount >= 3;
    });
    return result;
}

- (void)reset {
    dispatch_async(_queue, ^{
        [self->_latencyMovingAverage reset];
        self->_pendingKeypressMachTime = 0;
        self->_measurementCount = 0;
    });
}

@end
