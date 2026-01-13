#import <XCTest/XCTest.h>
#import <Cocoa/Cocoa.h>
#import <objc/runtime.h>

#import "iTermImage.h"
#import "iTermSandboxedWorkerClient.h"

@interface iTermImageTest : XCTestCase
@end

@implementation iTermImageTest

typedef iTermImage * _Nullable (^iTermImageDecoderBlock)(NSData * _Nonnull data);

- (NSData *)onePixelPNGData {
    NSBitmapImageRep *rep = [[NSBitmapImageRep alloc] initWithBitmapDataPlanes:NULL
                                                                    pixelsWide:1
                                                                    pixelsHigh:1
                                                                 bitsPerSample:8
                                                               samplesPerPixel:4
                                                                      hasAlpha:YES
                                                                      isPlanar:NO
                                                                colorSpaceName:NSDeviceRGBColorSpace
                                                                   bytesPerRow:0
                                                                  bitsPerPixel:0];
    NSColor *color = [NSColor colorWithDeviceRed:1.0 green:0 blue:0 alpha:1.0];
    [rep setColor:color atX:0 y:0];
    NSData *data = [rep representationUsingType:NSBitmapImageFileTypePNG properties:@{}];
    XCTAssertNotNil(data);
    return data;
}

- (void)withStubbedWorkerReturning:(iTermImageDecoderBlock)block run:(dispatch_block_t)body {
    Method method = class_getClassMethod([iTermSandboxedWorkerClient class], @selector(imageFromData:));
    IMP original = method_getImplementation(method);
    IMP stub = imp_implementationWithBlock(^iTermImage * (id __unused _self, NSData *data) {
        return block ? block(data) : nil;
    });
    method_setImplementation(method, stub);
    @try {
        body();
    } @finally {
        method_setImplementation(method, original);
        imp_removeBlock(stub);
    }
}

- (void)testImageWithCompressedDataPrefersSandboxedWorkerResultWhenAvailable {
    NSData *data = [self onePixelPNGData];
    NSImage *nsImage = [[NSImage alloc] initWithData:data];
    iTermImage *expected = [iTermImage imageWithNativeImage:nsImage];
    [self withStubbedWorkerReturning:^iTermImage * _Nullable(NSData *decodedData) {
        XCTAssertEqualObjects(decodedData, data);
        return expected;
    } run:^{
        iTermImage *result = [iTermImage imageWithCompressedData:data];
        XCTAssertEqual(result, expected);
    }];
}

- (void)testImageWithCompressedDataFallsBackWhenSandboxedWorkerFails {
    NSData *data = [self onePixelPNGData];
    [self withStubbedWorkerReturning:^iTermImage * _Nullable(NSData * __unused decodedData) {
        return nil;
    } run:^{
        iTermImage *result = [iTermImage imageWithCompressedData:data];
        XCTAssertNotNil(result);
        XCTAssertEqual(result.size.width, 1);
        XCTAssertEqual(result.size.height, 1);
    }];
}

@end
