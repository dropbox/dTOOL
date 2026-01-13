//
//  iTermImage.m
//  DashTerm2
//
//  Created by George Nachman on 8/27/16.
//
//

#import "iTermImage.h"
#import "iTermImage+Private.h"
#ifdef SANDBOXED_WORKER
#define DLog NSLog
#else
#import "DebugLogging.h"
#endif

#import "NSData+iTerm.h"
#import "NSImage+iTerm.h"
#import "iTermSandboxedWorkerClient.h"
#import "iTermImage+ImageWithData.h"

#import <math.h>

static const CGFloat kMaxDimension = 10000;

@interface iTermImage ()
@property (nonatomic, strong) NSMutableArray<NSNumber *> *delays;
@property (nonatomic, strong) NSMutableArray<NSImage *> *images;
@end

@implementation iTermImage

+ (instancetype)imageWithNativeImage:(NSImage *)nativeImage {
    iTermImage *image = [[iTermImage alloc] init];
    image.size = nativeImage.size;
    image->_scaledSize = nativeImage.size;
    [image.images addObject:nativeImage];
    return image;
}

+ (instancetype)imageWithCompressedData:(NSData *)compressedData {
    char *bytes = (char *)compressedData.bytes;
    if (compressedData.length > 2 && bytes[0] == 27 && bytes[1] == 'P') {
        return [self imageWithSixelData:compressedData];
    }
    iTermImage *image = [iTermSandboxedWorkerClient imageFromData:compressedData];
    if (!image) {
        DLog(@"Sandboxed worker failed to decode image data. Falling back to in-process decoder.");
        image = [[iTermImage alloc] initWithData:compressedData];
    }
    return image;
}

+ (instancetype)imageWithSixelData:(NSData *)sixelData {
    return [iTermSandboxedWorkerClient imageFromSixelData:sixelData];
}

- (instancetype)init {
    self = [super init];
    if (self) {
        _delays = [[NSMutableArray alloc] initWithCapacity:16]; // Frame delays for animated images
        _images = [[NSMutableArray alloc] initWithCapacity:16]; // Frames for animated images
    }
    return self;
}

#pragma mark - NSSecureCoding

+ (BOOL)supportsSecureCoding {
    return YES;
}
- (void)encodeWithCoder:(nonnull NSCoder *)coder {
    [coder encodeObject:self.delays forKey:@"delays"];
    [coder encodeSize:self.size forKey:@"size"];
    [coder encodeSize:self.scaledSize forKey:@"scaledSize"];
    NSMutableArray<NSData *> *imageDatas = [NSMutableArray new];
    for (NSImage *image in self.images) {
        NSData *imageData = [self dataForImage:image];
        [imageDatas addObject:imageData];
    }
    [coder encodeObject:imageDatas forKey:@"images"];
}

- (nullable instancetype)initWithCoder:(nonnull NSCoder *)coder {
    @try {
        _delays = [coder decodeObjectOfClasses:[NSSet setWithArray:@[ [NSMutableArray class], [NSNumber class] ]]
                                        forKey:@"delays"];
        _size = [coder decodeSizeForKey:@"size"];
        if (_size.width <= 0 || _size.width >= kMaxDimension || _size.height <= 0 || _size.height >= kMaxDimension) {
            DLog(@"Bogus size %@", NSStringFromSize(_size));
            return nil;
        }
        if ([coder containsValueForKey:@"scaledSize"]) {
            _scaledSize = [coder decodeSizeForKey:@"scaledSize"];
        } else {
            _scaledSize = _size;
        }
        _images = [NSMutableArray new];
        NSMutableArray<NSData *> *imageDatas =
            [coder decodeObjectOfClasses:[NSSet setWithArray:@[ [NSMutableArray class], [NSData class] ]]
                                  forKey:@"images"];
        for (NSData *imageData in imageDatas) {
            if (imageData.length != _size.width * _size.height * 4) {
                return nil;
            }
            NSImage *image = [NSImage imageWithRawData:imageData
                                                  size:_size
                                            scaledSize:_scaledSize
                                         bitsPerSample:8
                                       samplesPerPixel:4
                                              hasAlpha:YES
                                        colorSpaceName:NSDeviceRGBColorSpace];
            if (!image) {
                DLog(@"Failed to create NSImage from data");
                return nil;
            }
            [_images addObject:image];
        }
        if ((_delays.count != 0 || _images.count > 1) && _delays.count != _images.count) {
            DLog(@"delays.count=%@, images.count=%@", @(_delays.count), @(_images.count));
            return nil;
        }
    } @catch (NSException *exception) {
        DLog(@"Failed to decode image: %@", exception);
        return nil;
    }
    return self;
}

#pragma mark - Private

- (CGContextRef)newBitmapContextWithStorage:(NSMutableData *)data {
    NSSize size = self.size;
    if (!isfinite(size.width) || !isfinite(size.height) || size.width < 1 || size.height < 1 ||
        size.width >= kMaxDimension || size.height >= kMaxDimension) {
        DLog(@"Refusing to allocate bitmap context for invalid size %@", NSStringFromSize(size));
        return NULL;
    }
    const size_t width = (size_t)size.width;
    const size_t height = (size_t)size.height;
    const size_t bytesPerRow = width * 4;
    const size_t storageNeeded = bytesPerRow * height;
    [data setLength:storageNeeded];

    CGColorSpaceRef colorSpace = CGColorSpaceCreateDeviceRGB();
    CGContextRef context = CGBitmapContextCreate((void *)data.bytes, width, height, 8, bytesPerRow, colorSpace,
                                                 (CGBitmapInfo)kCGImageAlphaPremultipliedLast);
    CGColorSpaceRelease(colorSpace);
    if (!context) {
        return NULL;
    }

    return context;
}

- (NSData *)dataForImage:(NSImage *)image {
    // Estimate size: width * height * 4 bytes per pixel (RGBA)
    NSMutableData *storage =
        [[NSMutableData alloc] initWithCapacity:(NSUInteger)(self.size.width * self.size.height * 4)];
    NSBitmapImageRep *rep = ((NSBitmapImageRep *)image.representations.firstObject);
    if (![rep isKindOfClass:[NSBitmapImageRep class]]) {
        DLog(@"Only bitmap images should get to this point.");
        return storage;
    }
    CGContextRef context = [self newBitmapContextWithStorage:storage];
    // BUG-2759: Check for NULL context before using
    if (!context) {
        DLog(@"Failed to create bitmap context");
        return storage;
    }
    CGImageRef imageToDraw = rep.CGImage;
    CGContextDrawImage(context, NSMakeRect(0, 0, self.size.width, self.size.height), imageToDraw);
    CGContextRelease(context);
    return storage;
}

@end
