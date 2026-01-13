//
//  TmuxHistoryParser.m
//  iTerm
//
//  Created by George Nachman on 11/29/11.
//

#import "TmuxHistoryParser.h"

#import "DashTerm2SharedARC-Swift.h"
#import "iTermAdvancedSettingsModel.h"
#import "iTermMalloc.h"
#import "ScreenChar.h"
#import "VT100Terminal.h"

@implementation TmuxHistoryParser {
    // dterm-core parser adapter (created lazily)
    DTermCoreParserAdapter *_dtermCoreParserAdapter;
}

+ (TmuxHistoryParser *)sharedInstance {
    static TmuxHistoryParser *instance;
    if (!instance) {
        instance = [[TmuxHistoryParser alloc] init];
    }
    return instance;
}

// Returns nil on error
// TODO: Test with italics
// TODO: Add external attributes here when tmux gains OSC 8 or colored underline support.
// For underline color, see https://github.com/tmux/tmux/issues/2928
- (NSData *)dataForHistoryLine:(NSString *)hist
                  withTerminal:(VT100Terminal *)terminal
        ambiguousIsDoubleWidth:(BOOL)ambiguousIsDoubleWidth
                unicodeVersion:(NSInteger)unicodeVersion
               alternateScreen:(BOOL)alternateScreen
                      rtlFound:(BOOL *)rtlFoundPtr {
    if (rtlFoundPtr) {
        *rtlFoundPtr = NO;
    }
    screen_char_t *screenChars;
    // Pre-allocate based on input history size; each char may become a screen_char_t
    NSMutableData *result = [NSMutableData dataWithCapacity:hist.length * sizeof(screen_char_t)];
    NSData *histData = [hist dataUsingEncoding:NSUTF8StringEncoding];

    // Use dterm-core parser if enabled
    const BOOL useDTermCore = [iTermAdvancedSettingsModel dtermCoreEnabled] &&
                              [iTermAdvancedSettingsModel dtermCoreParserOutputEnabled];

    NSArray<VT100Token *> *tokens = nil;
    CVector vector;
    int n = 0;

    if (useDTermCore) {
        // dterm-core path: lazy-create adapter and parse
        if (!_dtermCoreParserAdapter) {
            _dtermCoreParserAdapter = [[DTermCoreParserAdapter alloc] init];
        }
        tokens = [_dtermCoreParserAdapter parseWithBytes:(const uint8_t *)histData.bytes
                                                   length:(int)histData.length];
        n = (int)tokens.count;
    } else {
        // Legacy path: use VT100Parser
        [terminal.parser putStreamData:histData.bytes length:histData.length];
        CVectorCreate(&vector, 100);
        [terminal.parser addParsedTokensToVector:&vector];
        n = CVectorCount(&vector);
    }

    for (int i = 0; i < n; i++) {
        VT100Token *token = useDTermCore ? tokens[i] : CVectorGetObject(&vector, i);
        [terminal executeToken:token];
        NSString *string = token.isStringType ? token.string : nil;
        if (!string &&
            (token->type == VT100_ASCIISTRING || token->type == VT100_MIXED_ASCII_CR_LF || token->type == VT100_GANG)) {
            string = [token stringForAsciiData];
        }

        if (string) {
            // Allocate double space in case they're all double-width characters.
            // BUG-1478: Check for integer overflow in buffer size calculation
            const NSUInteger stringLen = string.length;
            if (stringLen == 0) {
                continue;
            }
            const size_t screenCharSize = sizeof(screen_char_t);
            // Check: 2 * stringLen must not overflow
            if (stringLen > SIZE_MAX / 2) {
                continue; // Skip this string - too large
            }
            const size_t count = 2 * stringLen;
            // Check: screenCharSize * count must not overflow
            if (count > SIZE_MAX / screenCharSize) {
                continue; // Skip this string - too large
            }
            screenChars = iTermMalloc(screenCharSize * count);
            int len = 0;
            BOOL rtlFound = NO;
            StringToScreenChars(string, screenChars, [terminal foregroundColorCode], [terminal backgroundColorCode],
                                &len, ambiguousIsDoubleWidth, NULL, NULL, NO, unicodeVersion, alternateScreen,
                                &rtlFound);
            if (rtlFound && rtlFoundPtr != nil) {
                *rtlFoundPtr = YES;
            }
            if ([token isAscii] && [terminal charset]) {
                ConvertCharsToGraphicsCharset(screenChars, len);
            }
            [result appendBytes:screenChars length:sizeof(screen_char_t) * len];
            free(screenChars);
        }
        // Only release tokens from legacy parser (CVector owns them)
        if (!useDTermCore) {
            [token release];
        }
    }
    if (!useDTermCore) {
        CVectorDestroy(&vector);
    }

    return result;
}

// Return an NSArray of NSData's. Each NSData is an array of screen_char_t's,
// with the last element in each being the newline. Returns nil on error.
- (NSArray<NSData *> *)parseDumpHistoryResponse:(NSString *)response
                         ambiguousIsDoubleWidth:(BOOL)ambiguousIsDoubleWidth
                                 unicodeVersion:(NSInteger)unicodeVersion
                                alternateScreen:(BOOL)alternateScreen
                                       rtlFound:(BOOL *)rtlFoundPtr {
    if (rtlFoundPtr) {
        *rtlFoundPtr = NO;
    }
    if (![response length]) {
        return [NSArray array];
    }
    NSArray *lines = [response componentsSeparatedByString:@"\n"];
    NSMutableArray *screenLines = [NSMutableArray arrayWithCapacity:lines.count];
    VT100Terminal *terminal = [[[VT100Terminal alloc] init] autorelease];
    terminal.tmuxMode = YES;
    [terminal setEncoding:NSUTF8StringEncoding];
    for (NSString *line in lines) {
        BOOL rtlFound = NO;
        NSData *data = [self dataForHistoryLine:line
                                   withTerminal:terminal
                         ambiguousIsDoubleWidth:ambiguousIsDoubleWidth
                                 unicodeVersion:unicodeVersion
                                alternateScreen:alternateScreen
                                       rtlFound:&rtlFound];
        if (rtlFoundPtr != nil && rtlFound) {
            *rtlFoundPtr = YES;
        }
        if (!data) {
            return nil;
        }
        [screenLines addObject:data];
    }

    return screenLines;
}

@end
