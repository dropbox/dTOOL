//
//  iTermCoreSearch.m
//  DashTerm2
//
//  Created by George Nachman on 10/27/25.
//

#import "iTermCoreSearch.h"
#import "DebugLogging.h"
#import "iTermCache.h"
#import "NSArray+iTerm.h"
#import "RegexKitLite.h"
#import "ScreenChar.h"

#ifdef DEBUG_SEARCH
#define SearchLog(args...) NSLog(args)
#else
#define SearchLog(args...)
#endif

const unichar kPrefixChar = REGEX_START;
const unichar kSuffixChar = REGEX_END;

@interface NSString (LineBlockDebugging)
@end

@implementation NSString (LineBlockDebugging)
- (NSString *)asciified {
    NSMutableString *c = [self mutableCopy];
    NSRange range = [c rangeOfCharacterFromSet:[NSCharacterSet characterSetWithRange:NSMakeRange(0, 32)]];
    while (range.location != NSNotFound) {
        [c replaceCharactersInRange:range withString:@"."];
        range = [c rangeOfCharacterFromSet:[NSCharacterSet characterSetWithRange:NSMakeRange(0, 32)]];
    }
    return c;
}
@end

@interface NSString (CoreSearchAdditions)
- (NSArray<NSValue *> *)it_rangesOfString:(NSString *)needle options:(NSStringCompareOptions)options;
@end

static int CoreSearchStringIndexToScreenCharIndex(NSInteger stringIndex, NSUInteger stringLength, const int *deltas) {
    if (stringIndex >= stringLength) {
        if (stringLength == 0) {
            return 0;
        }
        return deltas[stringLength - 1] + 1;
    }
    return stringIndex + deltas[stringIndex];
}

static NSArray<ResultRange *> *CoreSearchResultsFromRanges(NSArray<NSValue *> *ranges, const int *deltas,
                                                           NSUInteger stringLength) {
    if (ranges.count == 0) {
        return @[];
    }
    NSMutableArray<ResultRange *> *resultRanges = [ranges mapToMutableArrayWithBlock:^id _Nullable(NSValue *value) {
        const NSRange range = value.rangeValue;
        if (range.length == 0) {
            return nil;
        }
        const int lowerBound = CoreSearchStringIndexToScreenCharIndex(range.location, stringLength, deltas);
        const int upperBound = CoreSearchStringIndexToScreenCharIndex(NSMaxRange(range) - 1, stringLength, deltas);
        return [[ResultRange alloc] initWithPosition:lowerBound length:upperBound - lowerBound + 1];
    }];
    if (resultRanges.count < 2) {
        return resultRanges;
    }
    ResultRange *prev = resultRanges.lastObject;
    for (NSInteger i = resultRanges.count - 2; i >= 0; i--) {
        ResultRange *current = resultRanges[i];
        // If two of them share a bound, keep the longer one.
        if (current.position == prev.position || current.upperBound == prev.upperBound) {
            if (current.length > prev.length) {
                [resultRanges replaceObjectsInRange:NSMakeRange(i, 2) withObjectsFromArray:@[ current ]];
                prev = current;
            } else {
                [resultRanges replaceObjectsInRange:NSMakeRange(i, 2) withObjectsFromArray:@[ prev ]];
            }
        } else {
            prev = current;
        }
    }
    return resultRanges;
}

static NSRegularExpression *CoreSearchGetCompiledRegex(BOOL caseInsensitive, NSString *pattern, NSError **regexError) {
    static iTermCache<NSString *, NSRegularExpression *> *regexCacheCaseSensitive;
    static iTermCache<NSString *, NSRegularExpression *> *regexCacheCaseInsensitive;
    static dispatch_once_t onceToken;
    dispatch_once(&onceToken, ^{
        regexCacheCaseSensitive = [[iTermCache alloc] initWithCapacity:32];
        regexCacheCaseInsensitive = [[iTermCache alloc] initWithCapacity:32];
    });
    iTermCache<NSString *, NSRegularExpression *> *regexCache =
        caseInsensitive ? regexCacheCaseInsensitive : regexCacheCaseSensitive;
    NSRegularExpression *compiled = regexCache[pattern];
    if (regexError) {
        *regexError = nil;
    }
    if (!compiled) {
        NSRegularExpressionOptions stringCompareOptions = 0;
        if (caseInsensitive) {
            stringCompareOptions |= NSRegularExpressionCaseInsensitive;
        }
        compiled = [[NSRegularExpression alloc] initWithPattern:pattern options:stringCompareOptions error:regexError];
        if (compiled) {
            regexCache[pattern] = compiled;
        }
    }
    return compiled;
}

// BUG-3347: ReDoS protection timeout for user-provided search regex (0.5 seconds)
static const NSTimeInterval kCoreSearchRegexTimeoutSeconds = 0.5;

static NSArray<ResultRange *> *CoreRegexSearch(const CoreSearchRequest *request) {
    const BOOL caseInsensitive = (request->mode == iTermFindModeCaseInsensitiveRegex);
    NSError *regexError = nil;
    NSRegularExpression *compiled = CoreSearchGetCompiledRegex(caseInsensitive, request->needle, &regexError);
    if (regexError || !compiled) {
        VLog(@"regex error: %@", regexError);
        return @[];
    }

    // BUG-3347: Use enumerateMatchesInString with NSMatchingReportProgress
    // to allow timeout checking during regex matching. This prevents ReDoS attacks
    // from user-provided search patterns that could hang the UI.
    NSMutableArray<NSValue *> *ranges = [NSMutableArray arrayWithCapacity:16]; // Initial regex match results
    __block NSDate *startTime = [NSDate date];
    __block BOOL timedOut = NO;

    [compiled
        enumerateMatchesInString:request->haystack
                         options:NSMatchingWithTransparentBounds | NSMatchingReportProgress
                           range:NSMakeRange(0, request->haystack.length)
                      usingBlock:^(NSTextCheckingResult *_Nullable result, NSMatchingFlags flags, BOOL *_Nonnull stop) {
                          // Check timeout on progress reports
                          if (flags & NSMatchingProgress) {
                              NSTimeInterval elapsed = -[startTime timeIntervalSinceNow];
                              if (elapsed > kCoreSearchRegexTimeoutSeconds) {
                                  *stop = YES;
                                  timedOut = YES;
                                  VLog(@"CoreRegexSearch: timeout after %.2fs for pattern: %@", elapsed,
                                       request->needle);
                              }
                              return;
                          }

                          // Collect actual matches
                          if (result) {
                              [ranges addObject:[NSValue valueWithRange:result.range]];
                          }
                      }];

    if (timedOut) {
        VLog(@"CoreRegexSearch: search timed out, returning partial results (%lu matches)",
             (unsigned long)ranges.count);
    }

    if (request->options & FindOptBackwards) {
        ranges = [[ranges reversed] mutableCopy];
    }

    return CoreSearchResultsFromRanges(ranges, request->deltas, request->haystack.length);
}

static NSArray<ResultRange *> *CoreSubstringSearch(const CoreSearchRequest *request) {
    NSStringCompareOptions stringCompareOptions = 0;
    if (request->options & FindOptBackwards) {
        stringCompareOptions |= NSBackwardsSearch;
    }
    BOOL caseInsensitive = (request->mode == iTermFindModeCaseInsensitiveSubstring);
    if (request->mode == iTermFindModeSmartCaseSensitivity &&
        [request->needle rangeOfCharacterFromSet:[NSCharacterSet uppercaseLetterCharacterSet]].location == NSNotFound) {
        caseInsensitive = YES;
    }
    if (caseInsensitive) {
        stringCompareOptions |= NSCaseInsensitiveSearch | NSDiacriticInsensitiveSearch | NSWidthInsensitiveSearch;
    }
    SearchLog(@"Search %@ for %@ with options %@", request->haystack, request->needle, @(stringCompareOptions));
    NSArray<NSValue *> *ranges = [request->haystack it_rangesOfString:request->needle options:stringCompareOptions];
    return CoreSearchResultsFromRanges(ranges, request->deltas, request->haystack.length);
}

NSArray<ResultRange *> *CoreSearch(const CoreSearchRequest *request) {
    if (request->haystack.length == 0) {
        return @[];
    }
    if (request->needle.length == 0) {
        if ((request->options & FindOptEmptyQueryMatches)) {
            NSRange range = NSMakeRange(0, request->haystack.length);
            return CoreSearchResultsFromRanges(@[ [NSValue valueWithRange:range] ], request->deltas,
                                               request->haystack.length);
        } else {
            return @[];
        }
    }

    const BOOL regex =
        (request->mode == iTermFindModeCaseInsensitiveRegex || request->mode == iTermFindModeCaseSensitiveRegex);
    if (regex) {
        return CoreRegexSearch(request);
    } else {
        return CoreSubstringSearch(request);
    }
}

@implementation NSString (CoreSearchAdditions)

// GitLab #12193: Support overlapping matches.
// When searching for "XX" in "XXX", find matches at both 0-1 and 1-2.
// Previously, after finding a match, next search started at NSMaxRange(range)
// which skipped overlapping matches. Now we start at range.location + 1.
- (NSArray<NSValue *> *)it_rangesOfString:(NSString *)needle options:(NSStringCompareOptions)options {
    NSMutableArray<NSValue *> *result = [NSMutableArray arrayWithCapacity:8]; // Substring match ranges
    const NSInteger length = self.length;
    NSRange rangeToSearch = NSMakeRange(0, length);
    while (rangeToSearch.location < length) {
        const NSRange range = [self rangeOfString:needle options:options range:rangeToSearch];
        if (range.location == NSNotFound) {
            break;
        }
        [result addObject:[NSValue valueWithRange:range]];
        if (options & NSBackwardsSearch) {
            // For backwards search, the search range starts at 0 and ends at rangeToSearch.length.
            // To find overlapping matches, set the new search range to end just before
            // the last character of the current match (range.location + range.length - 1).
            // This allows finding a match that starts 1 character earlier and overlaps.
            const NSInteger newLength = range.location + range.length - 1;
            if (newLength < (NSInteger)needle.length) {
                // Not enough room for another match
                break;
            }
            rangeToSearch.length = newLength;
        } else {
            // For forwards search, advance by 1 from match start (not end)
            // to allow overlapping matches
            rangeToSearch.location = range.location + 1;
            rangeToSearch.length = length - rangeToSearch.location;
        }
    }
    return result;
}

@end
