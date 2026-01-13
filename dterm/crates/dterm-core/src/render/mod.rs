//! Rendering coordination primitives.
//!
//! This module provides lightweight helpers for renderer integrations:
//! - `FrameSync` for compositor frame callbacks (Wayland-style sync).
//! - `TripleBuffer` for managing front/middle/back render buffers.

/// Render scheduling action for frame callback integrations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameAction {
    /// Request a compositor frame callback.
    RequestCallback,
    /// Render immediately.
    RenderNow,
    /// No action required.
    None,
}

/// Frame callback synchronization mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameSyncMode {
    /// Render immediately without waiting for compositor callbacks.
    Immediate,
    /// Wait for compositor frame callbacks before rendering.
    Callback,
}

/// Frame callback synchronization state machine.
#[derive(Debug, Clone, Copy)]
pub struct FrameSync {
    mode: FrameSyncMode,
    waiting_for_callback: bool,
    needs_render: bool,
}

impl FrameSync {
    /// Create a new frame synchronization state machine.
    #[must_use]
    pub const fn new(mode: FrameSyncMode) -> Self {
        Self {
            mode,
            waiting_for_callback: false,
            needs_render: false,
        }
    }

    /// Get the current synchronization mode.
    #[must_use]
    pub const fn mode(&self) -> FrameSyncMode {
        self.mode
    }

    /// Reset pending state while keeping the current mode.
    pub fn reset(&mut self) {
        self.waiting_for_callback = false;
        self.needs_render = false;
    }

    /// Notify that new frame damage occurred.
    ///
    /// Returns the action the renderer should take next.
    pub fn on_damage(&mut self) -> FrameAction {
        self.needs_render = true;
        match self.mode {
            FrameSyncMode::Immediate => {
                self.needs_render = false;
                FrameAction::RenderNow
            }
            FrameSyncMode::Callback => {
                if self.waiting_for_callback {
                    FrameAction::None
                } else {
                    self.waiting_for_callback = true;
                    FrameAction::RequestCallback
                }
            }
        }
    }

    /// Notify that the compositor frame callback fired.
    pub fn on_frame_callback(&mut self) -> FrameAction {
        if self.mode != FrameSyncMode::Callback {
            return FrameAction::None;
        }
        self.waiting_for_callback = false;
        if self.needs_render {
            self.needs_render = false;
            FrameAction::RenderNow
        } else {
            FrameAction::None
        }
    }

    /// Notify that a frame render/commit finished.
    pub fn on_rendered(&mut self) -> FrameAction {
        if self.mode != FrameSyncMode::Callback {
            return FrameAction::None;
        }
        if self.needs_render && !self.waiting_for_callback {
            self.waiting_for_callback = true;
            FrameAction::RequestCallback
        } else {
            FrameAction::None
        }
    }
}

/// Triple buffer container for renderers.
///
/// `front` is currently displayed, `middle` is pending presentation, and
/// `back` is the render target. Use `publish` when rendering completes and
/// `present` on vsync to swap in the latest completed frame.
#[derive(Debug, Clone)]
pub struct TripleBuffer<T> {
    buffers: [T; 3],
    front: usize,
    middle: usize,
    back: usize,
    pending: bool,
}

impl<T> TripleBuffer<T> {
    /// Create a new triple buffer with explicit initial buffers.
    #[must_use]
    pub const fn new(front: T, middle: T, back: T) -> Self {
        Self {
            buffers: [front, middle, back],
            front: 0,
            middle: 1,
            back: 2,
            pending: false,
        }
    }

    /// Return the buffer currently presented to the screen.
    #[must_use]
    pub fn front(&self) -> &T {
        &self.buffers[self.front]
    }

    /// Return the buffer currently presented to the screen (mutable).
    pub fn front_mut(&mut self) -> &mut T {
        let index = self.front;
        &mut self.buffers[index]
    }

    /// Return the buffer currently pending presentation.
    #[must_use]
    pub fn middle(&self) -> &T {
        &self.buffers[self.middle]
    }

    /// Return the current render target buffer.
    #[must_use]
    pub fn back(&self) -> &T {
        &self.buffers[self.back]
    }

    /// Return the current render target buffer (mutable).
    pub fn back_mut(&mut self) -> &mut T {
        let index = self.back;
        &mut self.buffers[index]
    }

    /// Returns true if a completed frame is pending presentation.
    #[must_use]
    pub const fn has_pending(&self) -> bool {
        self.pending
    }

    /// Publish the back buffer as the newest pending frame.
    ///
    /// Returns true if this replaced an already pending frame.
    pub fn publish(&mut self) -> bool {
        let replaced = self.pending;
        std::mem::swap(&mut self.middle, &mut self.back);
        self.pending = true;
        replaced
    }

    /// Present the pending frame, swapping it to the front buffer.
    ///
    /// Returns true if a frame was presented.
    pub fn present(&mut self) -> bool {
        if !self.pending {
            return false;
        }
        std::mem::swap(&mut self.front, &mut self.middle);
        self.pending = false;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::{FrameAction, FrameSync, FrameSyncMode, TripleBuffer};

    #[test]
    fn frame_sync_immediate_renders() {
        let mut sync = FrameSync::new(FrameSyncMode::Immediate);
        assert_eq!(sync.on_damage(), FrameAction::RenderNow);
        assert_eq!(sync.on_frame_callback(), FrameAction::None);
        assert_eq!(sync.on_rendered(), FrameAction::None);
    }

    #[test]
    fn frame_sync_callback_flow() {
        let mut sync = FrameSync::new(FrameSyncMode::Callback);
        assert_eq!(sync.on_damage(), FrameAction::RequestCallback);
        assert_eq!(sync.on_damage(), FrameAction::None);
        assert_eq!(sync.on_frame_callback(), FrameAction::RenderNow);
        assert_eq!(sync.on_rendered(), FrameAction::None);
    }

    #[test]
    fn frame_sync_damage_during_render_requests_next_callback() {
        let mut sync = FrameSync::new(FrameSyncMode::Callback);
        assert_eq!(sync.on_damage(), FrameAction::RequestCallback);
        assert_eq!(sync.on_frame_callback(), FrameAction::RenderNow);
        assert_eq!(sync.on_damage(), FrameAction::RequestCallback);
        assert_eq!(sync.on_rendered(), FrameAction::None);
    }

    #[test]
    fn triple_buffer_publish_and_present() {
        let mut buffer = TripleBuffer::new(1, 2, 3);
        assert_eq!(*buffer.front(), 1);
        assert_eq!(*buffer.back(), 3);
        assert!(!buffer.has_pending());

        assert!(!buffer.publish());
        assert!(buffer.has_pending());
        assert_eq!(*buffer.middle(), 3);
        assert_eq!(*buffer.back(), 2);

        assert!(buffer.present());
        assert_eq!(*buffer.front(), 3);
        assert!(!buffer.has_pending());
    }

    #[test]
    fn triple_buffer_publish_replaces_pending() {
        let mut buffer = TripleBuffer::new("front", "middle", "back");
        assert!(!buffer.publish());
        assert!(buffer.publish());
        assert!(buffer.has_pending());
        assert_eq!(buffer.middle(), &"middle");
    }
}
