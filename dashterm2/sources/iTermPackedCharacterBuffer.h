//
//  iTermPackedCharacterBuffer.h
//  DashTerm2SharedARC
//
//  Created by DashTerm2 AI on 12/17/24.
//
//  Description: Memory-efficient storage for screen characters in scrollback.
//    Uses packed_screen_char_t (8 bytes) instead of screen_char_t (12 bytes)
//    for 33% memory savings. Converts to/from unpacked format on demand.
//

#import <Foundation/Foundation.h>
#import "ScreenChar.h"
#import "PackedScreenChar.h"

NS_ASSUME_NONNULL_BEGIN

/// Storage class for packed screen characters.
/// Provides a similar interface to iTermCharacterBuffer but stores characters
/// in packed format (8 bytes vs 12 bytes) for memory efficiency.
///
/// Use for scrollback storage where characters are written once and read rarely.
/// The pack/unpack overhead is negligible compared to memory savings.
@interface iTermPackedCharacterBuffer : NSObject

/// Number of characters this buffer can hold.
@property (nonatomic, readonly) int size;

/// The color table used for 24-bit color storage.
/// Shared across related buffers for efficiency.
@property (nonatomic, strong, readonly) PackedColorTable *colorTable;

/// Memory used in bytes (packed storage).
@property (nonatomic, readonly) NSUInteger memoryUsage;

/// Memory that would be used by unpacked storage.
@property (nonatomic, readonly) NSUInteger unpackedMemoryUsage;

/// Memory saved compared to unpacked storage.
@property (nonatomic, readonly) NSUInteger memorySaved;

/// Create a new buffer with the given capacity.
/// @param size Number of characters the buffer can hold.
/// @param colorTable Color table for 24-bit colors (will create one if nil).
- (instancetype)initWithSize:(int)size colorTable:(nullable PackedColorTable *)colorTable;

/// Create from existing packed data.
/// @param data Packed character data.
/// @param colorTable Color table for 24-bit colors.
- (instancetype)initWithPackedData:(NSData *)data colorTable:(PackedColorTable *)colorTable;

/// Create from existing unpacked characters (will pack them).
/// @param chars Unpacked screen characters.
/// @param count Number of characters.
/// @param colorTable Color table for 24-bit colors.
- (instancetype)initWithChars:(const screen_char_t *)chars
                         size:(int)count
                   colorTable:(nullable PackedColorTable *)colorTable;

- (instancetype)init NS_UNAVAILABLE;

#pragma mark - Writing (Packing)

/// Write unpacked characters to the buffer at a given offset.
/// Characters are packed before storage.
/// @param chars Unpacked characters to write.
/// @param count Number of characters.
/// @param offset Offset in the buffer to write to.
- (void)writeChars:(const screen_char_t *)chars count:(int)count atOffset:(int)offset;

/// Append unpacked characters to the end of valid data.
/// @param chars Unpacked characters to append.
/// @param count Number of characters.
/// @param offset Current end position in the buffer.
- (void)appendChars:(const screen_char_t *)chars count:(int)count atOffset:(int)offset;

#pragma mark - Reading (Unpacking)

/// Read and unpack a single character.
/// @param offset Position in the buffer.
/// @return Unpacked screen character.
- (screen_char_t)charAtOffset:(int)offset;

/// Read and unpack characters into a provided buffer.
/// @param dst Destination buffer for unpacked characters.
/// @param count Number of characters to read.
/// @param offset Starting position in the packed buffer.
- (void)readChars:(screen_char_t *)dst count:(int)count fromOffset:(int)offset;

/// Get a newly allocated array of unpacked characters for a range.
/// Caller is responsible for freeing the returned memory.
/// @param offset Starting position.
/// @param count Number of characters.
/// @return Newly allocated array (caller must free).
- (screen_char_t *)copyCharsFromOffset:(int)offset count:(int)count;

/// Read characters into an existing ScreenCharArray's buffer.
/// @param array The array to read into.
/// @param offset Starting position in packed buffer.
/// @param count Number of characters to read.
- (void)readIntoScreenCharArray:(ScreenCharArray *)array
                     fromOffset:(int)offset
                          count:(int)count;

#pragma mark - Buffer Management

/// Resize the buffer.
/// @param newSize New capacity.
- (void)resize:(int)newSize;

/// Create a deep copy of this buffer.
- (iTermPackedCharacterBuffer *)clone;

/// Compare for equality with another buffer.
- (BOOL)deepIsEqual:(id)object;

#pragma mark - Raw Access (For Advanced Use)

/// Direct pointer to packed data (read-only).
/// Use with caution - caller must understand packed format.
@property (nonatomic, readonly) const packed_screen_char_t *packedPointer;

/// Mutable pointer to packed data.
/// Use with caution - modifying packed data directly requires understanding the format.
@property (nonatomic, readonly) packed_screen_char_t *mutablePackedPointer;

/// Get raw packed data for serialization.
@property (nonatomic, readonly) NSData *packedData;

#pragma mark - Description

/// Debug description.
@property (nonatomic, readonly) NSString *shortDescription;

@end

NS_ASSUME_NONNULL_END
