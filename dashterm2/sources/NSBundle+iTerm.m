//
//  NSBundle+iTerm.m
//  DashTerm2
//
//  Created by George Nachman on 6/29/17.
//
//

#import "NSBundle+iTerm.h"

@implementation NSBundle (iTerm)

+ (BOOL)it_isNightlyBuild {
    static dispatch_once_t onceToken;
    static BOOL result;
    dispatch_once(&onceToken, ^{
        NSString *testingFeed = [[NSBundle mainBundle] objectForInfoDictionaryKey:@"SUFeedURLForTesting"];
        result = [testingFeed containsString:@"nightly"];
    });
    return result;
}

+ (BOOL)it_isEarlyAdopter {
    NSString *testingFeed = [[NSBundle mainBundle] objectForInfoDictionaryKey:@"SUFeedURLForTesting"];
    return [testingFeed containsString:@"testing3.xml"];
}

+ (NSDate *)it_buildDate {
    NSDateFormatter *dateFormatter = [[NSDateFormatter alloc] init];
    // BUG-972: Use en_US_POSIX for fixed format parsing per Apple QA1480
    [dateFormatter setLocale:[NSLocale localeWithLocaleIdentifier:@"en_US_POSIX"]];
    // Parse without timezone - __DATE__ and __TIME__ don't include timezone info.
    // The resulting date will be in the system's local timezone.
    [dateFormatter setDateFormat:@"LLL d yyyy HH:mm:ss"];
    NSString *string = [NSString stringWithFormat:@"%s %s", __DATE__, __TIME__];
    NSDate *result = [dateFormatter dateFromString:string];
    // BUG-2738: Return distant past if parsing fails, so callers can safely compare dates.
    // This ensures nightly build age checks work even if __DATE__ format changes.
    if (!result) {
        return [NSDate distantPast];
    }
    return result;
}

@end
