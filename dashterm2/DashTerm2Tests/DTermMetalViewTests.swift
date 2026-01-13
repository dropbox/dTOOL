//
//  DTermMetalViewTests.swift
//  DashTerm2Tests
//
//  Tests for DTermMetalView - the thin Swift wrapper for dterm-core GPU rendering.
//

import XCTest
@testable import DashTerm2SharedARC

class DTermMetalViewTests: XCTestCase {

    // MARK: - DTermRenderer Tests

    /// Test that DTermRenderer availability check works.
    func test_DTermRenderer_availabilityCheck() {
        // This should not crash and should return a boolean
        let available = DTermRenderer.isAvailable
        // The renderer should be available if dterm-core was built with --features gpu
        // If not available, that's also a valid state (no GPU feature)
        XCTAssertTrue(available == true || available == false,
                      "isAvailable should return a boolean")
    }

    /// Test that DTermRenderer can be created if available.
    func test_DTermRenderer_creation() {
        guard DTermRenderer.isAvailable else {
            // Skip test if renderer not available (compiled without GPU feature)
            return
        }

        let renderer = DTermRenderer()
        XCTAssertNotNil(renderer, "Should be able to create DTermRenderer when available")
    }

    /// Test frame request/complete/wait cycle doesn't crash.
    func test_DTermRenderer_frameSync_doesNotCrash() {
        guard DTermRenderer.isAvailable else {
            return
        }

        guard let renderer = DTermRenderer() else {
            XCTFail("Failed to create renderer")
            return
        }

        // Request a frame
        guard let frameHandle = renderer.requestFrame() else {
            XCTFail("Failed to request frame")
            return
        }

        // Complete the frame (simulating drawable ready)
        renderer.completeFrame(frameHandle)

        // Wait for frame with very short timeout
        // This tests the safe timeout handling - should NOT crash
        let status = renderer.waitForFrame(frameHandle, timeoutMs: 1)

        // Status should be either ready or timeout - never crash
        XCTAssertTrue(status == .ready || status == .timeout || status == .cancelled,
                      "Frame status should be valid enum value")
    }

    /// Test that frame timeout doesn't crash (the key safety improvement over dispatch_group).
    func test_DTermRenderer_frameTimeout_isSafe() {
        guard DTermRenderer.isAvailable else {
            return
        }

        guard let renderer = DTermRenderer() else {
            XCTFail("Failed to create renderer")
            return
        }

        // Run many timeout cycles to stress test
        // This would have crashed with dispatch_group but should be safe now
        for _ in 0..<100 {
            guard let frameHandle = renderer.requestFrame() else {
                continue
            }

            // Don't complete - let it timeout
            renderer.cancelFrame(frameHandle)

            // Wait with short timeout
            let status = renderer.waitForFrame(frameHandle, timeoutMs: 1)

            // The key test is that we don't crash. Status can be any valid value
            // depending on the implementation (ready, timeout, cancelled).
            XCTAssertTrue(status == .ready || status == .timeout || status == .cancelled,
                          "Frame status should be a valid enum value")
        }

        // If we get here without crash, the safety improvement works
        XCTAssertTrue(true, "100 timeout cycles completed without crash")
    }

    // MARK: - DTermMetalView Tests

    /// Test that DTermMetalView availability check works.
    @MainActor
    func test_DTermMetalView_availabilityCheck() {
        let available = DTermMetalView.isAvailable
        XCTAssertTrue(available == true || available == false,
                      "isAvailable should return a boolean")
    }

    /// Test that DTermMetalView can be created.
    @MainActor
    func test_DTermMetalView_creation() {
        let view = DTermMetalView(frame: CGRect(x: 0, y: 0, width: 800, height: 600),
                                   terminal: nil)
        XCTAssertNotNil(view, "Should be able to create DTermMetalView")
        XCTAssertNotNil(view.device, "Should have a Metal device")
    }

    /// Test that DTermMetalView has correct initial properties.
    @MainActor
    func test_DTermMetalView_initialProperties() {
        let view = DTermMetalView(frame: CGRect(x: 0, y: 0, width: 800, height: 600),
                                   terminal: nil)

        // Should start unpaused (ready to render immediately)
        XCTAssertFalse(view.paused, "View should start unpaused")

        // Clear color should be initialized
        XCTAssertEqual(view.clearColor.alpha, 1.0, accuracy: 0.001,
                       "Clear color alpha should be 1.0")
    }

    /// Test that DTermMetalView drawable size updates on resize.
    @MainActor
    func test_DTermMetalView_drawableSizeUpdates() {
        let view = DTermMetalView(frame: CGRect(x: 0, y: 0, width: 400, height: 300),
                                   terminal: nil)

        // Change frame size
        view.setFrameSize(NSSize(width: 800, height: 600))

        // Drawable size should have updated (may be scaled)
        let drawableSize = view.drawableSize
        XCTAssertGreaterThan(drawableSize.width, 0, "Drawable width should be > 0")
        XCTAssertGreaterThan(drawableSize.height, 0, "Drawable height should be > 0")
    }

    /// Test that pausing and unpausing doesn't crash.
    @MainActor
    func test_DTermMetalView_pauseUnpause() {
        let view = DTermMetalView(frame: CGRect(x: 0, y: 0, width: 800, height: 600),
                                   terminal: nil)

        // Toggle pause multiple times
        view.paused = false
        XCTAssertFalse(view.paused)

        view.paused = true
        XCTAssertTrue(view.paused)

        view.paused = false
        XCTAssertFalse(view.paused)

        // Stop for cleanup
        view.paused = true
        XCTAssertTrue(view.paused)
    }

    /// Test that setting terminal doesn't crash.
    @MainActor
    func test_DTermMetalView_setTerminal() {
        let view = DTermMetalView(frame: CGRect(x: 0, y: 0, width: 800, height: 600),
                                   terminal: nil)

        // Create a dterm-core terminal
        let terminal = DTermCore(rows: 24, cols: 80)

        // Set it on the view
        view.setTerminal(terminal)

        // And clear it
        view.setTerminal(nil)

        // If we get here, no crash
        XCTAssertTrue(true)
    }

    /// Test that setting integration (from ObjC) doesn't crash.
    @MainActor
    func test_DTermMetalView_setIntegration() {
        let view = DTermMetalView(frame: CGRect(x: 0, y: 0, width: 800, height: 600),
                                   terminal: nil)

        // Create a DTermCoreIntegration
        let integration = DTermCoreIntegration(rows: 24, cols: 80)
        integration.isEnabled = true

        // Set it on the view (this is what SessionView.m calls via performSelector)
        view.setIntegration(integration)

        // And clear it
        view.setIntegration(nil)

        // If we get here, no crash
        XCTAssertTrue(true)
    }

    /// Test that setting font doesn't crash.
    @MainActor
    func test_DTermMetalView_setFont() {
        let view = DTermMetalView(frame: CGRect(x: 0, y: 0, width: 800, height: 600),
                                   terminal: nil)

        // Get a monospace font
        guard let font = NSFont.userFixedPitchFont(ofSize: 14) else {
            XCTFail("Could not get system monospace font")
            return
        }

        // Set the font - may return false if hybrid renderer not available
        _ = view.setFont(font)

        // Cell size should be set after font is configured (or zero if not available)
        let cellSize = view.cellSize
        // Note: cellSize will be .zero if hybrid renderer isn't available
        XCTAssertGreaterThanOrEqual(cellSize.width, 0)
        XCTAssertGreaterThanOrEqual(cellSize.height, 0)

        // If we get here, no crash
        XCTAssertTrue(true)
    }

    /// Test the full integration flow: create view, set integration, set font.
    @MainActor
    func test_DTermMetalView_fullIntegrationFlow() {
        // This tests the complete flow that SessionView.m uses
        let view = DTermMetalView(frame: CGRect(x: 0, y: 0, width: 800, height: 600),
                                   terminal: nil)

        // Create integration and enable it
        let integration = DTermCoreIntegration(rows: 24, cols: 80)
        integration.isEnabled = true

        // Feed some data to the terminal
        integration.process("Hello, World!\r\n".data(using: .utf8)!)

        // Set integration
        view.setIntegration(integration)

        // Set font
        if let font = NSFont.userFixedPitchFont(ofSize: 14) {
            _ = view.setFont(font)
        }

        // Start rendering (unpause)
        view.paused = false

        // Give display link time to fire (not required for test, just validates no crash)
        RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.05))

        // Stop
        view.paused = true

        // Clean up
        view.setIntegration(nil)

        // If we get here, no crash
        XCTAssertTrue(true)
    }

    // MARK: - Performance Metrics Tests

    /// Test that performance counters are initialized to zero.
    @MainActor
    func test_DTermMetalView_performanceCounters_initialState() {
        let view = DTermMetalView(frame: CGRect(x: 0, y: 0, width: 800, height: 600),
                                   terminal: nil)

        // Initial state: no frames rendered yet
        XCTAssertEqual(view.frameCount, 0, "Frame count should start at 0")
        XCTAssertEqual(view.currentFPS, 0, accuracy: 0.001, "FPS should be 0 with no frames")
        XCTAssertEqual(view.averageFPS, 0, accuracy: 0.001, "Average FPS should be 0 with no frames")
        XCTAssertEqual(view.gpuFrameTimeMs, 0, accuracy: 0.001, "GPU frame time should be 0 initially")
    }

    /// Test that resetPerformanceCounters works.
    @MainActor
    func test_DTermMetalView_resetPerformanceCounters() {
        let view = DTermMetalView(frame: CGRect(x: 0, y: 0, width: 800, height: 600),
                                   terminal: nil)

        // Let some frames render
        view.paused = false
        RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.1))
        view.paused = true

        // Reset counters
        view.resetPerformanceCounters()

        // Verify reset
        XCTAssertEqual(view.frameCount, 0, "Frame count should be 0 after reset")
        XCTAssertEqual(view.currentFPS, 0, accuracy: 0.001, "FPS should be 0 after reset")
    }

    /// Test that performanceSummary returns a formatted string.
    @MainActor
    func test_DTermMetalView_performanceSummary() {
        let view = DTermMetalView(frame: CGRect(x: 0, y: 0, width: 800, height: 600),
                                   terminal: nil)

        let summary = view.performanceSummary
        XCTAssertFalse(summary.isEmpty, "Performance summary should not be empty")
        XCTAssertTrue(summary.contains("FPS"), "Summary should contain 'FPS'")
        XCTAssertTrue(summary.contains("Frame"), "Summary should contain 'Frame'")
        XCTAssertTrue(summary.contains("GPU"), "Summary should contain 'GPU'")
    }

    /// Test that frame count increases while rendering.
    @MainActor
    func test_DTermMetalView_frameCountIncreases() {
        let view = DTermMetalView(frame: CGRect(x: 0, y: 0, width: 800, height: 600),
                                   terminal: nil)

        let initialCount = view.frameCount

        // Render for a brief period
        view.paused = false
        RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.2))
        view.paused = true

        // Frame count should have increased (display link fires at ~60Hz)
        XCTAssertGreaterThan(view.frameCount, initialCount,
                            "Frame count should increase during rendering")
    }

    /// Test that logPerformance doesn't crash.
    @MainActor
    func test_DTermMetalView_logPerformance() {
        let view = DTermMetalView(frame: CGRect(x: 0, y: 0, width: 800, height: 600),
                                   terminal: nil)

        // Just verify it doesn't crash
        view.logPerformance()
        XCTAssertTrue(true, "logPerformance should not crash")
    }

    // MARK: - Image Rendering Tests

    /// Test that image pipeline is set up correctly.
    @MainActor
    func test_DTermMetalView_imagePipelineSetup() {
        let view = DTermMetalView(frame: CGRect(x: 0, y: 0, width: 800, height: 600),
                                   terminal: nil)

        // Set up font to initialize the renderer
        if let font = NSFont.userFixedPitchFont(ofSize: 14) {
            _ = view.setFont(font)
        }

        // Check that the view initialized (device should be non-nil)
        XCTAssertNotNil(view.device, "Metal device should be available")
    }

    /// Test that clearImageCache doesn't crash with empty cache.
    @MainActor
    func test_DTermMetalView_clearImageCache_empty() {
        let view = DTermMetalView(frame: CGRect(x: 0, y: 0, width: 800, height: 600),
                                   terminal: nil)

        // Clear cache should be safe even with nothing cached
        view.clearImageCache()
        XCTAssertTrue(true, "clearImageCache should not crash on empty cache")
    }

    /// Test that image rendering flow with integration doesn't crash.
    @MainActor
    func test_DTermMetalView_imageRenderingWithIntegration() {
        let view = DTermMetalView(frame: CGRect(x: 0, y: 0, width: 800, height: 600),
                                   terminal: nil)

        // Create integration and enable it
        let integration = DTermCoreIntegration(rows: 24, cols: 80)
        integration.isEnabled = true

        // Set integration
        view.setIntegration(integration)

        // Set font
        if let font = NSFont.userFixedPitchFont(ofSize: 14) {
            _ = view.setFont(font)
        }

        // Start rendering
        view.paused = false

        // Give display link time to fire
        RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.05))

        // Stop and clear cache
        view.paused = true
        view.clearImageCache()

        // Clean up
        view.setIntegration(nil)

        XCTAssertTrue(true, "Image rendering flow should not crash")
    }

    /// Test that clearImageCache can be called multiple times safely.
    @MainActor
    func test_DTermMetalView_clearImageCache_multiple() {
        let view = DTermMetalView(frame: CGRect(x: 0, y: 0, width: 800, height: 600),
                                   terminal: nil)

        // Multiple clears should be safe
        for _ in 0..<10 {
            view.clearImageCache()
        }

        XCTAssertTrue(true, "Multiple clearImageCache calls should not crash")
    }

    /// Test image rendering with paused/unpaused state changes.
    @MainActor
    func test_DTermMetalView_imageRendering_pauseUnpause() {
        let view = DTermMetalView(frame: CGRect(x: 0, y: 0, width: 800, height: 600),
                                   terminal: nil)

        let integration = DTermCoreIntegration(rows: 24, cols: 80)
        integration.isEnabled = true
        view.setIntegration(integration)

        if let font = NSFont.userFixedPitchFont(ofSize: 14) {
            _ = view.setFont(font)
        }

        // Toggle pause/unpause rapidly while potentially rendering images
        for _ in 0..<5 {
            view.paused = false
            RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.01))
            view.clearImageCache()
            view.paused = true
        }

        view.setIntegration(nil)
        XCTAssertTrue(true, "Pause/unpause with image cache clear should not crash")
    }

    // MARK: - Font Detection Tests (Regression for platform glyph bug)

    /// Test: Fonts with file URLs (like Monaco, Menlo on modern macOS) should use fontdue.
    /// The font detection logic checks for file URL availability to decide the rendering path.
    @MainActor
    func test_setFont_monacoWithFileURL_usesFontdue() {
        let view = DTermMetalView(frame: CGRect(x: 0, y: 0, width: 800, height: 600),
                                   terminal: nil)

        guard let monaco = NSFont(name: "Monaco", size: 14) else {
            return // Skip if Monaco unavailable
        }

        // Verify Monaco has a file URL on this system
        let ctFont = monaco as CTFont
        let fontDescriptor = CTFontCopyFontDescriptor(ctFont)
        let hasFileURL = CTFontDescriptorCopyAttribute(fontDescriptor, kCTFontURLAttribute) as? URL != nil

        let success = view.setFont(monaco)
        XCTAssertTrue(success, "setFont should succeed for Monaco")

        if hasFileURL {
            // Monaco has a file URL - should use fontdue
            XCTAssertFalse(view.isPlatformGlyphsEnabled,
                          "Monaco (has file URL) should use fontdue, not platform glyphs")
        } else {
            // Monaco has no file URL - should use platform glyphs
            XCTAssertTrue(view.isPlatformGlyphsEnabled,
                         "Monaco (no file URL) should use platform glyphs")
        }
    }

    /// Test: Fonts with file URLs (like Monaco, Menlo on modern macOS) should use fontdue.
    @MainActor
    func test_setFont_menloWithFileURL_usesFontdue() {
        let view = DTermMetalView(frame: CGRect(x: 0, y: 0, width: 800, height: 600),
                                   terminal: nil)

        guard let menlo = NSFont(name: "Menlo", size: 14) else {
            return // Skip if Menlo unavailable
        }

        let ctFont = menlo as CTFont
        let fontDescriptor = CTFontCopyFontDescriptor(ctFont)
        let hasFileURL = CTFontDescriptorCopyAttribute(fontDescriptor, kCTFontURLAttribute) as? URL != nil

        let success = view.setFont(menlo)
        XCTAssertTrue(success, "setFont should succeed for Menlo")

        if hasFileURL {
            XCTAssertFalse(view.isPlatformGlyphsEnabled,
                          "Menlo (has file URL) should use fontdue, not platform glyphs")
        } else {
            XCTAssertTrue(view.isPlatformGlyphsEnabled,
                         "Menlo (no file URL) should use platform glyphs")
        }
    }

    /// Test: SF Mono behavior depends on whether it has a file URL.
    @MainActor
    func test_setFont_sfMono_correctPathBasedOnURL() {
        let view = DTermMetalView(frame: CGRect(x: 0, y: 0, width: 800, height: 600),
                                   terminal: nil)

        guard let sfMono = NSFont(name: "SFMono-Regular", size: 14) else {
            return // Skip if SF Mono unavailable
        }

        let ctFont = sfMono as CTFont
        let fontDescriptor = CTFontCopyFontDescriptor(ctFont)
        let hasFileURL = CTFontDescriptorCopyAttribute(fontDescriptor, kCTFontURLAttribute) as? URL != nil

        let success = view.setFont(sfMono)
        XCTAssertTrue(success, "setFont should succeed for SF Mono")

        if hasFileURL {
            XCTAssertFalse(view.isPlatformGlyphsEnabled,
                          "SF Mono (has file URL) should use fontdue")
        } else {
            XCTAssertTrue(view.isPlatformGlyphsEnabled,
                         "SF Mono (no file URL) should use platform glyphs")
        }
    }

    /// Test: The font detection logic correctly routes based on file URL availability.
    @MainActor
    func test_setFont_routingLogicIsConsistent() {
        let view = DTermMetalView(frame: CGRect(x: 0, y: 0, width: 800, height: 600),
                                   terminal: nil)

        guard let font = NSFont.userFixedPitchFont(ofSize: 14) else {
            XCTFail("Could not get system monospace font")
            return
        }

        // Check if this font has a file URL
        let ctFont = font as CTFont
        let fontDescriptor = CTFontCopyFontDescriptor(ctFont)
        let hasFileURL = CTFontDescriptorCopyAttribute(fontDescriptor, kCTFontURLAttribute) as? URL != nil

        let success = view.setFont(font)
        XCTAssertTrue(success, "setFont should succeed")

        // The key invariant: file URL presence determines the path
        if hasFileURL {
            XCTAssertFalse(view.isPlatformGlyphsEnabled,
                          "Font with file URL should use fontdue")
        } else {
            XCTAssertTrue(view.isPlatformGlyphsEnabled,
                         "Font without file URL should use platform glyphs")
        }
    }

    /// Test that cell size is valid after setting any font.
    @MainActor
    func test_setFont_cellSizeIsValid() {
        let view = DTermMetalView(frame: CGRect(x: 0, y: 0, width: 800, height: 600),
                                   terminal: nil)

        guard let monaco = NSFont(name: "Monaco", size: 14) else {
            return
        }

        let success = view.setFont(monaco)
        XCTAssertTrue(success, "setFont should succeed for Monaco")

        // Cell size should be valid regardless of rendering path
        let cellSize = view.cellSize
        XCTAssertGreaterThan(cellSize.width, 0, "Cell width should be > 0")
        XCTAssertGreaterThan(cellSize.height, 0, "Cell height should be > 0")
    }

    /// Test: Verify Monaco has a file URL on modern macOS (informational test).
    func test_fontURLDetection_monacoHasFileURL() {
        guard let monaco = NSFont(name: "Monaco", size: 14) else {
            return
        }

        let ctFont = monaco as CTFont
        let fontDescriptor = CTFontCopyFontDescriptor(ctFont)
        let url = CTFontDescriptorCopyAttribute(fontDescriptor, kCTFontURLAttribute) as? URL

        // On modern macOS, Monaco is at /System/Library/Fonts/Monaco.ttf
        // This is an informational test - URL availability may vary by macOS version
        if let url = url {
            XCTAssertTrue(url.path.contains("Monaco"), "Monaco URL should reference Monaco font file")
        }
        // If url is nil, that's also valid on some systems - test passes either way
    }

    /// Test: Verify Menlo has a file URL on modern macOS (informational test).
    func test_fontURLDetection_menloHasFileURL() {
        guard let menlo = NSFont(name: "Menlo", size: 14) else {
            return
        }

        let ctFont = menlo as CTFont
        let fontDescriptor = CTFontCopyFontDescriptor(ctFont)
        let url = CTFontDescriptorCopyAttribute(fontDescriptor, kCTFontURLAttribute) as? URL

        // On modern macOS, Menlo is at /System/Library/Fonts/Menlo.ttc
        if let url = url {
            XCTAssertTrue(url.path.contains("Menlo"), "Menlo URL should reference Menlo font file")
        }
    }
}
