//
//  iTermRestorableStateDriver.m
//  DashTerm2
//
//  Created by George Nachman on 7/28/20.
//

#import "iTermRestorableStateDriver.h"

#import "DebugLogging.h"
#import "iTermWarning.h"
#import "NSArray+iTerm.h"
#import "PTYWindow.h"

static NSString *const iTermRestorableStateControllerUserDefaultsKeyCount = @"NoSyncRestoreWindowsCount";

@implementation iTermRestorableStateDriver {
    BOOL _saving;
}

#pragma mark - Save

- (void)save {
    // BUG-f1169: Replace assert() with guard - off-main-thread call should be dispatched to main, not crash
    if (![NSThread isMainThread]) {
        DLog(@"WARNING: save called off main thread, dispatching to main");
        dispatch_async(dispatch_get_main_queue(), ^{
            [self save];
        });
        return;
    }
    [self saveSynchronously:NO];
}

- (void)saveSynchronously {
    // BUG-f1170: Replace assert() with guard - off-main-thread call should be dispatched to main, not crash
    if (![NSThread isMainThread]) {
        DLog(@"WARNING: saveSynchronously called off main thread, dispatching synchronously to main");
        dispatch_sync(dispatch_get_main_queue(), ^{
            [self saveSynchronously:YES];
        });
        return;
    }
    [self saveSynchronously:YES];
}

- (void)saveSynchronously:(BOOL)sync {
    // BUG-f1171: Replace assert() with guard - off-main-thread call should be dispatched to main, not crash
    if (![NSThread isMainThread]) {
        DLog(@"WARNING: saveSynchronously: called off main thread, dispatching to main");
        if (sync) {
            dispatch_sync(dispatch_get_main_queue(), ^{
                [self saveSynchronously:sync];
            });
        } else {
            dispatch_async(dispatch_get_main_queue(), ^{
                [self saveSynchronously:sync];
            });
        }
        return;
    }
    DLog(@"save sync=%@ saver=%@", @(sync), _saver);
    if (_saving) {
        DLog(@"Currently saving. Set needsSave.");
        _needsSave = YES;
        return;
    }
    __weak __typeof(self) weakSelf = self;
    const BOOL saved = [_saver saveSynchronously:sync withCompletion:^{
        [weakSelf didSave];
    }];
    // Do this after saveSynchronously:withCompletion:. It guarantees not to run its completion block
    // synchronously. It could fail if it was already busy saving, in which case we don't want
    // to reset _needsSave. Considering it is busy, the other guy will eventually finish and cause
    // didSave to be called, and it will try again.
    _needsSave = !saved;
}

// Main queue
- (void)didSave {
    // BUG-f1172: Replace assert() with guard - off-main-thread call should be dispatched to main, not crash
    if (![NSThread isMainThread]) {
        DLog(@"WARNING: didSave called off main thread, dispatching to main");
        dispatch_async(dispatch_get_main_queue(), ^{
            [self didSave];
        });
        return;
    }
    DLog(@"didSave");
    _saving = NO;
    if (_needsSave) {
        DLog(@"needsSave was YES");
        [self save];
    }
}

#pragma mark - Restore

- (void)restoreWithSystemCallbacks:(NSMutableDictionary<NSString *, void (^)(NSWindow *, NSError *)> *)callbacks
                             ready:(void (^)(void))ready
                        completion:(void (^)(void))completion {
    CFAbsoluteTime startTime = CFAbsoluteTimeGetCurrent();
    DLog(@"restoreWindows");
    NSLog(@"[STARTUP] iTermRestorableStateDriver.restoreWithSystemCallbacks starting");
    if (!self.restorer) {
        DLog(@"Have no restorer.");
        NSLog(@"[STARTUP] No restorer, calling ready+completion immediately");
        ready();
        completion();
        return;
    }
    __weak __typeof(self) weakSelf = self;
    DLog(@"Loading restorable state index…");
    NSLog(@"[STARTUP] Loading restorable state index...");
    [self.restorer loadRestorableStateIndexWithCompletion:^(id<iTermRestorableStateIndex> index) {
        CFAbsoluteTime indexLoadTime = CFAbsoluteTimeGetCurrent();
        NSLog(@"[STARTUP] Index loaded in %.3fs", indexLoadTime - startTime);
        [weakSelf restoreWithIndex:index callbacks:callbacks ready:ready completion:completion];
    }];
}

- (void)restoreWithIndex:(id<iTermRestorableStateIndex>)index
               callbacks:(NSMutableDictionary<NSString *, void (^)(NSWindow *, NSError *)> *)callbacks
                   ready:(void (^)(void))ready
              completion:(void (^)(void))completion {
    CFAbsoluteTime startTime = CFAbsoluteTimeGetCurrent();
    NSLog(@"[STARTUP] restoreWithIndex starting, window count=%lu", (unsigned long)[index restorableStateIndexNumberOfWindows]);
    DLog(@"Have an index. Proceeding to restore windows.");
    const NSInteger count = [[NSUserDefaults standardUserDefaults] integerForKey:iTermRestorableStateControllerUserDefaultsKeyCount];
    // Auto-restore silently: if previous restoration failed multiple times, skip restoration
    // but don't show a modal dialog - just open a new window later
    if (count > 3) {
        // After 3 failed attempts, give up on restoration silently
        DLog(@"Restoration failed %ld times, skipping silently", (long)count);
        NSLog(@"[STARTUP] Auto-restore: skipping after %ld failed attempts", (long)count);
        [index restorableStateIndexUnlink];
        [[NSUserDefaults standardUserDefaults] setInteger:0
                                                   forKey:iTermRestorableStateControllerUserDefaultsKeyCount];
        ready();
        completion();
        return;
    }
    [[NSUserDefaults standardUserDefaults] setInteger:count + 1
                                               forKey:iTermRestorableStateControllerUserDefaultsKeyCount];
    DLog(@"set restoring to YES");
    _restoring = YES;
    [self reallyRestoreWindows:index callbacks:callbacks withCompletion:^{
        [self didRestoreFromIndex:index];
        NSLog(@"[STARTUP] Window restoration completion called after %.3fs", CFAbsoluteTimeGetCurrent() - startTime);
        completion();
    }];
    DLog(@"Ready - normal case");
    NSLog(@"[STARTUP] Calling ready() callback after %.3fs", CFAbsoluteTimeGetCurrent() - startTime);
    ready();
}

// Main queue
- (void)didRestoreFromIndex:(id<iTermRestorableStateIndex>)index {
    DLog(@"set restoring to NO");
    _restoring = NO;
    [[NSUserDefaults standardUserDefaults] setInteger:0
                                               forKey:iTermRestorableStateControllerUserDefaultsKeyCount];
    [index restorableStateIndexUnlink];
}

// Main queue
- (void)reallyRestoreWindows:(id<iTermRestorableStateIndex>)index
                   callbacks:(NSMutableDictionary<NSString *, void (^)(NSWindow *, NSError *)> *)callbacks
              withCompletion:(void (^)(void))completion {
    CFAbsoluteTime startTime = CFAbsoluteTimeGetCurrent();
    NSLog(@"[STARTUP] reallyRestoreWindows starting");
    [self.restorer restoreApplicationState];
    NSLog(@"[STARTUP] restoreApplicationState took %.3fs", CFAbsoluteTimeGetCurrent() - startTime);

    const NSInteger count = [index restorableStateIndexNumberOfWindows];

    // When all windows have finished being restored, mark the restoration as a success.
    dispatch_group_t group = dispatch_group_create();
    dispatch_async(dispatch_get_main_queue(), ^{
        dispatch_group_notify(group, dispatch_get_main_queue(), ^{
            for (NSInteger i = 0; i < count; i++) {
                [[index restorableStateRecordAtIndex:i] didFinishRestoring];
            }
            completion();
        });
    });
    DLog(@"Restoring from index:\n%@", index);
    for (NSInteger i = 0; i < count; i++) {
        DLog(@"driver: restore window number %@", @(i));
        _numberOfWindowsRestored += 1;
        dispatch_group_enter(group);
        [self.restorer restoreWindowWithRecord:[index restorableStateRecordAtIndex:i]
                                    completion:^(NSString *windowIdentifier, NSWindow *window) {
            DLog(@"driver: restoration of window number %@ with identifier %@ finished", @(i), windowIdentifier);
            if (windowIdentifier) {
                void (^callback)(NSWindow *, NSError *) = callbacks[windowIdentifier];
                if (callback) {
                    DLog(@"Restorable state driver: Invoke callback callback with window %@", window);
                    [callbacks removeObjectForKey:windowIdentifier];
                    id<PTYWindow> ptyWindow = nil;
                    if ([window conformsToProtocol:@protocol(PTYWindow)]) {
                        ptyWindow = (id<PTYWindow>)window;
                    }
                    const BOOL saved = ptyWindow.it_preventFrameChange;
                    if (!(window.styleMask & NSWindowStyleMaskFullScreen)) {
                        ptyWindow.it_preventFrameChange = YES;
                    }
                    callback(window, nil);
                    ptyWindow.it_preventFrameChange = saved;

                    DLog(@"Restorable state driver: Returned from callback callback with window %@", window);
                } else {
                    DLog(@"No callback");
                }
            }
            dispatch_group_leave(group);
        }];
    }
}

#pragma mark - Erase

- (void)eraseSynchronously:(BOOL)sync {
    [self.restorer eraseStateRestorationDataSynchronously:sync];
}

@end
