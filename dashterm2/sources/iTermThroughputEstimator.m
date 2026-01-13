//
//  iTermThroughputEstimator.m
//  DashTerm2
//
//  Created by George Nachman on 4/26/16.
//
//

#import "iTermThroughputEstimator.h"
#import "DebugLogging.h"
#import "iTermMalloc.h"
#import <os/lock.h>

@implementation iTermThroughputEstimator {
    // Performance optimization: Use a C array of NSInteger instead of NSMutableArray<NSNumber *>
    // to avoid boxing/unboxing overhead on every addByteCount: call. This is called frequently
    // during data transfer and the NSNumber allocations were adding measurable overhead.
    NSInteger *_buckets;
    NSUInteger _numberOfBuckets;

    // Current write position in circular buffer (points to most recent bucket)
    NSUInteger _currentIndex;

    // Values of arguments of initWithHistoryOfDuration:secondsPerBucket:.
    NSTimeInterval _historyDuration;
    NSTimeInterval _secondsPerBucket;

    // Time this object was created.
    NSTimeInterval _startTime;

    // The "time index" for the last bucket. The time index is the number of
    // seconds since `_startTime` divided by `_secondsPerBucket`. It equals the
    // number of buckets elapsed since object creation.
    NSInteger _lastTimeIndex;

    // Performance optimization: os_unfair_lock has ~10x lower overhead than @synchronized
    os_unfair_lock _lock;
}

- (instancetype)initWithHistoryOfDuration:(NSTimeInterval)historyDuration
                         secondsPerBucket:(NSTimeInterval)secondsPerBucket {
    self = [super init];
    if (self) {
        _historyDuration = historyDuration;
        _secondsPerBucket = secondsPerBucket;
        _startTime = [NSDate timeIntervalSinceReferenceDate];
        _numberOfBuckets = (NSUInteger)MAX(1, round(historyDuration / secondsPerBucket));
        _buckets = (NSInteger *)iTermCalloc(_numberOfBuckets, sizeof(NSInteger));
        _currentIndex = _numberOfBuckets - 1;  // Last bucket is most recent
        _lock = OS_UNFAIR_LOCK_INIT;
    }
    return self;
}

- (void)dealloc {
    free(_buckets);
    [super dealloc];
}

- (NSInteger)estimatedThroughput {
    os_unfair_lock_lock(&_lock);
    const double delta = [self eraseBucketsIfNeededLocked];
    const double timeSpentInCurrentBucket = fmod(delta, _secondsPerBucket);
    // We want to weight the current bucket in proportion to how much time it
    // has left so as not to under-count it, but to keep the variance from
    // getting out of control there's a cap on this weight.
    const double weightForCurrentBucket = MIN(10, _secondsPerBucket / timeSpentInCurrentBucket);

    double weightedSum = 0;
    double weight = 1;
    // Iterate from oldest to newest bucket
    for (NSUInteger i = 0; i < _numberOfBuckets; i++) {
        // Calculate index: oldest bucket is at (_currentIndex + 1) % _numberOfBuckets
        NSUInteger bucketIndex = (_currentIndex + 1 + i) % _numberOfBuckets;
        double value = (double)_buckets[bucketIndex];
        if (i == _numberOfBuckets - 1) {
            // This is the most recent bucket
            value *= weightForCurrentBucket;
        }
        weightedSum += value * weight;
        weight *= 2;
    }
    double averageValuePerBucket = weightedSum / (weight - 1.0);
    double estimatedThroughput = averageValuePerBucket / _secondsPerBucket;
    os_unfair_lock_unlock(&_lock);
    return (NSInteger)estimatedThroughput;
}

- (void)addByteCount:(NSInteger)count {
    os_unfair_lock_lock(&_lock);
    [self eraseBucketsIfNeededLocked];
    _buckets[_currentIndex] += count;
    os_unfair_lock_unlock(&_lock);
}

// Returns the amount of time since _startTime.
// Must be called with _lock held.
- (double)eraseBucketsIfNeededLocked {
    const NSTimeInterval now = [NSDate timeIntervalSinceReferenceDate];
    const NSTimeInterval delta = now - _startTime;
    const NSInteger timeIndex = (NSInteger)floor(delta / _secondsPerBucket);
    NSInteger bucketsToAdvance = timeIndex - _lastTimeIndex;
    _lastTimeIndex = timeIndex;

    // Advance the circular buffer and zero out old buckets
    if (bucketsToAdvance > 0) {
        NSInteger bucketsToZero = MIN(bucketsToAdvance, (NSInteger)_numberOfBuckets);
        for (NSInteger i = 0; i < bucketsToZero; i++) {
            _currentIndex = (_currentIndex + 1) % _numberOfBuckets;
            _buckets[_currentIndex] = 0;
        }
    }
    return delta;
}

@end
