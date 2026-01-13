//
//  iTermLineBlockReading.h
//  DashTerm2SharedARC
//
//  Created by DashTerm2 AI on 12/17/24.
//
//  Description: Protocol defining read-only interface for LineBlock and LineBlockPacked.
//    This allows iTermLineBlockArray to store either type transparently.

#import <Foundation/Foundation.h>
#import "ScreenCharArray.h"
#import "iTermMetadata.h"
#import "FindContext.h"
#import "iTermFindViewController.h"

@class iTermBidiDisplayInfo;
@class LineBlock;

typedef BOOL (^LineBlockRelativeLineProvider)(LineBlock *startBlock,
                                              int startEntry,
                                              int relativeLineIndex,
                                              LineBlock *__autoreleasing _Nullable *outBlock,
                                              int *_Nullable outEntry);

NS_ASSUME_NONNULL_BEGIN

/// Protocol defining read-only interface for line blocks.
/// Both LineBlock and LineBlockPacked conform to this protocol.
@protocol iTermLineBlockReading <NSObject>

#pragma mark - Properties

/// Once this is set to true, it stays true. If double width characters are
/// possibly present then a slower algorithm is used to count the number of lines.
@property (nonatomic, readonly) BOOL mayHaveDoubleWidthCharacter;

/// Total number of screen_char_t characters stored.
@property (nonatomic, readonly) int numberOfCharacters;

/// Generation number for change tracking.
@property (nonatomic, readonly) NSInteger generation;

/// Unique 0-based counter.
@property (nonatomic, readonly) unsigned int index;

/// Block number in the line buffer.
@property (nonatomic, readonly) long long absoluteBlockNumber;

/// Returns true if there are no lines in the block.
@property (nonatomic, readonly) BOOL isEmpty;

/// Are all lines of length 0? True if there are no lines, as well.
@property (nonatomic, readonly) BOOL allLinesAreEmpty;

/// Returns YES if the block contains at least one non-empty line.
@property (nonatomic, readonly) BOOL containsAnyNonEmptyLine;

/// Returns true if the last raw line does not include a logical newline.
@property (nonatomic, readonly) BOOL hasPartial;

#pragma mark - Line Counting

/// Get the number of lines in this block at a given screen width.
- (int)getNumLinesWithWrapWidth:(int)width;

/// Returns whether getNumLinesWithWrapWidth will be fast.
- (BOOL)hasCachedNumLinesForWidth:(int)width;

/// Return the number of raw (unwrapped) lines.
- (int)numRawLines;

/// Returns the total number of screen_char_t's used.
- (int)rawSpaceUsed;

/// Number of empty lines at the end of the block.
- (int)numberOfTrailingEmptyLines;

/// Number of empty lines at the start of the block.
- (int)numberOfLeadingEmptyLines;

/// Return the position of the first used character in the raw buffer.
- (int)startOffset;

/// Return the length of a raw (unwrapped) line.
- (int)lengthOfRawLine:(int)linenum;

/// Length of the last line when wrapped to width.
- (int)lengthOfLastWrappedLineForWidth:(int)width;

#pragma mark - Line Access

/// Try to get a line that is lineNum after the first line in this block after wrapping.
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

/// Return a raw line.
- (const screen_char_t * _Nullable)rawLine:(int)linenum;

/// Get a ScreenCharArray for a raw line.
- (ScreenCharArray * _Nullable)screenCharArrayForRawLine:(int)linenum;

/// Returns the metadata associated with a line when wrapped to the specified width.
- (iTermImmutableMetadata)metadataForLineNumber:(int)lineNum width:(int)width;

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

#pragma mark - Serialization

/// Returns a dictionary for serialization.
- (NSDictionary *)dictionary;

#pragma mark - Debug

/// Dump contents for debugging.
- (void)dump:(int)rawOffset droppedChars:(long long)droppedChars toDebugLog:(BOOL)toDebugLog;

/// Appends the contents of the block to |s|.
- (void)appendToDebugString:(NSMutableString *)s;

/// Debug description for a raw line.
- (NSString *)debugStringForRawLine:(int)i;

#pragma mark - Size Computation

/// Get size from line.
- (NSInteger)sizeFromLine:(int)lineNum width:(int)width;

@end

NS_ASSUME_NONNULL_END
