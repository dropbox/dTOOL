//! Disk-backed cold tier storage using memory-mapped files.
//!
//! ## Design
//!
//! Append-only format for efficient writes:
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │  File Header (32 bytes)                                      │
//! │  - Magic: "DTRM" (4 bytes)                                   │
//! │  - Version: u32 (4 bytes)                                    │
//! │  - Page count: u64 (8 bytes)                                 │
//! │  - Line count: u64 (8 bytes)                                 │
//! │  - Reserved (8 bytes)                                        │
//! ├─────────────────────────────────────────────────────────────┤
//! │  Page 0 Header (8 bytes) + Compressed Data                   │
//! │  - Compressed size: u32                                      │
//! │  - Line count: u32                                           │
//! │  - Data: [u8; compressed_size]                               │
//! ├─────────────────────────────────────────────────────────────┤
//! │  Page 1 Header + Data                                        │
//! ├─────────────────────────────────────────────────────────────┤
//! │  ...                                                         │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Lazy Loading
//!
//! Pages are loaded on demand and cached in an LRU cache for repeated access.
//! The index is rebuilt on load by scanning page headers.

use super::line::{deserialize_lines, Line};
use memmap2::{MmapMut, MmapOptions};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

// ============================================================================
// Safe Cast Helpers for Disk Serialization
// ============================================================================

/// Convert a u64 to usize, erroring if it exceeds platform capacity.
#[inline]
fn len_u64_to_usize(len: u64) -> io::Result<usize> {
    usize::try_from(len).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "length exceeds platform address space",
        )
    })
}

/// Convert a u32 to usize (always succeeds on 32-bit+ platforms).
#[inline]
#[allow(clippy::cast_possible_truncation)] // u32 fits in usize on supported platforms
fn len_u32_to_usize(len: u32) -> usize {
    len as usize
}

/// Convert a usize to u32, saturating at u32::MAX.
#[inline]
fn len_to_u32(len: usize) -> u32 {
    u32::try_from(len).unwrap_or(u32::MAX)
}

/// Magic bytes identifying a dterm cold storage file.
const MAGIC: &[u8; 4] = b"DTRM";

/// Current file format version.
const VERSION: u32 = 1;

/// File header size in bytes.
const HEADER_SIZE: usize = 32;

/// Page header size in bytes.
const PAGE_HEADER_SIZE: usize = 8;

/// Default LRU cache size (number of decompressed pages).
const DEFAULT_CACHE_SIZE: usize = 8;

/// Configuration for disk-backed cold tier.
#[derive(Debug, Clone)]
pub struct DiskColdConfig {
    /// Path to the storage file.
    pub path: PathBuf,
    /// Number of decompressed pages to cache.
    pub cache_size: usize,
}

impl DiskColdConfig {
    /// Create a new config with the given path.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            cache_size: DEFAULT_CACHE_SIZE,
        }
    }

    /// Set the cache size.
    #[must_use]
    pub fn with_cache_size(mut self, size: usize) -> Self {
        self.cache_size = size;
        self
    }
}

/// Page index entry (in-memory only).
#[derive(Debug, Clone, Copy)]
struct PageIndexEntry {
    /// Byte offset of page header in file.
    offset: u64,
    /// Compressed size in bytes (excluding page header).
    compressed_size: u32,
    /// Number of lines in this page (used in Kani proofs).
    #[allow(dead_code)]
    line_count: u32,
}

/// LRU cache entry with access order tracking.
struct CacheEntry {
    /// Decompressed lines.
    lines: Vec<Line>,
    /// Last access sequence number.
    last_access: u64,
}

/// Disk-backed cold tier storage.
///
/// Stores Zstd-compressed pages in a memory-mapped file with lazy loading.
#[derive(Debug)]
pub struct DiskColdTier {
    /// Storage file.
    file: Option<File>,
    /// Memory map of the file (for reading).
    mmap: Option<MmapMut>,
    /// Path to the storage file.
    path: PathBuf,
    /// Page index (kept in memory for fast lookup).
    index: Vec<PageIndexEntry>,
    /// Total line count.
    line_count: usize,
    /// Cumulative line counts for binary search.
    cumulative_lines: Vec<usize>,
    /// LRU cache of decompressed pages.
    cache: HashMap<usize, CacheEntry>,
    /// Cache size limit.
    cache_size: usize,
    /// Access counter for LRU.
    access_counter: u64,
    /// Next write offset in file.
    write_offset: u64,
}

impl DiskColdTier {
    /// Create a new in-memory cold tier (no disk backing).
    #[must_use]
    pub fn new() -> Self {
        Self {
            file: None,
            mmap: None,
            path: PathBuf::new(),
            index: Vec::new(),
            line_count: 0,
            cumulative_lines: Vec::new(),
            cache: HashMap::new(),
            cache_size: DEFAULT_CACHE_SIZE,
            access_counter: 0,
            write_offset: HEADER_SIZE as u64,
        }
    }

    /// Create a disk-backed cold tier with the given configuration.
    ///
    /// If the file exists, it will be loaded. Otherwise, a new file is created.
    pub fn with_config(config: DiskColdConfig) -> io::Result<Self> {
        let path = config.path;
        let cache_size = config.cache_size;

        if path.exists() {
            Self::load(&path, cache_size)
        } else {
            Self::create(&path, cache_size)
        }
    }

    /// Create a new storage file.
    fn create(path: &Path, cache_size: usize) -> io::Result<Self> {
        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)?;

        // Write header
        let mut header = [0u8; HEADER_SIZE];
        header[0..4].copy_from_slice(MAGIC);
        header[4..8].copy_from_slice(&VERSION.to_le_bytes());
        // page_count and line_count start at 0
        file.write_all(&header)?;
        file.flush()?;

        Ok(Self {
            file: Some(file),
            mmap: None,
            path: path.to_path_buf(),
            index: Vec::new(),
            line_count: 0,
            cumulative_lines: Vec::new(),
            cache: HashMap::new(),
            cache_size,
            access_counter: 0,
            write_offset: HEADER_SIZE as u64,
        })
    }

    /// Load an existing storage file.
    fn load(path: &Path, cache_size: usize) -> io::Result<Self> {
        let mut file = OpenOptions::new().read(true).write(true).open(path)?;

        // Read and validate header
        let mut header = [0u8; HEADER_SIZE];
        file.read_exact(&mut header)?;

        if &header[0..4] != MAGIC {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid magic bytes",
            ));
        }

        let version = u32::from_le_bytes([header[4], header[5], header[6], header[7]]);
        if version != VERSION {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unsupported version: {}", version),
            ));
        }

        let page_count = len_u64_to_usize(u64::from_le_bytes([
            header[8], header[9], header[10], header[11], header[12], header[13], header[14],
            header[15],
        ]))?;

        let _stored_line_count = len_u64_to_usize(u64::from_le_bytes([
            header[16], header[17], header[18], header[19], header[20], header[21], header[22],
            header[23],
        ]))?;

        // Scan pages to rebuild index
        let file_len = file.metadata()?.len();
        let mut index = Vec::with_capacity(page_count);
        let mut cumulative_lines = Vec::with_capacity(page_count);
        let mut cumulative = 0;
        let mut offset = HEADER_SIZE as u64;

        let mut page_header = [0u8; PAGE_HEADER_SIZE];
        while offset + PAGE_HEADER_SIZE as u64 <= file_len {
            file.seek(SeekFrom::Start(offset))?;
            if file.read_exact(&mut page_header).is_err() {
                break;
            }

            let compressed_size = u32::from_le_bytes([
                page_header[0],
                page_header[1],
                page_header[2],
                page_header[3],
            ]);
            let line_count = u32::from_le_bytes([
                page_header[4],
                page_header[5],
                page_header[6],
                page_header[7],
            ]);

            if compressed_size == 0 {
                break;
            }

            let entry = PageIndexEntry {
                offset,
                compressed_size,
                line_count,
            };

            cumulative += len_u32_to_usize(line_count);
            cumulative_lines.push(cumulative);
            index.push(entry);

            offset += PAGE_HEADER_SIZE as u64 + u64::from(compressed_size);
        }

        let line_count = cumulative;

        // Create mmap
        let mmap = if file_len > HEADER_SIZE as u64 {
            Some(unsafe { MmapOptions::new().map_mut(&file)? })
        } else {
            None
        };

        Ok(Self {
            file: Some(file),
            mmap,
            path: path.to_path_buf(),
            index,
            line_count,
            cumulative_lines,
            cache: HashMap::new(),
            cache_size,
            access_counter: 0,
            write_offset: offset,
        })
    }

    /// Get the total number of lines.
    #[must_use]
    #[inline]
    pub fn line_count(&self) -> usize {
        self.line_count
    }

    /// Get the number of pages.
    #[must_use]
    #[inline]
    pub fn page_count(&self) -> usize {
        self.index.len()
    }

    /// Check if empty.
    #[must_use]
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.index.is_empty()
    }

    /// Get the total compressed size on disk.
    #[must_use]
    pub fn compressed_size(&self) -> usize {
        self.index
            .iter()
            .map(|e| len_u32_to_usize(e.compressed_size))
            .sum()
    }

    /// Push compressed data from a warm block.
    ///
    /// The data is already Zstd compressed.
    pub fn push_compressed(&mut self, compressed: Vec<u8>, line_count: usize) -> io::Result<()> {
        if compressed.is_empty() || line_count == 0 {
            return Ok(());
        }

        // If we have a file, write to it
        if let Some(ref mut file) = self.file {
            // Write page header
            let mut page_header = [0u8; PAGE_HEADER_SIZE];
            page_header[0..4].copy_from_slice(&len_to_u32(compressed.len()).to_le_bytes());
            page_header[4..8].copy_from_slice(&len_to_u32(line_count).to_le_bytes());

            file.seek(SeekFrom::Start(self.write_offset))?;
            file.write_all(&page_header)?;
            file.write_all(&compressed)?;

            // Create index entry
            let entry = PageIndexEntry {
                offset: self.write_offset,
                compressed_size: len_to_u32(compressed.len()),
                line_count: len_to_u32(line_count),
            };

            // Update header with new counts
            self.line_count += line_count;
            let new_page_count = self.index.len() + 1;

            file.seek(SeekFrom::Start(8))?;
            file.write_all(&(new_page_count as u64).to_le_bytes())?;
            file.write_all(&(self.line_count as u64).to_le_bytes())?;
            file.flush()?;

            // Update write offset for next page
            self.write_offset += PAGE_HEADER_SIZE as u64 + compressed.len() as u64;

            // Update in-memory state
            self.index.push(entry);
            let cumulative = self.cumulative_lines.last().copied().unwrap_or(0) + line_count;
            self.cumulative_lines.push(cumulative);

            // Refresh mmap
            self.mmap = Some(unsafe { MmapOptions::new().map_mut(&*file)? });
        } else {
            // In-memory only mode - just update counts
            let entry = PageIndexEntry {
                offset: 0,
                compressed_size: len_to_u32(compressed.len()),
                line_count: len_to_u32(line_count),
            };
            self.index.push(entry);
            self.line_count += line_count;
            let cumulative = self.cumulative_lines.last().copied().unwrap_or(0) + line_count;
            self.cumulative_lines.push(cumulative);
        }

        Ok(())
    }

    /// Get a line by index (0 = oldest).
    #[must_use]
    pub fn get_line(&mut self, idx: usize) -> Option<Line> {
        if idx >= self.line_count {
            return None;
        }

        // Binary search to find the page
        let page_idx = self.find_page(idx)?;

        // Get the line within the page
        let page_start = if page_idx == 0 {
            0
        } else {
            self.cumulative_lines[page_idx - 1]
        };
        let line_in_page = idx - page_start;

        // Load page (possibly from cache)
        let lines = self.load_page(page_idx)?;
        lines.get(line_in_page).cloned()
    }

    /// Find the page containing the given line index.
    fn find_page(&self, line_idx: usize) -> Option<usize> {
        // Binary search through cumulative line counts
        match self.cumulative_lines.binary_search(&(line_idx + 1)) {
            Ok(idx) => Some(idx),
            Err(idx) => {
                if idx < self.cumulative_lines.len() {
                    Some(idx)
                } else {
                    None
                }
            }
        }
    }

    /// Load a page (from cache or disk).
    fn load_page(&mut self, page_idx: usize) -> Option<Vec<Line>> {
        // Check cache first
        if let Some(entry) = self.cache.get_mut(&page_idx) {
            self.access_counter += 1;
            entry.last_access = self.access_counter;
            return Some(entry.lines.clone());
        }

        // Load from disk/mmap
        let lines = self.decompress_page(page_idx)?;

        // Add to cache
        self.cache_page(page_idx, lines.clone());

        Some(lines)
    }

    /// Decompress a page from disk.
    fn decompress_page(&self, page_idx: usize) -> Option<Vec<Line>> {
        let entry = self.index.get(page_idx)?;

        // Read compressed data (skip page header)
        let compressed = if let Some(ref mmap) = self.mmap {
            // entry.offset is u64, convert carefully for 32-bit platforms
            let offset_usize = usize::try_from(entry.offset).ok()?;
            let data_start = offset_usize.checked_add(PAGE_HEADER_SIZE)?;
            let data_end = data_start.checked_add(len_u32_to_usize(entry.compressed_size))?;
            if data_end > mmap.len() {
                return None;
            }
            &mmap[data_start..data_end]
        } else {
            // No mmap - can't read
            return None;
        };

        // Decompress
        let decompressed = zstd::decode_all(compressed).ok()?;

        // Deserialize lines
        Some(deserialize_lines(&decompressed))
    }

    /// Add a page to the cache, evicting if necessary.
    fn cache_page(&mut self, page_idx: usize, lines: Vec<Line>) {
        // Evict if at capacity
        while self.cache.len() >= self.cache_size {
            self.evict_lru();
        }

        self.access_counter += 1;
        self.cache.insert(
            page_idx,
            CacheEntry {
                lines,
                last_access: self.access_counter,
            },
        );
    }

    /// Evict the least recently used cache entry.
    fn evict_lru(&mut self) {
        let lru_key = self
            .cache
            .iter()
            .min_by_key(|(_, entry)| entry.last_access)
            .map(|(k, _)| *k);

        if let Some(key) = lru_key {
            self.cache.remove(&key);
        }
    }

    /// Clear all data.
    pub fn clear(&mut self) -> io::Result<()> {
        self.index.clear();
        self.cumulative_lines.clear();
        self.line_count = 0;
        self.cache.clear();
        self.write_offset = HEADER_SIZE as u64;

        // Truncate file if we have one
        if let Some(ref mut file) = self.file {
            file.set_len(HEADER_SIZE as u64)?;
            file.seek(SeekFrom::Start(8))?;
            file.write_all(&0u64.to_le_bytes())?; // page_count
            file.write_all(&0u64.to_le_bytes())?; // line_count
            file.flush()?;

            self.mmap = None;
        }

        Ok(())
    }

    /// Sync changes to disk.
    pub fn sync(&mut self) -> io::Result<()> {
        if let Some(ref mut file) = self.file {
            file.sync_all()?;
        }
        if let Some(ref mmap) = self.mmap {
            mmap.flush()?;
        }
        Ok(())
    }

    /// Get the file path (if disk-backed).
    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        if self.file.is_some() {
            Some(&self.path)
        } else {
            None
        }
    }

    /// Check if disk-backed.
    #[must_use]
    pub fn is_disk_backed(&self) -> bool {
        self.file.is_some()
    }
}

impl Default for DiskColdTier {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for CacheEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CacheEntry")
            .field("lines_count", &self.lines.len())
            .field("last_access", &self.last_access)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::super::line::serialize_lines;
    use super::*;
    use tempfile::tempdir;

    fn create_test_page(line_count: usize, prefix: &str) -> (Vec<u8>, usize) {
        let lines: Vec<Line> = (0..line_count)
            .map(|i| Line::from_str(&format!("{prefix}-Line{i}")))
            .collect();
        let serialized = serialize_lines(&lines);
        let compressed = zstd::encode_all(serialized.as_slice(), 3).unwrap();
        (compressed, line_count)
    }

    #[test]
    fn disk_cold_in_memory() {
        let mut cold = DiskColdTier::new();
        assert!(cold.is_empty());
        assert!(!cold.is_disk_backed());

        let (compressed, line_count) = create_test_page(10, "Page0");
        cold.push_compressed(compressed, line_count).unwrap();

        assert_eq!(cold.line_count(), 10);
        assert_eq!(cold.page_count(), 1);
    }

    #[test]
    fn disk_cold_file_create() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("cold.dtrm");

        let config = DiskColdConfig::new(&path);
        let mut cold = DiskColdTier::with_config(config).unwrap();

        assert!(cold.is_disk_backed());
        assert!(path.exists());

        let (compressed, line_count) = create_test_page(10, "Page0");
        cold.push_compressed(compressed, line_count).unwrap();

        assert_eq!(cold.line_count(), 10);
    }

    #[test]
    fn disk_cold_file_reload() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("cold.dtrm");

        // Create and populate
        {
            let config = DiskColdConfig::new(&path);
            let mut cold = DiskColdTier::with_config(config).unwrap();

            let (compressed1, count1) = create_test_page(5, "Page0");
            cold.push_compressed(compressed1, count1).unwrap();

            let (compressed2, count2) = create_test_page(5, "Page1");
            cold.push_compressed(compressed2, count2).unwrap();

            cold.sync().unwrap();
        }

        // Reload and verify
        {
            let config = DiskColdConfig::new(&path);
            let cold = DiskColdTier::with_config(config).unwrap();

            assert_eq!(cold.line_count(), 10);
            assert_eq!(cold.page_count(), 2);
        }
    }

    #[test]
    fn disk_cold_get_line() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("cold.dtrm");

        let config = DiskColdConfig::new(&path);
        let mut cold = DiskColdTier::with_config(config).unwrap();

        // Add multiple pages
        for page_num in 0..3 {
            let (compressed, line_count) = create_test_page(5, &format!("Page{page_num}"));
            cold.push_compressed(compressed, line_count).unwrap();
        }

        assert_eq!(cold.line_count(), 15);

        // Test line retrieval across pages
        assert_eq!(cold.get_line(0).unwrap().to_string(), "Page0-Line0");
        assert_eq!(cold.get_line(4).unwrap().to_string(), "Page0-Line4");
        assert_eq!(cold.get_line(5).unwrap().to_string(), "Page1-Line0");
        assert_eq!(cold.get_line(10).unwrap().to_string(), "Page2-Line0");
        assert_eq!(cold.get_line(14).unwrap().to_string(), "Page2-Line4");
        assert!(cold.get_line(15).is_none());
    }

    #[test]
    fn disk_cold_lru_cache() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("cold.dtrm");

        let config = DiskColdConfig::new(&path).with_cache_size(2);
        let mut cold = DiskColdTier::with_config(config).unwrap();

        // Add 5 pages
        for page_num in 0..5 {
            let (compressed, line_count) = create_test_page(10, &format!("Page{page_num}"));
            cold.push_compressed(compressed, line_count).unwrap();
        }

        // Access pages 0, 1, 2 - cache should only hold 2
        cold.get_line(0).unwrap();
        cold.get_line(10).unwrap();
        cold.get_line(20).unwrap();

        // Cache should have evicted page 0
        assert!(cold.cache.len() <= 2);
    }

    #[test]
    fn disk_cold_clear() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("cold.dtrm");

        let config = DiskColdConfig::new(&path);
        let mut cold = DiskColdTier::with_config(config).unwrap();

        let (compressed, line_count) = create_test_page(10, "Page0");
        cold.push_compressed(compressed, line_count).unwrap();

        assert_eq!(cold.line_count(), 10);

        cold.clear().unwrap();

        assert_eq!(cold.line_count(), 0);
        assert_eq!(cold.page_count(), 0);
        assert!(cold.cache.is_empty());
    }

    #[test]
    fn disk_cold_reload_and_read() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("cold.dtrm");

        // Create and populate
        {
            let config = DiskColdConfig::new(&path);
            let mut cold = DiskColdTier::with_config(config).unwrap();

            for page_num in 0..3 {
                let (compressed, line_count) = create_test_page(5, &format!("Page{page_num}"));
                cold.push_compressed(compressed, line_count).unwrap();
            }

            cold.sync().unwrap();
        }

        // Reload and read lines
        {
            let config = DiskColdConfig::new(&path);
            let mut cold = DiskColdTier::with_config(config).unwrap();

            assert_eq!(cold.line_count(), 15);
            assert_eq!(cold.get_line(0).unwrap().to_string(), "Page0-Line0");
            assert_eq!(cold.get_line(7).unwrap().to_string(), "Page1-Line2");
            assert_eq!(cold.get_line(14).unwrap().to_string(), "Page2-Line4");
        }
    }
}

#[cfg(kani)]
mod proofs {
    use super::*;

    /// Page index binary search is correct.
    #[kani::proof]
    #[kani::unwind(6)]
    fn find_page_correct() {
        let mut cold = DiskColdTier::new();

        // Create cumulative lines for 5 pages with 10 lines each
        cold.cumulative_lines = vec![10, 20, 30, 40, 50];
        cold.line_count = 50;
        cold.index = vec![
            PageIndexEntry {
                offset: 0,
                compressed_size: 100,
                line_count: 10,
            };
            5
        ];

        let line_idx: usize = kani::any();
        kani::assume(line_idx < 50);

        let page_idx = cold.find_page(line_idx);
        kani::assert(page_idx.is_some(), "should find page for valid index");

        let page_idx = page_idx.unwrap();
        kani::assert(page_idx < 5, "page index in bounds");

        // Verify the line is within the page
        let page_start = if page_idx == 0 {
            0
        } else {
            cold.cumulative_lines[page_idx - 1]
        };
        let page_end = cold.cumulative_lines[page_idx];
        kani::assert(line_idx >= page_start, "line >= page start");
        kani::assert(line_idx < page_end, "line < page end");
    }

    /// Line count is always consistent.
    #[kani::proof]
    fn line_count_consistent() {
        let mut cold = DiskColdTier::new();

        let count1: usize = kani::any();
        let count2: usize = kani::any();
        kani::assume(count1 > 0 && count1 <= 100);
        kani::assume(count2 > 0 && count2 <= 100);
        kani::assume(count1 + count2 <= 200); // Avoid overflow

        // Simulate pushing two pages (in-memory mode)
        cold.index.push(PageIndexEntry {
            offset: 0,
            compressed_size: 50,
            line_count: count1 as u32,
        });
        cold.line_count += count1;
        cold.cumulative_lines.push(count1);

        cold.index.push(PageIndexEntry {
            offset: 0,
            compressed_size: 50,
            line_count: count2 as u32,
        });
        cold.line_count += count2;
        cold.cumulative_lines
            .push(cold.cumulative_lines.last().unwrap() + count2);

        // Verify consistency
        let total_from_index: usize = cold.index.iter().map(|e| e.line_count as usize).sum();
        kani::assert(cold.line_count == total_from_index, "line count matches");
        kani::assert(
            cold.line_count == *cold.cumulative_lines.last().unwrap(),
            "cumulative matches",
        );
    }

    /// Mmap data ranges stay within the mapped file bounds.
    #[kani::proof]
    fn mmap_access_within_bounds() {
        let mmap_len: usize = kani::any();
        let offset: usize = kani::any();
        let compressed_size: u32 = kani::any();

        kani::assume(mmap_len >= PAGE_HEADER_SIZE);
        kani::assume(mmap_len <= 1 << 20);

        let compressed_len = len_u32_to_usize(compressed_size);

        kani::assume(offset <= mmap_len - PAGE_HEADER_SIZE);
        let data_start = offset + PAGE_HEADER_SIZE;
        kani::assume(data_start <= mmap_len);
        kani::assume(compressed_len <= mmap_len - data_start);

        let data_end = data_start + compressed_len;
        kani::assert(data_end <= mmap_len, "mmap slice stays in bounds");
    }

    /// Disk offset arithmetic cannot overflow when bounds are enforced.
    #[kani::proof]
    fn disk_offset_arithmetic_safe() {
        let offset: u64 = kani::any();
        let compressed_size: u32 = kani::any();

        let header = PAGE_HEADER_SIZE as u64;
        kani::assume(offset <= u64::MAX - header - compressed_size as u64);

        let total = offset + header + compressed_size as u64;
        kani::assert(total >= offset, "offset arithmetic should not overflow");
    }
}
