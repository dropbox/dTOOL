/*
 * DTermGPURendererTests.swift - Integration Tests for GPU Renderer
 *
 * Copyright 2024-2025 Dropbox, Inc.
 * Licensed under Apache 2.0
 *
 * These tests verify the Swift-to-Rust GPU renderer FFI integration.
 * They test the DTermFrameSync and DTermGPURenderer Swift wrappers.
 *
 * ## Key Safety Properties Tested
 *
 * - No "unbalanced dispatch_group_leave()" errors possible (TLA+ verified)
 * - Timeout cannot cause crashes (unlike ObjC dispatch_group)
 * - Late completion after timeout is safe (no-op, not crash)
 * - Rapid request/cancel cycles are safe
 *
 * ## Phase E Roadmap
 *
 * This file is part of Step 9: Integration Testing
 * See: docs/ROADMAP_PHASE_E_GPU_RENDERER.md
 */

import Foundation
import XCTest
@testable import DTermCore

final class DTermGPURendererTests: XCTestCase {

    // MARK: - Frame Status Tests

    func testFrameStatusValues() {
        XCTAssertEqual(FrameStatus.ready.rawValue, 0)
        XCTAssertEqual(FrameStatus.timeout.rawValue, 1)
        XCTAssertEqual(FrameStatus.cancelled.rawValue, 2)
    }

    // MARK: - Renderer Config Tests

    func testRendererConfigDefault() {
        let config = RendererConfig()
        XCTAssertEqual(config.initialWidth, 800)
        XCTAssertEqual(config.initialHeight, 600)
        XCTAssertEqual(config.scaleFactor, 1.0, accuracy: 0.001)
        XCTAssertEqual(config.backgroundColor.r, 0)
        XCTAssertEqual(config.backgroundColor.g, 0)
        XCTAssertEqual(config.backgroundColor.b, 0)
        XCTAssertEqual(config.backgroundColor.a, 255)
        XCTAssertTrue(config.vsync)
        XCTAssertEqual(config.targetFPS, 60)
        XCTAssertTrue(config.damageRendering)
        XCTAssertEqual(config.cursorBlinkMs, 530)
    }

    func testRendererConfigCustom() {
        let config = RendererConfig(
            backgroundColor: (r: 30, g: 30, b: 46, a: 255),
            vsync: false,
            targetFPS: 120,
            drawableTimeoutMs: 8,
            damageRendering: false
        )
        XCTAssertEqual(config.backgroundColor.r, 30)
        XCTAssertEqual(config.backgroundColor.g, 30)
        XCTAssertEqual(config.backgroundColor.b, 46)
        XCTAssertEqual(config.backgroundColor.a, 255)
        XCTAssertFalse(config.vsync)
        XCTAssertEqual(config.targetFPS, 120)
        XCTAssertEqual(config.drawableTimeoutMs, 8)
        XCTAssertFalse(config.damageRendering)
    }

    // MARK: - Atlas Config Tests

    func testAtlasConfigDefault() {
        let config = AtlasConfig()
        XCTAssertEqual(config.initialSize, 512)
        XCTAssertEqual(config.maxSize, 4096)
        XCTAssertEqual(config.defaultFontSize, 14)
        XCTAssertEqual(config.padding, 1)
    }

    func testAtlasConfigCustom() {
        let config = AtlasConfig(
            initialSize: 1024,
            maxSize: 8192,
            defaultFontSize: 16,
            padding: 2
        )
        XCTAssertEqual(config.initialSize, 1024)
        XCTAssertEqual(config.maxSize, 8192)
        XCTAssertEqual(config.defaultFontSize, 16)
        XCTAssertEqual(config.padding, 2)
    }

    // MARK: - Hybrid Data Tests

    func testAtlasDataStruct() {
        let data = Data([0x00, 0xFF])
        let atlas = AtlasData(width: 2, height: 1, data: data)
        XCTAssertEqual(atlas.width, 2)
        XCTAssertEqual(atlas.height, 1)
        XCTAssertEqual(atlas.data, data)
    }

    func testPendingGlyphStruct() {
        var entry = HybridGlyphEntry()
        entry.x = 1
        entry.y = 2
        entry.width = 3
        entry.height = 4
        entry.bearing_x = 5
        entry.bearing_y = 6
        entry.advance = 7
        let bitmap = Data([0x10, 0x20])
        let pending = PendingGlyph(entry: entry, bitmap: bitmap)
        XCTAssertEqual(pending.entry.x, 1)
        XCTAssertEqual(pending.entry.advance, 7)
        XCTAssertEqual(pending.bitmap, bitmap)
    }

    // MARK: - Cell Size Tests

    func testCellSizeStruct() {
        let cellSize = CellSize(width: 8.0, height: 16.0)
        XCTAssertEqual(cellSize.width, 8.0, accuracy: 0.001)
        XCTAssertEqual(cellSize.height, 16.0, accuracy: 0.001)
    }

    // MARK: - Damage Region Tests

    func testDamageRegionPartial() {
        let damage = DamageRegion(startRow: 0, endRow: 10, startCol: 0, endCol: 80)
        XCTAssertEqual(damage.startRow, 0)
        XCTAssertEqual(damage.endRow, 10)
        XCTAssertEqual(damage.startCol, 0)
        XCTAssertEqual(damage.endCol, 80)
        XCTAssertFalse(damage.isFull)
    }

    func testDamageRegionFull() {
        let damage = DamageRegion.full
        XCTAssertTrue(damage.isFull)
    }

    // MARK: - Frame Handle Tests

    // Note: Frame handle tests that create handles directly require access to
    // the internal DtermFrameHandle type from CDTermCore. These are tested
    // via the DTermFrameSync flow tests below which exercise the full API.

    // MARK: - DTermRenderError Tests

    func testRenderErrorDescription() {
        XCTAssertEqual(DTermRenderError.nullPointer.description, "Null pointer argument")
        XCTAssertEqual(DTermRenderError.invalidDevice.description, "Invalid GPU device handle")
        XCTAssertEqual(DTermRenderError.invalidQueue.description, "Invalid GPU queue handle")
        XCTAssertEqual(DTermRenderError.invalidSurfaceView.description, "Invalid surface view handle")
        XCTAssertEqual(DTermRenderError.renderFailed.description, "Rendering failed")
    }

    // MARK: - Frame Sync Tests (require linked library)

    #if DTERM_LINKED

    // MARK: Frame Sync Lifecycle

    func testFrameSyncCreation() {
        // Create with default config
        let sync = DTermFrameSync()
        XCTAssertNotNil(sync)
    }

    func testFrameSyncWithConfig() {
        var config = RendererConfig()
        config.vsync = false
        config.targetFPS = 120

        let sync = DTermFrameSync(config: config)
        XCTAssertNotNil(sync)
    }

    func testFrameSyncAvailable() {
        // The FFI layer should always be available when linked
        XCTAssertTrue(DTermFrameSync.isAvailable)
    }

    func testHybridRendererAvailable() {
        XCTAssertTrue(DTermHybridRenderer.isAvailable)
    }

    func testHybridRendererCreation() {
        let renderer = DTermHybridRenderer()
        XCTAssertNotNil(renderer)
    }

    func testFrameSyncDefaultConfig() {
        let config = DTermFrameSync.defaultConfig
        XCTAssertEqual(config.backgroundColor.r, 0)
        XCTAssertEqual(config.backgroundColor.g, 0)
        XCTAssertEqual(config.backgroundColor.b, 0)
        XCTAssertTrue(config.vsync)
        XCTAssertEqual(config.targetFPS, 60)
    }

    // MARK: Frame Request/Complete/Wait Cycle

    /// Tests the exact sequence that platform code will use:
    /// 1. Request frame
    /// 2. Complete frame (when drawable is available)
    /// 3. Wait for frame
    func testFrameSyncFullCycle() {
        let sync = DTermFrameSync()

        // Request a frame
        let frame = sync.requestFrame()
        XCTAssertTrue(frame.isValid, "Frame handle should be valid")

        // Complete the frame (platform provides drawable)
        sync.completeFrame(frame)

        // Wait for frame to be ready
        let status = sync.waitForFrame(timeoutMs: 100)
        XCTAssertEqual(status, .ready, "Frame should be ready after completion")
    }

    // MARK: Timeout Safety Tests (CRITICAL)

    /// Tests timeout handling - CRITICAL SAFETY TEST
    ///
    /// In ObjC this pattern would cause "unbalanced dispatch_group_leave()" crash.
    /// In Rust this safely times out.
    func testFrameSyncTimeout() {
        let sync = DTermFrameSync()

        // Request a frame but DON'T complete it
        let _ = sync.requestFrame()

        // Wait should timeout (NOT crash!)
        let status = sync.waitForFrame(timeoutMs: 5)
        XCTAssertEqual(status, .timeout, "Should timeout when no drawable provided")
    }

    /// Tests late completion after timeout - CRITICAL SAFETY TEST
    ///
    /// In ObjC, completing after timeout causes crash.
    /// In Rust, this is safe (send to closed channel is no-op).
    func testFrameSyncLateCompletion() {
        let sync = DTermFrameSync()

        // Request a frame
        let frame = sync.requestFrame()

        // Wait and timeout first
        let status = sync.waitForFrame(timeoutMs: 1)
        XCTAssertEqual(status, .timeout)

        // Now complete late - THIS WOULD CRASH IN OBJC
        // In Rust it's safe (no-op)
        sync.completeFrame(frame)

        // No crash! That's the test.
    }

    // MARK: Cancel Tests

    func testFrameSyncCancel() {
        let sync = DTermFrameSync()

        // Request a frame
        let frame = sync.requestFrame()

        // Cancel it
        sync.cancelFrame(frame)

        // Wait should return Ready (drop notifies completion)
        let status = sync.waitForFrame(timeoutMs: 100)
        XCTAssertEqual(status, .ready)
    }

    // MARK: Stress Tests

    /// Tests rapid frame request/complete cycles (window resize scenario)
    ///
    /// This pattern frequently causes "unbalanced" crashes in ObjC.
    func testFrameSyncStress() {
        let sync = DTermFrameSync()

        // Simulate 60 frames of rapid cycling
        for i in 0..<60 {
            let frame = sync.requestFrame()

            if i % 10 != 0 {
                // Normal case: complete frame
                sync.completeFrame(frame)
                let status = sync.waitForFrame(timeoutMs: 16)
                XCTAssertEqual(status, .ready)
            } else {
                // Every 10th frame: timeout
                let status = sync.waitForFrame(timeoutMs: 1)
                XCTAssertEqual(status, .timeout)
                // Late completion is safe
                sync.completeFrame(frame)
            }
        }
        // No crashes!
    }

    /// Tests that frame IDs increase monotonically (TLA+ property)
    func testFrameIdMonotonic() {
        let sync = DTermFrameSync()

        var lastId: UInt64 = 0
        for _ in 0..<100 {
            let frame = sync.requestFrame()
            XCTAssertGreaterThan(frame.id, lastId, "Frame ID should increase")
            lastId = frame.id
            sync.completeFrame(frame)
            let _ = sync.waitForFrame(timeoutMs: 1)
        }
    }

    #endif // DTERM_LINKED

    // MARK: - GPU Renderer Tests (require wgpu)

    // Note: Full DTermGPURenderer tests require wgpu device/queue which
    // are only available when running with an actual GPU. These tests
    // verify the Swift wrapper API without actual rendering.

    func testGPURendererAvailable() {
        // This checks if the GPU renderer FFI is compiled in
        XCTAssertTrue(DTermGPURenderer.isAvailable)
    }
}
