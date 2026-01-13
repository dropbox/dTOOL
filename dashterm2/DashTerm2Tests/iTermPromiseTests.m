//
//  iTermPromiseTests.m
//  DashTerm2XCTests
//
//  Created by George Nachman on 2/10/20.
//

#import <XCTest/XCTest.h>
#import "iTermPromise.h"

@interface iTermPromiseTests : XCTestCase

@end

@implementation iTermPromiseTests {
    NSError *_standardError;
}

- (void)setUp {
    _standardError = [[NSError alloc] initWithDomain:@"com.dashterm.dashterm2.promise-tests"
                                                code:123
                                            userInfo:nil];
}

- (void)tearDown {
    [_standardError release];
}

- (void)testFulfillFollowedByThen {
    iTermPromise<NSNumber *> *promise = [iTermPromise promise:^(id<iTermPromiseSeal> seal) {
        [seal fulfill:@123];
    }];
    __block BOOL ranThen = NO;
    [promise then:^(NSNumber * _Nonnull value) {
        XCTAssertEqualObjects(value, @123);
        ranThen = YES;
    }];
    XCTAssertTrue(ranThen);
}

- (void)testFulfillFollowedByCatchError {
    iTermPromise<NSNumber *> *promise = [iTermPromise promise:^(id<iTermPromiseSeal> seal) {
        [seal reject:_standardError];
    }];
    __block BOOL ranThen = NO;
    [promise catchError:^(NSError *error) {
        XCTAssertEqual(error, _standardError);
        ranThen = YES;
    }];
    XCTAssertTrue(ranThen);
}

- (void)testThenFollowedByFulfill {
    __block id<iTermPromiseSeal> savedSeal = nil;
    iTermPromise<NSNumber *> *promise = [iTermPromise promise:^(id<iTermPromiseSeal> seal) {
        savedSeal = [[seal retain] autorelease];
    }];

    __block BOOL ranThen = NO;
    [promise then:^(NSNumber * _Nonnull value) {
        XCTAssertEqualObjects(value, @123);
        ranThen = YES;
    }];

    XCTAssertFalse(ranThen);

    [savedSeal fulfill:@123];
    XCTAssertTrue(ranThen);
}

- (void)testThenFollowedByCatchError {
    __block id<iTermPromiseSeal> savedSeal = nil;
    iTermPromise<NSNumber *> *promise = [iTermPromise promise:^(id<iTermPromiseSeal> seal) {
        savedSeal = [[seal retain] autorelease];
    }];

    __block BOOL ranThen = NO;
    [promise catchError:^(NSError *value) {
        XCTAssertEqual(value, _standardError);
        ranThen = YES;
    }];

    XCTAssertFalse(ranThen);

    [savedSeal reject:_standardError];
    XCTAssertTrue(ranThen);
}

- (void)testFulfillFollowedByChain {
    iTermPromise<NSNumber *> *promise1 = [iTermPromise promise:^(id<iTermPromiseSeal> seal) {
        [seal fulfill:@123];
    }];
    __block int count = 0;
    iTermPromise<NSNumber *> *promise2 = [promise1 then:^(NSNumber *value) {
        XCTAssertEqualObjects(value, @123);
        count++;
    }];
    iTermPromise<NSNumber *> *promise3 = [promise2 then:^(NSNumber *value) {
        XCTAssertEqualObjects(value, @123);
        count++;
    }];
    iTermPromise<NSNumber *> *promise4 = [promise3 catchError:^(NSError *error) {
        XCTFail(@"%@", error);
    }];
    [promise4 then:^(NSNumber * value) {
        XCTAssertEqualObjects(value, @123);
        count++;
    }];
    XCTAssertEqual(count, 3);
}

- (void)testChainFollowedByFulfill {
    __block id<iTermPromiseSeal> savedSeal = nil;
    iTermPromise<NSNumber *> *promise1 = [iTermPromise promise:^(id<iTermPromiseSeal> seal) {
        savedSeal = [[seal retain] autorelease];
    }];
    __block int count = 0;
    iTermPromise<NSNumber *> *promise2 = [promise1 then:^(NSNumber * _Nonnull value) {
        XCTAssertEqualObjects(value, @123);
        count++;
    }];
    iTermPromise<NSNumber *> *promise3 = [promise2 then:^(NSNumber *value) {
        XCTAssertEqualObjects(value, @123);
        count++;
    }];
    iTermPromise<NSNumber *> *promise4 = [promise3 catchError:^(NSError *error) {
        XCTFail(@"%@", error);
    }];
    [promise4 then:^(NSNumber * value) {
        XCTAssertEqualObjects(value, @123);
        count++;
    }];
    XCTAssertEqual(count, 0);

    [savedSeal fulfill:@123];
    XCTAssertEqual(count, 3);
}

- (void)testRejectFollowedByChain {
    iTermPromise<NSNumber *> *promise1 = [iTermPromise promise:^(id<iTermPromiseSeal> seal) {
        [seal reject:_standardError];
    }];
    __block int count = 0;
    iTermPromise<NSNumber *> *promise2 = [promise1 then:^(NSNumber * _Nonnull value) {
        XCTFail(@"%@", value);
    }];
    iTermPromise<NSNumber *> *promise3 = [promise2 catchError:^(NSError *error) {
        XCTAssertEqual(error, _standardError);
        count++;
    }];
    iTermPromise<NSNumber *> *promise4 = [promise3 catchError:^(NSError *error) {
        XCTAssertEqual(error, _standardError);
        count++;
    }];
    [promise4 then:^(NSNumber * value) {
        XCTFail(@"Shouldn't be called");
    }];
    XCTAssertEqual(count, 2);
}

- (void)testChainFollowedByReject {
    __block id<iTermPromiseSeal> savedSeal = nil;
    iTermPromise<NSNumber *> *promise1 = [iTermPromise promise:^(id<iTermPromiseSeal> seal) {
        savedSeal = [[seal retain] autorelease];
    }];
    __block int count = 0;
    iTermPromise<NSNumber *> *promise2 = [promise1 then:^(NSNumber * _Nonnull value) {
        XCTFail(@"%@", value);
    }];
    iTermPromise<NSNumber *> *promise3 = [promise2 catchError:^(NSError *error) {
        XCTAssertEqual(error, _standardError);
        count++;
    }];
    iTermPromise<NSNumber *> *promise4 = [promise3 catchError:^(NSError *error) {
        XCTAssertEqual(error, _standardError);
        count++;
    }];
    [promise4 then:^(NSNumber * value) {
        XCTFail(@"Shouldn't be called");
    }];
    XCTAssertEqual(count, 0);

    [savedSeal reject:_standardError];
    XCTAssertEqual(count, 2);
}

// BUG-1620: Test waitWithTimeout returns value when promise is already fulfilled
- (void)test_BUG_1620_waitWithTimeoutAlreadyFulfilled {
    iTermPromise<NSNumber *> *promise = [iTermPromise promise:^(id<iTermPromiseSeal> seal) {
        [seal fulfill:@42];
    }];

    // Should return immediately since value is already set
    iTermOr<NSNumber *, NSError *> *result = [promise waitWithTimeout:1.0];

    XCTAssertTrue(result.hasFirst, @"Should have fulfilled value");
    XCTAssertEqualObjects(result.maybeFirst, @42, @"Should get the fulfilled value");
}

// BUG-1620: Test waitWithTimeout returns error on timeout
- (void)test_BUG_1620_waitWithTimeoutTimesOut {
    __block id<iTermPromiseSeal> savedSeal = nil;
    iTermPromise<NSNumber *> *promise = [iTermPromise promise:^(id<iTermPromiseSeal> seal) {
        savedSeal = [[seal retain] autorelease];
        // Never fulfill - will timeout
    }];

    // Should timeout after 0.1 seconds
    iTermOr<NSNumber *, NSError *> *result = [promise waitWithTimeout:0.1];

    XCTAssertTrue(result.hasSecond, @"Should have error");
    XCTAssertEqual(result.maybeSecond.code, iTermPromiseErrorCodeTimeout, @"Should be timeout error");

    // Fulfill after timeout to clean up (tests the race condition fix - this should not crash)
    [savedSeal fulfill:@123];
}

// BUG-1620: Stress test - race between timeout and callback completion.
// This tests the specific crash where dispatch_group_leave was called twice:
// once by the callback and once by the timeout code path.
- (void)test_BUG_1620_waitWithTimeoutRaceCondition {
    // Run multiple times to increase chance of hitting the race
    for (int i = 0; i < 100; i++) {
        __block id<iTermPromiseSeal> savedSeal = nil;
        iTermPromise<NSNumber *> *promise = [iTermPromise promise:^(id<iTermPromiseSeal> seal) {
            savedSeal = [[seal retain] autorelease];
        }];

        // Use a very short timeout to maximize race window
        dispatch_async(dispatch_get_global_queue(QOS_CLASS_USER_INITIATED, 0), ^{
            // Fulfill from background thread right around when timeout fires
            usleep(50000);  // 50ms - may be before or after the 100ms timeout
            [savedSeal fulfill:@(i)];
        });

        // Wait with short timeout - may complete or timeout depending on race
        iTermOr<NSNumber *, NSError *> *result = [promise waitWithTimeout:0.1];

        // Either result is valid - the important thing is we don't crash
        XCTAssertTrue(result.hasFirst || result.hasSecond,
                      @"Should have either fulfilled value or timeout error, not crash");
    }
}

// BUG-1620: Test wait (infinite timeout) returns value when fulfilled
- (void)test_BUG_1620_waitInfiniteReturnsWhenFulfilled {
    __block id<iTermPromiseSeal> savedSeal = nil;
    iTermPromise<NSNumber *> *promise = [iTermPromise promise:^(id<iTermPromiseSeal> seal) {
        savedSeal = [[seal retain] autorelease];
    }];

    // Fulfill from background thread after short delay
    dispatch_async(dispatch_get_global_queue(QOS_CLASS_USER_INITIATED, 0), ^{
        usleep(10000);  // 10ms
        [savedSeal fulfill:@99];
    });

    // Should wait until fulfilled
    iTermOr<NSNumber *, NSError *> *result = [promise wait];

    XCTAssertTrue(result.hasFirst, @"Should have fulfilled value");
    XCTAssertEqualObjects(result.maybeFirst, @99, @"Should get the fulfilled value");
}

@end
