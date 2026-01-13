#import "SearchResult.h"

#import "DashTerm2SharedARC-Swift.h"
#import "VT100GridTypes.h"

@implementation SearchResult

+ (SearchResult *)searchResultFromX:(int)x y:(long long)y toX:(int)endX y:(long long)endY {
    SearchResult *result = [[SearchResult alloc] init];
    result.internalStartX = x;
    result.internalEndX = endX;
    result.internalAbsStartY = y;
    result.internalAbsEndY = endY;
    return result;
}

+ (instancetype)searchResultFromExternal:(iTermExternalSearchResult *)externalResult
                                   index:(long long)index {
    SearchResult *result = [[SearchResult alloc] init];
    result->_externalResult = externalResult;
    result->_externalNumLines = externalResult.numLines;
    result->_externalIndex = index;
    result->_externalAbsY = externalResult.absLine;
    return result;
}

+ (instancetype)withCoordRange:(VT100GridCoordRange)coordRange
                      overflow:(long long)overflow {
    VT100GridAbsCoordRange absRange = VT100GridAbsCoordRangeFromCoordRange(coordRange, overflow);
    return [self searchResultFromX:absRange.start.x
                                 y:absRange.start.y
                               toX:absRange.end.x
                                 y:absRange.end.y];
}

- (BOOL)isExternal {
    return _externalResult != nil;
}

- (BOOL)isEqualToSearchResult:(SearchResult *)other {
    if (self.isExternal != other.isExternal) {
        return NO;
    }
    if (self.isExternal) {
        return [self.externalResult isEqual:other.externalResult];
    } else {
        return (_internalStartX == other.internalStartX &&
                _internalEndX == other.internalEndX &&
                _internalAbsStartY == other.internalAbsStartY &&
                _internalAbsEndY == other.internalAbsEndY);
    }
}

- (NSString *)description {
    if (self.isExternal) {
        return [NSString stringWithFormat:@"<%@: %p absLine=%@ index=%@ externalResult=%@>",
                NSStringFromClass([self class]), self, @(self.externalAbsY), @(self.externalIndex),
                self.externalResult];
    }
    return [NSString stringWithFormat:@"<%@: %p %d,%lld to %d,%lld>",
               [self class], self, _internalStartX, _internalAbsStartY, _internalEndX, _internalAbsEndY];
}

- (BOOL)isEqual:(id)object {
    if ([object isKindOfClass:[SearchResult class]]) {
        return [self isEqualToSearchResult:object];
    } else {
        return NO;
    }
}

- (NSUInteger)hash {
    if (self.isExternal) {
        return iTermCombineHash(iTermDJB2Hash((const unsigned char *)&_externalIndex,
                                              sizeof(_externalIndex)),
                                iTermDJB2Hash((const unsigned char *)&_externalAbsY,
                                              sizeof(_externalAbsY)));
    }
    return ((((((_internalStartX * 33) ^ _internalEndX) * 33) ^ _internalAbsStartY) * 33) ^ _internalAbsEndY);
}

- (NSComparisonResult)compare:(SearchResult *)other {
    if (!other) {
        return NSOrderedDescending;
    }
    if (self.isExternal && !other.isExternal) {
        if (self.externalAbsY < other.internalAbsStartY) {
            return NSOrderedAscending;
        } else if (self.externalAbsY == other.internalAbsStartY) {
            return NSOrderedSame;
        } else {
            return NSOrderedDescending;
        }
    }
    if (!self.isExternal && other.isExternal) {
        // BUG-7355: Was using externalAbsY for non-external self - should use internalAbsStartY
        if (self.internalAbsStartY < other.externalAbsY) {
            return NSOrderedAscending;
        } else if (self.internalAbsStartY == other.externalAbsY) {
            return NSOrderedSame;
        } else {
            return NSOrderedDescending;
        }
    }
    if (self.isExternal && other.isExternal) {
        // Optimization: Direct integer comparison instead of NSNumber boxing.
        // This method is called frequently during find operations.
        if (self.externalAbsY == other.externalAbsY) {
            if (self.externalIndex < other.externalIndex) {
                return NSOrderedAscending;
            }
            if (self.externalIndex > other.externalIndex) {
                return NSOrderedDescending;
            }
            return NSOrderedSame;
        }
        if (self.externalAbsY < other.externalAbsY) {
            return NSOrderedAscending;
        }
        return NSOrderedDescending;
    }
    return VT100GridAbsCoordOrder(VT100GridAbsCoordMake(_internalStartX,
                                                        _internalAbsStartY),
                                  VT100GridAbsCoordMake(other->_internalStartX,
                                                        other->_internalAbsStartY));
}

- (VT100GridAbsCoordRange)internalAbsCoordRange {
    // BUG-f1374: Convert assert to guard - return zero range for external results
    if (self.isExternal) {
        return VT100GridAbsCoordRangeMake(0, 0, 0, 0);
    }
    return VT100GridAbsCoordRangeMake(self.internalStartX,
                                      self.internalAbsStartY,
                                      self.internalEndX + 1,
                                      self.internalAbsEndY);
}

- (long long)safeAbsStartY {
    if (self.isExternal) {
        return self.externalAbsY;
    } else {
        return self.internalAbsStartY;
    }
}

- (long long)safeAbsEndY {
    if (self.isExternal) {
        return self.externalAbsY + self.externalNumLines - 1;
    } else {
        return self.internalAbsEndY;
    }
}
@end
