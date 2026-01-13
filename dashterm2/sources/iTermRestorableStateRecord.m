//
//  iTermRestorableStateRecord.m
//  DashTerm2
//
//  Created by George Nachman on 2/19/20.
//

#import "iTermRestorableStateRecord.h"

#import "DebugLogging.h"
#import "NSData+iTerm.h"
#import "NSFileManager+iTerm.h"
#import "NSObject+iTerm.h"
#import <Security/SecRandom.h>
#include <stdlib.h>

@implementation iTermRestorableStateRecord

static const NSUInteger kRestorableStateRecordLegacyVersion = 1;
static const NSUInteger kRestorableStateRecordCurrentVersion = 2;
static const NSUInteger kAESBlockSizeBytes = 16;

- (instancetype)initWithWindowNumber:(NSInteger)windowNumber
                          identifier:(NSString *)identifier
                                 key:(NSData *)key
                           plaintext:(NSData *)plaintext {
    self = [super init];
    if (self) {
        _windowNumber = windowNumber;
        _identifier = [identifier copy];
        _key = [key copy];
        _plaintext = plaintext;
    }
    return self;
}

+ (void)createWithIndexEntry:(id)indexEntry completion:(void (^)(iTermRestorableStateRecord *record))completion {
    dispatch_async(dispatch_get_global_queue(DISPATCH_QUEUE_PRIORITY_DEFAULT, 0), ^{
        iTermRestorableStateRecord *record = [[self alloc] initWithIndexEntry:indexEntry];
        dispatch_async(dispatch_get_main_queue(), ^{
            completion(record);
        });
    });
}

- (instancetype)initWithIndexEntry:(id)indexEntry {
    NSDictionary *dict = [NSDictionary castFrom:indexEntry];
    if (!dict) {
        return nil;
    }
    NSNumber *windowNumber = [NSNumber castFrom:dict[@"windowNumber"]];
    if (!windowNumber) {
        return nil;
    }
    NSString *identifier = [NSString castFrom:dict[@"identifier"]];
    if (!identifier) {
        return nil;
    }
    NSData *key = [NSData castFrom:dict[@"key"]];
    if (!key) {
        return nil;
    }
    self = [self initWithWindowNumber:[dict[@"windowNumber"] integerValue]
                           identifier:identifier
                                  key:key
                            plaintext:[NSData data]];
    if (self) {
        NSData *blob = [NSData dataWithContentsOfURL:[self url]];
        if (!blob) {
            return nil;
        }
        NSData *ciphertext = nil;
        NSData *iv = nil;
        if (![self extractCiphertext:&ciphertext iv:&iv fromBlob:blob]) {
            return nil;
        }
        _plaintext = [ciphertext decryptedAESCBCDataWithPCKS7PaddingAndKey:self.key iv:iv];
        if (!_plaintext) {
            return nil;
        }
    }
    return self;
}

#pragma mark - iTermRestorableStateRecord

- (void)didFinishRestoring {
    unlink(self.url.path.UTF8String);
}

#pragma mark - APIs

- (void)save {
    [self.data writeReadOnlyToURL:self.url];
}

- (id)indexEntry {
    return @{@"identifier" : self.identifier ?: @"", @"windowNumber" : @(self.windowNumber), @"key" : self.key};
}

- (iTermRestorableStateRecord *)withPlaintext:(NSData *)newPlaintext {
    return [[iTermRestorableStateRecord alloc] initWithWindowNumber:_windowNumber
                                                         identifier:_identifier
                                                                key:_key
                                                          plaintext:newPlaintext];
}

#pragma mark - Saving

- (NSData *)data {
    // Header (magic+version) + IV length word + IV + ciphertext length word + ciphertext
    // Estimate: 8 + 4 + 16 + 4 + self.plaintext.length + padding
    NSMutableData *buffer = [NSMutableData dataWithCapacity:32 + self.plaintext.length + 16];
    [buffer appendData:[self magic]];
    [buffer appendData:[self version]];
    NSData *iv = [self randomIV];
    NSData *ciphertext = [self encryptedPlaintextWithIV:iv];
    [buffer appendData:[self word:iv.length]];
    [buffer appendData:iv];
    [buffer appendData:[self word:ciphertext.length]];
    [buffer appendData:ciphertext];
    return buffer;
}

- (NSData *)encryptedPlaintextWithIV:(NSData *)iv {
    return [self.plaintext aesCBCEncryptedDataWithPCKS7PaddingAndKey:self.key iv:iv];
}

- (NSData *)randomIV {
    NSMutableData *iv = [NSMutableData dataWithLength:kAESBlockSizeBytes];
    const int status = SecRandomCopyBytes(kSecRandomDefault, iv.length, iv.mutableBytes);
    if (status != errSecSuccess) {
        ELog(@"SecRandomCopyBytes failed with status %d, falling back to arc4random_buf", status);
        arc4random_buf(iv.mutableBytes, iv.length);
    }
    return iv;
}

#pragma mark - Loading

- (NSData *)legacyZeroIV {
    return [NSMutableData dataWithLength:kAESBlockSizeBytes];
}

- (BOOL)extractCiphertext:(NSData *_Nullable *_Nonnull)ciphertext
                       iv:(NSData *_Nullable *_Nonnull)iv
                 fromBlob:(NSData *)blob {
    NSInteger offset = 0;

    NSData *magic = self.magic;
    if (blob.length < offset + magic.length) {
        return NO;
    }
    NSData *magicSlice = [blob subdataWithRange:NSMakeRange(offset, magic.length)];
    if (![magicSlice isEqualToData:magic]) {
        return NO;
    }
    offset += magic.length;

    if (blob.length < offset + sizeof(NSUInteger)) {
        return NO;
    }
    NSData *versionWord = [blob subdataWithRange:NSMakeRange(offset, sizeof(NSUInteger))];
    offset += sizeof(NSUInteger);
    const NSUInteger version = [self decodeWord:versionWord];

    if (version < kRestorableStateRecordLegacyVersion) {
        DLog(@"Unsupported restorable state record version %@", @(version));
        return NO;
    }

    NSData *ivData = nil;
    if (version >= kRestorableStateRecordCurrentVersion) {
        if (blob.length < offset + sizeof(NSUInteger)) {
            return NO;
        }
        NSData *ivLengthWord = [blob subdataWithRange:NSMakeRange(offset, sizeof(NSUInteger))];
        offset += sizeof(NSUInteger);
        const NSUInteger ivLength = [self decodeWord:ivLengthWord];
        if (blob.length < offset + ivLength) {
            return NO;
        }
        ivData = [blob subdataWithRange:NSMakeRange(offset, ivLength)];
        offset += ivLength;
    }

    if (blob.length < offset + sizeof(NSUInteger)) {
        return NO;
    }
    NSData *lengthWord = [blob subdataWithRange:NSMakeRange(offset, sizeof(NSUInteger))];
    offset += sizeof(NSUInteger);
    const NSUInteger length = [self decodeWord:lengthWord];
    if (blob.length < offset + length) {
        return NO;
    }

    if (!ivData) {
        ivData = [self legacyZeroIV];
    }
    if (ivData.length != kAESBlockSizeBytes) {
        DLog(@"Invalid IV length %@ for version %@", @(ivData.length), @(version));
        return NO;
    }

    if (ciphertext) {
        *ciphertext = [blob subdataWithRange:NSMakeRange(offset, length)];
    }
    if (iv) {
        *iv = ivData;
    }
    return YES;
}

- (NSUInteger)decodeWord:(NSData *)data {
    NSUInteger w = 0;
    // BUG-f1380: Replace assert with guard - corrupted data should return 0, not crash
    if (sizeof(w) != data.length) {
        DLog(@"WARNING: decodeWord called with wrong data length %lu, expected %lu", (unsigned long)data.length,
             (unsigned long)sizeof(w));
        return 0;
    }
    const unsigned char *bytes = (const unsigned char *)data.bytes;
    for (int i = 0; i < 8; i++) {
        w <<= 8;
        w |= bytes[i];
    }
    return w;
}

#pragma mark - Common

- (NSURL *)url {
    NSString *appSupport = [[NSFileManager defaultManager] applicationSupportDirectory];
    NSString *savedState = [appSupport stringByAppendingPathComponent:@"SavedState"];

    NSURL *url = [NSURL fileURLWithPath:savedState];
    [url setResourceValue:@YES forKey:NSURLIsExcludedFromBackupKey error:nil];
    url = [url URLByAppendingPathComponent:[NSString stringWithFormat:@"%@.data", @(self.windowNumber)]];
    return [url URLByResolvingSymlinksInPath];
}

- (NSData *)magic {
    return [NSData dataWithBytes:"itws" length:4];
}

- (NSData *)version {
    return [self word:kRestorableStateRecordCurrentVersion];
}

- (NSData *)word:(NSUInteger)value {
    char temp[8];
    // BUG-f1381: Replace assert with _Static_assert - compile-time check for 64-bit platform
    _Static_assert(sizeof(char[8]) == sizeof(NSUInteger), "NSUInteger must be 8 bytes (64-bit)");
    NSUInteger w = value;
    for (int i = 7; i >= 0; i--) {
        temp[i] = (w & 0xff);
        w >>= 8;
    }
    return [NSData dataWithBytes:temp length:sizeof(temp)];
}

- (NSKeyedUnarchiver *)unarchiver {
    DLog(@"Restore %@", @(self.windowNumber));
    NSError *error = nil;
    NSKeyedUnarchiver *unarchiver = [[NSKeyedUnarchiver alloc] initForReadingFromData:self.plaintext error:&error];
    // Note: requiresSecureCoding must be NO here because window state restoration
    // decodes many AppKit private classes that don't support NSSecureCoding.
    // This is safe because the data comes from the app's own sandbox and was
    // previously serialized by the same app.
    unarchiver.requiresSecureCoding = NO;
    if (error) {
        DLog(@"Restoration failed with %@", error);
        unlink(self.url.path.UTF8String);
        return nil;
    }

    return unarchiver;
}

- (nonnull id<iTermRestorableStateRecord>)recordWithPayload:(nonnull id)payload {
    return [self withPlaintext:payload];
}

@end
