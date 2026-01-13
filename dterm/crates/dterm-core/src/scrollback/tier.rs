//! Tiered storage implementations.
//!
//! - Hot: Uncompressed lines in VecDeque
//! - Warm: LZ4 compressed blocks in RAM
//! - Cold: Zstd compressed blocks (in-memory for now, disk later)

use super::line::{deserialize_lines, serialize_lines, Line};
use std::collections::VecDeque;

// ============================================================================
// Hot Tier - Uncompressed, instant access
// ============================================================================

/// Hot tier: uncompressed lines in RAM.
///
/// Uses VecDeque for efficient front/back operations.
#[derive(Debug)]
pub struct HotTier {
    /// Lines stored uncompressed.
    lines: VecDeque<Line>,
}

impl HotTier {
    /// Create a new hot tier.
    #[must_use]
    pub fn new() -> Self {
        Self {
            lines: VecDeque::new(),
        }
    }

    /// Get the number of lines.
    #[must_use]
    #[inline]
    pub fn len(&self) -> usize {
        self.lines.len()
    }

    /// Check if empty.
    #[must_use]
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// Push a line to the back.
    #[inline]
    pub fn push(&mut self, line: Line) {
        self.lines.push_back(line);
    }

    /// Get a line by index (0 = oldest).
    #[must_use]
    pub fn get(&self, idx: usize) -> Option<Line> {
        self.lines.get(idx).cloned()
    }

    /// Take n lines from the front.
    pub fn take_front(&mut self, n: usize) -> Vec<Line> {
        let n = n.min(self.lines.len());
        let mut result = Vec::with_capacity(n);
        for _ in 0..n {
            if let Some(line) = self.lines.pop_front() {
                result.push(line);
            }
        }
        result
    }

    /// Truncate to keep only the last n lines.
    pub fn truncate_front(&mut self, n: usize) {
        while self.lines.len() > n {
            self.lines.pop_front();
        }
    }

    /// Clear all lines.
    pub fn clear(&mut self) {
        self.lines.clear();
    }

    /// Calculate memory used.
    #[must_use]
    pub fn memory_used(&self) -> usize {
        let base = std::mem::size_of::<Self>();
        let lines_mem: usize = self.lines.iter().map(Line::memory_used).sum();
        base + lines_mem
    }
}

impl Default for HotTier {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Warm Tier - LZ4 compressed blocks
// ============================================================================

/// A compressed block of lines (LZ4).
#[derive(Debug, Clone)]
pub struct WarmBlock {
    /// LZ4 compressed data.
    compressed: Vec<u8>,
    /// Number of lines in block (for counting without decompression).
    line_count: usize,
    /// Uncompressed size (for memory estimation).
    uncompressed_size: usize,
}

impl WarmBlock {
    /// Create a warm block from lines.
    #[must_use]
    pub fn from_lines(lines: Vec<Line>) -> Self {
        let line_count = lines.len();
        let serialized = serialize_lines(&lines);
        let uncompressed_size = serialized.len();
        let compressed = lz4_flex::compress_prepend_size(&serialized);

        Self {
            compressed,
            line_count,
            uncompressed_size,
        }
    }

    /// Get the number of lines in this block.
    #[must_use]
    #[inline]
    pub fn line_count(&self) -> usize {
        self.line_count
    }

    /// Get the compressed size in bytes.
    #[must_use]
    #[inline]
    pub fn compressed_size(&self) -> usize {
        self.compressed.len()
    }

    /// Get the uncompressed size in bytes.
    #[must_use]
    #[inline]
    pub fn uncompressed_size(&self) -> usize {
        self.uncompressed_size
    }

    /// Decompress and get all lines.
    #[must_use]
    pub fn decompress(&self) -> Vec<Line> {
        match lz4_flex::decompress_size_prepended(&self.compressed) {
            Ok(decompressed) => deserialize_lines(&decompressed),
            Err(_) => Vec::new(),
        }
    }

    /// Get a specific line by index within this block.
    #[must_use]
    pub fn get_line(&self, idx: usize) -> Option<Line> {
        if idx >= self.line_count {
            return None;
        }
        let lines = self.decompress();
        lines.into_iter().nth(idx)
    }

    /// Get compressed bytes for further compression (cold tier).
    #[must_use]
    pub fn compressed_bytes(&self) -> &[u8] {
        &self.compressed
    }

    /// Create from already-compressed data.
    #[must_use]
    pub fn from_compressed(
        compressed: Vec<u8>,
        line_count: usize,
        uncompressed_size: usize,
    ) -> Self {
        Self {
            compressed,
            line_count,
            uncompressed_size,
        }
    }

    /// Convert to Zstd-compressed data for cold tier storage.
    ///
    /// Returns the compressed data and line count.
    #[must_use]
    pub fn to_zstd_compressed(&self) -> Option<(Vec<u8>, usize)> {
        // Decompress LZ4
        let decompressed = lz4_flex::decompress_size_prepended(&self.compressed).ok()?;
        // Re-compress with Zstd for better ratio
        let zstd_compressed = zstd::encode_all(decompressed.as_slice(), 3).ok()?;
        Some((zstd_compressed, self.line_count))
    }
}

/// Warm tier: LZ4 compressed blocks in RAM.
#[derive(Debug)]
pub struct WarmTier {
    /// Compressed blocks.
    blocks: VecDeque<WarmBlock>,
    /// Total line count across all blocks.
    line_count: usize,
}

impl WarmTier {
    /// Create a new warm tier.
    #[must_use]
    pub fn new() -> Self {
        Self {
            blocks: VecDeque::new(),
            line_count: 0,
        }
    }

    /// Get the total number of lines.
    #[must_use]
    #[inline]
    pub fn line_count(&self) -> usize {
        self.line_count
    }

    /// Get the number of blocks.
    #[must_use]
    #[inline]
    pub fn block_count(&self) -> usize {
        self.blocks.len()
    }

    /// Check if empty.
    #[must_use]
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }

    /// Push a block of lines (compresses them).
    pub fn push_block(&mut self, lines: Vec<Line>) {
        if lines.is_empty() {
            return;
        }
        let block = WarmBlock::from_lines(lines);
        self.line_count += block.line_count();
        self.blocks.push_back(block);
    }

    /// Pop the oldest block.
    pub fn pop_front(&mut self) -> Option<WarmBlock> {
        let block = self.blocks.pop_front()?;
        self.line_count -= block.line_count();
        Some(block)
    }

    /// Get a line by index (0 = oldest across all blocks).
    #[must_use]
    pub fn get_line(&self, idx: usize) -> Option<Line> {
        if idx >= self.line_count {
            return None;
        }

        let mut offset = 0;
        for block in &self.blocks {
            let block_lines = block.line_count();
            if idx < offset + block_lines {
                return block.get_line(idx - offset);
            }
            offset += block_lines;
        }
        None
    }

    /// Clear all blocks.
    pub fn clear(&mut self) {
        self.blocks.clear();
        self.line_count = 0;
    }

    /// Calculate memory used.
    #[must_use]
    pub fn memory_used(&self) -> usize {
        let base = std::mem::size_of::<Self>();
        let blocks_mem: usize = self.blocks.iter().map(|b| b.compressed_size()).sum();
        base + blocks_mem
    }
}

impl Default for WarmTier {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Cold Tier - Zstd compressed (disk-backed in future)
// ============================================================================

/// A cold page (Zstd compressed).
///
/// For now, stored in memory. Future: memory-mapped file.
#[derive(Debug, Clone)]
struct ColdPage {
    /// Zstd compressed data (from LZ4 warm block).
    compressed: Vec<u8>,
    /// Number of lines in page.
    line_count: usize,
}

impl ColdPage {
    /// Create a cold page from a warm block.
    #[must_use]
    fn from_warm_block(block: &WarmBlock) -> Self {
        // Decompress LZ4, then compress with Zstd for better ratio
        let decompressed = match lz4_flex::decompress_size_prepended(block.compressed_bytes()) {
            Ok(d) => d,
            Err(_) => {
                return Self {
                    compressed: Vec::new(),
                    line_count: 0,
                }
            }
        };

        let compressed = zstd::encode_all(decompressed.as_slice(), 3).unwrap_or_default();

        Self {
            compressed,
            line_count: block.line_count(),
        }
    }

    /// Decompress and get all lines.
    #[must_use]
    fn decompress(&self) -> Vec<Line> {
        let decompressed = match zstd::decode_all(self.compressed.as_slice()) {
            Ok(d) => d,
            Err(_) => return Vec::new(),
        };
        deserialize_lines(&decompressed)
    }

    /// Get a specific line.
    #[must_use]
    fn get_line(&self, idx: usize) -> Option<Line> {
        if idx >= self.line_count {
            return None;
        }
        let lines = self.decompress();
        lines.into_iter().nth(idx)
    }
}

/// Cold tier: Zstd compressed pages.
///
/// Currently in-memory, but designed for disk backing.
#[derive(Debug)]
pub struct ColdTier {
    /// Compressed pages.
    pages: Vec<ColdPage>,
    /// Total line count.
    line_count: usize,
}

impl ColdTier {
    /// Create a new cold tier.
    #[must_use]
    pub fn new() -> Self {
        Self {
            pages: Vec::new(),
            line_count: 0,
        }
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
        self.pages.len()
    }

    /// Check if empty.
    #[must_use]
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.pages.is_empty()
    }

    /// Push a warm block (re-compresses with Zstd).
    pub fn push_block(&mut self, block: WarmBlock) {
        let page = ColdPage::from_warm_block(&block);
        self.line_count += page.line_count;
        self.pages.push(page);
    }

    /// Get a line by index (0 = oldest).
    #[must_use]
    pub fn get_line(&self, idx: usize) -> Option<Line> {
        if idx >= self.line_count {
            return None;
        }

        let mut offset = 0;
        for page in &self.pages {
            if idx < offset + page.line_count {
                return page.get_line(idx - offset);
            }
            offset += page.line_count;
        }
        None
    }

    /// Clear all pages.
    pub fn clear(&mut self) {
        self.pages.clear();
        self.line_count = 0;
    }

    /// Get total compressed size (for stats).
    #[must_use]
    pub fn compressed_size(&self) -> usize {
        self.pages.iter().map(|p| p.compressed.len()).sum()
    }
}

impl Default for ColdTier {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Hot tier tests
    #[test]
    fn hot_tier_push_get() {
        let mut hot = HotTier::new();
        hot.push(Line::from_str("Line 0"));
        hot.push(Line::from_str("Line 1"));
        hot.push(Line::from_str("Line 2"));

        assert_eq!(hot.len(), 3);
        assert_eq!(hot.get(0).unwrap().to_string(), "Line 0");
        assert_eq!(hot.get(1).unwrap().to_string(), "Line 1");
        assert_eq!(hot.get(2).unwrap().to_string(), "Line 2");
        assert!(hot.get(3).is_none());
    }

    #[test]
    fn hot_tier_take_front() {
        let mut hot = HotTier::new();
        for i in 0..10 {
            hot.push(Line::from_str(&format!("Line {i}")));
        }

        let taken = hot.take_front(3);
        assert_eq!(taken.len(), 3);
        assert_eq!(taken[0].to_string(), "Line 0");
        assert_eq!(taken[2].to_string(), "Line 2");
        assert_eq!(hot.len(), 7);
        assert_eq!(hot.get(0).unwrap().to_string(), "Line 3");
    }

    #[test]
    fn hot_tier_truncate_front() {
        let mut hot = HotTier::new();
        for i in 0..10 {
            hot.push(Line::from_str(&format!("Line {i}")));
        }

        hot.truncate_front(3);
        assert_eq!(hot.len(), 3);
        assert_eq!(hot.get(0).unwrap().to_string(), "Line 7");
    }

    // Warm tier tests
    #[test]
    fn warm_block_roundtrip() {
        let lines: Vec<Line> = (0..10)
            .map(|i| Line::from_str(&format!("Line {i}")))
            .collect();

        let block = WarmBlock::from_lines(lines);
        assert_eq!(block.line_count(), 10);
        assert!(block.compressed_size() > 0);

        let decompressed = block.decompress();
        assert_eq!(decompressed.len(), 10);
        assert_eq!(decompressed[0].to_string(), "Line 0");
        assert_eq!(decompressed[9].to_string(), "Line 9");
    }

    #[test]
    fn warm_block_get_line() {
        let lines: Vec<Line> = (0..10)
            .map(|i| Line::from_str(&format!("Line {i}")))
            .collect();

        let block = WarmBlock::from_lines(lines);
        assert_eq!(block.get_line(0).unwrap().to_string(), "Line 0");
        assert_eq!(block.get_line(5).unwrap().to_string(), "Line 5");
        assert!(block.get_line(10).is_none());
    }

    #[test]
    fn warm_tier_push_get() {
        let mut warm = WarmTier::new();

        let lines1: Vec<Line> = (0..5)
            .map(|i| Line::from_str(&format!("Block0-Line{i}")))
            .collect();
        let lines2: Vec<Line> = (0..5)
            .map(|i| Line::from_str(&format!("Block1-Line{i}")))
            .collect();

        warm.push_block(lines1);
        warm.push_block(lines2);

        assert_eq!(warm.line_count(), 10);
        assert_eq!(warm.block_count(), 2);

        // Test access across blocks
        assert_eq!(warm.get_line(0).unwrap().to_string(), "Block0-Line0");
        assert_eq!(warm.get_line(4).unwrap().to_string(), "Block0-Line4");
        assert_eq!(warm.get_line(5).unwrap().to_string(), "Block1-Line0");
        assert_eq!(warm.get_line(9).unwrap().to_string(), "Block1-Line4");
        assert!(warm.get_line(10).is_none());
    }

    #[test]
    fn warm_tier_pop_front() {
        let mut warm = WarmTier::new();

        let lines1: Vec<Line> = (0..5)
            .map(|i| Line::from_str(&format!("Block0-Line{i}")))
            .collect();
        let lines2: Vec<Line> = (0..5)
            .map(|i| Line::from_str(&format!("Block1-Line{i}")))
            .collect();

        warm.push_block(lines1);
        warm.push_block(lines2);

        let block = warm.pop_front().unwrap();
        assert_eq!(block.line_count(), 5);
        assert_eq!(warm.line_count(), 5);
        assert_eq!(warm.get_line(0).unwrap().to_string(), "Block1-Line0");
    }

    // Cold tier tests
    #[test]
    fn cold_tier_push_get() {
        let mut cold = ColdTier::new();

        let lines: Vec<Line> = (0..10)
            .map(|i| Line::from_str(&format!("Line {i}")))
            .collect();
        let warm_block = WarmBlock::from_lines(lines);

        cold.push_block(warm_block);

        assert_eq!(cold.line_count(), 10);
        assert_eq!(cold.page_count(), 1);
        assert_eq!(cold.get_line(0).unwrap().to_string(), "Line 0");
        assert_eq!(cold.get_line(9).unwrap().to_string(), "Line 9");
        assert!(cold.get_line(10).is_none());
    }

    #[test]
    fn cold_tier_multiple_pages() {
        let mut cold = ColdTier::new();

        for block_idx in 0..3 {
            let lines: Vec<Line> = (0..5)
                .map(|i| Line::from_str(&format!("Block{block_idx}-Line{i}")))
                .collect();
            let warm_block = WarmBlock::from_lines(lines);
            cold.push_block(warm_block);
        }

        assert_eq!(cold.line_count(), 15);
        assert_eq!(cold.page_count(), 3);

        // Test access across pages
        assert_eq!(cold.get_line(0).unwrap().to_string(), "Block0-Line0");
        assert_eq!(cold.get_line(5).unwrap().to_string(), "Block1-Line0");
        assert_eq!(cold.get_line(10).unwrap().to_string(), "Block2-Line0");
        assert_eq!(cold.get_line(14).unwrap().to_string(), "Block2-Line4");
    }

    #[test]
    fn compression_ratio() {
        // Create some realistic terminal output
        let lines: Vec<Line> = (0..100)
            .map(|i| {
                Line::from_str(&format!(
                    "[2024-01-01 12:00:{:02}] INFO: Processing item {} - status OK",
                    i % 60,
                    i
                ))
            })
            .collect();

        let uncompressed_size: usize = lines.iter().map(|l| l.len()).sum();

        let warm_block = WarmBlock::from_lines(lines.clone());
        let lz4_size = warm_block.compressed_size();

        let cold_page = ColdPage::from_warm_block(&warm_block);
        let zstd_size = cold_page.compressed.len();

        // LZ4 should compress significantly
        assert!(lz4_size < uncompressed_size);
        // Zstd should compress more than LZ4
        assert!(zstd_size < lz4_size);

        // Verify data integrity
        let decompressed = cold_page.decompress();
        assert_eq!(decompressed.len(), 100);
        for (i, line) in decompressed.iter().enumerate() {
            assert_eq!(
                line.to_string(),
                format!(
                    "[2024-01-01 12:00:{:02}] INFO: Processing item {} - status OK",
                    i % 60,
                    i
                )
            );
        }
    }
}
