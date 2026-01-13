//! Application framework and event loop.

use crate::diff::Differ;
use crate::hit_test::HitTester;
use crate::hooks::{
    dispatch_click_event, dispatch_drag_event, focus_next, focus_prev, has_clickables,
    take_render_request,
};
use crate::layout::LayoutEngine;
use crate::node::Node;
use crate::render::render_to_buffer;
use crate::terminal::{Backend, CrosstermBackend, KeyCode, KeyEvent, MouseEvent, TerminalEvent};
use smallvec::SmallVec;
use std::io;
use std::time::{Duration, Instant};

/// Event coalescer to prevent render thrashing from rapid events.
///
/// This batches events together and coalesces resize events, keeping only
/// the latest resize. Key and mouse events are processed in order but batched.
struct EventCoalescer {
    /// Most recent resize event (we only care about the final size).
    pending_resize: Option<(u16, u16)>,
    /// Pending key events (max 16 to bound memory).
    pending_keys: SmallVec<[KeyEvent; 16]>,
    /// Pending mouse events (max 32 to bound memory - mouse moves generate many events).
    pending_mouse: SmallVec<[MouseEvent; 32]>,
    /// Pending paste events (max 4 to bound memory - paste payloads can be large).
    pending_paste: SmallVec<[String; 4]>,
}

impl EventCoalescer {
    const MAX_PENDING_KEYS: usize = 16;
    const MAX_PENDING_MOUSE: usize = 32;
    const MAX_PENDING_PASTE: usize = 4;

    fn new() -> Self {
        Self {
            pending_resize: None,
            pending_keys: SmallVec::new(),
            pending_mouse: SmallVec::new(),
            pending_paste: SmallVec::new(),
        }
    }

    /// Add an event to the coalescer.
    ///
    /// Returns true if the event queue is full and should be processed.
    fn push(&mut self, event: TerminalEvent) -> bool {
        match event {
            TerminalEvent::Resize { width, height } => {
                // Coalesce: keep only the latest resize
                self.pending_resize = Some((width, height));
                false
            }
            TerminalEvent::Key(key) => {
                if self.pending_keys.len() < Self::MAX_PENDING_KEYS {
                    self.pending_keys.push(key);
                    false
                } else {
                    // Queue full, signal to process now
                    true
                }
            }
            TerminalEvent::Mouse(mouse) => {
                if self.pending_mouse.len() < Self::MAX_PENDING_MOUSE {
                    self.pending_mouse.push(mouse);
                    false
                } else {
                    // Queue full, signal to process now
                    true
                }
            }
            TerminalEvent::Paste(text) => {
                if self.pending_paste.len() < Self::MAX_PENDING_PASTE {
                    self.pending_paste.push(text);
                    false
                } else {
                    true
                }
            }
            // Pass through other events immediately
            _ => true,
        }
    }

    /// Take the pending resize event if any.
    fn take_resize(&mut self) -> Option<(u16, u16)> {
        self.pending_resize.take()
    }

    /// Drain pending key events.
    fn drain_keys(&mut self) -> impl Iterator<Item = KeyEvent> + '_ {
        self.pending_keys.drain(..)
    }

    /// Drain pending mouse events.
    fn drain_mouse(&mut self) -> impl Iterator<Item = MouseEvent> + '_ {
        self.pending_mouse.drain(..)
    }

    /// Drain pending paste events.
    fn drain_paste(&mut self) -> impl Iterator<Item = String> + '_ {
        self.pending_paste.drain(..)
    }

    /// Check if there are any pending events.
    fn has_pending(&self) -> bool {
        self.pending_resize.is_some()
            || !self.pending_keys.is_empty()
            || !self.pending_mouse.is_empty()
            || !self.pending_paste.is_empty()
    }
}

/// Application error type.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    /// IO error during terminal operations.
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    /// Layout computation error.
    #[error("Layout error: {0}")]
    Layout(#[from] crate::layout::LayoutError),
    /// No render function was set before running the app.
    #[error("No render function set. Call .render() before .run()")]
    NoRenderFunction,
}

/// Render context passed to components.
pub struct Context<'a, S> {
    /// Application state.
    pub state: &'a S,
    /// Terminal size.
    pub size: (u16, u16),
}

impl<'a, S> Context<'a, S> {
    /// Get terminal width.
    pub fn width(&self) -> u16 {
        self.size.0
    }

    /// Get terminal height.
    pub fn height(&self) -> u16 {
        self.size.1
    }
}

/// Component trait for custom components.
pub trait Component {
    /// Render the component to a node tree.
    fn render(&self, ctx: &Context<()>) -> Node;
}

/// Frame statistics for performance monitoring.
#[derive(Debug, Clone, Copy, Default)]
pub struct FrameStats {
    /// Total frame time.
    pub frame_time: Duration,
    /// Time spent in layout.
    pub layout_time: Duration,
    /// Time spent in painting.
    pub paint_time: Duration,
    /// Time spent in diff.
    pub diff_time: Duration,
    /// Time spent writing output.
    pub output_time: Duration,
    /// Number of cells changed.
    pub cells_changed: usize,
    /// Frame number.
    pub frame_number: u64,
}

/// Result returned by event handlers to control rendering and quitting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppEventResult {
    /// Whether the app should quit.
    pub should_quit: bool,
    /// Whether the app should re-render.
    pub needs_render: bool,
}

impl AppEventResult {
    /// Continue without forcing a render.
    pub const fn skip_render() -> Self {
        Self {
            should_quit: false,
            needs_render: false,
        }
    }

    /// Continue and request a render.
    pub const fn render() -> Self {
        Self {
            should_quit: false,
            needs_render: true,
        }
    }

    /// Quit without rendering.
    pub const fn quit() -> Self {
        Self {
            should_quit: true,
            needs_render: false,
        }
    }
}

impl From<bool> for AppEventResult {
    fn from(should_quit: bool) -> Self {
        if should_quit {
            Self::quit()
        } else {
            Self::render()
        }
    }
}

/// Application handle for controlling the app from within.
#[derive(Clone)]
pub struct AppHandle {
    /// Internal quit flag.
    pub(crate) should_quit: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl AppHandle {
    /// Request the application to quit.
    pub fn quit(&self) {
        self.should_quit
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }
}

// Type aliases for callback functions to satisfy clippy type_complexity
type RenderFn<S> = Box<dyn Fn(&Context<S>) -> Node>;
type KeyHandler<S> = Box<dyn Fn(&mut S, &crate::terminal::KeyEvent) -> AppEventResult>;
type MouseHandler<S> = Box<dyn Fn(&mut S, &MouseEvent) -> AppEventResult>;
type PasteHandler<S> = Box<dyn Fn(&mut S, &str) -> AppEventResult>;
type ResizeHandler<S> = Box<dyn Fn(&mut S, u16, u16)>;

/// Application builder.
pub struct App<S> {
    state: S,
    render_fn: Option<RenderFn<S>>,
    on_key: Option<KeyHandler<S>>,
    on_mouse: Option<MouseHandler<S>>,
    on_paste: Option<PasteHandler<S>>,
    on_resize: Option<ResizeHandler<S>>,
    on_render: Option<Box<dyn Fn(FrameStats)>>,
    fps: u32,
    alt_screen: bool,
    backend: Option<Box<dyn Backend>>,
    should_quit: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl App<()> {
    /// Create a new app with default state.
    pub fn new() -> App<()> {
        App {
            state: (),
            render_fn: None,
            on_key: None,
            on_mouse: None,
            on_paste: None,
            on_resize: None,
            on_render: None,
            fps: 60,
            alt_screen: true,
            backend: None,
            should_quit: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }
}

impl<S> App<S> {
    /// Set application state.
    pub fn state<T>(self, state: T) -> App<T> {
        App {
            state,
            render_fn: None,
            on_key: None,
            on_mouse: None,
            on_paste: None,
            on_resize: None,
            on_render: None,
            fps: self.fps,
            alt_screen: self.alt_screen,
            backend: self.backend,
            should_quit: self.should_quit,
        }
    }

    /// Set the render function.
    pub fn render<F>(mut self, f: F) -> Self
    where
        F: Fn(&Context<S>) -> Node + 'static,
    {
        self.render_fn = Some(Box::new(f));
        self
    }

    /// Set keyboard event handler.
    /// Returns [`AppEventResult`] to control rendering and quitting.
    pub fn on_key<F, R>(mut self, handler: F) -> Self
    where
        F: Fn(&mut S, &crate::terminal::KeyEvent) -> R + 'static,
        R: Into<AppEventResult>,
    {
        self.on_key = Some(Box::new(move |state, event| handler(state, event).into()));
        self
    }

    /// Set mouse event handler.
    ///
    /// The handler receives mouse events including clicks, scrolls, and movement.
    /// Returns [`AppEventResult`] to control rendering and quitting.
    ///
    /// # Example
    ///
    /// ```ignore
    /// App::new()
    ///     .state(MyState::default())
    ///     .on_mouse(|state, mouse| {
    ///         match mouse.kind {
    ///             MouseEventKind::Down => {
    ///                 state.clicked_at = Some((mouse.x, mouse.y));
    ///             }
    ///             MouseEventKind::ScrollUp => {
    ///                 state.scroll_offset = state.scroll_offset.saturating_sub(1);
    ///             }
    ///             MouseEventKind::ScrollDown => {
    ///                 state.scroll_offset += 1;
    ///             }
    ///             _ => {}
    ///         }
    ///         false // don't quit
    ///     })
    ///     .run()
    /// ```
    pub fn on_mouse<F, R>(mut self, handler: F) -> Self
    where
        F: Fn(&mut S, &MouseEvent) -> R + 'static,
        R: Into<AppEventResult>,
    {
        self.on_mouse = Some(Box::new(move |state, event| handler(state, event).into()));
        self
    }

    /// Set paste event handler.
    /// Returns [`AppEventResult`] to control rendering and quitting.
    pub fn on_paste<F, R>(mut self, handler: F) -> Self
    where
        F: Fn(&mut S, &str) -> R + 'static,
        R: Into<AppEventResult>,
    {
        self.on_paste = Some(Box::new(move |state, text| handler(state, text).into()));
        self
    }

    /// Set resize handler.
    pub fn on_resize<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut S, u16, u16) + 'static,
    {
        self.on_resize = Some(Box::new(handler));
        self
    }

    /// Set frame render callback.
    pub fn on_render<F>(mut self, hook: F) -> Self
    where
        F: Fn(FrameStats) + 'static,
    {
        self.on_render = Some(Box::new(hook));
        self
    }

    /// Set target FPS.
    pub fn fps(mut self, fps: u32) -> Self {
        self.fps = fps;
        self
    }

    /// Set a custom rendering backend.
    pub fn backend<B: Backend + 'static>(mut self, backend: B) -> Self {
        self.backend = Some(Box::new(backend));
        self
    }

    /// Enable/disable alternate screen buffer.
    pub fn alt_screen(mut self, enable: bool) -> Self {
        self.alt_screen = enable;
        self
    }

    /// Get an app handle for external control.
    pub fn handle(&self) -> AppHandle {
        AppHandle {
            should_quit: self.should_quit.clone(),
        }
    }

    /// Run the application.
    pub fn run(mut self) -> Result<(), AppError>
    where
        S: 'static,
    {
        // Install panic hook to restore terminal on crash
        crate::terminal::install_panic_hook();

        let render_fn = self.render_fn.take().ok_or(AppError::NoRenderFunction)?;

        let mut backend = match self.backend.take() {
            Some(backend) => backend,
            None => Box::new(CrosstermBackend::new()?),
        };

        {
            let terminal = backend.terminal();
            terminal.enter_raw_mode()?;

            if self.alt_screen {
                terminal.enter_alt_screen()?;
            }

            terminal.hide_cursor()?;
            terminal.enable_mouse_capture()?;
            terminal.enable_bracketed_paste()?;
        }

        // Cache terminal size - only updated on resize events
        let mut cached_size = backend.terminal().size()?;
        let mut layout_engine = LayoutEngine::new();
        // Double-buffering: Differ owns both buffers, eliminating per-frame clones
        let mut differ = Differ::with_size(cached_size.0, cached_size.1);

        let frame_duration = Duration::from_secs_f64(1.0 / self.fps as f64);
        let mut frame_number = 0u64;
        let mut needs_render = true;
        let mut coalescer = EventCoalescer::new();
        let mut should_quit = false;
        // Store last rendered root for hit testing between frames
        let mut last_root: Option<Node> = None;

        loop {
            let frame_start = Instant::now();

            // Check quit flag
            if self.should_quit.load(std::sync::atomic::Ordering::SeqCst) || should_quit {
                break;
            }

            // Check if signals requested a re-render
            if take_render_request() {
                needs_render = true;
            }

            // Collect events with non-blocking poll while events are available
            let poll_timeout = if needs_render || coalescer.has_pending() {
                0
            } else {
                frame_duration.as_millis() as u64
            };

            // Gather events until queue full or no more events
            while let Some(event) = backend.terminal().poll_event(0)? {
                if coalescer.push(event) {
                    break; // Queue full, process now
                }
            }

            // If we didn't get any events with non-blocking poll and don't need render,
            // do a blocking poll to wait for the next event
            if !coalescer.has_pending() && !needs_render {
                if let Some(event) = backend.terminal().poll_event(poll_timeout)? {
                    coalescer.push(event);
                }
            }

            // Process coalesced resize event (only the latest)
            if let Some((width, height)) = coalescer.take_resize() {
                cached_size = (width, height);
                differ.resize(width, height);
                differ.reset();

                if let Some(ref handler) = self.on_resize {
                    handler(&mut self.state, width, height);
                }

                needs_render = true;
            }

            // Process key events in order
            for key in coalescer.drain_keys() {
                // Built-in quit on Ctrl+C
                if key.code == KeyCode::Char('c') && key.modifiers.ctrl {
                    should_quit = true;
                    break;
                }

                // Built-in Tab/Shift+Tab focus navigation
                match key.code {
                    KeyCode::Tab => {
                        focus_next();
                        needs_render = true;
                        continue;
                    }
                    KeyCode::BackTab => {
                        focus_prev();
                        needs_render = true;
                        continue;
                    }
                    _ => {}
                }

                if let Some(ref handler) = self.on_key {
                    let result = handler(&mut self.state, &key);
                    should_quit |= result.should_quit;
                    if result.needs_render {
                        needs_render = true;
                    }
                    if should_quit {
                        break;
                    }
                }
            }

            // Process paste events in order
            for paste in coalescer.drain_paste() {
                if let Some(ref handler) = self.on_paste {
                    let result = handler(&mut self.state, &paste);
                    should_quit |= result.should_quit;
                    if result.needs_render {
                        needs_render = true;
                    }
                    if should_quit {
                        break;
                    }
                }
            }

            if should_quit {
                break;
            }

            // Process mouse events in order
            for mouse in coalescer.drain_mouse() {
                let mut mouse_needs_render = false;
                if let Some(ref handler) = self.on_mouse {
                    let result = handler(&mut self.state, &mouse);
                    should_quit |= result.should_quit;
                    if result.needs_render {
                        mouse_needs_render = true;
                    }
                    if should_quit {
                        break;
                    }
                }

                // Perform hit testing and dispatch to clickables/draggables if any are registered
                if let Some(ref root) = last_root {
                    let tester = HitTester::new(root, &layout_engine);
                    let hit_result = tester.hit_test(mouse.x, mouse.y);

                    // Dispatch drag events first (drag takes priority over click)
                    let drag_handled = dispatch_drag_event(&mouse, hit_result.as_ref());

                    // Dispatch click events
                    let click_handled = if has_clickables() {
                        dispatch_click_event(&mouse, hit_result.as_ref())
                    } else {
                        false
                    };

                    if drag_handled || click_handled {
                        mouse_needs_render = true;
                    }
                }

                // Dispatch to general mouse hooks
                crate::hooks::dispatch_mouse_event(&mouse);

                if mouse_needs_render {
                    needs_render = true;
                }
            }

            if should_quit {
                break;
            }

            // Render if needed
            if needs_render {
                let mut stats = FrameStats {
                    frame_number,
                    ..Default::default()
                };

                // Use cached size instead of syscall every frame
                let size = cached_size;

                // Build context
                let ctx = Context {
                    state: &self.state,
                    size,
                };

                // Render to node tree
                let root = render_fn(&ctx);

                // Layout
                let layout_start = Instant::now();
                layout_engine.build(&root)?;
                layout_engine.compute(size.0, size.1)?;
                stats.layout_time = layout_start.elapsed();

                // Paint to buffer (using Differ's internal buffer - no clone needed!)
                // Use soft_clear() to enable incremental rendering - only cells that
                // actually change will be marked dirty, avoiding full redraws
                let paint_start = Instant::now();
                let buffer = differ.current_buffer();
                buffer.soft_clear();
                let cursor_pos = render_to_buffer(&root, &layout_engine, buffer);
                stats.paint_time = paint_start.elapsed();

                // Store root for hit testing on subsequent mouse events
                last_root = Some(root);

                // Diff and swap buffers (O(1) - just swaps index, no clone!)
                let diff_start = Instant::now();
                let changes = differ.diff_and_swap();
                stats.diff_time = diff_start.elapsed();

                // Get previous buffer (which was current before swap) for stats
                // INFALLIBLE: diff_and_swap always sets has_prev = true, so prev_buffer() returns Some.
                // If this ever fails, it indicates a bug in Differ's implementation.
                if let Some(prev_buffer) = differ.prev_buffer() {
                    // Output (with synchronized update to prevent tearing)
                    let output_start = Instant::now();
                    backend.render(prev_buffer, &changes)?;

                    // Position and show terminal cursor if any node requested it
                    let terminal = backend.terminal();
                    if let Some((cx, cy)) = cursor_pos {
                        terminal.move_cursor(cx, cy)?;
                        terminal.show_cursor()?;
                    } else {
                        terminal.hide_cursor()?;
                    }

                    stats.output_time = output_start.elapsed();
                    stats.frame_time = frame_start.elapsed();

                    // Only invoke stats callback if registered
                    if let Some(ref hook) = self.on_render {
                        stats.cells_changed = changes.len();
                        hook(stats);
                    }
                }

                needs_render = false;
                frame_number += 1;
            }

            // Frame rate limiting
            let elapsed = frame_start.elapsed();
            if elapsed < frame_duration {
                std::thread::sleep(frame_duration - elapsed);
            }
        }

        // Cleanup handled by Terminal Drop

        Ok(())
    }
}

impl Default for App<()> {
    fn default() -> Self {
        Self::new()
    }
}

// === Async Support ===

/// Application events that can be sent from async tasks.
///
/// Use `AsyncAppHandle::event_sender()` (when the `async` feature is enabled) to get a sender for this channel.
#[derive(Debug, Clone)]
pub enum AppEvent<M> {
    /// Custom message to be processed by the app.
    Message(M),
    /// Request a re-render.
    Render,
    /// Request the application to quit.
    Quit,
}

#[cfg(feature = "async")]
mod async_support {
    #[allow(clippy::wildcard_imports)]
    use super::*;
    use tokio::sync::mpsc;

    /// Async-capable application handle.
    ///
    /// Allows sending events from async tasks to the main app loop.
    #[derive(Clone)]
    pub struct AsyncAppHandle<M: Clone + Send + 'static> {
        /// Channel sender for app events.
        event_tx: mpsc::UnboundedSender<AppEvent<M>>,
        /// Internal quit flag.
        should_quit: std::sync::Arc<std::sync::atomic::AtomicBool>,
    }

    impl<M: Clone + Send + 'static> AsyncAppHandle<M> {
        /// Send a message to the app.
        pub fn send(&self, msg: M) {
            let _ = self.event_tx.send(AppEvent::Message(msg));
        }

        /// Request a re-render.
        pub fn render(&self) {
            let _ = self.event_tx.send(AppEvent::Render);
        }

        /// Request the application to quit.
        pub fn quit(&self) {
            self.should_quit
                .store(true, std::sync::atomic::Ordering::SeqCst);
            let _ = self.event_tx.send(AppEvent::Quit);
        }

        /// Get the event sender for direct channel access.
        pub fn event_sender(&self) -> mpsc::UnboundedSender<AppEvent<M>> {
            self.event_tx.clone()
        }
    }

    /// Async application builder.
    ///
    /// Extends [`App`] with async capabilities using tokio.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use inky::prelude::*;
    /// use inky::app::AsyncApp;
    ///
    /// #[derive(Clone)]
    /// enum Msg {
    ///     StreamDelta(String),
    ///     Done,
    /// }
    ///
    /// #[tokio::main]
    /// async fn main() -> anyhow::Result<()> {
    ///     let app = AsyncApp::new()
    ///         .state(AppState::default())
    ///         .render(|ctx| { /* ... */ })
    ///         .on_message(|state, msg| {
    ///             match msg {
    ///                 Msg::StreamDelta(s) => state.text.push_str(&s),
    ///                 Msg::Done => return true, // quit
    ///             }
    ///             false
    ///         });
    ///
    ///     let handle = app.async_handle();
    ///
    ///     // Spawn async task that sends messages
    ///     let h = handle.clone();
    ///     tokio::spawn(async move {
    ///         h.send(Msg::StreamDelta("Hello".into()));
    ///         h.send(Msg::Done);
    ///     });
    ///
    ///     app.run_async().await
    /// }
    /// ```
    #[allow(clippy::type_complexity)]
    pub struct AsyncApp<S, M: Clone + Send + 'static = ()> {
        state: S,
        render_fn: Option<RenderFn<S>>,
        on_key: Option<KeyHandler<S>>,
        on_mouse: Option<MouseHandler<S>>,
        on_paste: Option<PasteHandler<S>>,
        on_resize: Option<ResizeHandler<S>>,
        on_render: Option<Box<dyn Fn(FrameStats)>>,
        on_message: Option<Box<dyn Fn(&mut S, M) -> AppEventResult>>,
        fps: u32,
        alt_screen: bool,
        backend: Option<Box<dyn Backend>>,
        should_quit: std::sync::Arc<std::sync::atomic::AtomicBool>,
        event_tx: mpsc::UnboundedSender<AppEvent<M>>,
        event_rx: mpsc::UnboundedReceiver<AppEvent<M>>,
    }

    impl AsyncApp<(), ()> {
        /// Create a new async app with default state.
        pub fn new() -> AsyncApp<(), ()> {
            let (event_tx, event_rx) = mpsc::unbounded_channel();
            AsyncApp {
                state: (),
                render_fn: None,
                on_key: None,
                on_mouse: None,
                on_paste: None,
                on_resize: None,
                on_render: None,
                on_message: None,
                fps: 60,
                alt_screen: true,
                backend: None,
                should_quit: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
                event_tx,
                event_rx,
            }
        }
    }

    impl Default for AsyncApp<(), ()> {
        fn default() -> Self {
            Self::new()
        }
    }

    impl<S, M: Clone + Send + 'static> AsyncApp<S, M> {
        /// Set application state.
        pub fn state<T>(self, state: T) -> AsyncApp<T, M> {
            let (event_tx, event_rx) = mpsc::unbounded_channel();
            AsyncApp {
                state,
                render_fn: None,
                on_key: None,
                on_mouse: None,
                on_paste: None,
                on_resize: None,
                on_render: None,
                on_message: None,
                fps: self.fps,
                alt_screen: self.alt_screen,
                backend: self.backend,
                should_quit: self.should_quit,
                event_tx,
                event_rx,
            }
        }

        /// Set the message type for async events.
        pub fn message_type<N: Clone + Send + 'static>(self) -> AsyncApp<S, N> {
            let (event_tx, event_rx) = mpsc::unbounded_channel();
            AsyncApp {
                state: self.state,
                render_fn: self.render_fn,
                on_key: self.on_key,
                on_mouse: self.on_mouse,
                on_paste: self.on_paste,
                on_resize: self.on_resize,
                on_render: self.on_render,
                on_message: None,
                fps: self.fps,
                alt_screen: self.alt_screen,
                backend: self.backend,
                should_quit: self.should_quit,
                event_tx,
                event_rx,
            }
        }

        /// Set the render function.
        pub fn render<F>(mut self, f: F) -> Self
        where
            F: Fn(&Context<S>) -> Node + 'static,
        {
            self.render_fn = Some(Box::new(f));
            self
        }

        /// Set keyboard event handler.
        /// Returns [`AppEventResult`] to control rendering and quitting.
        pub fn on_key<F, R>(mut self, handler: F) -> Self
        where
            F: Fn(&mut S, &crate::terminal::KeyEvent) -> R + 'static,
            R: Into<AppEventResult>,
        {
            self.on_key = Some(Box::new(move |state, event| handler(state, event).into()));
            self
        }

        /// Set mouse event handler.
        ///
        /// The handler receives mouse events including clicks, scrolls, and movement.
        /// Returns [`AppEventResult`] to control rendering and quitting.
        pub fn on_mouse<F, R>(mut self, handler: F) -> Self
        where
            F: Fn(&mut S, &MouseEvent) -> R + 'static,
            R: Into<AppEventResult>,
        {
            self.on_mouse = Some(Box::new(move |state, event| handler(state, event).into()));
            self
        }

        /// Set paste event handler.
        /// Returns [`AppEventResult`] to control rendering and quitting.
        pub fn on_paste<F, R>(mut self, handler: F) -> Self
        where
            F: Fn(&mut S, &str) -> R + 'static,
            R: Into<AppEventResult>,
        {
            self.on_paste = Some(Box::new(move |state, text| handler(state, text).into()));
            self
        }

        /// Set message handler for async events.
        /// Returns [`AppEventResult`] to control rendering and quitting.
        pub fn on_message<F, R>(mut self, handler: F) -> Self
        where
            F: Fn(&mut S, M) -> R + 'static,
            R: Into<AppEventResult>,
        {
            self.on_message = Some(Box::new(move |state, msg| handler(state, msg).into()));
            self
        }

        /// Set resize handler.
        pub fn on_resize<F>(mut self, handler: F) -> Self
        where
            F: Fn(&mut S, u16, u16) + 'static,
        {
            self.on_resize = Some(Box::new(handler));
            self
        }

        /// Set frame render callback.
        pub fn on_render<F>(mut self, hook: F) -> Self
        where
            F: Fn(FrameStats) + 'static,
        {
            self.on_render = Some(Box::new(hook));
            self
        }

        /// Set target FPS.
        pub fn fps(mut self, fps: u32) -> Self {
            self.fps = fps;
            self
        }

        /// Set a custom rendering backend.
        pub fn backend<B: Backend + 'static>(mut self, backend: B) -> Self {
            self.backend = Some(Box::new(backend));
            self
        }

        /// Enable/disable alternate screen buffer.
        pub fn alt_screen(mut self, enable: bool) -> Self {
            self.alt_screen = enable;
            self
        }

        /// Get an async app handle for external control.
        pub fn async_handle(&self) -> AsyncAppHandle<M> {
            AsyncAppHandle {
                event_tx: self.event_tx.clone(),
                should_quit: self.should_quit.clone(),
            }
        }

        /// Run the application asynchronously with tokio.
        ///
        /// This method integrates with tokio's async runtime, allowing:
        /// - Concurrent async task communication via channels
        /// - Non-blocking event handling
        /// - Integration with external async streams
        ///
        /// Note: The returned future is not `Send` because render functions
        /// and handlers may capture non-Send types. If you need a `Send` future,
        /// ensure all closures passed to the app are `Send + Sync`.
        #[allow(clippy::future_not_send)]
        pub async fn run_async(mut self) -> Result<(), AppError>
        where
            S: 'static,
        {
            use tokio::time::{interval, Duration as TokioDuration};

            // Install panic hook to restore terminal on crash
            crate::terminal::install_panic_hook();

            let render_fn = self.render_fn.take().ok_or(AppError::NoRenderFunction)?;

            let mut backend = match self.backend.take() {
                Some(backend) => backend,
                None => Box::new(CrosstermBackend::new()?),
            };

            {
                let terminal = backend.terminal();
                terminal.enter_raw_mode()?;

                if self.alt_screen {
                    terminal.enter_alt_screen()?;
                }

                terminal.hide_cursor()?;
                terminal.enable_mouse_capture()?;
                terminal.enable_bracketed_paste()?;
            }

            // Cache terminal size - only updated on resize events
            let mut cached_size = backend.terminal().size()?;
            let mut layout_engine = LayoutEngine::new();
            // Double-buffering: Differ owns both buffers, eliminating per-frame clones
            let mut differ = Differ::with_size(cached_size.0, cached_size.1);

            let frame_duration = TokioDuration::from_secs_f64(1.0 / self.fps as f64);
            let mut frame_number = 0u64;
            let mut needs_render = true;
            let mut coalescer = EventCoalescer::new();
            let mut should_quit = false;
            // Store last rendered root for hit testing between frames
            let mut last_root: Option<Node> = None;

            let mut render_interval = interval(frame_duration);

            loop {
                // Check quit flag
                if self.should_quit.load(std::sync::atomic::Ordering::SeqCst) || should_quit {
                    break;
                }

                // Check if signals requested a re-render
                if take_render_request() {
                    needs_render = true;
                }

                // Collect terminal events with non-blocking poll
                while let Some(event) = backend.terminal().poll_event(0)? {
                    if coalescer.push(event) {
                        break; // Queue full, process now
                    }
                }

                // Process async events or wait for next frame
                tokio::select! {
                    biased;

                    // Check for async messages from tasks
                    Some(event) = self.event_rx.recv() => {
                        match event {
                            AppEvent::Message(msg) => {
                                if let Some(ref handler) = self.on_message {
                                    let result = handler(&mut self.state, msg);
                                    should_quit |= result.should_quit;
                                    if result.needs_render {
                                        needs_render = true;
                                    }
                                }
                            }
                            AppEvent::Render => {
                                needs_render = true;
                            }
                            AppEvent::Quit => {
                                should_quit = true;
                            }
                        }
                    }

                    // Frame tick for rendering
                    _ = render_interval.tick() => {
                        // Just continue to rendering
                    }
                }

                // Process coalesced resize event (only the latest)
                if let Some((width, height)) = coalescer.take_resize() {
                    cached_size = (width, height);
                    differ.resize(width, height);
                    differ.reset();

                    if let Some(ref handler) = self.on_resize {
                        handler(&mut self.state, width, height);
                    }

                    needs_render = true;
                }

                // Process key events in order
                for key in coalescer.drain_keys() {
                    // Built-in quit on Ctrl+C
                    if key.code == KeyCode::Char('c') && key.modifiers.ctrl {
                        should_quit = true;
                        break;
                    }

                    // Built-in Tab/Shift+Tab focus navigation
                    match key.code {
                        KeyCode::Tab => {
                            focus_next();
                            needs_render = true;
                            continue;
                        }
                        KeyCode::BackTab => {
                            focus_prev();
                            needs_render = true;
                            continue;
                        }
                        _ => {}
                    }

                    if let Some(ref handler) = self.on_key {
                        let result = handler(&mut self.state, &key);
                        should_quit |= result.should_quit;
                        if result.needs_render {
                            needs_render = true;
                        }
                        if should_quit {
                            break;
                        }
                    }
                }

                // Process paste events in order
                for paste in coalescer.drain_paste() {
                    if let Some(ref handler) = self.on_paste {
                        let result = handler(&mut self.state, &paste);
                        should_quit |= result.should_quit;
                        if result.needs_render {
                            needs_render = true;
                        }
                        if should_quit {
                            break;
                        }
                    }
                }

                if should_quit {
                    break;
                }

                // Process mouse events in order
                for mouse in coalescer.drain_mouse() {
                    let mut mouse_needs_render = false;
                    if let Some(ref handler) = self.on_mouse {
                        let result = handler(&mut self.state, &mouse);
                        should_quit |= result.should_quit;
                        if result.needs_render {
                            mouse_needs_render = true;
                        }
                        if should_quit {
                            break;
                        }
                    }

                    // Perform hit testing and dispatch to clickables/draggables if any are registered
                    if let Some(ref root) = last_root {
                        let tester = HitTester::new(root, &layout_engine);
                        let hit_result = tester.hit_test(mouse.x, mouse.y);

                        // Dispatch drag events first (drag takes priority over click)
                        let drag_handled = dispatch_drag_event(&mouse, hit_result.as_ref());

                        // Dispatch click events
                        let click_handled = if has_clickables() {
                            dispatch_click_event(&mouse, hit_result.as_ref())
                        } else {
                            false
                        };

                        if drag_handled || click_handled {
                            mouse_needs_render = true;
                        }
                    }

                    // Dispatch to general mouse hooks
                    crate::hooks::dispatch_mouse_event(&mouse);

                    if mouse_needs_render {
                        needs_render = true;
                    }
                }

                if should_quit {
                    break;
                }

                // Render if needed
                if needs_render {
                    let mut stats = FrameStats {
                        frame_number,
                        ..Default::default()
                    };

                    // Use cached size instead of syscall every frame
                    let size = cached_size;

                    // Build context
                    let ctx = Context {
                        state: &self.state,
                        size,
                    };

                    // Render to node tree
                    let root = render_fn(&ctx);

                    // Layout
                    let layout_start = Instant::now();
                    layout_engine.build(&root)?;
                    layout_engine.compute(size.0, size.1)?;
                    stats.layout_time = layout_start.elapsed();

                    // Paint to buffer (using Differ's internal buffer - no clone needed!)
                    // Use soft_clear() to enable incremental rendering - only cells that
                    // actually change will be marked dirty, avoiding full redraws
                    let paint_start = Instant::now();
                    let buffer = differ.current_buffer();
                    buffer.soft_clear();
                    let cursor_pos = render_to_buffer(&root, &layout_engine, buffer);
                    stats.paint_time = paint_start.elapsed();

                    // Store root for hit testing on subsequent mouse events
                    last_root = Some(root);

                    // Diff and swap buffers (O(1) - just swaps index, no clone!)
                    let diff_start = Instant::now();
                    let changes = differ.diff_and_swap();
                    stats.diff_time = diff_start.elapsed();

                    // Get previous buffer (which was current before swap) for stats
                    // INFALLIBLE: diff_and_swap always sets has_prev = true, so prev_buffer() returns Some.
                    // If this ever fails, it indicates a bug in Differ's implementation.
                    if let Some(prev_buffer) = differ.prev_buffer() {
                        // Output (with synchronized update to prevent tearing)
                        let output_start = Instant::now();
                        backend.render(prev_buffer, &changes)?;

                        // Position and show terminal cursor if any node requested it
                        let terminal = backend.terminal();
                        if let Some((cx, cy)) = cursor_pos {
                            terminal.move_cursor(cx, cy)?;
                            terminal.show_cursor()?;
                        } else {
                            terminal.hide_cursor()?;
                        }

                        stats.output_time = output_start.elapsed();
                        stats.frame_time = stats.layout_time
                            + stats.paint_time
                            + stats.diff_time
                            + stats.output_time;

                        // Only invoke stats callback if registered
                        if let Some(ref hook) = self.on_render {
                            stats.cells_changed = changes.len();
                            hook(stats);
                        }
                    }

                    needs_render = false;
                    frame_number += 1;
                }
            }

            // Cleanup handled by Terminal Drop

            Ok(())
        }
    }
}

/// External state support for applications that manage their own state.
///
/// This module provides an alternative to the owned-state `App` model,
/// allowing applications to bring their own state management (Redux-like patterns,
/// complex nested state, non-Clone types like `JoinHandle`, etc.).
///
/// # Example
///
/// ```ignore
/// use inky::prelude::*;
/// use inky::app::RenderOnce;
/// use std::sync::Arc;
/// use tokio::task::JoinHandle;
///
/// // Complex state that can't be owned by App
/// struct AppState {
///     agent_handle: Option<JoinHandle<()>>,
///     history: Vec<Message>,
///     tx: mpsc::UnboundedSender<Event>,
/// }
///
/// let mut state = AppState { /* ... */ };
///
/// // Render once with external state
/// RenderOnce::render(&state, |ctx| {
///     vbox![text!("Messages: {}", ctx.state.history.len())]
/// })?;
/// ```
pub mod external_state {
    use super::{
        AppError, Backend, CrosstermBackend, Differ, FrameStats, LayoutEngine, Node, TerminalEvent,
    };
    use crate::render::render_to_buffer;

    /// Context for external state rendering.
    pub struct ExternalContext<'a, S> {
        /// Reference to external state.
        pub state: &'a S,
        /// Terminal size.
        pub size: (u16, u16),
    }

    impl<'a, S> ExternalContext<'a, S> {
        /// Get terminal width.
        pub fn width(&self) -> u16 {
            self.size.0
        }

        /// Get terminal height.
        pub fn height(&self) -> u16 {
            self.size.1
        }
    }

    /// Single-shot renderer for external state management.
    ///
    /// Useful for applications that:
    /// - Own their state externally (e.g., in an async task)
    /// - Use non-Clone types (JoinHandle, channels, etc.)
    /// - Want fine-grained control over the render loop
    ///
    /// # Example
    ///
    /// ```ignore
    /// use inky::app::RenderOnce;
    ///
    /// struct MyState {
    ///     counter: u32,
    /// }
    ///
    /// let state = MyState { counter: 42 };
    ///
    /// // Render once without taking ownership of state
    /// RenderOnce::render(&state, |ctx| {
    ///     TextNode::new(format!("Count: {}", ctx.state.counter)).into()
    /// })?;
    /// ```
    pub struct RenderOnce;

    impl RenderOnce {
        /// Render a single frame with external state.
        ///
        /// This is useful for rendering with state that can't be moved into App,
        /// or for tight integration with external event loops.
        pub fn render<S, F>(state: &S, render_fn: F) -> Result<(), AppError>
        where
            F: Fn(&ExternalContext<S>) -> Node,
        {
            Self::render_to_backend(state, render_fn, CrosstermBackend::new()?)
        }

        /// Render a single frame with external state to a custom backend.
        pub fn render_to_backend<S, F, B: Backend>(
            state: &S,
            render_fn: F,
            mut backend: B,
        ) -> Result<(), AppError>
        where
            F: Fn(&ExternalContext<S>) -> Node,
        {
            let terminal = backend.terminal();
            let size = terminal.size()?;

            let ctx = ExternalContext { state, size };
            let root = render_fn(&ctx);

            let mut layout_engine = LayoutEngine::new();
            let mut buffer = crate::render::Buffer::new(size.0, size.1);

            layout_engine.build(&root)?;
            layout_engine.compute(size.0, size.1)?;
            let cursor_pos = render_to_buffer(&root, &layout_engine, &mut buffer);

            // Output the buffer using the differ
            let mut differ = Differ::with_size(size.0, size.1);
            // Copy buffer to differ's current buffer
            let differ_buf = differ.current_buffer();
            for y in 0..size.1 {
                for x in 0..size.0 {
                    if let Some(cell) = buffer.get(x, y) {
                        differ_buf.set(x, y, *cell);
                    }
                }
            }
            let changes = differ.diff_and_swap();
            let mut output = std::io::stdout();
            crate::diff::apply_changes(&mut output, &changes)?;

            // Position and show terminal cursor if any node requested it
            if let Some((cx, cy)) = cursor_pos {
                terminal.move_cursor(cx, cy)?;
                terminal.show_cursor()?;
            } else {
                terminal.hide_cursor()?;
            }

            Ok(())
        }
    }

    /// Streaming renderer for external state management.
    ///
    /// Provides a low-level API for applications that want full control
    /// over the render loop while using external state.
    pub struct StreamingRenderer {
        backend: Box<dyn Backend>,
        layout_engine: LayoutEngine,
        differ: Differ,
        frame_number: u64,
    }

    impl StreamingRenderer {
        /// Create a new streaming renderer.
        pub fn new() -> Result<Self, AppError> {
            Self::with_backend(CrosstermBackend::new()?)
        }

        /// Create a streaming renderer with a custom backend.
        pub fn with_backend<B: Backend + 'static>(backend: B) -> Result<Self, AppError> {
            let mut backend: Box<dyn Backend> = Box::new(backend);
            let size = backend.terminal().size()?;

            Ok(Self {
                backend,
                layout_engine: LayoutEngine::new(),
                differ: Differ::with_size(size.0, size.1),
                frame_number: 0,
            })
        }

        /// Initialize the terminal (raw mode, alt screen, hide cursor, mouse capture).
        pub fn init(&mut self) -> Result<(), AppError> {
            let terminal = self.backend.terminal();
            terminal.enter_raw_mode()?;
            terminal.enter_alt_screen()?;
            terminal.hide_cursor()?;
            terminal.enable_mouse_capture()?;
            terminal.enable_bracketed_paste()?;
            Ok(())
        }

        /// Cleanup the terminal (disable mouse, leave alt screen, show cursor, leave raw mode).
        ///
        /// This is called automatically on drop, but can be called manually.
        pub fn cleanup(&mut self) -> Result<(), AppError> {
            let terminal = self.backend.terminal();
            terminal.disable_mouse_capture()?;
            terminal.disable_bracketed_paste()?;
            terminal.show_cursor()?;
            terminal.leave_alt_screen()?;
            terminal.leave_raw_mode()?;
            Ok(())
        }

        /// Get the current terminal size.
        pub fn size(&mut self) -> Result<(u16, u16), AppError> {
            Ok(self.backend.terminal().size()?)
        }

        /// Handle a resize event.
        pub fn handle_resize(&mut self, width: u16, height: u16) {
            self.differ.resize(width, height);
        }

        /// Poll for terminal events (non-blocking).
        pub fn poll_event(&mut self, timeout_ms: u64) -> Result<Option<TerminalEvent>, AppError> {
            Ok(self.backend.terminal().poll_event(timeout_ms)?)
        }

        /// Render a frame with external state.
        ///
        /// Returns frame statistics for performance monitoring.
        pub fn render_frame<S, F>(
            &mut self,
            state: &S,
            render_fn: F,
        ) -> Result<FrameStats, AppError>
        where
            F: Fn(&ExternalContext<S>) -> Node,
        {
            use std::time::Instant;

            let frame_start = Instant::now();
            let size = self.backend.terminal().size()?;

            let ctx = ExternalContext { state, size };
            let root = render_fn(&ctx);

            // Layout
            let layout_start = Instant::now();
            self.layout_engine.build(&root)?;
            self.layout_engine.compute(size.0, size.1)?;
            let layout_time = layout_start.elapsed();

            // Paint
            // Use soft_clear() to enable incremental rendering - only cells that
            // actually change will be marked dirty, avoiding full redraws
            let paint_start = Instant::now();
            let buffer = self.differ.current_buffer();
            buffer.soft_clear();
            let cursor_pos = render_to_buffer(&root, &self.layout_engine, buffer);
            let paint_time = paint_start.elapsed();

            // Diff and output
            let diff_start = Instant::now();
            let changes = self.differ.diff_and_swap();
            let diff_time = diff_start.elapsed();

            let output_start = Instant::now();
            let mut output = std::io::stdout();
            let cells_changed = changes.len();
            crate::diff::apply_changes(&mut output, &changes)?;

            // Position and show terminal cursor if any node requested it
            let terminal = self.backend.terminal();
            if let Some((cx, cy)) = cursor_pos {
                terminal.move_cursor(cx, cy)?;
                terminal.show_cursor()?;
            } else {
                terminal.hide_cursor()?;
            }

            let output_time = output_start.elapsed();

            self.frame_number += 1;

            Ok(FrameStats {
                frame_time: frame_start.elapsed(),
                layout_time,
                paint_time,
                diff_time,
                output_time,
                cells_changed,
                frame_number: self.frame_number,
            })
        }

        /// Get the frame number.
        pub fn frame_number(&self) -> u64 {
            self.frame_number
        }
    }

    impl Drop for StreamingRenderer {
        fn drop(&mut self) {
            // Best-effort cleanup
            let _ = self.cleanup();
        }
    }
}

pub use external_state::{ExternalContext, RenderOnce, StreamingRenderer};

#[cfg(feature = "async")]
pub use async_support::{AsyncApp, AsyncAppHandle};

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::terminal::KeyModifiers;

    #[test]
    fn test_event_coalescer_resize_coalescing() {
        let mut coalescer = EventCoalescer::new();

        // Multiple resize events should be coalesced to just the last one
        coalescer.push(TerminalEvent::Resize {
            width: 80,
            height: 24,
        });
        coalescer.push(TerminalEvent::Resize {
            width: 100,
            height: 30,
        });
        coalescer.push(TerminalEvent::Resize {
            width: 120,
            height: 40,
        });

        assert!(coalescer.has_pending());

        // Should get only the last resize
        let resize = coalescer.take_resize();
        assert_eq!(resize, Some((120, 40)));

        // No more resize events
        assert!(coalescer.take_resize().is_none());
    }

    #[test]
    fn test_event_coalescer_key_batching() {
        let mut coalescer = EventCoalescer::new();

        // Add several key events
        for c in ['a', 'b', 'c'] {
            coalescer.push(TerminalEvent::Key(KeyEvent {
                code: KeyCode::Char(c),
                modifiers: KeyModifiers::NONE,
            }));
        }

        assert!(coalescer.has_pending());

        // Should get all keys in order
        let keys: Vec<_> = coalescer.drain_keys().collect();
        assert_eq!(keys.len(), 3);
        assert_eq!(keys[0].code, KeyCode::Char('a'));
        assert_eq!(keys[1].code, KeyCode::Char('b'));
        assert_eq!(keys[2].code, KeyCode::Char('c'));

        // No more keys
        assert!(!coalescer.has_pending());
    }

    #[test]
    fn test_event_coalescer_max_keys() {
        let mut coalescer = EventCoalescer::new();

        // Fill up to max
        for i in 0..EventCoalescer::MAX_PENDING_KEYS {
            let full = coalescer.push(TerminalEvent::Key(KeyEvent {
                code: KeyCode::Char(char::from(b'a' + i as u8)),
                modifiers: KeyModifiers::NONE,
            }));
            assert!(!full, "Should not be full at {}", i);
        }

        // Next push should signal full
        let full = coalescer.push(TerminalEvent::Key(KeyEvent {
            code: KeyCode::Enter,
            modifiers: KeyModifiers::NONE,
        }));
        assert!(full);
    }

    #[test]
    fn test_event_coalescer_mouse_batching() {
        use crate::terminal::MouseEventKind;

        let mut coalescer = EventCoalescer::new();

        // Add several mouse events
        for i in 0..5 {
            coalescer.push(TerminalEvent::Mouse(MouseEvent {
                button: None,
                kind: MouseEventKind::Moved,
                x: i * 10,
                y: i * 5,
                modifiers: KeyModifiers::NONE,
            }));
        }

        assert!(coalescer.has_pending());

        // Should get all mouse events in order
        let events: Vec<_> = coalescer.drain_mouse().collect();
        assert_eq!(events.len(), 5);
        assert_eq!(events[0].x, 0);
        assert_eq!(events[1].x, 10);
        assert_eq!(events[4].x, 40);

        // No more mouse events
        assert!(!coalescer.has_pending());
    }

    #[test]
    fn test_event_coalescer_max_mouse() {
        use crate::terminal::MouseEventKind;

        let mut coalescer = EventCoalescer::new();

        // Fill up to max
        for i in 0..EventCoalescer::MAX_PENDING_MOUSE {
            let full = coalescer.push(TerminalEvent::Mouse(MouseEvent {
                button: None,
                kind: MouseEventKind::Moved,
                x: i as u16,
                y: 0,
                modifiers: KeyModifiers::NONE,
            }));
            assert!(!full, "Should not be full at {}", i);
        }

        // Next push should signal full
        let full = coalescer.push(TerminalEvent::Mouse(MouseEvent {
            button: None,
            kind: MouseEventKind::Moved,
            x: 999,
            y: 999,
            modifiers: KeyModifiers::NONE,
        }));
        assert!(full);
    }

    #[test]
    fn test_event_coalescer_paste_batching() {
        let mut coalescer = EventCoalescer::new();

        coalescer.push(TerminalEvent::Paste("one".to_string()));
        coalescer.push(TerminalEvent::Paste("two".to_string()));

        assert!(coalescer.has_pending());

        let pastes: Vec<_> = coalescer.drain_paste().collect();
        assert_eq!(pastes, vec!["one".to_string(), "two".to_string()]);

        assert!(!coalescer.has_pending());
    }

    #[test]
    fn test_event_coalescer_mixed_events() {
        use crate::terminal::MouseEventKind;

        let mut coalescer = EventCoalescer::new();

        // Add mix of events
        coalescer.push(TerminalEvent::Key(KeyEvent {
            code: KeyCode::Char('a'),
            modifiers: KeyModifiers::NONE,
        }));
        coalescer.push(TerminalEvent::Mouse(MouseEvent {
            button: None,
            kind: MouseEventKind::Moved,
            x: 10,
            y: 20,
            modifiers: KeyModifiers::NONE,
        }));
        coalescer.push(TerminalEvent::Resize {
            width: 100,
            height: 50,
        });
        coalescer.push(TerminalEvent::Key(KeyEvent {
            code: KeyCode::Char('b'),
            modifiers: KeyModifiers::NONE,
        }));
        coalescer.push(TerminalEvent::Mouse(MouseEvent {
            button: None,
            kind: MouseEventKind::ScrollUp,
            x: 0,
            y: 0,
            modifiers: KeyModifiers::NONE,
        }));

        assert!(coalescer.has_pending());

        // Drain each type
        let resize = coalescer.take_resize();
        assert_eq!(resize, Some((100, 50)));

        let keys: Vec<_> = coalescer.drain_keys().collect();
        assert_eq!(keys.len(), 2);
        assert_eq!(keys[0].code, KeyCode::Char('a'));
        assert_eq!(keys[1].code, KeyCode::Char('b'));

        let mouse_events: Vec<_> = coalescer.drain_mouse().collect();
        assert_eq!(mouse_events.len(), 2);
        assert_eq!(mouse_events[0].kind, MouseEventKind::Moved);
        assert_eq!(mouse_events[1].kind, MouseEventKind::ScrollUp);

        assert!(!coalescer.has_pending());
    }

    #[test]
    fn test_app_event_result_from_bool() {
        assert_eq!(AppEventResult::from(false), AppEventResult::render());
        assert_eq!(AppEventResult::from(true), AppEventResult::quit());
    }
}
