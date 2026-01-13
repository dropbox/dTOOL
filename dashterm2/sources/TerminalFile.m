//
//  TerminalFile.m
//  iTerm
//
//  Created by George Nachman on 1/5/14.
//
//

#import "TerminalFile.h"

#import "DebugLogging.h"
#import "FileTransferManager.h"
#import "FutureMethods.h"
#import "NSSavePanel+iTerm.h"
#import "RegexKitLite.h"

#import <apr-1/apr_base64.h>
#import <os/lock.h>

// Performance optimization: os_unfair_lock has ~10x lower overhead than @synchronized

NSString *const kTerminalFileShouldStopNotification = @"kTerminalFileShouldStopNotification";

@interface TerminalFile () {
    os_unfair_lock _lock;
}
@property (nonatomic, strong) NSMutableString *data;
@property (nonatomic, copy) NSString *filename; // No path, just a name.
@property (nonatomic, strong) NSString *error;
@end

@implementation TerminalFileDownload

- (instancetype)initWithName:(NSString *)name size:(NSInteger)size {
    self = [super initWithName:name size:size];
    if (self) {
        if (self.localPath) {
            [TransferrableFile lockFileName:self.localPath];
        }
    }
    return self;
}

@end

@implementation TerminalFile

- (instancetype)initWithName:(NSString *)name size:(NSInteger)size {
    self = [super init];
    if (self) {
        _lock = OS_UNFAIR_LOCK_INIT;
        if (!name) {
            NSSavePanel *panel = [NSSavePanel savePanel];

            NSString *path = [self downloadsDirectory];
            if (path) {
                NSURL *url = [NSURL fileURLWithPath:path];
                [NSSavePanel setDirectoryURL:url onceForID:@"TerminalFile" savePanel:panel];
            }
            panel.nameFieldStringValue = @"";

            if ([panel runModal] == NSModalResponseOK) {
                _localPath = [panel.URL.path copy];
                _filename = [[_localPath lastPathComponent] copy];
            }
        } else {
            _filename = [[name lastPathComponent] copy];
            _localPath = [[self finalDestinationForPath:_filename
                                   destinationDirectory:[self downloadsDirectory]
                                                 prompt:YES] copy];
        }
        self.fileSize = size;
    }
    return self;
}

- (void)dealloc {
    [TransferrableFile unlockFileName:_localPath];
}

#pragma mark - Overridden methods from superclass

- (void)didFailWithError:(NSString *)error {
    os_unfair_lock_lock(&_lock);
    [super didFailWithError:error];
    self.error = error;
    os_unfair_lock_unlock(&_lock);
}

- (NSString *)displayName {
    return self.localPath ? [self.localPath lastPathComponent] : @"Unnamed file";
}

- (NSString *)shortName {
    return self.localPath ? [self.localPath lastPathComponent] : @"Unnamed file";
}

- (NSString *)subheading {
    return self.filename ?: @"Terminal download";
}

- (void)download {
    self.status = kTransferrableFileStatusStarting;
    [[FileTransferManager sharedInstance] transferrableFileDidStartTransfer:self];

    if (!self.localPath) {
        NSError *error;
        error = [self errorWithDescription:@"Canceled."];
        self.error = [error localizedDescription];
        [[FileTransferManager sharedInstance] transferrableFile:self didFinishTransmissionWithError:error];
    }
    // File downloads can be large; start with 16KB
    self.data = [NSMutableString stringWithCapacity:16 * 1024];
}

- (void)upload {
    // BUG-f647: Replace assert(false) with ELog - TerminalFile is download-only, upload is not supported
    ELog(@"BUG-f647: upload called on TerminalFile which is download-only - this method should not be called");
    // Notify of error instead of crashing
    NSError *error = [self errorWithDescription:@"TerminalFile does not support upload"];
    self.error = [error localizedDescription];
    [[FileTransferManager sharedInstance] transferrableFile:self didFinishTransmissionWithError:error];
}

- (void)stop {
    DLog(@"Stop file download.\n%@", [NSThread callStackSymbols]);
    self.status = kTransferrableFileStatusCancelling;
    [[FileTransferManager sharedInstance] transferrableFileWillStop:self];
    self.data = nil;
    [[NSNotificationCenter defaultCenter] postNotificationName:kTerminalFileShouldStopNotification object:self];
    [TransferrableFile unlockFileName:_localPath];
}

- (NSString *)destination {
    return self.localPath;
}

- (BOOL)isDownloading {
    return YES;
}

#pragma mark - APIs

- (BOOL)appendData:(NSString *)data {
    if (!self.data) {
        return YES;
    }
    self.status = kTransferrableFileStatusTransferring;

    // This keeps apr_base64_decode_len accurate.
    data = [data stringByReplacingOccurrencesOfRegex:@"[\r\n]" withString:@""];

    [self.data appendString:data];
    double approximateSize = self.data.length;
    approximateSize *= 3.0 / 4.0;
    self.bytesTransferred = ceil(approximateSize);
    if (self.fileSize >= 0) {
        self.bytesTransferred = MIN(self.fileSize, self.bytesTransferred);
    }
    [[FileTransferManager sharedInstance] transferrableFileProgressDidChange:self];
    if (approximateSize > self.fileSize + 5) {
        DLog(@"Have %@ bytes of base64 which encodes as much as %@ but the file's declared size is %@",
             @(self.data.length), @(approximateSize + 4), @(self.fileSize));
        return NO;
    }
    return YES;
}

- (NSInteger)length {
    return self.data.length;
}

- (void)endOfData {
    [self handleEndOfData];
    [TransferrableFile unlockFileName:_localPath];
}

- (void)handleEndOfData {
    if (!self.data) {
        self.status = kTransferrableFileStatusCancelled;
        [[FileTransferManager sharedInstance] transferrableFileDidStopTransfer:self];
        return;
    }
    const char *buffer = [self.data UTF8String];
    int destLength = apr_base64_decode_len(buffer);
    if (destLength < 1) {
        [[FileTransferManager sharedInstance] transferrableFile:self
                                 didFinishTransmissionWithError:[self errorWithDescription:@"No data received."]];
        return;
    }
    NSMutableData *data = [NSMutableData dataWithLength:destLength];
    char *decodedBuffer = [data mutableBytes];
    int resultLength = apr_base64_decode(decodedBuffer, buffer);
    if (resultLength < 0) {
        [[FileTransferManager sharedInstance]
                         transferrableFile:self
            didFinishTransmissionWithError:[self errorWithDescription:@"File corrupted (not valid base64)."]];
        return;
    }
    [data setLength:resultLength];
    NSError *error = nil;
    if (![data writeToFile:self.localPath options:NSDataWritingAtomic error:&error]) {
        [[FileTransferManager sharedInstance] transferrableFile:self
                                 didFinishTransmissionWithError:[self errorWithDescription:error.localizedDescription]];
        return;
    }
    if (![self quarantine:self.localPath sourceURL:nil]) {
        [[FileTransferManager sharedInstance]
                         transferrableFile:self
            didFinishTransmissionWithError:[self errorWithDescription:@"Failed to set quarantine."]];
        NSError *error = nil;
        const BOOL ok = [[NSFileManager defaultManager] removeItemAtPath:self.localPath error:&error];
        if (!ok || error) {
            // Avoid runloop in side-effect.
            dispatch_async(dispatch_get_main_queue(), ^{
                [self failedToRemoveUnquarantinedFileAt:self.localPath];
            });
        }
        return;
    }
    [[FileTransferManager sharedInstance] transferrableFile:self didFinishTransmissionWithError:nil];
    return;
}

#pragma mark - Private

- (NSError *)errorWithDescription:(NSString *)description {
    return [NSError errorWithDomain:@"com.dashterm.dashterm2.TerminalFile"
                               code:1
                           userInfo:@{NSLocalizedDescriptionKey : description}];
}


@end

@implementation TerminalFileUpload

- (BOOL)isDownloading {
    return NO;
}

- (void)upload {
    self.status = kTransferrableFileStatusTransferring;
    [[[FileTransferManager sharedInstance] files] addObject:self];
    [[FileTransferManager sharedInstance] transferrableFileDidStartTransfer:self];
}

- (void)endOfData {
    self.status = kTransferrableFileStatusCancelled;
    [[FileTransferManager sharedInstance] transferrableFileDidStopTransfer:self];
}

- (void)didUploadBytes:(NSInteger)count {
    self.bytesTransferred = count;
    if (count == self.fileSize) {
        [[FileTransferManager sharedInstance] transferrableFile:self didFinishTransmissionWithError:nil];
    } else {
        [[FileTransferManager sharedInstance] transferrableFileProgressDidChange:self];
    }
}

- (NSString *)finalDestinationForPath:(NSString *)originalBaseName
                 destinationDirectory:(NSString *)destinationDirectory
                               prompt:(BOOL)prompt {
    return originalBaseName;
}

@end
