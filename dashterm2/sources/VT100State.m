#import "VT100State.h"
#import "VT100StateTransition.h"
#import <limits.h>

// Optimization: Cache NSNumber objects for all unsigned char values (0-255) to avoid boxing.
// Character transitions are looked up for every single character during terminal parsing,
// making this one of the hottest paths in the codebase. Pre-caching eliminates @(character)
// allocations that would otherwise occur millions of times during normal terminal use.
static NSNumber *sCharacterKeyCache[256];

__attribute__((constructor))
static void VT100StateInitializeCharacterKeyCache(void) {
    for (int i = 0; i < 256; i++) {
        sCharacterKeyCache[i] = @(i);
    }
}

@interface VT100State()
@property(nonatomic, copy) NSString *name;
@end

@implementation VT100State {
    NSMutableDictionary *_transitions;
}

+ (instancetype)stateWithName:(NSString *)name identifier:(NSObject *)identifier {
    VT100State *state = [[self alloc] initWithName:name];
    state.identifier = identifier;
    return state;
}

- (instancetype)initWithName:(NSString *)name {
    self = [super init];
    if (self) {
        _name = [name copy];
        _transitions = [[NSMutableDictionary alloc] initWithCapacity:128];  // VT100 state transitions
    }
    return self;
}

- (NSString *)description {
    return [NSString stringWithFormat:@"<%@: %p %@>", [self class], self, _name];
}

- (void)addStateTransitionForCharacter:(unsigned char)character
                                    to:(VT100State *)state
                            withAction:(VT100StateAction)action {
    _transitions[sCharacterKeyCache[character]] = [VT100StateTransition transitionToState:state withAction:action];
}

- (void)addStateTransitionForCharacterRange:(NSRange)characterRange
                                         to:(VT100State *)state
                                 withAction:(VT100StateAction)action {
    const NSUInteger start = characterRange.location;
    if (start > UCHAR_MAX) {
        return;
    }

    const NSUInteger end = MIN(NSMaxRange(characterRange), (NSUInteger)UCHAR_MAX + 1);
    for (NSUInteger character = start; character < end; character++) {
        [self addStateTransitionForCharacter:(unsigned char)character
                                          to:state
                                  withAction:action];
    }
}

- (VT100StateTransition *)stateTransitionForCharacter:(unsigned char)character {
    return _transitions[sCharacterKeyCache[character]];
}

@end
