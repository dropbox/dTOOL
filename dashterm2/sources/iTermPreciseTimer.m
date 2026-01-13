//
//  iTermPreciseTimer.m
//  DashTerm2
//
//  Created by George Nachman on 7/13/16.
//
//

#import "iTermPreciseTimer.h"

#import "DebugLogging.h"
#import "iTermHistogram.h"
#import "iTermMalloc.h"
#import "NSStringITerm.h"
#include <assert.h>
#include <CoreServices/CoreServices.h>
#include <mach/mach.h>
#include <mach/mach_time.h>
#include <unistd.h>
#include <os/lock.h>

#if ENABLE_PRECISE_TIMERS
static BOOL gPreciseTimersEnabled;
static NSMutableDictionary *sLogs;

// Performance optimization: os_unfair_lock has ~10x lower overhead than @synchronized.
// This is important for precise timers since the lock acquisition time adds to timing measurements.
static os_unfair_lock sPreciseTimerLock = OS_UNFAIR_LOCK_INIT;

// Forward declaration for internal use - caller must hold sPreciseTimerLock
static NSTimeInterval iTermPreciseTimerMeasure_locked(iTermPreciseTimer *timer);
static void iTermPreciseTimerReset_locked(iTermPreciseTimer *timer);

@implementation iTermPreciseTimersLock
@end

void iTermPreciseTimerSetEnabled(BOOL enabled) {
    os_unfair_lock_lock(&sPreciseTimerLock);
    gPreciseTimersEnabled = enabled;
    os_unfair_lock_unlock(&sPreciseTimerLock);
}

void iTermPreciseTimerLock(void) {
    os_unfair_lock_lock(&sPreciseTimerLock);
}

void iTermPreciseTimerUnlock(void) {
    os_unfair_lock_unlock(&sPreciseTimerLock);
}

void iTermPreciseTimerStart(iTermPreciseTimer *timer) {
    os_unfair_lock_lock(&sPreciseTimerLock);
    if (!gPreciseTimersEnabled) {
        os_unfair_lock_unlock(&sPreciseTimerLock);
        return;
    }
    timer->start = mach_absolute_time();
    os_unfair_lock_unlock(&sPreciseTimerLock);
}

NSTimeInterval iTermPreciseTimerMeasureAndAccumulate(iTermPreciseTimer *timer) {
    os_unfair_lock_lock(&sPreciseTimerLock);
    if (!gPreciseTimersEnabled) {
        os_unfair_lock_unlock(&sPreciseTimerLock);
        return 0;
    }
    timer->total += iTermPreciseTimerMeasure_locked(timer);
    timer->eventCount += 1;
    NSTimeInterval result = timer->total;
    os_unfair_lock_unlock(&sPreciseTimerLock);
    return result;
}

NSTimeInterval iTermPreciseTimerAccumulate(iTermPreciseTimer *timer, NSTimeInterval value) {
    os_unfair_lock_lock(&sPreciseTimerLock);
    if (!gPreciseTimersEnabled) {
        os_unfair_lock_unlock(&sPreciseTimerLock);
        return 0;
    }
    NSTimeInterval result = timer->total;
    os_unfair_lock_unlock(&sPreciseTimerLock);
    return result;
}

void iTermPreciseTimerReset(iTermPreciseTimer *timer) {
    os_unfair_lock_lock(&sPreciseTimerLock);
    iTermPreciseTimerReset_locked(timer);
    os_unfair_lock_unlock(&sPreciseTimerLock);
}

// Internal version - caller must hold sPreciseTimerLock
static void iTermPreciseTimerReset_locked(iTermPreciseTimer *timer) {
    if (!gPreciseTimersEnabled) {
        return;
    }
    timer->total = 0;
    timer->eventCount = 0;
}

NSTimeInterval iTermPreciseTimerMeasure(iTermPreciseTimer *timer) {
    os_unfair_lock_lock(&sPreciseTimerLock);
    NSTimeInterval result = iTermPreciseTimerMeasure_locked(timer);
    os_unfair_lock_unlock(&sPreciseTimerLock);
    return result;
}

// Internal version - caller must hold sPreciseTimerLock
static NSTimeInterval iTermPreciseTimerMeasure_locked(iTermPreciseTimer *timer) {
    if (!gPreciseTimersEnabled) {
        return 0;
    }
    uint64_t end;
    NSTimeInterval elapsed;

    end = mach_absolute_time();
    elapsed = end - timer->start;

    static mach_timebase_info_data_t sTimebaseInfo;
    if (sTimebaseInfo.denom == 0) {
        mach_timebase_info(&sTimebaseInfo);
    }

    double nanoseconds = elapsed * sTimebaseInfo.numer / sTimebaseInfo.denom;
    return nanoseconds / 1000000000.0;
}

// Internal implementation - caller must hold sPreciseTimerLock
static void iTermPreciseTimerStatsInit_impl(iTermPreciseTimerStats *stats, const char *name) {
    if (!gPreciseTimersEnabled) {
        return;
    }
    stats->n = 0;
    stats->totalEventCount = 0;
    stats->mean = 0;
    stats->m2 = 0;
    stats->min = INFINITY;
    stats->max = -INFINITY;
    stats->level = 0;
    iTermPreciseTimerReset_locked(&stats->timer);
    if (name) {
        strlcpy(stats->name, name, sizeof(stats->name));
        const int len = strlen(stats->name);
        for (int i = len - 1; i >= 0; i--) {
            if (name[i] == '<') {
                stats->level++;
                stats->name[i] = '\0';
            } else {
                break;
            }
        }
    }
}

void iTermPreciseTimerStatsInit(iTermPreciseTimerStats *stats, const char *name) {
    os_unfair_lock_lock(&sPreciseTimerLock);
    iTermPreciseTimerStatsInit_impl(stats, name);
    os_unfair_lock_unlock(&sPreciseTimerLock);
}

// "_locked" version - caller must already hold sPreciseTimerLock
void iTermPreciseTimerStatsInit_locked(iTermPreciseTimerStats *stats, const char *name) {
    iTermPreciseTimerStatsInit_impl(stats, name);
}

NSInteger iTermPreciseTimerStatsGetCount(iTermPreciseTimerStats *stats) {
    os_unfair_lock_lock(&sPreciseTimerLock);
    if (!gPreciseTimersEnabled) {
        os_unfair_lock_unlock(&sPreciseTimerLock);
        return 0;
    }
    NSInteger result = stats->n;
    os_unfair_lock_unlock(&sPreciseTimerLock);
    return result;
}

void iTermPreciseTimerStatsStartTimer(iTermPreciseTimerStats *stats) {
    os_unfair_lock_lock(&sPreciseTimerLock);
    if (!gPreciseTimersEnabled) {
        os_unfair_lock_unlock(&sPreciseTimerLock);
        return;
    }
    stats->timer.start = mach_absolute_time();
    os_unfair_lock_unlock(&sPreciseTimerLock);
}

// Forward declaration for internal use - caller must hold sPreciseTimerLock
static void iTermPreciseTimerStatsRecord_locked(iTermPreciseTimerStats *stats, NSTimeInterval value, int eventCount);

double iTermPreciseTimerStatsMeasureAndRecordTimer(iTermPreciseTimerStats *stats) {
    os_unfair_lock_lock(&sPreciseTimerLock);
    if (!gPreciseTimersEnabled) {
        os_unfair_lock_unlock(&sPreciseTimerLock);
        return 0;
    }
    if (stats->timer.start) {
        stats->timer.total += iTermPreciseTimerMeasure_locked(&stats->timer);
        stats->timer.eventCount += 1;
        NSTimeInterval total = stats->timer.total;
        int eventCount = stats->timer.eventCount;
        iTermPreciseTimerStatsRecord_locked(stats, total, eventCount);
        iTermPreciseTimerReset_locked(&stats->timer);
        os_unfair_lock_unlock(&sPreciseTimerLock);
        return total;
    } else {
        os_unfair_lock_unlock(&sPreciseTimerLock);
        return 0;
    }
}

// Internal implementation - caller must hold sPreciseTimerLock
static void iTermPreciseTimerStatsRecordTimer_impl(iTermPreciseTimerStats *stats) {
    if (!gPreciseTimersEnabled) {
        return;
    }
    iTermPreciseTimerStatsRecord_locked(stats, stats->timer.total, stats->timer.eventCount);
    iTermPreciseTimerReset_locked(&stats->timer);
}

void iTermPreciseTimerStatsRecordTimer(iTermPreciseTimerStats *stats) {
    os_unfair_lock_lock(&sPreciseTimerLock);
    iTermPreciseTimerStatsRecordTimer_impl(stats);
    os_unfair_lock_unlock(&sPreciseTimerLock);
}

// "_locked" version - caller must already hold sPreciseTimerLock
void iTermPreciseTimerStatsRecordTimer_locked(iTermPreciseTimerStats *stats) {
    iTermPreciseTimerStatsRecordTimer_impl(stats);
}

void iTermPreciseTimerStatsMeasureAndAccumulate(iTermPreciseTimerStats *stats) {
    os_unfair_lock_lock(&sPreciseTimerLock);
    if (!gPreciseTimersEnabled) {
        os_unfair_lock_unlock(&sPreciseTimerLock);
        return;
    }
    stats->timer.total += iTermPreciseTimerMeasure_locked(&stats->timer);
    stats->timer.eventCount += 1;
    os_unfair_lock_unlock(&sPreciseTimerLock);
}

void iTermPreciseTimerStatsAccumulate(iTermPreciseTimerStats *stats, NSTimeInterval value) {
    os_unfair_lock_lock(&sPreciseTimerLock);
    if (!gPreciseTimersEnabled) {
        os_unfair_lock_unlock(&sPreciseTimerLock);
        return;
    }
    // Note: iTermPreciseTimerAccumulate just returns timer->total, doesn't modify it
    os_unfair_lock_unlock(&sPreciseTimerLock);
}

void iTermPreciseTimerStatsRecord(iTermPreciseTimerStats *stats, NSTimeInterval value, int eventCount) {
    os_unfair_lock_lock(&sPreciseTimerLock);
    iTermPreciseTimerStatsRecord_locked(stats, value, eventCount);
    os_unfair_lock_unlock(&sPreciseTimerLock);
}

// Internal version - caller must hold sPreciseTimerLock
static void iTermPreciseTimerStatsRecord_locked(iTermPreciseTimerStats *stats, NSTimeInterval value, int eventCount) {
    if (!gPreciseTimersEnabled) {
        return;
    }
    stats->totalEventCount += eventCount;

    // Welford's online variance algorithm, adopted from:
    // https://en.wikipedia.org/wiki/Algorithms_for_calculating_variance#Higher-order_statistics
    stats->n += 1;
    double delta = value - stats->mean;
    stats->mean += delta / stats->n;
    stats->m2 += delta * (value - stats->mean);
    stats->min = MIN(stats->min, value);
    stats->max = MAX(stats->max, value);
}

NSTimeInterval iTermPreciseTimerStatsGetMean(iTermPreciseTimerStats *stats) {
    os_unfair_lock_lock(&sPreciseTimerLock);
    if (!gPreciseTimersEnabled) {
        os_unfair_lock_unlock(&sPreciseTimerLock);
        return 0;
    }
    NSTimeInterval result = stats->mean;
    os_unfair_lock_unlock(&sPreciseTimerLock);
    return result;
}

NSTimeInterval iTermPreciseTimerStatsGetStddev(iTermPreciseTimerStats *stats) {
    os_unfair_lock_lock(&sPreciseTimerLock);
    if (!gPreciseTimersEnabled) {
        os_unfair_lock_unlock(&sPreciseTimerLock);
        return 0;
    }
    NSTimeInterval result;
    if (stats->n < 2) {
        result = NAN;
    } else {
        result = sqrt(stats->m2 / (stats->n - 1));
    }
    os_unfair_lock_unlock(&sPreciseTimerLock);
    return result;
}

iTermPreciseTimerStats *iTermPreciseTimerStatsCopy(const iTermPreciseTimerStats *source) {
    iTermPreciseTimerStats *copy = iTermMalloc(sizeof(*source));
    os_unfair_lock_lock(&sPreciseTimerLock);
    memmove(copy, source, sizeof(*source));
    os_unfair_lock_unlock(&sPreciseTimerLock);
    return copy;
}

void iTermPreciseTimerPeriodicLog(NSString *identifier, iTermPreciseTimerStats stats[], size_t count,
                                  NSTimeInterval interval, BOOL logToConsole, NSArray *histograms,
                                  NSString *additional) {
    os_unfair_lock_lock(&sPreciseTimerLock);
    if (!gPreciseTimersEnabled) {
        os_unfair_lock_unlock(&sPreciseTimerLock);
        return;
    }
    static iTermPreciseTimer gLastLog;
    if (!gLastLog.start) {
        gLastLog.start = mach_absolute_time();
    }

    if (iTermPreciseTimerMeasure_locked(&gLastLog) >= interval) {
        // Must unlock before calling iTermPreciseTimerLog since it takes its own lock
        os_unfair_lock_unlock(&sPreciseTimerLock);
        iTermPreciseTimerLog(identifier, stats, count, logToConsole, histograms, additional);
        os_unfair_lock_lock(&sPreciseTimerLock);
        gLastLog.start = mach_absolute_time();
    }
    os_unfair_lock_unlock(&sPreciseTimerLock);
}

// Internal implementation - caller must hold sPreciseTimerLock
static NSString *iTermPreciseTimerLogString_impl(NSString *identifier, iTermPreciseTimerStats stats[], size_t count,
                                                  NSArray *histograms, BOOL reset) {
    const int millisWidth = 7;
    NSString * (^formatMillis)(double) = ^NSString *(double ms) {
        NSString *numeric = [NSString stringWithFormat:@"%0.1fms", ms];
        return [[@" " stringRepeatedTimes:millisWidth - numeric.length] stringByAppendingString:numeric];
    };
    NSMutableString *log =
        [[[NSString stringWithFormat:@"-- Precise Timers for %@ --\n", identifier] mutableCopy] autorelease];
    int maxlevel = 0;
    for (size_t i = 0; i < count; i++) {
        maxlevel = MAX(maxlevel, stats[i].level);
    }
    if (histograms) {
        [log appendFormat:@"%-20s%@    %@µ      N  %@p50  %@p75  %@p95 [min  distribution  max]\n", "Statistic",
                          [@"    " stringRepeatedTimes:maxlevel],      // corresponds to dead space for indentation
                          [@" " stringRepeatedTimes:millisWidth - 1],  // average
                          [@" " stringRepeatedTimes:millisWidth - 3],  // p50
                          [@" " stringRepeatedTimes:millisWidth - 3],  // p75
                          [@" " stringRepeatedTimes:millisWidth - 3]]; // p95
        [log appendFormat:@"%@%@    %@-  -----  %@---  %@---  %@--- ------------------------\n",
                          [@"-" stringRepeatedTimes:20],
                          [@"----" stringRepeatedTimes:maxlevel],      // corresponds to dead space for indentation
                          [@"-" stringRepeatedTimes:millisWidth - 1],  // average
                          [@"-" stringRepeatedTimes:millisWidth - 3],  // p50
                          [@"-" stringRepeatedTimes:millisWidth - 3],  // p75
                          [@"-" stringRepeatedTimes:millisWidth - 3]]; // p95
    }
    for (size_t i = 0; i < count; i++) {
        if (histograms && [histograms[i] count] == 0) {
            continue;
        }
        // Read stats values while holding lock
        NSTimeInterval mean = stats[i].mean * 1000.0;
        if (histograms) {
            double p75 = [histograms[i] valueAtNTile:0.75];
            [log appendFormat:@"%@%@ %-20s%@ %@  %5d  %@  %@  %@ [%@]\n", [@"|   " stringRepeatedTimes:stats[i].level],
                              iTermEmojiForDuration(p75), stats[i].name,
                              [@"    " stringRepeatedTimes:maxlevel - stats[i].level], formatMillis(mean),
                              (int)[histograms[i] count], formatMillis([histograms[i] valueAtNTile:0.5]),
                              formatMillis(p75), formatMillis([histograms[i] valueAtNTile:0.95]),
                              [histograms[i] sparklineGraphWithPrecision:2 multiplier:1 units:@"ms"]];
        } else {
            NSTimeInterval stddev;
            if (stats[i].n < 2) {
                stddev = NAN;
            } else {
                stddev = sqrt(stats[i].m2 / (stats[i].n - 1)) * 1000.0;
            }
            [log appendFormat:
                     @"%@ %20s: µ=%0.3fms σ=%.03fms (95%% CI ≅ %0.3fms–%0.3fms) 𝚺=%.2fms N=%@ avg. events=%01.f\n",
                     iTermEmojiForDuration(mean), stats[i].name, mean, stddev, MAX(0, mean - stddev), mean + stddev,
                     stats[i].n * mean, @(stats[i].n), (double)stats[i].totalEventCount / (double)stats[i].n];
        }

        if (reset) {
            // Reset inline to avoid unlocking/relocking
            stats[i].n = 0;
            stats[i].totalEventCount = 0;
            stats[i].mean = 0;
            stats[i].m2 = 0;
            stats[i].min = INFINITY;
            stats[i].max = -INFINITY;
            iTermPreciseTimerReset_locked(&stats[i].timer);
        }
    }
    return log;
}

NSString *iTermPreciseTimerLogString(NSString *identifier, iTermPreciseTimerStats stats[], size_t count,
                                     NSArray *histograms, BOOL reset) {
    os_unfair_lock_lock(&sPreciseTimerLock);
    NSString *log = iTermPreciseTimerLogString_impl(identifier, stats, count, histograms, reset);
    os_unfair_lock_unlock(&sPreciseTimerLock);
    return log;
}

// "_locked" version - caller must already hold sPreciseTimerLock
NSString *iTermPreciseTimerLogString_locked(NSString *identifier, iTermPreciseTimerStats stats[], size_t count,
                                            NSArray *histograms, BOOL reset) {
    return iTermPreciseTimerLogString_impl(identifier, stats, count, histograms, reset);
}

void iTermPreciseTimerLog(NSString *identifier, iTermPreciseTimerStats stats[], size_t count, BOOL logToConsole,
                          NSArray *histograms, NSString *additional) {
    NSString *log = iTermPreciseTimerLogString(identifier, stats, count, histograms, YES);
    if (additional) {
        log = [log stringByAppendingFormat:@"\n%@", additional];
    }
    if (logToConsole) {
        NSLog(@"%@", log);
    }
    iTermPreciseTimerSaveLog(identifier, log);
    DLog(@"%@", log);
}

void iTermPreciseTimerLogOneEvent(NSString *identifier, iTermPreciseTimerStats stats[], size_t count, BOOL logToConsole,
                                  NSArray *histograms) {
    os_unfair_lock_lock(&sPreciseTimerLock);
    NSMutableString *log = [[@"-- Precise Timers (One Event) --\n" mutableCopy] autorelease];
    for (size_t i = 0; i < count; i++) {
        if (stats[i].n == 0) {
            continue;
        }
        const char *cname = stats[i].name;
        int length = strlen(cname);
        // Name + possible indentation (4 spaces per '<')
        NSMutableString *name = [NSMutableString stringWithCapacity:length + 40];
        while (length > 1 && cname[length - 1] == '<') {
            length--;
            [name appendString:@"    "];
        }
        // Read stats values while holding lock
        NSTimeInterval ms = stats[i].n * stats[i].mean * 1000.0;
        NSString *emoji = iTermEmojiForDuration(ms);
        [name appendString:[[[NSString alloc] initWithBytes:cname length:length
                                                   encoding:NSUTF8StringEncoding] autorelease]];
        NSString *other = @"";
        if (stats[i].n > 1) {
            NSInteger n = stats[i].n;
            double mean = stats[i].mean;
            if (histograms) {
                other = [NSString stringWithFormat:@"N=%@ µ=%0.1fms p50=%@ p95=%@ | %@", @(n), mean * 1000,
                                                   @([histograms[i] valueAtNTile:0.5]),
                                                   @([histograms[i] valueAtNTile:0.95]), [histograms[i] sparklines]];
            } else {
                other = [NSString stringWithFormat:@"N=%@ µ=%0.1fms [%0.1fms…%0.1fms]", @(n), mean * 1000,
                                                   stats[i].min * 1000, stats[i].max * 1000];
            }
        }
        [log appendFormat:@"%@ %0.1fms %@ %@\n", emoji, ms, name, other];
    }
    os_unfair_lock_unlock(&sPreciseTimerLock);
    if (logToConsole) {
        NSLog(@"%@", log);
    }
    iTermPreciseTimerSaveLog(identifier, log);

    DLog(@"%@", log);
}

void iTermPreciseTimerSaveLog(NSString *identifier, NSString *log) {
    os_unfair_lock_lock(&sPreciseTimerLock);
    if (!sLogs) {
        sLogs = [[NSMutableDictionary alloc] initWithCapacity:16]; // Timer logs
    }
    sLogs[identifier] = log;
    os_unfair_lock_unlock(&sPreciseTimerLock);
}

NSString *iTermPreciseTimerGetSavedLogs(void) {
    os_unfair_lock_lock(&sPreciseTimerLock);
    // Estimate ~2KB per log entry
    NSMutableString *result = [NSMutableString stringWithCapacity:sLogs.count * 2048];
    [sLogs enumerateKeysAndObjectsUsingBlock:^(id _Nonnull key, id _Nonnull obj, BOOL *_Nonnull stop) {
        NSInteger numLines = [[obj componentsSeparatedByString:@"\n"] count];
        [result appendFormat:@"Precise timers %@:%@%@\n", key, numLines > 1 ? @"\n" : @"", obj];
    }];
    os_unfair_lock_unlock(&sPreciseTimerLock);
    return result;
}

void iTermPreciseTimerClearLogs(void) {
    os_unfair_lock_lock(&sPreciseTimerLock);
    [sLogs removeAllObjects];
    os_unfair_lock_unlock(&sPreciseTimerLock);
}

#endif

NSString *iTermEmojiForDuration(double ms) {
    if (ms > 100) {
        return @"😱";
    } else if (ms > 10) {
        return @"😳";
    } else if (ms > 5) {
        return @"😢";
    } else if (ms > 1) {
        return @"🙁";
    } else if (ms > 0.5) {
        return @"🤔";
    } else {
        return @"  ";
    }
}
