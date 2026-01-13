#import "IntervalTree.h"
#import "DebugLogging.h"
#import "NSArray+iTerm.h"

static const long long kMinLocation = LLONG_MIN / 2;
static const long long kMaxLimit = kMinLocation + LLONG_MAX;

static NSString *const kIntervalTreeEntriesKey = @"Entries";
static NSString *const kIntervalTreeIntervalKey = @"Interval";
static NSString *const kIntervalTreeObjectKey = @"Object";
static NSString *const kIntervalTreeClassNameKey = @"Class";

static NSString *const kIntervalLocationKey = @"Location";
static NSString *const kIntervalLengthKey = @"Length";

@interface IntervalTreeValue : NSObject
@property (nonatomic, assign) long long maxLimitAtSubtree;
@property (nonatomic, retain) NSMutableArray *entries;

// Largest limit of all entries
@property (nonatomic, readonly) long long maxLimit;

// Interval including intervals of all entries at this entry exactly
- (Interval *)spanningInterval;

@end

@interface IntervalTreeForwardLocationEnumerator : NSEnumerator {
    long long previousLocation_;
    IntervalTree *tree_;
}
@property (nonatomic, assign) long long previousLocation;
@end

@implementation IntervalTreeForwardLocationEnumerator
@synthesize previousLocation = previousLocation_;

- (instancetype)initWithTree:(IntervalTree *)tree {
    self = [super init];
    if (self) {
        tree_ = [tree retain];
        previousLocation_ = -2;
    }
    return self;
}

- (void)dealloc {
    [tree_ release];
    [super dealloc];
}

- (NSArray *)allObjects {
    // Typically moderate number of objects in interval tree traversal
    NSMutableArray *result = [NSMutableArray arrayWithCapacity:32];
    NSObject *o = [self nextObject];
    while (o) {
        [result addObject:o];
    }
    return result;
}

- (id)nextObject {
    NSArray *objects;
    if (previousLocation_ == -1) {
        objects = [tree_ objectsWithSmallestLocation];
    } else if (previousLocation_ == -2) {
        return nil;
    } else {
        objects = [tree_ objectsWithSmallestLocationAfter:previousLocation_];
    }
    if (!objects.count) {
        previousLocation_ = -2;
    } else {
        id<IntervalTreeObject> obj = objects[0];
        previousLocation_ = [obj.entry.interval location];
    }
    return objects;
}
@end

@interface IntervalTreeForwardLimitEnumerator : NSEnumerator {
    long long previousLimit_;
    IntervalTree *tree_;
}
@property (nonatomic, assign) long long previousLimit;
@end

@implementation IntervalTreeForwardLimitEnumerator
@synthesize previousLimit = previousLimit_;

- (instancetype)initWithTree:(IntervalTree *)tree {
    self = [super init];
    if (self) {
        tree_ = [tree retain];
        previousLimit_ = -2;
    }
    return self;
}

- (void)dealloc {
    [tree_ release];
    [super dealloc];
}

- (NSArray *)allObjects {
    // Typically moderate number of objects in interval tree traversal
    NSMutableArray *result = [NSMutableArray arrayWithCapacity:32];
    NSObject *o = [self nextObject];
    while (o) {
        [result addObject:o];
    }
    return result;
}

- (id)nextObject {
    NSArray *objects;
    if (previousLimit_ == -2) {
        objects = [tree_ objectsWithSmallestLimit];
    } else if (previousLimit_ == -1) {
        return nil;
    } else {
        objects = [tree_ objectsWithSmallestLimitAfter:previousLimit_];
    }
    if (!objects.count) {
        previousLimit_ = -1;
    } else {
        id<IntervalTreeObject> obj = objects[0];
        previousLimit_ = [obj.entry.interval limit];
    }
    return objects;
}

@end

@interface IntervalTreeReverseLimitEnumerator : NSEnumerator {
    long long previousLimit_;
    IntervalTree *tree_;
}
@property (nonatomic, assign) long long previousLimit;
@end

@implementation IntervalTreeReverseLimitEnumerator

@synthesize previousLimit = previousLimit_;

- (instancetype)initWithTree:(IntervalTree *)tree {
    self = [super init];
    if (self) {
        tree_ = [tree retain];
        previousLimit_ = -2;
    }
    return self;
}

- (void)dealloc {
    [tree_ release];
    [super dealloc];
}

- (NSArray *)allObjects {
    // Typically moderate number of objects in interval tree traversal
    NSMutableArray *result = [NSMutableArray arrayWithCapacity:32];
    NSObject *o = [self nextObject];
    while (o) {
        [result addObject:o];
    }
    return result;
}

- (id)nextObject {
    NSArray *objects;
    if (previousLimit_ == -2) {
        objects = [tree_ objectsWithLargestLimit];
    } else if (previousLimit_ == -1) {
        return nil;
    } else {
        objects = [tree_ objectsWithLargestLimitBefore:previousLimit_];
    }
    if (!objects.count) {
        previousLimit_ = -1;
        return nil;
    } else {
        id<IntervalTreeObject> obj = objects[0];
        previousLimit_ = [obj.entry.interval limit];
        return objects;
    }
}

@end

@interface IntervalTreeReverseEnumerator : NSEnumerator {
    long long previousLocation_;
    IntervalTree *tree_;
}
@property (nonatomic, assign) long long previousLocation;
@end

@implementation IntervalTreeReverseEnumerator

@synthesize previousLocation = previousLocation_;

- (instancetype)initWithTree:(IntervalTree *)tree {
    self = [super init];
    if (self) {
        tree_ = [tree retain];
        previousLocation_ = -2;
    }
    return self;
}

- (void)dealloc {
    [tree_ release];
    [super dealloc];
}

- (NSArray *)allObjects {
    // Typically moderate number of objects in interval tree traversal
    NSMutableArray *result = [NSMutableArray arrayWithCapacity:32];
    NSObject *o = [self nextObject];
    while (o) {
        [result addObject:o];
    }
    return result;
}

- (id)nextObject {
    NSArray *objects;
    if (previousLocation_ == -2) {
        objects = [tree_ objectsWithLargestLocation];
    } else if (previousLocation_ == -1) {
        return nil;
    } else {
        objects = [tree_ objectsWithLargestLocationBefore:previousLocation_];
    }
    if (!objects.count) {
        previousLocation_ = -1;
        return nil;
    } else {
        id<IntervalTreeObject> obj = objects[0];
        previousLocation_ = [obj.entry.interval location];
        return objects;
    }
}

@end

@interface IntervalTreeSanitizingEnumerator<T> : NSEnumerator <T>

+ (instancetype)with:(NSEnumerator<IntervalTreeImmutableObject> *)source;

@end

@implementation IntervalTreeSanitizingEnumerator {
    NSEnumerator<IntervalTreeImmutableObject> *_source;
}

+ (instancetype)with:(NSEnumerator<IntervalTreeImmutableObject> *)source {
    return [[[self alloc] initWithSource:source] autorelease];
}

- (instancetype)initWithSource:(NSEnumerator<IntervalTreeImmutableObject> *)source {
    self = [super init];
    if (self) {
        _source = [source retain];
    }
    return self;
}

- (void)dealloc {
    [_source release];
    [super dealloc];
}

- (id)nextObject {
    return [[_source nextObject] mapWithBlock:^id _Nullable(id _Nonnull anObject) {
        return [anObject doppelganger];
    }];
}

- (NSUInteger)countByEnumeratingWithState:(NSFastEnumerationState *)state
                                  objects:(id __unsafe_unretained _Nullable[_Nonnull])buffer
                                    count:(NSUInteger)len {
    return [_source countByEnumeratingWithState:state objects:buffer count:len];
}

@end

@implementation Interval

+ (Interval *)intervalWithDictionary:(NSDictionary *)dict {
    if (!dict[kIntervalLocationKey] || !dict[kIntervalLengthKey]) {
        return nil;
    }
    return [self intervalWithLocation:[dict[kIntervalLocationKey] longLongValue]
                               length:[dict[kIntervalLengthKey] longLongValue]];
}

+ (Interval *)intervalWithLocation:(long long)location length:(long long)length {
    Interval *interval = [[[Interval alloc] initWithLocation:location length:length] autorelease];
    [interval boundsCheck];
    return interval;
}

+ (Interval *)maxInterval {
    Interval *interval = [[[Interval alloc] initWithLocation:kMinLocation length:kMaxLimit - kMinLocation] autorelease];
    return interval;
}

- (instancetype)initWithLocation:(long long)location length:(long long)length {
    self = [super init];
    if (self) {
        _location = location;
        _length = length;
    }
    return self;
}

- (long long)limit {
    return _location + _length;
}

- (BOOL)intersects:(Interval *)other {
    return MAX(self.location, other.location) < MIN(self.limit, other.limit);
}

- (NSString *)description {
    return [NSString stringWithFormat:@"<%@: %p [%lld, %lld)>", self.class, self, self.location, self.limit];
}

- (NSString *)shortDescription {
    return [NSString stringWithFormat:@"[%lld, %lld)", self.location, self.limit];
}

- (void)boundsCheck {
    // BUG-f680: Replace asserts with guards - invalid interval bounds should log error not crash
    if (_location < kMinLocation) {
        DLog(@"ERROR: Interval boundsCheck: location %lld < kMinLocation", _location);
        return;
    }
    if (_length < 0) {
        DLog(@"ERROR: Interval boundsCheck: length %lld < 0", _length);
        return;
    }
    if (_location > 0) {
        if (_location >= kMaxLimit - _length) {
            DLog(@"ERROR: Interval boundsCheck: location %lld >= kMaxLimit - length %lld", _location, _length);
            return;
        }
    } else {
        if (_location + _length >= kMaxLimit) {
            DLog(@"ERROR: Interval boundsCheck: location + length %lld >= kMaxLimit", _location + _length);
            return;
        }
    }
}

- (BOOL)isEqualToInterval:(Interval *)interval {
    return self.location == interval.location && self.length == interval.length;
}

- (NSDictionary *)dictionaryValue {
    return @{kIntervalLocationKey : @(_location), kIntervalLengthKey : @(_length)};
}

#pragma mark - NSCopying

- (instancetype)copyWithZone:(NSZone *)zone {
    return [[Interval intervalWithLocation:_location length:_length] retain];
}

@end

@implementation IntervalTreeEntry

+ (IntervalTreeEntry *)entryWithInterval:(Interval *)interval object:(id<IntervalTreeObject>)object {
    return [[[self alloc] initWithInterval:interval object:object] autorelease];
}

- (instancetype)initWithInterval:(Interval *)interval object:(id<IntervalTreeObject>)object {
    self = [super init];
    if (self) {
        // BUG-f681: Replace assert with guard - nil object should return nil not crash
        if (!object) {
            DLog(@"ERROR: IntervalTreeEntry initWithInterval: object is nil");
            [self release];
            return nil;
        }
        _interval = [interval retain];
        _object = [object retain];
    }
    return self;
}

- (void)dealloc {
    [_interval release];
    [_object release];
    [super dealloc];
}

- (NSString *)description {
    return [NSString stringWithFormat:@"<%@: %p interval=%@ object=%@>", self.class, self, self.interval, self.object];
}
@end

@implementation IntervalTreeValue

- (NSString *)description {
    // Each entry description is ~50 chars; allocate for all entries
    NSMutableString *entriesString = [NSMutableString stringWithCapacity:_entries.count * 50];
    for (IntervalTreeEntry *entry in _entries) {
        [entriesString appendFormat:@"%@, ", entry];
    }
    return [NSString stringWithFormat:@"<%@: %p maxLimitAtSubtree=%lld entries=[%@]>", self.class, self,
                                      self.maxLimitAtSubtree, entriesString];
}

- (instancetype)init {
    self = [super init];
    if (self) {
        // Typical nodes have 1-4 entries; pre-allocate to avoid growth
        _entries = [[NSMutableArray alloc] initWithCapacity:4];
    }
    return self;
}

- (void)dealloc {
    [_entries release];
    [super dealloc];
}

- (long long)maxLimit {
    long long max = -1;
    for (IntervalTreeEntry *entry in _entries) {
        max = MAX(max, [entry.interval limit]);
    }
    return max;
}

- (long long)location {
    // BUG-370: Guard against empty _entries array crash
    // If _entries is empty, return 0 as a safe default rather than crashing on [0] access
    if (_entries.count == 0) {
        return 0;
    }
    long long location = ((IntervalTreeEntry *)_entries[0]).interval.location;
    return location;
}

- (Interval *)spanningInterval {
    return [Interval intervalWithLocation:self.location length:[self maxLimit] - self.location];
}

@end

@implementation IntervalTree {
    AATree *_tree;
    int _count;
}

- (instancetype)initWithDictionary:(NSDictionary *)dict {
    self = [self init];
    if (self) {
        [self restoreFromDictionary:dict];
    }
    return self;
}

- (instancetype)init {
    self = [super init];
    if (self) {
        _tree = [[AATree alloc] initWithKeyComparator:^(NSNumber *key1, NSNumber *key2) {
            return [key1 compare:key2];
        }];
        // BUG-f682: Replace assert with guard - tree allocation failure should return nil not crash
        if (!_tree) {
            DLog(@"ERROR: IntervalTree init: failed to allocate AATree");
            [self release];
            return nil;
        }
        _tree.delegate = self;
    }
    return self;
}

- (void)dealloc {
    for (id<IntervalTreeObject> obj in [self objectsInInterval:[Interval maxInterval]]) {
        obj.entry = nil;
    }
    _tree.delegate = nil;
    [_tree release];
    [super dealloc];
}

- (NSString *)debugString {
    return [_tree debugStringWithDataFormatter:^NSString *(NSString *indent, id data) {
        IntervalTreeValue *value = data;
        // One string per entry
        NSMutableArray *strings = [NSMutableArray arrayWithCapacity:value.entries.count];
        for (IntervalTreeEntry *entry in value.entries) {
            [strings addObject:[NSString stringWithFormat:@"%@%@%@", indent, entry.interval.shortDescription,
                                                          entry.object.debugDescription]];
        }
        return [strings componentsJoinedByString:@"\n"];
    }];
}

- (void)restoreFromDictionary:(NSDictionary *)dict {
    for (NSDictionary *entry in dict[kIntervalTreeEntriesKey]) {
        NSDictionary *intervalDict = entry[kIntervalTreeIntervalKey];
        NSDictionary *objectDict = entry[kIntervalTreeObjectKey];
        NSString *className = entry[kIntervalTreeClassNameKey];
        if (intervalDict && objectDict && className) {
            Class theClass = NSClassFromString(className);
            if ([theClass instancesRespondToSelector:@selector(initWithDictionary:)]) {
                id<IntervalTreeObject> object = [[[theClass alloc] initWithDictionary:objectDict] autorelease];
                if (object) {
                    Interval *interval = [Interval intervalWithDictionary:intervalDict];
                    if (interval.limit >= 0) {
                        [self addObject:object withInterval:interval];
                    }
                }
            }
        }
    }
}

- (void)removeAllObjects {
    DLog(@"%p: removeAllObjects", self);
    for (id<IntervalTreeObject> obj in [self objectsInInterval:[Interval maxInterval]]) {
        obj.entry = nil;
    }
    _tree.delegate = nil;
    _count = 0;
    [_tree autorelease];
    _tree = [[AATree alloc] initWithKeyComparator:^(NSNumber *key1, NSNumber *key2) {
        return [key1 compare:key2];
    }];
    // BUG-f683: Replace assert with early return - tree allocation failure in removeAllObjects
    if (!_tree) {
        DLog(@"ERROR: IntervalTree removeAllObjects: failed to allocate new AATree");
        return;
    }
    _tree.delegate = self;
}

- (void)addObject:(id<IntervalTreeObject>)object withInterval:(Interval *)interval {
    DLog(@"%p: Add %@ at %@", self, object, interval);
#if DEBUG
    [self sanityCheck];
#endif
    [interval boundsCheck];
    // BUG-f684: Replace assert with guard - object already in tree should return not crash
    if (object.entry != nil) {
        DLog(@"ERROR: IntervalTree addObject: object already belongs to another tree");
        return;
    }
    IntervalTreeEntry *entry = [IntervalTreeEntry entryWithInterval:interval object:object];
    IntervalTreeValue *value = [_tree objectForKey:@(interval.location)];
    if (!value) {
        IntervalTreeValue *newValue = [[[IntervalTreeValue alloc] init] autorelease];
        [newValue.entries addObject:entry];
        [_tree setObject:newValue forKey:@(interval.location)];
    } else {
        [value.entries addObject:entry];
        [_tree notifyValueChangedForKey:@(interval.location)];
    }
    object.entry = entry;
    ++_count;
#if DEBUG
    [self sanityCheck];
#endif
}

- (BOOL)removeObject:(id<IntervalTreeObject>)object {
    DLog(@"%p: Remove %@", self, object);
#if DEBUG
    [self sanityCheck];
#endif
    Interval *interval = object.entry.interval;
    long long theLocation = interval.location;
    IntervalTreeValue *value = [_tree objectForKey:@(interval.location)];
    NSMutableArray *entries = value.entries;
    IntervalTreeEntry *entry = nil;
    int i;
    for (i = 0; i < entries.count; i++) {
        if ([((IntervalTreeEntry *)entries[i]).object isEqual:object]) {
            entry = entries[i];
            break;
        }
    }
    if (entry) {
        // BUG-f685: Replace assert with log - entry mismatch is a logic bug but shouldn't crash
        if (object.entry != entry) {
            DLog(@"ERROR: IntervalTree removeObject: object.entry mismatch - object may have been added to another "
                 @"tree");
        }
        object.entry = nil;
        [entries removeObjectAtIndex:i];
        if (entries.count == 0) {
            [_tree removeObjectForKey:@(theLocation)];
        } else {
            [_tree notifyValueChangedForKey:@(theLocation)];
        }
        --_count;
        return YES;
    } else {
        DLog(@"Failed to remove object not in tree");
        return NO;
    }
}

#pragma mark - Private

- (void)recalculateMaxLimitInSubtreeAtNode:(AATreeNode *)node removeFromToVisitList:(NSMutableSet *)toVisit {
    IntervalTreeValue *value = (IntervalTreeValue *)node.data;
    if (![toVisit containsObject:node]) {
        return;
    }

    [toVisit removeObject:node];
    long long max = [value maxLimit];
    DLog(@"Recalculate node %p with value %p: value.maxLimit=%@", node, value, @(max));
    if (node.left) {
        if ([toVisit containsObject:node.left]) {
            [self recalculateMaxLimitInSubtreeAtNode:node.left removeFromToVisitList:toVisit];
        }
        IntervalTreeValue *leftValue = (IntervalTreeValue *)node.left.data;
        max = MAX(max, leftValue.maxLimitAtSubtree);
        DLog(@"Recalculate node %p: leftValue.maxLimit=%@", node, @(leftValue.maxLimitAtSubtree));
    }
    if (node.right) {
        if ([toVisit containsObject:node.right]) {
            [self recalculateMaxLimitInSubtreeAtNode:node.right removeFromToVisitList:toVisit];
        }
        IntervalTreeValue *rightValue = (IntervalTreeValue *)node.right.data;
        max = MAX(max, rightValue.maxLimitAtSubtree);
        DLog(@"Recalculate node %p: rightValue.maxLimit=%@", node, @(rightValue.maxLimitAtSubtree));
    }
    DLog(@"Recalculate node %p with value %p: new maxLimitAtSubtree is %@", node, value, @(max));
    value.maxLimitAtSubtree = max;
#if DEBUG
    const long long bruteForced = [self bruteForceMaxLimitAtSubtree:node];
    assert(max == bruteForced);
#endif
}

#pragma mark - AATreeDelegate

#if DEBUG
- (BOOL)treeContainsNode:(AATreeNode *)node at:(AATreeNode *)current {
    if (!current) {
        return NO;
    }
    if (current == node) {
        return YES;
    }
    return [self treeContainsNode:node at:current.left] || [self treeContainsNode:node at:current.right];
}
#endif

- (void)aaTree:(AATree *)tree didChangeSubtreesAtNodes:(NSSet *)changedNodes {
    DLog(@"aaTree:didChangeSubtressAtNodes:");
    // Nodes to visit includes changed nodes plus their parents
    NSMutableSet *toVisit = [NSMutableSet setWithCapacity:changedNodes.count * 4];
    for (AATreeNode *node in changedNodes) {
        if (node.deleted) {
            continue;
        }
        [toVisit addObject:node];
        for (AATreeNode *parent in [tree pathFromNode:node]) {
            [toVisit addObject:parent];
        }
    }
    while (toVisit.count) {
        AATreeNode *node = [toVisit anyObject];
#if DEBUG
        assert(!node.deleted);
        assert([self treeContainsNode:node at:_tree.root]);
#endif
        DLog(@"  Recalculate node %p", node);
        [self recalculateMaxLimitInSubtreeAtNode:node removeFromToVisitList:toVisit];
    }
    DLog(@"Done with didChangeSubtreesAtNodes");
#if DEBUG
    [self sanityCheck];
#endif
}

- (void)aaTree:(AATree *)tree didChangeValueAtNode:(AATreeNode *)node {
    DLog(@"aaTree:didChangeValueAtNode:");
    NSArray *parents = [tree pathFromNode:node];
    NSMutableSet *parentSet = [NSMutableSet setWithArray:parents];
    for (AATreeNode *theNode in parents) {
        [self recalculateMaxLimitInSubtreeAtNode:theNode removeFromToVisitList:parentSet];
    }
#if DEBUG
    [self sanityCheck];
#endif
}

- (void)addObjectsInInterval:(Interval *)interval toArray:(NSMutableArray *)result fromNode:(AATreeNode *)node {
    IntervalTreeValue *nodeValue = (IntervalTreeValue *)node.data;
    if (nodeValue.maxLimitAtSubtree <= interval.location) {
        // The whole subtree has intervals that end before the requested |interval|.
        return;
    }

    Interval *nodeInterval = [nodeValue spanningInterval];
    if ([nodeInterval intersects:interval]) {
        // An entry at this node could possibly intersect the desired interval.
        for (IntervalTreeEntry *entry in nodeValue.entries) {
            if ([entry.interval intersects:interval]) {
                [result addObject:entry.object];
            }
        }
    }
    if (node.left) {
        // The requested interval includes points before this node's interval so we must search
        // intervals that start before this node.
        [self addObjectsInInterval:interval toArray:result fromNode:node.left];
    }
    if (interval.limit > nodeInterval.location && node.right) {
        [self addObjectsInInterval:interval toArray:result fromNode:node.right];
    }
}

- (NSArray<id<IntervalTreeObject>> *)mutableObjectsInInterval:(Interval *)interval {
    // Pre-allocate for typical query result size
    NSMutableArray *array = [NSMutableArray arrayWithCapacity:8];
    [self addObjectsInInterval:interval toArray:array fromNode:_tree.root];
    return array;
}

- (NSArray<id<IntervalTreeImmutableObject>> *)objectsInInterval:(Interval *)interval {
    return [self mutableObjectsInInterval:interval];
}

- (NSArray *)allObjects {
    return [self objectsInInterval:[Interval maxInterval]];
}

- (NSArray<id<IntervalTreeObject>> *)mutableObjects {
    return [self mutableObjectsInInterval:[Interval maxInterval]];
}

- (NSInteger)count {
    return _count;
}

- (NSString *)description {
    return [NSString stringWithFormat:@"<%@: %p tree=%@>", self.class, self, _tree.description];
}

- (BOOL)containsObject:(id<IntervalTreeObject>)object {
    IntervalTreeValue *value = [_tree objectForKey:@(object.entry.interval.location)];
    for (IntervalTreeEntry *entry in value.entries) {
        if (entry.object == object) {
            return YES;
        }
    }
    return NO;
}

- (NSArray *)objectsWithLargestLimitFromNode:(AATreeNode *)node {
    if (!node) {
        return nil;
    }
    long long myMaxLimit = ((IntervalTreeValue *)node.data).maxLimit;
    long long leftMaxLimit = ((IntervalTreeValue *)node.left.data).maxLimitAtSubtree;
    long long rightMaxLimit = ((IntervalTreeValue *)node.right.data).maxLimitAtSubtree;
    long long bestLimit = MAX(MAX(myMaxLimit, leftMaxLimit), rightMaxLimit);

    // Pre-allocate for typical result size (usually 1-4 objects)
    NSMutableArray *objects = [NSMutableArray arrayWithCapacity:4];
    if (myMaxLimit == bestLimit) {
        IntervalTreeValue *nodeValue = node.data;
        long long maxLimit = LLONG_MIN;
        for (IntervalTreeEntry *entry in nodeValue.entries) {
            if (entry.interval.limit > maxLimit) {
                [objects removeAllObjects];
                [objects addObject:entry.object];
                maxLimit = entry.interval.limit;
            } else if (entry.interval.limit == maxLimit) {
                [objects addObject:entry.object];
            }
        }
    }
    if (leftMaxLimit == bestLimit) {
        [objects addObjectsFromArray:[self objectsWithLargestLimitFromNode:node.left]];
    }
    if (rightMaxLimit == bestLimit) {
        [objects addObjectsFromArray:[self objectsWithLargestLimitFromNode:node.right]];
    }
    return objects.count ? objects : nil;
}

- (NSArray *)objectsWithSmallestLocationFromNode:(AATreeNode *)node {
    if (!node) {
        return nil;
    }
    if (node.left) {
        return [self objectsWithSmallestLimitFromNode:node.left];
    }
    if (node.data) {
        IntervalTreeValue *nodeValue = (IntervalTreeValue *)node.data;
        if (nodeValue.entries.count) {
            NSMutableArray *objects = [NSMutableArray arrayWithCapacity:nodeValue.entries.count];
            for (IntervalTreeEntry *entry in nodeValue.entries) {
                [objects addObject:entry.object];
            }
            return objects;
        }
    }
    return [self objectsWithSmallestLimitFromNode:node.right];
}

- (NSArray *)objectsWithSmallestLimitFromNode:(AATreeNode *)node {
    if (!node) {
        return nil;
    }
    // Searching for the smallest limit among node, node.left subtree, and node.right subtree
    // If node's key >= node.left's first object, don't search right subtree

    NSArray *objectsFromLeft = nil;
    if (node.left) {
        objectsFromLeft = [self objectsWithSmallestLimitFromNode:node.left];
    }

    Interval *nodeInterval = nil;
    // Pre-allocate for typical result size (usually 1-4 objects)
    NSMutableArray *myObjects = [NSMutableArray arrayWithCapacity:4];
    // Set nodeInterval to the best interval in this node's value.
    IntervalTreeValue *nodeValue = (IntervalTreeValue *)node.data;
    for (IntervalTreeEntry *entry in nodeValue.entries) {
        if (!nodeInterval) {
            [myObjects addObject:entry.object];
            nodeInterval = entry.interval;
        } else if (entry.interval.limit < nodeInterval.limit) {
            [myObjects removeAllObjects];
            [myObjects addObject:entry.object];
            nodeInterval = entry.interval;
        } else if (nodeInterval && entry.interval.limit == nodeInterval.limit) {
            [myObjects addObject:entry.object];
        }
    }

    NSArray *objectsFromRight = nil;
    // BUG-371: Guard against empty objectsFromLeft array crash
    // objectsFromLeft[0] would crash if objectsFromLeft is non-nil but empty
    id<IntervalTreeObject> leftValue = objectsFromLeft.count > 0 ? objectsFromLeft[0] : nil;
    if (node.right && (!objectsFromLeft.count || nodeInterval.location < leftValue.entry.interval.limit)) {
        objectsFromRight = [self objectsWithSmallestLimitFromNode:node.right];
    }

    // BUG-372: Guard against empty objectsFromRight array crash
    // objectsFromRight[0] would crash if objectsFromRight is non-nil but empty
    id<IntervalTreeObject> rightValue = objectsFromRight.count > 0 ? objectsFromRight[0] : nil;
    long long selfLimit = LLONG_MAX, leftLimit = LLONG_MAX, rightLimit = LLONG_MAX;
    if (nodeInterval) {
        selfLimit = nodeInterval.limit;
    }
    if (objectsFromLeft.count > 0) {
        leftLimit = leftValue.entry.interval.limit;
    }
    if (objectsFromRight.count > 0) {
        rightLimit = rightValue.entry.interval.limit;
    }
    long long bestLimit = MIN(MIN(selfLimit, leftLimit), rightLimit);
    // Pre-allocate for typical result size
    NSMutableArray *result = [NSMutableArray arrayWithCapacity:4];
    if (selfLimit == bestLimit) {
        [result addObjectsFromArray:myObjects];
    }
    if (leftLimit == bestLimit) {
        [result addObjectsFromArray:objectsFromLeft];
    }
    if (rightLimit == bestLimit) {
        [result addObjectsFromArray:objectsFromRight];
    }
    return result.count ? result : nil;
}

- (NSArray<id<IntervalTreeImmutableObject>> *_Nullable)objectsWithSmallestLocation {
    return [self objectsWithSmallestLocationFromNode:_tree.root];
}

- (NSArray<id<IntervalTreeImmutableObject>> *_Nullable)objectsWithSmallestLocationAfter:(long long)location {
    return [self objectsWithSmallestLocationAfter:location fromNode:_tree.root];
}

- (NSArray *)objectsWithSmallestLimit {
    return [self objectsWithSmallestLimitFromNode:_tree.root];
}

- (NSArray *)objectsWithLargestLimit {
    return [self objectsWithLargestLimitFromNode:_tree.root];
}

- (NSArray *)objectsWithLargestLocation {
    AATreeNode *node = _tree.root;
    while (node.right) {
        node = node.right;
    }
    IntervalTreeValue *value = node.data;
    // Pre-allocate based on known entries count
    NSMutableArray *objects = [NSMutableArray arrayWithCapacity:value.entries.count];
    for (IntervalTreeEntry *entry in value.entries) {
        [objects addObject:entry.object];
    }
    return objects;
}

- (NSArray *)objectsWithLargestLocationBefore:(long long)location {
    return [self objectsWithLargestLocationBefore:location atNode:_tree.root];
}

// Want objects with largest key < location
- (NSArray *)objectsWithLargestLocationBefore:(long long)location atNode:(AATreeNode *)node {
    if (!node) {
        return nil;
    }

    long long key = [node.key longLongValue];
    if (key >= location) {
        // If there is a left subtree, search it. If not, this will return an empty array.
        return [self objectsWithLargestLocationBefore:location atNode:node.left];
    }

    // key < location
    if (node.right) {
        // There is a larger value in the right subtree, but maybe there's nothing < location?
        NSArray *result = [self objectsWithLargestLocationBefore:location atNode:node.right];
        if (result) {
            return result;
        }
    }

    // If you get here then the there was no right subtree or the whole right subtrees had keys > location.
    // Return the objects in this node.
    IntervalTreeValue *value = node.data;
    // Pre-allocate based on known entries count
    NSMutableArray *objects = [NSMutableArray arrayWithCapacity:value.entries.count];
    for (IntervalTreeEntry *entry in value.entries) {
        [objects addObject:entry.object];
    }
    return objects;
}

- (NSArray<id<IntervalTreeImmutableObject>> *_Nullable)objectsWithSmallestLocationAfter:(long long)location
                                                                               fromNode:(AATreeNode *)node {
    if (!node) {
        return nil;
    }

    const long long key = [node.key longLongValue];
    if (key > location && node.left) {
        NSArray *left = [self objectsWithSmallestLocationAfter:location fromNode:node.left];
        if (left.count) {
            return left;
        }
    }
    if (key > location && node.data) {
        IntervalTreeValue *value = node.data;
        if (value) {
            // Pre-allocate based on known entries count
            NSMutableArray *objects = [NSMutableArray arrayWithCapacity:value.entries.count];
            for (IntervalTreeEntry *entry in value.entries) {
                [objects addObject:entry.object];
            }
            return objects;
        }
    }
    return [self objectsWithSmallestLocationAfter:location fromNode:node.right];
}

- (NSArray *)objectsWithSmallestLimitAfter:(long long)bound fromNode:(AATreeNode *)node {
    if (!node) {
        return nil;
    }
    // we can ignore all subtrees whose maxLimitAtSubtree is <= bound
    Interval *nodeInterval = nil;
    // Set nodeInterval to the best interval in this node's value.
    IntervalTreeValue *nodeValue = (IntervalTreeValue *)node.data;
    NSMutableArray *myObjects = nil;
    for (IntervalTreeEntry *entry in nodeValue.entries) {
        if (entry.interval.limit > bound && (!nodeInterval || entry.interval.limit < nodeInterval.limit)) {
            if (myObjects) {
                [myObjects removeAllObjects];
            } else {
                // Typically a few objects per node
                myObjects = [NSMutableArray arrayWithCapacity:4];
            }
            nodeInterval = entry.interval;
            [myObjects addObject:entry.object];
        } else if (nodeInterval && entry.interval.limit == nodeInterval.limit) {
            [myObjects addObject:entry.object];
        }
    }


    NSArray *leftObjects = nil;
    NSArray *rightObjects = nil;

    IntervalTreeValue *leftValue = (IntervalTreeValue *)node.left.data;
    IntervalTreeValue *rightValue = (IntervalTreeValue *)node.right.data;

    if (node.left && leftValue.maxLimitAtSubtree > bound) {
        leftObjects = [self objectsWithSmallestLimitAfter:bound fromNode:node.left];
    }

    long long thisLocation = [node.key longLongValue];

    // ignore right subtree if node's location > left subtree's smallest limit and left subtree's
    // smallest limit < bound (because every interval in the right subtree will have a limit larger
    // than this node's location, and the left subtree has an interval that ends before that
    // location).
    // BUG-373: Guard against empty leftObjects array crash
    // leftObjects[0] would crash if leftObjects is non-nil but empty
    id<IntervalTreeObject> bestLeft = leftObjects.count > 0 ? leftObjects[0] : nil;
    const BOOL thisNodesLocationIsAfterLeftSubtreesSmallestLimitAfterBound =
        (bestLeft && thisLocation > bestLeft.entry.interval.limit && bestLeft.entry.interval.limit > bound);
    if (node.right && rightValue.maxLimitAtSubtree > bound &&
        !thisNodesLocationIsAfterLeftSubtreesSmallestLimitAfterBound) {
        rightObjects = [self objectsWithSmallestLimitAfter:bound fromNode:node.right];
    }

    // BUG-374: Guard against empty rightObjects array crash
    // rightObjects[0] would crash if rightObjects is non-nil but empty
    id<IntervalTreeObject> bestRight = rightObjects.count > 0 ? rightObjects[0] : nil;
    long long selfLimit = LLONG_MAX, leftLimit = LLONG_MAX, rightLimit = LLONG_MAX;
    if (nodeInterval) {
        selfLimit = nodeInterval.limit;
    }
    if (bestLeft) {
        leftLimit = bestLeft.entry.interval.limit;
    }
    if (bestRight) {
        rightLimit = bestRight.entry.interval.limit;
    }
    long long bestLimit = MIN(MIN(selfLimit, leftLimit), rightLimit);
    // Pre-allocate for typical result size
    NSMutableArray *result = [NSMutableArray arrayWithCapacity:4];
    if (selfLimit == bestLimit) {
        [result addObjectsFromArray:myObjects];
    }
    if (leftLimit == bestLimit) {
        [result addObjectsFromArray:leftObjects];
    }
    if (rightLimit == bestLimit) {
        [result addObjectsFromArray:rightObjects];
    }

    return result.count ? result : nil;
}

- (NSArray *)objectsWithLargestLimitBelow:(long long)bound fromNode:(AATreeNode *)node {
    if (!node) {
        return nil;
    }
    Interval *nodeInterval = nil;
    long long thisLocation = [node.key longLongValue];

    // Set nodeInterval to the best interval in this node's value.
    IntervalTreeValue *nodeValue = (IntervalTreeValue *)node.data;
    NSMutableArray *myObjects = nil;
    for (IntervalTreeEntry *entry in nodeValue.entries) {
        if (entry.interval.limit < bound && (!nodeInterval || entry.interval.limit > nodeInterval.limit)) {
            if (myObjects) {
                [myObjects removeAllObjects];
            } else {
                // Typically a few objects per node
                myObjects = [NSMutableArray arrayWithCapacity:4];
            }
            nodeInterval = entry.interval;
            [myObjects addObject:entry.object];
        } else if (nodeInterval && entry.interval.limit == nodeInterval.limit) {
            [myObjects addObject:entry.object];
        }
    }

    NSArray *leftObjects = nil;
    NSArray *rightObjects = nil;
    IntervalTreeValue *leftValue = (IntervalTreeValue *)node.left.data;
    IntervalTreeValue *rightValue = (IntervalTreeValue *)node.right.data;

    if (node.left) {
        // Try to eliminate the left subtree from consideration if this node or the right subtree
        // is superior to it.
        const BOOL thisNodeBeatsWholeLeftSubtree =
            (nodeInterval && nodeInterval.limit < bound && nodeInterval.limit > leftValue.maxLimitAtSubtree);
        const BOOL rightSubtreeBeatsWholeLeftSubtree =
            (rightValue.maxLimitAtSubtree < bound && rightValue.maxLimitAtSubtree > leftValue.maxLimitAtSubtree);
        if (!thisNodeBeatsWholeLeftSubtree && !rightSubtreeBeatsWholeLeftSubtree) {
            leftObjects = [self objectsWithLargestLimitBelow:bound fromNode:node.left];
        }
    }
    // BUG-376: Guard against empty leftObjects array crash
    // leftObjects[0] would crash if leftObjects is non-nil but empty
    id<IntervalTreeObject> bestLeft = leftObjects.count > 0 ? leftObjects[0] : nil;
    if (thisLocation < bound && node.right) {
        // Try to eliminate the right subtree from consideration if this node or the left subtree
        // is superior to it.
        const BOOL thisNodeBeatsWholeRightSubtree =
            (nodeInterval && nodeInterval.limit < bound && nodeInterval.limit > rightValue.maxLimitAtSubtree);
        const BOOL leftSubtreeBeatsWholeRightSubtree = (leftValue && leftValue.maxLimitAtSubtree < bound &&
                                                        leftValue.maxLimitAtSubtree > rightValue.maxLimitAtSubtree);
        if (!thisNodeBeatsWholeRightSubtree && !leftSubtreeBeatsWholeRightSubtree) {
            rightObjects = [self objectsWithLargestLimitBelow:bound fromNode:node.right];
        }
    }
    // BUG-377: Guard against empty rightObjects array crash
    // rightObjects[0] would crash if rightObjects is non-nil but empty
    id<IntervalTreeObject> bestRight = rightObjects.count > 0 ? rightObjects[0] : nil;

    long long myLimit = LLONG_MIN;
    long long leftLimit = LLONG_MIN;
    long long rightLimit = LLONG_MIN;
    if (bestLeft) {
        leftLimit = bestLeft.entry.interval.limit;
    }
    if (bestRight) {
        rightLimit = bestRight.entry.interval.limit;
    }
    if (nodeInterval && bound > nodeInterval.limit) {
        myLimit = nodeInterval.limit;
    }
    long long bestLimit = MAX(MAX(leftLimit, rightLimit), myLimit);

    // Pre-allocate for typical result size
    NSMutableArray *result = [NSMutableArray arrayWithCapacity:4];
    if (myLimit == bestLimit) {
        [result addObjectsFromArray:myObjects];
    }
    if (leftLimit == bestLimit) {
        [result addObjectsFromArray:leftObjects];
    }
    if (rightLimit == bestLimit) {
        [result addObjectsFromArray:rightObjects];
    }
    return result.count ? result : nil;
}

- (NSArray *)objectsWithLargestLimitBefore:(long long)limit {
    return [self objectsWithLargestLimitBelow:limit fromNode:_tree.root];
}

- (NSArray *)objectsWithSmallestLimitAfter:(long long)limit {
    return [self objectsWithSmallestLimitAfter:limit fromNode:_tree.root];
}

- (NSEnumerator *)reverseEnumerator {
    IntervalTreeReverseEnumerator *enumerator = [[[IntervalTreeReverseEnumerator alloc] initWithTree:self] autorelease];
    enumerator.previousLocation = INT64_MAX;
    return enumerator;
}

- (NSEnumerator *)reverseEnumeratorAt:(long long)start {
    // BUG-f686: Replace assert with guard - negative start should return empty enumerator not crash
    if (start < 0) {
        DLog(@"ERROR: IntervalTree reverseEnumeratorAt: negative start %lld", start);
        return [[[IntervalTreeReverseEnumerator alloc] initWithTree:self] autorelease];
    }
    IntervalTreeReverseEnumerator *enumerator = [[[IntervalTreeReverseEnumerator alloc] initWithTree:self] autorelease];
    enumerator.previousLocation = start + 1;
    return enumerator;
}

- (NSEnumerator *)reverseLimitEnumeratorAt:(long long)start {
    // BUG-f687: Replace assert with guard - negative start should return empty enumerator not crash
    if (start < 0) {
        DLog(@"ERROR: IntervalTree reverseLimitEnumeratorAt: negative start %lld", start);
        IntervalTreeReverseLimitEnumerator *enumerator =
            [[[IntervalTreeReverseLimitEnumerator alloc] initWithTree:self] autorelease];
        enumerator.previousLimit = -1; // Will return no results
        return enumerator;
    }
    IntervalTreeReverseLimitEnumerator *enumerator =
        [[[IntervalTreeReverseLimitEnumerator alloc] initWithTree:self] autorelease];
    enumerator.previousLimit = start;
    return enumerator;
}

- (NSEnumerator *)forwardLimitEnumeratorAt:(long long)start {
    // BUG-f688: Replace assert with guard - negative start should return empty enumerator not crash
    if (start < 0) {
        DLog(@"ERROR: IntervalTree forwardLimitEnumeratorAt: negative start %lld", start);
        start = 0; // Use 0 as minimum valid start
    }
    IntervalTreeForwardLimitEnumerator *enumerator =
        [[[IntervalTreeForwardLimitEnumerator alloc] initWithTree:self] autorelease];
    enumerator.previousLimit = start;
    return enumerator;
}

- (NSEnumerator *)forwardLocationEnumeratorAt:(long long)start {
    // BUG-f689: Replace assert with guard - negative start should use 0 not crash
    if (start < 0) {
        DLog(@"ERROR: IntervalTree forwardLocationEnumeratorAt: negative start %lld", start);
        start = 0; // Use 0 as minimum valid start
    }
    IntervalTreeForwardLocationEnumerator *enumerator =
        [[[IntervalTreeForwardLocationEnumerator alloc] initWithTree:self] autorelease];
    enumerator.previousLocation = start - 1;
    return enumerator;
}

- (NSEnumerator *)reverseLimitEnumerator {
    return [[[IntervalTreeReverseLimitEnumerator alloc] initWithTree:self] autorelease];
}

- (NSEnumerator *)forwardLimitEnumerator {
    return [[[IntervalTreeForwardLimitEnumerator alloc] initWithTree:self] autorelease];
}

- (void)enumerateLimitsAfter:(long long)minimumLimit block:(void (^)(id<IntervalTreeObject> object, BOOL *stop))block {
    [self enumerateLimitsAfter:minimumLimit atNode:_tree.root block:block];
}

// returns whether the block directed us to stop enumerating.
- (BOOL)enumerateLimitsAfter:(long long)minimumLimit
                      atNode:(AATreeNode *)node
                       block:(void (^)(id<IntervalTreeObject> object, BOOL *stop))block {
    if (!node) {
        return NO;
    }
    IntervalTreeValue *value = node.data;
    if (value.maxLimit < minimumLimit) {
        return NO;
    }
    for (IntervalTreeEntry *entry in value.entries) {
        if (entry.interval.limit < minimumLimit) {
            continue;
        }
        BOOL stop = NO;
        block(entry.object, &stop);
        if (stop) {
            return YES;
        }
    }
    const long long leftMaxLimit = ((IntervalTreeValue *)node.left.data).maxLimitAtSubtree;
    if (leftMaxLimit >= minimumLimit) {
        if ([self enumerateLimitsAfter:minimumLimit atNode:node.left block:block]) {
            return YES;
        }
    }

    const long long rightMaxLimit = ((IntervalTreeValue *)node.right.data).maxLimitAtSubtree;
    if (rightMaxLimit >= minimumLimit) {
        if ([self enumerateLimitsAfter:minimumLimit atNode:node.right block:block]) {
            return YES;
        }
    }
    return NO;
}

- (long long)bruteForceMaxLimitAtSubtree:(AATreeNode *)node {
    IntervalTreeValue *value = node.data;
    long long result = LLONG_MIN;
    for (IntervalTreeEntry *entry in value.entries) {
        result = MAX(result, entry.interval.limit);
    }
    if (node.left) {
        result = MAX([self bruteForceMaxLimitAtSubtree:node.left], result);
    }
    if (node.right) {
        result = MAX([self bruteForceMaxLimitAtSubtree:node.right], result);
    }
    return result;
}

- (void)sanityCheckAtNode:(AATreeNode *)node {
    IntervalTreeValue *value = node.data;
    long long location = [(NSNumber *)node.key longLongValue];
    const long long limit = [self bruteForceMaxLimitAtSubtree:node];
    // BUG-f1389: Convert assert to ELog - maxLimitAtSubtree mismatch is a data corruption indicator, not crash-worthy
    if (value.maxLimitAtSubtree != limit) {
        ELog(@"IntervalTree sanity check failed: maxLimitAtSubtree %lld != limit %lld at node %p",
             value.maxLimitAtSubtree, limit, node);
    }
    IntervalTreeValue *leftValue = node.left.data;
    IntervalTreeValue *rightValue = node.right.data;
    if (leftValue) {
        // BUG-f1390: Convert assert to ELog - BST ordering violation is data corruption, log and continue
        if ([(NSNumber *)node.left.key longLongValue] >= location) {
            ELog(@"IntervalTree sanity check failed: left key %lld >= location %lld at node %p",
                 [(NSNumber *)node.left.key longLongValue], location, node);
        }
    }
    if (rightValue) {
        // BUG-f1391: Convert assert to ELog - BST ordering violation is data corruption, log and continue
        if ([(NSNumber *)node.right.key longLongValue] <= location) {
            ELog(@"IntervalTree sanity check failed: right key %lld <= location %lld at node %p",
                 [(NSNumber *)node.right.key longLongValue], location, node);
        }
    }
    for (IntervalTreeEntry *entry in value.entries) {
        // BUG-f1392: Convert assert to ELog - interval location mismatch is data corruption, log and continue
        if (entry.interval.location != location) {
            ELog(@"IntervalTree sanity check failed: entry interval location %lld != location %lld at node %p",
                 entry.interval.location, location, node);
        }
        // BUG-f1393: Convert assert to ELog - interval limit exceeds maxLimitAtSubtree is data corruption
        if (entry.interval.limit > value.maxLimitAtSubtree) {
            ELog(@"IntervalTree sanity check failed: entry limit %lld > maxLimitAtSubtree %lld at node %p",
                 entry.interval.limit, value.maxLimitAtSubtree, node);
        }
    }

    if (node.left) {
        [self sanityCheckAtNode:node.left];
        // BUG-f1394: Convert assert to ELog - node.left == node.right indicates tree corruption
        if (node.left == node.right) {
            ELog(@"IntervalTree sanity check failed: node.left == node.right at node %p (after left recursion)", node);
        }
    }
    if (node.right) {
        [self sanityCheckAtNode:node.right];
        // BUG-f1395: Convert assert to ELog - node.left == node.right indicates tree corruption
        if (node.left == node.right) {
            ELog(@"IntervalTree sanity check failed: node.left == node.right at node %p (after right recursion)", node);
        }
    }
}

- (void)sanityCheck {
#ifdef INTERVAL_TREE_SANITY_CHECK
    // One value per tree node for duplicate detection
    NSMutableSet *values = [NSMutableSet setWithCapacity:_tree.count];
    for (id value in _tree.allValues) {
        NSString *addr = [NSString stringWithFormat:@"%p", value];
        if ([values containsObject:addr]) {
            // BUG-447: Replace assert(NO) with ELog - duplicate detection should not crash
            // This is a sanity check that finds data corruption; log and continue checking
            ELog(@"IntervalTree sanity check failed: duplicate value found at address %@", addr);
        }
        [values addObject:addr];
    }

    if (_tree.root) {
        [self sanityCheckAtNode:_tree.root];
    }
#endif // INTERVAL_TREE_SANITY_CHECK
}

- (NSDictionary *)dictionaryValueWithOffset:(long long)offset {
    // One dictionary per object in the tree
    NSMutableArray *objectDicts = [NSMutableArray arrayWithCapacity:_tree.count];
    for (id<IntervalTreeObject> object in self.allObjects) {
        Interval *interval = [Interval intervalWithLocation:object.entry.interval.location + offset
                                                     length:object.entry.interval.length];
        [objectDicts addObject:@{
            kIntervalTreeIntervalKey : interval.dictionaryValue,
            kIntervalTreeObjectKey : object.dictionaryValue,
            kIntervalTreeClassNameKey : NSStringFromClass(object.class)
        }];
    }
    return @{kIntervalTreeEntriesKey : objectDicts};
}

@end

@implementation iTermIntervalTreeSanitizingAdapter {
    __weak IntervalTree *_source;
}

- (instancetype)initWithSource:(IntervalTree *)source {
    self = [super init];
    if (self) {
        _source = source;
    }
    return self;
}

- (NSString *)debugString {
    return [_source debugString];
}

- (NSArray<id<IntervalTreeImmutableObject>> *)objectsInInterval:(Interval *)interval {
    return [[_source objectsInInterval:interval] mapWithBlock:^id(id<IntervalTreeImmutableObject> anObject) {
        return [anObject doppelganger];
    }];
}

- (NSArray<id<IntervalTreeImmutableObject>> *)allObjects {
    return [[_source allObjects] mapWithBlock:^id(id<IntervalTreeImmutableObject> anObject) {
        return [anObject doppelganger];
    }];
}

- (BOOL)containsObject:(id<IntervalTreeImmutableObject> _Nullable)object {
    return [_source containsObject:[object progenitor]];
}

- (NSArray<id<IntervalTreeImmutableObject>> *_Nullable)objectsWithLargestLimit {
    return [[_source objectsWithLargestLimit] mapWithBlock:^id(id<IntervalTreeImmutableObject> anObject) {
        return [anObject doppelganger];
    }];
}

- (NSArray<id<IntervalTreeImmutableObject>> *_Nullable)objectsWithSmallestLimit {
    return [[_source objectsWithSmallestLimit] mapWithBlock:^id(id<IntervalTreeImmutableObject> anObject) {
        return [anObject doppelganger];
    }];
}

- (NSArray<id<IntervalTreeImmutableObject>> *_Nullable)objectsWithLargestLocation {
    return [[_source objectsWithLargestLocation] mapWithBlock:^id(id<IntervalTreeImmutableObject> anObject) {
        return [anObject doppelganger];
    }];
}

- (NSArray<id<IntervalTreeImmutableObject>> *_Nullable)objectsWithLargestLocationBefore:(long long)location {
    return [[_source objectsWithLargestLocationBefore:location]
        mapWithBlock:^id(id<IntervalTreeImmutableObject> anObject) {
            return [anObject doppelganger];
        }];
}

- (NSArray<id<IntervalTreeImmutableObject>> *_Nullable)objectsWithLargestLimitBefore:(long long)limit {
    return [[_source objectsWithLargestLimitBefore:limit] mapWithBlock:^id(id anObject) {
        return [anObject doppelganger];
    }];
}

- (NSArray<id<IntervalTreeImmutableObject>> *_Nullable)objectsWithSmallestLimitAfter:(long long)limit {
    return [[_source objectsWithSmallestLimitAfter:limit] mapWithBlock:^id(id<IntervalTreeImmutableObject> anObject) {
        return [anObject doppelganger];
    }];
}

- (NSEnumerator<IntervalTreeImmutableObject> *)reverseEnumerator {
    return [IntervalTreeSanitizingEnumerator<IntervalTreeImmutableObject> with:[_source reverseEnumerator]];
}

- (NSEnumerator<IntervalTreeImmutableObject> *)reverseEnumeratorAt:(long long)start {
    return [IntervalTreeSanitizingEnumerator<IntervalTreeImmutableObject> with:[_source reverseEnumeratorAt:start]];
}

- (NSEnumerator<IntervalTreeImmutableObject> *)reverseLimitEnumeratorAt:(long long)start {
    return
        [IntervalTreeSanitizingEnumerator<IntervalTreeImmutableObject> with:[_source reverseLimitEnumeratorAt:start]];
}


- (NSEnumerator<IntervalTreeImmutableObject> *)forwardLimitEnumeratorAt:(long long)start {
    return
        [IntervalTreeSanitizingEnumerator<IntervalTreeImmutableObject> with:[_source forwardLimitEnumeratorAt:start]];
}

- (NSEnumerator<IntervalTreeImmutableObject> *)reverseLimitEnumerator {
    return [IntervalTreeSanitizingEnumerator<IntervalTreeImmutableObject> with:[_source reverseLimitEnumerator]];
}

- (NSEnumerator<IntervalTreeImmutableObject> *)forwardLimitEnumerator {
    return [IntervalTreeSanitizingEnumerator<IntervalTreeImmutableObject> with:[_source forwardLimitEnumerator]];
}

- (NSDictionary *)dictionaryValueWithOffset:(long long)offset {
    return [_source dictionaryValueWithOffset:offset];
}

- (void)enumerateLimitsAfter:(long long)minimumLimit
                       block:(void (^)(id<IntervalTreeObject> _Nonnull, BOOL *_Nonnull))block {
    [_source enumerateLimitsAfter:minimumLimit
                            block:^(id<IntervalTreeObject> _Nonnull object, BOOL *_Nonnull stop) {
                                block(object.doppelganger, stop);
                            }];
}

- (nonnull NSEnumerator<IntervalTreeImmutableObject> *)forwardLocationEnumeratorAt:(long long)start {
    return [IntervalTreeSanitizingEnumerator<IntervalTreeImmutableObject>
        with:[_source forwardLocationEnumeratorAt:start]];
}


- (NSArray<id<IntervalTreeImmutableObject>> *_Nullable)objectsWithSmallestLocation {
    return [[_source objectsWithSmallestLocation]
        mapWithBlock:^id _Nullable(id<IntervalTreeImmutableObject> _Nonnull anObject) {
            return [anObject doppelganger];
        }];
}


- (NSArray<id<IntervalTreeImmutableObject>> *_Nullable)objectsWithSmallestLocationAfter:(long long)location {
    return [[_source objectsWithSmallestLocationAfter:location]
        mapWithBlock:^id _Nullable(id<IntervalTreeImmutableObject> _Nonnull anObject) {
            return [anObject doppelganger];
        }];
}


@end
