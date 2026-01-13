/*
 **  DVRDecoder.m
 **
 **  Copyright 20101
 **
 **  Author: George Nachman
 **
 **  Project: DashTerm2
 **
 **  Description: Decodes the key+diff frame scheme implemented in
 **    DVREncoder. Used by the instant replay feature to load screen
 **    images out of a circular DVRBuffer owned by a DVR.
 **
 **  This program is free software; you can redistribute it and/or modify
 **  it under the terms of the GNU General Public License as published by
 **  the Free Software Foundation; either version 2 of the License, or
 **  (at your option) any later version.
 **
 **  This program is distributed in the hope that it will be useful,
 **  but WITHOUT ANY WARRANTY; without even the implied warranty of
 **  MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 **  GNU General Public License for more details.
 **
 **  You should have received a copy of the GNU General Public License
 **  along with this program; if not, write to the Free Software
 **  Foundation, Inc., 675 Mass Ave, Cambridge, MA 02139, USA.
 */

#import "DVRDecoder.h"

#import "DebugLogging.h"
#import "DVRIndexEntry.h"
#import "iTermExternalAttributeIndex.h"
#import "iTermMalloc.h"
#import "LineBuffer.h"
#include <compression.h>

// #if DEBUG
// #define DVRDEBUG 1
// #endif

@implementation DVRDecoder {
    // Circular buffer not owned by us.
    DVRBuffer *buffer_;

    // Offset of the currently decoded frame in buffer_.
    ptrdiff_t currentFrameOffset_;

    // Most recent frame's metadata.
    DVRFrameInfo info_;

    // Most recent frame plus metadata.
    char *decodedBytes_;

    // Length of frame, including metadata.
    int frameLength_;

    // Most recent frame's key (not timestamp).
    long long key_;

    // Offsets into decodedBytes_. Will have info_.height entries.
    NSMutableArray<NSNumber *> *metadataOffsets_;
}

- (instancetype)initWithBuffer:(DVRBuffer *)buffer {
    self = [super init];
    if (self) {
        // BUG-1264: Retain buffer to prevent use-after-free if buffer is deallocated
        buffer_ = [buffer retain];
        decodedBytes_ = 0;
        frameLength_ = 0;
        key_ = -1;
    }
    return self;
}

- (void)dealloc {
    if (decodedBytes_) {
        free(decodedBytes_);
    }
    // BUG-1264: Release buffer that was retained in init
    [buffer_ release];
    [metadataOffsets_ release];
    [super dealloc];
}

- (BOOL)seek:(long long)timestamp {
    // Use binary search via DVRBuffer's optimized method (O(log n) instead of O(n))
    long long key = [buffer_ firstKeyWithTimestampAfter:timestamp - 1];
    if (key >= 0) {
        [self _seekToEntryWithKey:key];
        return YES;
    }
    return NO;
}

- (int)migrateFromVersion {
    return buffer_.migrateFromVersion;
}

- (char *)decodedFrame {
    return decodedBytes_;
}

- (int)encodedLength {
    return frameLength_;
}

// BUG-10368: Use NSInteger and check for overflow to prevent integer overflow crash
// when width * height * sizeof(screen_char_t) exceeds INT_MAX
- (NSInteger)screenCharArrayLength {
    // Use unsigned 64-bit arithmetic to safely compute the size
    // width and height are int, but we need to prevent overflow
    const NSUInteger widthPlusOne = (NSUInteger)(info_.width + 1);
    const NSUInteger height = (NSUInteger)info_.height;
    const NSUInteger charSize = sizeof(screen_char_t);

    // Check for overflow: if the result would exceed NSIntegerMax, return a safe maximum
    // This is defensive - in practice, terminal sizes are limited to reasonable values
    const NSUInteger maxSafeSize = (NSUInteger)NSIntegerMax;

    // Check widthPlusOne * height for overflow first
    if (height != 0 && widthPlusOne > maxSafeSize / height) {
        return NSIntegerMax; // Overflow would occur
    }
    NSUInteger cellCount = widthPlusOne * height;

    // Check cellCount * charSize for overflow
    if (charSize != 0 && cellCount > maxSafeSize / charSize) {
        return NSIntegerMax; // Overflow would occur
    }

    return (NSInteger)(cellCount * charSize);
}

- (BOOL)next {
    // RC-004 Fix: Return NO on empty buffer
    if ([buffer_ isEmpty]) {
        return NO;
    }
    long long newKey;
    if (key_ == -1) {
        newKey = [buffer_ firstKey];
    } else {
        newKey = key_ + 1;
        if (newKey < [buffer_ firstKey]) {
            newKey = [buffer_ firstKey];
        } else if (newKey > [buffer_ lastKey]) {
            return NO;
        }
    }
    [self _seekToEntryWithKey:newKey];
    return YES;
}

- (BOOL)prev {
    if (key_ <= [buffer_ firstKey]) {
        return NO;
    }
    [self _seekToEntryWithKey:key_ - 1];
    return YES;
}

- (long long)timestamp {
    return info_.timestamp;
}

- (void)invalidateIndex:(long long)i {
    if (i == key_) {
        key_ = -1;
    }
}

- (DVRFrameInfo)info {
    return info_;
}

#pragma mark - Private

- (NSString *)stringForFrame {
    NSMutableString *s = [NSMutableString stringWithCapacity:(info_.width + 1) * info_.height];
    screen_char_t *lines = (screen_char_t *)decodedBytes_;
    int i = 0;
    for (int y = 0; y < info_.height; y++) {
        for (int x = 0; x < info_.width; x++) {
            screen_char_t c = lines[i++];
            [s appendFormat:@"%c", c.code];
        }
        [s appendString:@"\n"];
        i++;
    }
    return s;
}

- (void)debug:(NSString *)prefix buffer:(char *)buffer length:(int)length {
    NSMutableData *temp = [NSMutableData dataWithLength:length];
    char *d = (char *)temp.mutableBytes;
    int i;
    for (i = 0; i * sizeof(screen_char_t) < length; i++) {
        screen_char_t s = ((screen_char_t *)buffer)[i];
        if (s.code && s.complexChar) {
            d[i] = s.code;
        } else {
            d[i] = ' ';
        }
    }
    d[i] = 0;
    DLog(@"%@: \"%s\"", prefix, d);
}


- (void)_seekToEntryWithKey:(long long)key {
#ifdef DVRDEBUG
    NSLog(@"Begin seek to %lld", key);
#endif

    // Make sure key is valid.
    if (key < [buffer_ firstKey] || key > [buffer_ lastKey]) {
#ifdef DVRDEBUG
        NSLog(@"Frame %lld doesn't exist so skipping to %lld", key, [buffer_ firstKey]);
#endif
        key = [buffer_ firstKey];
    }
    // Find the key frame before 'key'. Key frames can be regular or compressed.
    long long j = key;
    const long long firstKey = [buffer_ firstKey];
    // BUG-1265: Check for nil entry before dereferencing
    DVRIndexEntry *entry = [buffer_ entryForKey:j];
    if (!entry) {
        DLog(@"DVRDecoder: entryForKey %lld returned nil, aborting seek", j);
        return;
    }
    DVRFrameType frameType = entry->info.frameType;
    while (frameType != DVRFrameTypeKeyFrame && frameType != DVRFrameTypeCompressedKeyFrame) {
        // BUG-1268: Prevent infinite loop - if we've reached the first key and it's
        // not a keyframe, we cannot decode. This indicates buffer corruption.
        if (j <= firstKey) {
            DLog(@"DVRDecoder: no keyframe found before key %lld (first key %lld is not a keyframe)", key, firstKey);
            return;
        }
        --j;
        entry = [buffer_ entryForKey:j];
        if (!entry) {
            DLog(@"DVRDecoder: entryForKey %lld returned nil during keyframe search", j);
            return;
        }
        frameType = entry->info.frameType;
    }

    [self _loadKeyFrameWithKey:j];

#ifdef DVRDEBUG
    [self debug:@"Key frame:" buffer:decodedBytes_ length:frameLength_];
#endif

    // Apply all the diff frames up to key.
    while (j != key) {
        ++j;
        if (![self _loadDiffFrameWithKey:j]) {
            return;
        }
#ifdef DVRDEBUG
        [self debug:[NSString stringWithFormat:@"After applying diff of %lld:", j]
             buffer:decodedBytes_
             length:frameLength_];
#endif
    }
    key_ = j;
#ifdef DVRDEBUG
    NSLog(@"end seek to %lld", key);
#endif
}

- (void)_loadKeyFrameWithKey:(long long)key {
#ifdef DVRDEBUG
    NSLog(@"DVRDecoder: load frame %@", @(key));
#endif
    [metadataOffsets_ release];
    metadataOffsets_ = [[NSMutableArray alloc] initWithCapacity:info_.height];

    // RC-004: Capture generation before reading to detect concurrent modifications
    const NSUInteger generationBefore = buffer_.structuralGeneration;

    DVRIndexEntry *entry = [buffer_ entryForKey:key];
    // BUG-1266: Validate entry exists before accessing
    if (!entry) {
        DLog(@"DVRDecoder: _loadKeyFrameWithKey entry is nil for key %lld", key);
        return;
    }
    // RC-004: Make local copy of entry info since entry could be deallocated
    DVRFrameInfo localInfo = entry->info;
    int localFrameLength = entry->frameLength;
    info_ = localInfo;

    char *data = [buffer_ blockForKey:key];
    // BUG-1266: Validate data pointer
    if (!data) {
        DLog(@"DVRDecoder: _loadKeyFrameWithKey data is nil for key %lld", key);
        return;
    }

    // RC-004: Check if buffer structure changed while we were reading
    if (buffer_.structuralGeneration != generationBefore) {
        DLog(@"DVRDecoder: buffer structure changed during _loadKeyFrameWithKey for key %lld", key);
        return;
    }

    currentFrameOffset_ = [buffer_ offsetOfPointer:data];

    // Handle compressed key frames (DVRFrameTypeCompressedKeyFrame)
    // RC-004: Use local copies of entry fields to avoid reading from potentially freed entry
    if (localInfo.frameType == DVRFrameTypeCompressedKeyFrame) {
        // BUG-1266: Validate minimum size for compressed frame header
        if (localFrameLength < (int)sizeof(uint32_t)) {
            DLog(@"DVRDecoder: compressed frame too small (%d bytes) for header", localFrameLength);
            return;
        }
        // Format: [uint32_t uncompressed_size][compressed_data...]
        uint32_t uncompressedSize;
        memcpy(&uncompressedSize, data, sizeof(uncompressedSize));

        const char *compressedData = data + sizeof(uncompressedSize);
        const size_t compressedSize = localFrameLength - sizeof(uncompressedSize);

        // Reallocate decoded buffer if needed
        if (frameLength_ != (int)uncompressedSize && decodedBytes_) {
            free(decodedBytes_);
            decodedBytes_ = NULL;
        }
        frameLength_ = (int)uncompressedSize;
        if (!decodedBytes_) {
            decodedBytes_ = iTermMalloc(frameLength_);
        }

        // Decompress using Apple's Compression framework with LZ4
        size_t decodedSize = compression_decode_buffer((uint8_t *)decodedBytes_, uncompressedSize,
                                                       (const uint8_t *)compressedData, compressedSize,
                                                       NULL, // scratch_buffer
                                                       COMPRESSION_LZ4);

        if (decodedSize != uncompressedSize) {
            // BUG-7339: Decompression failed - don't use zeroed data as valid
            DLog(@"DVR: LZ4 decompression failed! Expected %u bytes, got %zu", uncompressedSize, decodedSize);
            // Free corrupted buffer and reset state
            free(decodedBytes_);
            decodedBytes_ = NULL;
            frameLength_ = 0;
            return;
        }
#ifdef DVRDEBUG
        NSLog(@"DVR: Decompressed key frame %zu -> %u bytes", compressedSize, uncompressedSize);
#endif
    } else {
        // Regular uncompressed key frame
        // BUG-1266: Validate frameLength is positive and reasonable
        if (localFrameLength <= 0) {
            DLog(@"DVRDecoder: invalid frameLength %d for key %lld", localFrameLength, key);
            return;
        }
        if (frameLength_ != localFrameLength && decodedBytes_) {
            free(decodedBytes_);
            decodedBytes_ = NULL;
        }
        frameLength_ = localFrameLength;
        if (!decodedBytes_) {
            decodedBytes_ = iTermMalloc(frameLength_);
        }
        memcpy(decodedBytes_, data, frameLength_);
    }

#ifdef DVRDEBUG
    NSLog(@"Frame length is %d", frameLength_);
#endif

    // Parse metadata offsets
    const NSInteger metadataStart = (info_.width + 1) * info_.height * sizeof(screen_char_t);
    NSInteger offset = metadataStart;
    while (offset + sizeof(int) <= frameLength_) {
        int length;
        memmove(&length, decodedBytes_ + offset, sizeof(length));
        if (length < 0) {
            break;
        }
        if (length > 1048576) {
            // This is an artificial limit to prevent overflows and weird behavior on bad input.
            break;
        }
        // For compressed frames, metadata offsets are relative to decodedBytes_
        // For uncompressed frames, they reference the buffer directly
        // RC-004: Use localInfo instead of entry->info
        if (localInfo.frameType == DVRFrameTypeCompressedKeyFrame) {
            // Store a special marker indicating this is a decoded offset
            // We'll need to handle this differently in metadataForLine
            [metadataOffsets_ addObject:@(-(offset + 1))]; // Negative = decoded buffer offset
        } else {
            [metadataOffsets_ addObject:@(currentFrameOffset_ + offset)];
        }
        offset += sizeof(length);
        offset += length;
    }
    DLog(@"Frame with key %lld has size %dx%d", key, info_.width, info_.height);
}

// Add two positive ints. Returns NO if it can't be done. Returns YES and places the result in
// *sum if possible.
static BOOL NS_WARN_UNUSED_RESULT SafeIncr(int summand, int addend, int *sum) {
    if (summand < 0 || addend < 0) {
        DLog(@"Have negative value: summand=%@ addend=%@", @(summand), @(addend));
        return NO;
    }
    // BUG-f1376: Convert assert to _Static_assert - compile-time check is safer
    _Static_assert(sizeof(long long) > sizeof(int), "long long must be larger than int for safe overflow check");
    const long long temp1 = summand;
    const long long temp2 = addend;
    const long long temp3 = temp1 + temp2;
    if (temp3 > INT_MAX) {
        DLog(@"Prevented overflow: summand=%@ addend=%@", @(summand), @(addend));
        return NO;
    }
    *sum = temp3;
    return YES;
}

// Returns NO if the input was broken.
- (BOOL)_loadDiffFrameWithKey:(long long)key {
#ifdef DVRDEBUG
    NSLog(@"Load diff frame with key %lld", key);
#endif
    // RC-004: Capture generation before reading to detect concurrent modifications
    const NSUInteger generationBefore = buffer_.structuralGeneration;

    DVRIndexEntry *entry = [buffer_ entryForKey:key];
    // BUG-1272: Guard against nil entry
    if (!entry) {
        DLog(@"DVRDecoder: _loadDiffFrameWithKey entry is nil for key %lld", key);
        return NO;
    }
    // RC-004: Make local copies of entry fields since entry could be deallocated
    DVRFrameInfo localInfo = entry->info;
    int localFrameLength = entry->frameLength;
    info_ = localInfo;

    char *diff = [buffer_ blockForKey:key];
    // Guard against nil diff data
    if (!diff) {
        DLog(@"DVRDecoder: _loadDiffFrameWithKey diff is nil for key %lld", key);
        return NO;
    }

    // RC-004: Check if buffer structure changed while we were reading
    if (buffer_.structuralGeneration != generationBefore) {
        DLog(@"DVRDecoder: buffer structure changed during _loadDiffFrameWithKey for key %lld", key);
        return NO;
    }

    int o = 0;
    int line = 0;
    if (!metadataOffsets_) {
        metadataOffsets_ = [[NSMutableArray alloc] initWithCapacity:info_.height];
    }
    // RC-004: Use localFrameLength instead of entry->frameLength
    for (int i = 0; i < localFrameLength; line++) {
#ifdef DVRDEBUG
        NSLog(@"Checking line at offset %d, address %p. type=%d", i, diff + i, (int)diff[i]);
#endif
        int n;
        switch (diff[i++]) {
            case kSameSequence:
                memcpy(&n, diff + i, sizeof(n));
                if (!SafeIncr(i, sizeof(n), &i)) {
                    return NO;
                }
#ifdef DVRDEBUG
                NSLog(@"%d bytes of sameness at offset %d", n, i);
#endif
                if (!SafeIncr(n, o, &o)) {
                    return NO;
                }
                break;

            case kDiffSequence:
                memcpy(&n, diff + i, sizeof(n));
                if (!SafeIncr(i, sizeof(n), &i)) {
                    return NO;
                }
                int proposedEnd;
                if (!SafeIncr(o, n, &proposedEnd)) {
                    return NO;
                }
                if (proposedEnd - 1 >= frameLength_) {
                    return NO;
                }
                memcpy(decodedBytes_ + o, diff + i, n);
#ifdef DVRDEBUG
                NSLog(@"%d bytes of difference at offset %d", n, o);
#endif
                if (line >= info_.height) {
                    metadataOffsets_[line - info_.height] = @(currentFrameOffset_ + i);
                }
                if (!SafeIncr(n, o, &o) || !SafeIncr(n, i, &i)) {
                    return NO;
                }
                break;

            default:
                // BUG-1273: Return NO instead of assert(0) which is disabled in release
                DLog(@"Unexpected block type %d at offset %d", (int)diff[i - 1], i - 1);
                return NO;
        }
    }
    return YES;
}

- (int)lengthForMetadataOnLine:(int)line {
    if (line < 0) {
        return 0;
    }
    if (line >= metadataOffsets_.count) {
        return 0;
    }
    const long long storedOffset = [metadataOffsets_[line] longLongValue];
    int result;

    if (storedOffset < 0) {
        // Compressed frame: offset is in decodedBytes_ (stored as -(offset+1))
        const int decodedOffset = (int)(-(storedOffset + 1));
        if (decodedOffset + sizeof(result) > frameLength_) {
            return 0;
        }
        memmove(&result, decodedBytes_ + decodedOffset, sizeof(result));
    } else {
        // Uncompressed frame: offset is in buffer
        NSData *data = [buffer_ dataAtOffset:storedOffset length:sizeof(result)];
        if (!data || data.length != sizeof(result)) {
            return 0;
        }
        memmove(&result, data.bytes, sizeof(result));
    }
    return result;
}

- (int)offsetOfMetadataOnLine:(int)line {
    if (line < 0 || line >= metadataOffsets_.count) {
        return -1;
    }
    const long long storedOffset = [metadataOffsets_[line] longLongValue];
    if (storedOffset < 0) {
        // Return the decoded buffer offset (positive)
        return (int)(-(storedOffset + 1));
    }
    return (int)storedOffset;
}

- (NSData *)metadataForLine:(int)line {
    if (line < 0 || line >= metadataOffsets_.count) {
        return nil;
    }
    const long long storedOffset = [metadataOffsets_[line] longLongValue];
    const int length = [self lengthForMetadataOnLine:line];
    if (length <= 0) {
        return nil;
    }

    if (storedOffset < 0) {
        // Compressed frame: read from decodedBytes_
        const int decodedOffset = (int)(-(storedOffset + 1));
        const int dataStart = decodedOffset + sizeof(int);
        if (dataStart + length > frameLength_) {
            return nil;
        }
        return [NSData dataWithBytes:decodedBytes_ + dataStart length:length];
    } else {
        // Uncompressed frame: read from buffer
        return [buffer_ dataAtOffset:storedOffset + sizeof(int) length:length];
    }
}

@end
