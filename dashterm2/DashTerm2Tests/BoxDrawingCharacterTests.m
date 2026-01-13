//
//  BoxDrawingCharacterTests.m
//  DashTerm2Tests
//
//  Created by AI Worker on 12/31/25.
//
//  Regression tests for box drawing character rendering.
//  These tests verify that the box drawing character sets and bezier path
//  factories are correctly configured to render all standard box drawing glyphs.
//
//  Related bug: dtermCoreRendererEnabled was enabled by default but dterm-core
//  does not implement box drawing character rendering, causing invisible characters.
//

#import <XCTest/XCTest.h>
#import "iTermBoxDrawingBezierCurveFactory.h"
#import "charmaps.h"

@interface BoxDrawingCharacterTests : XCTestCase
@end

@implementation BoxDrawingCharacterTests

#pragma mark - Character Set Membership Tests

/// BUG-BOX-DRAWING: Verify double-line box drawing characters are in the character set.
/// These are commonly used in TUI applications (e.g., dialog boxes, borders).
- (void)test_boxDrawingCharacterSet_includesDoubleLineCharacters {
    NSCharacterSet *boxSet = [iTermBoxDrawingBezierCurveFactory boxDrawingCharactersWithBezierPathsIncludingPowerline:NO];

    // Double-line box drawing characters (Unicode 0x2550-0x256C range)
    UTF32Char doubleLineChars[] = {
        0x2550,  // ═ BOX DRAWINGS DOUBLE HORIZONTAL
        0x2551,  // ║ BOX DRAWINGS DOUBLE VERTICAL
        0x2554,  // ╔ BOX DRAWINGS DOUBLE DOWN AND RIGHT
        0x2557,  // ╗ BOX DRAWINGS DOUBLE DOWN AND LEFT
        0x255A,  // ╚ BOX DRAWINGS DOUBLE UP AND RIGHT
        0x255D,  // ╝ BOX DRAWINGS DOUBLE UP AND LEFT
        0x2560,  // ╠ BOX DRAWINGS DOUBLE VERTICAL AND RIGHT
        0x2563,  // ╣ BOX DRAWINGS DOUBLE VERTICAL AND LEFT
        0x2566,  // ╦ BOX DRAWINGS DOUBLE DOWN AND HORIZONTAL
        0x2569,  // ╩ BOX DRAWINGS DOUBLE UP AND HORIZONTAL
        0x256C,  // ╬ BOX DRAWINGS DOUBLE VERTICAL AND HORIZONTAL
    };

    for (size_t i = 0; i < sizeof(doubleLineChars) / sizeof(doubleLineChars[0]); i++) {
        UTF32Char c = doubleLineChars[i];
        XCTAssertTrue([boxSet longCharacterIsMember:c],
                     @"Double-line box drawing character U+%04X should be in character set", c);
    }
}

/// BUG-BOX-DRAWING: Verify single-line box drawing characters are in the character set.
- (void)test_boxDrawingCharacterSet_includesSingleLineCharacters {
    NSCharacterSet *boxSet = [iTermBoxDrawingBezierCurveFactory boxDrawingCharactersWithBezierPathsIncludingPowerline:NO];

    // Single-line box drawing characters (Unicode 0x2500-0x254F range)
    UTF32Char singleLineChars[] = {
        0x2500,  // ─ BOX DRAWINGS LIGHT HORIZONTAL
        0x2502,  // │ BOX DRAWINGS LIGHT VERTICAL
        0x250C,  // ┌ BOX DRAWINGS LIGHT DOWN AND RIGHT
        0x2510,  // ┐ BOX DRAWINGS LIGHT DOWN AND LEFT
        0x2514,  // └ BOX DRAWINGS LIGHT UP AND RIGHT
        0x2518,  // ┘ BOX DRAWINGS LIGHT UP AND LEFT
        0x251C,  // ├ BOX DRAWINGS LIGHT VERTICAL AND RIGHT
        0x2524,  // ┤ BOX DRAWINGS LIGHT VERTICAL AND LEFT
        0x252C,  // ┬ BOX DRAWINGS LIGHT DOWN AND HORIZONTAL
        0x2534,  // ┴ BOX DRAWINGS LIGHT UP AND HORIZONTAL
        0x253C,  // ┼ BOX DRAWINGS LIGHT VERTICAL AND HORIZONTAL
    };

    for (size_t i = 0; i < sizeof(singleLineChars) / sizeof(singleLineChars[0]); i++) {
        UTF32Char c = singleLineChars[i];
        XCTAssertTrue([boxSet longCharacterIsMember:c],
                     @"Single-line box drawing character U+%04X should be in character set", c);
    }
}

/// BUG-BOX-DRAWING: Verify heavy/bold box drawing characters are in the character set.
- (void)test_boxDrawingCharacterSet_includesHeavyLineCharacters {
    NSCharacterSet *boxSet = [iTermBoxDrawingBezierCurveFactory boxDrawingCharactersWithBezierPathsIncludingPowerline:NO];

    // Heavy box drawing characters
    UTF32Char heavyChars[] = {
        0x2501,  // ━ BOX DRAWINGS HEAVY HORIZONTAL
        0x2503,  // ┃ BOX DRAWINGS HEAVY VERTICAL
        0x250F,  // ┏ BOX DRAWINGS HEAVY DOWN AND RIGHT
        0x2513,  // ┓ BOX DRAWINGS HEAVY DOWN AND LEFT
        0x2517,  // ┗ BOX DRAWINGS HEAVY UP AND RIGHT
        0x251B,  // ┛ BOX DRAWINGS HEAVY UP AND LEFT
    };

    for (size_t i = 0; i < sizeof(heavyChars) / sizeof(heavyChars[0]); i++) {
        UTF32Char c = heavyChars[i];
        XCTAssertTrue([boxSet longCharacterIsMember:c],
                     @"Heavy box drawing character U+%04X should be in character set", c);
    }
}

#pragma mark - Block Drawing Character Tests

/// BUG-BOX-DRAWING: Verify block elements are in the block drawing character set.
- (void)test_blockDrawingCharacterSet_includesBlockElements {
    NSCharacterSet *blockSet = [iTermBoxDrawingBezierCurveFactory blockDrawingCharacters];

    // Block elements (Unicode 0x2580-0x259F range)
    unichar blockChars[] = {
        0x2580,  // ▀ UPPER HALF BLOCK
        0x2584,  // ▄ LOWER HALF BLOCK
        0x2588,  // █ FULL BLOCK
        0x258C,  // ▌ LEFT HALF BLOCK
        0x2590,  // ▐ RIGHT HALF BLOCK
        0x2591,  // ░ LIGHT SHADE
        0x2592,  // ▒ MEDIUM SHADE
        0x2593,  // ▓ DARK SHADE
    };

    for (size_t i = 0; i < sizeof(blockChars) / sizeof(blockChars[0]); i++) {
        unichar c = blockChars[i];
        XCTAssertTrue([blockSet characterIsMember:c],
                     @"Block element character U+%04X should be in block character set", c);
    }
}

#pragma mark - Powerline Glyph Tests

/// BUG-BOX-DRAWING: Verify Powerline glyphs are detected correctly.
- (void)test_powerlineGlyphs_areDetectedCorrectly {
    // Common Powerline glyphs
    UTF32Char powerlineChars[] = {
        0xE0B0,  //  Right-pointing triangle
        0xE0B1,  //  Right-pointing chevron
        0xE0B2,  //  Left-pointing triangle
        0xE0B3,  //  Left-pointing chevron
    };

    for (size_t i = 0; i < sizeof(powerlineChars) / sizeof(powerlineChars[0]); i++) {
        UTF32Char c = powerlineChars[i];
        XCTAssertTrue([iTermBoxDrawingBezierCurveFactory isPowerlineGlyph:c],
                     @"Powerline glyph U+%04X should be detected as Powerline", c);
    }

    // Non-Powerline characters should return NO
    XCTAssertFalse([iTermBoxDrawingBezierCurveFactory isPowerlineGlyph:'A'],
                  @"ASCII 'A' should not be detected as Powerline");
    XCTAssertFalse([iTermBoxDrawingBezierCurveFactory isPowerlineGlyph:0x2500],
                  @"Box drawing character should not be detected as Powerline");
}

/// BUG-BOX-DRAWING: Verify Powerline glyphs are included when requested.
- (void)test_boxDrawingCharacterSet_includesPowerlineWhenRequested {
    NSCharacterSet *withPowerline = [iTermBoxDrawingBezierCurveFactory boxDrawingCharactersWithBezierPathsIncludingPowerline:YES];
    NSCharacterSet *withoutPowerline = [iTermBoxDrawingBezierCurveFactory boxDrawingCharactersWithBezierPathsIncludingPowerline:NO];

    UTF32Char powerlineGlyph = 0xE0B0;  //  Right-pointing triangle

    XCTAssertTrue([withPowerline longCharacterIsMember:powerlineGlyph],
                 @"Powerline glyph should be in character set when includingPowerline:YES");

    // Note: The without-powerline set may or may not include powerline characters
    // depending on implementation. We just verify the with-powerline set has them.
}

#pragma mark - Drawing Function Tests

/// BUG-BOX-DRAWING: Verify drawCodeInCurrentContext doesn't crash for valid codes.
- (void)test_drawCodeInCurrentContext_doesNotCrashForValidCodes {
    // Create a bitmap context to draw into
    CGColorSpaceRef colorSpace = CGColorSpaceCreateDeviceRGB();
    CGContextRef context = CGBitmapContextCreate(NULL, 100, 100, 8, 0, colorSpace,
                                                  kCGImageAlphaPremultipliedLast);
    CGColorSpaceRelease(colorSpace);

    XCTAssertNotEqual(context, NULL, @"Should be able to create bitmap context");

    if (context) {
        NSGraphicsContext *nsContext = [NSGraphicsContext graphicsContextWithCGContext:context flipped:NO];
        [NSGraphicsContext setCurrentContext:nsContext];

        CGColorRef color = CGColorCreateGenericRGB(1.0, 1.0, 1.0, 1.0);
        NSSize cellSize = NSMakeSize(10, 20);

        // Test drawing various box drawing characters
        UTF32Char testCodes[] = {
            0x2500,  // ─
            0x2502,  // │
            0x250C,  // ┌
            0x2550,  // ═
            0x2551,  // ║
        };

        for (size_t i = 0; i < sizeof(testCodes) / sizeof(testCodes[0]); i++) {
            // This should not crash
            [iTermBoxDrawingBezierCurveFactory drawCodeInCurrentContext:testCodes[i]
                                                               cellSize:cellSize
                                                                  scale:2.0
                                                               isPoints:NO
                                                                 offset:CGPointZero
                                                                  color:color
                                               useNativePowerlineGlyphs:NO];
        }

        CGColorRelease(color);
        CGContextRelease(context);
    }
}

#pragma mark - Regression Tests

/// BUG-BOX-DRAWING: Regression test for the issue where box drawing characters
/// were invisible when dtermCoreRendererEnabled was YES.
/// This test verifies that the standard Metal renderer's box drawing infrastructure
/// is complete and functional.
- (void)test_REGRESSION_boxDrawingInfrastructureIsComplete {
    // The bug: dtermCoreRendererEnabled=YES caused invisible box drawing because
    // dterm-core doesn't implement box drawing rendering.
    //
    // The fix: Changed default to dtermCoreRendererEnabled=NO until dterm-core
    // has feature parity.
    //
    // This test verifies the standard renderer has working box drawing:

    // 1. Character sets exist and are non-empty
    NSCharacterSet *boxSet = [iTermBoxDrawingBezierCurveFactory boxDrawingCharactersWithBezierPathsIncludingPowerline:YES];
    XCTAssertNotNil(boxSet, @"Box drawing character set should exist");

    // Verify some characters are in the set
    XCTAssertTrue([boxSet longCharacterIsMember:0x2500], @"Basic horizontal line should be in set");
    XCTAssertTrue([boxSet longCharacterIsMember:0x2502], @"Basic vertical line should be in set");

    // 2. Block drawing characters exist
    NSCharacterSet *blockSet = [iTermBoxDrawingBezierCurveFactory blockDrawingCharacters];
    XCTAssertNotNil(blockSet, @"Block drawing character set should exist");
    XCTAssertTrue([blockSet characterIsMember:0x2588], @"Full block should be in set");

    // 3. The class responds to the drawing selector (proves implementation exists)
    XCTAssertTrue([iTermBoxDrawingBezierCurveFactory respondsToSelector:@selector(drawCodeInCurrentContext:cellSize:scale:isPoints:offset:color:useNativePowerlineGlyphs:)],
                 @"Drawing method should exist");
}

@end
