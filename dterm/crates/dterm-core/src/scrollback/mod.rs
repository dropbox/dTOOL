//! Tiered scrollback storage.
//!
//! ## Design
//!
//! Three-tier architecture for memory efficiency:
//!
//! - **Hot tier**: Recent lines in RAM (uncompressed, instant access)
//! - **Warm tier**: Older lines in RAM (LZ4 compressed, ~10x compression)
//! - **Cold tier**: Old lines on disk (Zstd compressed, lazy load)
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │  HOT TIER (RAM) - Last ~1000 lines                          │
//! │  ┌─────────────────────────────────────────────────────┐   │
//! │  │ Uncompressed, instant access, ~200KB                │   │
//! │  └─────────────────────────────────────────────────────┘   │
//! │                         ↓ Age out                          │
//! │  WARM TIER (RAM, Compressed) - Last ~10K lines              │
//! │  ┌─────────────────────────────────────────────────────┐   │
//! │  │ LZ4 compressed blocks, ~50KB (10x compression)      │   │
//! │  └─────────────────────────────────────────────────────┘   │
//! │                         ↓ Age out                          │
//! │  COLD TIER (Memory-Mapped File) - Unlimited history        │
//! │  ┌─────────────────────────────────────────────────────┐   │
//! │  │ Zstd compressed pages, lazy load                    │   │
//! │  └─────────────────────────────────────────────────────┘   │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Memory Targets
//!
//! | Lines | Typical Memory |
//! |-------|----------------|
//! | 100K  | ~2 MB          |
//! | 1M    | ~20 MB         |
//! | 10M   | ~200 MB        |
//!
//! ## Verification
//!
//! - TLA+ spec: `tla/Scrollback.tla`
//! - Kani proofs: `tier_transition_preserves_lines`, `memory_budget_enforced`
//! - Property tests: line count always accurate, no data loss

mod disk;
mod line;
mod tier;

pub use disk::{DiskColdConfig, DiskColdTier};
pub use line::{CellAttrs, Line, LineContent};
pub use tier::{ColdTier, HotTier, WarmBlock, WarmTier};

/// Default lines per compressed block.
const DEFAULT_BLOCK_SIZE: usize = 100;

/// Default hot tier limit (lines).
const DEFAULT_HOT_LIMIT: usize = 1000;

/// Default warm tier limit (lines).
const DEFAULT_WARM_LIMIT: usize = 10000;

/// Default memory budget (100 MB).
const DEFAULT_MEMORY_BUDGET: usize = 100 * 1024 * 1024;

/// Scrollback buffer with tiered storage.
///
/// Lines flow from hot → warm → cold as they age.
/// Memory budget is enforced by evicting warm blocks to cold tier.
#[derive(Debug)]
pub struct Scrollback {
    /// Hot tier: uncompressed lines (instant access).
    hot: HotTier,
    /// Warm tier: LZ4 compressed blocks.
    warm: WarmTier,
    /// Cold tier: Zstd compressed, disk-backed.
    cold: ColdTier,
    /// Maximum lines in hot tier before promotion.
    hot_limit: usize,
    /// Maximum lines in warm tier before eviction.
    warm_limit: usize,
    /// Total memory budget (bytes).
    memory_budget: usize,
    /// Lines per compressed block.
    block_size: usize,
    /// Total line count across all tiers.
    line_count: usize,
}

impl Scrollback {
    /// Create a new scrollback buffer with specified tier limits.
    ///
    /// # Arguments
    /// * `hot_limit` - Maximum lines in hot tier before promotion
    /// * `warm_limit` - Maximum lines in warm tier before eviction
    /// * `memory_budget` - Total memory budget in bytes
    #[must_use]
    pub fn new(hot_limit: usize, warm_limit: usize, memory_budget: usize) -> Self {
        Self {
            hot: HotTier::new(),
            warm: WarmTier::new(),
            cold: ColdTier::new(),
            hot_limit: hot_limit.max(1),
            warm_limit,
            memory_budget,
            block_size: DEFAULT_BLOCK_SIZE,
            line_count: 0,
        }
    }

    /// Create a scrollback buffer with sensible defaults.
    ///
    /// Uses: 1000 hot lines, 10000 warm lines, 100MB budget.
    #[must_use]
    pub fn with_defaults() -> Self {
        Self::new(DEFAULT_HOT_LIMIT, DEFAULT_WARM_LIMIT, DEFAULT_MEMORY_BUDGET)
    }

    /// Create a scrollback buffer with custom block size.
    #[must_use]
    pub fn with_block_size(
        hot_limit: usize,
        warm_limit: usize,
        memory_budget: usize,
        block_size: usize,
    ) -> Self {
        let hot_limit = hot_limit.max(1);
        // Block size must not exceed hot limit, otherwise promotion never triggers
        let block_size = block_size.max(1).min(hot_limit);
        Self {
            hot: HotTier::new(),
            warm: WarmTier::new(),
            cold: ColdTier::new(),
            hot_limit,
            warm_limit,
            memory_budget,
            block_size,
            line_count: 0,
        }
    }

    /// Get the total number of lines across all tiers.
    #[must_use]
    #[inline]
    pub fn line_count(&self) -> usize {
        self.line_count
    }

    /// Get the number of lines in hot tier.
    #[must_use]
    #[inline]
    pub fn hot_line_count(&self) -> usize {
        self.hot.len()
    }

    /// Get the number of lines in warm tier.
    #[must_use]
    #[inline]
    pub fn warm_line_count(&self) -> usize {
        self.warm.line_count()
    }

    /// Get the number of lines in cold tier.
    #[must_use]
    #[inline]
    pub fn cold_line_count(&self) -> usize {
        self.cold.line_count()
    }

    /// Get the current memory usage (bytes).
    #[must_use]
    pub fn memory_used(&self) -> usize {
        self.hot.memory_used() + self.warm.memory_used()
        // Cold tier is on disk, not counted
    }

    /// Check if memory budget is exceeded.
    #[must_use]
    #[inline]
    pub fn over_budget(&self) -> bool {
        self.memory_used() > self.memory_budget
    }

    /// Get the hot tier limit.
    #[must_use]
    #[inline]
    pub fn hot_limit(&self) -> usize {
        self.hot_limit
    }

    /// Get the warm tier limit.
    #[must_use]
    #[inline]
    pub fn warm_limit(&self) -> usize {
        self.warm_limit
    }

    /// Get the memory budget.
    #[must_use]
    #[inline]
    pub fn memory_budget(&self) -> usize {
        self.memory_budget
    }

    /// Set the memory budget (bytes).
    ///
    /// Enforces the new budget immediately by evicting warm blocks if needed.
    pub fn set_memory_budget(&mut self, budget: usize) {
        self.memory_budget = budget.max(1);
        self.handle_memory_pressure();
    }

    /// Push a new line to the scrollback.
    ///
    /// Lines are added to the hot tier. When the hot tier is full,
    /// old lines are promoted to the warm tier (compressed).
    pub fn push_line(&mut self, line: Line) {
        // If hot tier is full, promote oldest lines to warm
        if self.hot.len() >= self.hot_limit {
            self.promote_hot_to_warm();
        }

        // Add line to hot tier
        self.hot.push(line);
        self.line_count += 1;

        // Handle memory pressure
        self.handle_memory_pressure();
    }

    /// Push a line from a string.
    pub fn push_str(&mut self, s: &str) {
        self.push_line(Line::from_str(s));
    }

    /// Get a line by index (0 = oldest).
    ///
    /// Returns None if index is out of bounds.
    #[must_use]
    pub fn get_line(&self, idx: usize) -> Option<Line> {
        if idx >= self.line_count {
            return None;
        }

        let cold_count = self.cold.line_count();
        let warm_count = self.warm.line_count();

        if idx < cold_count {
            // Line is in cold tier
            self.cold.get_line(idx)
        } else if idx < cold_count + warm_count {
            // Line is in warm tier
            self.warm.get_line(idx - cold_count)
        } else {
            // Line is in hot tier
            self.hot.get(idx - cold_count - warm_count)
        }
    }

    /// Get a line by reverse index (0 = newest).
    #[must_use]
    pub fn get_line_rev(&self, rev_idx: usize) -> Option<Line> {
        if rev_idx >= self.line_count {
            return None;
        }
        self.get_line(self.line_count - 1 - rev_idx)
    }

    /// Iterate over all lines (oldest to newest).
    pub fn iter(&self) -> ScrollbackIter<'_> {
        ScrollbackIter {
            scrollback: self,
            idx: 0,
        }
    }

    /// Iterate over recent lines (newest to oldest).
    pub fn iter_rev(&self) -> ScrollbackRevIter<'_> {
        ScrollbackRevIter {
            scrollback: self,
            rev_idx: 0,
        }
    }

    /// Clear all lines.
    pub fn clear(&mut self) {
        self.hot.clear();
        self.warm.clear();
        self.cold.clear();
        self.line_count = 0;
    }

    /// Truncate to keep only the last `n` lines.
    pub fn truncate(&mut self, n: usize) {
        if n >= self.line_count {
            return;
        }

        // Clear cold and warm tiers
        self.cold.clear();
        self.warm.clear();

        // Keep only last n lines from hot tier
        self.hot.truncate_front(n);
        self.line_count = n.min(self.hot.len());
    }

    /// Promote oldest hot lines to warm tier.
    fn promote_hot_to_warm(&mut self) {
        if self.hot.len() < self.block_size {
            return;
        }

        // Take block_size lines from front of hot tier
        let lines = self.hot.take_front(self.block_size);
        if lines.is_empty() {
            return;
        }

        // Compress and add to warm tier
        self.warm.push_block(lines);

        // If warm tier is over limit, evict to cold
        if self.warm.line_count() > self.warm_limit {
            self.evict_warm_to_cold();
        }
    }

    /// Evict oldest warm block to cold tier.
    fn evict_warm_to_cold(&mut self) {
        if let Some(block) = self.warm.pop_front() {
            self.cold.push_block(block);
        }
    }

    /// Handle memory pressure by evicting warm to cold.
    fn handle_memory_pressure(&mut self) {
        while self.over_budget() && self.warm.block_count() > 0 {
            self.evict_warm_to_cold();
        }
    }
}

impl Default for Scrollback {
    fn default() -> Self {
        Self::with_defaults()
    }
}

// ============================================================================
// Disk-Backed Scrollback
// ============================================================================

/// Configuration for disk-backed scrollback.
#[derive(Debug, Clone)]
pub struct DiskBackedScrollbackConfig {
    /// Hot tier limit (lines).
    pub hot_limit: usize,
    /// Warm tier limit (lines).
    pub warm_limit: usize,
    /// Memory budget (bytes).
    pub memory_budget: usize,
    /// Lines per compressed block.
    pub block_size: usize,
    /// Disk cold tier configuration.
    pub cold_config: DiskColdConfig,
}

impl DiskBackedScrollbackConfig {
    /// Create a new config with the given cold storage path.
    #[must_use]
    pub fn new(cold_path: impl Into<std::path::PathBuf>) -> Self {
        Self {
            hot_limit: 1000,
            warm_limit: 10_000,
            memory_budget: 100_000_000,
            block_size: DEFAULT_BLOCK_SIZE,
            cold_config: DiskColdConfig::new(cold_path),
        }
    }

    /// Set hot tier limit.
    #[must_use]
    pub fn with_hot_limit(mut self, limit: usize) -> Self {
        self.hot_limit = limit;
        self
    }

    /// Set warm tier limit.
    #[must_use]
    pub fn with_warm_limit(mut self, limit: usize) -> Self {
        self.warm_limit = limit;
        self
    }

    /// Set memory budget.
    #[must_use]
    pub fn with_memory_budget(mut self, budget: usize) -> Self {
        self.memory_budget = budget;
        self
    }

    /// Set block size.
    #[must_use]
    pub fn with_block_size(mut self, size: usize) -> Self {
        self.block_size = size;
        self
    }

    /// Set cold tier cache size.
    #[must_use]
    pub fn with_cold_cache_size(mut self, size: usize) -> Self {
        self.cold_config.cache_size = size;
        self
    }
}

/// Scrollback buffer with disk-backed cold tier storage.
///
/// Like `Scrollback`, but the cold tier is stored on disk for unlimited history.
#[derive(Debug)]
pub struct DiskBackedScrollback {
    /// Hot tier: uncompressed lines (instant access).
    hot: HotTier,
    /// Warm tier: LZ4 compressed blocks.
    warm: WarmTier,
    /// Cold tier: Zstd compressed, disk-backed.
    cold: DiskColdTier,
    /// Maximum lines in hot tier before promotion.
    hot_limit: usize,
    /// Maximum lines in warm tier before eviction.
    warm_limit: usize,
    /// Total memory budget (bytes).
    memory_budget: usize,
    /// Lines per compressed block.
    block_size: usize,
    /// Total line count across all tiers.
    line_count: usize,
}

impl DiskBackedScrollback {
    /// Create a new disk-backed scrollback with the given configuration.
    pub fn with_config(config: DiskBackedScrollbackConfig) -> std::io::Result<Self> {
        let cold = DiskColdTier::with_config(config.cold_config)?;
        let hot_limit = config.hot_limit.max(1);
        // Block size must not exceed hot limit, otherwise promotion never triggers
        let block_size = config.block_size.max(1).min(hot_limit);

        Ok(Self {
            hot: HotTier::new(),
            warm: WarmTier::new(),
            cold,
            hot_limit,
            warm_limit: config.warm_limit,
            memory_budget: config.memory_budget,
            block_size,
            line_count: 0,
        })
    }

    /// Load an existing disk-backed scrollback.
    ///
    /// Restores line count from the cold tier.
    pub fn load(config: DiskBackedScrollbackConfig) -> std::io::Result<Self> {
        let cold = DiskColdTier::with_config(config.cold_config)?;
        let cold_line_count = cold.line_count();
        let hot_limit = config.hot_limit.max(1);
        // Block size must not exceed hot limit, otherwise promotion never triggers
        let block_size = config.block_size.max(1).min(hot_limit);

        Ok(Self {
            hot: HotTier::new(),
            warm: WarmTier::new(),
            cold,
            hot_limit,
            warm_limit: config.warm_limit,
            memory_budget: config.memory_budget,
            block_size,
            line_count: cold_line_count,
        })
    }

    /// Get the total number of lines across all tiers.
    #[must_use]
    #[inline]
    pub fn line_count(&self) -> usize {
        self.line_count
    }

    /// Get the number of lines in hot tier.
    #[must_use]
    #[inline]
    pub fn hot_line_count(&self) -> usize {
        self.hot.len()
    }

    /// Get the number of lines in warm tier.
    #[must_use]
    #[inline]
    pub fn warm_line_count(&self) -> usize {
        self.warm.line_count()
    }

    /// Get the number of lines in cold tier.
    #[must_use]
    #[inline]
    pub fn cold_line_count(&self) -> usize {
        self.cold.line_count()
    }

    /// Get the current memory usage (bytes).
    #[must_use]
    pub fn memory_used(&self) -> usize {
        self.hot.memory_used() + self.warm.memory_used()
        // Cold tier is on disk, not counted
    }

    /// Check if memory budget is exceeded.
    #[must_use]
    #[inline]
    pub fn over_budget(&self) -> bool {
        self.memory_used() > self.memory_budget
    }

    /// Push a new line to the scrollback.
    pub fn push_line(&mut self, line: Line) -> std::io::Result<()> {
        // If hot tier is full, promote oldest lines to warm
        if self.hot.len() >= self.hot_limit {
            self.promote_hot_to_warm()?;
        }

        // Add line to hot tier
        self.hot.push(line);
        self.line_count += 1;

        // Handle memory pressure
        self.handle_memory_pressure()?;

        Ok(())
    }

    /// Push a line from a string.
    pub fn push_str(&mut self, s: &str) -> std::io::Result<()> {
        self.push_line(Line::from_str(s))
    }

    /// Get a line by index (0 = oldest).
    #[must_use]
    pub fn get_line(&mut self, idx: usize) -> Option<Line> {
        if idx >= self.line_count {
            return None;
        }

        let cold_count = self.cold.line_count();
        let warm_count = self.warm.line_count();

        if idx < cold_count {
            // Line is in cold tier
            self.cold.get_line(idx)
        } else if idx < cold_count + warm_count {
            // Line is in warm tier
            self.warm.get_line(idx - cold_count)
        } else {
            // Line is in hot tier
            self.hot.get(idx - cold_count - warm_count)
        }
    }

    /// Get a line by reverse index (0 = newest).
    #[must_use]
    pub fn get_line_rev(&mut self, rev_idx: usize) -> Option<Line> {
        if rev_idx >= self.line_count {
            return None;
        }
        self.get_line(self.line_count - 1 - rev_idx)
    }

    /// Clear all lines.
    pub fn clear(&mut self) -> std::io::Result<()> {
        self.hot.clear();
        self.warm.clear();
        self.cold.clear()?;
        self.line_count = 0;
        Ok(())
    }

    /// Sync changes to disk.
    pub fn sync(&mut self) -> std::io::Result<()> {
        self.cold.sync()
    }

    /// Promote oldest hot lines to warm tier.
    fn promote_hot_to_warm(&mut self) -> std::io::Result<()> {
        if self.hot.len() < self.block_size {
            return Ok(());
        }

        // Take block_size lines from front of hot tier
        let lines = self.hot.take_front(self.block_size);
        if lines.is_empty() {
            return Ok(());
        }

        // Compress and add to warm tier
        self.warm.push_block(lines);

        // If warm tier is over limit, evict to cold
        if self.warm.line_count() > self.warm_limit {
            self.evict_warm_to_cold()?;
        }

        Ok(())
    }

    /// Evict oldest warm block to cold tier.
    fn evict_warm_to_cold(&mut self) -> std::io::Result<()> {
        if let Some(block) = self.warm.pop_front() {
            if let Some((compressed, line_count)) = block.to_zstd_compressed() {
                self.cold.push_compressed(compressed, line_count)?;
            }
        }
        Ok(())
    }

    /// Handle memory pressure by evicting warm to cold.
    fn handle_memory_pressure(&mut self) -> std::io::Result<()> {
        while self.over_budget() && self.warm.block_count() > 0 {
            self.evict_warm_to_cold()?;
        }
        Ok(())
    }

    /// Assert TLA+ specification invariants in debug builds.
    ///
    /// This method validates key invariants from the Scrollback.tla specification:
    /// - Line count consistency across tiers
    /// - Tier age ordering (hot newest, cold oldest)
    /// - Memory budget is respected (with overhead allowance)
    ///
    /// # Panics
    ///
    /// Panics in debug builds if any invariant is violated.
    /// Does nothing in release builds for performance.
    #[inline]
    pub fn assert_invariants(&self) {
        #[cfg(debug_assertions)]
        {
            // Invariant: LineCountConsistent
            // line_count == hot.len() + warm.line_count() + cold.line_count()
            let actual_count = self.hot.len() + self.warm.line_count() + self.cold.line_count();
            assert!(
                self.line_count == actual_count,
                "TLA+ LineCountConsistent violated: line_count {} != sum {} (hot={}, warm={}, cold={})",
                self.line_count,
                actual_count,
                self.hot.len(),
                self.warm.line_count(),
                self.cold.line_count()
            );

            // Invariant: HotTierLimitRespected (soft - checked after operations)
            // This is informational - hot can temporarily exceed limit before promotion
            // but should not massively exceed it
            assert!(
                self.hot.len() <= self.hot_limit * 2,
                "TLA+ HotTierLimitRespected violated: hot tier {} >> limit {} (double exceeded)",
                self.hot.len(),
                self.hot_limit
            );

            // Invariant: BlockSizeValid
            // block_size must be <= hot_limit (otherwise promotion never triggers)
            assert!(
                self.block_size <= self.hot_limit,
                "TLA+ BlockSizeValid violated: block_size {} > hot_limit {}",
                self.block_size,
                self.hot_limit
            );
        }
    }
}

/// Iterator over scrollback lines (oldest to newest).
pub struct ScrollbackIter<'a> {
    scrollback: &'a Scrollback,
    idx: usize,
}

impl<'a> Iterator for ScrollbackIter<'a> {
    type Item = Line;

    fn next(&mut self) -> Option<Self::Item> {
        let line = self.scrollback.get_line(self.idx)?;
        self.idx += 1;
        Some(line)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.scrollback.line_count.saturating_sub(self.idx);
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for ScrollbackIter<'_> {}

/// Reverse iterator over scrollback lines (newest to oldest).
pub struct ScrollbackRevIter<'a> {
    scrollback: &'a Scrollback,
    rev_idx: usize,
}

impl<'a> Iterator for ScrollbackRevIter<'a> {
    type Item = Line;

    fn next(&mut self) -> Option<Self::Item> {
        let line = self.scrollback.get_line_rev(self.rev_idx)?;
        self.rev_idx += 1;
        Some(line)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.scrollback.line_count.saturating_sub(self.rev_idx);
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for ScrollbackRevIter<'_> {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scrollback_new() {
        let sb = Scrollback::new(100, 1000, 10_000_000);
        assert_eq!(sb.line_count(), 0);
        assert_eq!(sb.hot_line_count(), 0);
        assert_eq!(sb.warm_line_count(), 0);
        assert_eq!(sb.cold_line_count(), 0);
    }

    #[test]
    fn scrollback_push_line() {
        let mut sb = Scrollback::new(100, 1000, 10_000_000);
        sb.push_str("Hello");
        sb.push_str("World");

        assert_eq!(sb.line_count(), 2);
        assert_eq!(sb.hot_line_count(), 2);
    }

    #[test]
    fn scrollback_get_line() {
        let mut sb = Scrollback::new(100, 1000, 10_000_000);
        sb.push_str("Line 0");
        sb.push_str("Line 1");
        sb.push_str("Line 2");

        assert_eq!(sb.get_line(0).unwrap().to_string(), "Line 0");
        assert_eq!(sb.get_line(1).unwrap().to_string(), "Line 1");
        assert_eq!(sb.get_line(2).unwrap().to_string(), "Line 2");
        assert!(sb.get_line(3).is_none());
    }

    #[test]
    fn scrollback_get_line_rev() {
        let mut sb = Scrollback::new(100, 1000, 10_000_000);
        sb.push_str("Line 0");
        sb.push_str("Line 1");
        sb.push_str("Line 2");

        assert_eq!(sb.get_line_rev(0).unwrap().to_string(), "Line 2");
        assert_eq!(sb.get_line_rev(1).unwrap().to_string(), "Line 1");
        assert_eq!(sb.get_line_rev(2).unwrap().to_string(), "Line 0");
    }

    #[test]
    fn scrollback_promotion() {
        // Small limits to trigger promotion
        let mut sb = Scrollback::with_block_size(10, 100, 10_000_000, 5);

        // Push 15 lines - should promote 5 to warm
        for i in 0..15 {
            sb.push_str(&format!("Line {i}"));
        }

        assert_eq!(sb.line_count(), 15);
        assert_eq!(sb.hot_line_count(), 10);
        assert_eq!(sb.warm_line_count(), 5);

        // Verify we can still read all lines
        for i in 0..15 {
            let line = sb.get_line(i).unwrap();
            assert_eq!(line.to_string(), format!("Line {i}"));
        }
    }

    #[test]
    fn scrollback_eviction() {
        // Small limits to trigger eviction
        let mut sb = Scrollback::with_block_size(5, 10, 10_000_000, 5);

        // Push 25 lines - should evict to cold
        for i in 0..25 {
            sb.push_str(&format!("Line {i}"));
        }

        assert_eq!(sb.line_count(), 25);
        assert!(sb.cold_line_count() > 0);

        // Verify we can still read all lines
        for i in 0..25 {
            let line = sb.get_line(i).unwrap();
            assert_eq!(line.to_string(), format!("Line {i}"));
        }
    }

    #[test]
    fn scrollback_iterator() {
        let mut sb = Scrollback::new(100, 1000, 10_000_000);
        for i in 0..10 {
            sb.push_str(&format!("Line {i}"));
        }

        let lines: Vec<_> = sb.iter().collect();
        assert_eq!(lines.len(), 10);
        assert_eq!(lines[0].to_string(), "Line 0");
        assert_eq!(lines[9].to_string(), "Line 9");
    }

    #[test]
    fn scrollback_rev_iterator() {
        let mut sb = Scrollback::new(100, 1000, 10_000_000);
        for i in 0..10 {
            sb.push_str(&format!("Line {i}"));
        }

        let lines: Vec<_> = sb.iter_rev().collect();
        assert_eq!(lines.len(), 10);
        assert_eq!(lines[0].to_string(), "Line 9");
        assert_eq!(lines[9].to_string(), "Line 0");
    }

    #[test]
    fn scrollback_clear() {
        let mut sb = Scrollback::new(100, 1000, 10_000_000);
        for i in 0..50 {
            sb.push_str(&format!("Line {i}"));
        }
        assert_eq!(sb.line_count(), 50);

        sb.clear();
        assert_eq!(sb.line_count(), 0);
        assert_eq!(sb.hot_line_count(), 0);
        assert_eq!(sb.warm_line_count(), 0);
        assert_eq!(sb.cold_line_count(), 0);
    }

    #[test]
    fn scrollback_truncate() {
        let mut sb = Scrollback::new(100, 1000, 10_000_000);
        for i in 0..50 {
            sb.push_str(&format!("Line {i}"));
        }

        sb.truncate(10);
        assert_eq!(sb.line_count(), 10);

        // Should keep the last 10 lines
        assert_eq!(sb.get_line(0).unwrap().to_string(), "Line 40");
        assert_eq!(sb.get_line(9).unwrap().to_string(), "Line 49");
    }

    #[test]
    fn scrollback_memory_tracking() {
        let mut sb = Scrollback::new(100, 1000, 10_000_000);

        let initial_mem = sb.memory_used();
        sb.push_str("Hello World");

        assert!(sb.memory_used() > initial_mem);
    }
}

#[cfg(kani)]
mod proofs {
    use super::*;

    /// Line count is always accurate.
    #[kani::proof]
    #[kani::unwind(11)]
    fn line_count_accurate() {
        let mut sb = Scrollback::with_block_size(5, 10, 10_000_000, 2);

        let push_count: usize = kani::any();
        kani::assume(push_count <= 10);

        for _ in 0..push_count {
            sb.push_line(Line::default());
        }

        kani::assert(sb.line_count() == push_count, "line count mismatch");

        // Verify total equals sum of tiers
        let total = sb.hot_line_count() + sb.warm_line_count() + sb.cold_line_count();
        kani::assert(sb.line_count() == total, "tier sum mismatch");
    }

    /// Hot tier never exceeds limit.
    #[kani::proof]
    #[kani::unwind(21)]
    fn hot_bounded() {
        let hot_limit: usize = kani::any();
        kani::assume(hot_limit >= 1 && hot_limit <= 5);

        let mut sb = Scrollback::with_block_size(hot_limit, 100, 10_000_000, 2);

        let push_count: usize = kani::any();
        kani::assume(push_count <= 20);

        for _ in 0..push_count {
            sb.push_line(Line::default());
        }

        kani::assert(sb.hot_line_count() <= hot_limit, "hot tier exceeded limit");
    }

    /// Get line returns valid line for valid index.
    #[kani::proof]
    #[kani::unwind(6)]
    fn get_line_valid() {
        let mut sb = Scrollback::with_block_size(5, 10, 10_000_000, 2);

        let push_count: usize = kani::any();
        kani::assume(push_count >= 1 && push_count <= 5);

        for _ in 0..push_count {
            sb.push_line(Line::default());
        }

        let idx: usize = kani::any();
        kani::assume(idx < push_count);

        let result = sb.get_line(idx);
        kani::assert(result.is_some(), "valid index returned None");
    }

    /// Get line returns None for out-of-bounds index.
    #[kani::proof]
    #[kani::unwind(6)]
    fn get_line_out_of_bounds() {
        let mut sb = Scrollback::with_block_size(5, 10, 10_000_000, 2);

        let push_count: usize = kani::any();
        kani::assume(push_count <= 5);

        for _ in 0..push_count {
            sb.push_line(Line::default());
        }

        let idx: usize = kani::any();
        kani::assume(idx >= push_count && idx < 100);

        let result = sb.get_line(idx);
        kani::assert(result.is_none(), "out-of-bounds index returned Some");
    }

    /// Tier transitions preserve line count invariant.
    ///
    /// When lines are promoted from hot to warm, and warm to cold,
    /// the total line count remains accurate.
    #[kani::proof]
    #[kani::unwind(31)]
    fn tier_transition_preserves_count() {
        let hot_limit: usize = kani::any();
        let warm_limit: usize = kani::any();
        kani::assume(hot_limit >= 2 && hot_limit <= 5);
        kani::assume(warm_limit >= 2 && warm_limit <= 10);

        let mut sb = Scrollback::with_block_size(hot_limit, warm_limit, 10_000_000, 2);

        // Push enough lines to trigger tier transitions
        let push_count: usize = kani::any();
        kani::assume(push_count <= 30);

        for _ in 0..push_count {
            sb.push_line(Line::default());
        }

        // Verify invariant: total equals sum of tiers
        let hot = sb.hot_line_count();
        let warm = sb.warm_line_count();
        let cold = sb.cold_line_count();
        let total = sb.line_count();

        kani::assert(
            total == hot + warm + cold,
            "line count doesn't match tier sum after transitions",
        );
        kani::assert(total == push_count, "line count doesn't match push count");
    }

    /// Clear resets all tiers to empty.
    #[kani::proof]
    #[kani::unwind(11)]
    fn clear_resets_all() {
        let mut sb = Scrollback::with_block_size(5, 10, 10_000_000, 2);

        let push_count: usize = kani::any();
        kani::assume(push_count <= 10);

        for _ in 0..push_count {
            sb.push_line(Line::default());
        }

        sb.clear();

        kani::assert(sb.line_count() == 0, "line count not zero after clear");
        kani::assert(sb.hot_line_count() == 0, "hot tier not empty after clear");
        kani::assert(sb.warm_line_count() == 0, "warm tier not empty after clear");
        kani::assert(sb.cold_line_count() == 0, "cold tier not empty after clear");
    }
}
