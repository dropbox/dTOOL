#import "LineBufferPosition.h"

#import "DebugLogging.h"

@implementation LineBufferPosition {
    long long absolutePosition_;
    int yOffset_;
    BOOL extendsToEndOfLine_;
}

@synthesize absolutePosition = absolutePosition_;
@synthesize yOffset = yOffset_;
@synthesize extendsToEndOfLine = extendsToEndOfLine_;

+ (LineBufferPosition *)position {
    return [[self alloc] init];
}

- (NSString *)description {
    return [NSString stringWithFormat:@"<%@: %p abs=%@ yoff=%@ extends=%@>",
            [self class], self,
            @(absolutePosition_), @(yOffset_), @(extendsToEndOfLine_)];
}

- (LineBufferPosition *)predecessor {
    LineBufferPosition *predecessor = [LineBufferPosition position];
    predecessor.absolutePosition = absolutePosition_;
    predecessor.yOffset = yOffset_;
    predecessor.extendsToEndOfLine = extendsToEndOfLine_;

    if (extendsToEndOfLine_) {
        predecessor.extendsToEndOfLine = NO;
    } else if (yOffset_ > 0) {
        predecessor.yOffset = yOffset_ - 1;
    } else if (absolutePosition_ > 0) {
        predecessor.absolutePosition = absolutePosition_ - 1;
    }

    return predecessor;
}

- (BOOL)isEqualToLineBufferPosition:(LineBufferPosition *)other {
    if (!other) {
        return NO;
    }
    return (absolutePosition_ == other->absolutePosition_ &&
            yOffset_ == other->yOffset_ &&
            extendsToEndOfLine_ == other->extendsToEndOfLine_);
}

- (NSComparisonResult)compare:(LineBufferPosition *)other {
    // Optimization: Direct integer comparison instead of NSNumber boxing.
    // This method is called frequently during search and selection operations.
    const long long selfPos = self.absolutePosition;
    const long long otherPos = other.absolutePosition;
    if (selfPos < otherPos) {
        return NSOrderedAscending;
    }
    if (selfPos > otherPos) {
        return NSOrderedDescending;
    }

    const int selfOffset = self.yOffset;
    const int otherOffset = other.yOffset;
    if (selfOffset < otherOffset) {
        return NSOrderedAscending;
    }
    if (selfOffset > otherOffset) {
        return NSOrderedDescending;
    }

    if (self.extendsToEndOfLine == other.extendsToEndOfLine) {
        return NSOrderedSame;
    } else if (self.extendsToEndOfLine) {
        return NSOrderedDescending;
    } else {
        return NSOrderedAscending;
    }
}

- (LineBufferPosition *)advancedBy:(int)cells {
    LineBufferPosition *pos = [[LineBufferPosition alloc] init];
    pos.absolutePosition = self.absolutePosition + cells;
    return pos;
}

- (NSString *)compactStringValue {
    return [NSString stringWithFormat:@"%lld:%d%@", self.absolutePosition,
            self.yOffset,
            self.extendsToEndOfLine ? @"E" : @""];
}

+ (instancetype)fromCompactStringValue:(NSString *)value {
    NSArray *components = [value componentsSeparatedByString:@":"];
    if (components.count != 2) {
        return nil;
    }

    NSString *absPositionStr = components[0];
    NSString *yOffsetStr = components[1];

    BOOL extends = NO;
    if ([yOffsetStr hasSuffix:@"E"]) {
        extends = YES;
        yOffsetStr = [yOffsetStr substringToIndex:yOffsetStr.length - 1];
    }

    LineBufferPosition *position = [[self alloc] init];
    position.absolutePosition = [absPositionStr longLongValue];
    position.yOffset = [yOffsetStr intValue];
    position.extendsToEndOfLine = extends;

    return position;
}
@end

@implementation LineBufferPositionRange : NSObject

- (NSString *)description {
    return [NSString stringWithFormat:@"<%@: %p start=%@ end=%@>", [self class], self, _start, _end];
}

@end

