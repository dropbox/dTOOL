//
//  iTermParsedExpression.m
//  DashTerm2SharedARC
//
//  Created by George Nachman on 3/1/19.
//

#import "iTermParsedExpression.h"

#import "DebugLogging.h"
#import "iTermScriptFunctionCall.h"
#import "iTermVariableReference.h"
#import "NSArray+iTerm.h"
#import "NSObject+iTerm.h"
#import "NSStringITerm.h"

@implementation iTermExpressionParserArrayDereferencePlaceholder

@synthesize path = _path;

- (iTermParsedExpressionType)expressionType {
    return iTermParsedExpressionTypeArrayLookup;
}

- (instancetype)initWithPath:(NSString *)path index:(NSInteger)index {
    self = [super init];
    if (self) {
        _path = [path copy];
        _index = index;
    }
    return self;
}

@end

@implementation iTermExpressionParserVariableReferencePlaceholder

@synthesize path = _path;

- (iTermParsedExpressionType)expressionType {
    return iTermParsedExpressionTypeVariableReference;
}

- (instancetype)initWithPath:(NSString *)path {
    self = [super init];
    if (self) {
        _path = [path copy];
    }
    return self;
}

@end


@implementation iTermParsedExpression

- (NSString *)description {
    NSString *value = nil;
    switch (self.expressionType) {
        case iTermParsedExpressionTypeInterpolatedString:
            value = [[self.interpolatedStringParts mapWithBlock:^id(id anObject) {
                return [anObject description];
            }] componentsJoinedByString:@""];
            break;
        case iTermParsedExpressionTypeFunctionCall:
            value = self.functionCall.description;
            break;
        case iTermParsedExpressionTypeFunctionCalls:
            value = [[self.functionCalls mapWithBlock:^id _Nullable(iTermScriptFunctionCall * _Nonnull anObject) {
                return [anObject description];
            }] componentsJoinedByString:@"; "];
            break;
        case iTermParsedExpressionTypeNil:
            value = @"nil";
            break;
        case iTermParsedExpressionTypeError:
            value = self.error.description;
            break;
        case iTermParsedExpressionTypeNumber:
        case iTermParsedExpressionTypeBoolean:
            value = [self.number stringValue];
            break;
        case iTermParsedExpressionTypeReference:
            value = self.reference.path;
            break;
        case iTermParsedExpressionTypeString:
            value = self.string;
            break;
        case iTermParsedExpressionTypeArrayOfExpressions:
        case iTermParsedExpressionTypeArrayOfValues:
            value = [[(NSArray *)_object mapWithBlock:^id(id anObject) {
                return [anObject description];
            }] componentsJoinedByString:@" "];
            value = [NSString stringWithFormat:@"[ %@ ]", value];
            break;
        case iTermParsedExpressionTypeArrayLookup: {
            iTermExpressionParserArrayDereferencePlaceholder *placeholder = self.placeholder;
            return [NSString stringWithFormat:@"%@[%@]", placeholder.path, @(placeholder.index)];
        }
        case iTermParsedExpressionTypeVariableReference: {
            iTermExpressionParserVariableReferencePlaceholder *placeholder = self.placeholder;
            return [NSString stringWithFormat:@"%@", placeholder.path];
        }
    }
    if (self.optional) {
        value = [value stringByAppendingString:@"?"];
    }
    return [NSString stringWithFormat:@"<Expr %@>", value];
}

- (BOOL)isEqual:(id)object {
    iTermParsedExpression *other = [iTermParsedExpression castFrom:object];
    if (!other) {
        return NO;
    }
    return ([NSObject object:self.object isEqualToObject:other.object] &&
            self.expressionType == other.expressionType &&
            self.optional == other.optional);
}

+ (instancetype)parsedString:(NSString *)string {
    return [[self alloc] initWithString:string];
}

- (instancetype)initWithString:(NSString *)string {
    self = [super init];
    if (self) {
        _expressionType = iTermParsedExpressionTypeString;
        _optional = NO;
        _object = string;
    }
    return self;
}

- (instancetype)initWithFunctionCall:(iTermScriptFunctionCall *)functionCall {
    self = [super init];
    if (self) {
        _expressionType = iTermParsedExpressionTypeFunctionCall;
        _object = functionCall;
    }
    return self;
}

- (instancetype)initWithFunctionCalls:(NSArray<iTermScriptFunctionCall *> *)functionCalls {
    self = [super init];
    if (self) {
        _expressionType = iTermParsedExpressionTypeFunctionCalls;
        _object = functionCalls;
    }
    return self;
}

- (instancetype)initWithErrorCode:(int)code reason:(NSString *)localizedDescription {
    self = [super init];
    if (self) {
        _expressionType = iTermParsedExpressionTypeError;
        _object = [NSError errorWithDomain:@"com.dashterm.dashterm2.parser"
                                      code:code
                                  userInfo:@{ NSLocalizedDescriptionKey: localizedDescription ?: @"Unknown error" }];
    }
    return self;
}

// Object may be NSString, NSNumber, or NSArray. If it is not, an error will be created with the
// given reason.
- (instancetype)initWithObject:(id)object errorReason:(NSString *)errorReason {
    if ([object isKindOfClass:[NSString class]]) {
        return [self initWithString:object];
    }
    if ([object isKindOfClass:[NSNumber class]]) {
        return [self initWithNumber:object];
    }
    if ([object isKindOfClass:[NSArray class]]) {
        return [self initWithArrayOfValues:object];
    }
    return [self initWithErrorCode:7 reason:errorReason];
}

- (instancetype)initWithOptionalObject:(id)object {
    if (object) {
        self = [self initWithObject:object errorReason:[NSString stringWithFormat:@"Invalid type: %@", [object class]]];
    } else {
        self = [super init];
    }
    if (self) {
        _optional = YES;
    }
    return self;
}

- (instancetype)initWithArrayOfValues:(NSArray *)array {
    self = [super init];
    if (self) {
        _expressionType = iTermParsedExpressionTypeArrayOfValues;
        _object = array;
    }
    return self;
}

- (instancetype)initWithArrayOfExpressions:(NSArray<iTermParsedExpression *> *)array {
    self = [super init];
    if (self) {
        _expressionType = iTermParsedExpressionTypeArrayOfExpressions;
        _object = array;
    }
    return self;
}

- (instancetype)initWithReference:(iTermVariableReference *)ref {
    self = [super init];
    if (self) {
        _expressionType = iTermParsedExpressionTypeReference;
        _object = ref;
    }
    return self;
}

- (instancetype)initWithNumber:(NSNumber *)number {
    self = [super init];
    if (self) {
        _expressionType = iTermParsedExpressionTypeNumber;
        _object = number;
    }
    return self;
}

- (instancetype)initWithBoolean:(BOOL)value {
    self = [super init];
    if (self) {
        _expressionType = iTermParsedExpressionTypeBoolean;
        _object = @(value);
    }
    return self;
}

- (instancetype)initWithError:(NSError *)error {
    self = [super init];
    if (self) {
        _expressionType = iTermParsedExpressionTypeError;
        _object = error;
    }
    return self;
}

- (instancetype)initWithInterpolatedStringParts:(NSArray *)parts {
    self = [super init];
    if (self) {
        _expressionType = iTermParsedExpressionTypeInterpolatedString;
        _object = parts;
    }
    return self;
}

- (instancetype)initWithPlaceholder:(id<iTermExpressionParserPlaceholder>)placeholder
                           optional:(BOOL)optional {
    self = [super init];
    if (self) {
        _expressionType = placeholder.expressionType;
        _object = placeholder;
        _optional = optional;
    }
    return self;
}

// BUG-f1075: Replace assert() with guards to prevent crashes on type mismatch
- (NSArray *)arrayOfValues {
    if (![_object isKindOfClass:[NSArray class]]) {
        DLog(@"BUG-f1075: arrayOfValues called but _object is %@, not NSArray", [_object class]);
        return @[];  // Return empty array as safe fallback
    }
    return _object;
}

// BUG-f1076: Replace assert() with guards to prevent crashes on type mismatch
- (NSArray *)arrayOfExpressions {
    if (![_object isKindOfClass:[NSArray class]]) {
        DLog(@"BUG-f1076: arrayOfExpressions called but _object is %@, not NSArray", [_object class]);
        return @[];  // Return empty array as safe fallback
    }
    return _object;
}

// BUG-f1077: Replace assert() with guards to prevent crashes on type mismatch
- (NSString *)string {
    if (![_object isKindOfClass:[NSString class]]) {
        DLog(@"BUG-f1077: string called but _object is %@, not NSString", [_object class]);
        return @"";  // Return empty string as safe fallback
    }
    return _object;
}

// BUG-f1078: Replace assert() with guards to prevent crashes on type mismatch
- (iTermVariableReference *)reference {
    if (![_object isKindOfClass:[iTermVariableReference class]]) {
        DLog(@"BUG-f1078: reference called but _object is %@, not iTermVariableReference", [_object class]);
        return nil;  // Return nil as safe fallback
    }
    return (iTermVariableReference *)_object;
}

// BUG-f1079: Replace assert() with guards to prevent crashes on type mismatch
- (NSNumber *)number {
    if (![_object isKindOfClass:[NSNumber class]]) {
        DLog(@"BUG-f1079: number called but _object is %@, not NSNumber", [_object class]);
        return @0;  // Return zero as safe fallback
    }
    return _object;
}

// BUG-f1080: Replace assert() with guards to prevent crashes on type mismatch
- (NSError *)error {
    if (![_object isKindOfClass:[NSError class]]) {
        DLog(@"BUG-f1080: error called but _object is %@, not NSError", [_object class]);
        return [NSError errorWithDomain:@"com.dashterm.dashterm2" code:-1 userInfo:@{NSLocalizedDescriptionKey: @"Unknown error"}];
    }
    return _object;
}

- (iTermScriptFunctionCall *)functionCall {
    if (![_object isKindOfClass:[iTermScriptFunctionCall class]]) {
        return nil;
    }
    return _object;
}

// BUG-f1081: Replace assert() with guards to prevent crashes on type mismatch
- (NSArray<iTermScriptFunctionCall *> *)functionCalls {
    if (![_object isKindOfClass:[NSArray class]]) {
        DLog(@"BUG-f1081: functionCalls called but _object is %@, not NSArray", [_object class]);
        return @[];  // Return empty array as safe fallback
    }
    // Validate each child is a function call (log invalid entries but don't crash)
    for (id child in _object) {
        if (![child isKindOfClass:[iTermScriptFunctionCall class]]) {
            DLog(@"BUG-f1081: functionCalls contains non-function-call object: %@", [child class]);
        }
    }
    return _object;
}

// BUG-f1082: Replace assert() with guards to prevent crashes on type mismatch
- (NSArray *)interpolatedStringParts {
    if (![_object isKindOfClass:[NSArray class]]) {
        DLog(@"BUG-f1082: interpolatedStringParts called but _object is %@, not NSArray", [_object class]);
        return @[];  // Return empty array as safe fallback
    }
    return _object;
}

// BUG-f1083: Replace assert() with guards to prevent crashes on protocol mismatch
- (id<iTermExpressionParserPlaceholder>)placeholder {
    if (![_object conformsToProtocol:@protocol(iTermExpressionParserPlaceholder)]) {
        DLog(@"BUG-f1083: placeholder called but _object %@ does not conform to iTermExpressionParserPlaceholder", [_object class]);
        return nil;  // Return nil as safe fallback
    }
    return _object;
}

- (BOOL)containsAnyFunctionCall {
    switch (self.expressionType) {
        case iTermParsedExpressionTypeFunctionCall:
        case iTermParsedExpressionTypeFunctionCalls:
            return YES;
        case iTermParsedExpressionTypeNil:
        case iTermParsedExpressionTypeError:
        case iTermParsedExpressionTypeNumber:
        case iTermParsedExpressionTypeReference:
        case iTermParsedExpressionTypeBoolean:
        case iTermParsedExpressionTypeString:
        case iTermParsedExpressionTypeArrayOfValues:
        case iTermParsedExpressionTypeVariableReference:
        case iTermParsedExpressionTypeArrayLookup:
            return NO;
        case iTermParsedExpressionTypeArrayOfExpressions:
            return [self.arrayOfExpressions anyWithBlock:^BOOL(iTermParsedExpression *expression) {
                return [expression containsAnyFunctionCall];
            }];
        case iTermParsedExpressionTypeInterpolatedString:
            return [self.interpolatedStringParts anyWithBlock:^BOOL(iTermParsedExpression *expression) {
                return [expression containsAnyFunctionCall];
            }];
    }
    // BUG-421: Return YES (conservative) instead of crashing for unexpected expression type
    // This handles any new expression types that may be added in the future
    return YES;
}

@end
