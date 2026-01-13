//
//  iTermSearchResultsMinimapView.m
//  DashTerm2
//
//  Created by George Nachman on 3/14/20.
//

#import "iTermSearchResultsMinimapView.h"

#import "DebugLogging.h"
#import "iTermMalloc.h"
#import "iTermRateLimitedUpdate.h"

// Optimization: Cache NSNumber objects for minimap series type indices (0-15).
// Typical minimap views have 1-3 series types (search results, marks, etc.).
// 16 entries covers all realistic use cases.
static const NSInteger kCachedMinimapTypeCount = 16;
static NSNumber *sMinimapTypeCache[kCachedMinimapTypeCount];

NS_INLINE NSNumber *iTermMinimapTypeToNumber(NSInteger type) {
    if (type >= 0 && type < kCachedMinimapTypeCount) {
        return sMinimapTypeCache[type];
    }
    return @(type);
}

__attribute__((constructor)) static void iTermSearchResultsMinimapViewInitializeTypeCache(void) {
    for (NSInteger i = 0; i < kCachedMinimapTypeCount; i++) {
        sMinimapTypeCache[i] = @(i);
    }
}

const CGFloat iTermSearchResultsMinimapViewItemHeight = 3;

typedef struct {
    CGColorRef outlineColor;
    CGColorRef fillColor;
    NSIndexSet *indexes;
} iTermMinimapSeries;

static NSString *const iTermBaseMinimapViewInvalidateNotification = @"iTermBaseMinimapViewInvalidateNotification";

@interface iTermBaseMinimapView () <CALayerDelegate>
@property (nonatomic, readonly) iTermRateLimitedUpdate *rateLimit;
@end

@implementation iTermBaseMinimapView {
    BOOL _invalid;
    BOOL _baseInitialized; // BUG-3114: Track if base initialization was done
}

- (instancetype)init {
    self = [super initWithFrame:NSZeroRect];
    if (self) {
        [self commonInit];
    }
    return self;
}

// BUG-3114: Support initWithFrame: to prevent crash when created via that initializer
- (instancetype)initWithFrame:(NSRect)frameRect {
    self = [super initWithFrame:frameRect];
    if (self) {
        [self commonInit];
    }
    return self;
}

// BUG-3114: Centralize common initialization to ensure it happens regardless of init path
- (void)commonInit {
    if (_baseInitialized) {
        return; // Already initialized
    }
    self.wantsLayer = YES;
    self.layer = [[CALayer alloc] init];
    self.layer.opaque = NO;
    self.layer.backgroundColor = [[NSColor clearColor] CGColor];
    self.layer.opacity = 0.62;
    self.layer.delegate = self;
    self.hidden = YES;
    [[NSNotificationCenter defaultCenter] addObserver:self
                                             selector:@selector(performInvalidateIfNeeded)
                                                 name:iTermBaseMinimapViewInvalidateNotification
                                               object:nil];
    _baseInitialized = YES;
}

- (void)dealloc {
    // BUG-3201: Clear layer delegate to prevent use-after-free if layer outlives view
    self.layer.delegate = nil;
    [[NSNotificationCenter defaultCenter] removeObserver:self];
}

// Use a shared rate limit so all the minimaps update in sync.
- (iTermRateLimitedUpdate *)rateLimit {
    static iTermRateLimitedUpdate *rateLimit;
    static dispatch_once_t onceToken;
    dispatch_once(&onceToken, ^{
        rateLimit = [[iTermRateLimitedUpdate alloc] initWithName:@"Minimap update" minimumInterval:0.25];
    });
    return rateLimit;
}

- (void)setHasData:(BOOL)hasData {
    if (hasData) {
        DLog(@"Unhiding %@", self);
        self.hidden = NO;
        [self.layer setNeedsDisplay];
    } else if (!self.hidden) {
        DLog(@"Hiding %@", self);
        self.hidden = YES;
    }
}

#pragma mark - NSView

- (void)viewDidMoveToWindow {
    DLog(@"viewDidMoveToWindow:%@", self.window);
    if (self.window == nil) {
        return;
    }
    self.layer.contentsScale = MAX(1, self.window.backingScaleFactor);
    [self.layer setNeedsDisplay];
}

#pragma mark - Private

static inline void iTermSearchResultsMinimapViewDrawItem(CGFloat offset, CGFloat width, CGContextRef context) {
    const CGRect boundingRect = CGRectMake(0, offset, width, iTermSearchResultsMinimapViewItemHeight);
    const CGRect strokeRect = CGRectInset(boundingRect, 0.5, 0.5);
    CGContextStrokeRect(context, strokeRect);
    const CGRect fillRect = CGRectInset(boundingRect, 1, 1);
    CGContextFillRect(context, fillRect);
}

#pragma mark - CALayerDelegate

- (void)drawLayer:(CALayer *)layer inContext:(CGContextRef)ctx {
    DLog(@"drawLayer:%@", layer);
    for (NSInteger i = 0; i < self.numberOfSeries; i++) {
        iTermMinimapSeries series = [self seriesAtIndex:i];
        CGContextSetFillColorWithColor(ctx, series.fillColor);
        CGContextSetStrokeColorWithColor(ctx, series.outlineColor);
        NSIndexSet *indexes = series.indexes;
        const NSRange rangeOfVisibleLines = [self rangeOfVisibleLines];
        CGFloat numberOfLines = rangeOfVisibleLines.length;
        // BUG-3118: Guard against division by zero when rangeOfVisibleLines has zero length
        if (numberOfLines == 0) {
            continue;
        }
        const CGFloat width = layer.bounds.size.width;
        const CGFloat layerHeight = layer.bounds.size.height;
        const CGFloat height = layerHeight - iTermSearchResultsMinimapViewItemHeight;
        __block CGFloat lastPointOffset = INFINITY;
        DLog(@"Draw %@ indexes in %@ lines with height %@ fill color %@",
             @([indexes countOfIndexesInRange:rangeOfVisibleLines]), @(rangeOfVisibleLines.length), @(height),
             series.fillColor);
        [indexes enumerateIndexesInRange:rangeOfVisibleLines
                                 options:0
                              usingBlock:^(NSUInteger idx, BOOL *_Nonnull stop) {
                                  const CGFloat fraction =
                                      (CGFloat)(idx - rangeOfVisibleLines.location) / numberOfLines;
                                  const CGFloat flippedFraction = 1.0 - fraction;
                                  const CGFloat pointOffset = round(flippedFraction * height);
                                  if (pointOffset + 2 > lastPointOffset) {
                                      return;
                                  }
                                  iTermSearchResultsMinimapViewDrawItem(pointOffset, width, ctx);
                                  lastPointOffset = pointOffset;
                              }];
    }
    [self didDraw];
}

#pragma mark - Subclassable

- (NSRange)rangeOfVisibleLines {
    return NSMakeRange(0, 0);
}

- (void)didDraw {
}

- (NSInteger)numberOfSeries {
    return 0;
}

- (iTermMinimapSeries)seriesAtIndex:(NSInteger)i {
    [self doesNotRecognizeSelector:_cmd];
    iTermMinimapSeries ignore = {0};
    return ignore;
}

- (void)invalidate {
    DLog(@"Invalidate");
    _invalid = YES;
    [self.rateLimit performRateLimitedSelector:@selector(postInvalidateNotification)
                                      onTarget:[iTermBaseMinimapView class]
                                    withObject:nil];
}

+ (void)postInvalidateNotification {
    [[NSNotificationCenter defaultCenter] postNotificationName:iTermBaseMinimapViewInvalidateNotification object:nil];
}

// All minimaps get this called when any minimaps anywhere was invalidated.
- (void)performInvalidateIfNeeded {
    if (!_invalid) {
        return;
    }
    _invalid = NO;
    [self performInvalidate];
}

// Subclasses to override
- (void)performInvalidate {
    [self doesNotRecognizeSelector:_cmd];
}

@end

@implementation iTermSearchResultsMinimapView {
    NSRange _rangeOfVisibleLines;
    iTermMinimapSeries _series;
    // BUG-3132: Store NSColor* (ARC-managed) instead of CGColorRef (manual memory management)
    // This eliminates the entire class of CFRetain/CFRelease bugs by design
    NSColor *_outlineNSColor;
    NSColor *_fillNSColor;
}

- (instancetype)init {
    self = [super init];
    if (self) {
        [self initializeColors];
    }
    return self;
}

// BUG-3114: Support initWithFrame: to prevent crash when created via that initializer
- (instancetype)initWithFrame:(NSRect)frameRect {
    self = [super initWithFrame:frameRect];
    if (self) {
        [self initializeColors];
    }
    return self;
}

// BUG-3132: Store NSColor* which ARC manages automatically
// CGColor is obtained lazily in seriesAtIndex: - no manual retain/release needed
- (void)initializeColors {
    if (_outlineNSColor != nil) {
        return; // Already initialized
    }
    _outlineNSColor = [NSColor colorWithRed:0.5 green:0.5 blue:0 alpha:1];
    _fillNSColor = [NSColor colorWithRed:1 green:1 blue:0 alpha:1];
}

// BUG-3132: No dealloc needed for colors - ARC manages NSColor* ivars automatically
// This eliminates the CFRelease(NULL) crash entirely by removing manual memory management

- (void)performInvalidate {
    _series.indexes = [self.delegate searchResultsMinimapViewLocations:self];
    _rangeOfVisibleLines = [self.delegate searchResultsMinimapViewRangeOfVisibleLines:self];
    const NSUInteger count = [_series.indexes countOfIndexesInRange:_rangeOfVisibleLines];
    DLog(@"Count is %@", @(count));
    [self setHasData:count > 0];
}

- (NSIndexSet *)indexSet {
    return _series.indexes ?: [self.delegate searchResultsMinimapViewLocations:self];
}

- (void)didDraw {
    _series.indexes = nil;
}

- (NSInteger)numberOfSeries {
    return 1;
}

- (iTermMinimapSeries)seriesAtIndex:(NSInteger)i {
    if (!_series.indexes) {
        _series.indexes = [self.delegate searchResultsMinimapViewLocations:self];
    }
    // BUG-3132: Get CGColor lazily from ARC-managed NSColor ivars
    // CGColor is valid for duration of drawing since NSColor ivars remain alive
    // No CFRetain needed - the CGColorRef is backed by the NSColor's internal state
    _series.outlineColor = [_outlineNSColor CGColor];
    _series.fillColor = [_fillNSColor CGColor];
    return _series;
}

- (NSRange)rangeOfVisibleLines {
    return _rangeOfVisibleLines;
}

@end

@implementation iTermIncrementalMinimapView {
    NSMutableDictionary<NSNumber *, NSMutableIndexSet *> *_sets;
    NSRange _visibleLines;
    iTermMinimapSeries *_series;
    NSInteger _numberOfSeries;
}

- (instancetype)initWithColors:(NSArray<iTermTuple<NSColor *, NSColor *> *> *)colors {
    self = [super init];
    if (self) {
        _sets = [[NSMutableDictionary alloc] initWithCapacity:colors.count];
        _series = iTermMalloc(sizeof(*_series) * colors.count);
        _numberOfSeries = colors.count;
        memset((void *)_series, 0, sizeof(*_series) * colors.count);
        [colors enumerateObjectsUsingBlock:^(iTermTuple<NSColor *, NSColor *> *_Nonnull obj, NSUInteger idx,
                                             BOOL *_Nonnull stop) {
            _series[idx].outlineColor = [colors[idx].firstObject CGColor];
            CFRetain(_series[idx].outlineColor);
            _series[idx].fillColor = [colors[idx].secondObject CGColor];
            CFRetain(_series[idx].fillColor);
        }];
    }
    return self;
}

- (void)dealloc {
    // BUG-2756/BUG-3114: Release retained CGColorRefs before freeing the struct
    // Only release if _series was actually allocated (could be NULL if created via initWithFrame:)
    if (_series) {
        for (NSInteger i = 0; i < _numberOfSeries; i++) {
            if (_series[i].outlineColor) {
                CGColorRelease(_series[i].outlineColor);
            }
            if (_series[i].fillColor) {
                CGColorRelease(_series[i].fillColor);
            }
        }
        free(_series);
    }
}

- (void)updateHidden {
    [self invalidate];
}

- (void)performInvalidate {
    for (NSMutableIndexSet *set in _sets.allValues) {
        if (set.count > 0) {
            [self setHasData:YES];
            [self.layer setNeedsDisplay];
            return;
        }
    }
    [self setHasData:NO];
}

- (void)addObjectOfType:(NSInteger)objectType onLine:(NSInteger)line {
    [_sets[iTermMinimapTypeToNumber(objectType)] addIndex:line];
    [self updateHidden];
}

- (void)removeObjectOfType:(NSInteger)objectType fromLine:(NSInteger)line {
    [_sets[iTermMinimapTypeToNumber(objectType)] removeIndex:line];
    [self updateHidden];
}

- (void)setFirstVisibleLine:(NSInteger)firstVisibleLine numberOfVisibleLines:(NSInteger)numberOfVisibleLines {
    // BUG-f970: Use guard instead of assert - clamp negative values to 0 with warning
    if (firstVisibleLine < 0) {
        DLog(@"BUG-f970: setFirstVisibleLine: called with negative value %ld - clamping to 0", (long)firstVisibleLine);
        firstVisibleLine = 0;
    }
    _visibleLines = NSMakeRange(firstVisibleLine, numberOfVisibleLines);
    [self updateHidden];
}

- (void)removeAllObjects {
    _sets = [[NSMutableDictionary alloc] initWithCapacity:_numberOfSeries];
    [self updateHidden];
}

- (void)setLines:(NSMutableIndexSet *)lines forType:(NSInteger)type {
    _sets[iTermMinimapTypeToNumber(type)] = lines;
    [self updateHidden];
}

- (NSInteger)numberOfSeries {
    return _numberOfSeries;
}

- (iTermMinimapSeries)seriesAtIndex:(NSInteger)i {
    _series[i].indexes = _sets[iTermMinimapTypeToNumber(i)];
    return _series[i];
}

- (NSRange)rangeOfVisibleLines {
    return _visibleLines;
}

@end
