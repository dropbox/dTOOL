//
//  SearchTests.m
//  DashTerm2XCTests
//
//  Created by George Nachman on 3/2/23.
//

#import <XCTest/XCTest.h>
#import "LineBlock.h"
#import "LineBufferPosition.h"
#import "DashTerm2SharedARC-Swift.h"

@interface SearchTests : XCTestCase

@end

@implementation SearchTests

- (void)setUp {
    // Setup not needed after iTermCharacterBufferContext removal
}

// `end` is inclusive.
- (ResultRange *)rangeFrom:(int)start to:(int)end {
    ResultRange *rr = [[ResultRange alloc] init];
    rr->position = start;
    rr->length = end - start + 1;
    return rr;
}

// BUG-10455: Zalgo text search test - resource file zalgo_for_unit_test.txt
// was never added to the test bundle. Skip if resource unavailable.
- (void)testZalgo {
    NSString *zalgo = [NSString stringWithContentsOfFile:[[NSBundle bundleForClass:[self class]] pathForResource:@"zalgo_for_unit_test" ofType:@"txt"] encoding:NSUTF8StringEncoding error:nil];
    if (!zalgo) {
        // Resource file not available - skip test (pre-existing issue)
        return;
    }
    LineBlock *block = [[LineBlock alloc] initWithRawBufferSize:8192 absoluteBlockNumber:0];
    screen_char_t buf[8192];
    screen_char_t zero = { 0 };
    int len = 8192;
    BOOL foundDwc = NO;
    BOOL rtlFound = NO;
    StringToScreenChars(zalgo, buf, zero, zero, &len, NO, nil, &foundDwc, iTermUnicodeNormalizationNone, 9, NO, &rtlFound);
    screen_char_t eol = { .code = EOL_HARD };
    [block appendLine:buf length:len partial:NO width:80 metadata:iTermMetadataMakeImmutable(iTermMetadataDefault()) continuation:eol];

    NSMutableArray *actual = [NSMutableArray array];
    BOOL includesPartialLastLine = NO;
    [block findSubstring:@"zal"
                 options:FindOptBackwards | FindMultipleResults
                    mode:iTermFindModeSmartCaseSensitivity
                atOffset:-1
                 results:actual
         multipleResults:YES
 includesPartialLastLine:&includesPartialLastLine
                lineProvider:nil];

    NSArray<ResultRange *> *expected = @[
        [self rangeFrom:469 to:471],
        [self rangeFrom:0 to:2]
    ];
    XCTAssertEqualObjects(actual, expected);
}

- (void)testTrivial {
    NSString *haystack = @"abczal";
    LineBlock *block = [[LineBlock alloc] initWithRawBufferSize:8192 absoluteBlockNumber:0];
    screen_char_t buf[8192];
    screen_char_t zero = { 0 };
    int len = 8192;
    BOOL foundDwc = NO;
    BOOL rtlFound = NO;
    StringToScreenChars(haystack, buf, zero, zero, &len, NO, nil, &foundDwc, iTermUnicodeNormalizationNone, 9, NO, &rtlFound);
    screen_char_t eol = { .code = EOL_HARD };
    [block appendLine:buf length:len partial:NO width:80 metadata:iTermMetadataMakeImmutable(iTermMetadataDefault()) continuation:eol];

    NSMutableArray *actual = [NSMutableArray array];
    BOOL includesPartialLastLine = NO;
    [block findSubstring:@"zal"
                 options:FindOptBackwards | FindMultipleResults
                    mode:iTermFindModeSmartCaseSensitivity
                atOffset:-1
                 results:actual
         multipleResults:YES
 includesPartialLastLine:&includesPartialLastLine
                lineProvider:nil];
    NSArray<ResultRange *> *expected = @[
        [self rangeFrom:3 to:5],
    ];
    XCTAssertEqualObjects(actual, expected);
}

// GitLab #12193: Tests overlapping search behavior.
// The search implementation now finds overlapping matches.
// When searching backwards for "xx" in "xxx", it finds both matches:
// - "xx" at position 1-2
// - "xx" at position 0-1 (overlapping with the first)
- (void)testOverlapping {
    NSString *haystack = @"xxx";
    LineBlock *block = [[LineBlock alloc] initWithRawBufferSize:8192 absoluteBlockNumber:0];
    screen_char_t buf[8192];
    screen_char_t zero = { 0 };
    int len = 8192;
    BOOL foundDwc = NO;
    BOOL rtlFound = NO;
    StringToScreenChars(haystack, buf, zero, zero, &len, NO, nil, &foundDwc, iTermUnicodeNormalizationNone, 9, NO, &rtlFound);
    screen_char_t eol = { .code = EOL_HARD };
    [block appendLine:buf length:len partial:NO width:80 metadata:iTermMetadataMakeImmutable(iTermMetadataDefault()) continuation:eol];

    NSMutableArray *actual = [NSMutableArray array];
    BOOL includesPartialLastLine = NO;
    [block findSubstring:@"xx"
                 options:FindOptBackwards | FindMultipleResults
                    mode:iTermFindModeSmartCaseSensitivity
                atOffset:-1
                 results:actual
         multipleResults:YES
 includesPartialLastLine:&includesPartialLastLine
                lineProvider:nil];
    // GitLab #12193: Overlapping search finds both matches when searching backwards
    NSArray<ResultRange *> *expected = @[
        [self rangeFrom:1 to:2],
        [self rangeFrom:0 to:1],
    ];
    XCTAssertEqualObjects(actual, expected);
}

// GitLab #12193: Test overlapping matches searching forwards
- (void)test_BUG_12193_overlappingMatchesForward {
    NSString *haystack = @"xxx";
    LineBlock *block = [[LineBlock alloc] initWithRawBufferSize:8192 absoluteBlockNumber:0];
    screen_char_t buf[8192];
    screen_char_t zero = { 0 };
    int len = 8192;
    BOOL foundDwc = NO;
    BOOL rtlFound = NO;
    StringToScreenChars(haystack, buf, zero, zero, &len, NO, nil, &foundDwc, iTermUnicodeNormalizationNone, 9, NO, &rtlFound);
    screen_char_t eol = { .code = EOL_HARD };
    [block appendLine:buf length:len partial:NO width:80 metadata:iTermMetadataMakeImmutable(iTermMetadataDefault()) continuation:eol];

    NSMutableArray *actual = [NSMutableArray array];
    BOOL includesPartialLastLine = NO;
    [block findSubstring:@"xx"
                 options:FindMultipleResults  // Forward search (no FindOptBackwards)
                    mode:iTermFindModeSmartCaseSensitivity
                atOffset:0
                 results:actual
         multipleResults:YES
 includesPartialLastLine:&includesPartialLastLine
                lineProvider:nil];
    // GitLab #12193: Searching "xx" in "xxx" forward should find 2 overlapping matches
    // Match 1: positions 0-1 ("xx" starting at index 0)
    // Match 2: positions 1-2 ("xx" starting at index 1)
    XCTAssertEqual(actual.count, 2, @"Should find 2 overlapping matches for 'xx' in 'xxx'");
    if (actual.count >= 2) {
        ResultRange *first = actual[0];
        ResultRange *second = actual[1];
        XCTAssertEqual(first.position, 0, @"First match should start at position 0");
        XCTAssertEqual(first.length, 2, @"First match should have length 2");
        XCTAssertEqual(second.position, 1, @"Second match should start at position 1");
        XCTAssertEqual(second.length, 2, @"Second match should have length 2");
    }
}

// GitLab #12193: Test longer overlapping pattern "aaa" in "aaaa" (should find 2 matches)
- (void)test_BUG_12193_longerOverlappingPattern {
    NSString *haystack = @"aaaa";
    LineBlock *block = [[LineBlock alloc] initWithRawBufferSize:8192 absoluteBlockNumber:0];
    screen_char_t buf[8192];
    screen_char_t zero = { 0 };
    int len = 8192;
    BOOL foundDwc = NO;
    BOOL rtlFound = NO;
    StringToScreenChars(haystack, buf, zero, zero, &len, NO, nil, &foundDwc, iTermUnicodeNormalizationNone, 9, NO, &rtlFound);
    screen_char_t eol = { .code = EOL_HARD };
    [block appendLine:buf length:len partial:NO width:80 metadata:iTermMetadataMakeImmutable(iTermMetadataDefault()) continuation:eol];

    NSMutableArray *actual = [NSMutableArray array];
    BOOL includesPartialLastLine = NO;
    [block findSubstring:@"aaa"
                 options:FindMultipleResults
                    mode:iTermFindModeSmartCaseSensitivity
                atOffset:0
                 results:actual
         multipleResults:YES
 includesPartialLastLine:&includesPartialLastLine
                lineProvider:nil];
    // "aaa" in "aaaa" should find 2 overlapping matches:
    // Match 1: positions 0-2
    // Match 2: positions 1-3
    XCTAssertEqual(actual.count, 2, @"Should find 2 overlapping matches for 'aaa' in 'aaaa'");
}

- (void)testMultiLineSearchUsesLineProvider {
    LineBlock *first = [[LineBlock alloc] initWithRawBufferSize:256 absoluteBlockNumber:0];
    LineBlock *second = [[LineBlock alloc] initWithRawBufferSize:256 absoluteBlockNumber:1];
    screen_char_t zero = { 0 };
    screen_char_t eol = { .code = EOL_HARD };
    BOOL foundDwc = NO;
    BOOL rtlFound = NO;
    int len = 16;
    screen_char_t fooBuf[16] = { 0 };
    StringToScreenChars(@"foo", fooBuf, zero, zero, &len, NO, nil, &foundDwc, iTermUnicodeNormalizationNone, 9, NO, &rtlFound);
    iTermImmutableMetadata metadata = iTermMetadataMakeImmutable(iTermMetadataDefault());
    XCTAssertTrue([first appendLine:fooBuf
                             length:len
                            partial:NO
                              width:80
                           metadata:metadata
                       continuation:eol]);

    len = 16;
    screen_char_t barBuf[16] = { 0 };
    foundDwc = NO;
    rtlFound = NO;
    StringToScreenChars(@"bar", barBuf, zero, zero, &len, NO, nil, &foundDwc, iTermUnicodeNormalizationNone, 9, NO, &rtlFound);
    XCTAssertTrue([second appendLine:barBuf
                              length:len
                             partial:NO
                               width:80
                            metadata:metadata
                        continuation:eol]);

    NSMutableArray *results = [NSMutableArray array];
    BOOL includesPartial = NO;
    LineBlockRelativeLineProvider provider = ^BOOL(LineBlock *startBlock,
                                                  int startEntry,
                                                  int relativeLineIndex,
                                                  LineBlock *__autoreleasing *outBlock,
                                                  int *outEntry) {
        if (relativeLineIndex == 0) {
            if (outBlock) {
                *outBlock = startBlock;
            }
            if (outEntry) {
                *outEntry = startEntry;
            }
            return YES;
        }
        if (relativeLineIndex == 1) {
            if (outBlock) {
                *outBlock = second;
            }
            if (outEntry) {
                *outEntry = second.firstEntry;
            }
            return YES;
        }
        return NO;
    };

    [first findSubstring:@"foo\nbar"
                 options:FindOptMultiLine
                    mode:iTermFindModeCaseSensitiveSubstring
                atOffset:0
                 results:results
         multipleResults:NO
 includesPartialLastLine:&includesPartial
               lineProvider:provider];

    XCTAssertEqual(results.count, 1);
    ResultRange *range = results.firstObject;
    XCTAssertNotNil(range);
    XCTAssertEqual(range.position, 0);
    XCTAssertEqual(range.length, 6);
    XCTAssertFalse(includesPartial);
}

@end
