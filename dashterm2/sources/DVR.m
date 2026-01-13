// -*- mode:objc -*-
/*
 **  DVR.m
 **
 **  Copyright 2010
 **
 **  Author: George Nachman
 **
 **  Project: DashTerm2
 **
 **  Description: Implements a "digital video recorder" for DashTerm2.
 **    This is used by the "instant replay" feature to record and
 **    play back the screen contents.
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

#import "DVR.h"
#import "DebugLogging.h"
#import "DVRIndexEntry.h"
#import "NSData+iTerm.h"
#import "ScreenChar.h"
#import "iTermMetadata.h"
#include <sys/time.h>

@implementation DVR {
    DVRBuffer *buffer_;
    int capacity_;
    NSMutableArray *decoders_;
    DVREncoder *encoder_;
    dispatch_source_t _memoryPressureSource;
}

@synthesize readOnly = readOnly_;

- (instancetype)initWithBufferCapacity:(int)bytes {
    self = [super init];
    if (self) {
        // BUG-1263: Fixed improper init pattern - must capture return value from init
        buffer_ = [[DVRBuffer alloc] initWithBufferCapacity:bytes];
        capacity_ = bytes;
        decoders_ = [[NSMutableArray alloc] initWithCapacity:4];  // DVR decoders
        encoder_ = [[DVREncoder alloc] initWithBuffer:buffer_];
        [self setupMemoryPressureHandler];
    }
    return self;
}

- (void)setupMemoryPressureHandler {
    _memoryPressureSource = dispatch_source_create(DISPATCH_SOURCE_TYPE_MEMORYPRESSURE, 0,
                                                   DISPATCH_MEMORYPRESSURE_WARN | DISPATCH_MEMORYPRESSURE_CRITICAL,
                                                   dispatch_get_main_queue());

    __weak __typeof(self) weakSelf = self;
    dispatch_source_t source = _memoryPressureSource;
    dispatch_source_set_event_handler(_memoryPressureSource, ^{
        __strong __typeof(weakSelf) strongSelf = weakSelf;
        if (!strongSelf) {
            return;
        }
        dispatch_source_memorypressure_flags_t flags = dispatch_source_get_data(source);
        if (flags & (DISPATCH_MEMORYPRESSURE_WARN | DISPATCH_MEMORYPRESSURE_CRITICAL)) {
            DLog(@"Memory pressure detected, clearing DVR buffer if possible");
            [strongSelf clear];
        }
    });
    dispatch_resume(_memoryPressureSource);
}

- (void)dealloc {
    if (_memoryPressureSource) {
        dispatch_source_cancel(_memoryPressureSource);
    }
    [decoders_ release];
    [encoder_ release];
    [buffer_ release];
    [super dealloc];
}

- (int)lengthForMetadata:(NSArray<id<DVREncodable>> *)metadata {
    __block int sum = 0;
    [metadata enumerateObjectsUsingBlock:^(id _Nonnull obj, NSUInteger idx, BOOL *_Nonnull stop) {
        // Payload length, payload
        sum += sizeof(int) + [obj dvrEncodableData].length;
    }];
    return sum;
}

// BUG-10368: Changed length to NSInteger to support large frame sizes without overflow
- (void)appendFrame:(NSArray<NSData *> *)frameLines
             length:(NSInteger)screenCharLength
           metadata:(NSArray<id<DVREncodable>> *)metadata
         cleanLines:(NSIndexSet *)cleanLines
               info:(DVRFrameInfo *)info {
    if (readOnly_) {
        return;
    }
    // BUG-10368: Use NSInteger for intermediate calculation to prevent overflow
    const NSInteger length = screenCharLength + [self lengthForMetadata:metadata];
    if (length > [buffer_ capacity] / 2) {
        // Protect the buffer from overflowing if you have a really big window.
        return;
    }
    _empty = NO;
    int prevFirst = [buffer_ firstKey];
    if ([encoder_ reserve:length]) {
        // Leading frames were freed. Invalidate them in all decoders.
        for (DVRDecoder *decoder in decoders_) {
            int newFirst = [buffer_ firstKey];
            for (int i = prevFirst; i < newFirst; ++i) {
                [decoder invalidateIndex:i];
            }
        }
    }
    [encoder_ appendFrame:frameLines length:screenCharLength metadata:metadata cleanLines:cleanLines info:info];
}

- (BOOL)canClear {
    return !readOnly_ && decoders_.count == 0;
}

- (void)clear {
    if (![self canClear]) {
        return;
    }
    [buffer_ autorelease];
    buffer_ = [[DVRBuffer alloc] initWithBufferCapacity:capacity_];
    [encoder_ autorelease];
    encoder_ = [[DVREncoder alloc] initWithBuffer:buffer_];
}

- (DVRDecoder *)getDecoder {
    DVRDecoder *decoder = [[DVRDecoder alloc] initWithBuffer:buffer_];
    [decoders_ addObject:decoder];
    [decoder release];
    return decoder;
}

- (void)releaseDecoder:(DVRDecoder *)decoder {
    [decoders_ removeObject:decoder];
}

- (long long)lastTimeStamp {
    DVRIndexEntry *entry = [buffer_ entryForKey:[buffer_ lastKey]];
    if (!entry) {
        return 0;
    }
    return entry->info.timestamp;
}

- (long long)firstTimeStamp {
    DVRIndexEntry *entry = [buffer_ entryForKey:[buffer_ firstKey]];
    if (!entry) {
        return 0;
    }
    return entry->info.timestamp;
}

// RC-003/BUG-1269: Returns YES if there are at least 2 frames with different timestamps.
// This prevents division by zero when calculating the span for instant replay.
- (BOOL)hasMultipleFrames {
    if (_empty) {
        return NO;
    }
    long long first = self.firstTimeStamp;
    long long last = self.lastTimeStamp;
    // Require different timestamps - even if firstKey != lastKey, they could have
    // the same timestamp (e.g., multiple frames saved in the same microsecond).
    return first != last && first != 0 && last != 0;
}

- (long long)firstTimestampAfter:(long long)timestamp {
    DVRIndexEntry *entry = [buffer_ firstEntryWithTimestampAfter:timestamp];
    if (!entry) {
        return 0;
    }
    return entry->info.timestamp;
}

- (NSDictionary *)dictionaryValue {
    return [self dictionaryValueFrom:self.firstTimeStamp to:self.lastTimeStamp];
}

- (NSDictionary *)dictionaryValueFrom:(long long)from to:(long long)to {
    DVR *dvr;
    if (from == self.firstTimeStamp && to == self.lastTimeStamp) {
        dvr = self;
    } else {
        dvr = [[self copyWithFramesFrom:from to:to] autorelease];
    }
    return @{@"version" : @4, @"capacity" : @(dvr->capacity_), @"buffer" : dvr->buffer_.dictionaryValue};
}

- (BOOL)loadDictionary:(NSDictionary *)dict {
    if (!dict) {
        return NO;
    }
    // This is the inner version. It is set in -[DVR dictionaryValueFrom:to:].
    NSArray<NSNumber *> *knownVersions = @[ @1, @2, @3, @4 ];
    NSNumber *version = [NSNumber castFrom:dict[@"version"]];
    if (!version || ![knownVersions containsObject:version]) {
        return NO;
    }
    int capacity = [dict[@"capacity"] intValue];
    if (capacity == 0) {
        return NO;
    }
    NSDictionary *bufferDict = dict[@"buffer"];
    if (!bufferDict) {
        return NO;
    }

    [buffer_ release];
    buffer_ = [DVRBuffer alloc];
    [buffer_ initWithBufferCapacity:capacity];
    capacity_ = capacity;

    [decoders_ release];
    decoders_ = [[NSMutableArray alloc] initWithCapacity:4];  // DVR decoders

    [encoder_ release];
    encoder_ = [DVREncoder alloc];
    [encoder_ initWithBuffer:buffer_];

    if (![buffer_ loadFromDictionary:bufferDict version:version.intValue]) {
        return NO;
    }
    readOnly_ = YES;
    return YES;
}

- (instancetype)copyWithFramesFrom:(long long)from to:(long long)to {
    DVR *theCopy = [[DVR alloc] initWithBufferCapacity:capacity_];
    DVRDecoder *decoder = [self getDecoder];
    if ([decoder seek:from]) {
        while (decoder.timestamp <= to || to == -1) {
            screen_char_t *frame = (screen_char_t *)[decoder decodedFrame];
            DVRFrameInfo info = [decoder info];
            // Pre-size arrays based on terminal height from frame info
            NSMutableArray *lines = [NSMutableArray arrayWithCapacity:info.height];
            NSMutableArray *metadata = [NSMutableArray arrayWithCapacity:info.height];
            int offset = 0;
            const int lineLength = info.width + 1;
            for (int i = 0; i < info.height; i++) {
                NSMutableData *data = [NSMutableData dataWithBytes:frame + offset
                                                            length:lineLength * sizeof(screen_char_t)];
                [lines addObject:data];
                offset += lineLength;

                [metadata addObject:iTermMetadataArrayFromData([decoder metadataForLine:i]) ?: @[]];
            }
            [theCopy appendFrame:lines
                          length:[decoder screenCharArrayLength]
                        metadata:metadata
                      cleanLines:nil
                            info:&info];
            if (![decoder next]) {
                break;
            }
        }
    }
    [self releaseDecoder:decoder];
    return theCopy;
}

@end

@interface NSArray (DVREncodable) <DVREncodable>
@end

@implementation NSArray (DVREncodable)

- (NSData *)dvrEncodableData {
    iTermMetadata temp = {0};
    iTermMetadataInitFromArray(&temp, self);
    iTermMetadataAutorelease(temp);
    return iTermMetadataEncodeToData(temp);
}

@end
