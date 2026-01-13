//
//  DTermFPSOverlay.swift
//  DashTerm2
//
//  Development-only FPS overlay for GPU renderer performance monitoring.
//  Shows real-time FPS metrics from DTermMetalView.
//
//  Enable: defaults write com.dashterm.dashterm2 DtermCoreFPSOverlayEnabled -bool YES
//
//  Created by DashTerm2 AI Worker on 2024-12-30.
//

import AppKit

/// Development FPS overlay view that displays real-time performance metrics.
///
/// This overlay is shown when both conditions are met:
/// 1. GPU renderer is enabled (`DtermCoreRendererEnabled = YES`)
/// 2. FPS overlay is enabled (`DtermCoreFPSOverlayEnabled = YES`)
///
/// The overlay displays:
/// - Current FPS (rolling 60-frame average)
/// - Average FPS (since render start)
/// - Frame time in milliseconds
/// - GPU frame time in milliseconds
/// - Total frames rendered
@MainActor
@objc(DTermFPSOverlay)
public class DTermFPSOverlay: NSView {

    // MARK: - Configuration

    /// Check if FPS overlay is enabled via user defaults.
    @objc public static var isEnabled: Bool {
        return UserDefaults.standard.bool(forKey: "DtermCoreFPSOverlayEnabled")
    }

    // MARK: - Properties

    /// The DTermMetalView to read metrics from.
    private weak var metalView: DTermMetalView?

    /// Timer for periodic updates.
    private var updateTimer: Timer?

    /// Text field for FPS display.
    private let fpsLabel: NSTextField

    /// Background effect view.
    private let backgroundView: NSVisualEffectView

    /// Update interval in seconds.
    private let updateInterval: TimeInterval = 0.1  // 10 Hz updates

    // MARK: - Initialization

    /// Create a new FPS overlay.
    @objc public override init(frame: CGRect) {
        // Create background with vibrancy
        backgroundView = NSVisualEffectView(frame: NSRect(x: 0, y: 0, width: 200, height: 80))
        backgroundView.blendingMode = .withinWindow
        backgroundView.material = .hudWindow
        backgroundView.state = .active
        backgroundView.wantsLayer = true
        backgroundView.layer?.cornerRadius = 8
        backgroundView.layer?.masksToBounds = true

        // Create label
        fpsLabel = NSTextField(labelWithString: "FPS: --")
        fpsLabel.font = NSFont.monospacedSystemFont(ofSize: 11, weight: .medium)
        fpsLabel.textColor = .labelColor
        fpsLabel.alignment = .left
        fpsLabel.lineBreakMode = .byWordWrapping
        fpsLabel.maximumNumberOfLines = 5

        super.init(frame: frame)

        setupViews()
    }

    required init?(coder: NSCoder) {
        backgroundView = NSVisualEffectView(frame: .zero)
        fpsLabel = NSTextField(labelWithString: "FPS: --")
        super.init(coder: coder)
        setupViews()
    }

    deinit {
        MainActor.assumeIsolated {
            updateTimer?.invalidate()
        }
    }

    // MARK: - Setup

    private func setupViews() {
        // Add background
        addSubview(backgroundView)

        // Add label to background
        fpsLabel.frame = NSRect(x: 8, y: 8, width: 184, height: 64)
        backgroundView.addSubview(fpsLabel)

        // Configure self
        wantsLayer = true
        layer?.zPosition = 1000  // Above terminal content
    }

    // MARK: - Public API

    /// Attach to a DTermMetalView and start monitoring.
    @objc public func attach(to metalView: DTermMetalView) {
        self.metalView = metalView
        startUpdating()
    }

    /// Detach and stop monitoring.
    @objc public func detach() {
        stopUpdating()
        self.metalView = nil
    }

    // MARK: - Layout

    public override func layout() {
        super.layout()

        // Position in top-right corner
        let overlaySize = CGSize(width: 200, height: 80)
        let padding: CGFloat = 10

        backgroundView.frame = NSRect(
            x: bounds.width - overlaySize.width - padding,
            y: bounds.height - overlaySize.height - padding,
            width: overlaySize.width,
            height: overlaySize.height
        )

        fpsLabel.frame = NSRect(
            x: 8,
            y: 8,
            width: overlaySize.width - 16,
            height: overlaySize.height - 16
        )
    }

    // MARK: - Updating

    private func startUpdating() {
        guard updateTimer == nil else { return }

        updateTimer = Timer.scheduledTimer(withTimeInterval: updateInterval, repeats: true) { [weak self] _ in
            Task { @MainActor in
                self?.updateDisplay()
            }
        }

        // Initial update
        updateDisplay()
    }

    private func stopUpdating() {
        updateTimer?.invalidate()
        updateTimer = nil
    }

    private func updateDisplay() {
        guard let metalView = metalView else {
            fpsLabel.stringValue = "No Metal View"
            return
        }

        let fps = metalView.currentFPS
        let avgFps = metalView.averageFPS
        let frameTime = metalView.frameTimeMs
        let gpuTime = metalView.gpuFrameTimeMs
        let frames = metalView.frameCount

        // Color code based on FPS
        let fpsColor: NSColor
        if fps >= 60 {
            fpsColor = .systemGreen
        } else if fps >= 30 {
            fpsColor = .systemYellow
        } else {
            fpsColor = .systemRed
        }

        // Build display string
        let displayText = String(format: """
            FPS: %.1f (avg: %.1f)
            Frame: %.2f ms
            GPU: %.2f ms
            Frames: %llu
            """,
            fps, avgFps, frameTime, gpuTime, frames
        )

        // Update label
        let attrString = NSMutableAttributedString(string: displayText)

        // Color the FPS value
        if let range = displayText.range(of: String(format: "%.1f", fps)) {
            attrString.addAttribute(.foregroundColor, value: fpsColor,
                                   range: NSRange(range, in: displayText))
        }

        fpsLabel.attributedStringValue = attrString
    }

    // MARK: - Mouse Events

    public override func mouseDown(with event: NSEvent) {
        // Allow double-click to reset counters
        if event.clickCount == 2 {
            metalView?.resetPerformanceCounters()
            DLog("DTermFPSOverlay: Performance counters reset")
        }
    }

    public override var acceptsFirstResponder: Bool {
        return true
    }
}

// MARK: - Factory

extension DTermFPSOverlay {

    /// Create and configure an FPS overlay for a SessionView if conditions are met.
    ///
    /// Returns nil if:
    /// - FPS overlay is disabled
    /// - Metal view is nil
    ///
    /// - Parameter metalView: The DTermMetalView to monitor
    /// - Returns: Configured overlay, or nil if conditions not met
    @objc public static func createOverlay(for metalView: DTermMetalView?) -> DTermFPSOverlay? {
        guard isEnabled else { return nil }
        guard let metalView = metalView else { return nil }

        let overlay = DTermFPSOverlay(frame: .zero)
        overlay.attach(to: metalView)
        return overlay
    }
}
