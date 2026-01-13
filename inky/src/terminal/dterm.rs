//! dterm GPU backend integration.
//!
//! This module provides GPU-accelerated rendering through dterm, with optional
//! shared memory IPC for zero-copy buffer access by AI agents.
//!
//! # Shared Memory IPC
//!
//! When enabled, the backend writes cell data to a memory-mapped file that
//! can be read by external processes (like AI agents) without copying:
//!
//! ```ignore
//! let mut backend = DtermBackend::new()?;
//! backend.enable_shared_memory()?;
//!
//! // Render normally - shared memory is updated automatically
//! backend.render(&buffer, &changes)?;
//!
//! // AI agent can now read from shared_buffer_path(std::process::id())
//! ```

use crate::diff::{apply_changes, Change};
use crate::render::gpu::{copy_buffer_to_gpu_dirty, GpuBuffer, GpuCell};
use crate::render::ipc::SharedMemoryBuffer;
use crate::render::Buffer;
use crate::terminal::{Backend, CrosstermTerminal, Terminal};
use std::io;
use std::path::Path;
use std::ptr::NonNull;

use dterm_core::ffi as dterm_ffi;
use dterm_core::gpu::ffi as dterm_gpu_ffi;

struct DtermTerminalHandle {
    ptr: NonNull<dterm_ffi::DtermTerminal>,
}

impl DtermTerminalHandle {
    fn new(rows: u16, cols: u16) -> io::Result<Self> {
        let ptr = dterm_ffi::dterm_terminal_new(rows, cols);
        let ptr = NonNull::new(ptr).ok_or_else(|| {
            io::Error::new(io::ErrorKind::Other, "failed to create dterm terminal")
        })?;
        Ok(Self { ptr })
    }

    fn resize(&mut self, rows: u16, cols: u16) {
        // SAFETY: `self.ptr` is a valid, non-null pointer to a DtermTerminal
        // that was successfully created in `new()`. The pointer remains valid
        // for the lifetime of `DtermTerminalHandle` until `drop()` is called.
        // `dterm_terminal_resize` is FFI-safe and does not take ownership.
        unsafe { dterm_ffi::dterm_terminal_resize(self.ptr.as_ptr(), rows, cols) };
    }

    fn process(&mut self, data: &[u8]) {
        if data.is_empty() {
            return;
        }
        // SAFETY: `self.ptr` is a valid, non-null pointer to a DtermTerminal
        // that was successfully created in `new()`. `data.as_ptr()` and `data.len()`
        // provide a valid slice reference that the FFI function reads from but
        // does not store beyond this call. The slice remains valid for the
        // duration of the FFI call.
        unsafe {
            dterm_ffi::dterm_terminal_process(self.ptr.as_ptr(), data.as_ptr(), data.len());
        }
    }
}

impl Drop for DtermTerminalHandle {
    fn drop(&mut self) {
        // SAFETY: `self.ptr` is a valid, non-null pointer to a DtermTerminal
        // that was successfully created in `new()`. This is the only place
        // where `dterm_terminal_free` is called, and it happens exactly once
        // when the handle is dropped. After this call, the pointer is invalid
        // but will never be used again since the handle is being destroyed.
        unsafe { dterm_ffi::dterm_terminal_free(self.ptr.as_ptr()) };
    }
}

/// GPU buffer backed by dterm-compatible cell storage.
#[derive(Clone)]
pub struct DtermGpuBuffer {
    cells: Vec<GpuCell>,
    width: u16,
    height: u16,
}

impl DtermGpuBuffer {
    /// Create a new dterm GPU buffer.
    pub fn new(width: u16, height: u16) -> Self {
        let size = (width as usize).saturating_mul(height as usize);
        Self {
            cells: vec![GpuCell::blank(); size],
            width,
            height,
        }
    }

    /// Get the cells as a slice.
    pub fn cells(&self) -> &[GpuCell] {
        &self.cells
    }
}

impl GpuBuffer for DtermGpuBuffer {
    fn width(&self) -> u16 {
        self.width
    }

    fn height(&self) -> u16 {
        self.height
    }

    fn map_write(&mut self) -> &mut [GpuCell] {
        &mut self.cells
    }

    fn unmap(&mut self) {
        // No-op: CPU buffer staging for dterm renderer.
    }

    fn submit(&mut self) {
        // Signal dterm GPU renderer that buffer is ready.
        // When running in dterm context, this triggers a GPU render pass.
        if dterm_gpu_ffi::dterm_renderer_available() {
            // SAFETY: `self.cells` is a valid Vec<GpuCell> where GpuCell is a
            // `#[repr(C)]` struct with 8-byte packed layout matching dterm's
            // expected cell format. The pointer and byte length are derived from
            // a valid slice, and the renderer only reads from this buffer during
            // the FFI call (it copies data to GPU memory). The Vec remains valid
            // and owned by us; we do not transfer ownership.
            unsafe {
                dterm_gpu_ffi::dterm_renderer_submit_cells(
                    self.cells.as_ptr() as *const u8,
                    self.cells.len() * std::mem::size_of::<GpuCell>(),
                    self.width,
                    self.height,
                );
            }
        }
    }

    fn resize(&mut self, width: u16, height: u16) -> bool {
        if width == self.width && height == self.height {
            return true;
        }

        let mut new_cells =
            vec![GpuCell::blank(); (width as usize).saturating_mul(height as usize)];
        let copy_width = self.width.min(width);
        let copy_height = self.height.min(height);

        for y in 0..copy_height {
            for x in 0..copy_width {
                let old_idx = (y as usize) * (self.width as usize) + (x as usize);
                let new_idx = (y as usize) * (width as usize) + (x as usize);
                new_cells[new_idx] = self.cells[old_idx];
            }
        }

        self.cells = new_cells;
        self.width = width;
        self.height = height;
        true
    }

    fn is_available(&self) -> bool {
        dterm_gpu_ffi::dterm_renderer_available()
    }
}

/// dterm backend with GPU buffer integration and ANSI fallback.
///
/// This backend provides:
/// - GPU-accelerated rendering via dterm when available
/// - Automatic fallback to ANSI terminal output
/// - Optional shared memory IPC for AI agent access
pub struct DtermBackend {
    terminal: CrosstermTerminal,
    dterm: DtermTerminalHandle,
    gpu_buffer: DtermGpuBuffer,
    scratch: Vec<u8>,
    /// Optional shared memory buffer for IPC with AI agents.
    shared_buffer: Option<SharedMemoryBuffer>,
}

impl DtermBackend {
    /// Create a new dterm backend.
    pub fn new() -> io::Result<Self> {
        let terminal = CrosstermTerminal::new()?;
        let (cols, rows) = terminal.size()?;
        let dterm = DtermTerminalHandle::new(rows, cols)?;
        let gpu_buffer = DtermGpuBuffer::new(cols, rows);
        Ok(Self {
            terminal,
            dterm,
            gpu_buffer,
            scratch: Vec::new(),
            shared_buffer: None,
        })
    }

    /// Enable shared memory IPC for AI agent access.
    ///
    /// Creates a memory-mapped file that external processes can read to
    /// observe terminal state without copying. The file is created at the
    /// default path: `/dev/shm/inky-$PID-buffer` (Linux) or
    /// `$TMPDIR/inky-$PID-buffer.shm` (macOS).
    ///
    /// # Errors
    ///
    /// Returns an error if the shared memory file cannot be created.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut backend = DtermBackend::new()?;
    /// backend.enable_shared_memory()?;
    ///
    /// // AI agent can now read from shared_buffer_path(std::process::id())
    /// ```
    pub fn enable_shared_memory(&mut self) -> io::Result<()> {
        let width = self.gpu_buffer.width();
        let height = self.gpu_buffer.height();
        let shared = SharedMemoryBuffer::create(width, height)?;
        self.shared_buffer = Some(shared);
        Ok(())
    }

    /// Enable shared memory IPC at a specific path.
    ///
    /// Like [`enable_shared_memory`](Self::enable_shared_memory), but creates
    /// the shared memory file at a custom path.
    pub fn enable_shared_memory_at(&mut self, path: &Path) -> io::Result<()> {
        let width = self.gpu_buffer.width();
        let height = self.gpu_buffer.height();
        let shared = SharedMemoryBuffer::create_at(path, width, height)?;
        self.shared_buffer = Some(shared);
        Ok(())
    }

    /// Disable shared memory IPC.
    ///
    /// The shared memory file is deleted when this is called.
    pub fn disable_shared_memory(&mut self) {
        self.shared_buffer = None;
    }

    /// Check if shared memory IPC is enabled.
    pub fn has_shared_memory(&self) -> bool {
        self.shared_buffer.is_some()
    }

    /// Get the path to the shared memory file.
    ///
    /// Returns `None` if shared memory is not enabled.
    pub fn shared_memory_path(&self) -> Option<&Path> {
        self.shared_buffer.as_ref().map(|s| s.path())
    }

    /// Get the current generation counter from shared memory.
    ///
    /// External processes can poll this value to detect updates.
    /// Returns `None` if shared memory is not enabled.
    pub fn shared_memory_generation(&self) -> Option<u64> {
        self.shared_buffer.as_ref().map(|s| s.generation())
    }

    fn sync_size(&mut self, width: u16, height: u16) {
        if width != self.gpu_buffer.width() || height != self.gpu_buffer.height() {
            self.gpu_buffer.resize(width, height);
            self.dterm.resize(height, width);

            // Resize shared buffer if enabled
            if let Some(ref mut shared) = self.shared_buffer {
                shared.resize(width, height);
            }
        }
    }

    /// Sync GPU buffer contents to shared memory.
    fn sync_shared_buffer(&mut self) {
        if let Some(ref mut shared) = self.shared_buffer {
            // Copy cells from GPU buffer to shared memory
            let src = self.gpu_buffer.cells();
            let dst = shared.map_write();

            // Only copy if sizes match (they should after sync_size)
            let len = src.len().min(dst.len());
            dst[..len].copy_from_slice(&src[..len]);

            shared.unmap();
            shared.submit();
        }
    }

    fn update_dterm(&mut self, changes: &[Change]) -> io::Result<()> {
        if changes.is_empty() {
            return Ok(());
        }

        self.scratch.clear();
        apply_changes(&mut self.scratch, changes)?;
        self.dterm.process(&self.scratch);
        Ok(())
    }
}

impl Backend for DtermBackend {
    fn terminal(&mut self) -> &mut dyn Terminal {
        &mut self.terminal
    }

    fn render(&mut self, buffer: &Buffer, changes: &[Change]) -> io::Result<()> {
        self.sync_size(buffer.width(), buffer.height());

        // Use dirty-cell tracking for incremental GPU updates.
        // Only cells with the DIRTY flag are converted and copied,
        // reducing CPU→GPU bandwidth for typical UI updates (<5% cells change).
        copy_buffer_to_gpu_dirty(buffer, &mut self.gpu_buffer);

        // When GPU renderer is available, it handles display directly via Metal/wgpu.
        // Skip redundant ANSI output to stdout to avoid double-rendering overhead.
        if self.gpu_buffer.is_available() {
            self.gpu_buffer.submit();
        } else {
            // Fallback: GPU not available, use ANSI terminal output
            self.terminal.begin_sync()?;
            let mut stdout = io::stdout();
            apply_changes(&mut stdout, changes)?;
            self.terminal.end_sync()?;

            // Also update dterm's internal terminal state for consistency
            self.update_dterm(changes)?;
        }

        // Sync to shared memory for AI agent access (if enabled)
        self.sync_shared_buffer();

        Ok(())
    }
}
