//
//  iTermImage.h
//  DashTerm2
//
//  Created by George Nachman on 8/27/16.
//
//

#import <Cocoa/Cocoa.h>

// Images are decoded in a sandboxed worker by default. When the worker is not
// available, iTermImage falls back to decoding in-process (see iTermImage.m).

@interface iTermImage : NSObject<NSSecureCoding>

// For animated gifs, delays is 1:1 with images. For non-animated images, delays is empty.
@property(nonatomic, readonly) NSMutableArray<NSNumber *> *delays;
@property(nonatomic, readonly) NSSize size;
@property(nonatomic, readonly) NSSize scaledSize;
@property(nonatomic, readonly) NSMutableArray<NSImage *> *images;

// Animated GIFs are not supported through this interface.
+ (instancetype)imageWithNativeImage:(NSImage *)image;

// Decompresses in a sandboxed process. Returns nil if anything goes wrong.
+ (instancetype)imageWithCompressedData:(NSData *)data;

// Assumes it begins with DCS parameters followed by newline.
// Decompresses in a sandboxed processes. Returns nil if anything goes wrong.
+ (instancetype)imageWithSixelData:(NSData *)sixelData;

@end
