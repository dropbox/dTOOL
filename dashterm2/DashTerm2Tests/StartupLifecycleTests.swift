// StartupLifecycleTests.swift
// Tests for startup lifecycle bugs that are hard to catch otherwise
//
// These tests verify that key startup events happen in the correct order.
// They cannot fully test the app launch sequence, but they can verify
// the logic of individual components.

import XCTest
@testable import DashTerm2SharedARC

class StartupLifecycleTests: XCTestCase {

    // MARK: - Placeholder Window Tests

    /// BUG-STARTUP-1: Placeholder window was shown but never dismissed
    /// Root cause: applicationDidFinishLaunching is not called due to restorable state handling
    /// Fix: Dismiss placeholder in restoreWindows completion handler
    func test_placeholderWindow_dismissIsCalled() {
        // This is a documentation test - we can't easily test the full startup flow
        // but we can verify the placeholder window API works correctly

        // Verify showPlaceholder creates a window
        iTermStartupPlaceholderWindow.showPlaceholder()
        XCTAssertNotNil(iTermStartupPlaceholderWindow.sharedInstance(),
                        "showPlaceholder should create shared instance")

        // Verify dismissPlaceholder removes it
        iTermStartupPlaceholderWindow.dismissPlaceholder()

        // Give animation time to complete
        let expectation = self.expectation(description: "dismiss animation")
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.2) {
            expectation.fulfill()
        }
        wait(for: [expectation], timeout: 1.0)

        XCTAssertNil(iTermStartupPlaceholderWindow.sharedInstance(),
                     "dismissPlaceholder should clear shared instance")
    }

    /// Verify placeholder can be dismissed even if shown multiple times
    func test_placeholderWindow_idempotentDismiss() {
        // Call dismiss when nothing is shown - should not crash
        iTermStartupPlaceholderWindow.dismissPlaceholder()
        iTermStartupPlaceholderWindow.dismissPlaceholder()

        // Show then dismiss multiple times
        iTermStartupPlaceholderWindow.showPlaceholder()
        iTermStartupPlaceholderWindow.dismissPlaceholder()
        iTermStartupPlaceholderWindow.dismissPlaceholder()

        // Should not crash
        XCTAssertTrue(true, "Multiple dismiss calls should be safe")
    }

    // MARK: - Parser Configuration Tests

    /// BUG-STARTUP-2: Parser comparison was enabled by default, doubling overhead
    /// Root cause: dtermCoreParserComparisonEnabled defaulted to YES
    /// Fix: Changed to NO since dterm-core is validated
    func test_parserComparisonDisabledByDefault() {
        // Verify the comparison mode is OFF by default for performance
        let comparisonEnabled = iTermAdvancedSettingsModel.dtermCoreParserComparisonEnabled()
        XCTAssertFalse(comparisonEnabled,
                       "Parser comparison should be OFF by default - it doubles parsing overhead")
    }

    /// Verify dterm-core is the primary parser
    func test_dtermCoreIsDefaultParser() {
        let dtermEnabled = iTermAdvancedSettingsModel.dtermCoreEnabled()
        let dtermOutputEnabled = iTermAdvancedSettingsModel.dtermCoreParserOutputEnabled()

        XCTAssertTrue(dtermEnabled, "dterm-core should be enabled by default")
        XCTAssertTrue(dtermOutputEnabled, "dterm-core output should be used by default")
    }

    // MARK: - Profile Loading Tests

    /// Verify minimal profile init is used at startup
    func test_minimalProfileInit() {
        // ITAddressBookMgr should have a minimal init method
        // that creates just one default profile without disk I/O
        // This test verifies the method exists and doesn't crash
        ITAddressBookMgr.initializeMinimalForStartup()

        // Should have at least one profile after minimal init
        let profileCount = ProfileModel.sharedInstance().numberOfBookmarks()
        XCTAssertGreaterThan(profileCount, 0,
                            "Minimal init should create at least one default profile")
    }
}

// MARK: - Manual Test Procedures

/*
 MANUAL STARTUP TEST PROCEDURE:

 1. Kill any running DashTerm2 instances
 2. Clear app state: defaults delete com.dashterm.dashterm2
 3. Launch DashTerm2 from Spotlight
 4. VERIFY: Dark placeholder window appears immediately (<100ms)
 5. VERIFY: Placeholder shows blinking cursor
 6. VERIFY: Placeholder transitions smoothly to real terminal
 7. VERIFY: Startup time logged as <0.5s in Console.app
 8. Type 'ls' and press Enter
 9. VERIFY: Command executes instantly, no lag

 THINGS THAT COULD GO WRONG (add tests when possible):
 - Placeholder never shown (main.m not calling showPlaceholder)
 - Placeholder never dismissed (applicationDidFinishLaunching not called)
 - Placeholder dismisses too early (before real window ready)
 - Double parsing overhead (dtermCoreParserComparisonEnabled=YES)
 - Slow profile loading blocking main thread
 - Window restoration blocking main thread
 - Scripts menu building blocking main thread
 */
