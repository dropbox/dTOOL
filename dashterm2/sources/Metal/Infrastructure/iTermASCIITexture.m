
//
//  iTermASCIITexture.m
//  DashTerm2
//
//  Created by George Nachman on 12/2/17.
//

#import "iTermASCIITexture.h"

#import "DebugLogging.h"
#import "iTermCache.h"
#import "iTermCharacterSource.h"
#import "iTermMalloc.h"

#import <os/lock.h>

const unsigned char iTermASCIITextureMinimumCharacter = 32;  // space
const unsigned char iTermASCIITextureMaximumCharacter = 126; // ~

static const NSInteger iTermASCIITextureCapacity =
    iTermASCIITextureOffsetCount * (iTermASCIITextureMaximumCharacter - iTermASCIITextureMinimumCharacter + 1);

@interface iTermASCIITextureCache : NSObject

+ (instancetype)sharedInstance;
- (iTermASCIITexture *)asciiTextureWithAttributes:(iTermASCIITextureAttributes)attributes
                                       descriptor:(iTermCharacterSourceDescriptor *)descriptor
                                           device:(id<MTLDevice>)device
                                         creation:(iTermASCIITexture * (^)(void))creation;

@end

@implementation iTermASCIITextureCache {
    iTermCache<NSDictionary *, iTermASCIITexture *> *_cache;
    os_unfair_lock _lock; // Performance optimization: os_unfair_lock has ~10x lower overhead than @synchronized
}

+ (instancetype)sharedInstance {
    static id instance;
    static dispatch_once_t onceToken;
    dispatch_once(&onceToken, ^{
        instance = [[iTermASCIITextureCache alloc] init];
    });
    return instance;
}

- (instancetype)init {
    self = [super init];
    if (self) {
        _cache = [[iTermCache alloc] initWithCapacity:256];
        _lock = OS_UNFAIR_LOCK_INIT;
    }
    return self;
}

- (iTermASCIITexture *)asciiTextureWithAttributes:(iTermASCIITextureAttributes)attributes
                                       descriptor:(iTermCharacterSourceDescriptor *)descriptor
                                           device:(id<MTLDevice>)device
                                         creation:(iTermASCIITexture * (^)(void))creation {
    id key = [self keyForAttributes:attributes descriptor:descriptor device:device];
    iTermASCIITexture *texture;
    os_unfair_lock_lock(&_lock);
    texture = _cache[key];
    if (!texture) {
        texture = creation();
        [self addAsciiTextureLocked:texture withAttributes:attributes descriptor:descriptor device:device];
    }
    os_unfair_lock_unlock(&_lock);
    DLog(@"Texture for %@ is %@", key, texture);
    return texture;
}

// NOTE: Only call this when _lock is held
- (void)addAsciiTextureLocked:(iTermASCIITexture *)texture
               withAttributes:(iTermASCIITextureAttributes)attributes
                   descriptor:(iTermCharacterSourceDescriptor *)descriptor
                       device:(id<MTLDevice>)device {
    id key = [self keyForAttributes:attributes descriptor:descriptor device:device];
    DLog(@"Add texture %@ for key %@", texture, key);
    _cache[key] = texture;
}

- (id)keyForAttributes:(iTermASCIITextureAttributes)attributes
            descriptor:(iTermCharacterSourceDescriptor *)descriptor
                device:(id<MTLDevice>)device {
    return @{
        @"attributes" : @(attributes),
        @"descriptor" : descriptor.dictionaryValue,
        @"device" : [NSValue valueWithPointer:(__bridge const void *_Nullable)(device)]
    };
}

@end

@implementation iTermASCIITexture

- (instancetype)initWithAttributes:(iTermASCIITextureAttributes)attributes
                        descriptor:(iTermCharacterSourceDescriptor *)descriptor
                            device:(id<MTLDevice>)device
                          creation:(NSDictionary<NSNumber *, iTermCharacterBitmap *> *_Nonnull (^)(
                                       char, iTermASCIITextureAttributes))creation {
    self = [super init];
    if (self) {
        _parts = (iTermASCIITextureParts *)iTermCalloc(128, sizeof(iTermASCIITextureParts));
        _attributes = attributes;
        _textureArray = [[iTermTextureArray alloc] initWithTextureWidth:descriptor.glyphSize.width
                                                          textureHeight:descriptor.glyphSize.height
                                                            arrayLength:iTermASCIITextureCapacity
                                                            pixelFormat:MTLPixelFormatBGRA8Unorm
                                                                 device:device];
        _textureArray.texture.label = [NSString
            stringWithFormat:@"ASCII texture %@%@%@", (attributes & iTermASCIITextureAttributesBold) ? @"Bold" : @"",
                             (attributes & iTermASCIITextureAttributesItalic) ? @"Italic" : @"",
                             (attributes & iTermASCIITextureAttributesThinStrokes) ? @"ThinStrokes" : @""];

        for (int i = iTermASCIITextureMinimumCharacter; i <= iTermASCIITextureMaximumCharacter; i++) {
            NSDictionary<NSNumber *, iTermCharacterBitmap *> *dict = creation(i, attributes);
            iTermCharacterBitmap *left = dict[iTermImagePartToNumber(iTermImagePartFromDeltas(-1, 0))];
            iTermCharacterBitmap *center = dict[iTermImagePartToNumber(iTermImagePartFromDeltas(0, 0))];
            iTermCharacterBitmap *right = dict[iTermImagePartToNumber(iTermImagePartFromDeltas(1, 0))];
            if (left) {
                _parts[i] |= iTermASCIITexturePartsLeft;
                [_textureArray setSlice:iTermASCIITextureIndexOfCode(i, iTermASCIITextureOffsetLeft) withBitmap:left];
            }
            if (right) {
                _parts[i] |= iTermASCIITexturePartsRight;
                [_textureArray setSlice:iTermASCIITextureIndexOfCode(i, iTermASCIITextureOffsetRight) withBitmap:right];
            }
            if (center) {
                [_textureArray setSlice:iTermASCIITextureIndexOfCode(i, iTermASCIITextureOffsetCenter)
                             withBitmap:center];
            } else {
                ELog(@"Couldn't produce image for ascii %d", i);
            }
        }
    }
    return self;
}

- (void)dealloc {
    free(_parts);
}

@end

@implementation iTermASCIITextureGroup {
    iTermCharacterSourceDescriptor *_descriptor;
}

- (instancetype)initWithDevice:(id<MTLDevice>)device
            creationIdentifier:(id)creationIdentifier
                    descriptor:(iTermCharacterSourceDescriptor *)descriptor
                      creation:(NSDictionary<NSNumber *, iTermCharacterBitmap *> * (^)(
                                   char, iTermASCIITextureAttributes))creation {
    self = [super init];
    if (self) {
        _device = device;
        _descriptor = descriptor;
        _creationIdentifier = creationIdentifier;
        _creation = [creation copy];
        CGSize temp = [iTermTextureArray atlasSizeForUnitSize:descriptor.glyphSize
                                                  arrayLength:iTermASCIITextureCapacity
                                                  cellsPerRow:NULL];
        _atlasSize = simd_make_float2(temp.width, temp.height);
    }
    return self;
}

- (CGSize)glyphSize {
    return _descriptor.glyphSize;
}

- (iTermASCIITexture *)newASCIITextureForAttributes:(iTermASCIITextureAttributes)attributes {
    return [[iTermASCIITexture alloc] initWithAttributes:attributes
                                              descriptor:_descriptor
                                                  device:_device
                                                creation:_creation];
}

- (iTermASCIITexture *)asciiTextureForAttributes:(iTermASCIITextureAttributes)attributes {
    if (_textures[attributes]) {
        return _textures[attributes];
    }

    __weak __typeof(self) weakSelf = self;
    iTermASCIITexture *texture = [[iTermASCIITextureCache sharedInstance]
        asciiTextureWithAttributes:attributes
                        descriptor:_descriptor
                            device:_device
                          creation:^iTermASCIITexture * {
                              DLog(@"Create texture with attributes %@", @(attributes));
                              return [weakSelf newASCIITextureForAttributes:attributes];
                          }];
    _textures[attributes] = texture;
    return texture;
}

- (BOOL)isEqual:(id)object {
    if (![object isKindOfClass:[iTermASCIITextureGroup class]]) {
        return NO;
    }
    iTermASCIITextureGroup *other = object;
    return (CGSizeEqualToSize(other.glyphSize, self.glyphSize) && other.device == _device &&
            [other.creationIdentifier isEqual:_creationIdentifier]);
}

@end
