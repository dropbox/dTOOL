//
//  VT100Parser.m
//  iTerm
//
//  Created by George Nachman on 3/2/14.
//
//

#import "VT100Parser.h"

#import "DebugLogging.h"
#import "iTermMalloc.h"
#import "NSStringITerm.h"
#import "VT100ByteStream.h"
#import "VT100ControlParser.h"
#import "VT100StringParser.h"
#import "VT100TokenPool.h"
#import <os/lock.h>

// Optimization: Cache NSNumber objects for SSH parser PIDs (0-1023).
// PIDs are process IDs used as keys in the _sshParsers dictionary.
// Most SSH integration scenarios have few concurrent parsers with low PIDs.
static const int kCachedSSHPIDCount = 1024;
static NSNumber *sSSHPIDCache[kCachedSSHPIDCount];

NS_INLINE NSNumber *iTermSSHPIDToNumber(int pid) {
    if (pid >= 0 && pid < kCachedSSHPIDCount) {
        return sSSHPIDCache[pid];
    }
    return @(pid);
}

__attribute__((constructor)) static void iTermVT100ParserInitializeSSHPIDCache(void) {
    for (int i = 0; i < kCachedSSHPIDCount; i++) {
        sSSHPIDCache[i] = @(i);
    }
}

@interface VT100Parser ()
// Nested parsers count their depth. This happens with ssh integration.
@property (nonatomic) int depth;
@end

@implementation VT100Parser {
    VT100ByteStream _byteStream;
    BOOL _saveData;
    NSMutableDictionary *_savedStateForPartialParse;
    VT100ControlParser *_controlParser;
    BOOL _dcsHooked; // protected by _lock
    // Key is pid
    NSMutableDictionary<NSNumber *, VT100Parser *> *_sshParsers;
    int _mainSSHParserPID;
    // for ssh conductor recovery. When true this causes the parser to emit a special token
    // that marks the first post-recovery token to be parsed.
    BOOL _emitRecoveryToken;
    NSInteger _nextBoundaryNumber;
    os_unfair_lock _lock;
}

- (instancetype)init {
    self = [super init];
    if (self) {
        VT100ByteStreamInit(&_byteStream);
        _savedStateForPartialParse = [[NSMutableDictionary alloc] initWithCapacity:4];
        _controlParser = [[VT100ControlParser alloc] init];
        _sshParsers = [[NSMutableDictionary alloc] initWithCapacity:2];
        _mainSSHParserPID = -1;
        _lock = OS_UNFAIR_LOCK_INIT;
    }
    return self;
}

- (void)dealloc {
    VT100ByteStreamFree(&_byteStream);
    [_savedStateForPartialParse release];
    [_controlParser release];
    [_sshParsers release];
    [super dealloc];
}

- (void)forceUnhookDCS:(NSString *)uniqueID {
    os_unfair_lock_lock(&_lock);
    [self forceUnhookDCS_locked:uniqueID];
    os_unfair_lock_unlock(&_lock);
}

// Internal version for use when lock is already held
- (void)forceUnhookDCS_locked:(NSString *)uniqueID {
    if (uniqueID == nil || [_controlParser shouldUnhook:uniqueID]) {
        _dcsHooked = NO;
        [_controlParser unhookDCS];
    } else {
        // TOOD: Maybe do something with ssh parsers?
    }
}

// BUG-10286: This method accesses shared state (_byteStream, _dcsHooked, _emitRecoveryToken, etc.)
// that is also accessed by other synchronized methods. While currently all call paths come through
// addParsedTokensToVector: which provides synchronization, we use os_unfair_lock here as a defensive
// measure. Note: os_unfair_lock is NOT re-entrant, so we use _locked helper methods internally.
- (BOOL)addNextParsedTokensToVector:(CVector *)vector nonSignalingCount:(out int *)nonSignalingCountPtr {
    os_unfair_lock_lock(&_lock);
    BOOL result = [self addNextParsedTokensToVector_locked:vector nonSignalingCount:nonSignalingCountPtr];
    os_unfair_lock_unlock(&_lock);
    return result;
}

- (BOOL)addNextParsedTokensToVector_locked:(CVector *)vector nonSignalingCount:(out int *)nonSignalingCountPtr {
    *nonSignalingCountPtr = 0;

    VT100Token *token = [[VT100TokenPool sharedPool] acquireToken];
    token.string = nil;
    // get our current position in the stream
    VT100ByteStreamCursor cursor;
    VT100ByteStreamCursorInit(&cursor, &_byteStream);

    DLog(@"Have %d bytes to parse", VT100ByteStreamCursorGetSize(&cursor));

    if (_emitRecoveryToken) {
        VT100Token *recoveryToken = [[VT100TokenPool sharedPool] acquireToken];
        recoveryToken.type = SSH_RECOVERY_BOUNDARY;
        recoveryToken.csi->p[0] = _nextBoundaryNumber - 1;
        recoveryToken.csi->count = 1;
        [recoveryToken retain];
        CVectorAppend(vector, recoveryToken);
        _emitRecoveryToken = NO;
    }

    VT100ByteStreamCursor position = {0};
    int length = 0;
    const int initialOffset = _byteStream.offset;
    BOOL isSignaling = NO;
    if (VT100ByteStreamCursorGetSize(&cursor) == 0) {
        DLog(@"datalen is 0");
        token->type = VT100CC_NULL;

        VT100ByteStreamReset(&_byteStream);
    } else {
        VT100ByteStreamConsumer consumer;
        VT100ByteStreamConsumerInit(&consumer, cursor);

        const NSStringEncoding encoding = self.encoding;
        const BOOL support8BitControlCharacters =
            (encoding == NSASCIIStringEncoding || encoding == NSISOLatin1StringEncoding);
        const unsigned char firstChar = VT100ByteStreamCursorPeek(&cursor);
        const unsigned char secondChar = VT100ByteStreamCursorDoublePeek(&cursor);
        if (isMixedAsciiString(firstChar, secondChar) && !_dcsHooked) {
            ParseString(&consumer, token, encoding);
            position = cursor;
        } else if (iscontrol(firstChar) || _dcsHooked || (support8BitControlCharacters && isc1(firstChar))) {
            if (self.literalMode) {
                token->type = VT100_LITERAL;
                token->code = firstChar;
                VT100ByteStreamConsumerSetConsumed(&consumer, 1);
            } else {
                [_controlParser parseControlWithConsumer:&consumer
                                             incidentals:vector
                                                   token:token
                                                encoding:encoding
                                              savedState:_savedStateForPartialParse
                                               dcsHooked:&_dcsHooked];
                DLog(@"%@: control parser produced %@", self, token);
                if (token->type != VT100_WAIT) {
                    [_savedStateForPartialParse removeAllObjects];
                }
                // Some tokens have synchronous side-effects.
                switch (token->type) {
                    case XTERMCC_SET_KVP:
                        if ([token.kvpKey isEqualToString:@"CopyToClipboard"]) {
                            _saveData = YES;
                        } else if ([token.kvpKey isEqualToString:@"EndCopy"]) {
                            _saveData = NO;
                        }
                        break;

                    case SSH_TERMINATE: {
                        isSignaling = YES;
                        // TODO: Make sure we don't leak sshparsers when connections end
                        const int pid = token.csi->p[0];
                        NSNumber *pidKey = iTermSSHPIDToNumber(pid);
                        DLog(@"Remove ssh parser for pid %@", pidKey);
                        [_sshParsers removeObjectForKey:pidKey];
                        if (pid == _mainSSHParserPID) {
                            DLog(@"Lost main SSH process %d", pid);
                            _mainSSHParserPID = -1;
                        }
                        break;
                    }

                    case SSH_UNHOOK:
                        isSignaling = YES;
                        [_sshParsers removeAllObjects];
                        break;

                    case SSH_OUTPUT: {
                        isSignaling = YES;
                        const int pid = token.csi->p[0];
                        NSNumber *pidKey = iTermSSHPIDToNumber(pid);
                        DLog(@"%@: handling SSH_OUTPUT", self);
                        VT100Parser *sshParser = _sshParsers[pidKey];
                        if (!sshParser) {
                            DLog(@"%@: I lack a parser for pid %@. Existing parsers:\n%@", self, pidKey, _sshParsers);
                            if (_sshParsers.count == 0 && token.csi->p[1] == -1) {
                                DLog(@"Inferring %d is the main SSH process", pid);
                                _mainSSHParserPID = pid;
                            }
                            sshParser = [[[VT100Parser alloc] init] autorelease];
                            sshParser.encoding = self.encoding;
                            sshParser.depth = self.depth + 1;
                            DLog(@"%@: Allocate ssh parser %@", self, sshParser);
                            _sshParsers[pidKey] = sshParser;
                        }
                        DLog(@"%@: Using child %@, begin reparsing SSH output in token %@ at depth %@: %@", self,
                             sshParser, token, @(self.depth), token.savedData);
                        NSData *data = token.savedData;
                        [sshParser putStreamData:data.bytes length:data.length];
                        const int start = CVectorCount(vector);
                        DLog(@"count before adding parsed tokens is %@", @(start));
                        [sshParser addParsedTokensToVector:vector];
                        const int end = CVectorCount(vector);
                        DLog(@"count after adding parsed tokens is %@", @(end));
                        const SSHInfo myInfo = {
                            .pid = pid, .channel = token.csi->p[1], .valid = 1, .depth = self.depth};
                        const SSHInfo childInfo = {
                            .pid = pid, .channel = token.csi->p[1], .valid = 1, .depth = self.depth + 1};
                        DLog(@"%@: reparsing yielded %d tokens", self, end - start);
                        for (int i = start; i < end; i++) {
                            VT100Token *token = CVectorGet(vector, i);
                            SSHInfo sshInfo = token.sshInfo;
                            if (!sshInfo.valid) {
                                DLog(@"%@: Update ssh info in rewritten token %@ to %@", self, token,
                                     SSHInfoDescription(myInfo));
                                switch (token.type) {
                                    case SSH_OUTPUT:
                                        // BUG-469: Replace assert(NO) with ELog and fallthrough
                                        // VT100Parser cannot emit SSH_OUTPUT, but handle gracefully if it does
                                        ELog(@"VT100Parser unexpectedly saw SSH_OUTPUT token");
                                        // Intentional fallthrough - treat like other SSH meta-tokens
                                    case SSH_INIT:
                                    case SSH_LINE:
                                    case SSH_UNHOOK:
                                    case SSH_BEGIN:
                                    case SSH_END:
                                    case SSH_TERMINATE:
                                        // Meta-tokens, when emitted as the product of %output, belong
                                        // to the child but will not be properly marked up with ssh info.
                                        token.sshInfo = childInfo;
                                        break;
                                    default:
                                        // Regular tokens (e.g., VT100_STRING) belong to this parser's
                                        // depth. The extra parser just decoded the output that belongs
                                        // to us.
                                        token.sshInfo = myInfo;
                                        if (myInfo.valid && myInfo.channel < 0) {
                                            // Since we found a regular token in here, count the entire buffer
                                            // as non-inband-signaling. This will overestimate the number of
                                            // bytes of non-signaling input. That's ok because currently the
                                            // consumer only cares if it is zero or not.
                                            isSignaling = NO;
                                        }
                                        break;
                                }
                            } else {
                                DLog(@"%@: Rewritten token %@ has valid SSH info %@ so not rewriting it", self, token,
                                     SSHInfoDescription(token.sshInfo));
                            }
                            DLog(@"%@: Emit subtoken %@ with info %@", self, token, SSHInfoDescription(token.sshInfo));
                        }
                        DLog(@"%@: done reparsing SSH output at depth %@", self, @(self.depth));
                        if (pid == SSH_OUTPUT_AUTOPOLL_PID || pid == SSH_OUTPUT_NOTIF_PID) {
                            // No need to keep this around, especially since it may carry some state we don't want.
                            [_sshParsers removeObjectForKey:pidKey];
                        }
                        break;
                    }

                    case DCS_TMUX_CODE_WRAP: {
                        isSignaling = YES;
                        VT100Parser *tempParser = [[[VT100Parser alloc] init] autorelease];
                        tempParser.encoding = encoding;
                        NSData *data = [token.string dataUsingEncoding:encoding];
                        [tempParser putStreamData:data.bytes length:data.length];
                        [tempParser addParsedTokensToVector:vector];
                        break;
                    }

                    case TMUX_EXIT:
                    case TMUX_LINE:
                        isSignaling = YES;
                        break;

                    case DCS_SSH_HOOK:
                    case SSH_INIT:
                    case SSH_LINE:
                    case SSH_BEGIN:
                    case SSH_END:
                    case SSH_RECOVERY_BOUNDARY:
                    case SSH_SIDE_CHANNEL:
                        isSignaling = YES;
                        break;

                    case ISO2022_SELECT_LATIN_1:
                        _encoding = NSISOLatin1StringEncoding;
                        break;

                    case ISO2022_SELECT_UTF_8:
                        _encoding = NSUTF8StringEncoding;
                        break;

                    default:
                        break;
                }
                position = cursor;
            }
        } else {
            if (isString(firstChar, encoding)) {
                ParseString(&consumer, token, encoding);
                // If the encoding is UTF-8 then you get here only if *datap >= 0x80.
                if (token->type != VT100_WAIT && VT100ByteStreamConsumerGetConsumed(&consumer) == 0) {
                    token->type = VT100_UNKNOWNCHAR;
                    token->code = VT100ByteStreamCursorPeek(&cursor);
                    VT100ByteStreamConsumerSetConsumed(&consumer, 1);
                }
            } else {
                // If the encoding is UTF-8 you shouldn't get here.
                token->type = VT100_UNKNOWNCHAR;
                token->code = VT100ByteStreamCursorPeek(&cursor);
                VT100ByteStreamConsumerSetConsumed(&consumer, 1);
            }
            position = cursor;
        }
        length = VT100ByteStreamConsumerGetConsumed(&consumer);


        if (VT100ByteStreamConsumerGetConsumed(&consumer) > 0) {
            ITAssertWithMessage(VT100ByteStreamGetCapacity(&_byteStream) >=
                                    VT100ByteStreamGetConsumed(&_byteStream) +
                                        VT100ByteStreamConsumerGetConsumed(&consumer),
                                @"Consumed more bytes than are available");
            // mark our current position in the stream
            VT100ByteStreamConsume(&_byteStream, VT100ByteStreamConsumerGetConsumed(&consumer));
        }
    }

    token->savingData = _saveData;
    if (token->type != VT100_WAIT && token->type != VT100CC_NULL) {
        if (_saveData) {
            token.savedData = VT100ByteStreamCursorMakeData(&position, length);
        }
        if (token->type == VT100_ASCIISTRING || token->type == VT100_MIXED_ASCII_CR_LF) {
            [token setAsciiBytes:(char *)VT100ByteStreamCursorGetPointer(&position) length:length];
        }

        if (gDebugLogging) {
            NSString *prefix = _controlParser.hookDescription;
            if (prefix) {
                prefix = [prefix stringByAppendingString:@" "];
            } else {
                prefix = @"";
            }
            NSMutableString *loginfo = [[NSMutableString alloc] initWithCapacity:length * 3];
            NSMutableString *ascii = [[NSMutableString alloc] initWithCapacity:length];
            int i = 0;
            int start = 0;
            while (i < length) {
                const unsigned char c = VT100ByteStreamCursorPeekOffset(&cursor, i);
                [loginfo appendFormat:@"%02x ", (int)c];
                [ascii appendFormat:@"%c", (c >= 32 && c < 128) ? c : '.'];
                if (i == length - 1) {
                    DLog(@"%@Bytes %d-%d of %d: %@ (%@)", prefix, start, i, (int)length, loginfo, ascii);
                }
                i++;
            }
            DLog(@"%@Parsed as %@", prefix, token);
        }
        // Don't append the outer wrapper to the output. Earlier, it was unwrapped and the inner
        // tokens were already added.
        if (token->type != DCS_TMUX_CODE_WRAP && (token->type != SSH_OUTPUT || self.depth == 0)) {
            [token retain];
            CVectorAppend(vector, token);
        } else {
            // Token not added to vector - recycle it
            [[VT100TokenPool sharedPool] recycleToken:token];
        }
        if (!isSignaling) {
            *nonSignalingCountPtr += (_byteStream.offset - initialOffset);
        }
        return YES;
    } else {
        DLog(@"unable to parse. Resulting token was %@", token);
        // Token not used (VT100_WAIT or VT100CC_NULL) - recycle it
        [[VT100TokenPool sharedPool] recycleToken:token];
    }

    return NO;
}

- (void)putStreamData:(const char *)bytes length:(int)length {
    os_unfair_lock_lock(&_lock);
    VT100ByteStreamAppend(&_byteStream, (const unsigned char *)bytes, length);
    os_unfair_lock_unlock(&_lock);
}

- (int)streamLength {
    os_unfair_lock_lock(&_lock);
    int result = VT100ByteStreamGetRemainingSize(&_byteStream);
    os_unfair_lock_unlock(&_lock);
    return result;
}

- (NSData *)streamData {
    os_unfair_lock_lock(&_lock);
    NSData *result = VT100ByteStreamMakeData(&_byteStream);
    os_unfair_lock_unlock(&_lock);
    return result;
}

- (void)clearStream {
    os_unfair_lock_lock(&_lock);
    [self clearStream_locked];
    os_unfair_lock_unlock(&_lock);
}

// Internal version for use when lock is already held
- (void)clearStream_locked {
    VT100ByteStreamConsumeAll(&_byteStream);
    [_sshParsers[iTermSSHPIDToNumber(_mainSSHParserPID)] clearStream];
}

- (int)addParsedTokensToVector:(CVector *)vector {
    os_unfair_lock_lock(&_lock);
    int sum = 0;
    int nsc = 0;
    // Call _locked version directly to avoid re-acquiring lock (deadlock)
    while ([self addNextParsedTokensToVector_locked:vector nonSignalingCount:&nsc]) {
        sum += nsc;
    }
    os_unfair_lock_unlock(&_lock);
    return sum;
}

- (void)startTmuxRecoveryModeWithID:(NSString *)dcsID {
    os_unfair_lock_lock(&_lock);
    [_controlParser startTmuxRecoveryModeWithID:dcsID];
    _dcsHooked = YES;
    os_unfair_lock_unlock(&_lock);
}

- (void)cancelTmuxRecoveryMode {
    os_unfair_lock_lock(&_lock);
    [_controlParser cancelTmuxRecoveryMode];
    _dcsHooked = NO;
    os_unfair_lock_unlock(&_lock);
}

- (NSString *)description {
    return [NSString stringWithFormat:@"<%@: %p dcsHooked=%@ depth=%@>", NSStringFromClass([self class]), self,
                                      @(_dcsHooked), @(_depth)];
}

// tree: [child pid: [dcs ID, tree]]
- (NSInteger)startConductorRecoveryModeWithID:(NSString *)dcsID tree:(NSDictionary *)tree {
    DLog(@"%@: startConductorRecoveryModeWithID:%@ tree:%@", self, dcsID, tree);
    [_sshParsers removeAllObjects];
    const NSInteger boundary = [self reallyStartConductorRecoveryModeWithID:dcsID tree:tree];
    DLog(@"After recovery:");
    [self printParsers:@""];
    return boundary;
}

- (void)printParsers:(NSString *)prefix {
    [_sshParsers
        enumerateKeysAndObjectsUsingBlock:^(NSNumber *_Nonnull key, VT100Parser *_Nonnull obj, BOOL *_Nonnull stop) {
            DLog(@"%@%@", prefix, obj);
            [obj printParsers:[@"    " stringByAppendingString:prefix]];
        }];
}

- (NSInteger)reallyStartConductorRecoveryModeWithID:(NSString *)dcsID tree:(NSDictionary *)tree {
    DLog(@"%@: reallyStartConductorRecoveryModeWithID:%@ tree:%@", self, dcsID, tree);
    os_unfair_lock_lock(&_lock);
    NSInteger result;
    if (tree.count == 0) {
        result = _nextBoundaryNumber++;
        os_unfair_lock_unlock(&_lock);
        return result;
    }
    if (tree[@0]) {
        // No special parsing needed by this node.
        NSArray *tuple = tree[@0];
        NSString *childDcsId = tuple[0];
        NSDictionary *childTree = tuple[1];
        // Must unlock before recursive call to startConductorRecoveryModeWithID
        // which may call back into this method
        os_unfair_lock_unlock(&_lock);
        [self startConductorRecoveryModeWithID:childDcsId tree:childTree];
        os_unfair_lock_lock(&_lock);
        result = _nextBoundaryNumber++;
        os_unfair_lock_unlock(&_lock);
        return result;
    }
    [_controlParser startConductorRecoveryModeWithID:dcsID];
    _dcsHooked = YES;
    // recoverWithConductorTree calls reallyStartConductorRecoveryModeWithID on child
    // parsers (different objects), so no deadlock risk
    [self recoverWithConductorTree:tree];
    _emitRecoveryToken = YES;
    result = _nextBoundaryNumber++;
    os_unfair_lock_unlock(&_lock);
    return result;
}

- (void)recoverWithConductorTree:(NSDictionary *)tree {
    for (NSNumber *childPID in tree) {
        NSArray *tuple = tree[childPID];
        NSString *childDcsId = tuple[0];
        NSDictionary *childTree = tuple[1];

        VT100Parser *childParser = [[[VT100Parser alloc] init] autorelease];
        childParser.encoding = self.encoding;
        childParser.depth = self.depth + 1;
        _sshParsers[childPID] = childParser;
        [childParser reallyStartConductorRecoveryModeWithID:childDcsId tree:childTree];
        DLog(@"%@: add recovered child parser with pid %@: %@", self, childPID, childParser);
    }
}

- (void)cancelConductorRecoveryMode {
    os_unfair_lock_lock(&_lock);
    // TODO: This doesn't attempt to handle nested conductors.
    [_controlParser cancelConductorRecoveryMode];
    _dcsHooked = NO;
    os_unfair_lock_unlock(&_lock);
}

- (void)reset {
    os_unfair_lock_lock(&_lock);
    [_savedStateForPartialParse removeAllObjects];
    [self forceUnhookDCS_locked:nil];
    [self clearStream_locked];
    [_sshParsers[iTermSSHPIDToNumber(_mainSSHParserPID)] reset];
    os_unfair_lock_unlock(&_lock);
}

- (void)resetExceptSSH {
    os_unfair_lock_lock(&_lock);
    [_savedStateForPartialParse removeAllObjects];
    if (!_controlParser.dcsHookIsSSH) {
        [self forceUnhookDCS_locked:nil];
        [self clearStream_locked];
        [_sshParsers[iTermSSHPIDToNumber(_mainSSHParserPID)] reset];
    }
    os_unfair_lock_unlock(&_lock);
}

@end
