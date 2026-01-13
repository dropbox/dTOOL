//
//  LineBlockPacked.mm
//  DashTerm2SharedARC
//
//  Created by DashTerm2 AI on 12/17/24.
//
//  Description: Memory-efficient LineBlock implementation using packed storage.
//    Stores screen_char_t in 8-byte format instead of 12-byte format.
//    Provides read-only access with on-demand unpacking.

extern "C" {
#import "LineBlockPacked.h"
#import "LineBlock.h"
#import "LineBlock+Private.h"
#import "iTermCharacterBuffer.h"
#import "iTermPackedCharacterBuffer.h"
#import "PackedScreenChar.h"
#import "LineBlockMetadataArray.h"
#import "iTermMalloc.h"
#import "ScreenChar.h"
#import "DebugLogging.h"
}

#include <vector>
#include <mutex>

// Thread-local buffer for unpacking lines on-demand
// This avoids allocation overhead for frequent line reads
static thread_local std::vector<screen_char_t> tls_unpackBuffer;

@implementation LineBlockPacked {
    // Packed character storage
    iTermPackedCharacterBuffer *_packedBuffer;

    // Start offset in the buffer (mirrors LineBlock._startOffset)
    int _startOffset;

    // Cumulative line lengths (same as LineBlock)
    int *_cumulativeLineLengths;
    int _cllCapacity;
    int _cllEntries;
    int _firstEntry;

    // Metadata for each line (copied from source LineBlock)
    LineBlockMetadataArray *_metadataArray;

    // Cached line count for a specific width
    int _cachedNumlines;
    int _cachedNumlinesWidth;

    // Whether the last line is partial
    BOOL _isPartial;

    // Properties
    BOOL _mayHaveDoubleWidthCharacter;
    NSString *_guid;
    long long _absoluteBlockNumber;
    unsigned int _index;
    NSInteger _generation;
    __weak LineBlock *_progenitor;
}

#pragma mark - Initialization

- (nullable instancetype)initWithLineBlock:(LineBlock *)lineBlock colorTable:(nullable PackedColorTable *)colorTable {
    self = [super init];
    if (self) {
        if (!lineBlock) {
            return nil;
        }

        // Copy basic properties
        _mayHaveDoubleWidthCharacter = lineBlock.mayHaveDoubleWidthCharacter;
        _guid = [lineBlock->_guid copy];
        _absoluteBlockNumber = lineBlock.absoluteBlockNumber;
        _index = lineBlock.index;
        _generation = lineBlock.generation;
        _progenitor = lineBlock;

        // Copy line structure
        _firstEntry = lineBlock->_firstEntry;
        _cllEntries = lineBlock->cll_entries;
        _cllCapacity = lineBlock->cll_capacity;
        _startOffset = [lineBlock bufferStartOffset];
        _isPartial = lineBlock->is_partial;
        _cachedNumlines = lineBlock->cached_numlines;
        _cachedNumlinesWidth = lineBlock->cached_numlines_width;

        // Copy cumulative line lengths
        if (_cllCapacity > 0) {
            _cumulativeLineLengths = (int *)iTermMalloc(sizeof(int) * _cllCapacity);
            memcpy(_cumulativeLineLengths, lineBlock->cumulative_line_lengths, sizeof(int) * _cllCapacity);
        }

        // Copy metadata
        _metadataArray = [lineBlock->_metadataArray copy];

        // Pack the character data
        iTermCharacterBuffer *sourceBuffer = lineBlock->_characterBuffer;
        if (!sourceBuffer) {
            return nil;
        }

        const screen_char_t *sourceChars = sourceBuffer.pointer;
        int charCount = sourceBuffer.size;

        // Create packed buffer from source characters
        _packedBuffer = [[iTermPackedCharacterBuffer alloc] initWithChars:sourceChars
                                                                     size:charCount
                                                               colorTable:colorTable];
    }
    return self;
}

- (void)dealloc {
    if (_cumulativeLineLengths) {
        free(_cumulativeLineLengths);
    }
}

#pragma mark - Properties

- (BOOL)mayHaveDoubleWidthCharacter {
    return _mayHaveDoubleWidthCharacter;
}

- (int)numberOfCharacters {
    return _packedBuffer.size;
}

- (NSInteger)generation {
    return _generation;
}

- (LineBlock *)progenitor {
    return _progenitor;
}

- (NSString *)guid {
    return _guid;
}

- (long long)absoluteBlockNumber {
    return _absoluteBlockNumber;
}

- (unsigned int)index {
    return _index;
}

#pragma mark - Memory Statistics

- (NSUInteger)packedMemoryUsage {
    // Estimate object overhead (~256 bytes for instance vars + object overhead)
    NSUInteger base = 256;
    base += _packedBuffer.memoryUsage;
    base += _cllCapacity * sizeof(int);
    // Metadata array is harder to measure exactly
    return base;
}

- (NSUInteger)unpackedMemoryUsage {
    // Estimate LineBlock overhead (~256 bytes)
    NSUInteger base = 256;
    base += _packedBuffer.unpackedMemoryUsage;
    base += _cllCapacity * sizeof(int);
    return base;
}

- (NSUInteger)memorySaved {
    return _packedBuffer.memorySaved;
}

#pragma mark - Internal Helpers

// Get the offset of a raw line in the buffer
- (int)_lineRawOffset:(int)lineIndex {
    if (lineIndex == _firstEntry) {
        return _startOffset;
    }
    return _cumulativeLineLengths[lineIndex - 1];
}

// Get the length of a raw line
- (int)_lineLength:(int)lineIndex {
    int offset = [self _lineRawOffset:lineIndex];
    int end = _cumulativeLineLengths[lineIndex];
    return end - offset;
}

// Ensure the TLS buffer has enough space
- (screen_char_t *)_unpackBufferWithCapacity:(int)count {
    if ((int)tls_unpackBuffer.size() < count) {
        tls_unpackBuffer.resize(count);
    }
    return tls_unpackBuffer.data();
}

// Unpack a range of characters into the TLS buffer
- (const screen_char_t *)_unpackCharsFromOffset:(int)offset count:(int)count {
    screen_char_t *buffer = [self _unpackBufferWithCapacity:count];
    [_packedBuffer readChars:buffer count:count fromOffset:offset];
    return buffer;
}

#pragma mark - Line Access

- (int)getNumLinesWithWrapWidth:(int)width {
    if (width == _cachedNumlinesWidth) {
        return _cachedNumlines;
    }

    // Calculate wrapped line count
    // This is a simplified version - for full implementation we'd need the
    // complex wrapping logic from LineBlock
    int total = 0;
    for (int i = _firstEntry; i < _cllEntries; i++) {
        int lineLength = [self _lineLength:i];
        if (lineLength == 0) {
            total += 1; // Empty line still counts
        } else {
            total += (lineLength + width - 1) / width; // Ceiling division
        }
    }

    return total;
}

- (BOOL)hasCachedNumLinesForWidth:(int)width {
    return width == _cachedNumlinesWidth;
}

- (BOOL)hasPartial {
    return _isPartial;
}

- (BOOL)isEmpty {
    return _cllEntries <= _firstEntry;
}

- (BOOL)allLinesAreEmpty {
    if ([self isEmpty]) {
        return YES;
    }
    for (int i = _firstEntry; i < _cllEntries; i++) {
        if ([self _lineLength:i] > 0) {
            return NO;
        }
    }
    return YES;
}

- (int)numRawLines {
    return _cllEntries - _firstEntry;
}

- (int)startOffset {
    return _startOffset;
}

- (int)lengthOfRawLine:(int)linenum {
    int index = linenum + _firstEntry;
    if (index < _firstEntry || index >= _cllEntries) {
        return 0;
    }
    return [self _lineLength:index];
}

- (const screen_char_t *)getWrappedLineWithWrapWidth:(int)width
                                             lineNum:(int *)lineNum
                                          lineLength:(int *)lineLength
                                   includesEndOfLine:(int *)includesEndOfLine
                                        continuation:(screen_char_t *)continuationPtr {
    return [self getWrappedLineWithWrapWidth:width
                                     lineNum:lineNum
                                  lineLength:lineLength
                           includesEndOfLine:includesEndOfLine
                                     yOffset:NULL
                                continuation:continuationPtr
                        isStartOfWrappedLine:NULL
                                    metadata:NULL];
}

- (const screen_char_t *)getWrappedLineWithWrapWidth:(int)width
                                             lineNum:(int *)lineNum
                                          lineLength:(int *)lineLength
                                   includesEndOfLine:(int *)includesEndOfLine
                                             yOffset:(int *)yOffsetPtr
                                        continuation:(screen_char_t *)continuationPtr
                                isStartOfWrappedLine:(BOOL *)isStartOfWrappedLine
                                            metadata:(out iTermImmutableMetadata *)metadataPtr {
    if (!lineNum || *lineNum < 0) {
        return NULL;
    }

    int targetLine = *lineNum;
    int currentLine = 0;

    // Iterate through raw lines to find the wrapped line
    for (int i = _firstEntry; i < _cllEntries; i++) {
        int rawLength = [self _lineLength:i];
        int rawOffset = [self _lineRawOffset:i];
        int wrappedCount = rawLength == 0 ? 1 : (rawLength + width - 1) / width;

        if (currentLine + wrappedCount > targetLine) {
            // This raw line contains the target wrapped line
            int withinLine = targetLine - currentLine;
            int startInRaw = withinLine * width;
            int endInRaw = MIN(startInRaw + width, rawLength);
            int thisLineLength = endInRaw - startInRaw;

            // Unpack the needed portion
            const screen_char_t *chars = [self _unpackCharsFromOffset:rawOffset + startInRaw count:thisLineLength];

            if (lineLength)
                *lineLength = thisLineLength;
            if (includesEndOfLine)
                *includesEndOfLine = (endInRaw >= rawLength);
            if (isStartOfWrappedLine)
                *isStartOfWrappedLine = (withinLine == 0);
            if (yOffsetPtr)
                *yOffsetPtr = 0;

            if (continuationPtr && _metadataArray) {
                const LineBlockMetadata *meta = [_metadataArray metadataAtIndex:i];
                if (meta) {
                    *continuationPtr = meta->continuation;
                }
            }

            if (metadataPtr && _metadataArray) {
                *metadataPtr = [_metadataArray immutableLineMetadataAtIndex:i];
            }

            // Update lineNum to indicate we found it
            *lineNum = 0;
            return chars;
        }

        currentLine += wrappedCount;
    }

    // Line not found in this block
    *lineNum = *lineNum - currentLine;
    return NULL;
}

- (ScreenCharArray *)screenCharArrayForWrappedLineWithWrapWidth:(int)width
                                                        lineNum:(int)lineNum
                                                       paddedTo:(int)paddedSize
                                                 eligibleForDWC:(BOOL)eligibleForDWC {
    int tempLineNum = lineNum;
    int lineLength = 0;
    int includesEOL = 0;
    iTermImmutableMetadata metadata = iTermImmutableMetadataDefault();
    screen_char_t continuation = {0};

    const screen_char_t *chars = [self getWrappedLineWithWrapWidth:width
                                                           lineNum:&tempLineNum
                                                        lineLength:&lineLength
                                                 includesEndOfLine:&includesEOL
                                                           yOffset:NULL
                                                      continuation:&continuation
                                              isStartOfWrappedLine:NULL
                                                          metadata:&metadata];

    if (!chars || tempLineNum != 0) {
        return nil;
    }

    // Create a copy since our buffer is thread-local
    ScreenCharArray *array = [[ScreenCharArray alloc] initWithCopyOfLine:chars
                                                                  length:lineLength
                                                            continuation:continuation];

    if (paddedSize > lineLength) {
        array = [array paddedToLength:paddedSize eligibleForDWC:eligibleForDWC];
    }

    return array;
}

- (const screen_char_t *)rawLine:(int)linenum {
    int index = linenum + _firstEntry;
    if (index < _firstEntry || index >= _cllEntries) {
        return NULL;
    }

    int offset = [self _lineRawOffset:index];
    int length = [self _lineLength:index];

    return [self _unpackCharsFromOffset:offset count:length];
}

- (ScreenCharArray *)screenCharArrayForRawLine:(int)linenum {
    int index = linenum + _firstEntry;
    if (index < _firstEntry || index >= _cllEntries) {
        return nil;
    }

    int offset = [self _lineRawOffset:index];
    int length = [self _lineLength:index];

    const screen_char_t *chars = [self _unpackCharsFromOffset:offset count:length];

    screen_char_t continuation = {0};
    if (_metadataArray) {
        const LineBlockMetadata *meta = [_metadataArray metadataAtIndex:index];
        if (meta) {
            continuation = meta->continuation;
        }
    }

    return [[ScreenCharArray alloc] initWithCopyOfLine:chars length:length continuation:continuation];
}

- (iTermImmutableMetadata)metadataForLineNumber:(int)lineNum width:(int)width {
    // Find which raw line contains this wrapped line
    int currentLine = 0;

    for (int i = _firstEntry; i < _cllEntries; i++) {
        int rawLength = [self _lineLength:i];
        int wrappedCount = rawLength == 0 ? 1 : (rawLength + width - 1) / width;

        if (currentLine + wrappedCount > lineNum) {
            if (_metadataArray) {
                return [_metadataArray immutableLineMetadataAtIndex:i];
            }
            break;
        }

        currentLine += wrappedCount;
    }

    return iTermImmutableMetadataDefault();
}

- (int)rawSpaceUsed {
    if (_cllEntries == 0) {
        return 0;
    }
    return _cumulativeLineLengths[_cllEntries - 1];
}

- (BOOL)containsAnyNonEmptyLine {
    return ![self allLinesAreEmpty];
}

- (int)numberOfTrailingEmptyLines {
    int count = 0;
    for (int i = _cllEntries - 1; i >= _firstEntry; i--) {
        if ([self _lineLength:i] == 0) {
            count++;
        } else {
            break;
        }
    }
    return count;
}

- (int)numberOfLeadingEmptyLines {
    int count = 0;
    for (int i = _firstEntry; i < _cllEntries; i++) {
        if ([self _lineLength:i] == 0) {
            count++;
        } else {
            break;
        }
    }
    return count;
}

- (int)lengthOfLastWrappedLineForWidth:(int)width {
    if (_cllEntries == 0) {
        return 0;
    }
    int lastLineIndex = _cllEntries - 1;
    int lineLength = [self _lineLength:lastLineIndex];
    if (lineLength == 0) {
        return 0;
    }
    return lineLength % width ?: width;
}

- (ScreenCharArray *)rawLineAtWrappedLineOffset:(int)lineNum width:(int)width {
    int rawLineNum = 0;
    if (![self rawLineNumberAtWrappedLineOffset:lineNum width:width rawLineNumber:&rawLineNum]) {
        return nil;
    }
    return [self screenCharArrayForRawLine:rawLineNum];
}

- (BOOL)rawLineNumberAtWrappedLineOffset:(int)lineNum width:(int)width rawLineNumber:(int *)rawLineNumber {
    int wrappedLines = 0;
    for (int i = _firstEntry; i < _cllEntries; i++) {
        int lineLength = [self _lineLength:i];
        int linesForThis = MAX(1, (lineLength + width - 1) / width);
        if (wrappedLines + linesForThis > lineNum) {
            if (rawLineNumber) {
                *rawLineNumber = i - _firstEntry;
            }
            return YES;
        }
        wrappedLines += linesForThis;
    }
    return NO;
}

- (iTermBidiDisplayInfo *)bidiInfoForLineNumber:(int)lineNum width:(int)width {
    // Packed blocks don't store bidi info - return nil
    // The original LineBlock would have computed this
    return nil;
}

- (ScreenCharArray *)lastRawLine {
    if (_cllEntries == 0) {
        return nil;
    }
    return [self screenCharArrayForRawLine:_cllEntries - _firstEntry - 1];
}

- (NSInteger)sizeFromLine:(int)lineNum width:(int)width {
    // Calculate the number of characters from lineNum to the end
    int rawLineNum = 0;
    if (![self rawLineNumberAtWrappedLineOffset:lineNum width:width rawLineNumber:&rawLineNum]) {
        return 0;
    }
    int rawIndex = rawLineNum + _firstEntry;
    int startOffset = [self _lineRawOffset:rawIndex];
    int totalChars = [self rawSpaceUsed];
    return totalChars - startOffset;
}

#pragma mark - Search

- (void)findSubstring:(NSString *)substring
                    options:(FindOptions)options
                       mode:(iTermFindMode)mode
                   atOffset:(int)offset
                    results:(NSMutableArray *)results
            multipleResults:(BOOL)multipleResults
    includesPartialLastLine:(BOOL *)includesPartialLastLine
               lineProvider:(LineBlockRelativeLineProvider)lineProvider {
    // For search, we need to unpack the entire buffer
    // This is less efficient but searches are relatively rare
    int totalChars = _packedBuffer.size;
    if (totalChars == 0) {
        return;
    }

    screen_char_t *allChars = [_packedBuffer copyCharsFromOffset:0 count:totalChars];
    if (!allChars) {
        return;
    }

    // Create a temporary regular LineBlock for searching
    // This is a simplification - a full implementation would do the search directly
    // For now, we delegate to the original if available
    LineBlock *original = _progenitor;
    if (original) {
        [original findSubstring:substring
                            options:options
                               mode:mode
                           atOffset:offset
                            results:results
                    multipleResults:multipleResults
            includesPartialLastLine:includesPartialLastLine
                       lineProvider:lineProvider];
    }

    free(allChars);
}

#pragma mark - Position Conversion

- (BOOL)convertPosition:(int)position withWidth:(int)width wrapOnEOL:(BOOL)wrapOnEOL toX:(int *)x toY:(int *)y {
    if (position < _startOffset) {
        return NO;
    }

    int currentY = 0;

    for (int i = _firstEntry; i < _cllEntries; i++) {
        int lineStart = [self _lineRawOffset:i];
        int lineEnd = _cumulativeLineLengths[i];
        int lineLength = lineEnd - lineStart;

        if (position < lineEnd || (position == lineEnd && !wrapOnEOL)) {
            // Position is within this line
            int posInLine = position - lineStart;

            // Account for wrapping
            int wrappedLine = posInLine / width;
            int xPos = posInLine % width;

            if (x)
                *x = xPos;
            if (y)
                *y = currentY + wrappedLine;
            return YES;
        }

        // Add wrapped lines for this raw line
        int wrappedCount = lineLength == 0 ? 1 : (lineLength + width - 1) / width;
        currentY += wrappedCount;
    }

    return NO;
}

- (int)getPositionOfLine:(int *)lineNum
                     atX:(int)x
               withWidth:(int)width
                 yOffset:(int *)yOffsetPtr
                 extends:(BOOL *)extendsPtr {
    if (!lineNum) {
        return -1;
    }

    int targetLine = *lineNum;
    int currentLine = 0;

    for (int i = _firstEntry; i < _cllEntries; i++) {
        int rawLength = [self _lineLength:i];
        int rawOffset = [self _lineRawOffset:i];
        int wrappedCount = rawLength == 0 ? 1 : (rawLength + width - 1) / width;

        if (currentLine + wrappedCount > targetLine) {
            int withinLine = targetLine - currentLine;
            int position = rawOffset + (withinLine * width) + x;

            if (yOffsetPtr)
                *yOffsetPtr = 0;
            if (extendsPtr)
                *extendsPtr = (x >= width);

            *lineNum = 0;
            return position;
        }

        currentLine += wrappedCount;
    }

    *lineNum = *lineNum - currentLine;
    return -1;
}

#pragma mark - Serialization

- (NSDictionary *)dictionary {
    // Store in a new packed format (v5)
    // Note: We store as unpacked v3 format for compatibility
    // In a future version, we could store packed data directly for even more savings

    NSMutableArray *cllArray = [NSMutableArray arrayWithCapacity:_cllCapacity];
    for (int i = 0; i < _cllEntries; i++) {
        [cllArray addObject:@(_cumulativeLineLengths[i])];
    }

    // Get encoded metadata from the array
    NSArray *metadataEncoded = [_metadataArray encodedArray] ?: @[];

    // For now, unpack and store in v3 format for backwards compatibility
    // This is less efficient but ensures saved sessions can be loaded by regular DashTerm2
    int totalChars = _packedBuffer.size;
    screen_char_t *unpackedChars = [_packedBuffer copyCharsFromOffset:0 count:totalChars];
    NSData *rawData = nil;
    if (unpackedChars) {
        rawData = [NSData dataWithBytesNoCopy:unpackedChars length:totalChars * sizeof(screen_char_t) freeWhenDone:YES];
    } else {
        rawData = [NSData data];
    }

    return @{
        @"Raw Buffer v3" : rawData, // Store as v3 for compatibility
        @"Buffer Start Offset" : @(_startOffset),
        @"Start Offset" : @(_startOffset),
        @"First Entry" : @(_firstEntry),
        @"Buffer Size" : @(_packedBuffer.size),
        @"Cumulative Line Lengths" : cllArray,
        @"Metadata" : metadataEncoded,
        @"Is Partial" : @(_isPartial),
        @"May Have Double Width Character" : @(_mayHaveDoubleWidthCharacter),
        @"GUID" : _guid ?: @""
    };
}

#pragma mark - Debug

- (void)dump:(int)rawOffset droppedChars:(long long)droppedChars toDebugLog:(BOOL)toDebugLog {
    // Estimate: header (~80 chars) + numLines * ~60 chars per line
    NSMutableString *s = [NSMutableString stringWithCapacity:80 + (_cllEntries - _firstEntry) * 60];
    [self appendToDebugString:s];

    if (toDebugLog) {
        DLog(@"%@", s);
    } else {
        NSLog(@"%@", s);
    }
}

- (void)appendToDebugString:(NSMutableString *)s {
    [s appendFormat:@"LineBlockPacked[%u] abs=%lld packed=%lu saved=%lu\n", _index, _absoluteBlockNumber,
                    (unsigned long)_packedBuffer.memoryUsage, (unsigned long)self.memorySaved];

    for (int i = _firstEntry; i < _cllEntries; i++) {
        [s appendFormat:@"  Line %d: %@\n", i - _firstEntry, [self debugStringForRawLine:i - _firstEntry]];
    }
}

- (NSString *)debugStringForRawLine:(int)linenum {
    int index = linenum + _firstEntry;
    if (index < _firstEntry || index >= _cllEntries) {
        return @"<invalid>";
    }

    int length = [self _lineLength:index];
    if (length == 0) {
        return @"<empty>";
    }

    const screen_char_t *chars = [self rawLine:linenum];
    if (!chars) {
        return @"<unpack failed>";
    }

    NSMutableString *result = [NSMutableString stringWithCapacity:length];
    for (int i = 0; i < MIN(length, 80); i++) {
        unichar c = chars[i].code;
        if (c == 0) {
            [result appendString:@" "];
        } else if (c < 32) {
            [result appendString:@"?"];
        } else {
            [result appendFormat:@"%C", c];
        }
    }

    if (length > 80) {
        [result appendFormat:@"... (%d more)", length - 80];
    }

    return result;
}

@end

#pragma mark - LineBlock Extension

@implementation LineBlock (Packing)

- (nullable LineBlockPacked *)packedCopyWithColorTable:(nullable PackedColorTable *)colorTable {
    return [[LineBlockPacked alloc] initWithLineBlock:self colorTable:colorTable];
}

@end
