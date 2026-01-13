//
//  iTermSlowOperationGateway.m
//  DashTerm2SharedARC
//
//  Created by George Nachman on 8/12/20.
//

#import "iTermSlowOperationGateway.h"

#import "DebugLogging.h"
#import "DashTerm2SharedARC-Swift.h"
#import "iTermAdvancedSettingsModel.h"
#import "iTermGitState.h"
#import "iTermLSOF.h"
#import "ITAddressBookMgr.h"
#import "iTermOpenDirectory.h"
#import "NSStringITerm.h"
#import "ProfileModel.h"
#import "pidinfo.h"
#include <stdatomic.h>
#include <os/lock.h>

typedef void (^iTermRecentBranchFetchCallback)(NSArray<NSString *> *);

@interface iTermGitRecentBranchesBox : NSObject
@property (nonatomic, copy) iTermRecentBranchFetchCallback block;
@end

@implementation iTermGitRecentBranchesBox
- (BOOL)isEqual:(id)object {
    return self == object;
}
@end

@interface iTermGitStateHandlerBox : NSObject
@property (nonatomic, copy) void (^block)(iTermGitState *);
@end

@implementation iTermGitStateHandlerBox

- (BOOL)isEqual:(id)object {
    return self == object;
}
@end

@interface iTermSlowOperationGateway ()
@property (nonatomic, readwrite) BOOL ready;
@end

@implementation iTermSlowOperationGateway {
    NSXPCConnection *_connectionToService;
    NSTimeInterval _timeout;
    NSMutableArray<iTermGitStateHandlerBox *> *_gitStateHandlers;
    NSMutableArray<iTermGitRecentBranchesBox *> *_gitRecentBranchFetchCallbacks;
    os_unfair_lock _gitStateHandlersLock;
    os_unfair_lock _gitRecentBranchFetchCallbacksLock;
    os_unfair_lock _reqidLock;
}

+ (instancetype)sharedInstance {
    static dispatch_once_t onceToken;
    static iTermSlowOperationGateway *instance;
    dispatch_once(&onceToken, ^{
        instance = [[self alloc] initPrivate];
    });
    return instance;
}

- (instancetype)initPrivate {
    self = [super init];
    if (self) {
        // Pre-allocate for typical number of handlers/callbacks (usually 4-16)
        _gitStateHandlers = [NSMutableArray arrayWithCapacity:8];
        _gitRecentBranchFetchCallbacks = [NSMutableArray arrayWithCapacity:8];
        _gitStateHandlersLock = OS_UNFAIR_LOCK_INIT;
        _gitRecentBranchFetchCallbacksLock = OS_UNFAIR_LOCK_INIT;
        _reqidLock = OS_UNFAIR_LOCK_INIT;
        _timeout = 0.5;
        [self connect];
        __weak __typeof(self) weakSelf = self;
        [_connectionToService.remoteObjectProxy handshakeWithReply:^{
            __strong __typeof(self) strongSelf = weakSelf;
            if (!strongSelf) {
                return;
            }
            strongSelf.ready = YES;
        }];
    }
    return self;
}

- (void)didInvalidateConnection {
    self.ready = NO;
    [self connect];
}

- (void)connect {
    _connectionToService = [[NSXPCConnection alloc] initWithServiceName:@"com.dashterm.dashterm2.pidinfo"];
    _connectionToService.remoteObjectInterface = [NSXPCInterface interfaceWithProtocol:@protocol(pidinfoProtocol)];
    [_connectionToService resume];

    __weak __typeof(self) weakSelf = self;
    _connectionToService.invalidationHandler = ^{
        // I can't manage to get this called. This project:
        // https://github.com/brenwell/EvenBetterAuthorizationSample
        // originally from:
        // https://developer.apple.com/library/archive/samplecode/EvenBetterAuthorizationSample/Introduction/Intro.html
        // seems to have been written carefully and states that you can retry creating the
        // connection on the main thread.
        DLog(@"Invalidated");
        dispatch_async(dispatch_get_main_queue(), ^{
            [weakSelf didInvalidateConnection];
        });
    };
    _connectionToService.interruptionHandler = ^{
        [weakSelf didInterrupt];
    };
    // For getProcessInfoForProcessID:flavor:arg:size:reqid:withReply:
    [_connectionToService.remoteObjectInterface
           setClasses:[NSSet setWithObject:[NSNumber class]]
          forSelector:@selector(getProcessInfoForProcessID:flavor:arg:size:reqid:withReply:)
        argumentIndex:0
              ofReply:YES];
    [_connectionToService.remoteObjectInterface
           setClasses:[NSSet setWithObject:[NSData class]]
          forSelector:@selector(getProcessInfoForProcessID:flavor:arg:size:reqid:withReply:)
        argumentIndex:1
              ofReply:YES];

    // handshakeWithReply: – No registration needed.

    // For checkIfDirectoryExists:withReply:
    [_connectionToService.remoteObjectInterface setClasses:[NSSet setWithObject:[NSNumber class]]
                                               forSelector:@selector(checkIfDirectoryExists:withReply:)
                                             argumentIndex:0
                                                   ofReply:YES];

    // For checkIfExecutableRegularFile:searchPaths:withReply:
    [_connectionToService.remoteObjectInterface setClasses:[NSSet setWithObject:[NSNumber class]]
                                               forSelector:@selector(checkIfExecutableRegularFile:
                                                                                      searchPaths:withReply:)
                                             argumentIndex:0
                                                   ofReply:YES];

    // statFile:withReply: returns a struct and int – no registration needed.

    // For runShellScript:shell:withReply: (reply block: NSData *output, NSData *error, int status)
    [_connectionToService.remoteObjectInterface setClasses:[NSSet setWithArray:@[ [NSData class], [NSNumber class] ]]
                                               forSelector:@selector(runShellScript:shell:withReply:)
                                             argumentIndex:0
                                                   ofReply:YES];
    [_connectionToService.remoteObjectInterface setClasses:[NSSet setWithArray:@[ [NSData class], [NSNumber class] ]]
                                               forSelector:@selector(runShellScript:shell:withReply:)
                                             argumentIndex:1
                                                   ofReply:YES];

    // For findCompletionsWithPrefix:inDirectories:pwd:maxCount:executable:withReply:
    [_connectionToService.remoteObjectInterface
           setClasses:[NSSet setWithArray:@[ [NSArray class], [NSString class] ]]
          forSelector:@selector(findCompletionsWithPrefix:inDirectories:pwd:maxCount:executable:withReply:)
        argumentIndex:0
              ofReply:YES];

    // For requestGitStateForPath:timeout:completion:
    [_connectionToService.remoteObjectInterface setClasses:[NSSet setWithObject:[iTermGitState class]]
                                               forSelector:@selector(requestGitStateForPath:timeout:completion:)
                                             argumentIndex:0
                                                   ofReply:YES];

    // For fetchRecentBranchesAt:count:completion:
    [_connectionToService.remoteObjectInterface setClasses:[NSSet setWithArray:@[ [NSArray class], [NSString class] ]]
                                               forSelector:@selector(fetchRecentBranchesAt:count:completion:)
                                             argumentIndex:0
                                                   ofReply:YES];

    // For
    // findExistingFileWithPrefix:suffix:workingDirectory:trimWhitespace:pathsToIgnore:allowNetworkMounts:reqid:reply:
    [_connectionToService.remoteObjectInterface
           setClasses:[NSSet setWithObject:[NSString class]]
          forSelector:@selector
          (findExistingFileWithPrefix:
                               suffix:workingDirectory:trimWhitespace:pathsToIgnore:allowNetworkMounts:reqid:reply:)
        argumentIndex:0
              ofReply:YES];

    // For cancelFindExistingFileRequest:reply: – void reply (nothing to register).

    // For executeShellCommand:args:dir:env:reply: (reply block: NSData *stdout, NSData *stderr, uint8_t status,
    // NSTaskTerminationReason reason)
    [_connectionToService.remoteObjectInterface setClasses:[NSSet setWithObject:[NSData class]]
                                               forSelector:@selector(executeShellCommand:args:dir:env:reply:)
                                             argumentIndex:0
                                                   ofReply:YES];
    [_connectionToService.remoteObjectInterface setClasses:[NSSet setWithObject:[NSData class]]
                                               forSelector:@selector(executeShellCommand:args:dir:env:reply:)
                                             argumentIndex:1
                                                   ofReply:YES];

    // For fetchDirectoryListingOfPath:completion:
    [_connectionToService.remoteObjectInterface
           setClasses:[NSSet setWithArray:@[ [NSArray class], [iTermDirectoryEntry class] ]]
          forSelector:@selector(fetchDirectoryListingOfPath:completion:)
        argumentIndex:0
              ofReply:YES];
}

- (void)didInterrupt {
    {
        NSArray<iTermGitStateHandlerBox *> *handlers;
        os_unfair_lock_lock(&_gitStateHandlersLock);
        handlers = [_gitStateHandlers copy];
        [_gitStateHandlers removeAllObjects];
        os_unfair_lock_unlock(&_gitStateHandlersLock);
        DLog(@"didInterrupt. Run all %@ handlers", @(handlers.count));
        [handlers
            enumerateObjectsUsingBlock:^(iTermGitStateHandlerBox *_Nonnull obj, NSUInteger idx, BOOL *_Nonnull stop) {
                obj.block(nil);
            }];
    }
    {
        NSArray<iTermGitRecentBranchesBox *> *handlers;
        os_unfair_lock_lock(&_gitRecentBranchFetchCallbacksLock);
        handlers = [_gitRecentBranchFetchCallbacks copy];
        [_gitRecentBranchFetchCallbacks removeAllObjects];
        os_unfair_lock_unlock(&_gitRecentBranchFetchCallbacksLock);
        [handlers
            enumerateObjectsUsingBlock:^(iTermGitRecentBranchesBox *_Nonnull obj, NSUInteger idx, BOOL *_Nonnull stop) {
                obj.block(nil);
            }];
    }
}

- (int)nextReqid {
    static int next;
    os_unfair_lock_lock(&_reqidLock);
    int result = next++;
    os_unfair_lock_unlock(&_reqidLock);
    return result;
}

// BUG-3250: Always call completion handler, even when not ready or when exists is nil
- (void)checkIfDirectoryExists:(NSString *)directory completion:(void (^)(BOOL))completion {
    if (!self.ready) {
        dispatch_async(dispatch_get_main_queue(), ^{
            completion(NO);
        });
        return;
    }
    [[_connectionToService remoteObjectProxy] checkIfDirectoryExists:directory
                                                           withReply:^(NSNumber *_Nullable exists) {
                                                               dispatch_async(dispatch_get_main_queue(), ^{
                                                                   completion(exists.boolValue);
                                                               });
                                                           }];
}

// BUG-3251: Always call completion handler, even when not ready
- (void)statFile:(NSString *)path completion:(void (^)(struct stat, int))completion {
    if (!self.ready) {
        dispatch_async(dispatch_get_main_queue(), ^{
            struct stat emptyStat = {0};
            completion(emptyStat, ENOENT);
        });
        return;
    }
    [[_connectionToService remoteObjectProxy] statFile:path
                                             withReply:^(struct stat statbuf, int error) {
                                                 dispatch_async(dispatch_get_main_queue(), ^{
                                                     completion(statbuf, error);
                                                 });
                                             }];
}

// BUG-3252: Always call completion handler, even when not ready
- (void)checkIfExecutableRegularFile:(NSString *)filename
                         searchPaths:(NSArray<NSString *> *)searchPaths
                          completion:(void (^)(BOOL))completion {
    if (!self.ready) {
        dispatch_async(dispatch_get_main_queue(), ^{
            completion(NO);
        });
        return;
    }
    [[_connectionToService remoteObjectProxy] checkIfExecutableRegularFile:filename
                                                               searchPaths:searchPaths
                                                                 withReply:^(NSNumber *_Nullable exists) {
                                                                     dispatch_async(dispatch_get_main_queue(), ^{
                                                                         completion(exists.boolValue);
                                                                     });
                                                                 }];
}

// BUG-3257: Check ready state before using XPC connection
- (void)exfiltrateEnvironmentVariableNamed:(NSString *)name
                                     shell:(NSString *)shell
                                completion:(void (^)(NSString *_Nullable))completion {
    if (!self.ready) {
        dispatch_async(dispatch_get_main_queue(), ^{
            completion(nil);
        });
        return;
    }
    [[_connectionToService remoteObjectProxy]
        runShellScript:[NSString stringWithFormat:@"echo $%@", name]
                 shell:shell
             withReply:^(NSData *_Nullable data, NSData *_Nullable error, int status) {
                 dispatch_async(dispatch_get_main_queue(), ^{
                     completion(status == 0
                                    ? [[[NSString alloc] initWithData:data encoding:NSUTF8StringEncoding]
                                          stringByTrimmingTrailingCharactersFromCharacterSet:[NSCharacterSet
                                                                                                 newlineCharacterSet]]
                                    : nil);
                 });
             }];
}

- (void)asyncGetInfoForProcess:(int)pid
                        flavor:(int)flavor
                           arg:(uint64_t)arg
                    buffersize:(int)buffersize
                         reqid:(int)reqid
                    completion:(void (^)(int rc, NSData *buffer))completion {
    NSNumber *pidNumber = [iTermLSOF cachedNumberForPid:pid];
    __block atomic_flag finished = ATOMIC_FLAG_INIT;
    [[_connectionToService remoteObjectProxy]
        getProcessInfoForProcessID:pidNumber
                            flavor:@(flavor)
                               arg:@(arg)
                              size:@(buffersize)
                             reqid:reqid
                         withReply:^(NSNumber *rc, NSData *buffer) {
                             // Called on a private queue
                             if (atomic_flag_test_and_set(&finished)) {
                                 DLog(@"Return early because already timed out for pid %@", pidNumber);
                                 return;
                             }
                             DLog(@"Completed with rc=%@", rc);
                             if (buffer.length != buffersize) {
                                 completion(-3, [NSData data]);
                                 return;
                             }
                             completion(rc.intValue, buffer);
                         }];
    dispatch_after(dispatch_time(DISPATCH_TIME_NOW, (int64_t)(_timeout * NSEC_PER_SEC)), dispatch_get_main_queue(), ^{
        if (atomic_flag_test_and_set(&finished)) {
            return;
        }
        DLog(@"Timed out");
        completion(-4, [NSData data]);
    });
}

// BUG-3258: Check ready state before using XPC connection
- (void)runCommandInUserShell:(NSString *)command completion:(void (^)(NSString *))completion {
    if (!self.ready) {
        dispatch_async(dispatch_get_main_queue(), ^{
            completion(nil);
        });
        return;
    }
    [[_connectionToService remoteObjectProxy]
        runShellScript:command
                 shell:[iTermOpenDirectory userShell] ?: @"/bin/bash"
             withReply:^(NSData *_Nullable data, NSData *_Nullable error, int status) {
                 dispatch_async(dispatch_get_main_queue(), ^{
                     completion(status == 0
                                    ? [[[NSString alloc] initWithData:data encoding:NSUTF8StringEncoding]
                                          stringByTrimmingTrailingCharactersFromCharacterSet:[NSCharacterSet
                                                                                                 newlineCharacterSet]]
                                    : nil);
                 });
             }];
}

// BUG-3259: Check ready state before using XPC connection
- (void)findCompletionsWithPrefix:(NSString *)prefix
                    inDirectories:(NSArray<NSString *> *)directories
                              pwd:(NSString *)pwd
                         maxCount:(NSInteger)maxCount
                       executable:(BOOL)executable
                       completion:(void (^)(NSArray<NSString *> *))completion {
    DLog(@"findCompletionsWithPrefix:%@ inDirectories:%@ pwd:%@ maxCount:%@ executable:%@", prefix, directories, pwd,
         @(maxCount), @(executable));
    if (!self.ready) {
        dispatch_async(dispatch_get_main_queue(), ^{
            completion(@[]);
        });
        return;
    }
    [[_connectionToService remoteObjectProxy]
        findCompletionsWithPrefix:prefix
                    inDirectories:directories
                              pwd:pwd
                         maxCount:maxCount
                       executable:executable
                        withReply:^(NSArray<NSString *> *completions) {
                            dispatch_async(dispatch_get_main_queue(), ^{
                                DLog(@"findCompletionsWithPrefix:%@ inDirectories:%@ pwd:%@ maxCount:%@ executable:%@ "
                                     @"-> %@",
                                     prefix, directories, pwd, @(maxCount), @(executable), completions);
                                completion(completions);
                            });
                        }];
}

// BUG-3260: Check ready state before using XPC connection
- (void)requestGitStateForPath:(NSString *)path completion:(void (^)(iTermGitState *_Nullable))completion {
    if (!self.ready) {
        dispatch_async(dispatch_get_main_queue(), ^{
            completion(nil);
        });
        return;
    }
    iTermGitStateHandlerBox *box = [[iTermGitStateHandlerBox alloc] init];
    box.block = completion;
    os_unfair_lock_lock(&_gitStateHandlersLock);
    [_gitStateHandlers addObject:box];
    os_unfair_lock_unlock(&_gitStateHandlersLock);
    [[_connectionToService remoteObjectProxy] requestGitStateForPath:path
                                                             timeout:[iTermAdvancedSettingsModel gitTimeout]
                                                          completion:^(iTermGitState *_Nullable state) {
                                                              [self didGetGitState:state completion:box];
                                                          }];
}

// Runs on some random queue
- (void)didGetGitState:(iTermGitState *)gitState completion:(iTermGitStateHandlerBox *)completion {
    os_unfair_lock_lock(&_gitStateHandlersLock);
    BOOL found = [_gitStateHandlers containsObject:completion];
    if (found) {
        [_gitStateHandlers removeObject:completion];
    }
    os_unfair_lock_unlock(&_gitStateHandlersLock);
    if (!found) {
        return;
    }
    dispatch_async(dispatch_get_main_queue(), ^{
        completion.block(gitState);
    });
}

- (void)fetchRecentBranchesAt:(NSString *)path
                        count:(NSInteger)maxCount
                   completion:(void (^)(NSArray<NSString *> *))reply {
    iTermGitRecentBranchesBox *box = [[iTermGitRecentBranchesBox alloc] init];
    box.block = reply;
    os_unfair_lock_lock(&_gitRecentBranchFetchCallbacksLock);
    [_gitRecentBranchFetchCallbacks addObject:box];
    os_unfair_lock_unlock(&_gitRecentBranchFetchCallbacksLock);
    [[_connectionToService remoteObjectProxy] fetchRecentBranchesAt:path
                                                              count:maxCount
                                                         completion:^(NSArray<NSString *> *_Nonnull branches) {
                                                             [self didGetRecentBranches:branches box:box];
                                                         }];
}

// Runs on some random queue
- (void)didGetRecentBranches:(NSArray<NSString *> *)branches box:(iTermGitRecentBranchesBox *)box {
    os_unfair_lock_lock(&_gitRecentBranchFetchCallbacksLock);
    BOOL found = [_gitRecentBranchFetchCallbacks containsObject:box];
    if (found) {
        [_gitRecentBranchFetchCallbacks removeObject:box];
    }
    os_unfair_lock_unlock(&_gitRecentBranchFetchCallbacksLock);
    if (!found) {
        return;
    }
    dispatch_async(dispatch_get_main_queue(), ^{
        box.block(branches);
    });
}

- (id<iTermCancelable>)findExistingFileWithPrefix:(NSString *)prefix
                                           suffix:(NSString *)suffix
                                 workingDirectory:(NSString *)workingDirectory
                                   trimWhitespace:(BOOL)trimWhitespace
                                    pathsToIgnore:(NSString *)pathsToIgnore
                               allowNetworkMounts:(BOOL)allowNetworkMounts
                                       completion:(void (^)(NSString *path, int prefixChars, int suffixChars,
                                                            BOOL workingDirectoryIsLocal))completion {
    static int nextRequestID;
    const int reqid = nextRequestID++;
    __weak __typeof(self) weakSelf = self;
    id<pidinfoProtocol> proxy = [_connectionToService remoteObjectProxy];
    __block BOOL canceled = NO;
    DLog(@"[%d] Main app request %@ ... %@]", reqid, [prefix substringFromIndex:MAX(10, prefix.length) - 10],
         [suffix substringToIndex:MIN(suffix.length, 10)]);
    DLog(@"prefix=%@", prefix);
    DLog(@"suffix=%@", suffix);
    DLog(@"workingDirectory=%@", workingDirectory);
    DLog(@"trimWhitespace=%@", @(trimWhitespace));
    DLog(@"pathsToIgnore=%@", pathsToIgnore);
    DLog(@"allowNetworkMounts=%@ reqid=%@", @(allowNetworkMounts), @(reqid));
    [proxy
        findExistingFileWithPrefix:prefix
                            suffix:suffix
                  workingDirectory:workingDirectory
                    trimWhitespace:trimWhitespace
                     pathsToIgnore:pathsToIgnore
                allowNetworkMounts:allowNetworkMounts
                             reqid:reqid
                             reply:^(NSString *path, int prefixChars, int suffixChars, BOOL workingDirectoryIsLocal) {
                                 dispatch_async(dispatch_get_main_queue(), ^{
                                     if (canceled) {
                                         DLog(@"Drop result for %d because canceled", reqid);
                                         return;
                                     }
                                     DLog(@"Accept result for %d", reqid);
                                     completion(path, prefixChars, suffixChars, workingDirectoryIsLocal);
                                 });
                             }];

    iTermBlockCanceller *cancel = [[iTermBlockCanceller alloc] initWithBlock:^{
        canceled = YES;
        [weakSelf cancelFindExistingFileRequest:reqid];
    }];
    return cancel;
}

- (void)cancelFindExistingFileRequest:(int)reqid {
    id<pidinfoProtocol> proxy = [_connectionToService remoteObjectProxy];
    [proxy cancelFindExistingFileRequest:reqid
                                   reply:^{
                                   }];
}

- (void)executeShellCommand:(NSString *)command
                       args:(NSArray<NSString *> *)args
                        dir:(NSString *)dir
                        env:(NSDictionary<NSString *, NSString *> *)env
                 completion:(void (^)(NSData *stdout, NSData *stderr, uint8_t status,
                                      NSTaskTerminationReason reason))completion {
    DLog(@"executeShellCommand:%@ args:%@ dir:%@ env:%@", command, args, dir, env);
    id<pidinfoProtocol> proxy = [_connectionToService remoteObjectProxy];
    [proxy executeShellCommand:command
                          args:args
                           dir:dir
                           env:env
                         reply:^(NSData *_Nonnull stdout, NSData *_Nonnull stderr, uint8_t status,
                                 NSTaskTerminationReason reason) {
                             dispatch_async(dispatch_get_main_queue(), ^{
                                 completion(stdout, stderr, status, reason);
                             });
                         }];
}

- (void)fetchDirectoryListingOfPath:(NSString *)path
                         completion:(void (^)(NSArray<iTermDirectoryEntry *> *entries))completion {
    DLog(@"fetchDirectoryListingOfPath:%@", path);
    id<pidinfoProtocol> proxy = [_connectionToService remoteObjectProxy];
    [proxy fetchDirectoryListingOfPath:path
                            completion:^(NSArray<iTermDirectoryEntry *> *entries) {
                                dispatch_async(dispatch_get_main_queue(), ^{
                                    completion(entries);
                                });
                            }];
}

@end
