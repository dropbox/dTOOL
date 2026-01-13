//
//  iTermWebSocketCookieJar.m
//  DashTerm2
//
//  Created by George Nachman on 4/18/18.
//

#import "iTermWebSocketCookieJar.h"
#import <os/lock.h>

@implementation iTermWebSocketCookieJar {
    NSMutableSet<NSString *> *_cookies;
    os_unfair_lock _lock;
}

+ (instancetype)sharedInstance {
    static id instance;
    static dispatch_once_t onceToken;
    dispatch_once(&onceToken, ^{
        instance = [[self alloc] init];
    });
    return instance;
}

- (instancetype)init {
    self = [super init];
    if (self) {
        // Typically a few active cookies at a time
        _cookies = [NSMutableSet setWithCapacity:8];
        _lock = OS_UNFAIR_LOCK_INIT;
    }
    return self;
}

- (BOOL)consumeCookie:(NSString *)cookie {
    os_unfair_lock_lock(&_lock);
    if ([_cookies containsObject:cookie]) {
        if (![cookie hasSuffix:@"_"]) {
            [_cookies removeObject:cookie];
        }
        os_unfair_lock_unlock(&_lock);
        return YES;
    } else {
        os_unfair_lock_unlock(&_lock);
        return NO;
    }
}

- (NSString *)randomString {
    FILE *fp = fopen("/dev/random", "r");

    if (!fp) {
        return nil;
    }

    const int length = 16;
    // Each byte becomes 2 hex chars
    NSMutableString *cookie = [NSMutableString stringWithCapacity:length * 2];
    for (int i = 0; i < length; i++) {
        int b = fgetc(fp);
        if (b == EOF) {
            fclose(fp);
            return nil;
        }
        [cookie appendFormat:@"%02x", b];
    }
    fclose(fp);
    return cookie;
}

- (void)addCookie:(NSString *)cookie {
    os_unfair_lock_lock(&_lock);
    [_cookies addObject:cookie];
    os_unfair_lock_unlock(&_lock);
}

- (NSString *)randomStringForCookie {
    NSString *cookie = [self randomString];
    if (cookie) {
        [self addCookie:cookie];
    }
    return cookie;
}

- (void)removeCookie:(NSString *)cookie {
    os_unfair_lock_lock(&_lock);
    [_cookies removeObject:cookie];
    os_unfair_lock_unlock(&_lock);
}

@end
