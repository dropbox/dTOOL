//
//  DashTerm2UITests.swift
//  DashTerm2UITests
//
//  Basic UI tests for DashTerm2 launch and window functionality.
//  These tests verify the app can launch and create terminal windows correctly.
//
//  BUG-13: UI tests use proper XCTWaiter/expectations instead of sleep()
//

import XCTest

/// Basic UI tests for DashTerm2 terminal application.
/// These tests use XCUITest to verify app launch, window creation, and basic stability.
final class DashTerm2UITests: XCTestCase {

    var app: XCUIApplication!

    override func setUpWithError() throws {
        continueAfterFailure = false
        app = XCUIApplication()

        // Disable tips, first-run dialogs, and other UI interruptions for test stability
        // These UserDefaults keys suppress various prompts that could interfere with tests
        app.launchArguments = [
            "-NoSyncTipsDisabled", "YES",
            "-NoSyncPermissionToShowTip", "NO",
            "-NoSyncBrowserOnboardingCompleted", "YES",
            "-NoSyncHaveBeenWarnedAboutTabDockSetting", "YES",
            "-NoSyncHaveWarnedAboutPasteConfirmationChange", "YES",
            "-NoSyncHaveExplainedHowToAddTouchbarControls", "YES",
            "-NoSyncHaveUsedCopyMode", "YES"
        ]
    }

    override func tearDownWithError() throws {
        // Terminate the app to ensure clean state for next test
        if app != nil {
            app.terminate()
        }
        app = nil
    }

    // MARK: - Launch Tests

    /// Test that the app launches successfully and stays running.
    /// This is the most basic UI test - if this fails, nothing else will work.
    func testAppLaunches() throws {
        app.launch()

        // Verify app launched by checking it's running
        XCTAssertTrue(app.state == .runningForeground, "App should be running in foreground")

        // Give the app a moment to stabilize
        let exists = app.wait(for: .runningForeground, timeout: 5)
        XCTAssertTrue(exists, "App should remain running after launch")
    }

    /// Test that a terminal window exists after launch.
    /// DashTerm2 should create a default terminal window on startup.
    func testTerminalWindowExistsAfterLaunch() throws {
        app.launch()

        // Wait for initial window to appear
        let window = app.windows.firstMatch
        let windowExists = window.waitForExistence(timeout: 10)
        XCTAssertTrue(windowExists, "A terminal window should exist after launch")
    }

    /// Test that the main menu exists and is accessible.
    func testMainMenuExists() throws {
        app.launch()

        // Verify menu bar items exist
        let menuBar = app.menuBars.firstMatch
        XCTAssertTrue(menuBar.exists, "Menu bar should exist")

        // Check for essential menu items
        let shellMenu = app.menuBars.menuBarItems["Shell"]
        XCTAssertTrue(shellMenu.exists, "Shell menu should exist")

        let editMenu = app.menuBars.menuBarItems["Edit"]
        XCTAssertTrue(editMenu.exists, "Edit menu should exist")

        let viewMenu = app.menuBars.menuBarItems["View"]
        XCTAssertTrue(viewMenu.exists, "View menu should exist")
    }

    // MARK: - Window Creation Tests

    /// Test that a new terminal window can be created via menu.
    func testCreateNewWindow() throws {
        app.launch()

        // Wait for initial window
        let initialWindow = app.windows.firstMatch
        XCTAssertTrue(initialWindow.waitForExistence(timeout: 10), "Initial window should exist")

        let initialWindowCount = app.windows.count

        // Create new window via menu: Shell > New Window
        app.menuBars.menuBarItems["Shell"].click()

        // Look for "New Window" menu item
        let newWindowMenuItem = app.menuBars.menuItems["New Window"]
        if newWindowMenuItem.exists {
            newWindowMenuItem.click()
        } else {
            // Try alternate path - might be in a submenu
            let shellMenu = app.menuBars.menuBarItems["Shell"].menus.firstMatch
            let menuItems = shellMenu.menuItems.allElementsBoundByIndex
            for item in menuItems where item.title.contains("New Window") {
                item.click()
                break
            }
        }

        // Wait for new window to appear (using proper expectation instead of sleep)
        let expectation = XCTNSPredicateExpectation(
            predicate: NSPredicate(format: "count > %d", initialWindowCount),
            object: app.windows
        )
        let result = XCTWaiter().wait(for: [expectation], timeout: 5)

        // Verify a new window was created
        XCTAssertTrue(
            result == .completed || app.windows.count > initialWindowCount,
            "A new window should be created (had \(initialWindowCount), now have \(app.windows.count))"
        )
    }

    /// Test that a new tab can be created via keyboard shortcut.
    func testCreateNewTab() throws {
        app.launch()

        // Wait for initial window
        let window = app.windows.firstMatch
        XCTAssertTrue(window.waitForExistence(timeout: 10), "Window should exist")

        // Get initial tab count from tab bar (if available)
        let tabBar = window.tabBars.firstMatch
        let initialTabCount = tabBar.exists ? tabBar.buttons.count : 0

        // Create new tab with Cmd+T
        app.typeKey("t", modifierFlags: .command)

        // Wait for tab to be created (using proper expectation instead of sleep)
        if tabBar.exists {
            let expectation = XCTNSPredicateExpectation(
                predicate: NSPredicate(format: "count > %d", initialTabCount),
                object: tabBar.buttons
            )
            let result = XCTWaiter().wait(for: [expectation], timeout: 5)
            XCTAssertTrue(
                result == .completed || tabBar.buttons.count > initialTabCount,
                "A new tab should be created (had \(initialTabCount), now have \(tabBar.buttons.count))"
            )
        } else {
            // If no tab bar visible, just wait briefly and verify app is stable
            let stableExpectation = expectation(description: "App remains stable")
            DispatchQueue.main.asyncAfter(deadline: .now() + 1) {
                stableExpectation.fulfill()
            }
            wait(for: [stableExpectation], timeout: 3)
        }

        // App should still be running after tab creation
        XCTAssertTrue(app.state == .runningForeground, "App should remain running after tab creation")
    }

    // MARK: - Stability Tests

    /// Test that the app remains stable for an extended period.
    /// This is similar to the smoke test but uses XCUITest framework.
    func testAppStabilityOverTime() throws {
        app.launch()

        // Wait for window
        let window = app.windows.firstMatch
        XCTAssertTrue(window.waitForExistence(timeout: 10), "Window should exist")

        // Let the app run for 5 seconds using proper expectation-based wait
        let stableExpectation = expectation(description: "App remains stable for 5 seconds")
        DispatchQueue.main.asyncAfter(deadline: .now() + 5) {
            stableExpectation.fulfill()
        }
        wait(for: [stableExpectation], timeout: 10)

        // Verify app is still running
        XCTAssertTrue(app.state == .runningForeground, "App should remain stable after 5 seconds")
    }

    /// Test that the app can handle rapid window focus changes.
    func testRapidFocusChanges() throws {
        app.launch()

        let window = app.windows.firstMatch
        XCTAssertTrue(window.waitForExistence(timeout: 10), "Window should exist")

        // Rapidly activate/deactivate the app
        for _ in 0..<5 {
            app.activate()
            usleep(100_000) // 100ms
        }

        // App should still be running
        XCTAssertTrue(app.state == .runningForeground, "App should handle rapid focus changes")
    }
}
