//
//  iTermImage+Sixel.m
//  DashTerm2SandboxedWorker
//
//  Created by Benedek Kozma on 2020. 12. 26..
//

#import "iTermImage+Sixel.h"
#import "iTermImage+Private.h"
#include "sixel.h"

@implementation NSImage(ImageDecoder)

+ (instancetype)imageWithRawData:(NSData *)data
                            size:(NSSize)size
                      scaledSize:(NSSize)scaledSize
                   bitsPerSample:(NSInteger)bitsPerSample
                 samplesPerPixel:(NSInteger)samplesPerPixel
                        hasAlpha:(BOOL)hasAlpha
                  colorSpaceName:(NSString *)colorSpaceName {
    // BUG-10154: Use proper overflow-safe calculation instead of assert.
    // Calculate expected data length using NSUInteger to avoid overflow.
    const NSUInteger pixelCount = (NSUInteger)size.width * (NSUInteger)size.height;
    const NSUInteger bitsPerPixel = (NSUInteger)bitsPerSample * (NSUInteger)samplesPerPixel;
    const NSUInteger bytesPerPixel = bitsPerPixel / 8;
    const NSUInteger expectedLength = pixelCount * bytesPerPixel;

    // Validate input: ensure no overflow occurred and data length matches.
    if (size.width <= 0 || size.height <= 0 ||
        bitsPerSample <= 0 || samplesPerPixel <= 0 ||
        bitsPerPixel > 64 ||  // sanity check: no more than 64 bits per pixel
        pixelCount > NSUIntegerMax / bytesPerPixel ||
        data.length != expectedLength) {
        return nil;
    }

    NSBitmapImageRep *bitmapImageRep =
    [[NSBitmapImageRep alloc] initWithBitmapDataPlanes:nil  // allocate the pixel buffer for us
                                            pixelsWide:size.width
                                            pixelsHigh:size.height
                                         bitsPerSample:bitsPerSample
                                       samplesPerPixel:samplesPerPixel
                                              hasAlpha:hasAlpha
                                              isPlanar:NO
                                        colorSpaceName:colorSpaceName
                                           bytesPerRow:bytesPerPixel * (NSUInteger)size.width
                                          bitsPerPixel:bitsPerPixel];  // 0 means OS infers it
    if (!bitmapImageRep) {
        return nil;
    }

    memmove([bitmapImageRep bitmapData], data.bytes, data.length);

    NSImage *theImage = [[NSImage alloc] initWithSize:scaledSize];
    [theImage addRepresentation:bitmapImageRep];

    return theImage;
}

@end

static NSImage *DecodeSixelData(sixel_decoder_t *decoder, NSData *data) {
    unsigned char *pixels = NULL;
    int width = 0;
    int height = 0;
    unsigned char *palette = NULL;  // argb
    int ncolors = 0;
    if (data.length >= INT_MAX) {
        return nil;
    }
    // BUG-10136: Retain the mutable copy to prevent use-after-free during decode.
    // The previous code passed [[data mutableCopy] mutableBytes] directly, which
    // could deallocate the temporary before sixel_decode_raw completed.
    NSMutableData *mutableData = [data mutableCopy];
    SIXELSTATUS status = sixel_decode_raw((unsigned char *)[mutableData mutableBytes],
                                          (int)data.length,
                                          &pixels,
                                          &width,
                                          &height,
                                          &palette,
                                          &ncolors,
                                          NULL);
    if (status != SIXEL_OK || ncolors <= 0) {
        return nil;
    }

    // BUG-10137: Clamp ncolors to SIXEL_PALETTE_MAX to prevent reading beyond palette buffer.
    // The palette array returned by sixel_decode_raw is limited to SIXEL_PALETTE_MAX (256) entries.
    const int safeNcolors = MIN(ncolors, SIXEL_PALETTE_MAX);
    const int limit = safeNcolors - 1;

    // BUG-10146: Check for integer overflow before allocating RGBA buffer.
    // width and height are ints from sixel_decode_raw, and we need width * height * 4 bytes.
    // Use NSUInteger arithmetic to detect overflow.
    const NSUInteger pixelCount = (NSUInteger)width * (NSUInteger)height;
    const NSUInteger bytesNeeded = pixelCount * 4;
    // Check for overflow: if pixelCount is huge, multiplying by 4 could overflow or wrap.
    // Also validate reasonable image size limits (e.g., 256 MB max).
    const NSUInteger kMaxImageBytes = 256 * 1024 * 1024;  // 256 MB limit
    if (width <= 0 || height <= 0 || pixelCount > NSUIntegerMax / 4 || bytesNeeded > kMaxImageBytes) {
        free(palette);
        free(pixels);
        return nil;
    }

    NSMutableData *rgbaData = [NSMutableData dataWithLength:bytesNeeded];
    if (!rgbaData) {
        free(palette);
        free(pixels);
        return nil;
    }
    unsigned char *rgba = rgbaData.mutableBytes;
    const int stride = 3;
    // Use pixelCount (already validated) instead of width * height to avoid overflow in loop bound.
    for (NSUInteger i = 0; i < pixelCount; i++) {
        const unsigned char index = MAX(0, MIN(pixels[i], limit));
        rgba[i * 4 + 0] = palette[index * stride + 0];
        rgba[i * 4 + 1] = palette[index * stride + 1];
        rgba[i * 4 + 2] = palette[index * stride + 2];
        rgba[i * 4 + 3] = 0xff;
    }
    free(palette);
    free(pixels);
    return [NSImage imageWithRawData:rgbaData
                                size:NSMakeSize(width, height)
                          scaledSize:NSMakeSize(width, height)
                       bitsPerSample:8
                     samplesPerPixel:4
                            hasAlpha:YES
                      colorSpaceName:NSDeviceRGBColorSpace];
}


static NSImage *ImageFromSixelData(NSData *data) {
    NSData *newlineData = [@"\n" dataUsingEncoding:NSUTF8StringEncoding];
    NSRange range = [data rangeOfData:newlineData options:0 range:NSMakeRange(0, data.length)];
    if (range.location == NSNotFound) {
        return nil;
    }
    NSData *params = [data subdataWithRange:NSMakeRange(0, range.location)];
    NSData *payload = [data subdataWithRange:NSMakeRange(NSMaxRange(range), data.length - NSMaxRange(range))];
    NSString *paramString = [[NSString alloc] initWithData:params encoding:NSUTF8StringEncoding];
    if (!paramString) {
        return nil;
    }
    sixel_decoder_t *decoder;
    SIXELSTATUS status = sixel_decoder_new(&decoder, NULL);
    if (status != SIXEL_OK) {
        return nil;
    }
    NSArray<NSString *> *paramParts = [paramString componentsSeparatedByString:@";"];
    [paramParts enumerateObjectsUsingBlock:^(NSString * _Nonnull value, NSUInteger index, BOOL * _Nonnull stop) {
        sixel_decoder_setopt(decoder,
                             (int)index,
                             value.UTF8String);
    }];
    
    NSImage *image = DecodeSixelData(decoder, payload);
    sixel_decoder_unref(decoder);
    
    return image;
}

@implementation iTermImage(Sixel)

- (instancetype)initWithSixelData:(NSData *)data {
    self = [self init];
    if (self) {
        NSImage *image = ImageFromSixelData(data);
        if (!image) {
            return nil;
        }
        [self.images addObject:image];
        self.size = image.size;
        self.scaledSize = image.size;
    }
    return self;
}

@end
