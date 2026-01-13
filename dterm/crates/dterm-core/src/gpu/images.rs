//! Image texture management for GPU rendering.
//!
//! This module provides GPU texture management for inline images in terminals,
//! supporting Sixel, Kitty graphics protocol, and iTerm2 image protocol.
//!
//! ## Design
//!
//! - LRU cache with configurable memory budget (default 64MB)
//! - Handles RGBA, RGB, and ARGB (Sixel) formats
//! - Supports negative row indices for scrollback placement
//! - Automatic texture eviction when budget is exceeded

use rustc_hash::FxHashMap;
use std::collections::VecDeque;

/// Default image memory budget in bytes (64 MB).
pub const DEFAULT_IMAGE_BUDGET: usize = 64 * 1024 * 1024;

/// Maximum image dimension in pixels (same as Kitty).
pub const MAX_IMAGE_DIMENSION: u32 = 10000;

/// Handle to a GPU image texture.
///
/// Handles are unique within a cache instance and are never reused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ImageHandle(pub u64);

impl ImageHandle {
    /// Create a handle from a raw u64 value.
    #[inline]
    pub const fn from_raw(value: u64) -> Self {
        Self(value)
    }

    /// Get the raw u64 value.
    #[inline]
    pub const fn raw(self) -> u64 {
        self.0
    }

    /// A null/invalid handle.
    pub const NULL: Self = Self(0);

    /// Check if this is a null handle.
    #[inline]
    pub const fn is_null(self) -> bool {
        self.0 == 0
    }
}

/// Placement of an image in the terminal grid.
#[derive(Debug, Clone, Copy)]
pub struct ImagePlacement {
    /// Handle to the image texture.
    pub handle: ImageHandle,
    /// Row position (negative = scrollback).
    pub row: i64,
    /// Column position.
    pub col: u16,
    /// Width in terminal cells.
    pub width_cells: u16,
    /// Height in terminal cells.
    pub height_cells: u16,
    /// Z-index for stacking (negative = below text).
    pub z_index: i32,
}

impl ImagePlacement {
    /// Create a new placement.
    pub const fn new(
        handle: ImageHandle,
        row: i64,
        col: u16,
        width_cells: u16,
        height_cells: u16,
    ) -> Self {
        Self {
            handle,
            row,
            col,
            width_cells,
            height_cells,
            z_index: 0,
        }
    }

    /// Create a new placement with z-index.
    #[must_use]
    pub const fn with_z_index(mut self, z_index: i32) -> Self {
        self.z_index = z_index;
        self
    }
}

/// Image format for upload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ImageFormat {
    /// RGBA (4 bytes per pixel, R-G-B-A order).
    Rgba = 0,
    /// RGB (3 bytes per pixel, R-G-B order).
    Rgb = 1,
    /// ARGB (4 bytes per pixel, A-R-G-B order - Sixel format).
    Argb = 2,
}

impl ImageFormat {
    /// Bytes per pixel for this format.
    #[inline]
    pub const fn bytes_per_pixel(self) -> usize {
        match self {
            Self::Rgba | Self::Argb => 4,
            Self::Rgb => 3,
        }
    }

    /// Create from u8 value.
    pub const fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Rgba),
            1 => Some(Self::Rgb),
            2 => Some(Self::Argb),
            _ => None,
        }
    }
}

/// Metadata for a stored image texture.
#[derive(Debug)]
pub struct ImageEntry {
    /// Pixel width.
    pub width: u32,
    /// Pixel height.
    pub height: u32,
    /// Size in bytes of the GPU texture.
    pub gpu_size: usize,
    /// Original format used for upload.
    pub format: ImageFormat,
}

/// Cache of image textures with LRU eviction.
///
/// This cache manages GPU textures for inline images, automatically evicting
/// least-recently-used images when the memory budget is exceeded.
pub struct ImageTextureCache {
    /// Image metadata by handle.
    entries: FxHashMap<ImageHandle, ImageEntry>,
    /// LRU order (front = oldest, back = newest).
    lru_order: VecDeque<ImageHandle>,
    /// Image placements in the terminal.
    placements: Vec<ImagePlacement>,
    /// Next handle value (starts at 1, 0 is reserved for null).
    next_handle: u64,
    /// Memory budget in bytes.
    memory_budget: usize,
    /// Current memory usage in bytes.
    memory_used: usize,
}

impl Default for ImageTextureCache {
    fn default() -> Self {
        Self::new(DEFAULT_IMAGE_BUDGET)
    }
}

impl ImageTextureCache {
    /// Create a new image texture cache.
    ///
    /// # Arguments
    /// * `memory_budget` - Maximum GPU memory to use for images (bytes).
    pub fn new(memory_budget: usize) -> Self {
        Self {
            entries: FxHashMap::default(),
            lru_order: VecDeque::new(),
            placements: Vec::new(),
            next_handle: 1, // Start at 1, 0 is null
            memory_budget,
            memory_used: 0,
        }
    }

    /// Get the memory budget in bytes.
    #[inline]
    pub const fn memory_budget(&self) -> usize {
        self.memory_budget
    }

    /// Get the current memory usage in bytes.
    #[inline]
    pub const fn memory_used(&self) -> usize {
        self.memory_used
    }

    /// Get the number of stored images.
    #[inline]
    pub fn image_count(&self) -> usize {
        self.entries.len()
    }

    /// Get the number of active placements.
    #[inline]
    pub fn placement_count(&self) -> usize {
        self.placements.len()
    }

    /// Set the memory budget.
    ///
    /// If the new budget is lower than current usage, images will be evicted.
    pub fn set_memory_budget(&mut self, budget: usize) {
        self.memory_budget = budget;
        self.evict_to_budget();
    }

    /// Allocate a handle for a new image.
    ///
    /// This reserves a handle without uploading data. Call `register_upload`
    /// after successfully uploading the texture to the GPU.
    ///
    /// # Arguments
    /// * `width` - Image width in pixels.
    /// * `height` - Image height in pixels.
    /// * `format` - Image format.
    ///
    /// # Returns
    /// A new image handle, or `ImageHandle::NULL` if dimensions are invalid.
    pub fn allocate_handle(&mut self, width: u32, height: u32, format: ImageFormat) -> ImageHandle {
        // Validate dimensions
        if width == 0 || height == 0 || width > MAX_IMAGE_DIMENSION || height > MAX_IMAGE_DIMENSION
        {
            return ImageHandle::NULL;
        }

        let handle = ImageHandle(self.next_handle);
        self.next_handle += 1;

        // GPU textures are always RGBA (4 bytes per pixel)
        let gpu_size = (width as usize) * (height as usize) * 4;

        // Check if we need to evict
        while self.memory_used + gpu_size > self.memory_budget && !self.lru_order.is_empty() {
            self.evict_oldest();
        }

        let entry = ImageEntry {
            width,
            height,
            gpu_size,
            format,
        };

        self.entries.insert(handle, entry);
        self.lru_order.push_back(handle);
        self.memory_used += gpu_size;

        handle
    }

    /// Convert image data to RGBA format.
    ///
    /// # Arguments
    /// * `data` - Source image data.
    /// * `width` - Image width in pixels.
    /// * `height` - Image height in pixels.
    /// * `format` - Source format.
    ///
    /// # Returns
    /// RGBA data (4 bytes per pixel), or None if conversion fails.
    pub fn convert_to_rgba(
        data: &[u8],
        width: u32,
        height: u32,
        format: ImageFormat,
    ) -> Option<Vec<u8>> {
        let pixel_count = (width as usize) * (height as usize);
        let expected_size = pixel_count * format.bytes_per_pixel();

        if data.len() < expected_size {
            return None;
        }

        match format {
            ImageFormat::Rgba => {
                // Already RGBA, just copy
                Some(data[..expected_size].to_vec())
            }
            ImageFormat::Rgb => {
                // Convert RGB to RGBA
                let mut rgba = Vec::with_capacity(pixel_count * 4);
                for i in 0..pixel_count {
                    let offset = i * 3;
                    rgba.push(data[offset]); // R
                    rgba.push(data[offset + 1]); // G
                    rgba.push(data[offset + 2]); // B
                    rgba.push(255); // A (opaque)
                }
                Some(rgba)
            }
            ImageFormat::Argb => {
                // Convert ARGB to RGBA (Sixel format)
                let mut rgba = Vec::with_capacity(pixel_count * 4);
                for i in 0..pixel_count {
                    let offset = i * 4;
                    rgba.push(data[offset + 1]); // R (from ARGB position 1)
                    rgba.push(data[offset + 2]); // G (from ARGB position 2)
                    rgba.push(data[offset + 3]); // B (from ARGB position 3)
                    rgba.push(data[offset]); // A (from ARGB position 0)
                }
                Some(rgba)
            }
        }
    }

    /// Get image entry by handle.
    #[inline]
    pub fn get(&self, handle: ImageHandle) -> Option<&ImageEntry> {
        self.entries.get(&handle)
    }

    /// Mark an image as recently used (moves to back of LRU).
    pub fn touch(&mut self, handle: ImageHandle) {
        if self.entries.contains_key(&handle) {
            // Remove from current position and add to back
            if let Some(pos) = self.lru_order.iter().position(|h| *h == handle) {
                self.lru_order.remove(pos);
                self.lru_order.push_back(handle);
            }
        }
    }

    /// Remove an image and free memory.
    ///
    /// Note: The caller is responsible for freeing the actual GPU texture.
    pub fn remove(&mut self, handle: ImageHandle) -> bool {
        if let Some(entry) = self.entries.remove(&handle) {
            self.memory_used = self.memory_used.saturating_sub(entry.gpu_size);
            if let Some(pos) = self.lru_order.iter().position(|h| *h == handle) {
                self.lru_order.remove(pos);
            }
            // Remove any placements for this image
            self.placements.retain(|p| p.handle != handle);
            true
        } else {
            false
        }
    }

    /// Place an image at a terminal position.
    pub fn place(&mut self, placement: ImagePlacement) {
        // Remove any existing placement at the same handle/row/col
        self.placements
            .retain(|p| !(p.handle == placement.handle && p.row == placement.row && p.col == placement.col));
        self.placements.push(placement);
    }

    /// Remove all placements for an image.
    pub fn remove_placements(&mut self, handle: ImageHandle) {
        self.placements.retain(|p| p.handle != handle);
    }

    /// Get all placements.
    #[inline]
    pub fn placements(&self) -> &[ImagePlacement] {
        &self.placements
    }

    /// Get placements in a visible row range.
    pub fn visible_placements(&self, top_row: i64, bottom_row: i64) -> Vec<&ImagePlacement> {
        self.placements
            .iter()
            .filter(|p| {
                let placement_bottom = p.row + i64::from(p.height_cells);
                p.row <= bottom_row && placement_bottom > top_row
            })
            .collect()
    }

    /// Clear all images and placements.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.lru_order.clear();
        self.placements.clear();
        self.memory_used = 0;
    }

    /// Get handles that should be evicted to meet the budget.
    ///
    /// Returns handles in LRU order (oldest first).
    pub fn handles_to_evict(&self) -> Vec<ImageHandle> {
        let mut to_evict = Vec::new();
        let mut remaining = self.memory_used;

        for handle in &self.lru_order {
            if remaining <= self.memory_budget {
                break;
            }
            if let Some(entry) = self.entries.get(handle) {
                to_evict.push(*handle);
                remaining = remaining.saturating_sub(entry.gpu_size);
            }
        }

        to_evict
    }

    /// Evict oldest image.
    fn evict_oldest(&mut self) {
        if let Some(handle) = self.lru_order.pop_front() {
            if let Some(entry) = self.entries.remove(&handle) {
                self.memory_used = self.memory_used.saturating_sub(entry.gpu_size);
            }
            self.placements.retain(|p| p.handle != handle);
        }
    }

    /// Evict images until under budget.
    fn evict_to_budget(&mut self) {
        while self.memory_used > self.memory_budget && !self.lru_order.is_empty() {
            self.evict_oldest();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_image_handle_null() {
        assert!(ImageHandle::NULL.is_null());
        assert!(!ImageHandle(1).is_null());
    }

    #[test]
    fn test_image_format_bytes_per_pixel() {
        assert_eq!(ImageFormat::Rgba.bytes_per_pixel(), 4);
        assert_eq!(ImageFormat::Rgb.bytes_per_pixel(), 3);
        assert_eq!(ImageFormat::Argb.bytes_per_pixel(), 4);
    }

    #[test]
    fn test_image_format_from_u8() {
        assert_eq!(ImageFormat::from_u8(0), Some(ImageFormat::Rgba));
        assert_eq!(ImageFormat::from_u8(1), Some(ImageFormat::Rgb));
        assert_eq!(ImageFormat::from_u8(2), Some(ImageFormat::Argb));
        assert_eq!(ImageFormat::from_u8(3), None);
    }

    #[test]
    fn test_cache_allocate_handle() {
        let mut cache = ImageTextureCache::new(1024 * 1024);

        let h1 = cache.allocate_handle(100, 100, ImageFormat::Rgba);
        assert!(!h1.is_null());
        assert_eq!(cache.image_count(), 1);

        let h2 = cache.allocate_handle(200, 200, ImageFormat::Rgba);
        assert!(!h2.is_null());
        assert_ne!(h1, h2);
        assert_eq!(cache.image_count(), 2);
    }

    #[test]
    fn test_cache_invalid_dimensions() {
        let mut cache = ImageTextureCache::new(1024 * 1024);

        // Zero dimensions
        assert!(cache.allocate_handle(0, 100, ImageFormat::Rgba).is_null());
        assert!(cache.allocate_handle(100, 0, ImageFormat::Rgba).is_null());

        // Exceeds max
        assert!(cache
            .allocate_handle(MAX_IMAGE_DIMENSION + 1, 100, ImageFormat::Rgba)
            .is_null());
    }

    #[test]
    fn test_cache_memory_tracking() {
        let mut cache = ImageTextureCache::new(1024 * 1024);

        let h1 = cache.allocate_handle(100, 100, ImageFormat::Rgba);
        assert_eq!(cache.memory_used(), 100 * 100 * 4);

        cache.remove(h1);
        assert_eq!(cache.memory_used(), 0);
    }

    #[test]
    fn test_cache_lru_eviction() {
        // Budget for exactly 2 images of 10x10
        let budget = 10 * 10 * 4 * 2;
        let mut cache = ImageTextureCache::new(budget);

        let h1 = cache.allocate_handle(10, 10, ImageFormat::Rgba);
        let h2 = cache.allocate_handle(10, 10, ImageFormat::Rgba);

        assert_eq!(cache.image_count(), 2);

        // Allocating a third should evict h1 (oldest)
        let _h3 = cache.allocate_handle(10, 10, ImageFormat::Rgba);

        assert_eq!(cache.image_count(), 2);
        assert!(cache.get(h1).is_none());
        assert!(cache.get(h2).is_some());
    }

    #[test]
    fn test_cache_touch_updates_lru() {
        let budget = 10 * 10 * 4 * 2;
        let mut cache = ImageTextureCache::new(budget);

        let h1 = cache.allocate_handle(10, 10, ImageFormat::Rgba);
        let h2 = cache.allocate_handle(10, 10, ImageFormat::Rgba);

        // Touch h1 to make it more recent
        cache.touch(h1);

        // Allocating a third should now evict h2 (oldest after touch)
        let _h3 = cache.allocate_handle(10, 10, ImageFormat::Rgba);

        assert!(cache.get(h1).is_some());
        assert!(cache.get(h2).is_none());
    }

    #[test]
    fn test_placement() {
        let mut cache = ImageTextureCache::new(1024 * 1024);

        let h1 = cache.allocate_handle(100, 100, ImageFormat::Rgba);
        let placement = ImagePlacement::new(h1, 5, 10, 4, 3);

        cache.place(placement);
        assert_eq!(cache.placement_count(), 1);

        cache.remove_placements(h1);
        assert_eq!(cache.placement_count(), 0);
    }

    #[test]
    fn test_visible_placements() {
        let mut cache = ImageTextureCache::new(1024 * 1024);

        let h1 = cache.allocate_handle(100, 100, ImageFormat::Rgba);

        // Placement at rows 5-7 (row=5, height_cells=3)
        cache.place(ImagePlacement::new(h1, 5, 0, 4, 3));

        // Placement at rows 10-12
        cache.place(ImagePlacement::new(h1, 10, 0, 4, 3));

        // Visible range 4-8 should include first placement only
        let visible = cache.visible_placements(4, 8);
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].row, 5);

        // Visible range 0-20 should include both
        let visible = cache.visible_placements(0, 20);
        assert_eq!(visible.len(), 2);
    }

    #[test]
    fn test_scrollback_placement() {
        let mut cache = ImageTextureCache::new(1024 * 1024);

        let h1 = cache.allocate_handle(100, 100, ImageFormat::Rgba);

        // Placement in scrollback (negative row)
        cache.place(ImagePlacement::new(h1, -10, 0, 4, 3));

        let visible = cache.visible_placements(-15, 0);
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].row, -10);
    }

    #[test]
    fn test_convert_rgba() {
        let rgba = [255u8, 0, 0, 255, 0, 255, 0, 255]; // Red, Green pixels
        let result = ImageTextureCache::convert_to_rgba(&rgba, 2, 1, ImageFormat::Rgba);
        assert_eq!(result, Some(rgba.to_vec()));
    }

    #[test]
    fn test_convert_rgb_to_rgba() {
        let rgb = [255u8, 0, 0, 0, 255, 0]; // Red, Green pixels
        let result = ImageTextureCache::convert_to_rgba(&rgb, 2, 1, ImageFormat::Rgb);
        let expected = vec![255, 0, 0, 255, 0, 255, 0, 255];
        assert_eq!(result, Some(expected));
    }

    #[test]
    fn test_convert_argb_to_rgba() {
        // ARGB format: Alpha-Red-Green-Blue
        let argb = [255u8, 255, 0, 0, 128, 0, 255, 0]; // (alpha=255, R) and (alpha=128, G)
        let result = ImageTextureCache::convert_to_rgba(&argb, 2, 1, ImageFormat::Argb);
        // Expected RGBA: R-G-B-A
        let expected = vec![255, 0, 0, 255, 0, 255, 0, 128];
        assert_eq!(result, Some(expected));
    }

    #[test]
    fn test_clear() {
        let mut cache = ImageTextureCache::new(1024 * 1024);

        let h1 = cache.allocate_handle(100, 100, ImageFormat::Rgba);
        cache.place(ImagePlacement::new(h1, 0, 0, 4, 4));

        cache.clear();

        assert_eq!(cache.image_count(), 0);
        assert_eq!(cache.placement_count(), 0);
        assert_eq!(cache.memory_used(), 0);
    }

    #[test]
    fn test_set_memory_budget_evicts() {
        let mut cache = ImageTextureCache::new(1024 * 1024);

        let h1 = cache.allocate_handle(100, 100, ImageFormat::Rgba);
        let h2 = cache.allocate_handle(100, 100, ImageFormat::Rgba);

        assert_eq!(cache.image_count(), 2);

        // Reduce budget to fit only one image
        cache.set_memory_budget(100 * 100 * 4);

        assert_eq!(cache.image_count(), 1);
        assert!(cache.get(h1).is_none()); // h1 was oldest
        assert!(cache.get(h2).is_some());
    }
}

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// Prove that handles are always unique and never reused.
    #[kani::proof]
    fn image_handle_unique() {
        let mut cache = ImageTextureCache::new(1024 * 1024 * 64);

        let h1 = cache.allocate_handle(10, 10, ImageFormat::Rgba);
        let h2 = cache.allocate_handle(10, 10, ImageFormat::Rgba);
        let h3 = cache.allocate_handle(10, 10, ImageFormat::Rgba);

        // Handles should all be different (assuming none are null)
        if !h1.is_null() && !h2.is_null() {
            assert!(h1 != h2);
        }
        if !h2.is_null() && !h3.is_null() {
            assert!(h2 != h3);
        }
        if !h1.is_null() && !h3.is_null() {
            assert!(h1 != h3);
        }
    }

    /// Prove that memory budget is respected after eviction.
    #[kani::proof]
    fn memory_budget_respected() {
        let budget: usize = kani::any();
        kani::assume(budget > 0 && budget <= 1024 * 1024); // Reasonable budget

        let mut cache = ImageTextureCache::new(budget);

        // Try to allocate an image
        let width: u32 = kani::any();
        let height: u32 = kani::any();
        kani::assume(width > 0 && width <= 100);
        kani::assume(height > 0 && height <= 100);

        let _h = cache.allocate_handle(width, height, ImageFormat::Rgba);

        // After any operation, memory used should not exceed budget
        assert!(cache.memory_used() <= cache.memory_budget());
    }

    /// Prove that placement bounds are valid.
    #[kani::proof]
    fn placement_bounds_valid() {
        let row: i64 = kani::any();
        let col: u16 = kani::any();
        let width_cells: u16 = kani::any();
        let height_cells: u16 = kani::any();

        // Ensure reasonable bounds to avoid overflow
        kani::assume(row >= i64::MIN / 2 && row <= i64::MAX / 2);
        kani::assume(height_cells <= 1000);

        let handle = ImageHandle(1);
        let placement = ImagePlacement::new(handle, row, col, width_cells, height_cells);

        // Verify placement fields match construction
        assert_eq!(placement.handle, handle);
        assert_eq!(placement.row, row);
        assert_eq!(placement.col, col);
        assert_eq!(placement.width_cells, width_cells);
        assert_eq!(placement.height_cells, height_cells);
        assert_eq!(placement.z_index, 0);

        // Verify bottom row calculation doesn't overflow for reasonable values
        let _bottom = placement.row + i64::from(placement.height_cells);
    }
}
