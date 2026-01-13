//
//  iTermData.m
//  DashTerm2
//
//  Created by George Nachman on 2/4/18.
//

#import "iTermData.h"

#import "DebugLogging.h"
#import "iTermMalloc.h"
#import "iTermMetalGlyphKey.h"
#import "iTermTextRendererCommon.h"
#import "ScreenChar.h"

static const unsigned char iTermDataGuardRegionValue[64] = {
    0x70, 0x71, 0x72, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7A, 0x7B, 0x7C, 0x7D, 0x7E, 0x7F,
    0x80, 0x81, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89, 0x8A, 0x8B, 0x8C, 0x8D, 0x8E, 0x8F,
    0x90, 0x91, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9A, 0x9B, 0x9C, 0x9D, 0x9E, 0x9F,
    0xa0, 0xa1, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7, 0xa8, 0xa9, 0xaA, 0xaB, 0xaC, 0xaD, 0xaE, 0xaF};

@implementation iTermData {
  @protected
    void *_mutableBytes;
    NSUInteger _length;
}

- (instancetype)initWithLength:(NSUInteger)length {
    self = [super init];
    if (self) {
        unsigned char *buffer = iTermMalloc(length + sizeof(iTermDataGuardRegionValue));
        memmove(buffer + length, iTermDataGuardRegionValue, sizeof(iTermDataGuardRegionValue));

        _mutableBytes = buffer;
        _length = length;
    }
    return self;
}

- (void)dealloc {
    if (_mutableBytes) {
        [self checkForOverrun];
        free(_mutableBytes);
    }
    _length = 0xdeadbeef;
}

- (const unsigned char *)bytes {
    return _mutableBytes;
}

- (void)setLength:(NSUInteger)length {
    // BUG-7214: Only realloc if growing. Shrinking just updates the logical length
    // without touching the buffer, avoiding potential race conditions when another
    // thread is reading from mutableBytes. The guard region at the new logical end
    // will be checked on dealloc.
    NSUInteger currentCapacity = _length + sizeof(iTermDataGuardRegionValue);
    NSUInteger neededCapacity = length + sizeof(iTermDataGuardRegionValue);

    if (neededCapacity > currentCapacity) {
        // Growing - must realloc
        _mutableBytes = iTermRealloc(_mutableBytes, neededCapacity, 1);
    }

    // Update logical length and write guard region at new end
    _length = length;
    memmove((char *)_mutableBytes + length, iTermDataGuardRegionValue, sizeof(iTermDataGuardRegionValue));
}

- (void)checkForOverrun {
    if (_mutableBytes) {
        unsigned char *buffer = _mutableBytes;
        const int comparisonResult =
            memcmp(buffer + _length, iTermDataGuardRegionValue, sizeof(iTermDataGuardRegionValue));
        // BUG-f1371: Convert assert to guard - log buffer overrun but don't crash
        if (comparisonResult != 0) {
            DLog(@"WARNING BUG-f1371: iTermData buffer overrun detected - guard region corrupted");
            return;
        }
    }
}

- (NSUInteger)count {
    return _length / self.stride;
}

- (void)setCount:(NSUInteger)count {
    [self setLength:count * self.stride];
}

- (NSUInteger)stride {
    return 1;
}

@end


@implementation iTermScreenCharData : iTermData

+ (instancetype)dataOfLength:(NSUInteger)length {
    return [[self alloc] initWithLength:length];
}

- (void)checkForOverrun {
    if (_mutableBytes) {
        unsigned char *buffer = _mutableBytes;
        const int comparisonResult =
            memcmp(buffer + _length, iTermDataGuardRegionValue, sizeof(iTermDataGuardRegionValue));
        // BUG-f1372: Convert assert to guard - log buffer overrun but don't crash
        if (comparisonResult != 0) {
            DLog(@"WARNING BUG-f1372: iTermScreenCharData buffer overrun detected");
            return;
        }
    }
}

- (NSUInteger)stride {
    return sizeof(screen_char_t);
}

@end

@implementation iTermGlyphKeyData : iTermData

+ (instancetype)dataOfLength:(NSUInteger)length {
    return [[self alloc] initWithLength:length];
}

- (struct iTermMetalGlyphKey *)basePointer {
    return (struct iTermMetalGlyphKey *)self.mutableBytes;
}

- (void)checkForOverrun {
    if (_mutableBytes) {
        unsigned char *buffer = _mutableBytes;
        const int comparisonResult =
            memcmp(buffer + _length, iTermDataGuardRegionValue, sizeof(iTermDataGuardRegionValue));
        // BUG-f1373: Convert assert to guard - log buffer overrun but don't crash
        if (comparisonResult != 0) {
            DLog(@"WARNING BUG-f1373: iTermGlyphKeyData buffer overrun detected");
            return;
        }
    }
}
- (void)checkForOverrun1 {
    if (_mutableBytes) {
        unsigned char *buffer = _mutableBytes;
        const int comparisonResult =
            memcmp(buffer + _length, iTermDataGuardRegionValue, sizeof(iTermDataGuardRegionValue));
        // BUG-f1374: Convert assert to guard - log buffer overrun but don't crash
        if (comparisonResult != 0) {
            DLog(@"WARNING BUG-f1374: iTermGlyphKeyData buffer overrun 1 detected");
            return;
        }
    }
}
- (void)checkForOverrun2 {
    if (_mutableBytes) {
        unsigned char *buffer = _mutableBytes;
        const int comparisonResult =
            memcmp(buffer + _length, iTermDataGuardRegionValue, sizeof(iTermDataGuardRegionValue));
        // BUG-f1375: Convert assert to guard - log buffer overrun but don't crash
        if (comparisonResult != 0) {
            DLog(@"WARNING BUG-f1375: iTermGlyphKeyData buffer overrun 2 detected");
            return;
        }
    }
}
- (NSUInteger)stride {
    return sizeof(iTermMetalGlyphKey);
}
@end

@implementation iTermAttributesData : iTermData

+ (instancetype)dataOfLength:(NSUInteger)length {
    return [[self alloc] initWithLength:length];
}

- (void)checkForOverrun {
    if (_mutableBytes) {
        unsigned char *buffer = _mutableBytes;
        const int comparisonResult =
            memcmp(buffer + _length, iTermDataGuardRegionValue, sizeof(iTermDataGuardRegionValue));
        // BUG-f1376: Convert assert to guard - log buffer overrun but don't crash
        if (comparisonResult != 0) {
            DLog(@"WARNING BUG-f1376: iTermAttributesData buffer overrun detected");
            return;
        }
    }
}
- (void)checkForOverrun1 {
    if (_mutableBytes) {
        unsigned char *buffer = _mutableBytes;
        const int comparisonResult =
            memcmp(buffer + _length, iTermDataGuardRegionValue, sizeof(iTermDataGuardRegionValue));
        // BUG-f1377: Convert assert to guard - log buffer overrun but don't crash
        if (comparisonResult != 0) {
            DLog(@"WARNING BUG-f1377: iTermAttributesData buffer overrun 1 detected");
            return;
        }
    }
}
- (void)checkForOverrun2 {
    if (_mutableBytes) {
        unsigned char *buffer = _mutableBytes;
        const int comparisonResult =
            memcmp(buffer + _length, iTermDataGuardRegionValue, sizeof(iTermDataGuardRegionValue));
        // BUG-f1378: Convert assert to guard - log buffer overrun but don't crash
        if (comparisonResult != 0) {
            DLog(@"WARNING BUG-f1378: iTermAttributesData buffer overrun 2 detected");
            return;
        }
    }
}

- (NSUInteger)stride {
    return sizeof(iTermMetalGlyphAttributes);
}

@end

@implementation iTermBackgroundColorRLEsData : iTermData

+ (instancetype)dataOfLength:(NSUInteger)length {
    return [[self alloc] initWithLength:length];
}

- (void)checkForOverrun {
    if (_mutableBytes) {
        unsigned char *buffer = _mutableBytes;
        const int comparisonResult =
            memcmp(buffer + _length, iTermDataGuardRegionValue, sizeof(iTermDataGuardRegionValue));
        // BUG-f1379: Convert assert to guard - log buffer overrun but don't crash
        if (comparisonResult != 0) {
            DLog(@"WARNING BUG-f1379: iTermBackgroundColorRLEsData buffer overrun detected");
            return;
        }
    }
}

- (NSUInteger)stride {
    return sizeof(iTermMetalBackgroundColorRLE);
}

@end

@implementation iTermBitmapData : iTermData
+ (instancetype)dataOfLength:(NSUInteger)length {
    ITAssertWithMessage(length > 0, @"Zero length (%@)", @(length));
    return [[self alloc] initWithLength:length];
}

- (void)checkForOverrun {
    [self checkForOverrunWithInfo:@"No info"];
}

- (void)checkForOverrunWithInfo:(NSString *)info {
    if (_mutableBytes) {
        unsigned char *buffer = _mutableBytes;
        const int comparisonResult =
            memcmp(buffer + _length, iTermDataGuardRegionValue, sizeof(iTermDataGuardRegionValue));
        if (comparisonResult == 0) {
            return;
        }
        // 2 guard regions: actual and expected, 3 chars per byte ("xx "), plus "vs expected: "
        NSMutableString *hex = [NSMutableString stringWithCapacity:sizeof(iTermDataGuardRegionValue) * 3 * 2 + 14];
        for (NSInteger i = 0; i < sizeof(iTermDataGuardRegionValue); i++) {
            unsigned int value = buffer[_length + i];
            [hex appendFormat:@"%02x ", value];
        }
        [hex appendString:@"vs expected: "];
        for (NSInteger i = 0; i < sizeof(iTermDataGuardRegionValue); i++) {
            unsigned int value = iTermDataGuardRegionValue[i];
            [hex appendFormat:@"%02x ", value];
        }
        ITAssertWithMessage(NO, @"%@. Guard corrupted: actual is %@", info, hex);
    }
}
- (NSUInteger)stride {
    return 4;
}

@end
