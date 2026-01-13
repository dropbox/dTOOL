//
//  LineBlockPacked.h
//  DashTerm2
//
//  Created by DashTerm2 AI on 12/17/24.
//
//  Description: Memory-efficient LineBlock variant that stores characters in
//    packed format (8 bytes vs 12 bytes per character). Used for scrollback
//    storage where data is written once and read rarely.
//
//    This is a read-only wrapper around packed character data. It provides
//    the same read interface as LineBlock but does not support mutation.
//    Convert a LineBlock to LineBlockPacked via -packedCopy when the block
//    is sealed (no more writes expected).
//
//    Memory savings: ~33% reduction in character storage
//    Trade-off: ~3.6 μs/line unpack latency when reading

#import <Foundation/Foundation.h>
#import "iTermLineBlockReading.h"
#import "ScreenCharArray.h"
#import "iTermMetadata.h"
#import "iTermFindViewController.h"
#import "FindContext.h"

@class LineBlock;
@class iTermPackedCharacterBuffer;
@class PackedColorTable;
@class LineBlockMetadataArray;
@class iTermBidiDisplayInfo;

NS_ASSUME_NONNULL_BEGIN

/// Read-only LineBlock that stores characters in packed format for memory efficiency.
/// Created from a LineBlock via -packedCopyWithColorTable:.
///
/// Usage:
///   LineBlockPacked *packed = [regularBlock packedCopyWithColorTable:sharedColorTable];
///   // Use packed for reading, same API as LineBlock
///
/// This class is thread-safe for reading once created.
@interface LineBlockPacked : NSObject <iTermLineBlockReading>

#pragma mark - Properties (mirrors LineBlock)

/// Once this is set to true, it stays true. If double width characters are
/// possibly present then a slower algorithm is used to count the number of lines.
@property (nonatomic, readonly) BOOL mayHaveDoubleWidthCharacter;

/// Total number of screen_char_t characters stored.
@property (nonatomic, readonly) int numberOfCharacters;

/// Generation number for change tracking.
@property (nonatomic, readonly) NSInteger generation;

/// The LineBlock this was created from (weak reference).
@property (nonatomic, weak, readonly, nullable) LineBlock *progenitor;

/// Unique identifier.
@property (nonatomic, readonly) NSString *guid;

/// Block number in the line buffer.
@property (nonatomic, readonly) long long absoluteBlockNumber;

/// Unique 0-based counter.
@property (nonatomic, readonly) unsigned int index;

#pragma mark - Memory Statistics

/// Memory used by packed storage (in bytes).
@property (nonatomic, readonly) NSUInteger packedMemoryUsage;

/// Memory that would be used by unpacked storage (in bytes).
@property (nonatomic, readonly) NSUInteger unpackedMemoryUsage;

/// Memory saved compared to unpacked storage (in bytes).
@property (nonatomic, readonly) NSUInteger memorySaved;

#pragma mark - Initialization

/// Create a packed copy from a regular LineBlock.
/// @param lineBlock The source LineBlock to pack.
/// @param colorTable Shared color table for 24-bit colors.
/// @return A new packed LineBlock, or nil if packing failed.
- (nullable instancetype)initWithLineBlock:(LineBlock *)lineBlock
                                colorTable:(nullable PackedColorTable *)colorTable;

- (instancetype)init NS_UNAVAILABLE;

#pragma mark - Line Access (Read-Only)

/// Get the number of lines in this block at a given screen width.
- (int)getNumLinesWithWrapWidth:(int)width;

/// Returns whether getNumLinesWithWrapWidth will be fast.
- (BOOL)hasCachedNumLinesForWidth:(int)width;

/// Returns true if the last raw line does not include a logical newline.
- (BOOL)hasPartial;

/// Returns true if there are no lines in the block.
- (BOOL)isEmpty;

/// Are all lines of length 0? True if there are no lines, as well.
- (BOOL)allLinesAreEmpty;

/// Returns YES if the block contains at least one non-empty line.
@property (nonatomic, readonly) BOOL containsAnyNonEmptyLine;

/// Return the number of raw (unwrapped) lines.
- (int)numRawLines;

/// Return the position of the first used character in the raw buffer.
- (int)startOffset;

/// Return the length of a raw (unwrapped) line.
- (int)lengthOfRawLine:(int)linenum;

/// Try to get a line that is lineNum after the first line in this block after wrapping.
/// Returns characters unpacked on-demand.
- (const screen_char_t * _Nullable)getWrappedLineWithWrapWidth:(int)width
                                                       lineNum:(int *)lineNum
                                                    lineLength:(int *)lineLength
                                             includesEndOfLine:(int *)includesEndOfLine
                                                  continuation:(screen_char_t *)continuationPtr;

/// Extended version with metadata.
- (const screen_char_t * _Nullable)getWrappedLineWithWrapWidth:(int)width
                                                       lineNum:(int *)lineNum
                                                    lineLength:(int *)lineLength
                                             includesEndOfLine:(int *)includesEndOfLine
                                                       yOffset:(int * _Nullable)yOffsetPtr
                                                  continuation:(screen_char_t *)continuationPtr
                                          isStartOfWrappedLine:(BOOL * _Nullable)isStartOfWrappedLine
                                                      metadata:(out iTermImmutableMetadata * _Nullable)metadataPtr;

/// Get a ScreenCharArray for a wrapped line.
- (ScreenCharArray * _Nullable)screenCharArrayForWrappedLineWithWrapWidth:(int)width
                                                                  lineNum:(int)lineNum
                                                                 paddedTo:(int)paddedSize
                                                           eligibleForDWC:(BOOL)eligibleForDWC;

/// Return a raw line (unpacked on-demand).
- (const screen_char_t * _Nullable)rawLine:(int)linenum;

/// Get a ScreenCharArray for a raw line.
- (ScreenCharArray * _Nullable)screenCharArrayForRawLine:(int)linenum;

/// Returns the metadata associated with a line when wrapped to the specified width.
- (iTermImmutableMetadata)metadataForLineNumber:(int)lineNum width:(int)width;

/// Returns the total number of screen_char_t's used.
- (int)rawSpaceUsed;

/// Number of empty lines at the end of the block.
- (int)numberOfTrailingEmptyLines;

/// Number of empty lines at the start of the block.
- (int)numberOfLeadingEmptyLines;

/// Length of the last line when wrapped to width.
- (int)lengthOfLastWrappedLineForWidth:(int)width;

/// Get the raw line at a given wrapped line offset.
- (ScreenCharArray * _Nullable)rawLineAtWrappedLineOffset:(int)lineNum width:(int)width;

/// Convert wrapped line offset to raw line number.
- (BOOL)rawLineNumberAtWrappedLineOffset:(int)lineNum
                                   width:(int)width
                            rawLineNumber:(int *)rawLineNumber;

/// Get bidi info for a line.
- (iTermBidiDisplayInfo * _Nullable)bidiInfoForLineNumber:(int)lineNum width:(int)width;

/// Get last raw line.
- (ScreenCharArray * _Nullable)lastRawLine;

/// Get size from line.
- (NSInteger)sizeFromLine:(int)lineNum width:(int)width;

#pragma mark - Search

/// Searches for a substring, populating results with ResultRange objects.
- (void)findSubstring:(NSString *)substring
              options:(FindOptions)options
                 mode:(iTermFindMode)mode
             atOffset:(int)offset
              results:(NSMutableArray *)results
      multipleResults:(BOOL)multipleResults
includesPartialLastLine:(BOOL *)includesPartialLastLine
        lineProvider:(LineBlockRelativeLineProvider _Nullable)lineProvider;

#pragma mark - Position Conversion

/// Try to convert a byte offset into an x,y coordinate.
- (BOOL)convertPosition:(int)position
              withWidth:(int)width
              wrapOnEOL:(BOOL)wrapOnEOL
                    toX:(int *)x
                    toY:(int *)y;

/// Returns the position of a char at (x, lineNum).
- (int)getPositionOfLine:(int *)lineNum
                     atX:(int)x
               withWidth:(int)width
                 yOffset:(int * _Nullable)yOffsetPtr
                 extends:(BOOL * _Nullable)extendsPtr;

#pragma mark - Serialization

/// Returns a dictionary for serialization.
/// Note: Packed data is stored in a new format (v5) that includes compression.
- (NSDictionary *)dictionary;

#pragma mark - Debug

/// Dump contents for debugging.
- (void)dump:(int)rawOffset droppedChars:(long long)droppedChars toDebugLog:(BOOL)toDebugLog;

/// Appends the contents of the block to |s|.
- (void)appendToDebugString:(NSMutableString *)s;

/// Debug description for a raw line.
- (NSString *)debugStringForRawLine:(int)i;

@end

NS_ASSUME_NONNULL_END

// The category on LineBlock is in LineBlockPacked.mm to avoid circular imports
