//
//  iTermPreferencesBaseViewController.m
//  iTerm
//
//  Created by George Nachman on 4/6/14.
//
//

#import "iTermPreferencesBaseViewController.h"

#import "DebugLogging.h"
#import "DashTerm2SharedARC-Swift.h"
#import "iTermPreferences.h"
#import "iTermSlider.h"
#import "NSArray+iTerm.h"
#import "NSColor+iTerm.h"
#import "NSDictionary+iTerm.h"
#import "NSObject+iTerm.h"
#import "NSStringITerm.h"
#import "NSTextField+iTerm.h"
#import "NSView+iTerm.h"
#import "PreferencePanel.h"

#import <ColorPicker/ColorPicker.h>

@interface NSStepper (Prefs)
- (int)separatorTolerantIntValue;
@end

@implementation NSStepper (Prefs)
- (int)separatorTolerantIntValue {
    return [self intValue];
}
@end

@interface iTermPreferencesInnerTabContainerView : NSView
@end

@implementation iTermPreferencesInnerTabContainerView
@end

NSString *const kPreferenceDidChangeFromOtherPanel = @"kPreferenceDidChangeFromOtherPanel";

// key for userInfo dictionary of kPreferenceDidChangeFromOtherPanel notification having
// key of changed preference.
NSString *const kPreferenceDidChangeFromOtherPanelKeyUserInfoKey = @"key";
NSString *const iTermPreferencesDidToggleIndicateNonDefaultValues =
    @"iTermPreferencesDidToggleIndicateNonDefaultValues";

@interface iTermPreferencesBaseViewController () <NSControlTextEditingDelegate>
// If set to YES, then controls won't be updated with values from backing store when it changes.
@property (nonatomic, assign) BOOL disableUpdates;
@end

@implementation iTermPreferencesBaseViewController {
    NSMapTable<NSView *, PreferenceInfo *> *_keyMap;
    // Set of all defined keys (KEY_XXX).
    NSMutableSet *_keys;
    NSMutableSet *_keysWithSyntheticGetters;
    NSMutableSet *_keysWithSyntheticSetters;

    NSMutableArray<iTermPreferencesSearchDocument *> *_docs;
    NSRect _desiredFrame;

    NSPointerArray *_otherSearchableViews;
    BOOL _initialized;
}

- (instancetype)initWithNibName:(NSString *)nibNameOrNil bundle:(NSBundle *)nibBundleOrNil {
    self = [super initWithNibName:nibNameOrNil bundle:nibBundleOrNil];
    [self commonInit];
    return self;
}

- (instancetype)initWithCoder:(NSCoder *)coder {
    self = [super initWithCoder:coder];
    [self commonInit];
    return self;
}

- (void)commonInit {
    if (_initialized) {
        DLog(@"Double initialization detected for %@", self);
        return;
    }
    _initialized = YES;
    _internalState = [[NSMutableDictionary alloc] initWithCapacity:32];
    _keyMap = [[NSMapTable alloc] initWithKeyOptions:NSPointerFunctionsStrongMemory
                                        valueOptions:NSPointerFunctionsStrongMemory
                                            capacity:16];
    _keys = [[NSMutableSet alloc] initWithCapacity:32]; // Typical number of preference keys per controller
    _keysWithSyntheticGetters = [[NSMutableSet alloc] initWithCapacity:4]; // Few keys use synthetic getters
    _keysWithSyntheticSetters = [[NSMutableSet alloc] initWithCapacity:4]; // Few keys use synthetic setters
    _docs = [[NSMutableArray alloc] initWithCapacity:32];                  // One doc per searchable key
    _otherSearchableViews = [NSPointerArray weakObjectsPointerArray];
    [[NSNotificationCenter defaultCenter] addObserver:self
                                             selector:@selector(didToggleIndicateNonDefaultValues:)
                                                 name:iTermPreferencesDidToggleIndicateNonDefaultValues
                                               object:nil];

    [[NSNotificationCenter defaultCenter] addObserver:self
                                             selector:@selector(preferenceDidChangeFromOtherPanel:)
                                                 name:kPreferenceDidChangeFromOtherPanel
                                               object:nil];

    [[NSNotificationCenter defaultCenter] addObserver:self
                                             selector:@selector(preferencePanelWillClose:)
                                                 name:kPreferencePanelWillCloseNotification
                                               object:nil];
}

- (void)dealloc {
    [[NSNotificationCenter defaultCenter] removeObserver:self];
}

- (PreferenceInfo *)infoForKey:(NSString *)key {
    for (NSView *control in _keyMap) {
        PreferenceInfo *info = [self infoForControl:control];
        if ([info.key isEqualToString:key]) {
            return info;
        }
    }
    return nil;
}

#pragma mark - Methods to override

- (void)setObjectsFromDictionary:(NSDictionary *)dictionary {
    [dictionary enumerateKeysAndObjectsUsingBlock:^(id _Nonnull key, id _Nonnull obj, BOOL *_Nonnull stop) {
        [self setObject:obj forKey:key];
    }];
}

- (id)syntheticObjectForKey:(NSString *)key {
    PreferenceInfo *info = [self infoForKey:key];
    return info.syntheticGetter();
}

- (void)setSyntheticValue:(id)value forKey:(NSString *)key {
    PreferenceInfo *info = [self infoForKey:key];
    DLog(@"setSyntheticValue:%@ forKey:%@, info=%@", value, key, info);
    return info.syntheticSetter(value);
}

- (BOOL)keyHasSyntheticGetter:(NSString *)key {
    return [_keysWithSyntheticGetters containsObject:key];
}

- (BOOL)keyHasSyntheticSetter:(NSString *)key {
    return [_keysWithSyntheticSetters containsObject:key];
}

- (BOOL)boolForKey:(NSString *)key {
    if ([_keysWithSyntheticGetters containsObject:key]) {
        return [[self syntheticObjectForKey:key] boolValue];
    }
    return [iTermPreferences boolForKey:key];
}

- (void)setBool:(BOOL)value forKey:(NSString *)key {
    if ([_keysWithSyntheticSetters containsObject:key]) {
        [self setSyntheticValue:@(value) forKey:key];
        return;
    }
    [iTermPreferences setBool:value forKey:key];
}

- (int)intForKey:(NSString *)key {
    if ([_keysWithSyntheticGetters containsObject:key]) {
        return [[self syntheticObjectForKey:key] intValue];
    }
    return [iTermPreferences intForKey:key];
}

- (void)setInt:(int)value forKey:(NSString *)key {
    if ([_keysWithSyntheticSetters containsObject:key]) {
        [self setSyntheticValue:@(value) forKey:key];
        return;
    }
    [iTermPreferences setInt:value forKey:key];
}

- (NSInteger)integerForKey:(NSString *)key {
    if ([_keysWithSyntheticGetters containsObject:key]) {
        return [[self syntheticObjectForKey:key] integerValue];
    }
    return [iTermPreferences integerForKey:key];
}

- (void)setInteger:(NSInteger)value forKey:(NSString *)key {
    if ([_keysWithSyntheticSetters containsObject:key]) {
        [self setSyntheticValue:@(value) forKey:key];
        return;
    }
    [iTermPreferences setInteger:value forKey:key];
}

- (NSUInteger)unsignedIntegerForKey:(NSString *)key {
    if ([_keysWithSyntheticGetters containsObject:key]) {
        return [[self syntheticObjectForKey:key] unsignedIntegerValue];
    }
    return [iTermPreferences unsignedIntegerForKey:key];
}

- (void)setUnsignedInteger:(NSUInteger)value forKey:(NSString *)key {
    if ([_keysWithSyntheticSetters containsObject:key]) {
        [self setSyntheticValue:@(value) forKey:key];
        return;
    }
    [iTermPreferences setUnsignedInteger:value forKey:key];
}

- (double)floatForKey:(NSString *)key {
    if ([_keysWithSyntheticGetters containsObject:key]) {
        return [[self syntheticObjectForKey:key] doubleValue];
    }
    return [iTermPreferences floatForKey:key];
}

- (void)setFloat:(double)value forKey:(NSString *)key {
    if ([_keysWithSyntheticSetters containsObject:key]) {
        [self setSyntheticValue:@(value) forKey:key];
        return;
    }
    [iTermPreferences setFloat:value forKey:key];
}

- (double)doubleForKey:(NSString *)key {
    if ([_keysWithSyntheticGetters containsObject:key]) {
        return [[self syntheticObjectForKey:key] doubleValue];
    }
    return [iTermPreferences doubleForKey:key];
}

- (void)setDouble:(double)value forKey:(NSString *)key {
    if ([_keysWithSyntheticSetters containsObject:key]) {
        [self setSyntheticValue:@(value) forKey:key];
        return;
    }
    [iTermPreferences setDouble:value forKey:key];
}

- (NSString *)stringForKey:(NSString *)key {
    if ([_keysWithSyntheticGetters containsObject:key]) {
        return [self syntheticObjectForKey:key];
    }
    return [iTermPreferences stringForKey:key];
}

- (void)setString:(NSString *)value forKey:(NSString *)key {
    if ([_keysWithSyntheticSetters containsObject:key]) {
        [self setSyntheticValue:value forKey:key];
        return;
    }
    DLog(@"calling [iTermPreferences setString:%@ forKey:%@]", value, key);
    [iTermPreferences setString:value forKey:key];
}

- (NSObject *)objectForKey:(NSString *)key {
    if ([_keysWithSyntheticGetters containsObject:key]) {
        return [self syntheticObjectForKey:key];
    }
    return [iTermPreferences objectForKey:key];
}

- (void)setObject:(NSObject *)object forKey:(NSString *)key {
    if ([_keysWithSyntheticSetters containsObject:key]) {
        [self setSyntheticValue:object forKey:key];
        return;
    }
    [iTermPreferences setObject:object forKey:key];
}

- (BOOL)keyHasDefaultValue:(NSString *)key {
    return [iTermPreferences keyHasDefaultValue:key];
}

- (BOOL)defaultValueForKey:(NSString *)key isCompatibleWithType:(PreferenceInfoType)type {
    return [iTermPreferences defaultValueForKey:key isCompatibleWithType:type];
}

- (id)defaultValueForKey:(NSString *)key {
    return [iTermPreferences defaultObjectForKey:key];
}

- (BOOL)valueOfKeyEqualsDefaultValue:(NSString *)key {
    // I use nonzero epsilon because otherwise colors don't compare equal when they ought to.
    return [NSObject object:[iTermPreferences defaultObjectForKey:key]
        isNullablyEqualToObject:[self objectForKey:key]
                        epsilon:0.001];
}

- (BOOL)shouldUpdateOtherPanels {
    return YES;
}

#pragma mark - APIs

// BUG-f1086: Replace assert() with guard to prevent crash on unknown control
- (IBAction)settingChanged:(id)sender {
    PreferenceInfo *info = [self infoForControl:sender];
    if (!info) {
        DLog(@"BUG-f1086: settingChanged called for unknown control %@", sender);
        return; // Unknown control, ignore
    }
    DLog(@"settingChanged:%@", info.key);

    if (info.willChange) {
        DLog(@"willChange()");
        info.willChange();
    }

    if (info.customSettingChangedHandler) {
        DLog(@"customSettingChangedHandler()");
        info.customSettingChangedHandler(sender);
    } else {
        DLog(@"type=%@", @(info.type));
        switch (info.type) {
            case kPreferenceInfoTypeCheckbox:
                [self setBool:([(NSButton *)sender state] == NSControlStateValueOn) forKey:info.key];
                break;

            case kPreferenceInfoTypeInvertedCheckbox:
                [self setBool:([(NSButton *)sender state] == NSControlStateValueOff) forKey:info.key];
                break;

            case kPreferenceInfoTypeIntegerTextField: {
                // This clamps the main control's value and returns the attempted value.
                const int mainControlValue = [self applyIntegerConstraints:info];
                // The sender might be an associated view whose value is unclamped.
                const int senderValue = [sender separatorTolerantIntValue];
                int intValue = senderValue;
                const int before = MAX(mainControlValue, senderValue);
                if (!NSLocationInRange(before, info.range)) {
                    intValue = MAX(MIN(NSMaxRange(info.range) - 1, intValue), info.range.location);
                    if (intValue != before && info.didClamp) {
                        info.didClamp(before);
                    }
                }
                if (!info.deferUpdate) {
                    [self setInt:intValue forKey:info.key];
                }
                info.associatedStepper.integerValue = intValue;
                break;
            }

            case kPreferenceInfoTypeDoubleTextField: {
                NSScanner *scanner = [NSScanner localizedScannerWithString:[sender stringValue]];
                double value;
                if ([scanner scanDouble:&value]) {
                    // A double value may take a legal but non-canonical form during editing. For
                    // example, "1." is a valid way of specifying 1.0. But we don't want to replace
                    // the control's value with "1", or there's no way of entering "1.2", for
                    // example. We turn on disableUpdates so -setDouble:forKey: doesn't have the
                    // side-effect of updating the control with its canonical value.
                    BOOL savedValue = self.disableUpdates;
                    self.disableUpdates = YES;
                    [self setDouble:value forKey:info.key];
                    self.disableUpdates = savedValue;
                }
                break;
            }

            case kPreferenceInfoTypeUnsignedIntegerTextField:
                [self applyUnsignedIntegerConstraints:info];
                [self setUnsignedInteger:[sender separatorTolerantUnsignedIntegerValue] forKey:info.key];
                break;

            case kPreferenceInfoTypeStringTextField:
            case kPreferenceInfoTypePasswordTextField:
                [self setString:[sender stringValue] forKey:info.key];
                break;
            case kPreferenceInfoTypeStringTextView:
                [self setString:[[sender textStorage] string] forKey:info.key];
                break;
            case kPreferenceInfoTypeTokenField:
                [self setObject:[sender objectValue] forKey:info.key];
                break;

            case kPreferenceInfoTypeRadioButton: {
                NSNumber *defaultValue = [NSNumber castFrom:[self defaultValueForKey:info.key]];
                [self setInteger:[info.control it_selectedRadioButtonTagWithDefaultValue:defaultValue.integerValue]
                          forKey:info.key];
                break;
            }

            case kPreferenceInfoTypeMatrix:
                // BUG-428: Replace assert(false) with DLog - Matrix type must use a custom setting changed handler
                DLog(@"Warning: kPreferenceInfoTypeMatrix must use a custom setting changed handler for key '%@'. No "
                     @"action taken.",
                     info.key);
                break;

            case kPreferenceInfoTypePopup:
                [self setInt:[sender selectedTag] forKey:info.key];
                break;

            case kPreferenceInfoTypeSegmentedControl:
                [self setInt:[sender selectedSegment] forKey:info.key];
                break;

            case kPreferenceInfoTypeStringPopup:
                [self setObject:[[sender selectedItem] representedObject] forKey:info.key];
                break;

            case kPreferenceInfoTypeUnsignedIntegerPopup:
                // BUG-f1087: Replace assert() with guard to prevent crash on invalid tag
                if ([sender selectedTag] < 0) {
                    DLog(@"BUG-f1087: UnsignedIntegerPopup has negative selectedTag %ld for key '%@'",
                         (long)[sender selectedTag], info.key);
                    break; // Invalid tag, skip update
                }
                [self setUnsignedInteger:[sender selectedTag] forKey:info.key];
                break;

            case kPreferenceInfoTypeSlider:
                [self setFloat:[sender doubleValue] forKey:info.key];
                break;

            case kPreferenceInfoTypeColorWell:
                [self setObject:[[(CPKColorWell *)sender color] dictionaryValue] forKey:info.key];
                break;

            default:
                // BUG-429: Replace assert(false) with ELog - unknown preference type in settingChanged
                ELog(@"Unknown preference info type %d for key '%@'. No action taken.", (int)info.type, info.key);
                break;
        }
        if (info.onChange) {
            info.onChange();
        }
    }
    if (info.observer) {
        info.observer();
    }
    if ([self shouldUpdateOtherPanels]) {
        [[NSNotificationCenter defaultCenter]
            postNotificationName:kPreferenceDidChangeFromOtherPanel
                          object:self
                        userInfo:@{kPreferenceDidChangeFromOtherPanelKeyUserInfoKey : info.key}];
    }
    [self updateNonDefaultIndicatorVisibleForInfo:info];
}

- (PreferenceInfo *)defineControl:(NSView *)control
                              key:(NSString *)key
                      displayName:(NSString *)displayName
                             type:(PreferenceInfoType)type {
    return [self defineControl:control
                           key:key
                   relatedView:nil
                   displayName:displayName
                          type:type
                settingChanged:NULL
                        update:NULL
                    searchable:YES];
}

- (PreferenceInfo *)defineControl:(NSView *)control
                              key:(NSString *)key
                      relatedView:(NSView *)relatedView
                             type:(PreferenceInfoType)type {
    return [self defineControl:control
                           key:key
                   relatedView:relatedView
                   displayName:nil
                          type:type
                settingChanged:NULL
                        update:NULL
                    searchable:YES];
}

- (PreferenceInfo *)defineControl:(NSView *)control
                              key:(NSString *)key
                      displayName:(NSString *)displayName
                             type:(PreferenceInfoType)type
                   settingChanged:(void (^)(id))settingChanged
                           update:(BOOL (^)(void))update {
    return [self defineControl:control
                           key:key
                   relatedView:nil
                   displayName:displayName
                          type:type
                settingChanged:settingChanged
                        update:update
                    searchable:YES];
}

- (PreferenceInfo *)defineControl:(NSView *)control
                              key:(NSString *)key
                      relatedView:(NSView *)relatedView
                             type:(PreferenceInfoType)type
                   settingChanged:(void (^)(id))settingChanged
                           update:(BOOL (^)(void))update {
    return [self defineControl:control
                           key:key
                   relatedView:relatedView
                   displayName:nil
                          type:type
                settingChanged:settingChanged
                        update:update
                    searchable:YES];
}

- (PreferenceInfo *)defineUnsearchableControl:(NSView *)control key:(NSString *)key type:(PreferenceInfoType)type {
    return [self defineControl:control
                           key:key
                   relatedView:nil
                   displayName:nil
                          type:type
                settingChanged:nil
                        update:nil
                    searchable:NO];
}

- (ProfileType)profileTypesForView:(NSView *)view {
    if ([self isKindOfClass:[iTermProfilePreferencesBaseViewController class]]) {
        return view.enclosingModalEnclosure.visibleForProfileTypes;
    }
    return ProfileTypeAll;
}

- (void)addViewToSearchIndex:(NSView *)view
                 displayName:(NSString *)displayName
                     phrases:(NSArray<NSString *> *)phrases
                         key:(NSString *)key {
    NSString *nonnilKey = key ?: [[NSUUID UUID] UUIDString];
    if (displayName) {
        iTermPreferencesSearchDocument *doc =
            [iTermPreferencesSearchDocument documentWithDisplayName:displayName
                                                         identifier:nonnilKey
                                                     keywordPhrases:phrases
                                                       profileTypes:[self profileTypesForView:view]];
        doc.ownerIdentifier = self.documentOwnerIdentifier;
        [_docs addObject:doc];
    } else {
        DLog(@"Warning: no display name for %@", key);
    }
    if (!key || [key hasPrefix:@"NoUserDefault"]) {
        // BUG-f1088: Replace assert() with guard to prevent crash on duplicate accessibility identifier
        if (view.accessibilityIdentifier) {
            DLog(@"BUG-f1088: view already has accessibilityIdentifier '%@', expected nil for key '%@'",
                 view.accessibilityIdentifier, nonnilKey);
        }
        view.accessibilityIdentifier = nonnilKey;
        [_otherSearchableViews addPointer:(__bridge void *)view];
    }
}

- (PreferenceInfo *)defineControl:(NSView *)control
                              key:(NSString *)key
                      relatedView:(NSView *)relatedView
                      displayName:(NSString *)forceDisplayName
                             type:(PreferenceInfoType)type
                   settingChanged:(void (^)(id))settingChanged
                           update:(BOOL (^)(void))update
                       searchable:(BOOL)searchable {
    if (!control) {
        [self checkAppSignatureForMissingControlWithKey:key];
    }
    // BUG-f1089: Replace assert() calls with guards to prevent crashes on invalid control definitions
    if ([_keyMap objectForKey:(id)key]) {
        DLog(@"BUG-f1089: Duplicate control definition for key '%@' - ignoring duplicate", key);
        return nil; // Duplicate key, return existing or nil
    }
    if (!key) {
        DLog(@"BUG-f1089: defineControl called with nil key");
        return nil; // No key, can't define control
    }
    if (!control) {
        DLog(@"BUG-f1089: defineControl called with nil control for key '%@'", key);
        return nil; // No control, can't define
    }
    if (![self keyHasDefaultValue:key]) {
        DLog(@"BUG-f1089: key '%@' has no default value", key);
        return nil; // No default value configured
    }
    if (!settingChanged || !update) {
        if (![self defaultValueForKey:key isCompatibleWithType:type]) {
            DLog(@"BUG-f1089: default value for key '%@' is not compatible with type %d", key, (int)type);
            return nil; // Type mismatch
        }
        if (type == kPreferenceInfoTypeMatrix) {
            DLog(@"BUG-f1089: Matrix type for key '%@' requires both settingChanged and update handlers", key);
            return nil; // Matrix type requires both handlers
        }
    }

    return [self unsafeDefineControl:control
                                 key:key
                         relatedView:relatedView
                         displayName:forceDisplayName
                                type:type
                      settingChanged:settingChanged
                              update:update
                          searchable:searchable];
}

- (void)checkAppSignatureForMissingControlWithKey:(NSString *)key {
    NSString *team = [iTermAppSignatureValidator currentAppTeamID];
    NSString *message;
    if (!team) {
        message = @"A required file appears to be missing or corrupted and DashTerm2's code signature could not be "
                  @"verified. You should download a fresh copy of the app and reinstall it.";
    } else if (![team isEqualToString:@"H7V7XYVQ7D"]) {
        message = @"A required file appears to be missing or corrupted and DashTerm2's code signature did not match "
                  @"that of the official distribution. You should download a fresh copy of the app and reinstall it.";
    } else {
        message = @"A required file appears to be missing or corrupted, yet against all odds the code signature for "
                  @"DashTerm2 is valid. Please file a bug at https://github.com/ayates_dbx/dashterm2/issues";
    }
    NSAlert *alert = [[NSAlert alloc] init];
    [alert setMessageText:@"Application Corrupt"];
    [alert
        setInformativeText:[NSString stringWithFormat:@"While trying to load the setting for “%@”: %@", key, message]];
    [alert addButtonWithTitle:@"OK"];
    [alert setAlertStyle:NSAlertStyleCritical];
    [alert runModal];
}
- (PreferenceInfo *)unsafeDefineControl:(NSView *)control
                                    key:(NSString *)key
                            relatedView:(NSView *)relatedView
                            displayName:(NSString *)forceDisplayName
                                   type:(PreferenceInfoType)type
                         settingChanged:(void (^)(id))settingChanged
                                 update:(BOOL (^)(void))update
                             searchable:(BOOL)searchable {
    PreferenceInfo *info = [PreferenceInfo infoForPreferenceWithKey:key type:type control:control];
    info.relatedView = relatedView;
    info.customSettingChangedHandler = settingChanged;
    info.onUpdate = update;
    [_keyMap setObject:info forKey:control];
    [_keys addObject:key];

    if (searchable) {
        NSMutableArray<NSString *> *phrases =
            [[NSMutableArray alloc] initWithCapacity:8]; // Typical number of search phrases per control
        NSString *inferredDisplayName = [self displayNameForControl:control
                                                        relatedView:relatedView
                                                               type:type
                                                            phrases:phrases];
        NSString *displayName = forceDisplayName ?: inferredDisplayName;
        [self addViewToSearchIndex:control displayName:displayName phrases:phrases key:key];
    }

    [self updateValueForInfo:info];

    return info;
}

- (void)setControl:(NSView *)control inPreference:(PreferenceInfo *)info {
    [_keyMap removeObjectForKey:info.control];
    info.control = control;
    [_keyMap setObject:info forKey:control];
}

- (NSString *)displayNameForControl:(NSView *)control
                        relatedView:(NSView *)relatedView
                               type:(PreferenceInfoType)type
                            phrases:(NSMutableArray<NSString *> *)phrases {
    NSString *displayName = nil;
    if (control.toolTip) {
        [phrases addObject:control.toolTip];
    }
    NSPopUpButton *popup = [NSPopUpButton castFrom:control];
    if (popup) {
        if (popup.title) {
            displayName = popup.title;
            [phrases addObject:popup.title];
        }
        for (NSMenuItem *item in popup.menu.itemArray) {
            [phrases addObject:item.title];
        }
    }
    NSMatrix *matrix = [NSMatrix castFrom:control];
    if (matrix) {
        for (NSInteger y = 0; y < matrix.numberOfRows; y++) {
            for (NSInteger x = 0; x < matrix.numberOfColumns; x++) {
                NSCell *cell = [matrix cellAtRow:y column:x];
                NSString *title = [cell title];
                if (title) {
                    [phrases addObject:title];
                }
            }
        }
    }
    NSButton *button = [NSButton castFrom:control];
    if (button) {
        NSString *title = button.title;
        if (title) {
            displayName = title;
            [phrases addObject:title];
        }
    }
    NSSlider *slider = [NSSlider castFrom:control];
    if (slider) {
        [phrases addObject:@"slider"];
    }
    CPKColorWell *colorWell = [CPKColorWell castFrom:control];
    if (colorWell) {
        [phrases addObject:@"color"];
    }
    NSTextField *textField = [NSTextField castFrom:control];
    if (textField) {
        if (textField.placeholderString) {
            [phrases addObject:textField.placeholderString];
        }
    }
    switch (type) {
        case kPreferenceInfoTypePopup:
        case kPreferenceInfoTypeStringPopup:
        case kPreferenceInfoTypeMatrix:
        case kPreferenceInfoTypeSlider:
        case kPreferenceInfoTypeInvertedCheckbox:
        case kPreferenceInfoTypeCheckbox:
        case kPreferenceInfoTypeColorWell:
        case kPreferenceInfoTypeTokenField:
        case kPreferenceInfoTypeDoubleTextField:
        case kPreferenceInfoTypeStringTextField:
        case kPreferenceInfoTypeStringTextView:
        case kPreferenceInfoTypeIntegerTextField:
        case kPreferenceInfoTypeUnsignedIntegerPopup:
        case kPreferenceInfoTypeUnsignedIntegerTextField:
        case kPreferenceInfoTypePasswordTextField:
            break;

        case kPreferenceInfoTypeSegmentedControl:
            for (NSInteger i = 0; i < [(NSSegmentedControl *)control segmentCount]; i++) {
                [phrases addObject:[(NSSegmentedControl *)control labelForSegment:i]];
            }
            break;

        case kPreferenceInfoTypeRadioButton: {
            for (NSView *subview in control.subviews) {
                NSButton *button = [NSButton castFrom:subview];
                if (!button) {
                    continue;
                }
                NSString *title = button.title;
                if (title) {
                    [phrases addObject:title];
                }
            }
            break;
        }
    }
    NSTextField *relatedTextField = [NSTextField castFrom:relatedView];
    NSString *relatedPhrase = relatedTextField.stringValue;
    if (!relatedPhrase) {
        relatedPhrase = [NSButton castFrom:relatedView].title;
    }
    if (relatedPhrase) {
        relatedPhrase =
            [relatedPhrase stringByTrimmingCharactersInSet:[NSCharacterSet characterSetWithCharactersInString:@": "]];
        [phrases addObject:relatedPhrase];
        displayName = relatedPhrase;
    }
    displayName =
        [displayName stringByTrimmingCharactersInSet:[NSCharacterSet characterSetWithCharactersInString:@": "]];

    return displayName;
}

- (void)updateControlForKey:(NSString *)key {
    [self updateValueForInfo:[self infoForKey:key]];
}

- (void)updateValueForInfo:(PreferenceInfo *)info {
    if (_disableUpdates) {
        return;
    }
    if (info.onUpdate) {
        if (info.onUpdate()) {
            [self updateNonDefaultIndicatorVisibleForInfo:info];
            return;
        }
    }
    switch (info.type) {
        case kPreferenceInfoTypeCheckbox: {
            // BUG-f1090: Replace assert with guard to prevent crash on type mismatch
            NSButton *button = [NSButton castFrom:info.control];
            if (!button) {
                DLog(@"Warning: Expected NSButton for kPreferenceInfoTypeCheckbox but got %@", [info.control class]);
                break;
            }
            button.state = [self boolForKey:info.key] ? NSControlStateValueOn : NSControlStateValueOff;
            break;
        }

        case kPreferenceInfoTypeInvertedCheckbox: {
            // BUG-f1091: Replace assert with guard to prevent crash on type mismatch
            NSButton *button = [NSButton castFrom:info.control];
            if (!button) {
                DLog(@"Warning: Expected NSButton for kPreferenceInfoTypeInvertedCheckbox but got %@",
                     [info.control class]);
                break;
            }
            button.state = [self boolForKey:info.key] ? NSControlStateValueOff : NSControlStateValueOn;
            break;
        }

        case kPreferenceInfoTypeIntegerTextField: {
            // BUG-f1092: Replace assert with guard to prevent crash on type mismatch
            NSTextField *field = [NSTextField castFrom:info.control];
            if (!field) {
                DLog(@"Warning: Expected NSTextField for kPreferenceInfoTypeIntegerTextField but got %@",
                     [info.control class]);
                break;
            }
            field.intValue = [self intForKey:info.key];
            break;
        }

        case kPreferenceInfoTypeUnsignedIntegerTextField: {
            // BUG-f1093: Replace assert with guard to prevent crash on type mismatch
            NSTextField *field = [NSTextField castFrom:info.control];
            if (!field) {
                DLog(@"Warning: Expected NSTextField for kPreferenceInfoTypeUnsignedIntegerTextField but got %@",
                     [info.control class]);
                break;
            }
            field.stringValue = [NSString stringWithFormat:@"%lu", [self unsignedIntegerForKey:info.key]];
        }

        case kPreferenceInfoTypeDoubleTextField: {
            // BUG-f1094: Replace assert with guard to prevent crash on type mismatch
            NSTextField *field = [NSTextField castFrom:info.control];
            if (!field) {
                DLog(@"Warning: Expected NSTextField for kPreferenceInfoTypeDoubleTextField but got %@",
                     [info.control class]);
                break;
            }
            field.doubleValue = [self doubleForKey:info.key];
            break;
        }

        case kPreferenceInfoTypePasswordTextField:
        case kPreferenceInfoTypeStringTextField: {
            // BUG-f1095: Replace assert with guard to prevent crash on type mismatch
            NSTextField *field = [NSTextField castFrom:info.control];
            if (!field) {
                DLog(@"Warning: Expected NSTextField for kPreferenceInfoTypeStringTextField but got %@",
                     [info.control class]);
                break;
            }
            field.stringValue = [self stringForKey:info.key] ?: @"";
            break;
        }

        case kPreferenceInfoTypeStringTextView: {
            // BUG-f1096: Replace assert with guard to prevent crash on type mismatch
            NSTextView *textView = [NSTextView castFrom:info.control];
            if (!textView) {
                DLog(@"Warning: Expected NSTextView for kPreferenceInfoTypeStringTextView but got %@",
                     [info.control class]);
                break;
            }
            NSString *string = [self stringForKey:info.key] ?: @"";
            if (![string isEqualToString:textView.textStorage.string]) {
                [textView.textStorage
                    setAttributedString:[NSAttributedString attributedStringWithString:string
                                                                            attributes:textView.typingAttributes]];
            }
            break;
        }

        case kPreferenceInfoTypeTokenField: {
            // BUG-f1097: Replace assert with guard to prevent crash on type mismatch
            NSTokenField *field = [NSTokenField castFrom:info.control];
            if (!field) {
                DLog(@"Warning: Expected NSTokenField for kPreferenceInfoTypeTokenField but got %@",
                     [info.control class]);
                break;
            }
            NSObject *object = [self objectForKey:info.key];
            // BUG-f1098: Replace assert with guard - skip assignment if object doesn't conform to NSCopying
            if (object && ![object conformsToProtocol:@protocol(NSCopying)]) {
                DLog(@"Warning: Object for key '%@' does not conform to NSCopying, skipping update", info.key);
                break;
            }
            field.objectValue = (id<NSCopying>)object;
            break;
        }

        case kPreferenceInfoTypePopup: {
            // BUG-f1099: Replace assert with guard to prevent crash on type mismatch
            NSPopUpButton *popup = [NSPopUpButton castFrom:info.control];
            if (!popup) {
                DLog(@"Warning: Expected NSPopUpButton for kPreferenceInfoTypePopup but got %@", [info.control class]);
                break;
            }
            [popup selectItemWithTag:[self intForKey:info.key]];
            break;
        }

        case kPreferenceInfoTypeSegmentedControl: {
            // BUG-f1100: Replace assert with guard to prevent crash on type mismatch
            NSSegmentedControl *control = [NSSegmentedControl castFrom:info.control];
            if (!control) {
                DLog(@"Warning: Expected NSSegmentedControl for kPreferenceInfoTypeSegmentedControl but got %@",
                     [info.control class]);
                break;
            }
            [control selectSegmentWithTag:[self intForKey:info.key]];
            break;
        }
        case kPreferenceInfoTypeStringPopup: {
            // BUG-f1101: Replace assert with guard to prevent crash on type mismatch
            NSPopUpButton *popup = [NSPopUpButton castFrom:info.control];
            if (!popup) {
                DLog(@"Warning: Expected NSPopUpButton for kPreferenceInfoTypeStringPopup but got %@",
                     [info.control class]);
                break;
            }
            id obj = [self objectForKey:info.key];
            if (obj) {
                const NSInteger i = [popup indexOfItemWithRepresentedObject:[self objectForKey:info.key]];
                if (i != NSNotFound) {
                    [popup selectItemAtIndex:i];
                }
            }
            break;
        }

        case kPreferenceInfoTypeUnsignedIntegerPopup: {
            // BUG-f1102: Replace assert with guard to prevent crash on type mismatch
            NSPopUpButton *popup = [NSPopUpButton castFrom:info.control];
            if (!popup) {
                DLog(@"Warning: Expected NSPopUpButton for kPreferenceInfoTypeUnsignedIntegerPopup but got %@",
                     [info.control class]);
                break;
            }
            const NSUInteger value = [self unsignedIntegerForKey:info.key];
            // BUG-f1103: Replace assert with clamp - prevent overflow when converting to tag
            NSInteger safeTag = (value > NSIntegerMax) ? NSIntegerMax : (NSInteger)value;
            [popup selectItemWithTag:safeTag];
            break;
        }

        case kPreferenceInfoTypeSlider: {
            // BUG-f1104: Replace assert with guard to prevent crash on type mismatch
            NSSlider *slider = [NSSlider castFrom:info.control];
            if (!slider) {
                DLog(@"Warning: Expected NSSlider for kPreferenceInfoTypeSlider but got %@", [info.control class]);
                break;
            }
            slider.doubleValue = [self floatForKey:info.key];
            break;
        }

        case kPreferenceInfoTypeRadioButton: {
            // BUG-f1105: Replace assert with guard to prevent crash on type mismatch
            NSView *container = [NSView castFrom:info.control];
            if (!container) {
                DLog(@"Warning: Expected NSView for kPreferenceInfoTypeRadioButton but got %@", [info.control class]);
                break;
            }
            id obj = [self objectForKey:info.key];
            if (obj) {
                const NSInteger tag = [self integerForKey:info.key];
                for (NSView *subview in container.subviews) {
                    NSButton *radioButton = [NSButton castFrom:subview];
                    if (!radioButton) {
                        continue;
                    }
                    if (radioButton.tag == tag) {
                        radioButton.state = NSControlStateValueOn;
                    } else {
                        radioButton.state = NSControlStateValueOff;
                    }
                }
            }
            break;
        }

        case kPreferenceInfoTypeMatrix:
            // BUG-430: Replace assert(false) with DLog - Matrix type must use onChange() callback only
            DLog(@"Warning: kPreferenceInfoTypeMatrix must use onChange() only for key '%@'. No update performed.",
                 info.key);
            break;

        case kPreferenceInfoTypeColorWell: {
            // BUG-f1106: Replace assert with guard to prevent crash on type mismatch
            CPKColorWell *colorWell = [CPKColorWell castFrom:info.control];
            if (!colorWell) {
                DLog(@"Warning: Expected CPKColorWell for kPreferenceInfoTypeColorWell but got %@",
                     [info.control class]);
                break;
            }
            NSDictionary *dict = [NSDictionary castFrom:[self objectForKey:info.key]];
            // BUG-f1107: Replace assert with guard - only set color if dict is valid
            if (dict) {
                [colorWell setColor:[dict colorValue]];
            }
            break;
        }

        default:
            // BUG-431: Replace assert(false) with ELog - unknown preference type in updateValueForInfo
            ELog(@"Unknown preference info type %d for key '%@' in updateValueForInfo. No update performed.",
                 (int)info.type, info.key);
            break;
    }
    if (info.observer) {
        info.observer();
    }
    [self updateNonDefaultIndicatorVisibleForInfo:info];
    [self updateEnabledStateForInfo:info];
}

- (void)updateNonDefaultIndicatorVisibleForInfo:(PreferenceInfo *)info {
    const BOOL hasDefaultValue =
        info.hasDefaultValue != nil ? info.hasDefaultValue() : [self valueOfKeyEqualsDefaultValue:info.key];
    NSView *view = [NSView castFrom:info.control];
    if ([view.superview isKindOfClass:[iTermSlider class]]) {
        view = view.superview;
    }
    view.it_showNonDefaultIndicator =
        [iTermPreferences boolForKey:kPreferenceKeyIndicateNonDefaultValues] && !hasDefaultValue;
}

- (void)updateEnabledState {
    for (NSView *control in _keyMap) {
        PreferenceInfo *info = [self infoForControl:control];
        [self updateEnabledStateForInfo:info];
    }
}

- (void)updateEnabledStateForInfo:(PreferenceInfo *)info {
    if (info.shouldBeEnabled) {
        if ([info.control respondsToSelector:@selector(setEnabled:)]) {
            [(id)info.control setEnabled:info.shouldBeEnabled()];
        }
    }
}

- (iTermProfilePreferenceObserver *)profileObserver {
    return nil;
}

- (ProfileModel *)profileModel {
    return nil;
}

- (PreferenceInfo *)infoForControl:(NSView *)control {
    PreferenceInfo *info = [self safeInfoForControl:control];
    // BUG-f1113: Replace assert with guard and return nil for unknown controls
    if (!info) {
        DLog(@"Warning: No preference info found for control %@", control);
    }
    return info;
}

- (PreferenceInfo *)safeInfoForControl:(NSView *)control {
    PreferenceInfo *info = [_keyMap objectForKey:control];
    if (!info && control.superview) {
        info = [_keyMap objectForKey:control.superview];
        if (info && info.type == kPreferenceInfoTypeRadioButton) {
            return info;
        } else {
            return nil;
        }
    }
    return info;
}

- (void)associateStepper:(NSStepper *)stepper withPreference:(PreferenceInfo *)info {
    stepper.integerValue = [self integerForKey:info.key];
    info.associatedStepper = stepper;
    [_keyMap setObject:info forKey:stepper];
}

- (void)postRefreshNotification {
    [[NSNotificationCenter defaultCenter] postNotificationName:kRefreshTerminalNotification object:nil userInfo:nil];
}

- (NSArray<NSString *> *)keysForBulkCopy {
    return [_keys allObjects];
}

- (void)windowWillClose {
    // Documented as doing nothing.
}

- (void)willDeselectTab {
    // Documented as doing nothing.
}

- (void)commitControls {
    for (id control in _keyMap) {
        PreferenceInfo *info = [_keyMap objectForKey:control];
        if (!info) {
            continue;
        }
        NSString *key = info.key;
        BOOL needsUpdate = NO;
        if (info.syntheticGetter) {
            [_keysWithSyntheticGetters addObject:key];
            needsUpdate = YES;
        }
        if (info.syntheticSetter) {
            [_keysWithSyntheticSetters addObject:key];
            needsUpdate = YES;
        }
        if (needsUpdate) {
            [self updateValueForInfo:info];
        }
    }
}

- (NSArray<iTermSetting *> *)allSettingsWithPathComponents:(NSArray<NSString *> *)pathComponents {
    NSDictionary<NSArray<NSString *> *, NSArray<NSView *> *> *dict =
        [self.view descendantsTabviewTreeWithBelongingToKeysIn:_keyMap];
    NSMutableArray<iTermSetting *> *settings =
        [[NSMutableArray alloc] initWithCapacity:dict.count]; // One setting per view
    const BOOL isProfile = [self isKindOfClass:[iTermProfilePreferencesBaseViewController class]];
    [dict enumerateKeysAndObjectsUsingBlock:^(NSArray<NSString *> *components, NSArray<NSView *> *views,
                                              BOOL *_Nonnull stop) {
        for (NSView *control in views) {
            PreferenceInfo *info = [_keyMap objectForKey:control];
            [settings addObject:[[iTermSetting alloc]
                                      initWithInfo:info
                                         isProfile:isProfile
                                    pathComponents:[pathComponents arrayByAddingObjectsFromArray:components]]];
        }
    }];
    return settings;
}

- (BOOL)hasControlWithKey:(NSString *)key {
    return [_keys containsObject:key];
}

- (BOOL)tryToggleControlWithKey:(NSString *)key {
    [self view];
    PreferenceInfo *info = [self infoForKey:key];
    if (info.type != kPreferenceInfoTypeCheckbox && info.type != kPreferenceInfoTypeInvertedCheckbox) {
        return NO;
    }
    NSButton *button = [NSButton castFrom:info.control];
    if (!button) {
        return NO;
    }
    const NSControlStateValue valueBefore = button.state;
    [button performClick:nil];
    return valueBefore != button.state;
}

#pragma mark - Private

- (void)saveDeferredUpdates {
    [_keys enumerateObjectsUsingBlock:^(id _Nonnull key, BOOL *_Nonnull stop) {
        PreferenceInfo *info = [self infoForKey:key];
        if (info.deferUpdate) {
            switch (info.type) {
                case kPreferenceInfoTypeIntegerTextField:
                    [self setInt:[[NSTextField castFrom:info.control] separatorTolerantIntValue] forKey:key];
                    break;

                case kPreferenceInfoTypeCheckbox:
                case kPreferenceInfoTypeInvertedCheckbox:
                case kPreferenceInfoTypeUnsignedIntegerTextField:
                case kPreferenceInfoTypeDoubleTextField:
                case kPreferenceInfoTypeStringTextField:
                case kPreferenceInfoTypeStringTextView:
                case kPreferenceInfoTypePasswordTextField:
                case kPreferenceInfoTypePopup:
                case kPreferenceInfoTypeSegmentedControl:
                case kPreferenceInfoTypeStringPopup:
                case kPreferenceInfoTypeUnsignedIntegerPopup:
                case kPreferenceInfoTypeSlider:
                case kPreferenceInfoTypeTokenField:
                case kPreferenceInfoTypeMatrix:
                case kPreferenceInfoTypeColorWell:
                case kPreferenceInfoTypeRadioButton:
                    break;
            }
        }
    }];
}
#pragma mark - Constraints

// Pick out the digits from s and clamp it to a range.
- (int)intForString:(NSString *)s inRange:(NSRange)range {
    NSString *i = [s stringWithOnlyDigits];

    int val = 0;
    if ([i length]) {
        val = [i intValue];
    }
    val = MAX(val, range.location);
    val = MIN(val, range.location + range.length - 1);
    return val;
}

- (NSUInteger)unsignedIntegerForString:(NSString *)s inRange:(NSRange)range {
    NSString *i = [s stringWithOnlyDigits];

    NSUInteger val = 0;
    if ([i length]) {
        val = [i iterm_unsignedIntegerValue];
    }
    val = MAX(val, range.location);
    val = MIN(val, range.location + range.length - 1);
    return val;
}

- (int)applyIntegerConstraints:(PreferenceInfo *)info {
    // NSNumberFormatter seems to have lost its mind on Lion. See a description of the problem here:
    // http://stackoverflow.com/questions/7976951/nsnumberformatter-erasing-value-when-it-violates-constraints
    // BUG-f1108: Replace assert with guard to prevent crash on type mismatch
    NSTextField *textField = [NSTextField castFrom:info.control];
    if (!textField) {
        DLog(@"Warning: Expected NSTextField in applyIntegerConstraints but got %@", [info.control class]);
        return 0;
    }
    const int original = [self intForString:[textField stringValue] inRange:NSMakeRange(0, NSUIntegerMax)];
    int iv = [self intForString:[textField stringValue] inRange:info.range];
    if (iv != [textField separatorTolerantIntValue]) {
        // If the int values don't match up or there are terminal non-number
        // chars, then update the value.
        [textField setIntValue:iv];
    }
    return original;
}

- (void)applyUnsignedIntegerConstraints:(PreferenceInfo *)info {
    // BUG-f1109: Replace assert with guard to prevent crash on type mismatch
    NSTextField *textField = [NSTextField castFrom:info.control];
    if (!textField) {
        DLog(@"Warning: Expected NSTextField in applyUnsignedIntegerConstraints but got %@", [info.control class]);
        return;
    }
    NSUInteger iv = [self unsignedIntegerForString:[textField stringValue] inRange:info.range];
    if (iv != [textField separatorTolerantUnsignedIntegerValue]) {
        [textField setStringValue:[NSString stringWithFormat:@"%lu", iv]];
    }
}

#pragma mark - NSTextViewDelegate

- (void)textDidChange:(NSNotification *)notification {
    [self controlTextDidChange:notification];
}

- (void)textDidEndEditing:(NSNotification *)notification {
    [self controlTextDidEndEditing:notification];
}

#pragma mark - NSControlTextEditingDelegate

// This is a notification signature but it gets called because we're the delegate of text fields.
- (void)controlTextDidChange:(NSNotification *)aNotification {
    id control = [aNotification object];
    PreferenceInfo *info = [_keyMap objectForKey:control];
    DLog(@"Control text did change for control %@, info.key=%@", control, info.key);
    if (info) {
        [self settingChanged:control];
    }
}

- (void)controlTextDidEndEditing:(NSNotification *)aNotification {
    id control = [aNotification object];
    PreferenceInfo *info = [_keyMap objectForKey:control];
    if (info.controlTextDidEndEditing) {
        info.controlTextDidEndEditing(aNotification);
    } else {
        switch (info.type) {
            case kPreferenceInfoTypeCheckbox:
            case kPreferenceInfoTypeColorWell:
            case kPreferenceInfoTypeInvertedCheckbox:
            case kPreferenceInfoTypeMatrix:
            case kPreferenceInfoTypeRadioButton:
            case kPreferenceInfoTypePopup:
            case kPreferenceInfoTypeSegmentedControl:
            case kPreferenceInfoTypeStringPopup:
            case kPreferenceInfoTypeSlider:
            case kPreferenceInfoTypeStringTextField:
            case kPreferenceInfoTypeStringTextView:
            case kPreferenceInfoTypePasswordTextField:
            case kPreferenceInfoTypeTokenField:
            case kPreferenceInfoTypeUnsignedIntegerPopup:
            case kPreferenceInfoTypeUnsignedIntegerTextField:
                break;

            case kPreferenceInfoTypeIntegerTextField:
                if (info.deferUpdate) {
                    const int intValue = [[NSTextField castFrom:info.control] separatorTolerantIntValue];
                    [self setInt:intValue forKey:info.key];
                }
                break;

            case kPreferenceInfoTypeDoubleTextField:
                // Replace the control with its canonical value. Only floating point text fields can
                // temporarily take illegal values, which are tolerated until editing ends.
                // See the comments in -settingChanged: for more details on why doubles are special.
                [control setDoubleValue:[self doubleForKey:info.key]];
                break;
        }
    }
}

#pragma mark - Notifications

- (void)didToggleIndicateNonDefaultValues:(NSNotification *)notification {
    [self updateNonDefaultIndicators];
}

- (void)updateNonDefaultIndicators {
    for (NSView *key in _keyMap.keyEnumerator) {
        if (!key) {
            continue;
        }
        PreferenceInfo *info = [_keyMap objectForKey:key];
        if (info) {
            [self updateValueForInfo:info];
        }
    }
}

- (void)preferenceDidChangeFromOtherPanel:(NSNotification *)notification {
    NSString *key = notification.userInfo[kPreferenceDidChangeFromOtherPanelKeyUserInfoKey];
    if (![_keys containsObject:key]) {
        return;
    }
    PreferenceInfo *info = [self infoForKey:key];
    // BUG-f1114: Replace assert with guard - skip update if info not found
    if (!info) {
        DLog(@"Warning: No preference info found for key '%@' in preferenceDidChangeFromOtherPanel", key);
        return;
    }
    if (!info.deferUpdate) {
        [self updateValueForInfo:info];
    }
}

- (void)preferencePanelWillClose:(NSNotification *)notification {
    if (_preferencePanel == notification.object) {
        [self saveDeferredUpdates];
        // Give subclasses a chance to do something first.
        [self windowWillClose];

        dispatch_async(dispatch_get_main_queue(), ^{
            [self->_keyMap removeAllObjects];
        });
    }
}

- (NSTabView *)tabView {
    return nil;
}

- (CGFloat)minimumWidth {
    return 0;
}

- (void)resizeWindowForCurrentTabAnimated:(BOOL)animated {
    if (!self.tabView) {
        return;
    }
    [self resizeWindowForTabViewItem:self.tabView.selectedTabViewItem animated:animated];
}

- (void)resizeWindowForTabViewItem:(NSTabViewItem *)tabViewItem animated:(BOOL)animated {
    NSView *theView = tabViewItem.view.subviews.firstObject;
    [self resizeWindowForView:theView tabView:self.tabView animated:animated];
}

- (NSRect)windowFrameForTabViewSize:(NSSize)tabViewSize tabView:(NSTabView *)tabView {
    NSWindow *window = self.view.window;
    NSView *contentView = window.contentView;
    const CGFloat left = NSMinX([tabView convertRect:tabView.bounds toView:contentView]);
    const CGFloat right = NSWidth(contentView.frame) - NSMaxX([tabView convertRect:tabView.bounds toView:contentView]);
    const CGFloat below = NSMinY([tabView convertRect:tabView.bounds toView:contentView]);
    const CGFloat above = NSHeight(contentView.frame) - NSMaxY([tabView convertRect:tabView.bounds toView:contentView]);
    const NSRect contentViewFrame =
        NSMakeRect(0, 0, left + tabViewSize.width + right, below + tabViewSize.height + above);
    const NSSize windowSize = [window frameRectForContentRect:contentViewFrame].size;
    const NSPoint windowTopLeft = NSMakePoint(NSMinX(window.frame), NSMaxY(window.frame));
    return NSMakeRect(windowTopLeft.x, windowTopLeft.y - windowSize.height, windowSize.width, windowSize.height);
}

- (void)resizeWindowForView:(NSView *)theView tabView:(NSTabView *)tabView animated:(BOOL)animated {
    const CGFloat minimumWidthForOuterTabView = 644; // Reserve space for general, appearance, profiles, etc. buttons
    const CGFloat kTabViewMinWidth = MAX(minimumWidthForOuterTabView, self.minimumWidth);
    const CGFloat inset = NSWidth(tabView.bounds) - NSWidth(theView.superview.bounds);
    const CGFloat bottomMargin = 36;
    CGSize tabViewSize =
        NSMakeSize(MAX(kTabViewMinWidth, theView.bounds.size.width) + inset, theView.bounds.size.height + bottomMargin);
    NSRect frame = [self windowFrameForTabViewSize:tabViewSize tabView:tabView];
    frame.size.width =
        MAX(self.preferencePanel.preferencePanelMinimumWidth ?: iTermPreferencePanelGetWindowMinimumWidth(NO),
            frame.size.width);
    if (NSEqualRects(_desiredFrame, frame)) {
        return;
    }
    NSWindow *window = self.view.window;
    if (!animated) {
        _desiredFrame = NSZeroRect;
        [window setFrame:frame display:YES];
        return;
    }

    // Animated
    _desiredFrame = frame;
    NSTimeInterval duration = [window animationResizeTime:frame];
    [NSAnimationContext
        runAnimationGroup:^(NSAnimationContext *context) {
            context.duration = duration;
            [window.animator setFrame:frame display:YES];
        }
        completionHandler:^{
            self->_desiredFrame = NSZeroRect;
            NSRect rect = frame;
            [window setFrame:rect display:YES];
        }];
}

#pragma mark - NSTabViewDelegate

- (void)tabView:(NSTabView *)tabView didSelectTabViewItem:(NSTabViewItem *)tabViewItem {
    // BUG-f1115: Replace assert with guard - skip resize if tabView is nil
    if (!self.tabView) {
        DLog(@"Warning: tabView:didSelectTabViewItem: called but self.tabView is nil");
        return;
    }
    [self resizeWindowForTabViewItem:tabViewItem animated:YES];
}

#pragma mark - iTermSearchableViewController

- (NSString *)documentOwnerIdentifier {
    return NSStringFromClass(self.class);
}

- (NSArray<iTermPreferencesSearchDocument *> *)searchableViewControllerDocuments {
    return _docs;
}

- (BOOL)revealControl:(NSView *)control {
    // Is there an inner tab container view?
    for (NSTabViewItem *item in self.tabView.tabViewItems) {
        NSView *view = item.view;
        if ([view containsDescendant:control]) {
            if (self.tabView.selectedTabViewItem != item) {
                [self.tabView selectTabViewItem:item];
                return YES;
            } else {
                return NO;
            }
        }
    }
    return NO;
}

- (NSView *)searchableViewControllerRevealItemForDocument:(iTermPreferencesSearchDocument *)document
                                                 forQuery:(NSString *)query
                                            willChangeTab:(BOOL *)willChangeTab {
    *willChangeTab = NO;
    NSString *key = document.identifier;
    for (NSView *control in _keyMap) {
        if ([control isKindOfClass:[NSStepper class]]) {
            continue;
        }
        PreferenceInfo *info = [_keyMap objectForKey:control];
        if ([key isEqualToString:info.key]) {
            *willChangeTab = [self revealControl:control];
            return control;
        }
    }
    for (NSView *control in _otherSearchableViews) {
        if ([control.accessibilityIdentifier isEqualToString:document.identifier]) {
            *willChangeTab = [self revealControl:control];
            return control;
        }
    }
    return nil;
}

@end
