//
//  iTermColorSuggester.m
//  DashTerm2
//
//  Created by George Nachman on 10/30/20.
//

#import "iTermColorSuggester.h"
#import "NSColor+iTerm.h"

typedef struct {
    CGFloat l;
    CGFloat theta;
} LightnessAndAngle;

// BUG-1599: Use thread-safe erand48 with local state instead of global drand48
static LightnessAndAngle RandomLightnessAndAngleWithState(unsigned short xsubi[3]) {
    LightnessAndAngle laa;
    laa.l = erand48(xsubi) * 100.0;
    if (laa.l < 50) {
        laa.l /= 2.0;
    } else {
        laa.l = 100 - (laa.l - 50) / 2.0;
    }
    laa.theta = erand48(xsubi) * M_PI * 2;
    return laa;
}

static iTermLABColor ClampedLAB(iTermLABColor lab) {
    // Round trip through rgb to keep it in gamut.
    return iTermLABFromSRGB(iTermSRGBFromLAB(lab));
}

static iTermLABColor TextLAB(LightnessAndAngle laa) {
    iTermLABColor lab;
    lab.l = laa.l;
    lab.a = sin(laa.theta) * 100.0;
    lab.b = cos(laa.theta) * 100.0;
    return ClampedLAB(lab);
}

static iTermLABColor BackgroundLAB(LightnessAndAngle laa) {
    const iTermLABColor lab = {
        .l = 100.0 - laa.l,
        .a = sin(laa.theta + M_PI_2) * 100.0,
        .b = cos(laa.theta + M_PI_2) * 100.0
    };
    return ClampedLAB(lab);
}

@implementation iTermColorSuggester

- (instancetype)initWithDefaultTextColor:(NSColor *)defaultTextColor
                  defaultBackgroundColor:(NSColor *)defaultBackgroundColor
                       minimumDifference:(CGFloat)minimumDifference
                                    seed:(long)seed {
    self = [super init];
    if (self) {
        const iTermLABColor defaultBackgroundLAB = [defaultBackgroundColor labColor];

        // BUG-1599: Use thread-local state instead of global srand48/drand48
        unsigned short xsubi[3];
        xsubi[0] = (unsigned short)(seed & 0xFFFF);
        xsubi[1] = (unsigned short)((seed >> 16) & 0xFFFF);
        xsubi[2] = (unsigned short)((seed >> 32) & 0xFFFF);

        iTermLABColor textLAB;
        iTermLABColor backgroundLAB;
        do {
            const LightnessAndAngle laa = RandomLightnessAndAngleWithState(xsubi);
            textLAB = TextLAB(laa);
            backgroundLAB = BackgroundLAB(laa);
        } while (fabs(backgroundLAB.l / 100.0 - defaultBackgroundLAB.l / 100.0) < minimumDifference);
        _suggestedTextColor = [NSColor withLABColor:textLAB];
        _suggestedBackgroundColor = [NSColor withLABColor:backgroundLAB];
    }
    return self;
}

@end
