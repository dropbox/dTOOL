//
//  iTermCharacterParts.h
//  DashTerm2
//
//  Created by George Nachman on 12/15/17.
//

#import <Foundation/Foundation.h>

#ifdef __cplusplus
extern "C" {
#endif

extern const int iTermTextureMapMaxCharacterParts;
extern const int iTermTextureMapMiddleCharacterPart;

NS_INLINE int iTermImagePartDX(int part) {
    return (part % iTermTextureMapMaxCharacterParts) - (iTermTextureMapMaxCharacterParts / 2);
}

NS_INLINE int iTermImagePartDY(int part) {
    return (part / iTermTextureMapMaxCharacterParts) - (iTermTextureMapMaxCharacterParts / 2);
}

NS_INLINE int iTermImagePartFromDeltas(int dx, int dy) {
    const int radius = iTermTextureMapMaxCharacterParts / 2;
    return (dx + radius) + (dy + radius) * iTermTextureMapMaxCharacterParts;
}

// Returns a cached NSNumber for the given image part index.
// Part indices range from 0-24 (5x5 grid).
extern NSNumber *iTermImagePartToNumber(int part);

#if __cplusplus
}
#endif

