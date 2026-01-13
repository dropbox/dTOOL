//
//  iTermPackedCharacterBuffer.m
//  DashTerm2
//
//  Created by DashTerm2 AI on 12/17/24.
//

#import "iTermPackedCharacterBuffer.h"
#import "iTermMalloc.h"
#import "ScreenCharArray.h"

@implementation iTermPackedCharacterBuffer {
    packed_screen_char_t *_packedBuffer;
    int _size;
    PackedColorTable *_colorTable;
}

- (instancetype)initWithSize:(int)size colorTable:(nullable PackedColorTable *)colorTable {
    self = [super init];
    if (self) {
        _size = size;
        _packedBuffer = iTermCalloc(size, sizeof(packed_screen_char_t));
        _colorTable = colorTable ?: [[PackedColorTable alloc] initWithCapacity:251];
    }
    return self;
}

- (instancetype)initWithPackedData:(NSData *)data colorTable:(PackedColorTable *)colorTable {
    self = [super init];
    if (self) {
        _size = (int)(data.length / sizeof(packed_screen_char_t));
        _packedBuffer = iTermMemdup((void *)data.bytes, _size, sizeof(packed_screen_char_t));
        _colorTable = colorTable ?: [[PackedColorTable alloc] initWithCapacity:251];
    }
    return self;
}

- (instancetype)initWithChars:(const screen_char_t *)chars
                         size:(int)count
                   colorTable:(nullable PackedColorTable *)colorTable {
    self = [super init];
    if (self) {
        _size = count;
        _packedBuffer = iTermCalloc(count, sizeof(packed_screen_char_t));
        _colorTable = colorTable ?: [[PackedColorTable alloc] initWithCapacity:251];

        // Pack the input characters
        PackScreenCharArray(chars, _packedBuffer, count, _colorTable);
    }
    return self;
}

- (void)dealloc {
    if (_packedBuffer) {
        free(_packedBuffer);
    }
}

#pragma mark - Properties

- (int)size {
    return _size;
}

- (PackedColorTable *)colorTable {
    return _colorTable;
}

- (NSUInteger)memoryUsage {
    return _size * sizeof(packed_screen_char_t);
}

- (NSUInteger)unpackedMemoryUsage {
    return _size * sizeof(screen_char_t);
}

- (NSUInteger)memorySaved {
    return self.unpackedMemoryUsage - self.memoryUsage;
}

#pragma mark - Writing (Packing)

- (void)writeChars:(const screen_char_t *)chars count:(int)count atOffset:(int)offset {
    if (offset < 0 || offset + count > _size) {
        return;
    }
    PackScreenCharArray(chars, _packedBuffer + offset, count, _colorTable);
}

- (void)appendChars:(const screen_char_t *)chars count:(int)count atOffset:(int)offset {
    if (offset < 0 || offset + count > _size) {
        return;
    }
    PackScreenCharArray(chars, _packedBuffer + offset, count, _colorTable);
}

#pragma mark - Reading (Unpacking)

- (screen_char_t)charAtOffset:(int)offset {
    if (offset < 0 || offset >= _size) {
        screen_char_t empty = {0};
        return empty;
    }
    return UnpackScreenChar(_packedBuffer[offset], _colorTable);
}

- (void)readChars:(screen_char_t *)dst count:(int)count fromOffset:(int)offset {
    if (offset < 0 || offset + count > _size || !dst) {
        return;
    }
    UnpackScreenCharArray(_packedBuffer + offset, dst, count, _colorTable);
}

- (screen_char_t *)copyCharsFromOffset:(int)offset count:(int)count {
    if (offset < 0 || offset + count > _size || count <= 0) {
        return NULL;
    }

    screen_char_t *result = iTermCalloc(count, sizeof(screen_char_t));
    UnpackScreenCharArray(_packedBuffer + offset, result, count, _colorTable);
    return result;
}

- (void)readIntoScreenCharArray:(ScreenCharArray *)array fromOffset:(int)offset count:(int)count {
    if (!array || offset < 0 || offset + count > _size) {
        return;
    }

    // Get mutable pointer to the array's internal buffer
    screen_char_t *dst = (screen_char_t *)array.line;
    if (dst && count <= array.length) {
        UnpackScreenCharArray(_packedBuffer + offset, dst, count, _colorTable);
    }
}

#pragma mark - Buffer Management

- (void)resize:(int)newSize {
    if (newSize == _size) {
        return;
    }

    _packedBuffer = iTermRealloc(_packedBuffer, newSize, sizeof(packed_screen_char_t));

    // Zero out new space if growing
    if (newSize > _size) {
        memset(_packedBuffer + _size, 0, (newSize - _size) * sizeof(packed_screen_char_t));
    }

    _size = newSize;
}

- (iTermPackedCharacterBuffer *)clone {
    iTermPackedCharacterBuffer *copy = [[iTermPackedCharacterBuffer alloc] initWithSize:_size colorTable:_colorTable];
    memcpy(copy->_packedBuffer, _packedBuffer, _size * sizeof(packed_screen_char_t));
    return copy;
}

- (BOOL)deepIsEqual:(id)object {
    if (object == self) {
        return YES;
    }

    iTermPackedCharacterBuffer *other = [object isKindOfClass:[iTermPackedCharacterBuffer class]] ? object : nil;
    if (!other) {
        return NO;
    }

    return _size == other->_size && !memcmp(_packedBuffer, other->_packedBuffer, _size * sizeof(packed_screen_char_t));
}

#pragma mark - Raw Access

- (const packed_screen_char_t *)packedPointer {
    return _packedBuffer;
}

- (packed_screen_char_t *)mutablePackedPointer {
    return _packedBuffer;
}

- (NSData *)packedData {
    return [NSData dataWithBytes:_packedBuffer length:_size * sizeof(packed_screen_char_t)];
}

#pragma mark - Description

- (NSString *)description {
    return [NSString stringWithFormat:@"<iTermPackedCharacterBuffer size=%d memory=%lu saved=%lu>", _size,
                                      (unsigned long)self.memoryUsage, (unsigned long)self.memorySaved];
}

- (NSString *)shortDescription {
    const int maxChars = 20;
    int displayCount = MIN(_size, maxChars);

    // Max 20 chars + possible "..."
    NSMutableString *str = [NSMutableString stringWithCapacity:maxChars + 3];
    for (int i = 0; i < displayCount; i++) {
        screen_char_t c = [self charAtOffset:i];
        if (c.code == 0) {
            [str appendString:@" "];
        } else if (c.code < 32) {
            [str appendString:@"?"];
        } else {
            [str appendFormat:@"%C", c.code];
        }
    }

    if (_size > maxChars) {
        [str appendString:@"..."];
    }

    return [NSString stringWithFormat:@"<%@>", str];
}

@end
