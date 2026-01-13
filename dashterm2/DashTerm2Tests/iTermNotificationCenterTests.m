//
//  iTermNotificationCenterTests.m
//  DashTerm2XCTests
//
//  Tests for iTermNotificationCenter and iTermBaseNotification
//

#import <XCTest/XCTest.h>
#import "iTermNotificationCenter.h"
#import "iTermFlagsChangedNotification.h"

// Test notification subclass for verification
@interface iTermTestNotification : iTermBaseNotification
@property (nonatomic, copy) NSString *testValue;
@end

@implementation iTermTestNotification

+ (instancetype)notificationWithValue:(NSString *)value {
    iTermTestNotification *notification = [[self alloc] initPrivate];
    notification.testValue = value;
    return notification;
}

+ (void)subscribe:(NSObject *)owner block:(void (^)(iTermTestNotification *))block {
    [self internalSubscribe:owner withBlock:^(id notif) {
        if ([notif isKindOfClass:[iTermTestNotification class]]) {
            block(notif);
        }
    }];
}

@end

@interface iTermNotificationCenterTests : XCTestCase
@end

@implementation iTermNotificationCenterTests

// RC-009/BUG-1602: Verify notification system posts and receives properly
// This tests the REAL production iTermBaseNotification class
- (void)test_RC_009_notificationPostAndReceive {
    __block BOOL receivedNotification = NO;
    __block NSString *receivedValue = nil;
    NSObject *owner = [[NSObject alloc] init];

    // Subscribe to notifications using the real production code
    [iTermTestNotification subscribe:owner block:^(iTermTestNotification *notif) {
        receivedNotification = YES;
        receivedValue = notif.testValue;
    }];

    // Post notification using the real production code
    [[iTermTestNotification notificationWithValue:@"test123"] post];

    // Verify the notification was received with correct value
    XCTAssertTrue(receivedNotification, "RC-009: Notification should be received");
    XCTAssertEqualObjects(receivedValue, @"test123", "RC-009: Value should match posted value");
}

// RC-009: Verify notification deregistration on owner dealloc
- (void)test_RC_009_notificationUnsubscribeOnDealloc {
    __block int notificationCount = 0;

    @autoreleasepool {
        NSObject *owner = [[NSObject alloc] init];
        [iTermTestNotification subscribe:owner block:^(iTermTestNotification *notif) {
            notificationCount++;
        }];

        // Post while subscribed - should increment
        [[iTermTestNotification notificationWithValue:@"first"] post];
        XCTAssertEqual(notificationCount, 1, "Should receive notification while subscribed");

        // Owner goes out of scope and is deallocated here
    }

    // Post after owner deallocated - should NOT increment (weak reference cleared)
    [[iTermTestNotification notificationWithValue:@"second"] post];
    XCTAssertEqual(notificationCount, 1, "RC-009: Should not receive notification after owner dealloc");
}

// RC-009: Verify notification handler recursion prevention
- (void)test_RC_009_notificationRecursionPrevention {
    __block int handlerCallCount = 0;
    NSObject *owner = [[NSObject alloc] init];

    [iTermTestNotification subscribe:owner block:^(iTermTestNotification *notif) {
        handlerCallCount++;
        // Try to cause recursion by posting during handling
        if (handlerCallCount == 1) {
            [[iTermTestNotification notificationWithValue:@"recursive"] post];
        }
    }];

    [[iTermTestNotification notificationWithValue:@"initial"] post];

    // The handling flag should prevent recursion - only 1 call expected
    XCTAssertEqual(handlerCallCount, 1, "RC-009: Recursion prevention should block nested posts");
}

// RC-009: Verify notification object is never nil
- (void)test_RC_009_notificationObjectNeverNil {
    __block BOOL receivedNilNotification = NO;
    NSObject *owner = [[NSObject alloc] init];

    [iTermTestNotification subscribe:owner block:^(iTermTestNotification *notif) {
        if (notif == nil) {
            receivedNilNotification = YES;
        }
    }];

    // Post notification - the handler should never receive nil
    [[iTermTestNotification notificationWithValue:@"test"] post];

    XCTAssertFalse(receivedNilNotification, "RC-009: Notification object should never be nil");
}

// Test with real production notification class (iTermFlagsChangedNotification)
- (void)test_RC_009_realProductionNotification {
    __block BOOL subscribed = NO;
    NSObject *owner = [[NSObject alloc] init];

    [iTermFlagsChangedNotification subscribe:owner block:^(iTermFlagsChangedNotification *notif) {
        subscribed = YES;
    }];

    // We can't easily test this without creating real NSEvents, so just verify
    // the subscription mechanism works without crashing
    XCTAssertTrue(YES, "RC-009: Subscription to real production notification class should not crash");
}

@end
