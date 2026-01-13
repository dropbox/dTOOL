//
//  PerformanceBenchmarkTests.m
//  DashTerm2XCTests
//
//  Performance benchmarks for VT100 grid operations.
//  Run with: xcodebuild test -project DashTerm2.xcodeproj -scheme DashTerm2XCTests
//  -only-testing:DashTerm2XCTests/PerformanceBenchmarkTests
//

#import <XCTest/XCTest.h>
#import "VT100Grid.h"
#import "LineBuffer.h"

@interface PerformanceBenchmarkTests : XCTestCase <VT100GridDelegate>
@end

@implementation PerformanceBenchmarkTests {
    BOOL wraparoundMode_;
    BOOL insertMode_;
}

#pragma mark - Setup

- (void)setUp {
    [super setUp];
    wraparoundMode_ = YES;
    insertMode_ = NO;
}

#pragma mark - VT100GridDelegate

- (screen_char_t)gridForegroundColorCode {
    screen_char_t c = {0};
    return c;
}

- (screen_char_t)gridBackgroundColorCode {
    screen_char_t c = {0};
    return c;
}

- (void)gridCursorDidChangeLine {
}

- (BOOL)gridUseHFSPlusMapping {
    return NO;
}

- (void)gridCursorDidMove {
}

- (iTermUnicodeNormalization)gridUnicodeNormalizationForm {
    return iTermUnicodeNormalizationNone;
}

- (NSInteger)gridUnicodeVersion {
    return 9;
}

#pragma mark - Helper Methods

- (VT100Grid *)gridWithWidth:(int)width height:(int)height {
    VT100Grid *grid = [[VT100Grid alloc] initWithSize:VT100GridSizeMake(width, height) delegate:self];
    return grid;
}

- (void)fillGrid:(VT100Grid *)grid withCharacter:(unichar)c {
    screen_char_t sc = {0};
    sc.code = c;

    for (int y = 0; y < grid.size.height; y++) {
        screen_char_t *line = [grid screenCharsAtLineNumber:y];
        for (int x = 0; x < grid.size.width; x++) {
            line[x] = sc;
        }
    }
}

#pragma mark - Grid Creation Benchmarks

- (void)testPerformanceGridCreation80x24 {
    // Measure time to create typical terminal size
    [self measureBlock:^{
        for (int i = 0; i < 1000; i++) {
            @autoreleasepool {
                VT100Grid *grid = [self gridWithWidth:80 height:24];
                (void)grid; // Prevent optimization
            }
        }
    }];
}

- (void)testPerformanceGridCreation200x50 {
    // Measure time to create larger terminal size
    [self measureBlock:^{
        for (int i = 0; i < 1000; i++) {
            @autoreleasepool {
                VT100Grid *grid = [self gridWithWidth:200 height:50];
                (void)grid;
            }
        }
    }];
}

#pragma mark - Grid Fill Benchmarks

- (void)testPerformanceGridFill80x24 {
    VT100Grid *grid = [self gridWithWidth:80 height:24];

    [self measureBlock:^{
        for (int i = 0; i < 100; i++) {
            [self fillGrid:grid withCharacter:'A' + (i % 26)];
        }
    }];
}

- (void)testPerformanceGridFill200x50 {
    VT100Grid *grid = [self gridWithWidth:200 height:50];

    [self measureBlock:^{
        for (int i = 0; i < 100; i++) {
            [self fillGrid:grid withCharacter:'A' + (i % 26)];
        }
    }];
}

#pragma mark - Scroll Benchmarks

- (void)testPerformanceScrollUp80x24 {
    VT100Grid *grid = [self gridWithWidth:80 height:24];
    [self fillGrid:grid withCharacter:'X'];

    [self measureBlock:^{
        for (int i = 0; i < 10000; i++) {
            [grid scrollWholeScreenUpIntoLineBuffer:nil unlimitedScrollback:NO];
        }
    }];
}

- (void)testPerformanceScrollUp200x50 {
    VT100Grid *grid = [self gridWithWidth:200 height:50];
    [self fillGrid:grid withCharacter:'X'];

    [self measureBlock:^{
        for (int i = 0; i < 10000; i++) {
            [grid scrollWholeScreenUpIntoLineBuffer:nil unlimitedScrollback:NO];
        }
    }];
}

#pragma mark - LineBuffer Benchmarks

- (void)testPerformanceLineBufferAppend {
    LineBuffer *lineBuffer = [[LineBuffer alloc] init];
    [lineBuffer setMaxLines:10000];

    // Create a sample line
    screen_char_t chars[80];
    for (int i = 0; i < 80; i++) {
        memset(&chars[i], 0, sizeof(screen_char_t));
        chars[i].code = 'A' + (i % 26);
    }

    [self measureBlock:^{
        for (int i = 0; i < 10000; i++) {
            [lineBuffer appendLine:chars length:80 partial:NO width:80 timestamp:0 continuation:(screen_char_t){0}];
        }
    }];
}

- (void)testPerformanceLineBufferPopAndPush {
    LineBuffer *lineBuffer = [[LineBuffer alloc] init];
    [lineBuffer setMaxLines:10000];

    // Pre-fill the buffer
    screen_char_t chars[80];
    for (int i = 0; i < 80; i++) {
        memset(&chars[i], 0, sizeof(screen_char_t));
        chars[i].code = 'A' + (i % 26);
    }

    for (int i = 0; i < 5000; i++) {
        [lineBuffer appendLine:chars length:80 partial:NO width:80 timestamp:0 continuation:(screen_char_t){0}];
    }

    [self measureBlock:^{
        for (int i = 0; i < 1000; i++) {
            screen_char_t cont;
            screen_char_t buffer[80];
            int length = 0;
            [lineBuffer popAndCopyLastLineInto:buffer
                                         width:80
                             includesEndOfLine:NULL
                                     timestamp:NULL
                                  continuation:&cont];

            [lineBuffer appendLine:chars length:80 partial:NO width:80 timestamp:0 continuation:(screen_char_t){0}];
        }
    }];
}

#pragma mark - Resize Benchmarks

- (void)testPerformanceGridResize {
    VT100Grid *grid = [self gridWithWidth:80 height:24];
    [self fillGrid:grid withCharacter:'X'];

    [self measureBlock:^{
        for (int i = 0; i < 100; i++) {
            // Alternate between two sizes
            [grid setSize:VT100GridSizeMake(120, 40)];
            [grid setSize:VT100GridSizeMake(80, 24)];
        }
    }];
}

#pragma mark - Character Access Benchmarks

- (void)testPerformanceScreenCharsAccess {
    VT100Grid *grid = [self gridWithWidth:200 height:50];
    [self fillGrid:grid withCharacter:'X'];

    [self measureBlock:^{
        for (int iter = 0; iter < 1000; iter++) {
            for (int y = 0; y < grid.size.height; y++) {
                screen_char_t *line = [grid screenCharsAtLineNumber:y];
                // Read each character
                for (int x = 0; x < grid.size.width; x++) {
                    unichar c = line[x].code;
                    (void)c; // Prevent optimization
                }
            }
        }
    }];
}

@end
