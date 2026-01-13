/*
 * DTermUIBridge.swift - UI Bridge for platform integration
 *
 * Copyright 2024-2025 Dropbox, Inc.
 * Licensed under Apache 2.0
 *
 * This module provides a Swift interface to the dterm-core UI Bridge,
 * which coordinates terminal UI events with formal verification guarantees.
 */

import Foundation
import CDTermCore

// MARK: - Error Types

/// Errors that can occur during UI Bridge operations.
public enum DTermUIError: Error, CustomStringConvertible {
    case queueFull
    case shuttingDown
    case invalidTerminalId
    case invalidTerminalState
    case duplicateCallback
    case noEventPending
    case invalidStateTransition
    case nullPointer
    case unknown(Int)

    init(from code: DtermUIErrorCode) {
        switch code {
        case DTERM_UI_ERROR_CODE_OK:
            // This shouldn't happen - success isn't an error
            self = .unknown(0)
        case DTERM_UI_ERROR_CODE_QUEUE_FULL:
            self = .queueFull
        case DTERM_UI_ERROR_CODE_SHUTTING_DOWN:
            self = .shuttingDown
        case DTERM_UI_ERROR_CODE_INVALID_TERMINAL_ID:
            self = .invalidTerminalId
        case DTERM_UI_ERROR_CODE_INVALID_TERMINAL_STATE:
            self = .invalidTerminalState
        case DTERM_UI_ERROR_CODE_DUPLICATE_CALLBACK:
            self = .duplicateCallback
        case DTERM_UI_ERROR_CODE_NO_EVENT_PENDING:
            self = .noEventPending
        case DTERM_UI_ERROR_CODE_INVALID_STATE_TRANSITION:
            self = .invalidStateTransition
        case DTERM_UI_ERROR_CODE_NULL_POINTER:
            self = .nullPointer
        default:
            self = .unknown(Int(code.rawValue))
        }
    }

    public var description: String {
        switch self {
        case .queueFull:
            return "Event queue is full"
        case .shuttingDown:
            return "System is shutting down"
        case .invalidTerminalId:
            return "Invalid terminal ID"
        case .invalidTerminalState:
            return "Invalid terminal state"
        case .duplicateCallback:
            return "Duplicate callback ID"
        case .noEventPending:
            return "No event pending"
        case .invalidStateTransition:
            return "Invalid state transition"
        case .nullPointer:
            return "Null pointer"
        case .unknown(let code):
            return "Unknown error (code: \(code))"
        }
    }
}

// MARK: - State Types

/// UI Bridge state.
public enum UIBridgeState: Int {
    case idle = 0
    case processing = 1
    case rendering = 2
    case waitingForCallback = 3
    case shuttingDown = 4

    init(from state: DtermUIState) {
        switch state {
        case DTERM_UI_STATE_IDLE:
            self = .idle
        case DTERM_UI_STATE_PROCESSING:
            self = .processing
        case DTERM_UI_STATE_RENDERING:
            self = .rendering
        case DTERM_UI_STATE_WAITING_FOR_CALLBACK:
            self = .waitingForCallback
        case DTERM_UI_STATE_SHUTTING_DOWN:
            self = .shuttingDown
        default:
            self = .idle
        }
    }
}

/// Terminal state within the UI Bridge.
public enum UITerminalState: Int {
    case inactive = 0
    case active = 1
    case disposed = 2

    init(from state: DtermUITerminalState) {
        switch state {
        case DTERM_UI_TERMINAL_STATE_INACTIVE:
            self = .inactive
        case DTERM_UI_TERMINAL_STATE_ACTIVE:
            self = .active
        case DTERM_UI_TERMINAL_STATE_DISPOSED:
            self = .disposed
        default:
            self = .inactive
        }
    }
}

/// Event kind for UI Bridge events.
public enum UIEventKind: Int {
    case input = 0
    case resize = 1
    case render = 2
    case createTerminal = 3
    case destroyTerminal = 4
    case requestCallback = 5
    case shutdown = 6

    init(from kind: DtermUIEventKind) {
        switch kind {
        case DTERM_UI_EVENT_KIND_INPUT:
            self = .input
        case DTERM_UI_EVENT_KIND_RESIZE:
            self = .resize
        case DTERM_UI_EVENT_KIND_RENDER:
            self = .render
        case DTERM_UI_EVENT_KIND_CREATE_TERMINAL:
            self = .createTerminal
        case DTERM_UI_EVENT_KIND_DESTROY_TERMINAL:
            self = .destroyTerminal
        case DTERM_UI_EVENT_KIND_REQUEST_CALLBACK:
            self = .requestCallback
        case DTERM_UI_EVENT_KIND_SHUTDOWN:
            self = .shutdown
        default:
            self = .input
        }
    }
}

/// Event information returned from start_processing.
public struct UIEventInfo {
    public let eventId: UInt64
    public let kind: UIEventKind
    public let terminalId: UInt32?
    public let callbackId: UInt32?
    public let rows: UInt16
    public let cols: UInt16

    init(from info: DtermUIEventInfo) {
        self.eventId = info.event_id
        self.kind = UIEventKind(from: info.kind)
        self.terminalId = info.terminal_id == UInt32.max ? nil : info.terminal_id
        self.callbackId = info.callback_id == UInt32.max ? nil : info.callback_id
        self.rows = info.rows
        self.cols = info.cols
    }
}

// MARK: - UI Bridge

/// UI Bridge for coordinating terminal UI events.
///
/// The UI Bridge provides a formally verified state machine for managing
/// terminal lifecycle and event processing. It ensures:
/// - No event loss
/// - No double-free of terminal state
/// - Proper state transitions
/// - TLA+ verified invariants
///
/// ## Usage
///
/// ```swift
/// let bridge = DTermUIBridge()
///
/// // Create a terminal
/// try bridge.handleCreateTerminal(terminalId: 0)
///
/// // Send input
/// try bridge.handleInput(terminalId: 0, data: inputData)
///
/// // Resize
/// try bridge.handleResize(terminalId: 0, rows: 24, cols: 80)
///
/// // Destroy terminal
/// try bridge.handleDestroyTerminal(terminalId: 0)
///
/// // Shutdown
/// try bridge.handleShutdown()
/// ```
public final class DTermUIBridge {
    /// Opaque handle to the underlying dterm-core UI Bridge.
    private var handle: OpaquePointer?

    // MARK: - Lifecycle

    /// Create a new UI Bridge.
    public init() {
        self.handle = dterm_ui_create()
    }

    deinit {
        if let handle = handle {
            dterm_ui_free(handle)
        }
    }

    // MARK: - State Queries

    /// Get the current UI Bridge state.
    public var state: UIBridgeState {
        guard let handle = handle else { return .idle }
        return UIBridgeState(from: dterm_ui_state(handle))
    }

    /// Get the number of pending events in the queue.
    public var pendingCount: Int {
        guard let handle = handle else { return 0 }
        return Int(dterm_ui_pending_count(handle))
    }

    /// Get the number of pending callbacks.
    public var callbackCount: Int {
        guard let handle = handle else { return 0 }
        return Int(dterm_ui_callback_count(handle))
    }

    /// Get the number of pending renders.
    public var renderPendingCount: Int {
        guard let handle = handle else { return 0 }
        return Int(dterm_ui_render_pending_count(handle))
    }

    /// Check if the UI Bridge is in a consistent state.
    ///
    /// This verifies all TLA+ invariants hold.
    public var isConsistent: Bool {
        guard let handle = handle else { return false }
        return dterm_ui_is_consistent(handle)
    }

    /// Get the state of a terminal.
    ///
    /// - Parameter terminalId: The terminal ID.
    /// - Returns: The terminal state.
    public func terminalState(terminalId: UInt32) -> UITerminalState {
        guard let handle = handle else { return .inactive }
        return UITerminalState(from: dterm_ui_terminal_state(handle, terminalId))
    }

    // MARK: - Event Enqueueing (Low-level API)

    /// Enqueue an input event.
    ///
    /// - Parameters:
    ///   - terminalId: Target terminal ID.
    ///   - data: Input data.
    /// - Throws: `DTermUIError` on failure.
    public func enqueueInput(terminalId: UInt32, data: Data) throws {
        guard let handle = handle else {
            throw DTermUIError.nullPointer
        }

        let result = data.withUnsafeBytes { buffer -> DtermUIErrorCode in
            let ptr = buffer.baseAddress?.assumingMemoryBound(to: UInt8.self)
            return dterm_ui_enqueue_input(handle, terminalId, ptr, UInt(buffer.count))
        }

        if result != DTERM_UI_ERROR_CODE_OK {
            throw DTermUIError(from: result)
        }
    }

    /// Enqueue a resize event.
    ///
    /// - Parameters:
    ///   - terminalId: Target terminal ID.
    ///   - rows: New row count.
    ///   - cols: New column count.
    /// - Throws: `DTermUIError` on failure.
    public func enqueueResize(terminalId: UInt32, rows: UInt16, cols: UInt16) throws {
        guard let handle = handle else {
            throw DTermUIError.nullPointer
        }

        let result = dterm_ui_enqueue_resize(handle, terminalId, rows, cols)
        if result != DTERM_UI_ERROR_CODE_OK {
            throw DTermUIError(from: result)
        }
    }

    /// Enqueue a render event.
    ///
    /// - Parameter terminalId: Target terminal ID.
    /// - Throws: `DTermUIError` on failure.
    public func enqueueRender(terminalId: UInt32) throws {
        guard let handle = handle else {
            throw DTermUIError.nullPointer
        }

        let result = dterm_ui_enqueue_render(handle, terminalId)
        if result != DTERM_UI_ERROR_CODE_OK {
            throw DTermUIError(from: result)
        }
    }

    /// Enqueue a create terminal event.
    ///
    /// - Parameter terminalId: ID for the new terminal.
    /// - Throws: `DTermUIError` on failure.
    public func enqueueCreateTerminal(terminalId: UInt32) throws {
        guard let handle = handle else {
            throw DTermUIError.nullPointer
        }

        let result = dterm_ui_enqueue_create_terminal(handle, terminalId)
        if result != DTERM_UI_ERROR_CODE_OK {
            throw DTermUIError(from: result)
        }
    }

    /// Enqueue a destroy terminal event.
    ///
    /// - Parameter terminalId: ID of terminal to destroy.
    /// - Throws: `DTermUIError` on failure.
    public func enqueueDestroyTerminal(terminalId: UInt32) throws {
        guard let handle = handle else {
            throw DTermUIError.nullPointer
        }

        let result = dterm_ui_enqueue_destroy_terminal(handle, terminalId)
        if result != DTERM_UI_ERROR_CODE_OK {
            throw DTermUIError(from: result)
        }
    }

    /// Enqueue a callback request event.
    ///
    /// - Parameters:
    ///   - terminalId: Target terminal ID.
    ///   - callbackId: Callback identifier.
    /// - Throws: `DTermUIError` on failure.
    public func enqueueCallback(terminalId: UInt32, callbackId: UInt32) throws {
        guard let handle = handle else {
            throw DTermUIError.nullPointer
        }

        let result = dterm_ui_enqueue_callback(handle, terminalId, callbackId)
        if result != DTERM_UI_ERROR_CODE_OK {
            throw DTermUIError(from: result)
        }
    }

    /// Enqueue a shutdown event.
    ///
    /// - Throws: `DTermUIError` on failure.
    public func enqueueShutdown() throws {
        guard let handle = handle else {
            throw DTermUIError.nullPointer
        }

        let result = dterm_ui_enqueue_shutdown(handle)
        if result != DTERM_UI_ERROR_CODE_OK {
            throw DTermUIError(from: result)
        }
    }

    // MARK: - Event Processing (Low-level API)

    /// Start processing the next event.
    ///
    /// - Returns: Information about the event being processed.
    /// - Throws: `DTermUIError` on failure.
    public func startProcessing() throws -> UIEventInfo {
        guard let handle = handle else {
            throw DTermUIError.nullPointer
        }

        var info = DtermUIEventInfo()
        let result = dterm_ui_start_processing(handle, &info)
        if result != DTERM_UI_ERROR_CODE_OK {
            throw DTermUIError(from: result)
        }

        return UIEventInfo(from: info)
    }

    /// Complete processing the current event.
    ///
    /// - Throws: `DTermUIError` on failure.
    public func completeProcessing() throws {
        guard let handle = handle else {
            throw DTermUIError.nullPointer
        }

        let result = dterm_ui_complete_processing(handle)
        if result != DTERM_UI_ERROR_CODE_OK {
            throw DTermUIError(from: result)
        }
    }

    /// Complete a render for a terminal.
    ///
    /// - Parameter terminalId: Terminal that finished rendering.
    /// - Throws: `DTermUIError` on failure.
    public func completeRender(terminalId: UInt32) throws {
        guard let handle = handle else {
            throw DTermUIError.nullPointer
        }

        let result = dterm_ui_complete_render(handle, terminalId)
        if result != DTERM_UI_ERROR_CODE_OK {
            throw DTermUIError(from: result)
        }
    }

    /// Complete a callback.
    ///
    /// - Parameter callbackId: Callback that completed.
    /// - Throws: `DTermUIError` on failure.
    public func completeCallback(callbackId: UInt32) throws {
        guard let handle = handle else {
            throw DTermUIError.nullPointer
        }

        let result = dterm_ui_complete_callback(handle, callbackId)
        if result != DTERM_UI_ERROR_CODE_OK {
            throw DTermUIError(from: result)
        }
    }

    // MARK: - Convenience API (High-level)

    /// Handle a create terminal event in one shot.
    ///
    /// This enqueues and processes the event immediately if the bridge is idle.
    ///
    /// - Parameter terminalId: ID for the new terminal.
    /// - Throws: `DTermUIError` on failure.
    public func handleCreateTerminal(terminalId: UInt32) throws {
        guard let handle = handle else {
            throw DTermUIError.nullPointer
        }

        let result = dterm_ui_handle_create_terminal(handle, terminalId)
        if result != DTERM_UI_ERROR_CODE_OK {
            throw DTermUIError(from: result)
        }
    }

    /// Handle a destroy terminal event in one shot.
    ///
    /// - Parameter terminalId: ID of terminal to destroy.
    /// - Throws: `DTermUIError` on failure.
    public func handleDestroyTerminal(terminalId: UInt32) throws {
        guard let handle = handle else {
            throw DTermUIError.nullPointer
        }

        let result = dterm_ui_handle_destroy_terminal(handle, terminalId)
        if result != DTERM_UI_ERROR_CODE_OK {
            throw DTermUIError(from: result)
        }
    }

    /// Handle an input event in one shot.
    ///
    /// - Parameters:
    ///   - terminalId: Target terminal ID.
    ///   - data: Input data.
    /// - Throws: `DTermUIError` on failure.
    public func handleInput(terminalId: UInt32, data: Data) throws {
        guard let handle = handle else {
            throw DTermUIError.nullPointer
        }

        let result = data.withUnsafeBytes { buffer -> DtermUIErrorCode in
            let ptr = buffer.baseAddress?.assumingMemoryBound(to: UInt8.self)
            return dterm_ui_handle_input(handle, terminalId, ptr, UInt(buffer.count))
        }

        if result != DTERM_UI_ERROR_CODE_OK {
            throw DTermUIError(from: result)
        }
    }

    /// Handle a resize event in one shot.
    ///
    /// - Parameters:
    ///   - terminalId: Target terminal ID.
    ///   - rows: New row count.
    ///   - cols: New column count.
    /// - Throws: `DTermUIError` on failure.
    public func handleResize(terminalId: UInt32, rows: UInt16, cols: UInt16) throws {
        guard let handle = handle else {
            throw DTermUIError.nullPointer
        }

        let result = dterm_ui_handle_resize(handle, terminalId, rows, cols)
        if result != DTERM_UI_ERROR_CODE_OK {
            throw DTermUIError(from: result)
        }
    }

    /// Handle a shutdown event in one shot.
    ///
    /// - Throws: `DTermUIError` on failure.
    public func handleShutdown() throws {
        guard let handle = handle else {
            throw DTermUIError.nullPointer
        }

        let result = dterm_ui_handle_shutdown(handle)
        if result != DTERM_UI_ERROR_CODE_OK {
            throw DTermUIError(from: result)
        }
    }
}
