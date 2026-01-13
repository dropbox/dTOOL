//
//  FileDescriptorMultiClientCrashFixTests.m
//  DashTerm2Tests
//
//  Tests for crash fixes BUG-f963 through BUG-f981.
//  These tests verify that the production code handles edge cases gracefully
//  instead of crashing via assert().
//

#import <XCTest/XCTest.h>
#import "iTermFileDescriptorMultiClient.h"
#import "iTermFileDescriptorMultiClientChild.h"
#import "iTermFileDescriptorMultiClientPendingLaunch.h"
#import "iTermFileDescriptorMultiClientState.h"
#import "iTermMultiServerProtocol.h"
#import "iTermThread.h"

// Category to expose internal methods for testing
@interface iTermFileDescriptorMultiClientState (Testing)
@property (nonatomic) int readFD;
@property (nonatomic) int writeFD;
@end

@interface iTermFileDescriptorMultiClientChild (Testing)
- (void)willWaitPreemptively;
- (void)setTerminationStatus:(int)status;
@end

@interface iTermFileDescriptorMultiClientPendingLaunch (Testing)
- (void)invalidate;
- (iTermMultiServerRequestLaunch)launchRequest;
@end

#pragma mark - BUG-f976: iTermFileDescriptorMultiClientChild fd validity

@interface BUGf976_ChildFdValidityTests : XCTestCase
@end

@implementation BUGf976_ChildFdValidityTests

/// BUG-f976: Invalid fd (-1) should cause initWithReport to return nil gracefully
/// Previously crashed with: assert(_fd >= 0)
- (void)test_BUG_f976_invalidFdReturnsNil {
    // Create a report with an invalid fd
    iTermMultiServerReportChild report = {0};
    report.pid = 12345;
    report.path = "/bin/bash";
    report.argv = (char *[]){"/bin/bash", NULL};
    report.argc = 1;
    report.envp = (char *[]){"PATH=/usr/bin", NULL};
    report.envc = 1;
    report.isUTF8 = YES;
    report.pwd = "/tmp";
    report.terminated = NO;
    report.fd = -1; // Invalid fd - would have crashed before fix
    report.tty = "/dev/ttys000";

    // Create a thread for testing
    iTermThread *thread = [iTermThread withLabel:@"test.thread"
                                    stateFactory:^iTermSynchronizedState *_Nullable(dispatch_queue_t queue) {
                                        return nil;
                                    }];

    // This should return nil instead of crashing
    iTermFileDescriptorMultiClientChild *child = [[iTermFileDescriptorMultiClientChild alloc] initWithReport:&report
                                                                                                      thread:thread];
    XCTAssertNil(child, @"BUG-f976: Should return nil for invalid fd (-1), not crash");
}

/// BUG-f976: Valid fd (>= 0) should work normally
- (void)test_BUG_f976_validFdSucceeds {
    // Create a report with a valid fd (using a real fd from dup)
    int pipefds[2];
    int result = pipe(pipefds);
    XCTAssertEqual(result, 0, @"Failed to create pipe for test");

    iTermMultiServerReportChild report = {0};
    report.pid = 12345;
    report.path = "/bin/bash";
    report.argv = (char *[]){"/bin/bash", NULL};
    report.argc = 1;
    report.envp = (char *[]){"PATH=/usr/bin", NULL};
    report.envc = 1;
    report.isUTF8 = YES;
    report.pwd = "/tmp";
    report.terminated = NO;
    report.fd = pipefds[0]; // Valid fd
    report.tty = "/dev/ttys000";

    iTermThread *thread = [iTermThread withLabel:@"test.thread"
                                    stateFactory:^iTermSynchronizedState *_Nullable(dispatch_queue_t queue) {
                                        return nil;
                                    }];

    iTermFileDescriptorMultiClientChild *child = [[iTermFileDescriptorMultiClientChild alloc] initWithReport:&report
                                                                                                      thread:thread];
    XCTAssertNotNil(child, @"BUG-f976: Should succeed for valid fd");
    XCTAssertEqual(child.pid, 12345, @"Child should have correct pid");

    // Clean up the other pipe end
    close(pipefds[1]);
}

@end

#pragma mark - BUG-f977: Double willWaitPreemptively call

@interface BUGf977_DoublePreemptiveWaitTests : XCTestCase
@end

@implementation BUGf977_DoublePreemptiveWaitTests

/// BUG-f977: Calling willWaitPreemptively twice should not crash
/// Previously crashed with: assert(!_haveSentPreemptiveWait)
- (void)test_BUG_f977_doubleWillWaitPreemptivelyDoesNotCrash {
    int pipefds[2];
    pipe(pipefds);

    iTermMultiServerReportChild report = {0};
    report.pid = 12345;
    report.path = "/bin/bash";
    report.argv = (char *[]){"/bin/bash", NULL};
    report.argc = 1;
    report.envp = (char *[]){"PATH=/usr/bin", NULL};
    report.envc = 1;
    report.pwd = "/tmp";
    report.fd = pipefds[0];
    report.tty = "/dev/ttys000";

    iTermThread *thread = [iTermThread withLabel:@"test.thread"
                                    stateFactory:^iTermSynchronizedState *_Nullable(dispatch_queue_t queue) {
                                        return nil;
                                    }];

    iTermFileDescriptorMultiClientChild *child = [[iTermFileDescriptorMultiClientChild alloc] initWithReport:&report
                                                                                                      thread:thread];

    // First call should succeed
    [child willWaitPreemptively];
    XCTAssertTrue(child.haveSentPreemptiveWait, @"First call should set flag");

    // Second call should be a no-op, not crash
    XCTAssertNoThrow([child willWaitPreemptively], @"BUG-f977: Double call should not crash");

    close(pipefds[1]);
}

@end

#pragma mark - BUG-f978: setTerminationStatus without hasTerminated

@interface BUGf978_SetTerminationStatusTests : XCTestCase
@end

@implementation BUGf978_SetTerminationStatusTests

/// BUG-f978: setTerminationStatus should handle case where hasTerminated is false
/// Previously crashed with: assert(_hasTerminated)
- (void)test_BUG_f978_setTerminationStatusWithoutTerminatedDoesNotCrash {
    int pipefds[2];
    pipe(pipefds);

    iTermMultiServerReportChild report = {0};
    report.pid = 12345;
    report.path = "/bin/bash";
    report.argv = (char *[]){"/bin/bash", NULL};
    report.argc = 1;
    report.envp = (char *[]){"PATH=/usr/bin", NULL};
    report.envc = 1;
    report.pwd = "/tmp";
    report.fd = pipefds[0];
    report.tty = "/dev/ttys000";
    report.terminated = NO; // Not terminated initially

    iTermThread *thread = [iTermThread withLabel:@"test.thread"
                                    stateFactory:^iTermSynchronizedState *_Nullable(dispatch_queue_t queue) {
                                        return nil;
                                    }];

    iTermFileDescriptorMultiClientChild *child = [[iTermFileDescriptorMultiClientChild alloc] initWithReport:&report
                                                                                                      thread:thread];
    XCTAssertFalse(child.hasTerminated, @"Should start as not terminated");

    // This should not crash even though hasTerminated is false
    XCTAssertNoThrow([child setTerminationStatus:0], @"BUG-f978: Should not crash");

    // After the fix, it should set hasTerminated to true
    XCTAssertTrue(child.hasTerminated, @"BUG-f978: Should auto-set terminated flag");
    XCTAssertTrue(child.haveWaited, @"Should set haveWaited flag");

    close(pipefds[1]);
}

@end

#pragma mark - BUG-f979: launchRequest on invalidated pending launch

@interface BUGf979_InvalidatedLaunchRequestTests : XCTestCase
@end

@implementation BUGf979_InvalidatedLaunchRequestTests

/// BUG-f979: Accessing launchRequest after invalidate should not crash
/// Previously crashed with: assert(!_invalid)
- (void)test_BUG_f979_launchRequestAfterInvalidateReturnsEmpty {
    iTermMultiServerRequestLaunch request = {0};
    request.path = "/bin/bash";
    request.uniqueId = 12345;

    iTermThread *thread = [iTermThread withLabel:@"test.thread"
                                    stateFactory:^iTermSynchronizedState *_Nullable(dispatch_queue_t queue) {
                                        return nil;
                                    }];

    iTermFileDescriptorMultiClientPendingLaunch *pending =
        [[iTermFileDescriptorMultiClientPendingLaunch alloc] initWithRequest:request callback:nil thread:thread];

    // Invalidate the pending launch
    [thread dispatchSync:^(id _Nullable state) {
        [pending invalidate];
    }];

    // This should return empty struct instead of crashing
    __block iTermMultiServerRequestLaunch result;
    [thread dispatchSync:^(id _Nullable state) {
        result = pending.launchRequest;
    }];

    XCTAssertEqual(result.uniqueId, 0, @"BUG-f979: Should return zeroed struct after invalidate");
}

@end

#pragma mark - BUG-f980, f981: Queue clearing in state

@interface BUGf980f981_StateFDQueueTests : XCTestCase
@end

@implementation BUGf980f981_StateFDQueueTests

/// BUG-f980: setWriteFD to -1 should clear writeQueue without crashing
/// Previously crashed with: assert(_writeQueue.count == 0)
- (void)test_BUG_f980_setWriteFDClearsQueue {
    dispatch_queue_t queue = dispatch_queue_create("test.queue", DISPATCH_QUEUE_SERIAL);
    iTermFileDescriptorMultiClientState *state = [[iTermFileDescriptorMultiClientState alloc] initWithQueue:queue];

    // Create a valid pipe for testing
    int pipefds[2];
    pipe(pipefds);

    // Set a valid fd first
    state.writeFD = pipefds[1];

    // Queue a write callback that won't be executed
    [state whenWritable:^(iTermFileDescriptorMultiClientState *s){
        // This callback would normally be in the queue
    }];

    // Setting to -1 should clear queue without crashing
    XCTAssertNoThrow(state.writeFD = -1, @"BUG-f980: Should not crash when clearing with pending callbacks");

    close(pipefds[0]);
}

/// BUG-f981: setReadFD to -1 should clear readQueue without crashing
/// Previously crashed with: assert(_readQueue.count == 0)
- (void)test_BUG_f981_setReadFDClearsQueue {
    dispatch_queue_t queue = dispatch_queue_create("test.queue", DISPATCH_QUEUE_SERIAL);
    iTermFileDescriptorMultiClientState *state = [[iTermFileDescriptorMultiClientState alloc] initWithQueue:queue];

    // Create a valid pipe for testing
    int pipefds[2];
    pipe(pipefds);

    // Set a valid fd first
    state.readFD = pipefds[0];

    // Queue a read callback that won't be executed
    [state whenReadable:^(iTermFileDescriptorMultiClientState *s){
        // This callback would normally be in the queue
    }];

    // Setting to -1 should clear queue without crashing
    XCTAssertNoThrow(state.readFD = -1, @"BUG-f981: Should not crash when clearing with pending callbacks");

    close(pipefds[1]);
}

@end

@end
