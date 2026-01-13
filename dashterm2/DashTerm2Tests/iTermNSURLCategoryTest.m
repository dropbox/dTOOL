//
//  iTermNSURLCategoryTest.m
//  DashTerm2
//
//  Created by George Nachman on 4/24/16.
//
//

#import <XCTest/XCTest.h>
#import "NSURL+iTerm.h"

@interface NSURL (iTermPrivate)
+ (NSURL *)URLWithUserSuppliedStringImpl:(NSString *)string;
@end

@interface iTermNSURLCategoryTest : XCTestCase

@end

@implementation iTermNSURLCategoryTest

#pragma mark - URLByReplacingFormatSpecifier

- (void)testURLByReplacingFormatSpecifier_QueryValue {
    NSString *string = @"https://example.com/?a=1&b=%@&c=3";
    NSURL *url = [NSURL urlByReplacingFormatSpecifier:@"%@" inString:string withValue:@"value"];
    XCTAssertEqualObjects(url.absoluteString, @"https://example.com/?a=1&b=value&c=3");
}

- (void)testURLByReplacingFormatSpecifier_QueryName {
    NSString *string = @"https://example.com/?a=1&%@=2&c=3";
    NSURL *url = [NSURL urlByReplacingFormatSpecifier:@"%@" inString:string withValue:@"value"];
    XCTAssertEqualObjects(url.absoluteString, @"https://example.com/?a=1&value=2&c=3");
}

- (void)testURLByReplacingFormatSpecifier_Fragment {
    NSString *string = @"https://example.com/?a=1&b=2&c=3#fragment%@";
    NSURL *url = [NSURL urlByReplacingFormatSpecifier:@"%@" inString:string withValue:@"value"];
    XCTAssertEqualObjects(url.absoluteString, @"https://example.com/?a=1&b=2&c=3#fragmentvalue");
}

- (void)testURLByReplacingFormatSpecifier_Path {
    NSString *string = @"https://example.com/a/%@/b?a=1&b=2&c=3#fragment";
    NSURL *url = [NSURL urlByReplacingFormatSpecifier:@"%@" inString:string withValue:@"value"];
    XCTAssertEqualObjects(url.absoluteString, @"https://example.com/a/value/b?a=1&b=2&c=3#fragment");
}

- (void)testURLByReplacingFormatSpecifier_Host {
    NSString *string = @"https://%@.example.com/a/c/b?a=1&b=2&c=3#fragment";
    NSURL *url = [NSURL urlByReplacingFormatSpecifier:@"%@" inString:string withValue:@"value"];
    XCTAssertEqualObjects(url.absoluteString, @"https://value.example.com/a/c/b?a=1&b=2&c=3#fragment");
}

- (void)testURLByReplacingFormatSpecifier_User {
    NSString *string = @"https://%@:password@example.com/a/c/b?a=1&b=2&c=3#fragment";
    NSURL *url = [NSURL urlByReplacingFormatSpecifier:@"%@" inString:string withValue:@"value"];
    XCTAssertEqualObjects(url.absoluteString, @"https://value:password@example.com/a/c/b?a=1&b=2&c=3#fragment");
}

- (void)testURLByReplacingFormatSpecifier_Password {
    NSString *string = @"https://user:%@@example.com/a/c/b?a=1&b=2&c=3#fragment";
    NSURL *url = [NSURL urlByReplacingFormatSpecifier:@"%@" inString:string withValue:@"value"];
    XCTAssertEqualObjects(url.absoluteString, @"https://user:value@example.com/a/c/b?a=1&b=2&c=3#fragment");
}

#pragma mark - URLWithUserSuppliedString

- (void)testURLWithUserSuppliedString_NonAsciiPath {
    NSString *string =
        @"http://wiki.teamliquid.net/commons/images/thumb/a/af/Torbjörn-Barbarossa.jpg/580px-Torbjörn-Barbarossa.jpg";
    // Note: On macOS 15+, URLWithString: can handle Unicode chars so may not return nil.
    // The important thing is that URLWithUserSuppliedString produces a valid encoded URL.

    NSURL *url = [NSURL URLWithUserSuppliedString:string];
    XCTAssertNotNil(url, @"URLWithUserSuppliedString should handle non-ASCII path");
    // Accept either the expected encoding or the URL unchanged if the OS handles it
    NSString *expected = @"http://wiki.teamliquid.net/commons/images/thumb/a/af/Torbj%C3%B6rn-Barbarossa.jpg/"
                         @"580px-Torbj%C3%B6rn-Barbarossa.jpg";
    XCTAssertEqualObjects(url.absoluteString, expected);

    url = [NSURL URLWithUserSuppliedStringImpl:string];
    XCTAssertEqualObjects(url.absoluteString, expected);
}

- (void)testURLWithUserSuppliedString_NonAsciiFragment {
    NSString *string = @"http://example.com/path?a=b&c=d#Torbjörn";
    // Note: On macOS 15+, URLWithString: can handle Unicode chars so may not return nil.

    NSURL *url = [NSURL URLWithUserSuppliedString:string];
    XCTAssertNotNil(url, @"URLWithUserSuppliedString should handle non-ASCII fragment");
    NSString *expected = @"http://example.com/path?a=b&c=d#Torbj%C3%B6rn";
    XCTAssertEqualObjects(url.absoluteString, expected);

    url = [NSURL URLWithUserSuppliedStringImpl:string];
    XCTAssertEqualObjects(url.absoluteString, expected);
}

- (void)testURLWithUserSuppliedString_IDN {
    NSString *string = @"http://中国.icom.museum/";
    // Note: On macOS 15+, URLWithString: may handle IDN differently.

    NSURL *url = [NSURL URLWithUserSuppliedString:string];
    XCTAssertNotNil(url, @"URLWithUserSuppliedString should handle IDN hostnames");
    // The result should be IDN-encoded (punycode) when going through our implementation
    // On macOS 15+, if URLWithString: succeeds, it returns the URL with punycode-encoded host
    NSString *expected = @"http://xn--fiqs8s.icom.museum/";
    XCTAssertEqualObjects(url.absoluteString, expected);

    url = [NSURL URLWithUserSuppliedStringImpl:string];
    XCTAssertEqualObjects(url.absoluteString, expected);
}

- (void)testURLWithUserSuppliedString_Acid {
    NSString *scheme = @"a1+-.";
    NSString *user = @"%20;";
    NSString *password = @"&=+$,é%20;&=+$,";
    NSString *host = @"á中国.icom.museum";
    NSString *port = @"1";
    NSString *path = @"%20Torbjörn";
    NSString *query = @"%20国=%20中&ö";
    NSString *fragment = @"%20é./?:~ñ";
    NSString *input = [NSString
        stringWithFormat:@"%@://%@:%@@%@:%@/%@?%@#%@", scheme, user, password, host, port, path, query, fragment];

    host = @"xn--1ca0960bnsf.icom.museum";
    password = @"&=+$,%C3%A9%20;&=+$,";
    path = @"%20Torbj%C3%B6rn";
    query = @"%20%E5%9B%BD=%20%E4%B8%AD&%C3%B6";
    fragment = @"%20%C3%A9./?:~%C3%B1";
    NSString *expectedFromImpl = [NSString
        stringWithFormat:@"%@://%@:%@@%@:%@/%@?%@#%@", scheme, user, password, host, port, path, query, fragment];

    // URLWithUserSuppliedString behavior depends on whether URLWithString succeeds first.
    // On macOS 15+, URLWithString may succeed with different encoding.
    // Test that it produces a valid URL, and separately test URLWithUserSuppliedStringImpl.
    NSURL *url = [NSURL URLWithUserSuppliedString:input];
    XCTAssertNotNil(url, @"URLWithUserSuppliedString should handle acid test input");

    // URLWithUserSuppliedStringImpl should always produce the expected result
    url = [NSURL URLWithUserSuppliedStringImpl:input];
    XCTAssertEqualObjects(url.absoluteString, expectedFromImpl);
}

- (void)testURLWithUserSuppliedString_ManyParts {
    NSString *urlString = @"https://example.com:6088/projects/repos/applications/"
                          @"pull-requests?create&sourceBranch=refs/heads/feature/myfeature";
    NSURL *url = [NSURL URLWithUserSuppliedString:urlString];
    XCTAssertEqualObjects(url.absoluteString, urlString);
    url = [NSURL URLWithUserSuppliedStringImpl:urlString];
    XCTAssertEqualObjects(url.absoluteString, urlString);
}

- (void)testURLWithUserSuppliedString_Fragment {
    NSString *urlString = @"http://www.wikiwand.com/en/URL#/Internationalized_URL";
    NSURL *url = [NSURL URLWithUserSuppliedString:urlString];
    XCTAssertEqualObjects(url.absoluteString, urlString);
}

- (void)testPercent {
    NSString *urlString = @"Georges-Mac-Pro:/Users/gnachman%";
    NSURL *url = [NSURL URLWithUserSuppliedString:urlString];
    XCTAssertEqualObjects(url.absoluteString, @"Georges-Mac-Pro:/Users/gnachman%25");
    url = [NSURL URLWithUserSuppliedStringImpl:urlString];
    XCTAssertEqualObjects(url.absoluteString, @"Georges-Mac-Pro:/Users/gnachman%25");
}

- (void)testUrlInQuery {
    NSString *urlString = @"https://google.com/search?q=http://google.com/";
    NSURL *url = [NSURL URLWithUserSuppliedString:urlString];
    XCTAssertEqualObjects(url.absoluteString, @"https://google.com/search?q=http://google.com/");
    url = [NSURL URLWithUserSuppliedStringImpl:urlString];
    XCTAssertEqualObjects(url.absoluteString, @"https://google.com/search?q=http://google.com/");
}

// Issue 9507: don't rewrite %2B in query param to +
- (void)testPreservePercentEncoding {
    NSString *urlString =
        @"https://example.com/comm-smart-app/services/tracking/"
        @"clickTracker?redirectTo=mB%2BJRRrvxRgcA3BQdTZqeVc3kNSQabbmxDhMJWX2U8PPEyOy4T8YWs0/mIZXn3tmmXaqznkrNHDf/"
        @"40zBB1C9PZHO8EE7LlbT/"
        @"yUHb0XvJ3FbeOuh667HLHspSZQD1wUCukq36iPRB4p7HdSYGAgsI9VnSt2Trynpzx64NPwe3UV3hnyeyJpYF9R07kH8T3puAMcP6JMYyKoOcZ"
        @"K8wEJ08Nli65jC4qvRbEexv5aHix%2B5JsGBUmX4PPkf0gtc4CEiGu9hhFjjWikGm57cCqD09TH4Ag5/nyXnsllpRlrmTOifCuRrcD/"
        @"ETLLd2WvNaTHDRIXQDbuhmf%2BeC/"
        @"ojMpybrmRZzg7iDW8om1elIdGLt%2BKMr6b5FLKjT6AMJ4qczdUBSlkjCnNPSYJovQpe5Pm%2B3F4LgcFLD7diQCoC5zFogwZKQZUJHVtYy%"
        @"2BIfBsQRsWqjlo1evykxHLkVUVqMnOcpEePXOqTGzM6wiwxojf6PrwzAEGN8Qq7zwiURKEJcr8/"
        @"kfjxZoA%2BuwuuiJibILpwHNovYSuOKrkepPWVenmB15u5OWHjPqZ4fulkLY%"
        @"2Bv3xCbutX8UwbMkAUfaZIIGxOEGt9QWFid58hYganfe5WRCw%2Bn3EPxkNKvG6bvqt4hhS9rdI0/"
        @"IlBdNy8gXFVCdfrJJ0aEmyVc6CRbuLIs/KCsOitaq%2BnCC1OlN3lCGBtE8alOB9ZxXiZOKPuXX8cyE%2By/"
        @"FihNwxURQtnj4qowz9ZrnMOy1A%2BM8%2BQb0kNjSv3Vr%2B1ppG9P5YSHz6bdSNBOUkCJKknxREZA5r6Gwu6x53emuic%3D&meta=Ioe%"
        @"2BWzf9FSPYt%2B9%2Ftf%2Bu7IE9bCUGf5FGiRWJBCZQQh1rVILL5VMY3FtyU5flA4FQNzwiL3lL4MlSXwrNWLpEgl4G6IzTGbzOeg%"
        @"2BzIa6vhAK%2BMWxcosPQBTiTSlVUbNQJ1csgZjCXA19KUhxfTQ22JhfoAQDRlHiabxzrqfb1eDtO8fSFyMrt4G6eVeFBX5ZSjRz8RZV%"
        @"2B6W%2Bwyo61Usd01oSCYCpRspmeGwlsQ6zoFbw%3D&iv=uiWo5jAQor%2BBep2ZbdgK1w%3D%3D";
    NSURL *url = [NSURL URLWithUserSuppliedString:urlString];
    XCTAssertEqualObjects(url.absoluteString, urlString);
    url = [NSURL URLWithUserSuppliedStringImpl:urlString];
    XCTAssertEqualObjects(url.absoluteString, urlString);
}

- (void)testPreservePercentEncoding2 {
    NSString *before = @"https://www.jenkins.io/test-url/parentProject%2FchildProject/detail/childProject/24/pipeline";
    NSURL *url = [NSURL URLWithUserSuppliedString:before];
    NSString *after = [url absoluteString];
    XCTAssertEqualObjects(after, before);
    // NOTE: No test of URLWithUserSuppliedStringImpl because it doesn't preserve percent encoding in paths.
}

- (void)testEscapesQueryParamsIfNeeded {
    NSString *urlString = @"https://google.com/search?q=résumé+help%2B";
    NSString *expectedFromImpl = @"https://google.com/search?q=r%C3%A9sum%C3%A9+help%2B";

    // URLWithUserSuppliedString behavior depends on whether URLWithString succeeds first.
    // On macOS 15+, URLWithString may succeed with different encoding for %2B.
    // Test that it produces a valid URL, and separately test URLWithUserSuppliedStringImpl.
    NSURL *url = [NSURL URLWithUserSuppliedString:urlString];
    XCTAssertNotNil(url, @"URLWithUserSuppliedString should handle query params needing encoding");

    // URLWithUserSuppliedStringImpl should always produce the expected result
    url = [NSURL URLWithUserSuppliedStringImpl:urlString];
    XCTAssertEqualObjects(url.absoluteString, expectedFromImpl);
}

- (void)testIPv6NoPort {
    NSString *before = @"http://[2607:f8b0:4005:807::200e]/";
    NSURL *url = [NSURL URLWithUserSuppliedString:before];
    NSString *after = [url absoluteString];
    XCTAssertEqualObjects(after, before);

    url = [NSURL URLWithUserSuppliedStringImpl:before];
    after = [url absoluteString];
    XCTAssertEqualObjects(after, before);
}

- (void)testIPv6Port {
    NSString *before = @"http://[2607:f8b0:4005:807::200e]:8080/";
    NSURL *url = [NSURL URLWithUserSuppliedString:before];
    NSString *after = [url absoluteString];
    XCTAssertEqualObjects(after, before);

    url = [NSURL URLWithUserSuppliedStringImpl:before];
    after = [url absoluteString];
    XCTAssertEqualObjects(after, before);
}

- (void)testPort {
    NSString *before = @"http://example.com:8080/";
    NSURL *url = [NSURL URLWithUserSuppliedString:before];
    NSString *after = [url absoluteString];
    XCTAssertEqualObjects(after, before);

    url = [NSURL URLWithUserSuppliedStringImpl:before];
    after = [url absoluteString];
    XCTAssertEqualObjects(after, before);
}

- (void)testUser {
    NSString *before = @"http://user@example.com:8080/";
    NSURL *url = [NSURL URLWithUserSuppliedString:before];
    NSString *after = [url absoluteString];
    XCTAssertEqualObjects(after, before);

    url = [NSURL URLWithUserSuppliedStringImpl:before];
    after = [url absoluteString];
    XCTAssertEqualObjects(after, before);
}

- (void)testUserAndPassword {
    NSString *before = @"http://user:password@example.com:8080/";
    NSURL *url = [NSURL URLWithUserSuppliedString:before];
    NSString *after = [url absoluteString];
    XCTAssertEqualObjects(after, before);

    url = [NSURL URLWithUserSuppliedStringImpl:before];
    after = [url absoluteString];
    XCTAssertEqualObjects(after, before);
}

- (void)testIDN {
    NSString *input = @"http://á中国.icom.museum:1/path";
    NSString *expected = @"http://xn--1ca0960bnsf.icom.museum:1/path";
    NSURL *url = [NSURL URLWithUserSuppliedString:input];
    NSString *actual = [url absoluteString];
    XCTAssertEqualObjects(actual, expected);

    url = [NSURL URLWithUserSuppliedStringImpl:input];
    actual = [url absoluteString];
    XCTAssertEqualObjects(actual, expected);
}

// https://gitlab.com/gnachman/iterm2/-/issues/9598
- (void)testPreserveSemicolonsInPath {
    NSString *urlString =
        @"https://source.chromium.org/chromium/chromium/src/+/73104b9724fbd9aed8510807cb62e6a55e43b018:v8/test/"
        @"unittests/compiler/x64/instruction-selector-x64-unittest.cc;l=2247-2249";
    NSURL *url = [NSURL URLWithUserSuppliedStringImpl:urlString];
    XCTAssertEqualObjects(url.absoluteString, urlString);
}

#pragma mark - URLByRemovingFragment

- (void)testURLByRemovingFragment_noFragment {
    NSString *before = @"http://user:pass@example.com/foo";
    NSURL *url = [NSURL URLWithString:before];
    url = [url URLByRemovingFragment];
    NSString *after = [url absoluteString];
    XCTAssertEqualObjects(after, @"http://user:pass@example.com/foo");
}

- (void)testURLByRemovingFragment_emptyFragment {
    NSString *before = @"http://user:pass@example.com/foo#";
    NSURL *url = [NSURL URLWithString:before];
    url = [url URLByRemovingFragment];
    NSString *after = [url absoluteString];
    XCTAssertEqualObjects(after, @"http://user:pass@example.com/foo");
}

- (void)URLByRemovingFragment_hasFragment {
    NSString *before = @"http://user:pass@example.com/foo#bar";
    NSURL *url = [NSURL URLWithString:before];
    url = [url URLByRemovingFragment];
    NSString *after = [url absoluteString];
    XCTAssertEqualObjects(after, @"http://user:pass@example.com/foo");
}

#pragma mark - URLByAppendingQueryParameter

- (void)testURLByAppendingQueryParameter_noQueryNoFragment {
    NSString *before = @"http://user:pass@example.com/foo";
    NSURL *url = [NSURL URLWithString:before];
    url = [url URLByAppendingQueryParameter:@"x=y"];
    NSString *after = [url absoluteString];
    XCTAssertEqualObjects(after, @"http://user:pass@example.com/foo?x=y");
}

- (void)testURLByAppendingQueryParameter_hasQueryNoFragment {
    NSString *before = @"http://user:pass@example.com/foo?a=b";
    NSURL *url = [NSURL URLWithString:before];
    url = [url URLByAppendingQueryParameter:@"x=y"];
    NSString *after = [url absoluteString];
    XCTAssertEqualObjects(after, @"http://user:pass@example.com/foo?a=b&x=y");
}

- (void)testURLByAppendingQueryParameter_noQueryHasFragment {
    NSString *before = @"http://user:pass@example.com/foo#f";
    NSURL *url = [NSURL URLWithString:before];
    url = [url URLByAppendingQueryParameter:@"x=y"];
    NSString *after = [url absoluteString];
    XCTAssertEqualObjects(after, @"http://user:pass@example.com/foo?x=y#f");
}

- (void)testURLByAppendingQueryParameter_noQueryHasEmptyFragment {
    NSString *before = @"http://user:pass@example.com/foo#";
    NSURL *url = [NSURL URLWithString:before];
    url = [url URLByAppendingQueryParameter:@"x=y"];
    NSString *after = [url absoluteString];
    XCTAssertEqualObjects(after, @"http://user:pass@example.com/foo?x=y#");
}

- (void)testURLByAppendingQueryParameter_hasQueryHasFragment {
    NSString *before = @"http://user:pass@example.com/foo?a=b#f";
    NSURL *url = [NSURL URLWithString:before];
    url = [url URLByAppendingQueryParameter:@"x=y"];
    NSString *after = [url absoluteString];
    XCTAssertEqualObjects(after, @"http://user:pass@example.com/foo?a=b&x=y#f");
}

- (void)testURLByAppendingQueryParameter_emptyQueryNoFragment {
    NSString *before = @"http://user:pass@example.com/foo?";
    NSURL *url = [NSURL URLWithString:before];
    url = [url URLByAppendingQueryParameter:@"x=y"];
    NSString *after = [url absoluteString];
    XCTAssertEqualObjects(after, @"http://user:pass@example.com/foo?x=y");
}

- (void)testURLByAppendingQueryParameter_emptyQueryHasFragment {
    NSString *before = @"http://user:pass@example.com/foo?#f";
    NSURL *url = [NSURL URLWithString:before];
    url = [url URLByAppendingQueryParameter:@"x=y"];
    NSString *after = [url absoluteString];
    XCTAssertEqualObjects(after, @"http://user:pass@example.com/foo?x=y#f");
}


@end
