//
//  iTermMetalRowData.m
//  DashTerm2
//
//  Created by George Nachman on 10/27/17.
//

#import "iTermMetalRowData.h"

#import "DashTerm2SharedARC-Swift.h"
#import "iTermMetalGlyphKey.h"
#import "iTermTextRendererCommon.h"
#import "ScreenChar.h"
#import "ScreenCharArray.h"

@implementation iTermMetalRowData

- (instancetype)init {
    self = [super init];
    return self;
}

- (void)writeDebugInfoToFolder:(NSURL *)folder {
    NSString *info = [NSString
        stringWithFormat:@"y=%@\n"
                         @"numberOfBackgroundRLEs=%@\n"
                         @"numberOfDrawableGlyphs=%@\n"
                         @"markStyle=%@\n"
                         @"belongsToBlock=%@\n"
                         @"date=%@\n"
                         @"bidi=%@\n",
                         @(self.y), @(self.numberOfBackgroundRLEs), @(self.numberOfDrawableGlyphs), @(self.markStyle),
                         @(self.belongsToBlock), self.date, [self.screenCharArray.bidiInfo description]];
    [info writeToURL:[folder URLByAppendingPathComponent:@"info.txt"]
          atomically:NO
            encoding:NSUTF8StringEncoding
               error:NULL];

    @autoreleasepool {
        const NSUInteger keyCount = _keysData.length / sizeof(iTermMetalGlyphKey);
        NSMutableString *glyphKeysString = [[NSMutableString alloc] initWithCapacity:keyCount * 64];
        const iTermMetalGlyphKey *glyphKeys = (iTermMetalGlyphKey *)_keysData.bytes;
        for (int i = 0; i < keyCount; i++) {
            NSString *glyphKey = iTermMetalGlyphKeyDescription(&glyphKeys[i]);
            [glyphKeysString appendFormat:@"%4d: %@\n", i, glyphKey];
        }
        [glyphKeysString writeToURL:[folder URLByAppendingPathComponent:@"GlyphKeys.txt"]
                         atomically:NO
                           encoding:NSUTF8StringEncoding
                              error:NULL];
    }

    @autoreleasepool {
        const NSUInteger attrCount = _attributesData.length / sizeof(iTermMetalGlyphAttributes);
        NSMutableString *attributesString = [[NSMutableString alloc] initWithCapacity:attrCount * 64];
        iTermMetalGlyphAttributes *attributes = (iTermMetalGlyphAttributes *)_attributesData.mutableBytes;
        for (int i = 0; i < attrCount; i++) {
            NSString *attribute = iTermMetalGlyphAttributesDescription(&attributes[i]);
            [attributesString appendFormat:@"%4d: %@\n", i, attribute];
        }
        [attributesString writeToURL:[folder URLByAppendingPathComponent:@"Attributes.txt"]
                          atomically:NO
                            encoding:NSUTF8StringEncoding
                               error:NULL];
    }

    @autoreleasepool {
        NSMutableString *bgColorsString = [[NSMutableString alloc] initWithCapacity:_numberOfBackgroundRLEs * 48];
        iTermMetalBackgroundColorRLE *bg = (iTermMetalBackgroundColorRLE *)_backgroundColorRLEData.mutableBytes;
        for (int i = 0; i < _numberOfBackgroundRLEs; i++) {
            [bgColorsString appendFormat:@"%@\n", iTermMetalBackgroundColorRLEDescription(&bg[i])];
        }
        [bgColorsString writeToURL:[folder URLByAppendingPathComponent:@"BackgroundColors.txt"]
                        atomically:NO
                          encoding:NSUTF8StringEncoding
                             error:NULL];
    }

    @autoreleasepool {
        NSMutableString *lineString = [[NSMutableString alloc] initWithCapacity:_screenCharArray.length * 64];
        const screen_char_t *const line = _screenCharArray.line;
        for (int i = 0; i < _screenCharArray.length; i++) {
            screen_char_t c = line[i];
            [lineString appendFormat:@"%4d: %@\n", i, [self formatChar:c]];
        }
        [lineString writeToURL:[folder URLByAppendingPathComponent:@"ScreenChars.txt"]
                    atomically:NO
                      encoding:NSUTF8StringEncoding
                         error:NULL];
    }
}

- (NSString *)formatChar:(screen_char_t)c {
    return DebugStringForScreenChar(c);
}

- (BOOL)hasFold {
    switch (self.markStyle) {
        case iTermMarkStyleNone:
        case iTermMarkStyleRegularSuccess:
        case iTermMarkStyleRegularFailure:
        case iTermMarkStyleRegularOther:
            return NO;
        case iTermMarkStyleFoldedSuccess:
        case iTermMarkStyleFoldedFailure:
        case iTermMarkStyleFoldedOther:
            return YES;
    }
}

@end
