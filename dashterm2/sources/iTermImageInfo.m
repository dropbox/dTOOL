//
//  iTermImageInfo.m
//  DashTerm2
//
//  Created by George Nachman on 5/11/15.
//
//

#import "iTermImageInfo.h"

#import <os/lock.h>

#import "DebugLogging.h"
#import "DashTerm2SharedARC-Swift.h"
#import "iTermAnimatedImageInfo.h"
#import "iTermImage.h"
#import "iTermTuple.h"
#import "FutureMethods.h"
#import "NSData+iTerm.h"
#import "NSImage+iTerm.h"
#import "NSWorkspace+iTerm.h"
#import <UniformTypeIdentifiers/UniformTypeIdentifiers.h>

static NSString *const kImageInfoSizeKey = @"Size";
static NSString *const kImageInfoImageKey = @"Image"; // data
static NSString *const kImageInfoPreserveAspectRatioKey = @"Preserve Aspect Ratio";
static NSString *const kImageInfoFilenameKey = @"Filename";
static NSString *const kImageInfoInsetKey = @"Edge Insets";
static NSString *const kImageInfoCodeKey = @"Code";
static NSString *const kImageInfoBrokenKey = @"Broken";

NSString *const iTermImageDidLoad = @"iTermImageDidLoad";

@interface iTermImageInfo ()

@property (atomic, strong) NSMutableDictionary *embeddedImages; // frame number->downscaled image
@property (atomic, assign) unichar code;
@property (atomic, strong) iTermAnimatedImageInfo *animatedImage; // If animated GIF, this is nonnil
@end

@implementation iTermImageInfo {
    NSData *_data;
    NSString *_uniqueIdentifier;
    NSDictionary *_dictionary;
    void (^_queuedBlock)(void);
    BOOL _paused;
    iTermImage *_image;
    iTermAnimatedImageInfo *_animatedImage;
    os_unfair_lock _lock;
}

@synthesize image = _image;
@synthesize data = _data;
@synthesize code = _code;
@synthesize broken = _broken;
@synthesize paused = _paused;
@synthesize uniqueIdentifier = _uniqueIdentifier;
@synthesize size = _size;
@synthesize filename = _filename;

- (instancetype)initWithCode:(unichar)code {
    self = [super init];
    if (self) {
        _code = code;
        _lock = OS_UNFAIR_LOCK_INIT;
    }
    return self;
}

- (instancetype)initWithDictionary:(NSDictionary *)dictionary {
    self = [super init];
    if (self) {
        _size = [dictionary[kImageInfoSizeKey] sizeValue];
        _broken = [dictionary[kImageInfoBrokenKey] boolValue];
        _inset = [dictionary[kImageInfoInsetKey] futureEdgeInsetsValue];
        _data = [dictionary[kImageInfoImageKey] copy];
        _dictionary = [dictionary copy];
        _preserveAspectRatio = [dictionary[kImageInfoPreserveAspectRatioKey] boolValue];
        _filename = [dictionary[kImageInfoFilenameKey] copy];
        _code = [dictionary[kImageInfoCodeKey] shortValue];
        _lock = OS_UNFAIR_LOCK_INIT;
    }
    return self;
}

- (NSString *)description {
    return [NSString stringWithFormat:@"<%@: %p code=%@ size=%@ uniqueIdentifier=%@ filename=%@ broken=%@>", self.class,
                                      self, @(self.code), NSStringFromSize(self.size), self.uniqueIdentifier,
                                      self.filename, @(self.broken)];
}

- (NSString *)uniqueIdentifier {
    os_unfair_lock_lock(&_lock);
    if (!_uniqueIdentifier) {
        _uniqueIdentifier = [[[NSUUID UUID] UUIDString] copy];
    }
    NSString *result = _uniqueIdentifier;
    os_unfair_lock_unlock(&_lock);
    return result;
}

// Internal locked version - caller must hold _lock
- (void)loadFromDictionaryIfNeeded_locked {
    static dispatch_once_t onceToken;
    static dispatch_queue_t queue;
    static NSMutableArray *blocks;
    static os_unfair_lock sBlocksLock = OS_UNFAIR_LOCK_INIT;
    dispatch_once(&onceToken, ^{
        blocks = [[NSMutableArray alloc] initWithCapacity:16];
        queue = dispatch_queue_create("com.dashterm.dashterm2.LazyImageDecoding", DISPATCH_QUEUE_SERIAL);
    });

    if (!_dictionary) {
        if (_queuedBlock) {
            // Move to the head of the queue.
            os_unfair_lock_lock(&sBlocksLock);
            NSUInteger index = [blocks indexOfObjectIdenticalTo:_queuedBlock];
            if (index != NSNotFound) {
                [blocks removeObjectAtIndex:index];
                [blocks insertObject:_queuedBlock atIndex:0];
            }
            os_unfair_lock_unlock(&sBlocksLock);
        }
        return;
    }

    _dictionary = nil;

    // Cache uniqueIdentifier while holding lock
    NSString *uniqueId = _uniqueIdentifier;
    if (!uniqueId) {
        _uniqueIdentifier = [[[NSUUID UUID] UUIDString] copy];
        uniqueId = _uniqueIdentifier;
    }
    DLog(@"Queueing load of %@", uniqueId);
    void (^block)(void) = ^{
        // This is a slow operation that blocks for a long time.
        iTermImage *image = [iTermImage imageWithCompressedData:self->_data];
        void (^mainThreadBlock)(void) = ^{
            self->_queuedBlock = nil;
            self->_animatedImage = [[iTermAnimatedImageInfo alloc] initWithImage:image];
            if (!self->_animatedImage) {
                self->_image = image;
            }
            if (self->_image || self->_animatedImage) {
                DLog(@"Loaded %@", uniqueId);
                [[NSNotificationCenter defaultCenter] postNotificationName:iTermImageDidLoad object:self];
            }
        };
        // Always hop to the main queue without risking a sync deadlock.
        if ([NSThread isMainThread]) {
            mainThreadBlock();
        } else {
            dispatch_async(dispatch_get_main_queue(), mainThreadBlock);
        }
    };
    _queuedBlock = [block copy];
    os_unfair_lock_lock(&sBlocksLock);
    [blocks insertObject:_queuedBlock atIndex:0];
    os_unfair_lock_unlock(&sBlocksLock);

    dispatch_async(queue, ^{
        void (^blockToRun)(void) = nil;
        os_unfair_lock_lock(&sBlocksLock);
        blockToRun = [blocks firstObject];
        [blocks removeObjectAtIndex:0];
        os_unfair_lock_unlock(&sBlocksLock);
        blockToRun();
    });
}

// Public API wrapper
- (void)loadFromDictionaryIfNeeded {
    os_unfair_lock_lock(&_lock);
    [self loadFromDictionaryIfNeeded_locked];
    os_unfair_lock_unlock(&_lock);
}

- (void)saveToFile:(NSString *)filename {
    NSData *data = [self dataForSavingFilename:filename];
    [data writeToFile:filename atomically:NO];
}

- (NSData *)dataForSavingFilename:(NSString *)filename {
    os_unfair_lock_lock(&_lock);
    NSBitmapImageFileType fileType = NSBitmapImageFileTypePNG;
    if ([filename hasSuffix:@".bmp"]) {
        fileType = NSBitmapImageFileTypeBMP;
    } else if ([filename hasSuffix:@".gif"]) {
        fileType = NSBitmapImageFileTypeGIF;
    } else if ([filename hasSuffix:@".jp2"]) {
        fileType = NSBitmapImageFileTypeJPEG2000;
    } else if ([filename hasSuffix:@".jpg"] || [filename hasSuffix:@".jpeg"]) {
        fileType = NSBitmapImageFileTypeJPEG;
    } else if ([filename hasSuffix:@".png"]) {
        fileType = NSBitmapImageFileTypePNG;
    } else if ([filename hasSuffix:@".tiff"]) {
        fileType = NSBitmapImageFileTypeTIFF;
    }

    NSData *data = nil;
    NSDictionary *universalTypeToCocoaMap = @{
        UTTypeBMP.identifier : @(NSBitmapImageFileTypeBMP),
        UTTypeGIF.identifier : @(NSBitmapImageFileTypeGIF),
        UTTypeJPEG.identifier : @(NSBitmapImageFileTypeJPEG),
        UTTypePNG.identifier : @(NSBitmapImageFileTypePNG),
        UTTypeTIFF.identifier : @(NSBitmapImageFileTypeTIFF)
    };
    // Use ivar _data directly to get imageType without re-acquiring lock
    NSString *imageType = [_data uniformTypeIdentifierForImageData];
    if (!imageType) {
        imageType = UTTypeImage.identifier;
    }
    if (_broken) {
        data = _data;
    } else if (imageType) {
        NSNumber *nsTypeNumber = universalTypeToCocoaMap[imageType];
        if (nsTypeNumber.integerValue == fileType) {
            data = _data;
        }
    }
    if (!data) {
        NSBitmapImageRep *rep = [_image.images.firstObject bitmapImageRep];
        data = [rep representationUsingType:fileType properties:@{}];
    }
    os_unfair_lock_unlock(&_lock);
    return data;
}

- (void)saveToItem:(iTermSavePanelItem *)item {
    NSData *data = [self dataForSavingFilename:item.filename];
    [data writeToSaveItem:item
        completionHandler:^(NSError *error) {
            if (!error) {
                [item revealInFinderIfLocal];
            }
        }];
}

- (void)setImageFromImage:(iTermImage *)image data:(NSData *)data {
    os_unfair_lock_lock(&_lock);
    _dictionary = nil;
    _animatedImage = [[iTermAnimatedImageInfo alloc] initWithImage:image];
    _data = [data copy];
    _image = image;
    os_unfair_lock_unlock(&_lock);
}

- (NSString *)imageType {
    os_unfair_lock_lock(&_lock);
    NSString *type = [_data uniformTypeIdentifierForImageData];
    os_unfair_lock_unlock(&_lock);
    if (type) {
        return type;
    }
    return UTTypeImage.identifier;
}

- (NSDictionary<NSString *, NSObject<NSCopying> *> *)dictionary {
    os_unfair_lock_lock(&_lock);
    NSDictionary *result = @{
        kImageInfoSizeKey : [NSValue valueWithSize:_size],
        kImageInfoInsetKey : [NSValue futureValueWithEdgeInsets:_inset],
        kImageInfoImageKey : _data ?: [NSData data],
        kImageInfoPreserveAspectRatioKey : @(_preserveAspectRatio),
        kImageInfoFilenameKey : _filename ?: @"",
        kImageInfoCodeKey : @(_code),
        kImageInfoBrokenKey : @(_broken)
    };
    os_unfair_lock_unlock(&_lock);
    return result;
}


- (BOOL)animated {
    os_unfair_lock_lock(&_lock);
    BOOL result = !_paused && _animatedImage != nil;
    os_unfair_lock_unlock(&_lock);
    return result;
}

- (void)setPaused:(BOOL)paused {
    os_unfair_lock_lock(&_lock);
    _paused = paused;
    _animatedImage.paused = paused;
    os_unfair_lock_unlock(&_lock);
}

- (BOOL)paused {
    os_unfair_lock_lock(&_lock);
    BOOL result = _paused;
    os_unfair_lock_unlock(&_lock);
    return result;
}

- (void)setImage:(iTermImage *)image {
    os_unfair_lock_lock(&_lock);
    _image = image;
    os_unfair_lock_unlock(&_lock);
}

- (iTermImage *)image {
    os_unfair_lock_lock(&_lock);
    [self loadFromDictionaryIfNeeded_locked];
    iTermImage *result = _image;
    os_unfair_lock_unlock(&_lock);
    return result;
}

- (void)setAnimatedImage:(iTermAnimatedImageInfo *)animatedImage {
    os_unfair_lock_lock(&_lock);
    _animatedImage = animatedImage;
    os_unfair_lock_unlock(&_lock);
}

- (iTermAnimatedImageInfo *)animatedImage {
    os_unfair_lock_lock(&_lock);
    [self loadFromDictionaryIfNeeded_locked];
    iTermAnimatedImageInfo *result = _animatedImage;
    os_unfair_lock_unlock(&_lock);
    return result;
}

- (NSImage *)imageWithCellSize:(CGSize)cellSize scale:(CGFloat)scale {
    os_unfair_lock_lock(&_lock);
    NSImage *result = [self imageWithCellSize_locked:cellSize
                                           timestamp:[NSDate timeIntervalSinceReferenceDate]
                                               scale:scale];
    os_unfair_lock_unlock(&_lock);
    return result;
}

- (int)frameForTimestamp:(NSTimeInterval)timestamp {
    os_unfair_lock_lock(&_lock);
    int result = [_animatedImage frameForTimestamp:timestamp];
    os_unfair_lock_unlock(&_lock);
    return result;
}

- (BOOL)ready {
    os_unfair_lock_lock(&_lock);
    BOOL result = [self ready_locked];
    os_unfair_lock_unlock(&_lock);
    return result;
}

// Internal unlocked helper - caller must hold _lock
- (BOOL)ready_locked {
    [self loadFromDictionaryIfNeeded_locked];
    return (_image || _animatedImage);
}
static NSSize iTermImageInfoGetSizeForRegionPreservingAspectRatio(const NSSize region, NSSize imageSize) {
    double imageAR = imageSize.width / imageSize.height;
    double canvasAR = region.width / region.height;
    if (imageAR > canvasAR) {
        // Image is wider than canvas, add letterboxes on top and bottom.
        return NSMakeSize(region.width, region.width / imageAR);
    } else {
        // Image is taller than canvas, add pillarboxes on sides.
        return NSMakeSize(region.height * imageAR, region.height);
    }
}

// NOTE: This gets called off the main queue in the metal renderer.
// Internal locked version - caller must hold _lock
- (NSImage *)imageWithCellSize_locked:(CGSize)cellSize timestamp:(NSTimeInterval)timestamp scale:(CGFloat)scale {
    if (![self ready_locked]) {
        // Get uniqueIdentifier without re-acquiring lock
        NSString *uniqueId = _uniqueIdentifier;
        if (!uniqueId) {
            _uniqueIdentifier = [[[NSUUID UUID] UUIDString] copy];
            uniqueId = _uniqueIdentifier;
        }
        DLog(@"%@ not ready", uniqueId);
        return nil;
    }
    DLog(@"[%p imageWithCellSize:%@ timestamp:%@ scale:%@]", self, NSStringFromSize(cellSize), @(timestamp), @(scale));
    if (!_embeddedImages) {
        _embeddedImages = [[NSMutableDictionary alloc] initWithCapacity:4];
    }
    int frame = [_animatedImage frameForTimestamp:timestamp]; // 0 if not animated
    iTermTuple *key = [iTermTuple tupleWithObject:@(frame) andObject:@(scale)];
    NSImage *embeddedImage = _embeddedImages[key];
    DLog(@"embeddedImage=%@", embeddedImage);
    NSSize region = NSMakeSize(cellSize.width * _size.width, cellSize.height * _size.height);
    DLog(@"region=%@", NSStringFromSize(region));
    if (!NSEqualSizes(embeddedImage.size, region)) {
        DLog(@"Sizes differ. Resize.");
        NSImage *theImage;
        if (_animatedImage) {
            theImage = [_animatedImage imageForFrame:frame];
        } else {
            theImage = [_image.images firstObject];
        }
        DLog(@"theImage is %@", theImage);
        NSEdgeInsets inset = _inset;
        inset.top *= cellSize.height;
        inset.bottom *= cellSize.height;
        inset.left *= cellSize.width;
        inset.right *= cellSize.width;
        const NSRect destinationRect =
            NSMakeRect(inset.left, inset.bottom, MAX(0, region.width - inset.left - inset.right),
                       MAX(0, region.height - inset.top - inset.bottom));
        NSImage *canvas = [theImage safelyResizedImageWithSize:region destinationRect:destinationRect scale:scale];
        DLog(@"Assign %@ to %@", canvas, key);
        // BUG-5068: Use ivar directly for thread safety
        _embeddedImages[key] = canvas;
    }
    NSImage *image = _embeddedImages[key];
    DLog(@"return %@", image);
    return image;
}

// Public API wrapper
- (NSImage *)imageWithCellSize:(CGSize)cellSize timestamp:(NSTimeInterval)timestamp scale:(CGFloat)scale {
    os_unfair_lock_lock(&_lock);
    NSImage *result = [self imageWithCellSize_locked:cellSize timestamp:timestamp scale:scale];
    os_unfair_lock_unlock(&_lock);
    return result;
}

- (NSImage *)firstFrame {
    if (self.animatedImage) {
        return [self.animatedImage imageForFrame:0];
    } else {
        return [self.image.images firstObject];
    }
}

+ (NSEdgeInsets)fractionalInsetsStretchingToDesiredSize:(NSSize)desiredSize
                                              imageSize:(NSSize)imageSize
                                               cellSize:(NSSize)cellSize
                                          numberOfCells:(NSSize)numberOfCells {
    const NSSize region = NSMakeSize(cellSize.width * numberOfCells.width, cellSize.height * numberOfCells.height);
    const NSEdgeInsets pointInsets =
        NSEdgeInsetsMake(0, 0, region.height - desiredSize.height, region.width - desiredSize.width);
    return NSEdgeInsetsMake(pointInsets.top / cellSize.height, pointInsets.left / cellSize.width,
                            pointInsets.bottom / cellSize.height, pointInsets.right / cellSize.width);
}

+ (NSEdgeInsets)fractionalInsetsForPreservedAspectRatioWithDesiredSize:(NSSize)desiredSize
                                                          forImageSize:(NSSize)imageSize
                                                              cellSize:(NSSize)cellSize
                                                         numberOfCells:(NSSize)numberOfCells {
    const NSSize region = NSMakeSize(cellSize.width * numberOfCells.width, cellSize.height * numberOfCells.height);
    const NSSize size = iTermImageInfoGetSizeForRegionPreservingAspectRatio(desiredSize, imageSize);

    const NSEdgeInsets pointInsets = NSEdgeInsetsMake(0, 0, region.height - size.height, region.width - size.width);
    return NSEdgeInsetsMake(pointInsets.top / cellSize.height, pointInsets.left / cellSize.width,
                            pointInsets.bottom / cellSize.height, pointInsets.right / cellSize.width);
}

// Internal locked version - caller must hold _lock
- (NSString *)nameForNewSavedTempFile_locked {
    NSString *name = nil;
    if (_filename.pathExtension.length) {
        // The filename has an extension. Preserve its name in the tempfile's name,
        // and especially importantly, preserve its extension.
        NSString *suffix = [@"." stringByAppendingString:_filename.lastPathComponent];
        name = [[NSWorkspace sharedWorkspace] temporaryFileNameWithPrefix:@"DashTerm2." suffix:suffix];
    } else {
        // Empty extension case. Try to guess the extension.
        NSString *imageType = [_data uniformTypeIdentifierForImageData];
        if (!imageType) {
            imageType = UTTypeImage.identifier;
        }
        NSString *extension = [NSImage extensionForUniformType:imageType];
        if (extension) {
            extension = [@"." stringByAppendingString:extension];
        }
        name = [[NSWorkspace sharedWorkspace] temporaryFileNameWithPrefix:@"DashTerm2." suffix:extension];
    }
    [_data writeToFile:name atomically:NO];
    return name;
}

// Public API wrapper
- (NSString *)nameForNewSavedTempFile {
    os_unfair_lock_lock(&_lock);
    NSString *result = [self nameForNewSavedTempFile_locked];
    os_unfair_lock_unlock(&_lock);
    return result;
}

- (NSPasteboardItem *)pasteboardItem {
    os_unfair_lock_lock(&_lock);
    NSPasteboardItem *pbItem = [[NSPasteboardItem alloc] init];
    NSArray *types;
    // Use ivar _data directly to get imageType without re-acquiring lock
    NSString *imageType = [_data uniformTypeIdentifierForImageData];
    if (!imageType) {
        imageType = UTTypeImage.identifier;
    }
    if (imageType) {
        types = @[ NSPasteboardTypeFileURL, imageType ];
    } else {
        types = @[ NSPasteboardTypeFileURL ];
    }
    [pbItem setDataProvider:self forTypes:types];
    os_unfair_lock_unlock(&_lock);
    return pbItem;
}

#pragma mark - NSPasteboardItemDataProvider

- (void)pasteboard:(NSPasteboard *)pasteboard item:(NSPasteboardItem *)item provideDataForType:(NSString *)type {
    os_unfair_lock_lock(&_lock);
    if ([type isEqualToString:NSPasteboardTypeFileURL]) {
        NSURL *url = [NSURL fileURLWithPath:[self nameForNewSavedTempFile_locked]];
        [item setString:url.absoluteString forType:NSPasteboardTypeFileURL];
    } else {
        if ([type isEqualToString:UTTypeImage.identifier] && ![_data uniformTypeIdentifierForImageData]) {
            [item setData:_data forType:UTTypeImage.identifier];
        } else {
            [item setData:_data forType:type];
        }
    }
    os_unfair_lock_unlock(&_lock);
}

@end
