//
//  iTermExpressionEvaluator.m
//  DashTerm2
//
//  Created by George Nachman on 2/28/19.
//

#import "iTermExpressionEvaluator.h"

#import "DebugLogging.h"
#import "iTermAPIHelper.h"
#import "iTermExpressionParser.h"
#import "iTermGCD.h"
#import "iTermScriptFunctionCall+Private.h"
#import "iTermScriptHistory.h"
#import "iTermVariableScope.h"
#import "NSArray+iTerm.h"
#import "NSDictionary+iTerm.h"
#import "NSJSONSerialization+iTerm.h"
#import "NSObject+iTerm.h"
#import "NSStringITerm.h"

@interface iTermExpressionEvaluator (Private)
- (void)didCompleteWithResult:(id)result
                        error:(NSError *)error
                      missing:(NSSet<NSString *> *)missing
                   completion:(void (^)(iTermExpressionEvaluator *))completion;
@end

@implementation iTermExpressionEvaluator {
    BOOL _hasBeenEvaluated;
    BOOL _isBeingEvaluated;
    id _value;
    iTermParsedExpression *_parsedExpression;
    iTermVariableScope *_scope;
    NSMutableArray<iTermExpressionEvaluator *> *_innerEvaluators;
    NSString *_invocation;
}

+ (void)evaluateExpression:(NSString *)expression
                   timeout:(NSTimeInterval)timeout
        sideEffectsAllowed:(BOOL)sideEffectsAllowed
                     scope:(iTermVariableScope *)scope
                completion:(void (^)(id, NSError *, NSSet<NSString *> *))completion {
    iTermExpressionEvaluator *evaluator = [[iTermExpressionEvaluator alloc] initWithExpressionString:expression scope:scope];
    [evaluator evaluateWithTimeout:timeout
                sideEffectsAllowed:sideEffectsAllowed
                        completion:^(iTermExpressionEvaluator *evaluator) {
        dispatch_async(dispatch_get_main_queue(), ^{
            completion(evaluator.value, evaluator.error, evaluator.missingValues);
        });
    }];
}
- (instancetype)initWithParsedExpression:(iTermParsedExpression *)parsedExpression
                              invocation:(NSString *)invocation
                                   scope:(iTermVariableScope *)scope {
    self = [super init];
    if (self) {
        _invocation = [invocation copy];
        _parsedExpression = parsedExpression;
        _scope = scope;
        _innerEvaluators = [NSMutableArray arrayWithCapacity:4]; // Typical nested evaluator count
    }
    return self;
}

- (instancetype)initWithExpressionString:(NSString *)expressionString scope:(iTermVariableScope *)scope {
    iTermParsedExpression *parsedExpression = [[iTermExpressionParser expressionParser] parse:expressionString
                                                                                        scope:scope];
    return [self initWithParsedExpression:parsedExpression invocation:expressionString scope:scope];
}

- (instancetype)initWithStrictInterpolatedString:(NSString *)interpolatedString scope:(iTermVariableScope *)scope {
    iTermParsedExpression *parsedExpression =
        [iTermExpressionParser parsedExpressionWithInterpolatedString:interpolatedString
                                                     escapingFunction:nil
                                                                scope:scope
                                                               strict:YES];
    return [self initWithParsedExpression:parsedExpression invocation:interpolatedString scope:scope];
}

- (instancetype)initWithInterpolatedString:(NSString *)interpolatedString scope:(iTermVariableScope *)scope {
    iTermParsedExpression *parsedExpression =
        [iTermExpressionParser parsedExpressionWithInterpolatedString:interpolatedString scope:scope];
    return [self initWithParsedExpression:parsedExpression invocation:interpolatedString scope:scope];
}

- (id)value {
    if (_hasBeenEvaluated) {
        return _value;
    }
    // BUG-f950: Guard against recursive evaluation in debug builds instead of crashing
    if (_isBeingEvaluated) {
        ELog(@"iTermExpressionEvaluator: value called while already being evaluated");
        return nil;
    }
    [self evaluateWithTimeout:0
           sideEffectsAllowed:NO
                   completion:^(iTermExpressionEvaluator *_Nonnull evaluator){
                   }];
    return _value;
}

static NSMutableArray *iTermExpressionEvaluatorGlobalStore(void) {
    [iTermGCD assertMainQueueSafe:@"Expression evaluation must be main-queue safe"];
    static NSMutableArray *array;
    static dispatch_once_t onceToken;
    dispatch_once(&onceToken, ^{
        array = [NSMutableArray arrayWithCapacity:16]; // Typical concurrent evaluators
    });
    return array;
}

- (void)evaluateWithTimeout:(NSTimeInterval)timeout
         sideEffectsAllowed:(BOOL)sideEffectsAllowed
                 completion:(void (^)(iTermExpressionEvaluator *))completion {
    _hasBeenEvaluated = YES;
    // BUG-f951: Guard against recursive evaluation instead of crashing
    if (_isBeingEvaluated) {
        ELog(@"iTermExpressionEvaluator: evaluateWithTimeout called while already being evaluated");
        return;
    }
    _isBeingEvaluated = YES;

    [iTermExpressionEvaluatorGlobalStore() addObject:self];

    BOOL debug = _debug;
    [self reallyEvaluateWithTimeout:timeout
                 sideEffectsAllowed:sideEffectsAllowed
                              debug:debug
                         completion:completion];
}

- (void)reallyEvaluateWithTimeout:(NSTimeInterval)timeout
               sideEffectsAllowed:(BOOL)sideEffectsAllowed
                            debug:(BOOL)debug
                       completion:(void (^)(iTermExpressionEvaluator *))completion {
    NSString *descr = [NSString stringWithFormat:@"%@: %@", self, _invocation];
    if (debug) {
        NSLog(@"Evaluate %@", _parsedExpression);
    }
    __weak __typeof(self) weakSelf = self;
    [self evaluateParsedExpression:_parsedExpression
                        invocation:_invocation
                       withTimeout:timeout
                sideEffectsAllowed:sideEffectsAllowed
                        completion:^(id result, NSError *error, NSSet<NSString *> *missing) {
                            DLog(@"%@ result=%@, error=%@, missing=%@", descr, result, error, missing);
                            if (debug) {
                                NSLog(@"%@ result=%@, error=%@, missing=%@", descr, result, error, missing);
                            }
                            if (self.retryUntil.timeIntervalSinceNow > 0 &&
                                [error.domain isEqual:iTermAPIHelperErrorDomain] &&
                                error.code == iTermAPIHelperErrorCodeUnregisteredFunction) {
                                DLog(@"Schedule retry of %@", descr);
                                dispatch_after(dispatch_time(DISPATCH_TIME_NOW, (int64_t)(1 * NSEC_PER_SEC)),
                                               dispatch_get_main_queue(), ^{
                                                   DLog(@"Retrying");
                                                   [weakSelf reallyEvaluateWithTimeout:timeout
                                                                    sideEffectsAllowed:sideEffectsAllowed
                                                                                 debug:debug
                                                                            completion:completion];
                                               });
                                return;
                            }

                            if (debug) {
                                NSLog(@"Return result=%@ error=%@", result, error);
                            }
                            DLog(@"Return result=%@ error=%@", result, error);
                            [weakSelf didCompleteWithResult:result error:error missing:missing completion:completion];
                        }];
}

- (void)didCompleteWithResult:(id)result
                        error:(NSError *)error
                      missing:(NSSet<NSString *> *)missing
                   completion:(void (^)(iTermExpressionEvaluator *))completion {
    if (error) {
        _value = nil;
    } else {
        _value = result;
    }
    _error = error;
    _missingValues = missing;
    _isBeingEvaluated = NO;
    completion(self);
    [iTermExpressionEvaluatorGlobalStore() removeObject:self];
}

- (void)evaluateSwiftyString:(NSString *)string
                 withTimeout:(NSTimeInterval)timeout
          sideEffectsAllowed:(BOOL)sideEffectsAllowed
                  completion:(void (^)(id, NSError *, NSSet<NSString *> *))completion {
    NSMutableArray *parts = [NSMutableArray arrayWithCapacity:8]; // Typical swifty string parts
    __block NSError *firstError = nil;
    dispatch_group_t group = dispatch_group_create();
    NSMutableSet<NSString *> *missingFunctionSignatures = [NSMutableSet setWithCapacity:2];
    [string enumerateSwiftySubstrings:^(NSUInteger index, NSString *substring, BOOL isLiteral, BOOL *stop) {
        if (isLiteral) {
            [parts addObject:[substring it_stringByExpandingBackslashEscapedCharacters]];
        } else {
            dispatch_group_enter(group);
            [parts addObject:@""];

            iTermParsedExpression *parsedExpression = [[iTermExpressionParser expressionParser] parse:substring
                                                                                                scope:self->_scope];
            iTermExpressionEvaluator *innerEvaluator =
                [[iTermExpressionEvaluator alloc] initWithParsedExpression:parsedExpression
                                                                invocation:string
                                                                     scope:self->_scope];
            [self->_innerEvaluators addObject:innerEvaluator];
            [innerEvaluator evaluateWithTimeout:timeout
                              sideEffectsAllowed:sideEffectsAllowed
                                     completion:^(iTermExpressionEvaluator *evaluator) {
                                         [missingFunctionSignatures unionSet:evaluator.missingValues];
                                         if (evaluator.error) {
                                             firstError = evaluator.error;
                                         } else {
                                             parts[index] = [self stringFromJSONObject:evaluator.value];
                                         }
                                         dispatch_group_leave(group);
                                     }];
        }
    }];
    if (timeout == 0) {
        completion([parts componentsJoinedByString:@""], firstError, missingFunctionSignatures);
    } else {
        dispatch_group_notify(group, dispatch_get_main_queue(), ^{
            completion([parts componentsJoinedByString:@""], firstError, missingFunctionSignatures);
        });
    }
}

- (void)evaluateParsedExpression:(iTermParsedExpression *)parsedExpression
                      invocation:(NSString *)invocation
                     withTimeout:(NSTimeInterval)timeout
              sideEffectsAllowed:(BOOL)sideEffectsAllowed
                      completion:(void (^)(id, NSError *, NSSet<NSString *> *))completion {
    switch (parsedExpression.expressionType) {
        case iTermParsedExpressionTypeFunctionCall: {
            // BUG-f952: Guard against nil functionCall instead of crashing
            if (!parsedExpression.functionCall) {
                ELog(@"iTermExpressionEvaluator: FunctionCall expression type but functionCall is nil");
                completion(nil,
                           [NSError errorWithDomain:@"com.dashterm.dashterm2.expression-evaluator"
                                               code:3
                                           userInfo:@{NSLocalizedDescriptionKey : @"Missing function call"}],
                           nil);
                return;
            }
            [parsedExpression.functionCall performFunctionCallFromInvocation:invocation
                                                                    receiver:nil
                                                                       scope:_scope
                                                                     timeout:timeout
                                                          sideEffectsAllowed:sideEffectsAllowed
                                                                  completion:completion];
            return;
        }

        case iTermParsedExpressionTypeFunctionCalls: {
            // BUG-f953: Guard against nil functionCalls instead of crashing
            if (!parsedExpression.functionCalls) {
                ELog(@"iTermExpressionEvaluator: FunctionCalls expression type but functionCalls is nil");
                completion(nil,
                           [NSError errorWithDomain:@"com.dashterm.dashterm2.expression-evaluator"
                                               code:4
                                           userInfo:@{NSLocalizedDescriptionKey : @"Missing function calls"}],
                           nil);
                return;
            }
            [iTermScriptFunctionCall executeFunctionCalls:parsedExpression.functionCalls
                                               invocation:invocation
                                                 receiver:nil
                                                  timeout:timeout
                                       sideEffectsAllowed:sideEffectsAllowed
                                                    scope:_scope
                                               completion:completion];
            return;
        }

        case iTermParsedExpressionTypeInterpolatedString: {
            [self evaluateInterpolatedStringParts:parsedExpression.interpolatedStringParts
                                       invocation:invocation
                                      withTimeout:timeout
                               sideEffectsAllowed:sideEffectsAllowed
                                       completion:completion];
            return;
        }

        case iTermParsedExpressionTypeArrayOfExpressions: {
            [self evaluateArray:parsedExpression.arrayOfExpressions
                     invocation:invocation
                    withTimeout:timeout
             sideEffectsAllowed:sideEffectsAllowed
                     completion:completion];
            return;
        }
        case iTermParsedExpressionTypeArrayOfValues:
        case iTermParsedExpressionTypeString:
        case iTermParsedExpressionTypeNumber:
        case iTermParsedExpressionTypeBoolean:
        case iTermParsedExpressionTypeReference:
            completion(parsedExpression.object, nil, nil);
            return;

        case iTermParsedExpressionTypeError:
            completion(nil, parsedExpression.error, nil);
            return;

        case iTermParsedExpressionTypeNil:
            completion(nil, nil, nil);
            return;

        case iTermParsedExpressionTypeArrayLookup:
        case iTermParsedExpressionTypeVariableReference:
            // BUG-13009: Replace assert(NO) with error completion
            // These types should be evaluated elsewhere but handle gracefully
            DLog(@"Unexpected expression type %d in evaluateParsedExpression", (int)parsedExpression.expressionType);
            break; // Fall through to error handling below
    }

    NSString *reason =
        [NSString stringWithFormat:@"Invalid parsed expression type %@", @(parsedExpression.expressionType)];
    NSError *error = [NSError errorWithDomain:@"com.dashterm.dashterm2.expression-evaluator"
                                         code:2
                                     userInfo:@{NSLocalizedDescriptionKey : reason}];
    completion(nil, error, nil);
}

- (void)evaluateInterpolatedStringParts:(NSArray<iTermParsedExpression *> *)interpolatedStringParts
                             invocation:(NSString *)invocation
                            withTimeout:(NSTimeInterval)timeout
                     sideEffectsAllowed:(BOOL)sideEffectsAllowed
                             completion:(void (^)(id, NSError *, NSSet<NSString *> *))completion {
    BOOL debug = _debug;
    if (_debug) {
        NSLog(@"Evaluate parts: %@", interpolatedStringParts);
    }
    dispatch_group_t group = NULL;
    __block NSError *firstError = nil;
    NSMutableArray *parts = [NSMutableArray arrayWithCapacity:interpolatedStringParts.count];
    NSMutableSet<NSString *> *missingFunctionSignatures = [NSMutableSet setWithCapacity:2];
    if (timeout > 0) {
        group = dispatch_group_create();
    }
    [interpolatedStringParts enumerateObjectsUsingBlock:^(iTermParsedExpression *_Nonnull parsedExpression,
                                                          NSUInteger idx, BOOL *_Nonnull stop) {
        if (parsedExpression.expressionType == iTermParsedExpressionTypeString && parsedExpression.string) {
            // Shortcut. String literals get appended without messing with dispatch groups or inner
            // evaluators. They are also not subject to escaping, since they were under the control
            // of the caller before getting here.
            [parts addObject:parsedExpression.string];
            return;
        }

        [parts addObject:@""];
        iTermExpressionEvaluator *innerEvaluator =
            [[iTermExpressionEvaluator alloc] initWithParsedExpression:parsedExpression
                                                            invocation:invocation
                                                                 scope:self->_scope];
        [self->_innerEvaluators addObject:innerEvaluator];
        if (group) {
            if (debug) {
                NSLog(@"Enter group %@", group);
            }
            dispatch_group_enter(group);
        }
        [innerEvaluator evaluateWithTimeout:timeout
                         sideEffectsAllowed:sideEffectsAllowed
                                 completion:^(iTermExpressionEvaluator *evaluator) {
                                     [missingFunctionSignatures unionSet:evaluator.missingValues];
                                     if (evaluator.error) {
                                         firstError = evaluator.error;
                                         [self logError:evaluator.error invocation:invocation];
                                     } else {
                                         NSString *decodedString = [self stringFromJSONObject:evaluator.value];
                                         if (self.escapingFunction) {
                                             decodedString = self.escapingFunction(decodedString);
                                         }
                                         parts[idx] = decodedString;
                                     }
                                     if (group) {
                                         if (debug) {
                                             NSLog(@"Leave group %@", group);
                                         }
                                         dispatch_group_leave(group);
                                     }
                                 }];
    }];
    if (!group) {
        completion([parts componentsJoinedByString:@""], firstError, missingFunctionSignatures);
    } else {
        __weak __typeof(self) weakSelf = self;
        dispatch_notify(group, dispatch_get_main_queue(), ^{
            [weakSelf didFinishEvaluatingInterpolatedStringWithParts:parts
                                                               error:firstError
                                                             missing:missingFunctionSignatures
                                                          completion:completion];
        });
    }
}

- (void)didFinishEvaluatingInterpolatedStringWithParts:(NSArray *)parts
                                                 error:(NSError *)firstError
                                               missing:(NSSet<NSString *> *)missingFunctionSignatures
                                            completion:(void (^)(id, NSError *, NSSet<NSString *> *))completion {
    if (_debug) {
        NSLog(@"Group completed");
    }
    completion(firstError ? nil : [parts componentsJoinedByString:@""], firstError, missingFunctionSignatures);
}

- (void)evaluateArray:(NSArray *)array
           invocation:(NSString *)invocation
          withTimeout:(NSTimeInterval)timeInterval
   sideEffectsAllowed:(BOOL)sideEffectsAllowed
           completion:(void (^)(id, NSError *, NSSet<NSString *> *))completion {
    __block NSError *errorOut = nil;
    NSMutableSet<NSString *> *missing = [NSMutableSet setWithCapacity:2];
    NSMutableArray *populatedArray = [array mutableCopy];
    dispatch_group_t group = nil;
    if (timeInterval > 0) {
        group = dispatch_group_create();
    }
    [array enumerateObjectsUsingBlock:^(iTermParsedExpression *_Nonnull parsedExpression, NSUInteger idx,
                                        BOOL *_Nonnull stop) {
        iTermExpressionEvaluator *innerEvaluator =
            [[iTermExpressionEvaluator alloc] initWithParsedExpression:parsedExpression
                                                            invocation:invocation
                                                                 scope:self->_scope];
        [self->_innerEvaluators addObject:innerEvaluator];
        if (group) {
            dispatch_group_enter(group);
        }
        __block BOOL alreadyRun = NO;
        [innerEvaluator
            evaluateWithTimeout:timeInterval
             sideEffectsAllowed:sideEffectsAllowed
                     completion:^(iTermExpressionEvaluator *evaluator) {
                         // BUG-f954: Guard against multiple completion calls instead of crashing
                         if (alreadyRun) {
                             ELog(@"iTermExpressionEvaluator: evaluateArray completion called multiple times");
                             return;
                         }
                         alreadyRun = YES;
                         [missing unionSet:evaluator.missingValues];
                         if (evaluator.error) {
                             errorOut = evaluator.error;
                         } else {
                             populatedArray[idx] = evaluator.value;
                         }
                         if (group) {
                             dispatch_group_leave(group);
                         }
                     }];
    }];
    if (group) {
        dispatch_group_notify(group, dispatch_get_main_queue(), ^{
            completion(populatedArray, errorOut, missing);
        });
    } else {
        completion(populatedArray, errorOut, missing);
    }
}

- (NSString *)stringFromJSONObject:(id)jsonObject {
    NSString *string = [NSString castFrom:jsonObject];
    if (string) {
        return string;
    }
    NSNumber *number = [NSNumber castFrom:jsonObject];
    if (number) {
        return [number stringValue];
    }
    NSArray *array = [NSArray castFrom:jsonObject];
    if (array) {
        return [NSString stringWithFormat:@"[%@]", [[array mapWithBlock:^id(id anObject) {
                                              return [self stringFromJSONObject:anObject];
                                          }] componentsJoinedByString:@", "]];
    }

    if ([NSNull castFrom:jsonObject] || !jsonObject) {
        return @"";
    }

    return [NSJSONSerialization it_jsonStringForObject:jsonObject];
}

- (void)logError:(NSError *)error invocation:(NSString *)invocation {
    NSString *message =
        [NSString stringWithFormat:@"Error evaluating expression %@: %@\n", invocation, error.localizedDescription];
    [[iTermScriptHistoryEntry globalEntry] addOutput:message
                                          completion:^{
                                          }];
}

@end
