//
//  iTermSecureKeyboardEntryController.m
//  DashTerm2
//
//  Created by George Nachman on 02/05/19.
//

#import "iTermSecureKeyboardEntryController.h"

#import "DebugLogging.h"
#import "iTermAdvancedSettingsModel.h"
#import "iTermUserDefaults.h"
#import "iTermWarning.h"

#import <Carbon/Carbon.h>

NSString *const iTermDidToggleSecureInputNotification = @"iTermDidToggleSecureInputNotification";

@implementation iTermSecureKeyboardEntryController {
    int _count;
    BOOL _focusStolen;
    BOOL _temporarilyDisabled;
    NSTimer *_backstop;
    BOOL _warningShown;
}

+ (instancetype)sharedInstance {
    static id instance;
    static dispatch_once_t onceToken;
    dispatch_once(&onceToken, ^{
        instance = [[self alloc] init];
    });
    return instance;
}

- (instancetype)init {
    self = [super init];
    if (self) {
        _enabledByUserDefault = iTermUserDefaults.secureKeyboardEntry;

        [[NSNotificationCenter defaultCenter] addObserver:self
                                                 selector:@selector(applicationDidResignActive:)
                                                     name:NSApplicationDidResignActiveNotification
                                                   object:nil];
        [[NSNotificationCenter defaultCenter] addObserver:self
                                                 selector:@selector(applicationDidBecomeActive:)
                                                     name:NSApplicationDidBecomeActiveNotification
                                                   object:nil];
        if ([NSApp isActive]) {
            [self update];
        }
    }
    return self;
}

#pragma mark - API

- (void)toggle {
    // Set _desired to the opposite of the current state.
    _enabledByUserDefault = !_enabledByUserDefault;
    DLog(@"toggle called. Setting desired to %@", @(_enabledByUserDefault));

    // Try to set the system's state of secure input to the desired state.
    [self update];

    // Save the preference, independent of whether it succeeded or not.
    iTermUserDefaults.secureKeyboardEntry = _enabledByUserDefault;
}

- (void)didStealFocus {
    _focusStolen = YES;
    [self update];
}

- (void)didReleaseFocus {
    _focusStolen = NO;
    [self update];
}

- (BOOL)isEnabled {
    return !!IsSecureEventInputEnabled();
}

- (void)setDesired:(BOOL)desired {
    _enabledByUserDefault = desired;
}

- (BOOL)isDesired {
    if (_temporarilyDisabled) {
        return NO;
    }
    return _enabledByUserDefault || [self currentSessionAtPasswordPrompt];
}

- (void)disableUntilDeactivated {
    DLog(@"disableUntilDeactivated");
    if (_backstop) {
        DLog(@"Already have a backstop");
        return;
    }
    DLog(@"Set flag");
    _temporarilyDisabled = YES;
    _backstop = [NSTimer scheduledTimerWithTimeInterval:1 repeats:NO block:^(NSTimer * _Nonnull timer) {
        [[iTermSecureKeyboardEntryController sharedInstance] temporaryDisablementDidExpire];
    }];
    [self update];
}

- (void)temporaryDisablementDidExpire {
    DLog(@"temporaryDisablementDidExpire _temporarilyDisabled=%@", @(_temporarilyDisabled));
    _backstop = nil;
    _temporarilyDisabled = NO;
    [self update];
}

#pragma mark - Notifications

- (void)applicationDidResignActive:(NSNotification *)notification {
    _temporarilyDisabled = NO;
    [_backstop invalidate];
    _backstop = nil;
    if (_count > 0) {
        DLog(@"Application resigning active.");
        [self update];
    }
}

- (void)applicationDidBecomeActive:(NSNotification *)notification {
    if (self.isDesired) {
        DLog(@"Application became active.");
        [self update];
    }
}

#pragma mark - Private

- (BOOL)currentSessionAtPasswordPrompt {
    if (![iTermAdvancedSettingsModel enableSecureKeyboardEntryAutomatically]) {
        return NO;
    }
    NSResponder *firstResponder = [[NSApp keyWindow] firstResponder];
    if (![firstResponder conformsToProtocol:@protocol(iTermSecureInputRequesting)]) {
        return NO;
    }
    id<iTermSecureInputRequesting> requesting = (id<iTermSecureInputRequesting>)firstResponder;
    const BOOL result = [requesting isRequestingSecureInput];
    DLog(@"Current session at password prompt=%@", @(result));
    return result;
}

- (BOOL)allowed {
    if ([NSApp isActive]) {
        return YES;
    }
    return _focusStolen;
}

- (void)update {
    DLog(@"Update secure keyboard entry. desired=%@ active=%@ focusStolen=%@",
         @(self.isDesired), @([NSApp isActive]), @(_focusStolen));
    const BOOL secure = self.isDesired && [self allowed];

    if (secure && _count > 0) {
        DLog(@"Want to turn on secure input but it's already on");
        return;
    }

    if (!secure && _count == 0) {
        DLog(@"Want to turn off secure input but it's already off");
        return;
    }

    DLog(@"Before: IsSecureEventInputEnabled returns %d", (int)self.isEnabled);
    if (secure) {
        if (![self currentSessionAtPasswordPrompt]) {
            [self warnIfNeeded];
        }
        OSErr err = EnableSecureEventInput();
        DLog(@"EnableSecureEventInput err=%d", (int)err);
        if (err) {
            XLog(@"EnableSecureEventInput failed with error %d", (int)err);
        } else {
            DLog(@"Secure keyboard entry enabled");
            _count += 1;
        }
    } else {
        OSErr err = DisableSecureEventInput();
        DLog(@"DisableSecureEventInput err=%d", (int)err);
        if (err) {
            XLog(@"DisableSecureEventInput failed with error %d", (int)err);
        } else {
            DLog(@"Secure keyboard entry disabled");
            _count -= 1;
        }
    }
    [[NSNotificationCenter defaultCenter] postNotificationName:iTermDidToggleSecureInputNotification object:nil];
    DLog(@"After: IsSecureEventInputEnabled returns %d", (int)self.isEnabled);
}

- (void)warnIfNeeded {
    if (@available(macOS 12.0, *)) {
        // This prevents reentrancy. If called during -windowDidBecomeKey showing the warning
        // causes the window to resign key in the same stack which crashes.
        [self performSelector:@selector(showMontereyWarning) withObject:nil afterDelay:0];
    }
}

- (void)showMontereyWarning NS_AVAILABLE_MAC(12_0) {
    // BUG-10846: Check and set _warningShown at the start to prevent reentrancy.
    // This method is called via performSelector:afterDelay:0 from warnIfNeeded,
    // and if multiple calls are queued before the first executes, they could all
    // pass the _warningShown check. By setting the flag first and only clearing
    // it if we don't actually show the warning, we prevent the crash loop.
    if (_warningShown) {
        return;
    }
    _warningShown = YES;

    if (!_enabledByUserDefault) {
        // Don't show warning, but keep _warningShown true to prevent future attempts
        // during this session since the user hasn't enabled it.
        _warningShown = NO;
        return;
    }
    if (![self isEnabled]) {
        // Secure input was disabled between warnIfNeeded and this call
        _warningShown = NO;
        return;
    }

    // Use the async variant to avoid blocking the run loop and causing reentrancy
    // issues when the modal dialog affects window focus events.
    [iTermWarning asyncShowWarningWithTitle:@"Secure keyboard entry is enabled.\n\nIn macOS 12 and later, enabling Secure Keyboard Entry prevents other programs from being activated. This affects the `open` command as well as the panel shown when using Touch ID for sudo."
                                    actions:@[ @"OK", @"Cancel" ]
                              actionMapping:nil
                                  accessory:nil
                                 identifier:@"NoSyncMontereySecureKeyboardEntryWarning"
                                silenceable:kiTermWarningTypePermanentlySilenceable
                                    heading:@"Secure Keyboard Entry Enabled"
                                cancelLabel:@"Cancel"
                                     window:[NSApp keyWindow]
                                 completion:^(iTermWarningSelection selection, iTermWarning *warning) {
        if (selection == kiTermWarningSelection1) {
            [self toggle];
        }
    }];
}

@end
