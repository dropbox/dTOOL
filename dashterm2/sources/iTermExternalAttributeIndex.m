//
//  iTermExternalAttributeIndex.m
//  DashTerm2
//
//  Created by George Nachman on 9/17/21.
//

#import "iTermExternalAttributeIndex.h"
#import "DebugLogging.h"
#import "iTermTLVCodec.h"
#import "iTermURLStore.h"
#import "NSArray+iTerm.h"
#import "NSDictionary+iTerm.h"
#import "NSMutableData+iTerm.h"
#import "ScreenChar.h"
#import <os/lock.h>

NSString *const iTermExternalAttributeBlockIDDelimiter = @"\uf8ff";

// Optimization: Cache NSNumber objects for common attribute indices (0-511) to avoid boxing.
// Terminal widths are typically 80-400 columns, so 512 covers most use cases.
// This eliminates repeated @(x) allocations in hot loops during character attribute operations.
static const int kExternalAttributeCachedKeyCount = 512;
static NSNumber *sExternalAttributeCachedKeys[kExternalAttributeCachedKeyCount];

// Cache uniform attribute wrappers so ASCII bursts don't allocate per run.
static os_unfair_lock sUniformExternalAttributeCacheLock = OS_UNFAIR_LOCK_INIT;
static NSMapTable<iTermExternalAttribute *, iTermUniformExternalAttributes *> *sUniformExternalAttributeCache;

// Returns a cached NSNumber for common indices, or creates a new one for larger indices.
NS_INLINE NSNumber *iTermExternalAttributeKeyToNumber(int key) {
    if (key >= 0 && key < kExternalAttributeCachedKeyCount) {
        return sExternalAttributeCachedKeys[key];
    }
    return @(key);
}

__attribute__((constructor)) static void iTermExternalAttributeInitializeCachedKeys(void) {
    for (int i = 0; i < kExternalAttributeCachedKeyCount; i++) {
        sExternalAttributeCachedKeys[i] = @(i);
    }
}

@implementation iTermExternalAttributeIndex {
    NSMutableDictionary<NSNumber *, iTermExternalAttribute *> *_attributes;
    NSInteger _offset; // Add this to externally visible indexes to get keys into _attributes.
}

+ (instancetype)withDictionary:(NSDictionary *)dictionary {
    return [[self alloc] initWithDictionary:dictionary];
}

- (instancetype)initWithAttributeDictionary:(NSDictionary<NSNumber *, iTermExternalAttribute *> *)attributes {
    self = [super init];
    if (self) {
        _attributes = [attributes copy];
    }
    return self;
}

- (instancetype)initWithDictionary:(NSDictionary *)dictionary {
    if (!dictionary.count) {
        return nil;
    }
    iTermUniformExternalAttributes *uniform = [[iTermUniformExternalAttributes alloc] initWithDictionary:dictionary];
    if (uniform) {
        return uniform;
    }
    self = [self init];
    if (self) {
        [dictionary enumerateKeysAndObjectsUsingBlock:^(id _Nonnull key, id _Nonnull obj, BOOL *_Nonnull stop) {
            NSNumber *x = [NSNumber castFrom:key];
            if (!x) {
                return;
            }
            NSDictionary *dict = [[NSDictionary castFrom:obj] dictionaryByRemovingNullValues];
            if (!dict) {
                return;
            }
            iTermExternalAttribute *attr = [[iTermExternalAttribute alloc] initWithDictionary:dict];
            [self setAttributes:attr at:x.intValue count:1];
        }];
    }
    return self;
}

- (instancetype)init {
    self = [super init];
    if (self) {
        _attributes = [NSMutableDictionary dictionaryWithCapacity:8];
    }
    return self;
}

+ (instancetype)fromData:(NSData *)data {
    iTermExternalAttributeIndex *eaIndex = [[iTermExternalAttributeIndex alloc] init];
    iTermTLVDecoder *decoder = [[iTermTLVDecoder alloc] initWithData:data];
    while (!decoder.finished) {
        NSRange range;
        if (![decoder decodeRange:&range]) {
            return nil;
        }
        NSData *data = [decoder decodeData];
        if (!data) {
            return nil;
        }
        iTermExternalAttribute *attr = [iTermExternalAttribute fromData:data];
        [eaIndex setAttributes:attr at:range.location count:range.length];
    }
    return eaIndex;
}

- (NSData *)encodedRange:(NSRange)range {
    return [NSData dataWithBytes:&range length:sizeof(range)];
}

- (NSData *)data {
    iTermTLVEncoder *encoder = [[iTermTLVEncoder alloc] init];
    [self enumerateValuesInRange:NSMakeRange(0, NSUIntegerMax)
                           block:^(NSRange range, iTermExternalAttribute *attr) {
                               [encoder encodeRange:range];
                               [encoder encodeData:[attr data]];
                           }];
    return encoder.data;
}

- (NSDictionary *)attributes {
    return _attributes;
}

- (NSInteger)offset {
    return _offset;
}

- (NSDictionary *)dictionaryValue {
    // TODO: Consider runlength encoding this to save space, e.g. for long hyperlinks
    return [_attributes mapValuesWithBlock:^id(NSNumber *key, iTermExternalAttribute *attribute) {
        return attribute.dictionaryValue;
    }];
}

- (NSString *)description {
    return [NSString stringWithFormat:@"<%@: %p %@>", NSStringFromClass([self class]), self,
                                      [self shortDescriptionWithLength:[self largestKey] + 1]];
}

- (NSUInteger)largestKey {
    NSNumber *key = [_attributes.allKeys maxWithComparator:^(NSNumber *lhs, NSNumber *rhs) {
        return [lhs compare:rhs];
    }];
    return [key unsignedIntegerValue];
}

- (NSString *)shortDescriptionWithLength:(int)length {
    NSMutableArray<NSString *> *array = [NSMutableArray arrayWithCapacity:_attributes.count];
    [self enumerateValuesInRange:NSMakeRange(0, length)
                           block:^(NSRange range, iTermExternalAttribute *attr) {
                               [array addObject:[NSString stringWithFormat:@"%@=%@", NSStringFromRange(range),
                                                                           [attr description]]];
                           }];
    return [array componentsJoinedByString:@","];
}

- (void)enumerateValuesInRange:(NSRange)range
                         block:(void (^NS_NOESCAPE)(NSRange, iTermExternalAttribute *_Nonnull))block {
    __block NSNumber *startOfRunKey = nil;
    __block NSNumber *endOfRunKey = nil;
    void (^emit)(void) = ^{
        // BUG-f955: Guard against nil startOfRunKey/endOfRunKey instead of crashing
        if (!startOfRunKey || !endOfRunKey) {
            ELog(@"iTermExternalAttributeIndex: emit called with nil keys - startOfRunKey=%@, endOfRunKey=%@",
                 startOfRunKey, endOfRunKey);
            return;
        }
        // BUG-f956: Guard against nil attribute instead of crashing
        iTermExternalAttribute *attr = self[startOfRunKey.unsignedIntegerValue];
        if (!attr) {
            ELog(@"iTermExternalAttributeIndex: emit called but attribute at %@ is nil", startOfRunKey);
            return;
        }
        // BUG-f957: Guard against invalid range instead of crashing
        if (endOfRunKey.unsignedIntegerValue < startOfRunKey.unsignedIntegerValue) {
            ELog(@"iTermExternalAttributeIndex: endOfRunKey %@ < startOfRunKey %@", endOfRunKey, startOfRunKey);
            return;
        }
        block(NSMakeRange(startOfRunKey.unsignedIntegerValue,
                          endOfRunKey.unsignedIntegerValue - startOfRunKey.unsignedIntegerValue + 1),
              attr);
    };
    void (^accumulate)(NSNumber *) = ^(NSNumber *key) {
        // BUG-f958: Guard against nil key instead of crashing
        if (!key) {
            ELog(@"iTermExternalAttributeIndex: accumulate called with nil key");
            return;
        }
        if (!startOfRunKey) {
            // Start of first run.
            startOfRunKey = key;
            endOfRunKey = key;
            return;
        }
        if (key.unsignedIntegerValue == endOfRunKey.unsignedIntegerValue + 1 &&
            [self[startOfRunKey.unsignedIntegerValue] isEqualToExternalAttribute:self[key.unsignedIntegerValue]]) {
            // Continue current run.
            endOfRunKey = key;
            return;
        }

        // Run ended. Begin a new run.
        emit();
        startOfRunKey = key;
        endOfRunKey = key;
    };
    [self enumerateSortedKeysInRange:range
                               block:^(NSNumber *key) {
                                   accumulate(key);
                               }];
    if (startOfRunKey) {
        emit();
    }
}

// Subclasses override this.
- (void)enumerateSortedKeysInRange:(NSRange)range block:(void (^)(NSNumber *key))block {
    NSArray<NSNumber *> *sortedKeys = [[_attributes allKeys] sortedArrayUsingSelector:@selector(compare:)];
    [sortedKeys enumerateObjectsUsingBlock:^(NSNumber *_Nonnull key, NSUInteger idx, BOOL *_Nonnull stop) {
        if (!NSLocationInRange(key.unsignedIntegerValue - _offset, range)) {
            return;
        }
        block(key);
    }];
}

- (void)copyFrom:(id<iTermExternalAttributeIndexReading>)source
          source:(int)loadBase
     destination:(int)storeBase
           count:(int)count {
    int start;
    int end;
    int stride;
    if (source == self && storeBase > loadBase) {
        // Copying to the right within self.
        start = count - 1;
        end = -1;
        stride = -1;
    } else {
        // Copying to other object or to the left.
        start = 0;
        end = count;
        stride = 1;
    }
    for (int i = start; i != end; i += stride) {
        _attributes[iTermExternalAttributeKeyToNumber(storeBase + i)] = source[loadBase + i];
    }
}

- (BOOL)isEmpty {
    return _attributes.count == 0;
}

- (void)mutateAttributesFrom:(int)start
                          to:(int)end
                       block:(iTermExternalAttribute *_Nullable (^)(iTermExternalAttribute *_Nullable))block {
    for (int x = start; x <= end; x++) {
        NSNumber *key = iTermExternalAttributeKeyToNumber(x);
        _attributes[key] = block(_attributes[key]);
    }
}

- (id)objectForKeyedSubscript:(id)key {
    [self doesNotRecognizeSelector:_cmd];
    return nil;
}

- (iTermExternalAttribute *)objectAtIndexedSubscript:(NSInteger)idx {
    return _attributes[iTermExternalAttributeKeyToNumber((int)(idx + _offset))];
}

- (iTermExternalAttribute *)attributeAtIndex:(int)i {
    return _attributes[iTermExternalAttributeKeyToNumber(i + _offset)];
}

- (void)setObject:(iTermExternalAttribute *_Nullable)ea atIndexedSubscript:(NSUInteger)i {
    _attributes[iTermExternalAttributeKeyToNumber((int)(i + _offset))] = ea;
}

- (iTermExternalAttributeIndex *)indexByDeletingFirst:(int)n {
    // Pre-allocate based on current attribute count - most/all may be retained
    NSMutableDictionary<NSNumber *, iTermExternalAttribute *> *temp =
        [NSMutableDictionary dictionaryWithCapacity:_attributes.count];
    [_attributes enumerateKeysAndObjectsUsingBlock:^(NSNumber *_Nonnull key, iTermExternalAttribute *_Nonnull obj,
                                                     BOOL *_Nonnull stop) {
        const int i = key.intValue;
        if (i < n) {
            return;
        }
        temp[iTermExternalAttributeKeyToNumber(i - n)] = obj;
    }];
    return [[iTermExternalAttributeIndex alloc] initWithAttributeDictionary:temp];
}

- (iTermExternalAttributeIndex *)subAttributesFromIndex:(int)index {
    return [self subAttributesFromIndex:index maximumLength:INT_MAX];
}

- (iTermExternalAttributeIndex *)subAttributesToIndex:(int)index {
    return [self subAttributesFromIndex:0 maximumLength:index];
}

- (iTermExternalAttributeIndex *)subAttributesInRange:(NSRange)range {
    return [self subAttributesFromIndex:range.location maximumLength:range.length];
}

- (iTermExternalAttributeIndex *)subAttributesFromIndex:(int)index maximumLength:(int)maxLength {
    iTermExternalAttributeIndex *sub = [[iTermExternalAttributeIndex alloc] init];
    sub->_offset = 0;
    [_attributes enumerateKeysAndObjectsUsingBlock:^(NSNumber *_Nonnull key, iTermExternalAttribute *_Nonnull obj,
                                                     BOOL *_Nonnull stop) {
        const int intKey = key.intValue + _offset;
        if (intKey < index) {
            return;
        }
        if (intKey >= (NSInteger)index + (NSInteger)maxLength) {
            return;
        }
        sub->_attributes[iTermExternalAttributeKeyToNumber(intKey - index)] = obj;
    }];
    return sub;
}

- (id)copyWithZone:(NSZone *)zone {
    // Optimization: Return nil for empty indices to avoid allocating
    // empty dictionaries during the per-frame row state copy operation.
    // Most terminal lines have no external attributes (URLs, underline colors, etc.),
    // so this saves significant allocation overhead in the Metal rendering hot path.
    if (_attributes.count == 0) {
        return nil;
    }
    iTermExternalAttributeIndex *copy = [[iTermExternalAttributeIndex alloc] init];
    copy->_attributes = [_attributes mutableCopy];
    return copy;
}

- (id)mutableCopyWithZone:(NSZone *)zone {
    return [self copyWithZone:zone];
}

- (void)eraseAt:(int)x {
    [_attributes removeObjectForKey:iTermExternalAttributeKeyToNumber(x + _offset)];
}

- (void)eraseInRange:(VT100GridRange)range {
    for (int i = 0; i < range.length; i++) {
        [self eraseAt:i + range.location];
    }
}

- (void)deleteRange:(NSRange)range {
    // Pre-allocate based on current attribute count - most may be retained after deletion
    NSMutableDictionary *updated = [NSMutableDictionary dictionaryWithCapacity:_attributes.count];
    [_attributes enumerateKeysAndObjectsUsingBlock:^(NSNumber *key, iTermExternalAttribute *obj, BOOL *stop) {
        const int i = key.intValue;
        if (NSLocationInRange(i, range)) {
            return;
        }
        if (i >= NSMaxRange(range)) {
            updated[iTermExternalAttributeKeyToNumber(i - (int)range.length)] = obj;
        } else {
            updated[key] = obj;
        }
    }];
    _attributes = updated;
}

- (void)insertFrom:(iTermExternalAttributeIndex *)eaIndex
       sourceRange:(NSRange)sourceRange
           atIndex:(int)destinationIndex {
    // Pre-allocate for existing attributes plus inserted range
    NSMutableDictionary *updated = [NSMutableDictionary dictionaryWithCapacity:_attributes.count + sourceRange.length];
    // Shift items right beginning at range.location
    [_attributes enumerateKeysAndObjectsUsingBlock:^(NSNumber *key, iTermExternalAttribute *obj, BOOL *stop) {
        const int i = key.intValue;
        if (i >= destinationIndex) {
            updated[iTermExternalAttributeKeyToNumber(i + (int)sourceRange.length)] = obj;
        } else {
            updated[iTermExternalAttributeKeyToNumber(i)] = obj;
        }
    }];
    // Copy source range over
    for (int i = 0; i < sourceRange.length; i++) {
        updated[iTermExternalAttributeKeyToNumber(i + destinationIndex)] = eaIndex[sourceRange.location + i];
    }
    _attributes = updated;
}

- (void)copyInto:(iTermExternalAttributeIndex *)destination {
    destination->_attributes = [_attributes mutableCopy];
}

- (void)copyFrom:(id<iTermExternalAttributeIndexReading>)source startOffset:(int)startOffset {
    [source.attributes enumerateKeysAndObjectsUsingBlock:^(NSNumber *_Nonnull key, iTermExternalAttribute *_Nonnull obj,
                                                           BOOL *_Nonnull stop) {
        const int i = key.intValue;
        if (i >= startOffset) {
            _attributes[iTermExternalAttributeKeyToNumber(i - startOffset)] = obj;
        }
    }];
}

- (BOOL)isEqualToExternalAttributeIndex:(id<iTermExternalAttributeIndexReading>)other {
    if (other == nil && self.attributes.count == 0) {
        return YES;
    }
    NSMutableSet<NSNumber *> *keys = [NSMutableSet setWithArray:self.attributes.allKeys];
    [keys addObjectsFromArray:other.attributes.allKeys];
    for (NSNumber *key in keys) {
        iTermExternalAttribute *lhs = self.attributes[key];
        iTermExternalAttribute *rhs = self.attributes[key];
        if (lhs) {
            if (![lhs isEqualToExternalAttribute:rhs]) {
                return NO;
            }
        } else {
            if (![rhs isEqualToExternalAttribute:lhs]) {
                return NO;
            }
        }
    }
    return YES;
}

+ (BOOL)externalAttributeIndex:(id<iTermExternalAttributeIndexReading>)lhs
                isEqualToIndex:(id<iTermExternalAttributeIndexReading>)rhs {
    if (lhs == nil && rhs == nil) {
        return YES;
    }
    if (lhs) {
        return [lhs isEqualToExternalAttributeIndex:rhs];
    } else {
        return [rhs isEqualToExternalAttributeIndex:lhs];
    }
}

- (void)setAttributes:(iTermExternalAttribute *)attributes at:(int)start count:(int)count {
    for (int i = 0; i < count; i++) {
        _attributes[iTermExternalAttributeKeyToNumber(i + start + (int)_offset)] = attributes;
    }
}

+ (iTermExternalAttributeIndex *)concatenationOf:(id<iTermExternalAttributeIndexReading>)lhs
                                          length:(int)lhsLength
                                            with:(id<iTermExternalAttributeIndexReading>)rhs
                                          length:(int)rhsLength {
    iTermExternalAttributeIndex *result = [[iTermExternalAttributeIndex alloc] init];
    [result appendValuesFrom:lhs range:NSMakeRange(0, lhsLength) at:0];
    [result appendValuesFrom:rhs range:NSMakeRange(0, rhsLength) at:lhsLength];
    return result;
}

- (void)appendValuesFrom:(id<iTermExternalAttributeIndexReading>)source range:(NSRange)range at:(int)base {
    [source.attributes enumerateKeysAndObjectsUsingBlock:^(NSNumber *_Nonnull key, iTermExternalAttribute *_Nonnull obj,
                                                           BOOL *_Nonnull stop) {
        const int intKey = key.intValue;
        if (intKey < range.location) {
            return;
        }
        if (intKey >= NSMaxRange(range)) {
            return;
        }
        _attributes[iTermExternalAttributeKeyToNumber(intKey + base + (int)_offset)] = obj;
    }];
}

@end

static NSString *const iTermExternalAttributeKeyUnderlineColor = @"uc";
static NSString *const iTermExternalAttributeKeyURLCode_Deprecated = @"url";
static NSString *const iTermExternalAttributeKeyBlockIDList = @"b";
static NSString *const iTermExternalAttributeKeyControlCode = @"cc";
static NSString *const iTermExternalAttributeKeyURL = @"u";

@interface iTermExternalAttribute ()
@property (atomic, readwrite) BOOL hasUnderlineColor;
@property (atomic, readwrite) VT100TerminalColorValue underlineColor;
@property (atomic, copy, readwrite) NSString *blockIDList;
@property (atomic, readwrite) iTermControlCodeAttribute controlCode;
@end

@implementation iTermExternalAttribute

static BOOL iTermControlCodeAttributeEqualsNumber(const iTermControlCodeAttribute *attr, NSNumber *number) {
    if (!number && !attr->valid) {
        return YES;
    }
    if (number && attr->valid && number.intValue == attr->code) {
        return YES;
    }
    return NO;
}

+ (iTermExternalAttribute *)attributeHavingUnderlineColor:(BOOL)hasUnderlineColor
                                           underlineColor:(VT100TerminalColorValue)underlineColor
                                                      url:(iTermURL *_Nullable)url
                                              blockIDList:(NSString *)blockIDList
                                              controlCode:(NSNumber *)code {
    if (!hasUnderlineColor && !url && blockIDList == nil && !code) {
        return nil;
    }
    static iTermExternalAttribute *last;
    if (last && last.hasUnderlineColor == hasUnderlineColor &&
        !memcmp(&last->_underlineColor, &underlineColor, sizeof(underlineColor)) &&
        [NSObject object:last.url isEqualToObject:url] &&
        [NSObject object:last.blockIDList isEqualToObject:blockIDList] &&
        iTermControlCodeAttributeEqualsNumber(&last->_controlCode, code)) {
        // Since this class is immutable, there's a nice optimization in reusing the last one created.
        return last;
    }
    if (hasUnderlineColor) {
        return [[self alloc] initWithUnderlineColor:underlineColor url:url blockIDList:blockIDList controlCode:code];
    }
    last = [[self alloc] initWithURL:url blockIDList:blockIDList controlCode:code];
    return last;
}

+ (instancetype)fromData:(NSData *)data {
    iTermTLVDecoder *decoder = [[iTermTLVDecoder alloc] initWithData:data];

    // v1
    BOOL hasUnderlineColor;
    if (![decoder decodeBool:&hasUnderlineColor]) {
        return nil;
    }
    VT100TerminalColorValue underlineColor = {0};
    if (hasUnderlineColor) {
        if (![decoder decodeInt:&underlineColor.red]) {
            return nil;
        }
        if (![decoder decodeInt:&underlineColor.green]) {
            return nil;
        }
        if (![decoder decodeInt:&underlineColor.blue]) {
            return nil;
        }
        int temp;
        if (![decoder decodeInt:&temp]) {
            return nil;
        }
        underlineColor.mode = temp;
    }

    // v2
    unsigned int urlCode = 0;
    [decoder decodeUnsignedInt:&urlCode];

    // V3
    NSData *blockData = [decoder decodeData];
    NSString *blockIDList = nil;
    if (blockData) {
        blockIDList = [[NSString alloc] initWithData:blockData encoding:NSUTF8StringEncoding];
        if (!blockIDList.length) {
            blockIDList = nil;
        }
    }

    int cc = -1;
    [decoder decodeInt:&cc];

    NSData *urlData = [decoder decodeData];
    iTermURL *url = nil;
    if (urlData || urlCode) {
        url = [iTermURL urlWithData:urlData code:urlCode];
    }

    if (!hasUnderlineColor && !blockIDList && !url) {
        return nil;
    }

    return [[self alloc] initWithUnderlineColor:underlineColor
                                            url:url
                                    blockIDList:blockIDList
                                    controlCode:cc >= 0 && cc < 256 ? @(cc) : nil];
}

+ (BOOL)externalAttribute:(iTermExternalAttribute *)lhs isEqualToExternalAttribute:(iTermExternalAttribute *)rhs {
    if (lhs == rhs) {
        return YES;
    }
    if (lhs && rhs) {
        return [lhs isEqualToExternalAttribute:rhs];
    }
    if (lhs && lhs.isDefault) {
        return YES;
    }
    return rhs.isDefault;
}

- (instancetype)init {
    return [super init];
}

- (instancetype)initWithUnderlineColor:(VT100TerminalColorValue)color
                                   url:(iTermURL *_Nullable)url
                           blockIDList:(NSString *)blockIDList
                           controlCode:(NSNumber *)code {
    self = [self init];
    if (self) {
        self.hasUnderlineColor = YES;
        self.underlineColor = color;
        _url = url;
        self.blockIDList = blockIDList;
        if (code) {
            self.controlCode = (iTermControlCodeAttribute){.valid = YES, .code = code.intValue};
        } else {
            self.controlCode = (iTermControlCodeAttribute){.valid = NO};
        }
    }
    return self;
}

- (instancetype)initWithURL:(iTermURL *)url blockIDList:(NSString *)blockIDList controlCode:(nonnull NSNumber *)code {
    self = [self init];
    if (self) {
        _url = url;
        self.blockIDList = blockIDList;
        if (code) {
            self.controlCode = (iTermControlCodeAttribute){.valid = YES, .code = code.intValue};
        } else {
            self.controlCode = (iTermControlCodeAttribute){.valid = NO};
        }
    }
    return self;
}

- (NSNumber *)controlCodeNumber {
    if (_controlCode.valid) {
        return @(_controlCode.code);
    }
    return nil;
}

// This is intended for users, so don't make it too wonky.
- (NSString *)humanReadableDescription {
    NSMutableArray<NSString *> *parts = [NSMutableArray arrayWithCapacity:4];
    if (_hasUnderlineColor) {
        [parts addObject:[NSString stringWithFormat:@"underline=%@",
                                                    VT100TerminalColorValueDescription(_underlineColor, YES)]];
    }
    if (_url) {
        [parts addObject:@"URL"];
    }
    if (_blockIDList) {
        [parts addObject:@"block"];
    }
    if (parts.count == 0) {
        return @"none";
    }
    if (_controlCode.valid) {
        [parts addObject:@"Control Code"];
    }
    return [parts componentsJoinedByString:@","];
}

- (NSString *)description {
    NSMutableArray<NSString *> *parts = [NSMutableArray arrayWithCapacity:4];
    if (_hasUnderlineColor) {
        [parts
            addObject:[NSString stringWithFormat:@"ulc=%@", VT100TerminalColorValueDescription(_underlineColor, YES)]];
    }
    if (_url) {
        [parts addObject:[NSString stringWithFormat:@"url=%@", _url]];
    }
    if (_blockIDList) {
        [parts addObject:[NSString stringWithFormat:@"block=%@", _blockIDList]];
    }
    if (_controlCode.valid) {
        [parts addObject:[NSString stringWithFormat:@"cc=%d", _controlCode.code]];
    }
    if (parts.count == 0) {
        return @"none";
    }
    return [parts componentsJoinedByString:@","];
}

- (NSData *)data {
    iTermTLVEncoder *encoder = [[iTermTLVEncoder alloc] init];
    [encoder encodeBool:_hasUnderlineColor];
    if (_hasUnderlineColor) {
        [encoder encodeInt:_underlineColor.red];
        [encoder encodeInt:_underlineColor.green];
        [encoder encodeInt:_underlineColor.blue];
        [encoder encodeInt:_underlineColor.mode];
    }
    [encoder encodeUnsignedInt:-1]; // urlCode, which is now deprecated and replaced with the URL itself.
    [encoder encodeData:[_blockIDList dataUsingEncoding:NSUTF8StringEncoding] ?: [NSData data]];
    if (_controlCode.valid) {
        [encoder encodeInt:_controlCode.code];
    } else {
        [encoder encodeInt:-1];
    }
    if (_url) {
        [encoder encodeData:_url.data];
    }
    return encoder.data;
}

- (instancetype)initWithDictionary:(NSDictionary *)dict {
    self = [super init];
    if (self) {
        self.blockIDList = [NSString castFrom:dict[iTermExternalAttributeKeyBlockIDList]];
        id obj = dict[iTermExternalAttributeKeyUnderlineColor];
        if (obj != nil && ![obj isKindOfClass:[NSNull class]]) {
            NSArray<NSNumber *> *values = [NSArray castFrom:obj];
            if (!values || values.count < 4) {
                return nil;
            }
            _hasUnderlineColor = YES;
            _underlineColor.mode = [values[0] intValue];
            _underlineColor.red = [values[1] intValue];
            _underlineColor.green = [values[2] intValue];
            _underlineColor.blue = [values[3] intValue];
        }
        const int urlCode = [dict[iTermExternalAttributeKeyURLCode_Deprecated] unsignedIntValue];
        if (urlCode) {
            _url = [iTermURL urlWithData:nil code:urlCode];
        } else {
            _url = [iTermURL urlWithData:dict[iTermExternalAttributeKeyURL] code:0];
        }
        NSNumber *cc = [NSNumber castFrom:dict[iTermExternalAttributeKeyControlCode]];
        if (cc && cc.intValue >= 0 && cc.intValue < 256) {
            _controlCode = (iTermControlCodeAttribute){.valid = YES, .code = cc.intValue};
        } else {
            _controlCode = (iTermControlCodeAttribute){.valid = NO};
        }
        if (!_hasUnderlineColor && !_url && !self.blockIDList && !_controlCode.valid) {
            return nil;
        }
    }
    return self;
}

- (NSDictionary *)dictionaryValue {
    return [@{
        iTermExternalAttributeKeyURL : _url.data ?: [NSNull null],
        iTermExternalAttributeKeyBlockIDList : self.blockIDList ?: [NSNull null],
        iTermExternalAttributeKeyUnderlineColor : _hasUnderlineColor
            ? @[ @(_underlineColor.mode), @(_underlineColor.red), @(_underlineColor.green), @(_underlineColor.blue) ]
            : [NSNull null],
        iTermExternalAttributeKeyControlCode : _controlCode.valid ? @(_controlCode.code) : [NSNull null]
    } dictionaryByRemovingNullValues];
}

- (id)copyWithZone:(NSZone *)zone {
    return self;
}

- (BOOL)isEqualToExternalAttribute:(iTermExternalAttribute *)rhs {
    if (![NSObject object:_url isEqualToObject:rhs.url]) {
        return NO;
    }
    if (_hasUnderlineColor != rhs.hasUnderlineColor) {
        return NO;
    }
    if (!_hasUnderlineColor && !rhs.hasUnderlineColor) {
        return YES;
    }
    if (_controlCode.valid != rhs.controlCode.valid) {
        return NO;
    }
    if (_controlCode.valid && _controlCode.code != rhs.controlCode.code) {
        return NO;
    }
    return !memcmp(&_underlineColor, &rhs->_underlineColor, sizeof(_underlineColor));
}

- (BOOL)isDefault {
    return !self.hasUnderlineColor && self.blockIDList == nil && self.controlCodeNumber == nil && self.url == nil;
}

- (BOOL)isEqual:(id)object {
    if (self == object) {
        return YES;
    }
    iTermExternalAttribute *other = [iTermExternalAttribute castFrom:object];
    if (!other) {
        return NO;
    }
    return [self isEqualToExternalAttribute:other];
}

@end

@implementation iTermUniformExternalAttributes {
    iTermExternalAttribute *_attr;
}

+ (instancetype)withAttribute:(iTermExternalAttribute *)attr {
    if (!attr) {
        return nil;
    }
    os_unfair_lock_lock(&sUniformExternalAttributeCacheLock);
    if (!sUniformExternalAttributeCache) {
        sUniformExternalAttributeCache = [[NSMapTable alloc]
            initWithKeyOptions:NSPointerFunctionsWeakMemory | NSPointerFunctionsObjectPointerPersonality
                  valueOptions:NSPointerFunctionsStrongMemory
                      capacity:64];
    }
    iTermUniformExternalAttributes *cached = [sUniformExternalAttributeCache objectForKey:attr];
    if (!cached) {
        cached = [[self alloc] initWithAttribute:attr];
        if (cached) {
            [sUniformExternalAttributeCache setObject:cached forKey:attr];
        }
    }
    os_unfair_lock_unlock(&sUniformExternalAttributeCacheLock);
    return cached;
}

- (instancetype)initWithAttribute:(iTermExternalAttribute *)attr {
    if (!attr) {
        return nil;
    }
    self = [super init];
    if (self) {
        _attr = attr;
    }
    return self;
}

- (instancetype)initWithDictionary:(NSDictionary *)dictionary {
    NSDictionary *value = dictionary[@"all"];
    if (!value) {
        return nil;
    }
    self = [super init];
    if (self) {
        _attr = [[iTermExternalAttribute alloc] initWithDictionary:value];
    }
    return self;
}

- (NSString *)description {
    return [NSString stringWithFormat:@"<%@: %p %@>", NSStringFromClass([self class]), self, _attr];
}

- (NSString *)shortDescriptionWithLength:(int)length {
    return [self description];
}

- (BOOL)isEmpty {
    return _attr == nil || _attr.isDefault;
}

- (NSDictionary *)dictionaryValue {
    return @{@"all" : _attr.dictionaryValue};
}

- (void)copyFrom:(id<iTermExternalAttributeIndexReading>)source
          source:(int)loadBase
     destination:(int)storeBase
           count:(int)count {
    [self doesNotRecognizeSelector:_cmd];
}

- (iTermExternalAttribute *)objectAtIndexedSubscript:(NSInteger)idx {
    return _attr;
}

- (iTermExternalAttribute *)attributeAtIndex:(int)i {
    return _attr;
}

- (iTermExternalAttributeIndex *)indexByDeletingFirst:(int)n {
    return self;
}

- (iTermExternalAttributeIndex *)subAttributesToIndex:(int)index {
    return self;
}

- (iTermExternalAttributeIndex *)subAttributesFromIndex:(int)index {
    return self;
}

- (iTermExternalAttributeIndex *)subAttributesFromIndex:(int)index maximumLength:(int)maxLength {
    return self;
}

- (id)mutableCopyWithZone:(NSZone *)zone {
    return [self copyWithZone:zone];
}

- (id)copyWithZone:(NSZone *)zone {
    return self;
}

- (void)eraseAt:(int)x {
    [self doesNotRecognizeSelector:_cmd];
}

- (void)eraseInRange:(VT100GridRange)range {
    [self doesNotRecognizeSelector:_cmd];
}

- (void)deleteRange:(NSRange)range {
}

- (void)setAttributes:(iTermExternalAttribute *)attributes at:(int)start count:(int)count {
    [self doesNotRecognizeSelector:_cmd];
}

- (void)setObject:(iTermExternalAttribute *_Nullable)ea atIndexedSubscript:(NSUInteger)i {
    [self doesNotRecognizeSelector:_cmd];
}

- (void)mutateAttributesFrom:(int)start
                          to:(int)end
                       block:(iTermExternalAttribute *_Nullable (^)(iTermExternalAttribute *_Nullable))block {
    [self doesNotRecognizeSelector:_cmd];
}

- (void)enumerateSortedKeysInRange:(NSRange)range block:(void (^)(NSNumber *key))block {
    for (NSUInteger i = 0; i < range.length; i++) {
        block(iTermExternalAttributeKeyToNumber((int)(range.location + i)));
    }
}

@end

@interface NSMutableData (ScreenCharMigration)
- (void)migrateV2ToV3InPlace;
@end

@implementation NSMutableData (ScreenCharMigration)
- (void)migrateV2ToV3InPlace {
    screen_char_t *chars = (screen_char_t *)self.mutableBytes;
    for (NSUInteger i = 0; i < self.length / sizeof(screen_char_t); i++) {
        if (!chars[i].complexChar && !chars[i].image && chars[i].code >= ITERM2_LEGACY_PRIVATE_BEGIN &&
            chars[i].code <= ITERM2_LEGACY_PRIVATE_END) {
            chars[i].code = ITERM2_PRIVATE_BEGIN + (chars[i].code - ITERM2_LEGACY_PRIVATE_BEGIN);
        }
    }
}
@end

@implementation NSData (iTermExternalAttributes)

- (void)migrateV2ToV1:(NSMutableData *)modern {
    screen_char_t *chars = (screen_char_t *)modern.mutableBytes;
    for (NSUInteger i = 0; i < self.length / sizeof(screen_char_t); i++) {
        if (!chars[i].complexChar && !chars[i].image && chars[i].code >= ITERM2_PRIVATE_BEGIN &&
            chars[i].code <= ITERM2_PRIVATE_END) {
            chars[i].code = ITERM2_LEGACY_PRIVATE_BEGIN + (chars[i].code - ITERM2_PRIVATE_BEGIN);
        }
    }
}

- (NSData *)migrateV1ToV3:(iTermExternalAttributeIndex **)indexOut {
    const legacy_screen_char_t *source = (legacy_screen_char_t *)self.bytes;
    const NSUInteger length = self.length;
    // BUG-f959: Guard against invalid length instead of crashing
    if (length >= NSUIntegerMax) {
        ELog(@"iTermExternalAttributeIndex: migrateV1ToV3 length %lu >= NSUIntegerMax", (unsigned long)length);
        if (indexOut)
            *indexOut = nil;
        return self;
    }
    // BUG-f960: Guard against misaligned data instead of crashing
    if (length % sizeof(screen_char_t) != 0) {
        ELog(@"iTermExternalAttributeIndex: migrateV1ToV3 length %lu not aligned to screen_char_t size %lu",
             (unsigned long)length, (unsigned long)sizeof(screen_char_t));
        if (indexOut)
            *indexOut = nil;
        return self;
    }
    // Static assertion for struct size equality (compile-time safe)
    _Static_assert(sizeof(legacy_screen_char_t) == sizeof(screen_char_t),
                   "legacy_screen_char_t and screen_char_t must be the same size");
    const NSUInteger count = length / sizeof(legacy_screen_char_t);
    NSUInteger firstURLIndex = 0;
    for (firstURLIndex = 0; firstURLIndex < count; firstURLIndex++) {
        if (source[firstURLIndex].urlCode) {
            break;
        }
    }
    if (firstURLIndex == count) {
        // Fast path - no URLs present.
        if (indexOut) {
            *indexOut = nil;
        }
        return self;
    }

    // Slow path - convert URLs to external attributes.
    NSMutableData *modern = [NSMutableData dataWithLength:length];
    legacy_screen_char_t *dest = (legacy_screen_char_t *)modern.mutableBytes;
    memmove(dest, self.bytes, length);
    iTermExternalAttributeIndex *eaIndex = nil;
    for (NSUInteger i = firstURLIndex; i < count; i++) {
        if (dest[i].urlCode) {
            if (!eaIndex) {
                eaIndex = [[iTermExternalAttributeIndex alloc] init];
            }

            iTermURL *url = [iTermURL urlWithData:nil code:dest[i].urlCode];
            iTermExternalAttribute *ea =
                [iTermExternalAttribute attributeHavingUnderlineColor:NO
                                                       underlineColor:(VT100TerminalColorValue) {}
                                                                  url:url
                                                          blockIDList:nil
                                                          controlCode:nil];
            eaIndex[i] = ea;
            // This is a little hinky. dest goes from being a pointer to legacy_screen_char_t to screen_char_t at this
            // point. There's a rule that you can safely initialize a screen_char_t with 0s, so regardless of what
            // future changes screen_char_t undergoes, it will always migrate to 0s in the fields formerly occupied by
            // urlCode.
            dest[i].urlCode = 0;
        }
    }
    [modern migrateV2ToV3InPlace];
    if (indexOut) {
        *indexOut = eaIndex;
    }
    return modern;
}

- (NSMutableData *)migrateV2ToV3 {
    NSMutableData *temp = [self mutableCopy];
    [temp migrateV2ToV3InPlace];
    return temp;
}

- (NSData *)legacyScreenCharArrayWithExternalAttributes:(iTermExternalAttributeIndex *_Nullable)eaIndex {
    const NSUInteger length = self.length;
    // BUG-f961: Guard against invalid length instead of crashing
    if (length >= NSUIntegerMax) {
        ELog(@"iTermExternalAttributeIndex: legacyScreenCharArrayWithExternalAttributes length %lu >= NSUIntegerMax",
             (unsigned long)length);
        return self;
    }
    // BUG-f962: Guard against misaligned data instead of crashing
    if (length % sizeof(screen_char_t) != 0) {
        ELog(@"iTermExternalAttributeIndex: legacyScreenCharArrayWithExternalAttributes length %lu not aligned to "
             @"screen_char_t size %lu",
             (unsigned long)length, (unsigned long)sizeof(screen_char_t));
        return self;
    }
    // Static assertion for struct size equality (compile-time safe)
    _Static_assert(sizeof(legacy_screen_char_t) == sizeof(screen_char_t),
                   "legacy_screen_char_t and screen_char_t must be the same size");

    NSMutableData *legacyData = [self mutableCopy];
    [self migrateV2ToV1:legacyData];
    // The deprecation of URL codes means that they won't survive downgrading.

    return legacyData;
}

@end

@implementation iTermURL

+ (instancetype _Nullable)urlWithData:(NSData *_Nullable)data code:(int)code {
    return [[self alloc] initWithData:data code:code];
}

- (instancetype)initWithData:(NSData *)data code:(int)code {
    if (!data.length && code == 0) {
        return nil;
    }

    static NSData *lastData;
    static int lastCode;
    static iTermURL *lastURL;

    self = [super init];
    if (self) {
        if (data) {
            if (lastURL != nil && [NSObject object:data isEqualToObject:lastData]) {
                return lastURL;
            }
            iTermTLVDecoder *decoder = [[iTermTLVDecoder alloc] initWithData:data];
            NSData *urlData = [decoder decodeData];
            if (!urlData) {
                return nil;
            }
            NSString *urlString = [[NSString alloc] initWithData:urlData encoding:NSUTF8StringEncoding];
            if (!urlString) {
                return nil;
            }
            _url = [NSURL URLWithString:urlString];
            if (!_url) {
                return nil;
            }

            NSData *idData = [decoder decodeData];
            if (idData.length > 0) {
                _identifier = [[NSString alloc] initWithData:idData encoding:NSUTF8StringEncoding];
            }

            NSData *targetData = [decoder decodeData];
            if (targetData.length > 0) {
                _target = [[NSString alloc] initWithData:targetData encoding:NSUTF8StringEncoding];
            }
        } else {
            if (code == lastCode && lastURL != nil) {
                return lastURL;
            }
            _url = [[iTermURLStore sharedInstance] urlForCode:code];
            if (!_url) {
                return nil;
            }
            _identifier = [[iTermURLStore sharedInstance] paramWithKey:@"id" forCode:code];
            _target = [[iTermURLStore sharedInstance] paramWithKey:@"target" forCode:code];
        }
    }

    lastData = [data copy];
    lastCode = code;
    lastURL = self;

    return self;
}

+ (instancetype)urlWithURL:(NSURL *)url identifier:(NSString *_Nullable)identifier target:(NSString *_Nullable)target {
    return [[iTermURL alloc] initWithURL:url identifier:identifier target:target];
}

- (instancetype)initWithURL:(NSURL *)url identifier:(NSString *)identifier target:(NSString *_Nullable)target {
    static iTermURL *lastObject;

    if (lastObject != nil && [lastObject.url isEqual:url] &&
        [NSObject object:identifier isEqualToObject:lastObject.identifier] &&
        [NSObject object:target isEqualToObject:lastObject.target]) {
        return lastObject;
    }

    self = [super init];
    if (self) {
        _url = url;
        _identifier = [identifier copy];
        _target = [target copy];
    }

    lastObject = self;

    return self;
}

- (NSData *)data {
    iTermTLVEncoder *encoder = [[iTermTLVEncoder alloc] init];
    [encoder encodeData:[_url.absoluteString dataUsingEncoding:NSUTF8StringEncoding]];
    [encoder encodeData:[_identifier dataUsingEncoding:NSUTF8StringEncoding] ?: [NSData data]];
    [encoder encodeData:[_target dataUsingEncoding:NSUTF8StringEncoding] ?: [NSData data]];
    return [encoder data];
}

@end
