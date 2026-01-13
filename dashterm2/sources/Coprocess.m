//
//  Coprocess.m
//  iTerm
//
//  Created by George Nachman on 9/24/11.
//  Copyright 2011 Georgetech. All rights reserved.
//

#import "Coprocess.h"

#import <os/lock.h>

#import "DebugLogging.h"
#import "NSArray+iTerm.h"
#import "NSDictionary+iTerm.h"

#include <errno.h>
#include <stdlib.h>
#include <unistd.h>

const int kMaxInputBufferSize = 1024;
const int kMaxOutputBufferSize = 1024;

static NSString *kCoprocessMruKey = @"Coprocess MRU";
static NSString *const iTermCoprocessCommandsToIgnoreErrorOutputPrefsKey =
    @"NoSyncCoprocessCommandsToIgnoreErrorOutput";

static void CoprocessDup2OrDie(int fromFd, int toFd, const char *label) {
    if (dup2(fromFd, toFd) < 0) {
        const char *operation = label ?: "fd";
        fprintf(stderr, "## dup2(%s -> %d) failed: %s ##\n", operation, toFd, strerror(errno));
        _exit(EXIT_FAILURE);
    }
}

@interface Coprocess ()
@property (nonatomic, copy) NSString *command;
@property (atomic, copy) NSString *savedErrors;
@end

@implementation Coprocess {
    // When this is set, writing is no longer an option (probably because the
    // coprocess terminated).  This is different than eof_, which indicates that
    // reading is no longer an option and the coprocess is well and truly dead.
    BOOL writePipeClosed_;

    NSMutableString *_errors;
    dispatch_group_t _stderrGroup;
    os_unfair_lock _lock; // Performance optimization: os_unfair_lock has ~10x lower overhead than @synchronized
}

@synthesize pid = pid_;
@synthesize outputFd = outputFd_;
@synthesize inputFd = inputFd_;
@synthesize inputBuffer = inputBuffer_;
@synthesize outputBuffer = outputBuffer_;
@synthesize eof = eof_;
@synthesize mute = mute_;

+ (void)addCommandToMostRecentlyUsed:(NSString *)command {
    if (!command) {
        return;
    }
    NSArray *oldMru = [[NSUserDefaults standardUserDefaults] stringArrayForKey:kCoprocessMruKey];
    NSMutableArray *newMru;
    if (oldMru) {
        newMru = [oldMru mutableCopy];
    } else {
        // kMaxMru is 20, start with reasonable capacity
        newMru = [NSMutableArray arrayWithCapacity:20];
    }
    [newMru removeObject:command];
    [newMru insertObject:command atIndex:0];
    const int kMaxMru = 20;
    while (newMru.count > kMaxMru) {
        [newMru removeLastObject];
    }
    [[NSUserDefaults standardUserDefaults] setObject:newMru forKey:kCoprocessMruKey];
}

+ (NSArray *)mostRecentlyUsedCommands {
    return [[NSUserDefaults standardUserDefaults] stringArrayForKey:kCoprocessMruKey];
}

+ (Coprocess *)launchedCoprocessWithCommand:(NSString *)command
                                environment:(NSDictionary<NSString *, NSString *> *)environment {
    char **replacementEnvironment = NULL;
    if (environment) {
        NSMutableDictionary *combined = [[[NSProcessInfo processInfo] environment] mutableCopy];
        [environment
            enumerateKeysAndObjectsUsingBlock:^(NSString *_Nonnull key, NSString *_Nonnull obj, BOOL *_Nonnull stop) {
                combined[key] = obj;
            }];
        NSArray<NSString *> *kvps = [combined.allKeys mapWithBlock:^id _Nonnull(id _Nonnull key) {
            return [NSString stringWithFormat:@"%@=%@", key, combined[key]];
        }];
        replacementEnvironment = [kvps nullTerminatedCStringArray];
    }
    [Coprocess addCommandToMostRecentlyUsed:command];
    int inputPipe[2];
    int outputPipe[2];
    int errorPipe[2];
    // BUG-2914: Check pipe() return values - failed pipes would leave uninitialized fds
    if (pipe(inputPipe) != 0 || pipe(outputPipe) != 0 || pipe(errorPipe) != 0) {
        NSAlert *alert = [[NSAlert alloc] init];
        alert.messageText = @"Failed to create pipes for coprocess.";
        alert.informativeText = [NSString stringWithFormat:@"pipe() error: %s", strerror(errno)];
        [alert addButtonWithTitle:@"OK"];
        [alert runModal];
        return nil;
    }
    signal(SIGPIPE, SIG_IGN);
    const int maxfd = getdtablesize();
    pid_t pid = fork();
    if (pid == 0) {
        signal(SIGCHLD, SIG_DFL);
        signal(SIGPIPE, SIG_DFL);

        CoprocessDup2OrDie(inputPipe[0], STDIN_FILENO, "stdin");
        close(inputPipe[0]);
        close(inputPipe[1]);

        CoprocessDup2OrDie(outputPipe[1], STDOUT_FILENO, "stdout");
        close(outputPipe[0]);
        close(outputPipe[1]);

        CoprocessDup2OrDie(errorPipe[1], STDERR_FILENO, "stderr");
        close(errorPipe[0]);
        close(errorPipe[1]);

        // BUG-10365: Close all fds from 3 to maxfd.
        // The pipe fds have already been dup2'd to 0/1/2 and closed above,
        // so we simply close everything >= 3 without checking stale fd values.
        for (int i = 3; i < maxfd; i++) {
            close(i);
        }

        if (replacementEnvironment) {
            extern char **environ;
            environ = replacementEnvironment;
        }
        execl("/bin/sh", "/bin/sh", "-c", [command UTF8String], 0);

        /* exec error */
        fprintf(stderr, "## exec failed %s for command /bin/sh -c %s##\n", strerror(errno), [command UTF8String]);
        _exit(-1);
    } else if (pid < (pid_t)0) {
        // BUG-1461: Close all 6 pipe FDs on fork failure to prevent FD leak
        close(inputPipe[0]);
        close(inputPipe[1]);
        close(outputPipe[0]);
        close(outputPipe[1]);
        close(errorPipe[0]);
        close(errorPipe[1]);
        // BUG-1467: Free environment array on fork failure
        if (replacementEnvironment) {
            iTermFreeeNullTerminatedCStringArray(replacementEnvironment);
        }
        NSAlert *alert = [[NSAlert alloc] init];
        alert.messageText = @"Failed to launch coprocess.";
        [alert addButtonWithTitle:@"OK"];
        [alert runModal];
        return nil;
    }

    close(inputPipe[0]);
    close(outputPipe[1]);
    close(errorPipe[1]);
    if (replacementEnvironment) {
        iTermFreeeNullTerminatedCStringArray(replacementEnvironment);
    }

    return [Coprocess coprocessWithPid:pid
                               command:command
                              outputFd:inputPipe[1]
                               inputFd:outputPipe[0]
                               errorFd:errorPipe[0]];
}

+ (Coprocess *)coprocessWithPid:(pid_t)pid
                        command:(NSString *)command
                       outputFd:(int)outputFd
                        inputFd:(int)inputFd
                        errorFd:(int)errorFd {
    Coprocess *result = [[Coprocess alloc] init];
    result.pid = pid;
    result.outputFd = outputFd;
    result.inputFd = inputFd;

    // Make sure the file descriptors are non-blocking.
    // BUG-1464: Check fcntl return values - if F_GETFL fails, assume 0 flags.
    // If F_SETFL fails, log but continue (non-blocking is preferred but not critical).
    int flags = fcntl(outputFd, F_GETFL);
    if (flags < 0) {
        flags = 0; // Use 0 flags if F_GETFL fails
    }
    if (fcntl(outputFd, F_SETFL, flags | O_NONBLOCK) < 0) {
        DLog(@"Warning: fcntl F_SETFL failed for outputFd: %s", strerror(errno));
    }
    flags = fcntl(inputFd, F_GETFL);
    if (flags < 0) {
        flags = 0;
    }
    if (fcntl(inputFd, F_SETFL, flags | O_NONBLOCK) < 0) {
        DLog(@"Warning: fcntl F_SETFL failed for inputFd: %s", strerror(errno));
    }
    result->_stderrGroup = dispatch_group_create();
    [result monitorErrorsOnFileDescriptor:errorFd];
    result.command = command;
    return result;
}

+ (void)setSilentlyIgnoreErrors:(BOOL)shouldIgnore fromCommand:(NSString *)command {
    NSArray *array =
        [[NSUserDefaults standardUserDefaults] objectForKey:iTermCoprocessCommandsToIgnoreErrorOutputPrefsKey] ?: @[];
    if (shouldIgnore) {
        array = [array arrayByAddingObject:command];
        array = [[NSSet setWithArray:array] allObjects];
    } else {
        array = [array arrayByRemovingObject:command];
    }
    [[NSUserDefaults standardUserDefaults] setObject:array forKey:iTermCoprocessCommandsToIgnoreErrorOutputPrefsKey];
}

+ (BOOL)shouldIgnoreErrorsFromCommand:(NSString *)command {
    if (!command) {
        return YES;
    }
    return ([[[NSUserDefaults standardUserDefaults] objectForKey:iTermCoprocessCommandsToIgnoreErrorOutputPrefsKey]
        containsObject:command]);
}

- (instancetype)init {
    self = [super init];
    if (self) {
        inputBuffer_ = [[NSMutableData alloc] init];
        outputBuffer_ = [[NSMutableData alloc] init];
        // BUG-2753: Initialize FDs to -1 so dealloc knows if they need closing
        outputFd_ = -1;
        inputFd_ = -1;
        _lock = OS_UNFAIR_LOCK_INIT;
    }
    return self;
}

// BUG-2753: Ensure file descriptors are closed on dealloc to prevent leaks
// if the object is deallocated without terminate being called explicitly.
- (void)dealloc {
    if (outputFd_ >= 0) {
        close(outputFd_);
    }
    if (inputFd_ >= 0) {
        close(inputFd_);
    }
}

- (void)monitorErrorsOnFileDescriptor:(int)errorFd {
    dispatch_queue_t queue = dispatch_queue_create("com.dashterm.dashterm2.coprocess-errors", 0);
    _errors = [[NSMutableString alloc] init];
    dispatch_group_t group = _stderrGroup;
    dispatch_group_enter(group);
    dispatch_async(queue, ^{
        FILE *f = fdopen(errorFd, "r");
        // BUG-1463: fdopen can fail if errorFd is invalid
        if (!f) {
            close(errorFd);
            self.savedErrors = @"";
            dispatch_group_leave(group);
            return;
        }
        while (1) {
            size_t size = 0;
            char *line = fgetln(f, &size);
            if (line == NULL) {
                break;
            }
            if (size > 0) {
                NSData *data = [NSData dataWithBytes:line length:size];
                NSString *string = [[NSString alloc] initWithData:data encoding:NSUTF8StringEncoding];
                os_unfair_lock_lock(&self->_lock);
                if (self->_errors.length < 100000) {
                    [self->_errors appendFormat:@"%@\n", string];
                    if (self->_errors.length >= 100000) {
                        [self->_errors appendString:@"\n-- output truncated --\n"];
                    }
                }
                os_unfair_lock_unlock(&self->_lock);
            }
        }
        // BUG-1463: Close the FILE to release resources (fclose also closes errorFd)
        fclose(f);
        self.savedErrors = self->_errors;
        dispatch_group_leave(group);
    });
}

- (int)write {
    if (self.pid < 0 || writePipeClosed_) {
        return -1;
    }
    int fd = [self writeFileDescriptor];
    int n = write(fd, [outputBuffer_ bytes], [outputBuffer_ length]);

    if (n < 0 && (!(errno == EAGAIN || errno == EINTR))) {
        writePipeClosed_ = YES;
    } else if (n == 0) {
        writePipeClosed_ = YES;
    } else if (n > 0) {
        [outputBuffer_ replaceBytesInRange:NSMakeRange(0, n) withBytes:"" length:0];
    }
    return n;
}

- (int)read {
    if (self.pid < 0) {
        return -1;
    }
    int rc = 0;
    int fd = [self readFileDescriptor];
    while (inputBuffer_.length < kMaxInputBufferSize) {
        char buffer[1024];
        int n = read(fd, buffer, sizeof(buffer));
        if (n == 0) {
            rc = 0;
            eof_ = YES;
            break;
        } else if (n < 0) {
            if (errno != EAGAIN && errno != EINTR) {
                eof_ = YES;
                rc = n;
                break;
            }
        } else {
            rc += n;
            [inputBuffer_ appendBytes:buffer length:n];
        }
        if (n < sizeof(buffer)) {
            break;
        }
    }
    return rc;
}

- (BOOL)wantToRead {
    return self.pid >= 0 && !eof_ && (inputBuffer_.length < kMaxInputBufferSize);
}

- (BOOL)wantToWrite {
    return self.pid >= 0 && !eof_ && !writePipeClosed_ && (outputBuffer_.length > 0);
}

- (void)mainProcessDidTerminate {
    [self terminate];
}

- (void)terminate {
    if (self.pid > 0) {
        pid_t pidToReap = self.pid;
        kill(pidToReap, SIGTERM);
        close(self.outputFd);
        close(self.inputFd);
        self.outputFd = -1;
        self.inputFd = -1;
        self.pid = -1;

        // BUG-1469: Reap the child process to prevent zombie.
        // Do this asynchronously to avoid blocking main thread if child doesn't exit immediately.
        dispatch_async(dispatch_get_global_queue(DISPATCH_QUEUE_PRIORITY_LOW, 0), ^{
            int status;
            // Give the process time to terminate gracefully
            for (int i = 0; i < 10; i++) {
                pid_t result = waitpid(pidToReap, &status, WNOHANG);
                if (result == pidToReap || result == -1) {
                    return; // Reaped or already reaped
                }
                usleep(100000); // 100ms
            }
            // If still not terminated, force kill and reap
            kill(pidToReap, SIGKILL);
            waitpid(pidToReap, &status, 0); // Blocking wait
        });

        os_unfair_lock_lock(&_lock);
        dispatch_group_t group = _stderrGroup;
        os_unfair_lock_unlock(&_lock);
        dispatch_group_notify(group, dispatch_get_main_queue(), ^{
            if (self.savedErrors.length > 0) {
                [self.delegate coprocess:self didTerminateWithErrorOutput:self.savedErrors];
            }
        });
    }
}

- (int)readFileDescriptor {
    return self.inputFd;
}

- (int)writeFileDescriptor {
    return self.outputFd;
}

@end
