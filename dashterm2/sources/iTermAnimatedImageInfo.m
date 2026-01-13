//
//  iTermAnimatedImageInfo.m
//  DashTerm2
//
//  Created by George Nachman on 5/11/15.
//
//

#import "iTermAnimatedImageInfo.h"
#import "iTermImage.h"

@implementation iTermAnimatedImageInfo {
    iTermImage *_image;
    NSTimeInterval _creationTime;
    NSTimeInterval _maxDelay;
    int _lastFrameNumber;
}

- (instancetype)initWithImage:(iTermImage *)image {
    if (!image || image.delays.count == 0) {
        // Not animated or no image available.
        return nil;
    }
    self = [super init];
    if (self) {
        _image = image;
        _maxDelay = [_image.delays.lastObject doubleValue];
        _creationTime = [NSDate timeIntervalSinceReferenceDate];
    }
    return self;
}

- (void)setPaused:(BOOL)paused {
    _paused = paused;
}

- (int)frameForTimestamp:(NSTimeInterval)timestamp {
    if (_paused) {
        return _lastFrameNumber;
    }
    NSTimeInterval offset = timestamp - _creationTime;
    NSTimeInterval delay = fmod(offset, _maxDelay);
    for (int i = 0; i < _image.delays.count; i++) {
        if ([_image.delays[i] doubleValue] >= delay) {
            _lastFrameNumber = i;
            return i;
        }
    }
    _lastFrameNumber = 0;
    return 0;
}

- (int)currentFrame {
    if (_paused) {
        return _lastFrameNumber;
    }
    return [self frameForTimestamp:[NSDate timeIntervalSinceReferenceDate]];
}

- (NSImage *)currentImage {
    // BUG-10145: Add bounds check to prevent array index out of bounds crash.
    const int frame = self.currentFrame;
    NSArray *images = _image.images;
    if (frame < 0 || (NSUInteger)frame >= images.count) {
        return nil;
    }
    return images[frame];
}

- (NSImage *)imageForFrame:(int)frame {
    // BUG-10145: Add bounds check to prevent array index out of bounds crash.
    NSArray *images = _image.images;
    if (frame < 0 || (NSUInteger)frame >= images.count) {
        return nil;
    }
    return images[frame];
}

@end
