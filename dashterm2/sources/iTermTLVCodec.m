//
//  iTermTLVCodec.m
//  DashTerm2SharedARC
//
//  Created by George Nachman on 9/20/21.
//

#import "iTermTLVCodec.h"

#import "DebugLogging.h"

@implementation iTermTLVEncoder {
    NSMutableData *_data;
}

- (instancetype)init {
    self = [super init];
    if (self) {
        // Typical TLV payloads are a few KB; 1KB is a reasonable starting capacity
        _data = [NSMutableData dataWithCapacity:1024];
    }
    return self;
}

- (void)encodeBytes:(const void *)bytes length:(size_t)length {
    const NSUInteger offset = _data.length;
    [_data setLength:offset + length];
    memmove(((char *)_data.mutableBytes) + offset, bytes, length);
}

- (void)encodeInt:(int)i {
    [self encodeBytes:&i length:sizeof(i)];
}

- (void)encodeUnsignedInt:(unsigned int)i {
    [self encodeBytes:&i length:sizeof(i)];
}

- (void)encodeData:(NSData *)data {
    // BUG-f1386: Replace assert with safe guard - data exceeding INT_MAX can't be encoded
    // but should not crash the app. Truncate to INT_MAX with logging.
    if (data.length > INT_MAX) {
        ELog(@"iTermTLVCodec: Data length %lu exceeds INT_MAX, truncating", (unsigned long)data.length);
        data = [data subdataWithRange:NSMakeRange(0, INT_MAX)];
    }

    [self encodeInt:(int)data.length];
    [self encodeBytes:data.bytes length:data.length];
}

- (void)encodeRange:(NSRange)range {
    [self encodeBytes:&range length:sizeof(range)];
}

- (void)encodeBool:(BOOL)b {
    char c = b ? 1 : 0;
    [self encodeBytes:&c length:1];
}

- (void)encodeDouble:(double)d {
    [self encodeBytes:&d length:sizeof(d)];
}
@end

@implementation iTermTLVDecoder {
    NSInteger _offset;
}

- (instancetype)initWithData:(NSData *)data {
    self = [super init];
    if (self) {
        _data = data ?: [NSData data];
    }
    return self;
}

- (BOOL)finished {
    return _offset >= _data.length;
}

- (BOOL)decodeBytes:(void *)destination length:(size_t)length {
    if (_offset + length > _data.length) {
        return NO;
    }
    memmove(destination, ((const char *)_data.bytes) + _offset, length);
    _offset += length;
    return YES;
}

- (BOOL)decodeInt:(int *)i {
    return [self decodeBytes:i length:sizeof(*i)];
}

- (BOOL)decodeUnsignedInt:(unsigned int *)i {
    return [self decodeBytes:i length:sizeof(*i)];
}

- (NSData *)decodeData {
    int length = 0;
    if (![self decodeInt:&length]) {
        return nil;
    }
    if (length < 0) {
        return nil;
    }
    NSMutableData *data = [NSMutableData dataWithLength:length];
    if (![self decodeBytes:data.mutableBytes length:length]) {
        return nil;
    }
    return data;
}

- (BOOL)decodeRange:(NSRange *)range {
    return [self decodeBytes:range length:sizeof(*range)];
}

- (BOOL)decodeBool:(BOOL *)b {
    char c;
    if (![self decodeBytes:&c length:sizeof(c)]) {
        return NO;
    }
    *b = !!c;
    return YES;
}

- (BOOL)decodeDouble:(double *)d {
    if (![self decodeBytes:d length:sizeof(*d)]) {
        return NO;
    }
    return YES;
}

@end
