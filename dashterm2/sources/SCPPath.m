//
//  SCPPath.m
//  iTerm
//
//  Created by George Nachman on 12/21/13.
//
//

#import "SCPPath.h"

@implementation SCPPath

// BUG-1452: Remove MRC code - the main target uses ARC (CLANG_ENABLE_OBJC_ARC = YES).
// ARC handles memory management automatically.

- (NSString *)description {
    return [NSString stringWithFormat:@"<%@: %p hostname=%@ username=%@ path=%@>",
            self.class, self, self.hostname, self.username, self.path];
}

- (NSString *)stringValue {
    return [NSString stringWithFormat:@"%@@%@:%@", _username, _hostname, _path];
}

- (NSURL *)URL {
    // BUG-1452: URLWithString: returns nil if string contains unescaped special characters.
    // Percent-encode the path component to handle special characters in filenames.
    NSString *encodedPath = [_path stringByAddingPercentEncodingWithAllowedCharacters:
                             [NSCharacterSet URLPathAllowedCharacterSet]];
    if (!encodedPath) {
        return nil;
    }
    NSString *urlString = [NSString stringWithFormat:@"%@@%@:%@", _username, _hostname, encodedPath];
    return [NSURL URLWithString:urlString];
}

- (NSString *)usernameHostnameString {
    if (!self.username) {
        return self.hostname;
    }
    return [NSString stringWithFormat:@"%@@%@",self.username, self.hostname];
}

@end
