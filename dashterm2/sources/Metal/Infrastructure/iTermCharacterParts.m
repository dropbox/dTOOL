//
//  iTermCharacterParts.m
//  DashTerm2
//
//  Created by George Nachman on 12/15/17.
//

#import "iTermCharacterParts.h"

const int iTermTextureMapMaxCharacterParts = 5;
const int iTermTextureMapMiddleCharacterPart = (iTermTextureMapMaxCharacterParts / 2) * iTermTextureMapMaxCharacterParts + (iTermTextureMapMaxCharacterParts / 2);

// Optimization: Cache NSNumber objects for image part indices (0-24).
// Image parts represent positions in a 5x5 grid for character rendering.
// Total parts = iTermTextureMapMaxCharacterParts^2 = 5*5 = 25 values.
static const int kCachedImagePartCount = 25;
static NSNumber *sImagePartCache[kCachedImagePartCount];

NSNumber *iTermImagePartToNumber(int part) {
    if (part >= 0 && part < kCachedImagePartCount) {
        return sImagePartCache[part];
    }
    return @(part);
}

__attribute__((constructor))
static void iTermCharacterPartsInitializeCache(void) {
    for (int i = 0; i < kCachedImagePartCount; i++) {
        sImagePartCache[i] = @(i);
    }
}
