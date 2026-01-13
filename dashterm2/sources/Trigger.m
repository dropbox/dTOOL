//
//  Trigger.m
//  iTerm
//
//  Created by George Nachman on 9/23/11.
//

#import "Trigger.h"
#import "DebugLogging.h"
#import "iTermSwiftyString.h"
#import "iTermVariableScope.h"
#import "iTermWarning.h"
#import "iTermAdvancedSettingsModel.h"
#import "NSArray+iTerm.h"
#import "NSNumber+iTerm.h"
#import "NSStringITerm.h"
#import "ScreenChar.h"
#import <CommonCrypto/CommonDigest.h>
#import "DashTerm2SharedARC-Swift.h"

NSString *const kTriggerMatchTypeKey = @"matchType";
NSString *const kTriggerRegexKey = @"regex";
NSString *const kTriggerContentRegexKey = @"contentregex";
NSString *const kTriggerActionKey = @"action";
NSString *const kTriggerParameterKey = @"parameter";
NSString *const kTriggerPartialLineKey = @"partial";
NSString *const kTriggerDisabledKey = @"disabled";
NSString *const kTriggerNameKey = @"name";
NSString *const kTriggerPerformanceKey = @"performance";

@interface Trigger ()
@end

@implementation Trigger {
    // The last absolute line number on which this trigger fired for a partial
    // line. -1 means it has not fired on the current line.
    long long _lastLineNumber;
    NSString *regex_;
    NSString *contentRegex_;
    id param_;
    iTermSwiftyStringWithBackreferencesEvaluator *_evaluator;
    NSRegularExpression *_compiledRegex;
    NSError *_compiledRegexError; // Non-nil if regex compilation failed
    iTermMovingHistogram *_stats;
}

static void TriggerWarnAboutLegacyRegexPreference(void) {
    static dispatch_once_t onceToken;
    dispatch_once(&onceToken, ^{
        DLog(@"fastTriggerRegexes was disabled, but RegexKitLite-based evaluation is no longer available for security "
             @"reasons. Using NSRegularExpression with timeout protection instead.");
    });
}

@synthesize regex = regex_;
@synthesize param = param_;
@synthesize contentRegex = contentRegex_;

+ (NSSet<NSString *> *)synonyms {
    return [NSSet set];
}

// The purpose of this is to re-encode colors that were previously key-value encoded into hex so that the Python APi can
// consume them.
+ (NSDictionary *)sanitizedTriggerDictionary:(NSDictionary *)dict {
    Trigger *trigger = [self triggerFromUntrustedDict:dict];
    [trigger sanitize];
    return trigger.dictionaryValue;
}

+ (nullable Trigger *)triggerFromUntrustedDict:(NSDictionary *)dict {
    NSString *className = [NSString castFrom:[dict objectForKey:kTriggerActionKey]];
    if (!className) {
        DLog(@"Bad class name in %@", dict);
        return nil;
    }
    Class class = NSClassFromString(className);
    if (![class isSubclassOfClass:[Trigger class]] || class == [Trigger class]) {
        DLog(@"Bad class for valid name in %@", dict);
        return nil;
    }
    Trigger *trigger = [[class alloc] init];
    trigger.regex = [NSString castFrom:dict[kTriggerRegexKey]];
    trigger.contentRegex = [NSString castFrom:dict[kTriggerContentRegexKey]];
    trigger.param = dict[kTriggerParameterKey];
    trigger.partialLine = [[NSNumber coerceFrom:dict[kTriggerPartialLineKey]] boolValue];
    trigger.disabled = [[NSNumber coerceFrom:dict[kTriggerDisabledKey]] boolValue];
    trigger->_matchType = [[NSNumber coerceFrom:dict[kTriggerMatchTypeKey]] unsignedIntegerValue];
    trigger->_name = [NSString castFrom:dict[kTriggerNameKey]];
    if ([NSDictionary castFrom:dict[kTriggerPerformanceKey]]) {
        iTermHistogram *histogram =
            [[iTermHistogram alloc] initWithDictionary:[NSDictionary castFrom:dict[kTriggerPerformanceKey]]];
        trigger.performanceHistogram = histogram;
    }
    return trigger;
}

- (instancetype)init {
    self = [super init];
    if (self) {
        _lastLineNumber = -1;
        _stats = [[iTermMovingHistogram alloc] initWithBucketSize:100 numberOfBuckets:10];
    }
    return self;
}

- (NSString *)description {
    return
        [NSString stringWithFormat:@"<%@: %p name=%@ regex=%@ contentRegex=%@ param=%@>", NSStringFromClass(self.class),
                                   self, self.name, self.regex, self.contentRegex, self.param];
}

- (NSString *)action {
    return NSStringFromClass([self class]);
}

- (void)setAction:(NSString *)action {
    // BUG-f649: Replace assert(false) with ELog - action is read-only for Trigger
    ELog(@"BUG-f649: setAction called on Trigger - action is determined by class type and cannot be set");
}

- (void)sanitize {
    // Do nothing by default because most triggers don't neet sanitization.
}

+ (NSString *)title {
    // BUG-f650: Replace assert(false) with ELog and fallback - subclasses must override this
    ELog(@"BUG-f650: +title called on base Trigger class - subclass must override this method");
    return @"Unknown Trigger";
}

- (NSString *)title {
    // BUG-f651: Replace assert(false) with ELog and fallback - subclasses must override this
    ELog(@"BUG-f651: -title called on base Trigger class - subclass must override this method");
    return [[self class] title];
}

- (NSString *)triggerOptionalParameterPlaceholderWithInterpolation:(BOOL)interpolation {
    // BUG-f652: Replace assert(false) with ELog and fallback - subclasses must override this
    ELog(@"BUG-f652: triggerOptionalParameterPlaceholderWithInterpolation called on base Trigger class");
    return @"";
}

- (NSString *)triggerOptionalDefaultParameterValueWithInterpolation:(BOOL)interpolation {
    return nil;
}

- (BOOL)takesParameter {
    // BUG-f653: Replace assert(false) with ELog and fallback - subclasses must override this
    ELog(@"BUG-f653: takesParameter called on base Trigger class - subclass must override this method");
    return NO;
}

- (BOOL)paramIsPopupButton {
    return NO;
}

- (BOOL)paramIsTwoColorWells {
    return NO;
}

- (BOOL)paramIsTwoStrings {
    return NO;
}

- (NSDictionary *)menuItemsForPoupupButton {
    return nil;
}

- (BOOL)isIdempotent {
    return NO;
}

- (BOOL)detectsPrompt {
    return NO;
}

- (NSString *)helpText {
    return nil;
}

- (NSSet<NSNumber *> *)allowedMatchTypes {
    return [NSSet setWithArray:@[ @(iTermTriggerMatchTypeRegex) ]];
}

- (NSArray *)groupedMenuItemsForPopupButton {
    NSDictionary *menuItems = [self menuItemsForPoupupButton];
    if (menuItems) {
        return @[ menuItems ];
    } else {
        return nil;
    }
}

- (id<iTermFocusReportingTextFieldDelegate>)newParameterDelegateWithPassthrough:(id<NSTextFieldDelegate>)passthrough {
    return nil;
}

- (BOOL)performActionWithCapturedStrings:(NSArray<NSString *> *)stringArray
                          capturedRanges:(const NSRange *)capturedRanges
                               inSession:(id<iTermTriggerSession>)aSession
                                onString:(iTermStringLine *)stringLine
                    atAbsoluteLineNumber:(long long)lineNumber
                        useInterpolation:(BOOL)useInterpolation
                                    stop:(BOOL *)stop {
    // BUG-f654: Replace assert(false) with ELog and fallback - subclasses must override this
    ELog(@"BUG-f654: performActionWithCapturedStrings called on base Trigger class - subclass must override this "
         @"method");
    return NO;
}

- (BOOL)instantTriggerCanFireMultipleTimesPerLine {
    return NO;
}

- (void)setRegex:(NSString *)regex {
    regex_ = [regex copy];
    _compiledRegexError = nil;
    _compiledRegex = nil;

    if (regex_.length == 0) {
        // Empty regex is allowed - trigger simply won't match anything
        return;
    }

    NSError *error = nil;
    _compiledRegex = [NSRegularExpression regularExpressionWithPattern:regex_ options:0 error:&error];
    if (error) {
        _compiledRegexError = error;
        DLog(@"Failed to compile trigger regex '%@': %@", regex_, error.localizedDescription);
    }
}

- (void)setContentRegex:(NSString *_Nonnull)contentRegex {
    contentRegex_ = [contentRegex copy];
}

- (BOOL)hasValidRegex {
    // Valid if: no regex set (empty), OR regex compiled successfully (no error)
    return _compiledRegexError == nil;
}

- (NSError *)regexCompilationError {
    return _compiledRegexError;
}

// Maximum time in seconds to allow regex matching before aborting (ReDoS protection)
static const NSTimeInterval kRegexMatchTimeoutSeconds = 0.1;

- (void)enumerateMatchesInString:(NSString *)string
                           block:(void (^)(NSArray<NSString *> *capturedStrings, const NSRange *capturedRanges,
                                           BOOL *stop))block {
    // BUG-10355: Validate that regex compiled successfully before use
    if (!_compiledRegex) {
        if (_compiledRegexError) {
            DLog(@"Trigger regex '%@' failed to compile: %@. Skipping match.", regex_,
                 _compiledRegexError.localizedDescription);
        }
        // No compiled regex - nothing to match
        return;
    }

    const size_t maxStaticRangeCount = 16;
    __block size_t rangeCapacity = maxStaticRangeCount;
    NSRange rangeStorage[maxStaticRangeCount];
    __block NSRange *ranges = rangeStorage;
    __block NSRange *dynamicRangeStorage = NULL;

    // Track start time for ReDoS timeout protection
    NSDate *startTime = [NSDate date];

    // Use NSMatchingReportProgress to allow periodic timeout checks
    NSMatchingOptions options = NSMatchingReportProgress;

    [_compiledRegex
        enumerateMatchesInString:string
                         options:options
                           range:NSMakeRange(0, string.length)
                      usingBlock:^(NSTextCheckingResult *_Nullable result, NSMatchingFlags flags, BOOL *_Nonnull stop) {
                          // Check for timeout on progress callbacks (ReDoS protection)
                          if (flags & NSMatchingProgress) {
                              NSTimeInterval elapsed = -[startTime timeIntervalSinceNow];
                              if (elapsed > kRegexMatchTimeoutSeconds) {
                                  DLog(@"Trigger regex match timeout after %.3fs - possible ReDoS pattern", elapsed);
                                  *stop = YES;
                                  return;
                              }
                              return; // Progress callback, no match yet
                          }

                          // Skip if no result (can happen with progress/completion flags)
                          if (!result) {
                              return;
                          }

                          NSMutableArray<NSString *> *captures =
                              [NSMutableArray arrayWithCapacity:result.numberOfRanges];
                          if (result.numberOfRanges > rangeCapacity) {
                              dynamicRangeStorage =
                                  iTermRealloc(dynamicRangeStorage, result.numberOfRanges, sizeof(NSRange));
                              ranges = dynamicRangeStorage;
                              rangeCapacity = result.numberOfRanges;
                          }

                          for (NSInteger i = 0; i < result.numberOfRanges; i++) {
                              const NSRange range = [result rangeAtIndex:i];
                              NSString *substring;
                              // BUG-7306: Check for NSNotFound - capture group may not participate in match
                              if (range.location == NSNotFound) {
                                  substring = @"";
                              } else if (range.length == 0) {
                                  substring = @"";
                              } else {
                                  substring = [string substringWithRange:range];
                              }
                              [captures addObject:substring];
                              ranges[i] = range;
                          }
                          block(captures, ranges, stop);
                      }];
    if (ranges != rangeStorage) {
        free(ranges);
    }
}

- (iTermHistogram *)performanceHistogram {
    return _stats.histogram;
}

- (void)setPerformanceHistogram:(iTermHistogram *)performanceHistogram {
    [_stats setFromHistogram:performanceHistogram];
}

- (BOOL)tryString:(iTermStringLine *)stringLine
           inSession:(id<iTermTriggerSession>)aSession
         partialLine:(BOOL)partialLine
          lineNumber:(long long)lineNumber
    useInterpolation:(BOOL)useInterpolation {
    if (self.disabled) {
        return NO;
    }
    if (_partialLine && !self.instantTriggerCanFireMultipleTimesPerLine && _lastLineNumber == lineNumber) {
        // Already fired a on a partial line on this line.
        if (!partialLine) {
            _lastLineNumber = -1;
        }
        return NO;
    }
    if (partialLine && !_partialLine) {
        // This trigger doesn't support partial lines.
        return NO;
    }

    __block BOOL result = NO;
    const NSTimeInterval duration = [NSDate durationOfBlock:^{
        result = [self reallyTryString:stringLine
                             inSession:aSession
                           partialLine:partialLine
                            lineNumber:lineNumber
                      useInterpolation:useInterpolation];
    }];
    [_stats addValue:duration * 1000000];
    return result;
}

- (BOOL)reallyTryString:(iTermStringLine *)stringLine
              inSession:(id<iTermTriggerSession>)aSession
            partialLine:(BOOL)partialLine
             lineNumber:(long long)lineNumber
       useInterpolation:(BOOL)useInterpolation {
    __block BOOL stopFutureTriggersFromRunningOnThisLine = NO;
    NSString *s = stringLine.stringValue;
    DLog(@"Search for regex %@ in string %@", regex_, s);
    if (![iTermAdvancedSettingsModel fastTriggerRegexes]) {
        TriggerWarnAboutLegacyRegexPreference();
    }
    if (s != nil) {
        DLog(@"Use NSRegularExpression");
        [self
            enumerateMatchesInString:s
                               block:^(NSArray<NSString *> *stringArray, const NSRange *capturedRanges,
                                       BOOL *stopEnumerating) {
                                   self->_lastLineNumber = lineNumber;
                                   DLog(@"Trigger %@ matched string %@", self, s);
                                   if (![self
                                           performActionWithCapturedStrings:stringArray
                                                             capturedRanges:capturedRanges
                                                                  inSession:aSession
                                                                   onString:stringLine
                                                       atAbsoluteLineNumber:lineNumber
                                                           useInterpolation:useInterpolation
                                                                       stop:&stopFutureTriggersFromRunningOnThisLine]) {
                                       *stopEnumerating = YES;
                                   }
                               }];
    }
    if (!partialLine) {
        _lastLineNumber = -1;
    }
    return stopFutureTriggersFromRunningOnThisLine;
}

- (iTermPromise<NSString *> *)paramWithBackreferencesReplacedWithValues:(NSArray *)stringArray
                                                                absLine:(long long)absLine
                                                                  scope:(id<iTermTriggerScopeProvider>)scopeProvider
                                                       useInterpolation:(BOOL)useInterpolation {
    NSString *p = [NSString castFrom:self.param] ?: @"";
    if (useInterpolation && [p interpolatedStringContainsNonliteral]) {
        return [iTermPromise promise:^(id<iTermPromiseSeal> _Nonnull seal) {
            [scopeProvider performBlockWithScope:^(iTermVariableScope *_Nonnull scope,
                                                   id<iTermObject> _Nonnull object) {
                // BUG-f1367: Convert assert to guard - if not on main thread, dispatch to main thread
                if (![NSThread isMainThread]) {
                    dispatch_async(dispatch_get_main_queue(), ^{
                        [self
                            evaluateSwiftyStringParameter:p
                                           backreferences:stringArray
                                                  absLine:absLine
                                                    scope:scope
                                                    owner:object
                                               completion:^(NSString *value) {
                                                   if (value) {
                                                       [seal fulfill:value];
                                                   } else {
                                                       [seal
                                                           reject:[NSError
                                                                      errorWithDomain:@"com.dashterm.dashterm2.trigger"
                                                                                 code:0
                                                                             userInfo:nil]];
                                                   }
                                               }];
                    });
                    return;
                }
                [self evaluateSwiftyStringParameter:p
                                     backreferences:stringArray
                                            absLine:absLine
                                              scope:scope
                                              owner:object
                                         completion:^(NSString *value) {
                                             if (value) {
                                                 [seal fulfill:value];
                                             } else {
                                                 [seal reject:[NSError errorWithDomain:@"com.dashterm.dashterm2.trigger"
                                                                                  code:0
                                                                              userInfo:nil]];
                                             }
                                         }];
            }];
        }];
    }

    const NSUInteger count = stringArray.count;
    for (int i = 0; i < 9; i++) {
        NSString *rep = @"";
        if (count > i) {
            rep = stringArray[i];
        }
        p = [p stringByReplacingBackreference:i withString:rep];
    }
    p = [p stringByReplacingEscapedChar:'a' withString:@"\x07"];
    p = [p stringByReplacingEscapedChar:'b' withString:@"\x08"];
    p = [p stringByReplacingEscapedChar:'e' withString:@"\x1b"];
    p = [p stringByReplacingEscapedChar:'n' withString:@"\n"];
    p = [p stringByReplacingEscapedChar:'r' withString:@"\r"];
    p = [p stringByReplacingEscapedChar:'t' withString:@"\t"];
    p = [p stringByReplacingEscapedChar:'\\' withString:@"\\"];
    p = [p stringByReplacingEscapedHexValuesWithChars];

    return [iTermPromise promise:^(id<iTermPromiseSeal> _Nonnull seal) {
        [seal fulfill:p];
    }];
}

- (iTermVariableScope *)variableScope:(iTermVariableScope *)scope
               byAddingBackreferences:(NSArray<NSString *> *)backreferences {
    return [scope variableScopeByAddingBackreferences:backreferences owner:self];
}

- (void)evaluateSwiftyStringParameter:(NSString *)expression
                       backreferences:(NSArray<NSString *> *)backreferences
                              absLine:(long long)absLine
                                scope:(iTermVariableScope *)scope
                                owner:(id<iTermObject>)owner
                           completion:(void (^)(NSString *))completion {
    if (!_evaluator) {
        _evaluator = [[iTermSwiftyStringWithBackreferencesEvaluator alloc] initWithExpression:expression];
    } else {
        _evaluator.expression = expression;
    }
    __weak __typeof(self) weakSelf = self;
    NSDictionary *additionalContext = @{@"matches" : backreferences, @"line" : @(absLine)};
    if (absLine < 0) {
        additionalContext = [additionalContext dictionaryByRemovingObjectForKey:@"line"];
    }
    [_evaluator evaluateWithAdditionalContext:additionalContext
                                        scope:scope
                                        owner:owner
                                   completion:^(NSString *_Nullable value, NSError *_Nullable error) {
                                       if (error) {
                                           [weakSelf evaluationDidFailWithError:error];
                                           completion(nil);
                                       } else {
                                           completion(value);
                                       }
                                   }];
}

- (void)evaluationDidFailWithError:(NSError *)error {
    NSString *title = [NSString
        stringWithFormat:
            @"The following parameter for a “%@” trigger could not be evaluated:\n\n%@\n\nThe error was:\n\n%@",
            [[self class] title], _evaluator.expression, error.localizedDescription];
    [iTermWarning showWarningWithTitle:title
                               actions:@[ @"OK" ]
                             accessory:nil
                            identifier:@"NoSyncErrorInTriggerParameter"
                           silenceable:kiTermWarningTypeTemporarilySilenceable
                               heading:@"Error in Trigger Parameter"
                                window:nil];
}

- (NSComparisonResult)compareTitle:(Trigger *)other {
    return [[self title] compare:[other title]];
}

- (NSInteger)indexForObject:(id)object {
    return [object intValue];
}

- (id)objectAtIndex:(NSInteger)index {
    return @(index);
}

- (NSArray *)objectsSortedByValueInDict:(NSDictionary *)dict {
    return [dict keysSortedByValueUsingSelector:@selector(localizedCaseInsensitiveCompare:)];
}

- (int)defaultIndex {
    return 0;
}

- (id)defaultPopupParameterObject {
    return @0;
}

// Called before a trigger window opens.
- (void)reloadData {
}

- (NSDictionary *)dictionaryValue {
    return [@{
        kTriggerActionKey : NSStringFromClass(self.class),
        kTriggerRegexKey : self.regex ?: @"",
        kTriggerContentRegexKey : self.contentRegex ?: @"",
        kTriggerMatchTypeKey : @(self.matchType),
        kTriggerParameterKey : self.param ?: @"",
        kTriggerPartialLineKey : @(self.partialLine),
        kTriggerDisabledKey : @(self.disabled),
        kTriggerNameKey : self.name ?: [NSNull null]
    } dictionaryByRemovingNullValues];
}

- (NSData *)digest {
    NSDictionary *triggerDictionary = [self dictionaryValue];

    // Glom all the data together as key=value\nkey=value\n...
    // Estimate ~50 chars per key-value pair
    NSMutableString *temp = [NSMutableString stringWithCapacity:triggerDictionary.count * 50];
    for (NSString *key in [[triggerDictionary allKeys] sortedArrayUsingSelector:@selector(compare:)]) {
        [temp appendFormat:@"%@=%@\n", key, triggerDictionary[key]];
    }

    NSData *data = [temp dataUsingEncoding:NSUTF8StringEncoding];
    unsigned char hash[CC_SHA1_DIGEST_LENGTH];
    if (CC_SHA1([data bytes], [data length], hash)) {
        NSData *sha1 = [NSData dataWithBytes:hash length:CC_SHA1_DIGEST_LENGTH];
        return sha1;
    } else {
        return data;
    }
}

+ (NSDictionary *)triggerNormalizedDictionary:(NSDictionary *)dict {
    NSMutableDictionary *temp = [dict mutableCopy];
    if (!temp[kTriggerPartialLineKey]) {
        temp[kTriggerPartialLineKey] = @NO;
    }
    if (!temp[kTriggerDisabledKey]) {
        temp[kTriggerDisabledKey] = @NO;
    }
    if (!temp[kTriggerParameterKey]) {
        temp[kTriggerParameterKey] = @"";
    }
    [temp removeObjectForKey:kTriggerPerformanceKey];
    return temp;
}

- (NSAttributedString *)titleAttributedString {
    NSMutableParagraphStyle *paragraphStyle = [[NSParagraphStyle defaultParagraphStyle] mutableCopy];
    NSDictionary *boldAttributes = @{
        NSParagraphStyleAttributeName : paragraphStyle,
        NSFontAttributeName : [NSFont systemFontOfSize:[NSFont systemFontSize] weight:NSFontWeightSemibold]
    };
    return [[NSAttributedString alloc] initWithString:[self.class.title stringByRemovingSuffix:@"…"]
                                           attributes:boldAttributes];
}

- (NSAttributedString *)attributedString {
    NSAttributedString *newline = [[NSAttributedString alloc] initWithString:@"\n" attributes:self.regularAttributes];
    id regexAttributedString = self.regex.length > 0 ? [self regexAttributedString] : [NSNull null];
    NSArray *lines = nil;
    NSString *instantEmoji = self.partialLine ? @"⚡︎ " : nil;
    if ([self.name stringByTrimmingCharactersInSet:[NSCharacterSet whitespaceAndNewlineCharacterSet]].length > 0) {
        NSString *name = self.name;
        if (instantEmoji) {
            name = [instantEmoji stringByAppendingString:name];
        }
        NSAttributedString *nameAttributedString = [[NSAttributedString alloc] initWithString:name
                                                                                   attributes:self.nameAttributes];
        NSAttributedString *functionAttributedString = [self functionAttributedString];
        lines = @[ nameAttributedString, regexAttributedString, functionAttributedString ];
    } else {
        NSAttributedString *line2;
        if (instantEmoji) {
            NSAttributedString *instant = [[NSAttributedString alloc] initWithString:instantEmoji
                                                                          attributes:self.regularAttributes];
            line2 = [instant attributedStringByAppendingAttributedString:self.functionAttributedString];
        } else {
            line2 = self.functionAttributedString;
        }
        lines = @[ regexAttributedString, line2 ];
    }
    lines = [lines filteredArrayUsingBlock:^BOOL(id anObject) {
        return [anObject isKindOfClass:[NSAttributedString class]];
    }];
    return [lines it_componentsJoinedBySeparator:newline];
}

- (NSAttributedString *)regexAttributedString {
    NSMutableParagraphStyle *paragraphStyle = [[NSParagraphStyle defaultParagraphStyle] mutableCopy];
    NSDictionary *monospacedAttributes = @{
        NSParagraphStyleAttributeName : paragraphStyle,
        NSFontAttributeName : [NSFont monospacedSystemFontOfSize:[NSFont systemFontSize] weight:NSFontWeightRegular]
    };
    NSDictionary *plainAttributes = @{
        NSParagraphStyleAttributeName : paragraphStyle,
        NSFontAttributeName : [NSFont systemFontOfSize:[NSFont systemFontSize] weight:NSFontWeightRegular]
    };
    switch (self.matchType) {
        case iTermTriggerMatchTypeRegex:
        case iTermTriggerMatchTypeURLRegex:
            return [[NSAttributedString alloc] initWithString:[NSString stringWithFormat:@"/%@/", self.regex ?: @""]
                                                   attributes:monospacedAttributes];
        case iTermTriggerMatchTypePageContentRegex:
            return [@[
                [[NSAttributedString alloc] initWithString:@"Content: " attributes:plainAttributes],
                [[NSAttributedString alloc] initWithString:[NSString stringWithFormat:@"/%@/", self.contentRegex ?: @""]
                                                attributes:monospacedAttributes],
                [[NSAttributedString alloc] initWithString:@" URL: " attributes:plainAttributes],
                [[NSAttributedString alloc] initWithString:[NSString stringWithFormat:@"/%@/", self.regex ?: @""]
                                                attributes:monospacedAttributes]
            ] attributedComponentsJoinedByAttributedString:nil];
    }
}

- (NSAttributedString *)functionAttributedString {
    NSAttributedString *paramAttributedString = [self paramAttributedString];
    if (!paramAttributedString) {
        return [self titleAttributedString];
    }

    NSAttributedString *space = [[NSAttributedString alloc] initWithString:@" " attributes:self.regularAttributes];

    return [[self.titleAttributedString attributedStringByAppendingAttributedString:space]
        attributedStringByAppendingAttributedString:paramAttributedString];
}

- (NSDictionary *)regularAttributes {
    NSMutableParagraphStyle *paragraphStyle = [[NSParagraphStyle defaultParagraphStyle] mutableCopy];
    paragraphStyle.lineBreakMode = NSLineBreakByTruncatingTail;
    NSDictionary *attributes = @{
        NSParagraphStyleAttributeName : paragraphStyle,
        NSFontAttributeName : [NSFont systemFontOfSize:[NSFont systemFontSize]]
    };
    return attributes;
}

- (NSDictionary *)nameAttributes {
    NSMutableParagraphStyle *paragraphStyle = [[NSParagraphStyle defaultParagraphStyle] mutableCopy];
    paragraphStyle.lineBreakMode = NSLineBreakByTruncatingTail;
    NSDictionary *attributes = @{
        NSParagraphStyleAttributeName : paragraphStyle,
        NSFontAttributeName : [NSFont boldSystemFontOfSize:[NSFont systemFontSize] + 2]
    };
    return attributes;
}

- (NSAttributedString *)paramAttributedString {
    NSString *string = [NSString castFrom:self.param];
    if (!string) {
        return nil;
    }
    return [[NSAttributedString alloc] initWithString:string attributes:self.regularAttributes];
}

- (BOOL)isBrowserTrigger {
    return NO;
}

#pragma mark - iTermObject

- (iTermBuiltInFunctions *)objectMethodRegistry {
    return nil;
}

- (iTermVariableScope *)objectScope {
    return nil;
}

@end
