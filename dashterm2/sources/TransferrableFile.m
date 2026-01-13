//
//  TransferrableFile.m
//  iTerm
//
//  Created by George Nachman on 12/23/13.
//
//

#import "TransferrableFile.h"

#import "DebugLogging.h"
#import "NSFileManager+iTerm.h"
#import "iTermNotificationController.h"
#import "iTermWarning.h"
#import <os/lock.h>

@implementation TransferrableFile {
    NSTimeInterval _timeOfLastStatusChange;
    TransferrableFileStatus _status;
    TransferrableFile *_successor;
    os_unfair_lock _lock;
}

static NSMutableSet<NSString *> *iTermTransferrableFileLockedFileNames(void) {
    static NSMutableSet<NSString *> *locks;
    static dispatch_once_t onceToken;
    dispatch_once(&onceToken, ^{
        locks = [[NSMutableSet alloc] initWithCapacity:8];  // Locked file names
    });
    return locks;
}

+ (void)lockFileName:(NSString *)name {
    if (name) {
        [iTermTransferrableFileLockedFileNames() addObject:name];
    }
}

+ (void)unlockFileName:(NSString *)name {
    if (name) {
        [iTermTransferrableFileLockedFileNames() removeObject:name];
    }
}

+ (BOOL)fileNameIsLocked:(NSString *)name {
    if (!name) {
        return NO;
    }
    return [iTermTransferrableFileLockedFileNames() containsObject:name];
}

- (instancetype)init {
    self = [super init];
    if (self) {
        _status = kTransferrableFileStatusUnstarted;
        _fileSize = -1;
        _lock = OS_UNFAIR_LOCK_INIT;
    }
    return self;
}

// BUG-494 to BUG-498: Abstract methods - subclasses must override
// Using ELog + nil return instead of assert(false) to avoid crashing

- (NSString *)protocolName {
    // BUG-494: Replace assert(false) with ELog - abstract method must be overridden
    ELog(@"TransferrableFile subclass %@ must override protocolName", NSStringFromClass([self class]));
    return nil;
}

- (NSString *)authRequestor {
    // BUG-494: Replace assert(false) with ELog - abstract method must be overridden
    ELog(@"TransferrableFile subclass %@ must override authRequestor", NSStringFromClass([self class]));
    return nil;
}

- (NSString *)displayName {
    // BUG-495: Replace assert(false) with ELog - abstract method must be overridden
    ELog(@"TransferrableFile subclass %@ must override displayName", NSStringFromClass([self class]));
    return nil;
}

- (NSString *)shortName {
    // BUG-495: Replace assert(false) with ELog - abstract method must be overridden
    ELog(@"TransferrableFile subclass %@ must override shortName", NSStringFromClass([self class]));
    return nil;
}

- (NSString *)subheading {
    // BUG-496: Replace assert(false) with ELog - abstract method must be overridden
    ELog(@"TransferrableFile subclass %@ must override subheading", NSStringFromClass([self class]));
    return nil;
}

- (void)download {
    // BUG-496: Replace assert(false) with ELog - abstract method must be overridden
    ELog(@"TransferrableFile subclass %@ must override download", NSStringFromClass([self class]));
}

- (void)upload {
    // BUG-497: Replace assert(false) with ELog - abstract method must be overridden
    ELog(@"TransferrableFile subclass %@ must override upload", NSStringFromClass([self class]));
}

- (void)stop {
    // BUG-497: Replace assert(false) with ELog - abstract method must be overridden
    ELog(@"TransferrableFile subclass %@ must override stop", NSStringFromClass([self class]));
}

- (NSString *)localPath {
    // BUG-498: Replace assert(false) with ELog - abstract method must be overridden
    ELog(@"TransferrableFile subclass %@ must override localPath", NSStringFromClass([self class]));
    return nil;
}

- (NSString *)error {
    // BUG-498: Replace assert(false) with ELog - abstract method must be overridden
    ELog(@"TransferrableFile subclass %@ must override error", NSStringFromClass([self class]));
    return nil;
}

- (NSString *)destination {
    // BUG-498: Replace assert(false) with ELog - abstract method must be overridden
    ELog(@"TransferrableFile subclass %@ must override destination", NSStringFromClass([self class]));
    return nil;
}

- (BOOL)isDownloading {
    // BUG-498: Replace assert(false) with ELog - abstract method must be overridden
    ELog(@"TransferrableFile subclass %@ must override isDownloading", NSStringFromClass([self class]));
    return NO;
}

- (NSString *)finalDestinationForPath:(NSString *)originalBaseName
                 destinationDirectory:(NSString *)destinationDirectory
                               prompt:(BOOL)prompt {
    NSString *baseName = originalBaseName;
    if (self.isZipOfFolder) {
        baseName = [baseName stringByAppendingString:@".zip"];
    }
    NSString *name = baseName;
    NSString *finalDestination = nil;
    int retries = 0;
    do {
        finalDestination = [destinationDirectory stringByAppendingPathComponent:name];
        ++retries;
        NSRange rangeOfDot = [baseName rangeOfString:@"."];
        NSString *prefix = baseName;
        NSString *suffix = @"";
        if (rangeOfDot.length > 0) {
            prefix = [baseName substringToIndex:rangeOfDot.location];
            suffix = [baseName substringFromIndex:rangeOfDot.location];
        }
        name = [NSString stringWithFormat:@"%@ (%d)%@", prefix, retries, suffix];
    } while ([[NSFileManager defaultManager] fileExistsAtPath:finalDestination] ||
             [TransferrableFile fileNameIsLocked:finalDestination]);
    if (retries == 1 || !prompt) {
        return finalDestination;
    }
    NSString *message = [NSString
        stringWithFormat:@"A file named %@ already exists. Keep both files or replace the existing file?", baseName];
    const iTermWarningSelection selection = [iTermWarning showWarningWithTitle:message
                                                                       actions:@[ @"Keep Both", @"Replace" ]
                                                                     accessory:nil
                                                                    identifier:@"NoSyncOverwriteOrReplaceFile"
                                                                   silenceable:kiTermWarningTypePermanentlySilenceable
                                                                       heading:@"Overwrite existing file?"
                                                                        window:nil];
    if (selection == kiTermWarningSelection1) {
        return [destinationDirectory stringByAppendingPathComponent:baseName];
    }
    return finalDestination;
}

- (NSString *)downloadsDirectory {
    return [[NSFileManager defaultManager] downloadsDirectory] ?: NSHomeDirectory();
}

- (void)setSuccessor:(TransferrableFile *)successor {
    os_unfair_lock_lock(&_lock);
    [_successor autorelease];
    _successor = [successor retain];
    os_unfair_lock_unlock(&_lock);
    successor.hasPredecessor = YES;
}

- (TransferrableFile *)successor {
    os_unfair_lock_lock(&_lock);
    TransferrableFile *result = _successor;
    os_unfair_lock_unlock(&_lock);
    return result;
}

- (void)didFailWithError:(NSString *)error {
    DLog(@"didFailWithError:%@", error);
    os_unfair_lock_lock(&_lock);
    if (_status != kTransferrableFileStatusFinishedWithError) {
        _status = kTransferrableFileStatusFinishedWithError;
        _timeOfLastStatusChange = [NSDate timeIntervalSinceReferenceDate];
        os_unfair_lock_unlock(&_lock);
        [[iTermNotificationController sharedInstance] notify:error];
    } else {
        os_unfair_lock_unlock(&_lock);
    }
}

- (void)setStatus:(TransferrableFileStatus)status {
    DLog(@"setStatus:%@\n%@", @(status), [NSThread callStackSymbols]);
    BOOL shouldNotify = NO;
    TransferrableFileStatus notifyStatus = kTransferrableFileStatusUnstarted;
    os_unfair_lock_lock(&_lock);
    if (status != _status) {
        _status = status;
        _timeOfLastStatusChange = [NSDate timeIntervalSinceReferenceDate];
        if (status == kTransferrableFileStatusFinishedSuccessfully ||
            status == kTransferrableFileStatusFinishedWithError) {
            shouldNotify = YES;
            notifyStatus = status;
        }
    }
    os_unfair_lock_unlock(&_lock);
    // Notify outside the lock to minimize hold time
    if (shouldNotify) {
        if (notifyStatus == kTransferrableFileStatusFinishedSuccessfully) {
            [[iTermNotificationController sharedInstance]
                notify:[NSString stringWithFormat:@"%@ finished for \u201c%@\u201d.",
                                                  self.isDownloading ? @"Download" : @"Upload",
                                                  [self shortName]]];
        } else {
            [[iTermNotificationController sharedInstance]
                notify:[NSString stringWithFormat:@"%@ failed for \u201c%@\u201d.",
                                                  self.isDownloading ? @"Download" : @"Upload",
                                                  [self shortName]]];
        }
    }
}

- (TransferrableFileStatus)status {
    os_unfair_lock_lock(&_lock);
    TransferrableFileStatus result = _status;
    os_unfair_lock_unlock(&_lock);
    return result;
}

- (NSTimeInterval)timeOfLastStatusChange {
    return _timeOfLastStatusChange;
}

- (void)failedToRemoveUnquarantinedFileAt:(NSString *)path {
    [iTermWarning showWarningWithTitle:[NSString stringWithFormat:@"The file at “%@” could not be quarantined or "
                                                                  @"deleted! It is dangerous and should be removed.",
                                                                  path]
                               actions:@[ @"OK" ]
                             accessory:nil
                            identifier:nil
                           silenceable:kiTermWarningTypePersistent
                               heading:@"Danger!"
                                window:nil];
}

- (BOOL)quarantine:(NSString *)path sourceURL:(NSURL *)sourceURL {
    NSURL *url = [NSURL fileURLWithPath:path];

    NSMutableDictionary *properties = nil;
    {
        NSError *error = nil;
        NSDictionary *temp;
        const BOOL ok = [url getResourceValue:&temp forKey:NSURLQuarantinePropertiesKey error:&error];
        if (!ok) {
            XLog(@"Get quarantine of %@ failed: %@", path, error);
            return NO;
        }
        if (temp && ![temp isKindOfClass:[NSDictionary class]]) {
            XLog(@"Quarantine of wrong class: %@", NSStringFromClass([temp class]));
            return NO;
        }
        properties = [[temp ?: @{} mutableCopy] autorelease];
    }

    NSBundle *bundle = [NSBundle mainBundle];
    NSDictionary *info = bundle.infoDictionary;
    properties[(__bridge NSString *)kLSQuarantineAgentNameKey] =
        info[(__bridge NSString *)kCFBundleNameKey] ?: @"DashTerm2";
    properties[(__bridge NSString *)kLSQuarantineAgentBundleIdentifierKey] =
        info[(__bridge NSString *)kCFBundleIdentifierKey] ?: @"com.dashterm.dashterm2";
    if (sourceURL.absoluteString) {
        properties[(__bridge NSString *)kLSQuarantineDataURLKey] = sourceURL.absoluteString;
    }
    properties[(__bridge NSString *)kLSQuarantineTimeStampKey] = [NSDate date];
    properties[(__bridge NSString *)kLSQuarantineTypeKey] = (__bridge NSString *)kLSQuarantineTypeOtherDownload;

    {
        NSError *error = nil;
        const BOOL ok = [url setResourceValue:properties forKey:NSURLQuarantinePropertiesKey error:&error];
        if (!ok) {
            XLog(@"Set quarantine of %@ failed: %@", path, error);
            return NO;
        }
    }
    return YES;
}

@end
