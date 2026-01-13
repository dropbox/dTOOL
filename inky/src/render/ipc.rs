//! Shared memory IPC for zero-copy buffer access.
//!
//! This module provides shared memory-backed GPU buffers that enable
//! external processes (like AI agents) to directly read/write terminal
//! cell data without copying.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                    Shared Memory Region                         │
//! ├──────────────┬──────────────────────────────────────────────────┤
//! │    Header    │                  Cell Data                       │
//! │  (64 bytes)  │            (width × height × 8 bytes)            │
//! └──────────────┴──────────────────────────────────────────────────┘
//! ```
//!
//! # Protocol
//!
//! 1. **Creation**: inky creates a memory-mapped file and writes the header
//! 2. **Discovery**: External processes find the file at a well-known path
//! 3. **Access**: Both processes can read/write cells directly
//! 4. **Synchronization**: The header contains a generation counter for
//!    detecting updates (atomic operations ensure consistency)
//!
//! # Well-Known Paths
//!
//! | Platform | Path |
//! |----------|------|
//! | macOS    | `$TMPDIR/inky-$PID-buffer.shm` |
//! | Linux    | `/dev/shm/inky-$PID-buffer` or `/tmp/inky-$PID-buffer.shm` |
//!
//! # Example
//!
//! ```ignore
//! use inky::render::ipc::SharedMemoryBuffer;
//!
//! // Create a new shared memory buffer
//! let mut buffer = SharedMemoryBuffer::create(200, 50)?;
//!
//! // Get the path for external processes
//! let path = buffer.path();
//!
//! // Write cells (same as any GpuBuffer)
//! {
//!     let cells = buffer.map_write();
//!     cells[0] = GpuCell::new('H');
//! }
//! buffer.unmap();
//! buffer.submit(); // Updates generation counter
//! ```
//!
//! # External Process Access
//!
//! ```ignore
//! use inky::render::ipc::SharedMemoryBuffer;
//!
//! // Open existing buffer by path
//! let buffer = SharedMemoryBuffer::open("/tmp/inky-12345-buffer.shm")?;
//!
//! // Read current generation
//! let gen = buffer.generation();
//!
//! // Read cells (zero-copy)
//! let cells = buffer.cells();
//! ```

use super::gpu::{GpuBuffer, GpuCell};
use memmap2::{MmapMut, MmapOptions};
use std::fs::{File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// Magic number for identifying inky shared memory buffers.
const MAGIC: u32 = 0x494E_4B59; // "INKY" in ASCII

/// Current protocol version.
const VERSION: u32 = 1;

/// Header size in bytes (must be a multiple of 8 for alignment).
const HEADER_SIZE: usize = 64;

/// Shared memory buffer header.
///
/// This is written at the start of the memory-mapped region.
/// All fields are little-endian.
///
/// # Layout (64 bytes)
///
/// | Offset | Size | Field | Description |
/// |--------|------|-------|-------------|
/// | 0 | 4 | magic | Magic number (0x494E4B59 = "INKY") |
/// | 4 | 4 | version | Protocol version |
/// | 8 | 2 | width | Buffer width in cells |
/// | 10 | 2 | height | Buffer height in cells |
/// | 12 | 4 | flags | Buffer flags |
/// | 16 | 8 | generation | Update generation counter |
/// | 24 | 8 | last_update_us | Last update timestamp (microseconds) |
/// | 32 | 4 | pid | Creator process ID |
/// | 36 | 28 | reserved | Reserved for future use |
#[repr(C)]
#[derive(Debug)]
struct SharedBufferHeader {
    magic: AtomicU32,
    version: AtomicU32,
    width: AtomicU32,  // Using u32 for atomic, actual value is u16
    height: AtomicU32, // Using u32 for atomic, actual value is u16
    generation: AtomicU64,
    last_update_us: AtomicU64,
    pid: AtomicU32,
    flags: AtomicU32,
    _reserved: [u8; 24],
}

// Compile-time size assertion
const _: () = assert!(std::mem::size_of::<SharedBufferHeader>() == HEADER_SIZE);

/// Flags for shared buffer state.
#[derive(Debug, Clone, Copy)]
pub struct SharedBufferFlags(u32);

impl SharedBufferFlags {
    /// Buffer is being written to.
    pub const WRITING: Self = Self(1 << 0);
    /// Buffer is ready for reading.
    pub const READY: Self = Self(1 << 1);
    /// Buffer has been resized (readers should re-check dimensions).
    pub const RESIZED: Self = Self(1 << 2);
}

/// Shared memory buffer for zero-copy IPC.
///
/// This buffer is backed by a memory-mapped file that can be accessed
/// by multiple processes. The data format matches [`GpuCell`] for
/// direct GPU upload compatibility.
pub struct SharedMemoryBuffer {
    /// Memory-mapped region
    mmap: MmapMut,
    /// Path to the shared memory file
    path: PathBuf,
    /// Underlying file handle (kept open for the lifetime of the mapping)
    #[allow(dead_code)]
    file: File,
    /// Cached width
    width: u16,
    /// Cached height
    height: u16,
    /// Whether we own this buffer (created it vs opened it)
    owned: bool,
}

impl SharedMemoryBuffer {
    /// Create a new shared memory buffer.
    ///
    /// This creates a memory-mapped file at the default path for the
    /// current process. The file is automatically deleted when the
    /// buffer is dropped.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be created or mapped.
    pub fn create(width: u16, height: u16) -> io::Result<Self> {
        let path = Self::default_path();
        Self::create_at(&path, width, height)
    }

    /// Create a new shared memory buffer at a specific path.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be created or mapped.
    pub fn create_at(path: &Path, width: u16, height: u16) -> io::Result<Self> {
        let size = Self::required_size(width, height);

        // Create and size the file
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)?;

        file.set_len(size as u64)?;

        // SAFETY: We just created the file with the correct size and have
        // exclusive access until we return. The mapping is aligned to page
        // boundaries by the OS.
        let mut mmap = unsafe { MmapOptions::new().map_mut(&file)? };

        // Initialize header
        Self::init_header(&mut mmap, width, height);

        // Initialize cells to blank
        let cell_data = &mut mmap[HEADER_SIZE..];
        let cells = Self::cells_from_bytes_mut(cell_data, width, height);
        for cell in cells {
            *cell = GpuCell::blank();
        }

        // Flush initial data
        mmap.flush()?;

        Ok(Self {
            mmap,
            path: path.to_path_buf(),
            file,
            width,
            height,
            owned: true,
        })
    }

    /// Open an existing shared memory buffer.
    ///
    /// This opens a memory-mapped file created by another process.
    /// The buffer must have been created with [`create`](Self::create) or
    /// [`create_at`](Self::create_at).
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The file does not exist
    /// - The file is not a valid inky shared buffer
    /// - The magic number or version is wrong
    pub fn open(path: &Path) -> io::Result<Self> {
        let file = OpenOptions::new().read(true).write(true).open(path)?;

        // SAFETY: The file exists and we're opening it read-write.
        // We validate the header before using the mapping.
        let mmap = unsafe { MmapOptions::new().map_mut(&file)? };

        // Validate header
        if mmap.len() < HEADER_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "shared buffer file too small",
            ));
        }

        let header = Self::header_from_bytes(&mmap);
        let magic = header.magic.load(Ordering::Acquire);
        if magic != MAGIC {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("invalid magic number: expected {MAGIC:#x}, got {magic:#x}"),
            ));
        }

        let version = header.version.load(Ordering::Acquire);
        if version != VERSION {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unsupported version: expected {VERSION}, got {version}"),
            ));
        }

        let width = header.width.load(Ordering::Acquire) as u16;
        let height = header.height.load(Ordering::Acquire) as u16;

        // Verify file size matches dimensions
        let expected_size = Self::required_size(width, height);
        if mmap.len() < expected_size {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "file size mismatch: expected {expected_size}, got {}",
                    mmap.len()
                ),
            ));
        }

        Ok(Self {
            mmap,
            path: path.to_path_buf(),
            file,
            width,
            height,
            owned: false,
        })
    }

    /// Get the path to the shared memory file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Get the current generation counter.
    ///
    /// This counter is incremented each time [`submit`](Self::submit) is called.
    /// External processes can poll this value to detect updates.
    pub fn generation(&self) -> u64 {
        let header = Self::header_from_bytes(&self.mmap);
        header.generation.load(Ordering::Acquire)
    }

    /// Get the last update timestamp in microseconds.
    pub fn last_update_us(&self) -> u64 {
        let header = Self::header_from_bytes(&self.mmap);
        header.last_update_us.load(Ordering::Acquire)
    }

    /// Get the creator process ID.
    pub fn pid(&self) -> u32 {
        let header = Self::header_from_bytes(&self.mmap);
        header.pid.load(Ordering::Acquire)
    }

    /// Get read-only access to cells (zero-copy).
    pub fn cells(&self) -> &[GpuCell] {
        let cell_data = &self.mmap[HEADER_SIZE..];
        Self::cells_from_bytes(cell_data, self.width, self.height)
    }

    /// Calculate the required file size for given dimensions.
    fn required_size(width: u16, height: u16) -> usize {
        HEADER_SIZE + (width as usize) * (height as usize) * std::mem::size_of::<GpuCell>()
    }

    /// Get the default path for shared memory files.
    fn default_path() -> PathBuf {
        let pid = std::process::id();

        // Try /dev/shm first (Linux), fall back to TMPDIR
        #[cfg(target_os = "linux")]
        {
            let shm_path = PathBuf::from(format!("/dev/shm/inky-{pid}-buffer"));
            if shm_path.parent().map(|p| p.exists()).unwrap_or(false) {
                return shm_path;
            }
        }

        // Use TMPDIR or /tmp
        let tmp_dir = std::env::var("TMPDIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/tmp"));

        tmp_dir.join(format!("inky-{pid}-buffer.shm"))
    }

    /// Initialize the header in a memory region.
    fn init_header(mmap: &mut MmapMut, width: u16, height: u16) {
        let header = Self::header_from_bytes_mut(mmap);
        header.magic.store(MAGIC, Ordering::Release);
        header.version.store(VERSION, Ordering::Release);
        header.width.store(width as u32, Ordering::Release);
        header.height.store(height as u32, Ordering::Release);
        header.generation.store(0, Ordering::Release);
        header.last_update_us.store(0, Ordering::Release);
        header.pid.store(std::process::id(), Ordering::Release);
        header
            .flags
            .store(SharedBufferFlags::READY.0, Ordering::Release);
    }

    /// Get a reference to the header from raw bytes.
    #[allow(clippy::cast_ptr_alignment)] // Memory mappings are page-aligned
    fn header_from_bytes(bytes: &[u8]) -> &SharedBufferHeader {
        // SAFETY: The header is at the start of the mapping and is
        // properly aligned (memory mappings are page-aligned).
        // The size was verified before this function is called.
        unsafe { &*bytes.as_ptr().cast::<SharedBufferHeader>() }
    }

    /// Get a mutable reference to the header from raw bytes.
    #[allow(clippy::cast_ptr_alignment)] // Memory mappings are page-aligned
    fn header_from_bytes_mut(bytes: &mut [u8]) -> &mut SharedBufferHeader {
        // SAFETY: Same as header_from_bytes, plus we have exclusive access.
        unsafe { &mut *bytes.as_mut_ptr().cast::<SharedBufferHeader>() }
    }

    /// Convert raw bytes to a slice of cells.
    fn cells_from_bytes(bytes: &[u8], width: u16, height: u16) -> &[GpuCell] {
        let count = (width as usize) * (height as usize);
        // SAFETY: GpuCell is repr(C, packed) with 8 bytes. The memory
        // region was sized to hold exactly this many cells. The alignment
        // is handled by placing cells after the 64-byte header.
        unsafe { std::slice::from_raw_parts(bytes.as_ptr().cast::<GpuCell>(), count) }
    }

    /// Convert raw bytes to a mutable slice of cells.
    fn cells_from_bytes_mut(bytes: &mut [u8], width: u16, height: u16) -> &mut [GpuCell] {
        let count = (width as usize) * (height as usize);
        // SAFETY: Same as cells_from_bytes, plus we have exclusive access.
        unsafe { std::slice::from_raw_parts_mut(bytes.as_mut_ptr().cast::<GpuCell>(), count) }
    }

    /// Get current timestamp in microseconds.
    fn now_us() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_micros() as u64)
            .unwrap_or(0)
    }
}

impl GpuBuffer for SharedMemoryBuffer {
    fn width(&self) -> u16 {
        self.width
    }

    fn height(&self) -> u16 {
        self.height
    }

    fn map_write(&mut self) -> &mut [GpuCell] {
        // Set WRITING flag
        let header = Self::header_from_bytes_mut(&mut self.mmap);
        let flags = header.flags.load(Ordering::Acquire);
        header
            .flags
            .store(flags | SharedBufferFlags::WRITING.0, Ordering::Release);

        let cell_data = &mut self.mmap[HEADER_SIZE..];
        Self::cells_from_bytes_mut(cell_data, self.width, self.height)
    }

    fn unmap(&mut self) {
        // Clear WRITING flag
        let header = Self::header_from_bytes_mut(&mut self.mmap);
        let flags = header.flags.load(Ordering::Acquire);
        header
            .flags
            .store(flags & !SharedBufferFlags::WRITING.0, Ordering::Release);
    }

    fn submit(&mut self) {
        let header = Self::header_from_bytes_mut(&mut self.mmap);

        // Increment generation counter
        let gen = header.generation.load(Ordering::Acquire);
        header.generation.store(gen + 1, Ordering::Release);

        // Update timestamp
        header
            .last_update_us
            .store(Self::now_us(), Ordering::Release);

        // Ensure changes are visible to other processes
        if let Err(e) = self.mmap.flush_async() {
            // Non-fatal: log but continue
            eprintln!("warning: failed to flush shared buffer: {e}");
        }
    }

    fn resize(&mut self, width: u16, height: u16) -> bool {
        if width == self.width && height == self.height {
            return true;
        }

        if !self.owned {
            // Cannot resize a buffer we don't own
            return false;
        }

        let new_size = Self::required_size(width, height);

        // Re-open and resize the file
        let file = match OpenOptions::new().read(true).write(true).open(&self.path) {
            Ok(f) => f,
            Err(_) => return false,
        };

        if file.set_len(new_size as u64).is_err() {
            return false;
        }

        // SAFETY: We've resized the file and have exclusive access.
        let mmap = match unsafe { MmapOptions::new().map_mut(&file) } {
            Ok(m) => m,
            Err(_) => return false,
        };

        // Copy old cells to new mapping
        let old_cells = self.cells().to_vec();

        // Update mapping
        self.mmap = mmap;
        self.file = file;

        // Update header
        let header = Self::header_from_bytes_mut(&mut self.mmap);
        header.width.store(width as u32, Ordering::Release);
        header.height.store(height as u32, Ordering::Release);
        header
            .flags
            .fetch_or(SharedBufferFlags::RESIZED.0, Ordering::Release);

        // Initialize new cells
        let cell_data = &mut self.mmap[HEADER_SIZE..];
        let cells = Self::cells_from_bytes_mut(cell_data, width, height);
        for cell in cells.iter_mut() {
            *cell = GpuCell::blank();
        }

        // Copy old content
        let copy_width = self.width.min(width);
        let copy_height = self.height.min(height);
        for y in 0..copy_height as usize {
            for x in 0..copy_width as usize {
                let old_idx = y * (self.width as usize) + x;
                let new_idx = y * (width as usize) + x;
                if old_idx < old_cells.len() && new_idx < cells.len() {
                    cells[new_idx] = old_cells[old_idx];
                }
            }
        }

        self.width = width;
        self.height = height;

        true
    }

    fn is_available(&self) -> bool {
        true
    }
}

impl Drop for SharedMemoryBuffer {
    fn drop(&mut self) {
        if self.owned {
            // Try to remove the file when we're done
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

/// Get the path to the shared buffer for a given process ID.
///
/// This is useful for discovering buffers created by other processes.
pub fn shared_buffer_path(pid: u32) -> PathBuf {
    #[cfg(target_os = "linux")]
    {
        let shm_path = PathBuf::from(format!("/dev/shm/inky-{pid}-buffer"));
        if shm_path.exists() {
            return shm_path;
        }
    }

    let tmp_dir = std::env::var("TMPDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp"));

    tmp_dir.join(format!("inky-{pid}-buffer.shm"))
}

/// List all shared buffers on the system.
///
/// Returns a list of (PID, path) pairs for all discoverable inky buffers.
pub fn list_shared_buffers() -> Vec<(u32, PathBuf)> {
    let mut results = Vec::new();

    // Check /dev/shm on Linux
    #[cfg(target_os = "linux")]
    if let Ok(entries) = std::fs::read_dir("/dev/shm") {
        for entry in entries.flatten() {
            if let Some(pid) = parse_buffer_filename(&entry.file_name()) {
                results.push((pid, entry.path()));
            }
        }
    }

    // Check TMPDIR
    let tmp_dir = std::env::var("TMPDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp"));

    if let Ok(entries) = std::fs::read_dir(&tmp_dir) {
        for entry in entries.flatten() {
            if let Some(pid) = parse_buffer_filename(&entry.file_name()) {
                // Avoid duplicates from /dev/shm
                if !results.iter().any(|(p, _)| *p == pid) {
                    results.push((pid, entry.path()));
                }
            }
        }
    }

    results
}

/// Parse PID from buffer filename.
fn parse_buffer_filename(name: &std::ffi::OsStr) -> Option<u32> {
    let name = name.to_str()?;
    if !name.starts_with("inky-") {
        return None;
    }

    let rest = name.strip_prefix("inky-")?;
    let pid_str = rest
        .strip_suffix("-buffer.shm")
        .or_else(|| rest.strip_suffix("-buffer"))?;

    pid_str.parse().ok()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_shared_buffer_create() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test-buffer.shm");

        let buffer = SharedMemoryBuffer::create_at(&path, 80, 24).unwrap();
        assert_eq!(buffer.width(), 80);
        assert_eq!(buffer.height(), 24);
        assert!(buffer.is_available());
        assert_eq!(buffer.generation(), 0);
        assert!(path.exists());

        drop(buffer);
        assert!(!path.exists()); // File should be cleaned up
    }

    #[test]
    fn test_shared_buffer_write_read() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test-buffer.shm");

        let mut buffer = SharedMemoryBuffer::create_at(&path, 10, 5).unwrap();

        // Write some cells
        {
            let cells = buffer.map_write();
            cells[0] = GpuCell::new('H');
            cells[1] = GpuCell::new('i');
        }
        buffer.unmap();
        buffer.submit();

        assert_eq!(buffer.generation(), 1);

        // Read cells back
        let cells = buffer.cells();
        assert_eq!(cells[0].char(), 'H');
        assert_eq!(cells[1].char(), 'i');
    }

    #[test]
    fn test_shared_buffer_open() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test-buffer.shm");

        // Create buffer
        let mut buffer1 = SharedMemoryBuffer::create_at(&path, 10, 5).unwrap();
        {
            let cells = buffer1.map_write();
            cells[0] = GpuCell::new('X');
        }
        buffer1.unmap();
        buffer1.submit();

        // Open from another "process"
        let buffer2 = SharedMemoryBuffer::open(&path).unwrap();
        assert_eq!(buffer2.width(), 10);
        assert_eq!(buffer2.height(), 5);
        assert_eq!(buffer2.generation(), 1);

        // Read cells
        let cells = buffer2.cells();
        assert_eq!(cells[0].char(), 'X');

        // Drop buffer2 first (doesn't own the file)
        drop(buffer2);
        assert!(path.exists());

        // Drop buffer1 (owns the file, will delete it)
        drop(buffer1);
        assert!(!path.exists());
    }

    #[test]
    fn test_shared_buffer_resize() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test-buffer.shm");

        let mut buffer = SharedMemoryBuffer::create_at(&path, 5, 5).unwrap();

        // Write initial content
        {
            let cells = buffer.map_write();
            cells[0] = GpuCell::new('A');
            cells[4] = GpuCell::new('B'); // Position (4, 0)
        }
        buffer.unmap();

        // Resize larger
        assert!(buffer.resize(10, 10));
        assert_eq!(buffer.width(), 10);
        assert_eq!(buffer.height(), 10);

        // Content should be preserved
        let cells = buffer.cells();
        assert_eq!(cells[0].char(), 'A');
        assert_eq!(cells[4].char(), 'B');
    }

    #[test]
    fn test_shared_buffer_concurrent_access() {
        let dir = tempfile::tempdir().unwrap();
        let path = Arc::new(dir.path().join("test-buffer.shm"));

        // Create buffer
        let path_clone = Arc::clone(&path);
        let buffer = SharedMemoryBuffer::create_at(&path_clone, 10, 10).unwrap();

        // Writer thread
        let path_write = Arc::clone(&path);
        let writer = thread::spawn(move || {
            let mut buf = SharedMemoryBuffer::open(&path_write).unwrap();
            for i in 0..100 {
                {
                    let cells = buf.map_write();
                    cells[0] = GpuCell::new(char::from_u32(65 + (i % 26)).unwrap_or('?'));
                }
                buf.unmap();
                buf.submit();
                // Small delay to allow reader to catch up
                thread::sleep(std::time::Duration::from_micros(100));
            }
        });

        // Reader thread - read updates with a timeout to avoid hanging
        let path_read = Arc::clone(&path);
        let reader = thread::spawn(move || {
            let buf = SharedMemoryBuffer::open(&path_read).unwrap();
            let mut last_gen = 0;
            let mut reads = 0;
            let start = std::time::Instant::now();
            let timeout = std::time::Duration::from_secs(5);

            while reads < 50 && start.elapsed() < timeout {
                let gen = buf.generation();
                if gen > last_gen {
                    let _ = buf.cells()[0].char();
                    last_gen = gen;
                    reads += 1;
                }
                thread::yield_now();
            }
            // Return number of reads (for verification)
            reads
        });

        // Let them run for a bit
        writer.join().unwrap();
        let reads = reader.join().unwrap();

        // Verify we read at least some updates (writer does 100, reader should see multiple)
        assert!(reads > 0, "Reader should have observed at least one update");

        // Verify we can still read from the main buffer handle
        let _ = buffer.generation();
    }

    #[test]
    fn test_header_size() {
        assert_eq!(std::mem::size_of::<SharedBufferHeader>(), HEADER_SIZE);
    }

    #[test]
    fn test_parse_buffer_filename() {
        assert_eq!(
            parse_buffer_filename(std::ffi::OsStr::new("inky-12345-buffer.shm")),
            Some(12345)
        );
        assert_eq!(
            parse_buffer_filename(std::ffi::OsStr::new("inky-1-buffer")),
            Some(1)
        );
        assert_eq!(
            parse_buffer_filename(std::ffi::OsStr::new("other-file.txt")),
            None
        );
    }
}
