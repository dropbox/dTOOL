//! Kitty graphics image storage and placement management.
//!
//! Manages transmitted images and their placements on the terminal screen.

use std::collections::HashMap;
use std::sync::Arc;

use smallvec::SmallVec;

use super::command::{
    AnimationState, CompositionMode, DeleteAction, ImageFormat, KittyGraphicsCommand,
};
use super::{DEFAULT_STORAGE_QUOTA, KITTY_MAX_DIMENSION, MAX_IMAGES};

/// Error type for Kitty graphics operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KittyGraphicsError {
    /// Invalid image ID (not found).
    ImageNotFound(u32),
    /// Invalid placement ID (not found).
    PlacementNotFound(u32, u32),
    /// Image dimensions exceed maximum.
    DimensionsTooLarge {
        /// Requested width.
        width: u32,
        /// Requested height.
        height: u32,
        /// Maximum allowed width.
        max_width: u32,
        /// Maximum allowed height.
        max_height: u32,
    },
    /// Storage quota exceeded.
    StorageQuotaExceeded,
    /// Too many images stored.
    TooManyImages,
    /// Invalid image data.
    InvalidImageData(String),
    /// Base64 decode error.
    Base64DecodeError,
    /// Compression/decompression error.
    CompressionError(String),
    /// File access error.
    FileAccessError(String),
    /// Parent placement not found (for relative placement).
    ParentNotFound(u32, u32),
    /// Placement chain too deep.
    PlacementChainTooDeep,
    /// Chunked transmission in progress.
    ChunkedTransmissionInProgress,
    /// No chunked transmission in progress.
    NoChunkedTransmission,
    /// Animation frame not found.
    FrameNotFound(u32),
    /// Too many animation frames.
    TooManyFrames,
}

impl std::fmt::Display for KittyGraphicsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ImageNotFound(id) => write!(f, "ENOENT:image {} not found", id),
            Self::PlacementNotFound(img, p) => {
                write!(f, "ENOENT:placement {}/{} not found", img, p)
            }
            Self::DimensionsTooLarge {
                width,
                height,
                max_width,
                max_height,
            } => {
                write!(
                    f,
                    "EINVAL:dimensions {}x{} exceed maximum {}x{}",
                    width, height, max_width, max_height
                )
            }
            Self::StorageQuotaExceeded => write!(f, "ENOMEM:storage quota exceeded"),
            Self::TooManyImages => write!(f, "ENOMEM:too many images"),
            Self::InvalidImageData(msg) => write!(f, "EINVAL:{}", msg),
            Self::Base64DecodeError => write!(f, "EINVAL:invalid base64 data"),
            Self::CompressionError(msg) => write!(f, "EINVAL:compression error: {}", msg),
            Self::FileAccessError(msg) => write!(f, "ENOENT:{}", msg),
            Self::ParentNotFound(img, p) => write!(f, "ENOENT:parent {}/{} not found", img, p),
            Self::PlacementChainTooDeep => write!(f, "EINVAL:placement chain too deep"),
            Self::ChunkedTransmissionInProgress => {
                write!(f, "EINVAL:chunked transmission in progress")
            }
            Self::NoChunkedTransmission => write!(f, "EINVAL:no chunked transmission in progress"),
            Self::FrameNotFound(frame) => write!(f, "ENOENT:frame {} not found", frame),
            Self::TooManyFrames => write!(f, "ENOMEM:too many animation frames"),
        }
    }
}

impl std::error::Error for KittyGraphicsError {}

/// Location of a placement on the screen.
#[derive(Debug, Clone)]
pub enum PlacementLocation {
    /// Placed at absolute cursor position.
    Absolute {
        /// Row position.
        row: u32,
        /// Column position.
        col: u32,
    },
    /// Virtual placement (for Unicode placeholder mode).
    Virtual {
        /// Virtual placement ID.
        id: u32,
    },
    /// Relative to another placement.
    Relative {
        /// Parent image ID.
        parent_image_id: u32,
        /// Parent placement ID.
        parent_placement_id: u32,
        /// Horizontal offset from parent in cells.
        offset_x: i32,
        /// Vertical offset from parent in cells.
        offset_y: i32,
    },
}

/// A placement of an image on the screen.
#[derive(Debug, Clone)]
pub struct KittyPlacement {
    /// Unique placement ID within the image.
    pub id: u32,
    /// Location of the placement.
    pub location: PlacementLocation,
    /// Source rectangle x offset (in pixels).
    pub source_x: u32,
    /// Source rectangle y offset (in pixels).
    pub source_y: u32,
    /// Source rectangle width (0 = full image).
    pub source_width: u32,
    /// Source rectangle height (0 = full image).
    pub source_height: u32,
    /// Pixel offset within starting cell, x.
    pub cell_x_offset: u16,
    /// Pixel offset within starting cell, y.
    pub cell_y_offset: u16,
    /// Number of columns to display (0 = auto).
    pub num_columns: u16,
    /// Number of rows to display (0 = auto).
    pub num_rows: u16,
    /// Z-index for stacking (negative = below text).
    pub z_index: i32,
    /// Whether this is a virtual placement.
    pub is_virtual: bool,
}

impl KittyPlacement {
    /// Create a new placement at the given screen position.
    pub fn new(id: u32, row: u32, col: u32) -> Self {
        Self {
            id,
            location: PlacementLocation::Absolute { row, col },
            source_x: 0,
            source_y: 0,
            source_width: 0,
            source_height: 0,
            cell_x_offset: 0,
            cell_y_offset: 0,
            num_columns: 0,
            num_rows: 0,
            z_index: 0,
            is_virtual: false,
        }
    }

    /// Create a placement from a command.
    pub fn from_command(cmd: &KittyGraphicsCommand, row: u32, col: u32) -> Self {
        let location = if cmd.unicode_placement {
            PlacementLocation::Virtual {
                id: cmd.placement_id,
            }
        } else if cmd.parent_id != 0 {
            PlacementLocation::Relative {
                parent_image_id: cmd.parent_id,
                parent_placement_id: cmd.parent_placement_id,
                offset_x: cmd.offset_from_parent_x,
                offset_y: cmd.offset_from_parent_y,
            }
        } else {
            PlacementLocation::Absolute { row, col }
        };

        Self {
            id: cmd.placement_id,
            location,
            source_x: cmd.source_x,
            source_y: cmd.source_y,
            source_width: cmd.source_width,
            source_height: cmd.source_height,
            cell_x_offset: Self::clamp_u16(cmd.cell_x_offset),
            cell_y_offset: Self::clamp_u16(cmd.cell_y_offset),
            num_columns: Self::clamp_u16(cmd.num_columns),
            num_rows: Self::clamp_u16(cmd.num_rows),
            z_index: cmd.z_index,
            is_virtual: cmd.unicode_placement,
        }
    }

    /// Returns the absolute position if this is an absolute placement.
    pub fn absolute_position(&self) -> Option<(u32, u32)> {
        if let PlacementLocation::Absolute { row, col } = &self.location {
            Some((*row, *col))
        } else {
            None
        }
    }

    fn clamp_u16(value: u32) -> u16 {
        u16::try_from(value).unwrap_or(u16::MAX)
    }
}

/// Maximum number of animation frames per image.
pub const MAX_FRAMES_PER_IMAGE: usize = 1000;

/// An animation frame within an image.
#[derive(Debug, Clone)]
pub struct AnimationFrame {
    /// Frame number (1-based, 0 means root frame).
    pub number: u32,
    /// Frame pixel data (RGBA, same dimensions as root frame).
    pub data: Arc<[u8]>,
    /// Width (can differ from root for delta frames).
    pub width: u32,
    /// Height (can differ from root for delta frames).
    pub height: u32,
    /// X offset within the image for this frame's data.
    pub x_offset: u32,
    /// Y offset within the image for this frame's data.
    pub y_offset: u32,
    /// Gap (delay) in milliseconds before showing next frame.
    /// Negative means gapless (instantly skip to next).
    pub gap: i32,
    /// Base frame number for delta composition (0 = root frame).
    pub base_frame: u32,
    /// Background color for composition (RGBA).
    pub background_color: u32,
    /// Whether to alpha-blend or overwrite when compositing.
    pub composition_mode: CompositionMode,
}

impl AnimationFrame {
    /// Create a new animation frame.
    pub fn new(number: u32, data: Vec<u8>, width: u32, height: u32) -> Self {
        Self {
            number,
            data: Arc::from(data),
            width,
            height,
            x_offset: 0,
            y_offset: 0,
            gap: 0,
            base_frame: 0,
            background_color: 0,
            composition_mode: CompositionMode::AlphaBlend,
        }
    }

    /// Returns the size in bytes of the frame data.
    pub fn data_size(&self) -> usize {
        self.data.len()
    }
}

/// A stored image.
#[derive(Debug, Clone)]
pub struct KittyImage {
    /// Image ID (assigned by terminal or client).
    pub id: u32,
    /// Image number (if assigned).
    pub number: Option<u32>,
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
    /// Image format.
    pub format: ImageFormat,
    /// Decoded pixel data (RGBA format, 4 bytes per pixel) - the "root" frame.
    pub data: Arc<[u8]>,
    /// Placements of this image.
    ///
    /// Uses SmallVec to avoid HashMap overhead for typical images with 1-3 placements.
    /// Linear search is faster than HashMap for small collections due to cache locality.
    placements: SmallVec<[KittyPlacement; 2]>,
    /// Next placement ID to assign.
    next_placement_id: u32,
    /// Extra animation frames (frame numbers 1+).
    pub frames: Vec<AnimationFrame>,
    /// Current animation state.
    pub animation_state: AnimationState,
    /// Current frame index being displayed (into frames vec, or 0 for root).
    pub current_frame_index: usize,
    /// Maximum loops (0 = not set, 1 = infinite, >1 = loop n-1 times).
    pub max_loops: u32,
    /// Current loop count.
    pub current_loop: u32,
    /// Next frame ID to assign.
    next_frame_number: u32,
}

impl KittyImage {
    /// Create a new image with the given data.
    pub fn new(id: u32, width: u32, height: u32, data: Vec<u8>) -> Self {
        Self {
            id,
            number: None,
            width,
            height,
            format: ImageFormat::Rgba32,
            data: Arc::from(data),
            placements: SmallVec::new(),
            next_placement_id: 1,
            frames: Vec::new(),
            animation_state: AnimationState::Stopped,
            current_frame_index: 0,
            max_loops: 0,
            current_loop: 0,
            next_frame_number: 1,
        }
    }

    /// Returns the size in bytes of the image data (including all frames).
    pub fn data_size(&self) -> usize {
        let frames_size: usize = self.frames.iter().map(|f| f.data_size()).sum();
        self.data.len() + frames_size
    }

    /// Add a placement to this image.
    pub fn add_placement(&mut self, placement: KittyPlacement) -> u32 {
        let id = if placement.id == 0 {
            let id = self.next_placement_id;
            self.next_placement_id = self.next_placement_id.wrapping_add(1);
            id
        } else {
            placement.id
        };

        let mut placement = placement;
        placement.id = id;

        // Check if placement with this ID already exists (replace it)
        if let Some(existing) = self.placements.iter_mut().find(|p| p.id == id) {
            *existing = placement;
        } else {
            self.placements.push(placement);
        }
        id
    }

    /// Remove a placement by ID.
    pub fn remove_placement(&mut self, placement_id: u32) -> Option<KittyPlacement> {
        if let Some(idx) = self.placements.iter().position(|p| p.id == placement_id) {
            Some(self.placements.remove(idx))
        } else {
            None
        }
    }

    /// Get a placement by ID.
    pub fn get_placement(&self, placement_id: u32) -> Option<&KittyPlacement> {
        self.placements.iter().find(|p| p.id == placement_id)
    }

    /// Returns true if this image has any placements.
    pub fn has_placements(&self) -> bool {
        !self.placements.is_empty()
    }

    /// Clear all placements.
    pub fn clear_placements(&mut self) {
        self.placements.clear();
    }

    /// Iterate over all placements.
    pub fn iter_placements(&self) -> impl Iterator<Item = &KittyPlacement> {
        self.placements.iter()
    }

    /// Get the number of placements.
    pub fn placement_count(&self) -> usize {
        self.placements.len()
    }

    // === Animation Methods ===

    /// Add an animation frame to this image.
    ///
    /// Returns the assigned frame number.
    pub fn add_frame(&mut self, mut frame: AnimationFrame) -> Result<u32, KittyGraphicsError> {
        if self.frames.len() >= MAX_FRAMES_PER_IMAGE {
            return Err(KittyGraphicsError::TooManyFrames);
        }

        // Assign frame number if not set
        if frame.number == 0 {
            frame.number = self.next_frame_number;
            self.next_frame_number = self.next_frame_number.wrapping_add(1);
            if self.next_frame_number == 0 {
                self.next_frame_number = 1;
            }
        }

        let number = frame.number;
        self.frames.push(frame);
        Ok(number)
    }

    /// Get a frame by number.
    ///
    /// Frame number 0 refers to the root frame (returns None, use self.data directly).
    pub fn get_frame(&self, number: u32) -> Option<&AnimationFrame> {
        if number == 0 {
            None // Root frame is self.data
        } else {
            self.frames.iter().find(|f| f.number == number)
        }
    }

    /// Get a mutable frame by number.
    pub fn get_frame_mut(&mut self, number: u32) -> Option<&mut AnimationFrame> {
        if number == 0 {
            None // Root frame is self.data
        } else {
            self.frames.iter_mut().find(|f| f.number == number)
        }
    }

    /// Remove a frame by number.
    pub fn remove_frame(&mut self, number: u32) -> Option<AnimationFrame> {
        if let Some(idx) = self.frames.iter().position(|f| f.number == number) {
            Some(self.frames.remove(idx))
        } else {
            None
        }
    }

    /// Clear all animation frames.
    pub fn clear_frames(&mut self) {
        self.frames.clear();
        self.current_frame_index = 0;
        self.current_loop = 0;
        self.animation_state = AnimationState::Stopped;
    }

    /// Returns the number of animation frames (not including root).
    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }

    /// Returns true if this image has animation frames.
    pub fn is_animated(&self) -> bool {
        !self.frames.is_empty()
    }

    /// Get the current frame's data for display.
    ///
    /// Returns the root frame data if no animation or at frame 0.
    pub fn current_frame_data(&self) -> &[u8] {
        if self.current_frame_index == 0 || self.frames.is_empty() {
            &self.data
        } else if let Some(frame) = self.frames.get(self.current_frame_index.saturating_sub(1)) {
            &frame.data
        } else {
            &self.data
        }
    }

    /// Get the current frame's gap (delay) in milliseconds.
    ///
    /// Returns 0 if no animation or at root frame.
    pub fn current_frame_gap(&self) -> i32 {
        if self.current_frame_index == 0 || self.frames.is_empty() {
            0
        } else if let Some(frame) = self.frames.get(self.current_frame_index.saturating_sub(1)) {
            frame.gap
        } else {
            0
        }
    }

    /// Advance to the next frame in the animation.
    ///
    /// Returns true if advanced, false if animation ended.
    pub fn advance_frame(&mut self) -> bool {
        if self.frames.is_empty() || self.animation_state != AnimationState::Running {
            return false;
        }

        self.current_frame_index += 1;

        // Check if we've reached the end (past all frames including root at 0)
        if self.current_frame_index > self.frames.len() {
            // End of frames, check loop count
            if self.max_loops == 1 {
                // Infinite loop
                self.current_frame_index = 0;
                return true;
            } else if self.max_loops > 1 {
                // max_loops of N means play N-1 times after the first play
                // So we loop if current_loop < max_loops - 1
                if self.current_loop < self.max_loops - 1 {
                    self.current_loop += 1;
                    self.current_frame_index = 0;
                    return true;
                }
            }
            // Animation ended (either max_loops == 0 or we've completed all loops)
            self.current_frame_index = self.frames.len(); // Stay at last frame
            self.animation_state = AnimationState::Stopped;
            return false;
        }

        true
    }

    /// Reset animation to first frame.
    pub fn reset_animation(&mut self) {
        self.current_frame_index = 0;
        self.current_loop = 0;
    }

    /// Compose source frame onto destination frame.
    ///
    /// Per the Kitty graphics protocol, this composites the source frame's pixels
    /// onto the destination frame using the specified composition mode.
    ///
    /// # Arguments
    /// * `source_frame` - Frame number to read from (0 = root frame)
    /// * `dest_frame` - Frame number to write to (0 = root frame)
    /// * `mode` - Composition mode (alpha blend or overwrite)
    /// * `background_color` - RGBA background color for composition
    ///
    /// # Returns
    /// * `Ok(())` on success
    /// * `Err(FrameNotFound)` if source or destination frame doesn't exist
    pub fn compose_frames(
        &mut self,
        source_frame: u32,
        dest_frame: u32,
        mode: CompositionMode,
        background_color: u32,
    ) -> Result<(), KittyGraphicsError> {
        // Get source frame data
        let (src_data, src_width, src_height, src_x_offset, src_y_offset) = if source_frame == 0 {
            // Root frame
            (
                self.data.as_ref(),
                self.width,
                self.height,
                0u32,
                0u32,
            )
        } else {
            let frame = self
                .get_frame(source_frame)
                .ok_or(KittyGraphicsError::FrameNotFound(source_frame))?;
            (
                frame.data.as_ref(),
                frame.width,
                frame.height,
                frame.x_offset,
                frame.y_offset,
            )
        };

        // Clone source data so we can mutably borrow for destination
        let src_data = src_data.to_vec();

        // Get destination frame data (mutable)
        let (dest_data, dest_width, dest_height) = if dest_frame == 0 {
            // Root frame - we need to clone and replace
            let data = Arc::make_mut(&mut self.data).to_vec();
            (data, self.width, self.height)
        } else {
            // Verify destination frame exists
            if self.get_frame(dest_frame).is_none() {
                return Err(KittyGraphicsError::FrameNotFound(dest_frame));
            }
            // Get frame data - we'll update it later
            let frame = self.get_frame(dest_frame).unwrap();
            (frame.data.to_vec(), frame.width, frame.height)
        };

        // Perform composition
        let mut result = dest_data;
        compose_rgba(
            &src_data,
            src_width,
            src_height,
            src_x_offset,
            src_y_offset,
            &mut result,
            dest_width,
            dest_height,
            mode,
            background_color,
        );

        // Update destination frame
        if dest_frame == 0 {
            self.data = Arc::from(result);
        } else {
            // Find and update the frame
            if let Some(frame) = self.frames.iter_mut().find(|f| f.number == dest_frame) {
                frame.data = Arc::from(result);
            }
        }

        Ok(())
    }
}

/// In-progress chunked image transmission.
#[derive(Debug)]
pub struct LoadingImage {
    /// Command from the first chunk.
    pub command: KittyGraphicsCommand,
    /// Accumulated base64 data.
    pub data: Vec<u8>,
}

impl LoadingImage {
    /// Create a new loading image from the first chunk.
    ///
    /// Pre-allocates buffer based on expected image size to reduce reallocations.
    pub fn new(command: KittyGraphicsCommand) -> Self {
        // Estimate expected data size for pre-allocation
        let estimated_size = Self::estimate_size(&command);
        Self {
            command,
            data: Vec::with_capacity(estimated_size),
        }
    }

    /// Estimate the expected data size based on command parameters.
    ///
    /// Returns a reasonable capacity hint to reduce reallocations during chunked transfer.
    fn estimate_size(command: &KittyGraphicsCommand) -> usize {
        // If data_size is specified (file transmission), use that
        if command.data_size > 0 {
            return command.data_size as usize;
        }

        // Otherwise estimate from dimensions
        if command.data_width > 0 && command.data_height > 0 {
            // Bytes per pixel based on format (default to RGBA = 4)
            let bytes_per_pixel = match command.format {
                super::command::ImageFormat::Rgb24 => 3,
                super::command::ImageFormat::Rgba32 => 4,
                super::command::ImageFormat::Png => 4, // Compressed, but estimate uncompressed
            };
            // Add ~35% for base64 encoding overhead
            let raw_size =
                command.data_width as usize * command.data_height as usize * bytes_per_pixel;
            return raw_size * 4 / 3 + 4; // Base64 expansion + padding
        }

        // Default: start with reasonable buffer for small images
        4096
    }

    /// Append data to the accumulator.
    pub fn append(&mut self, data: &[u8]) {
        self.data.extend_from_slice(data);
    }
}

/// Storage for Kitty graphics images and placements.
#[derive(Debug)]
pub struct KittyImageStorage {
    /// Stored images by ID.
    images: HashMap<u32, KittyImage>,
    /// Image number to ID mapping.
    number_to_id: HashMap<u32, u32>,
    /// Next image ID to assign.
    next_image_id: u32,
    /// Total bytes used by stored images.
    total_bytes: usize,
    /// Storage quota in bytes.
    quota: usize,
    /// In-progress chunked transmission.
    loading: Option<LoadingImage>,
    /// Dirty flag for rendering.
    dirty: bool,
}

impl Default for KittyImageStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl KittyImageStorage {
    /// Create a new image storage with default quota.
    pub fn new() -> Self {
        Self::with_quota(DEFAULT_STORAGE_QUOTA)
    }

    /// Create a new image storage with the given quota.
    pub fn with_quota(quota: usize) -> Self {
        Self {
            images: HashMap::new(),
            number_to_id: HashMap::new(),
            next_image_id: 1,
            total_bytes: 0,
            quota,
            loading: None,
            dirty: false,
        }
    }

    /// Returns true if the storage has been modified since last render.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Clear the dirty flag.
    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }

    /// Mark the storage as dirty.
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Returns the number of stored images.
    pub fn image_count(&self) -> usize {
        self.images.len()
    }

    /// Returns the total bytes used by stored images.
    pub fn total_bytes(&self) -> usize {
        self.total_bytes
    }

    /// Returns the storage quota.
    pub fn quota(&self) -> usize {
        self.quota
    }

    /// Set the storage quota.
    pub fn set_quota(&mut self, quota: usize) {
        self.quota = quota;
    }

    /// Returns true if a chunked transmission is in progress.
    pub fn is_loading(&self) -> bool {
        self.loading.is_some()
    }

    /// Get an image by ID.
    pub fn get_image(&self, id: u32) -> Option<&KittyImage> {
        self.images.get(&id)
    }

    /// Get a mutable image by ID.
    pub fn get_image_mut(&mut self, id: u32) -> Option<&mut KittyImage> {
        self.images.get_mut(&id)
    }

    /// Get an image by number.
    pub fn get_image_by_number(&self, number: u32) -> Option<&KittyImage> {
        self.number_to_id
            .get(&number)
            .and_then(|id| self.images.get(id))
    }

    /// Store an image.
    ///
    /// Returns the assigned image ID.
    pub fn store_image(&mut self, mut image: KittyImage) -> Result<u32, KittyGraphicsError> {
        // Check dimension limits
        if image.width > KITTY_MAX_DIMENSION || image.height > KITTY_MAX_DIMENSION {
            return Err(KittyGraphicsError::DimensionsTooLarge {
                width: image.width,
                height: image.height,
                max_width: KITTY_MAX_DIMENSION,
                max_height: KITTY_MAX_DIMENSION,
            });
        }

        // Check image count limit
        if self.images.len() >= MAX_IMAGES {
            // Try to evict unreferenced images
            self.evict_unreferenced();
            if self.images.len() >= MAX_IMAGES {
                return Err(KittyGraphicsError::TooManyImages);
            }
        }

        // Check storage quota
        let data_size = image.data_size();
        if self.total_bytes + data_size > self.quota {
            // Try to evict unreferenced images
            self.evict_unreferenced();
            if self.total_bytes + data_size > self.quota {
                return Err(KittyGraphicsError::StorageQuotaExceeded);
            }
        }

        // Assign ID if needed
        let id = if image.id == 0 {
            let id = self.next_image_id;
            self.next_image_id = self.next_image_id.wrapping_add(1);
            if self.next_image_id == 0 {
                self.next_image_id = 1;
            }
            image.id = id;
            id
        } else {
            // Remove existing image with same ID
            if let Some(old) = self.images.remove(&image.id) {
                self.total_bytes = self.total_bytes.saturating_sub(old.data_size());
                if let Some(num) = old.number {
                    self.number_to_id.remove(&num);
                }
            }
            image.id
        };

        // Assign number if requested
        if image.number.is_some() {
            let num = image.number.unwrap();
            self.number_to_id.insert(num, id);
        }

        self.total_bytes += data_size;
        self.images.insert(id, image);
        self.dirty = true;

        Ok(id)
    }

    /// Remove an image by ID.
    pub fn remove_image(&mut self, id: u32) -> Option<KittyImage> {
        if let Some(image) = self.images.remove(&id) {
            self.total_bytes = self.total_bytes.saturating_sub(image.data_size());
            if let Some(num) = image.number {
                self.number_to_id.remove(&num);
            }
            self.dirty = true;
            Some(image)
        } else {
            None
        }
    }

    /// Remove all images without placements.
    pub fn evict_unreferenced(&mut self) {
        let ids_to_remove: Vec<u32> = self
            .images
            .iter()
            .filter(|(_, img)| !img.has_placements())
            .map(|(id, _)| *id)
            .collect();

        for id in ids_to_remove {
            self.remove_image(id);
        }
    }

    /// Start a chunked transmission.
    pub fn start_chunked(
        &mut self,
        command: KittyGraphicsCommand,
    ) -> Result<(), KittyGraphicsError> {
        if self.loading.is_some() {
            return Err(KittyGraphicsError::ChunkedTransmissionInProgress);
        }
        self.loading = Some(LoadingImage::new(command));
        Ok(())
    }

    /// Continue a chunked transmission.
    pub fn continue_chunked(
        &mut self,
        data: &[u8],
        is_final: bool,
    ) -> Result<Option<LoadingImage>, KittyGraphicsError> {
        let loading = self
            .loading
            .as_mut()
            .ok_or(KittyGraphicsError::NoChunkedTransmission)?;
        loading.append(data);

        if is_final {
            Ok(self.loading.take())
        } else {
            Ok(None)
        }
    }

    /// Cancel any in-progress chunked transmission.
    pub fn cancel_chunked(&mut self) {
        self.loading = None;
    }

    /// Add a placement to an image.
    pub fn add_placement(
        &mut self,
        image_id: u32,
        placement: KittyPlacement,
    ) -> Result<u32, KittyGraphicsError> {
        let image = self
            .images
            .get_mut(&image_id)
            .ok_or(KittyGraphicsError::ImageNotFound(image_id))?;
        let id = image.add_placement(placement);
        self.dirty = true;
        Ok(id)
    }

    /// Remove a placement.
    pub fn remove_placement(
        &mut self,
        image_id: u32,
        placement_id: u32,
    ) -> Result<KittyPlacement, KittyGraphicsError> {
        let image = self
            .images
            .get_mut(&image_id)
            .ok_or(KittyGraphicsError::ImageNotFound(image_id))?;
        image
            .remove_placement(placement_id)
            .ok_or(KittyGraphicsError::PlacementNotFound(
                image_id,
                placement_id,
            ))
            .inspect(|_| {
                self.dirty = true;
            })
    }

    /// Delete images/placements based on a delete command.
    pub fn delete(&mut self, cmd: &KittyGraphicsCommand) {
        match cmd.delete_action {
            DeleteAction::AllVisiblePlacements | DeleteAction::AllVisiblePlacementsAndData => {
                // Clear all placements (optionally free data)
                if cmd.delete_action.frees_data() {
                    self.images.clear();
                    self.number_to_id.clear();
                    self.total_bytes = 0;
                } else {
                    for image in self.images.values_mut() {
                        image.clear_placements();
                    }
                }
                self.dirty = true;
            }

            DeleteAction::ById | DeleteAction::ByIdAndData => {
                if cmd.delete_action.frees_data() {
                    self.remove_image(cmd.image_id);
                } else if let Some(image) = self.images.get_mut(&cmd.image_id) {
                    image.clear_placements();
                    self.dirty = true;
                }
            }

            DeleteAction::ByNumber | DeleteAction::ByNumberAndData => {
                if let Some(&id) = self.number_to_id.get(&cmd.image_number) {
                    if cmd.delete_action.frees_data() {
                        self.remove_image(id);
                    } else if let Some(image) = self.images.get_mut(&id) {
                        image.clear_placements();
                        self.dirty = true;
                    }
                }
            }

            DeleteAction::AtCursor | DeleteAction::AtCursorAndData => {
                // Delete placements at cursor position
                // This requires cursor position to be passed in - handled by terminal
                self.dirty = true;
            }

            DeleteAction::ByIdRange | DeleteAction::ByIdRangeAndData => {
                // x and y params define the range
                let start = cmd.source_x;
                let end = cmd.source_y;
                let ids_to_delete: Vec<u32> = self
                    .images
                    .keys()
                    .filter(|&&id| id >= start && id <= end)
                    .copied()
                    .collect();

                for id in ids_to_delete {
                    if cmd.delete_action.frees_data() {
                        self.remove_image(id);
                    } else if let Some(image) = self.images.get_mut(&id) {
                        image.clear_placements();
                    }
                }
                self.dirty = true;
            }

            DeleteAction::ByZIndex | DeleteAction::ByZIndexAndData => {
                // Delete placements at specific z-index
                for image in self.images.values_mut() {
                    let z = cmd.z_index;
                    let ids_to_remove: Vec<u32> = image
                        .iter_placements()
                        .filter(|p| p.z_index == z)
                        .map(|p| p.id)
                        .collect();
                    for id in ids_to_remove {
                        image.remove_placement(id);
                    }
                }
                if cmd.delete_action.frees_data() {
                    self.evict_unreferenced();
                }
                self.dirty = true;
            }

            // Other delete actions require screen position info
            _ => {
                self.dirty = true;
            }
        }
    }

    /// Delete placements at a specific screen position.
    pub fn delete_at_position(&mut self, row: u32, col: u32, free_data: bool) {
        for image in self.images.values_mut() {
            let ids_to_remove: Vec<u32> = image
                .iter_placements()
                .filter(|p| {
                    if let Some((pr, pc)) = p.absolute_position() {
                        pr == row && pc == col
                    } else {
                        false
                    }
                })
                .map(|p| p.id)
                .collect();
            for id in ids_to_remove {
                image.remove_placement(id);
            }
        }
        if free_data {
            self.evict_unreferenced();
        }
        self.dirty = true;
    }

    /// Delete placements in a specific row.
    pub fn delete_in_row(&mut self, row: u32, free_data: bool) {
        for image in self.images.values_mut() {
            let ids_to_remove: Vec<u32> = image
                .iter_placements()
                .filter(|p| {
                    if let Some((pr, _)) = p.absolute_position() {
                        pr == row
                    } else {
                        false
                    }
                })
                .map(|p| p.id)
                .collect();
            for id in ids_to_remove {
                image.remove_placement(id);
            }
        }
        if free_data {
            self.evict_unreferenced();
        }
        self.dirty = true;
    }

    /// Delete placements in a specific column.
    pub fn delete_in_column(&mut self, col: u32, free_data: bool) {
        for image in self.images.values_mut() {
            let ids_to_remove: Vec<u32> = image
                .iter_placements()
                .filter(|p| {
                    if let Some((_, pc)) = p.absolute_position() {
                        pc == col
                    } else {
                        false
                    }
                })
                .map(|p| p.id)
                .collect();
            for id in ids_to_remove {
                image.remove_placement(id);
            }
        }
        if free_data {
            self.evict_unreferenced();
        }
        self.dirty = true;
    }

    /// Clear all images and placements.
    pub fn clear(&mut self) {
        self.images.clear();
        self.number_to_id.clear();
        self.total_bytes = 0;
        self.loading = None;
        self.dirty = true;
    }

    /// Iterate over all images.
    pub fn iter_images(&self) -> impl Iterator<Item = &KittyImage> {
        self.images.values()
    }

    /// Get all image IDs.
    pub fn image_ids(&self) -> Vec<u32> {
        self.images.keys().copied().collect()
    }

    /// Iterate over all placements across all images.
    pub fn iter_placements(&self) -> impl Iterator<Item = (u32, &KittyPlacement)> {
        self.images
            .iter()
            .flat_map(|(img_id, img)| img.iter_placements().map(move |p| (*img_id, p)))
    }

    /// Get placements sorted by z-index for rendering.
    pub fn placements_by_z_index(&self) -> Vec<(u32, &KittyImage, &KittyPlacement)> {
        let mut placements: Vec<_> = self
            .images
            .iter()
            .flat_map(|(_, img)| img.iter_placements().map(move |p| (img.id, img, p)))
            .collect();
        placements.sort_by_key(|(_, _, p)| p.z_index);
        placements
    }

    // === Animation Methods ===

    /// Add an animation frame to an image.
    ///
    /// Returns the assigned frame number.
    pub fn add_frame(
        &mut self,
        image_id: u32,
        frame: AnimationFrame,
    ) -> Result<u32, KittyGraphicsError> {
        let image = self
            .images
            .get_mut(&image_id)
            .ok_or(KittyGraphicsError::ImageNotFound(image_id))?;

        // Track data size increase
        let frame_size = frame.data_size();
        if self.total_bytes + frame_size > self.quota {
            return Err(KittyGraphicsError::StorageQuotaExceeded);
        }

        let number = image.add_frame(frame)?;
        self.total_bytes += frame_size;
        self.dirty = true;
        Ok(number)
    }

    /// Control animation playback on an image.
    pub fn control_animation(
        &mut self,
        image_id: u32,
        state: AnimationState,
        loop_count: Option<u32>,
    ) -> Result<(), KittyGraphicsError> {
        let image = self
            .images
            .get_mut(&image_id)
            .ok_or(KittyGraphicsError::ImageNotFound(image_id))?;

        image.animation_state = state;
        if let Some(loops) = loop_count {
            if loops > 0 {
                image.max_loops = loops;
            }
        }

        // If starting, reset to first frame
        if state == AnimationState::Running {
            image.reset_animation();
        }

        self.dirty = true;
        Ok(())
    }

    /// Delete animation frames from an image.
    pub fn delete_frames(&mut self, image_id: u32, free_data: bool) {
        if let Some(image) = self.images.get_mut(&image_id) {
            let frame_size: usize = image.frames.iter().map(|f| f.data_size()).sum();
            image.clear_frames();
            self.total_bytes = self.total_bytes.saturating_sub(frame_size);
            self.dirty = true;

            if free_data && !image.has_placements() {
                self.remove_image(image_id);
            }
        }
    }

    /// Get all images that have active animations.
    pub fn animated_images(&self) -> Vec<u32> {
        self.images
            .iter()
            .filter(|(_, img)| img.animation_state == AnimationState::Running)
            .map(|(id, _)| *id)
            .collect()
    }

    /// Advance all running animations by one frame.
    ///
    /// Returns the IDs of images that advanced.
    pub fn advance_animations(&mut self) -> Vec<u32> {
        let mut advanced = Vec::new();
        for (id, image) in self.images.iter_mut() {
            if image.animation_state == AnimationState::Running && image.advance_frame() {
                advanced.push(*id);
            }
        }
        if !advanced.is_empty() {
            self.dirty = true;
        }
        advanced
    }
}

/// Decode base64 data.
#[allow(dead_code)] // Will be used by Terminal integration
pub fn decode_base64(data: &[u8]) -> Result<Vec<u8>, KittyGraphicsError> {
    // Simple base64 decoder
    // SAFETY: Base64 table indices are 0-63, which fit in i8 (max 127)
    #[allow(clippy::cast_possible_wrap)]
    const DECODE_TABLE: [i8; 256] = {
        let mut table = [-1i8; 256];
        let mut i = 0u8;
        while i < 26 {
            table[(b'A' + i) as usize] = i as i8;
            table[(b'a' + i) as usize] = (i + 26) as i8;
            i += 1;
        }
        i = 0;
        while i < 10 {
            table[(b'0' + i) as usize] = (i + 52) as i8;
            i += 1;
        }
        table[b'+' as usize] = 62;
        table[b'/' as usize] = 63;
        table[b'=' as usize] = 0; // Padding
        table
    };

    let mut output = Vec::with_capacity(data.len() * 3 / 4);
    let mut buffer = 0u32;
    let mut bits = 0u32;

    for &byte in data {
        if byte == b'=' {
            break;
        }

        let value = DECODE_TABLE[byte as usize];
        if value < 0 {
            // Skip whitespace and invalid characters
            continue;
        }

        // SAFETY: value is >= 0 (checked above), so cast to u32 is safe
        #[allow(clippy::cast_sign_loss)]
        let value_u32 = value as u32;
        buffer = (buffer << 6) | value_u32;
        bits += 6;

        if bits >= 8 {
            bits -= 8;
            // buffer >> bits extracts exactly 8 bits (bits is 0..8), fits in u8
            #[allow(clippy::cast_possible_truncation)]
            output.push((buffer >> bits) as u8);
            buffer &= (1 << bits) - 1;
        }
    }

    Ok(output)
}

/// Decompress zlib data.
#[allow(dead_code)] // Will be used by Terminal integration
#[cfg(feature = "zlib")]
pub fn decompress_zlib(data: &[u8]) -> Result<Vec<u8>, KittyGraphicsError> {
    use flate2::read::ZlibDecoder;
    use std::io::Read;

    let mut decoder = ZlibDecoder::new(data);
    let mut output = Vec::new();
    decoder
        .read_to_end(&mut output)
        .map_err(|e| KittyGraphicsError::CompressionError(e.to_string()))?;
    Ok(output)
}

/// Decompress zlib data (stub when zlib feature is disabled).
#[allow(dead_code)] // Will be used by Terminal integration
#[cfg(not(feature = "zlib"))]
pub fn decompress_zlib(_data: &[u8]) -> Result<Vec<u8>, KittyGraphicsError> {
    Err(KittyGraphicsError::CompressionError(
        "zlib support not compiled".to_string(),
    ))
}

/// Compose RGBA source pixels onto destination with optional offset and alpha blending.
///
/// This implements the Kitty graphics protocol's animation frame composition:
/// - Alpha blending: `dest = src * src_alpha + dest * (1 - src_alpha)`
/// - Overwrite: `dest = src` (or background color where src is transparent)
///
/// # Arguments
/// * `src` - Source RGBA pixel data
/// * `src_width` - Width of source frame in pixels
/// * `src_height` - Height of source frame in pixels
/// * `src_x_offset` - X offset within destination for source placement
/// * `src_y_offset` - Y offset within destination for source placement
/// * `dest` - Destination RGBA pixel data (modified in place)
/// * `dest_width` - Width of destination frame in pixels
/// * `dest_height` - Height of destination frame in pixels
/// * `mode` - Composition mode
/// * `background_color` - RGBA background color (used in overwrite mode for transparent pixels)
#[allow(clippy::too_many_arguments)]
fn compose_rgba(
    src: &[u8],
    src_width: u32,
    src_height: u32,
    src_x_offset: u32,
    src_y_offset: u32,
    dest: &mut [u8],
    dest_width: u32,
    dest_height: u32,
    mode: CompositionMode,
    background_color: u32,
) {
    // Extract background color components
    #[allow(clippy::cast_possible_truncation)]
    let bg_r = ((background_color >> 24) & 0xFF) as u8;
    #[allow(clippy::cast_possible_truncation)]
    let bg_g = ((background_color >> 16) & 0xFF) as u8;
    #[allow(clippy::cast_possible_truncation)]
    let bg_b = ((background_color >> 8) & 0xFF) as u8;
    #[allow(clippy::cast_possible_truncation)]
    let bg_a = (background_color & 0xFF) as u8;

    for src_y in 0..src_height {
        let dest_y = src_y_offset.saturating_add(src_y);
        if dest_y >= dest_height {
            break;
        }

        for src_x in 0..src_width {
            let dest_x = src_x_offset.saturating_add(src_x);
            if dest_x >= dest_width {
                break;
            }

            // Calculate offsets in pixel arrays (4 bytes per pixel)
            let src_idx = ((src_y * src_width + src_x) * 4) as usize;
            let dest_idx = ((dest_y * dest_width + dest_x) * 4) as usize;

            // Bounds check
            if src_idx + 3 >= src.len() || dest_idx + 3 >= dest.len() {
                continue;
            }

            let src_r = src[src_idx];
            let src_g = src[src_idx + 1];
            let src_b = src[src_idx + 2];
            let src_a = src[src_idx + 3];

            match mode {
                CompositionMode::Overwrite => {
                    // In overwrite mode, use background color where source is transparent
                    if src_a == 0 {
                        dest[dest_idx] = bg_r;
                        dest[dest_idx + 1] = bg_g;
                        dest[dest_idx + 2] = bg_b;
                        dest[dest_idx + 3] = bg_a;
                    } else {
                        dest[dest_idx] = src_r;
                        dest[dest_idx + 1] = src_g;
                        dest[dest_idx + 2] = src_b;
                        dest[dest_idx + 3] = src_a;
                    }
                }
                CompositionMode::AlphaBlend => {
                    // Standard alpha blending: out = src * src_a + dest * (1 - src_a)
                    if src_a == 255 {
                        // Fully opaque - just copy
                        dest[dest_idx] = src_r;
                        dest[dest_idx + 1] = src_g;
                        dest[dest_idx + 2] = src_b;
                        dest[dest_idx + 3] = 255;
                    } else if src_a > 0 {
                        // Partial transparency - blend
                        let src_alpha = u32::from(src_a);
                        let inv_alpha = 255 - src_alpha;

                        #[allow(clippy::cast_possible_truncation)]
                        let blend = |s: u8, d: u8| -> u8 {
                            ((u32::from(s) * src_alpha + u32::from(d) * inv_alpha) / 255) as u8
                        };

                        dest[dest_idx] = blend(src_r, dest[dest_idx]);
                        dest[dest_idx + 1] = blend(src_g, dest[dest_idx + 1]);
                        dest[dest_idx + 2] = blend(src_b, dest[dest_idx + 2]);
                        // Alpha channel: combine alphas
                        #[allow(clippy::cast_possible_truncation)]
                        let new_alpha = (src_alpha
                            + (u32::from(dest[dest_idx + 3]) * inv_alpha / 255))
                        .min(255) as u8;
                        dest[dest_idx + 3] = new_alpha;
                    }
                    // src_a == 0: fully transparent, leave dest unchanged
                }
            }
        }
    }
}

/// Convert RGB to RGBA.
#[allow(dead_code)] // Will be used by Terminal integration
pub fn rgb_to_rgba(rgb_data: &[u8]) -> Vec<u8> {
    let pixel_count = rgb_data.len() / 3;
    let mut rgba_data = Vec::with_capacity(pixel_count * 4);

    for chunk in rgb_data.chunks_exact(3) {
        rgba_data.push(chunk[0]); // R
        rgba_data.push(chunk[1]); // G
        rgba_data.push(chunk[2]); // B
        rgba_data.push(255); // A
    }

    rgba_data
}

/// Decode PNG data to RGBA.
///
/// Returns RGBA pixel data and dimensions (width, height).
#[cfg(feature = "png-images")]
pub fn decode_png(png_data: &[u8]) -> Result<(Vec<u8>, u32, u32), KittyGraphicsError> {
    use std::io::Cursor;

    let decoder = png::Decoder::new(Cursor::new(png_data));
    let mut reader = decoder.read_info().map_err(|e| {
        KittyGraphicsError::InvalidImageData(format!("PNG decode error: {}", e))
    })?;

    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).map_err(|e| {
        KittyGraphicsError::InvalidImageData(format!("PNG frame error: {}", e))
    })?;

    let width = info.width;
    let height = info.height;

    // Convert to RGBA based on color type
    let rgba_data = match info.color_type {
        png::ColorType::Rgba => {
            // Already RGBA
            buf.truncate(info.buffer_size());
            buf
        }
        png::ColorType::Rgb => {
            // RGB -> RGBA
            rgb_to_rgba(&buf[..info.buffer_size()])
        }
        png::ColorType::GrayscaleAlpha => {
            // Grayscale+Alpha -> RGBA
            let pixel_count = info.buffer_size() / 2;
            let mut rgba = Vec::with_capacity(pixel_count * 4);
            for chunk in buf[..info.buffer_size()].chunks_exact(2) {
                let gray = chunk[0];
                let alpha = chunk[1];
                rgba.push(gray); // R
                rgba.push(gray); // G
                rgba.push(gray); // B
                rgba.push(alpha); // A
            }
            rgba
        }
        png::ColorType::Grayscale => {
            // Grayscale -> RGBA
            let pixel_count = info.buffer_size();
            let mut rgba = Vec::with_capacity(pixel_count * 4);
            for &gray in &buf[..info.buffer_size()] {
                rgba.push(gray); // R
                rgba.push(gray); // G
                rgba.push(gray); // B
                rgba.push(255); // A
            }
            rgba
        }
        png::ColorType::Indexed => {
            // Indexed color - need to expand using palette
            return Err(KittyGraphicsError::InvalidImageData(
                "PNG indexed color not supported".to_string(),
            ));
        }
    };

    Ok((rgba_data, width, height))
}

/// Stub for PNG decoding when feature is disabled.
#[cfg(not(feature = "png-images"))]
pub fn decode_png(_png_data: &[u8]) -> Result<(Vec<u8>, u32, u32), KittyGraphicsError> {
    Err(KittyGraphicsError::InvalidImageData(
        "PNG support not compiled (enable png-images feature)".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storage_new() {
        let storage = KittyImageStorage::new();
        assert_eq!(storage.image_count(), 0);
        assert_eq!(storage.total_bytes(), 0);
        assert_eq!(storage.quota(), DEFAULT_STORAGE_QUOTA);
    }

    #[test]
    fn storage_store_image() {
        let mut storage = KittyImageStorage::new();
        let data = vec![0u8; 100];
        let image = KittyImage::new(0, 10, 10, data);

        let id = storage.store_image(image).unwrap();
        assert!(id > 0);
        assert_eq!(storage.image_count(), 1);
        assert_eq!(storage.total_bytes(), 100);
    }

    #[test]
    fn storage_remove_image() {
        let mut storage = KittyImageStorage::new();
        let data = vec![0u8; 100];
        let image = KittyImage::new(0, 10, 10, data);

        let id = storage.store_image(image).unwrap();
        assert!(storage.remove_image(id).is_some());
        assert_eq!(storage.image_count(), 0);
        assert_eq!(storage.total_bytes(), 0);
    }

    #[test]
    fn storage_dimension_limit() {
        let mut storage = KittyImageStorage::new();
        let data = vec![0u8; 100];
        let image = KittyImage::new(0, KITTY_MAX_DIMENSION + 1, 10, data);

        let result = storage.store_image(image);
        assert!(matches!(
            result,
            Err(KittyGraphicsError::DimensionsTooLarge { .. })
        ));
    }

    #[test]
    fn storage_quota_limit() {
        let mut storage = KittyImageStorage::with_quota(100);
        let data = vec![0u8; 101];
        let image = KittyImage::new(0, 10, 10, data);

        let result = storage.store_image(image);
        assert!(matches!(
            result,
            Err(KittyGraphicsError::StorageQuotaExceeded)
        ));
    }

    #[test]
    fn storage_placements() {
        let mut storage = KittyImageStorage::new();
        let data = vec![0u8; 100];
        let image = KittyImage::new(0, 10, 10, data);

        let id = storage.store_image(image).unwrap();
        let placement = KittyPlacement::new(0, 5, 10);
        let placement_id = storage.add_placement(id, placement).unwrap();

        assert!(placement_id > 0);
        assert!(storage.get_image(id).unwrap().has_placements());
    }

    #[test]
    fn storage_evict_unreferenced() {
        let mut storage = KittyImageStorage::new();

        // Image with placement
        let data1 = vec![0u8; 100];
        let image1 = KittyImage::new(0, 10, 10, data1);
        let id1 = storage.store_image(image1).unwrap();
        let placement = KittyPlacement::new(0, 5, 10);
        storage.add_placement(id1, placement).unwrap();

        // Image without placement
        let data2 = vec![0u8; 100];
        let image2 = KittyImage::new(0, 10, 10, data2);
        let id2 = storage.store_image(image2).unwrap();

        assert_eq!(storage.image_count(), 2);
        storage.evict_unreferenced();
        assert_eq!(storage.image_count(), 1);
        assert!(storage.get_image(id1).is_some());
        assert!(storage.get_image(id2).is_none());
    }

    #[test]
    fn storage_chunked_transmission() {
        let mut storage = KittyImageStorage::new();
        let cmd = KittyGraphicsCommand::parse(b"a=t,i=1,s=10,v=10,m=1");

        storage.start_chunked(cmd).unwrap();
        assert!(storage.is_loading());

        let result = storage.continue_chunked(b"dGVzdA==", false).unwrap();
        assert!(result.is_none());

        let result = storage.continue_chunked(b"", true).unwrap();
        assert!(result.is_some());
        assert!(!storage.is_loading());
    }

    #[test]
    fn storage_delete_by_id() {
        let mut storage = KittyImageStorage::new();
        let data = vec![0u8; 100];
        let mut image = KittyImage::new(42, 10, 10, data);
        image.add_placement(KittyPlacement::new(1, 0, 0));

        storage.store_image(image).unwrap();
        assert!(storage.get_image(42).is_some());

        let cmd = KittyGraphicsCommand::parse(b"a=d,d=I,i=42");
        storage.delete(&cmd);
        assert!(storage.get_image(42).is_none());
    }

    #[test]
    fn base64_decode() {
        let result = decode_base64(b"SGVsbG8gV29ybGQ=").unwrap();
        assert_eq!(result, b"Hello World");
    }

    #[test]
    fn base64_decode_no_padding() {
        let result = decode_base64(b"dGVzdA").unwrap();
        assert_eq!(result, b"test");
    }

    #[test]
    fn rgb_to_rgba_conversion() {
        let rgb = vec![255, 0, 0, 0, 255, 0, 0, 0, 255]; // R, G, B pixels
        let rgba = rgb_to_rgba(&rgb);
        assert_eq!(rgba, vec![255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255]);
    }

    #[test]
    fn placement_from_command() {
        let cmd = KittyGraphicsCommand::parse(b"a=p,p=5,x=10,y=20,w=50,h=30,X=2,Y=3,c=4,r=2,z=-1");
        let placement = KittyPlacement::from_command(&cmd, 100, 50);

        assert_eq!(placement.id, 5);
        assert_eq!(placement.source_x, 10);
        assert_eq!(placement.source_y, 20);
        assert_eq!(placement.source_width, 50);
        assert_eq!(placement.source_height, 30);
        assert_eq!(placement.cell_x_offset, 2);
        assert_eq!(placement.cell_y_offset, 3);
        assert_eq!(placement.num_columns, 4);
        assert_eq!(placement.num_rows, 2);
        assert_eq!(placement.z_index, -1);
    }

    #[test]
    fn placement_relative() {
        let cmd = KittyGraphicsCommand::parse(b"a=p,P=42,Q=1,H=-5,V=3");
        let placement = KittyPlacement::from_command(&cmd, 0, 0);

        if let PlacementLocation::Relative {
            parent_image_id,
            parent_placement_id,
            offset_x,
            offset_y,
        } = placement.location
        {
            assert_eq!(parent_image_id, 42);
            assert_eq!(parent_placement_id, 1);
            assert_eq!(offset_x, -5);
            assert_eq!(offset_y, 3);
        } else {
            panic!("Expected relative placement");
        }
    }

    #[test]
    fn placement_virtual() {
        let cmd = KittyGraphicsCommand::parse(b"a=T,U=1,p=99");
        let placement = KittyPlacement::from_command(&cmd, 0, 0);

        assert!(placement.is_virtual);
        if let PlacementLocation::Virtual { id } = placement.location {
            assert_eq!(id, 99);
        } else {
            panic!("Expected virtual placement");
        }
    }

    #[test]
    fn placements_by_z_index() {
        let mut storage = KittyImageStorage::new();

        let data = vec![0u8; 100];
        let image = KittyImage::new(1, 10, 10, data);
        storage.store_image(image).unwrap();

        let mut p1 = KittyPlacement::new(1, 0, 0);
        p1.z_index = 10;
        let mut p2 = KittyPlacement::new(2, 0, 0);
        p2.z_index = -5;
        let mut p3 = KittyPlacement::new(3, 0, 0);
        p3.z_index = 0;

        storage.add_placement(1, p1).unwrap();
        storage.add_placement(1, p2).unwrap();
        storage.add_placement(1, p3).unwrap();

        let sorted = storage.placements_by_z_index();
        assert_eq!(sorted.len(), 3);
        assert_eq!(sorted[0].2.z_index, -5);
        assert_eq!(sorted[1].2.z_index, 0);
        assert_eq!(sorted[2].2.z_index, 10);
    }

    #[test]
    fn delete_in_row() {
        let mut storage = KittyImageStorage::new();

        let data = vec![0u8; 100];
        let image = KittyImage::new(1, 10, 10, data);
        storage.store_image(image).unwrap();

        storage
            .add_placement(1, KittyPlacement::new(1, 5, 0))
            .unwrap();
        storage
            .add_placement(1, KittyPlacement::new(2, 5, 10))
            .unwrap();
        storage
            .add_placement(1, KittyPlacement::new(3, 10, 0))
            .unwrap();

        storage.delete_in_row(5, false);

        let image = storage.get_image(1).unwrap();
        assert_eq!(image.placement_count(), 1);
        assert!(image.get_placement(3).is_some());
    }

    #[test]
    fn delete_in_column() {
        let mut storage = KittyImageStorage::new();

        let data = vec![0u8; 100];
        let image = KittyImage::new(1, 10, 10, data);
        storage.store_image(image).unwrap();

        storage
            .add_placement(1, KittyPlacement::new(1, 0, 5))
            .unwrap();
        storage
            .add_placement(1, KittyPlacement::new(2, 10, 5))
            .unwrap();
        storage
            .add_placement(1, KittyPlacement::new(3, 0, 10))
            .unwrap();

        storage.delete_in_column(5, false);

        let image = storage.get_image(1).unwrap();
        assert_eq!(image.placement_count(), 1);
        assert!(image.get_placement(3).is_some());
    }

    #[test]
    #[cfg(feature = "png-images")]
    fn decode_png_rgba() {
        // Create a minimal valid PNG: 2x2 RGBA image
        // PNG header + IHDR chunk + IDAT chunk + IEND chunk
        let png_data = create_test_png(2, 2, png::ColorType::Rgba);
        let result = decode_png(&png_data);
        assert!(result.is_ok(), "PNG decode failed: {:?}", result);
        let (rgba, width, height) = result.unwrap();
        assert_eq!(width, 2);
        assert_eq!(height, 2);
        assert_eq!(rgba.len(), 2 * 2 * 4); // 4 bytes per pixel
    }

    #[test]
    #[cfg(feature = "png-images")]
    fn decode_png_rgb() {
        // Create a 2x2 RGB PNG - should be converted to RGBA
        let png_data = create_test_png(2, 2, png::ColorType::Rgb);
        let result = decode_png(&png_data);
        assert!(result.is_ok(), "PNG decode failed: {:?}", result);
        let (rgba, width, height) = result.unwrap();
        assert_eq!(width, 2);
        assert_eq!(height, 2);
        assert_eq!(rgba.len(), 2 * 2 * 4); // Converted to 4 bytes per pixel
    }

    #[test]
    #[cfg(feature = "png-images")]
    fn decode_png_grayscale() {
        // Create a 2x2 grayscale PNG - should be converted to RGBA
        let png_data = create_test_png(2, 2, png::ColorType::Grayscale);
        let result = decode_png(&png_data);
        assert!(result.is_ok(), "PNG decode failed: {:?}", result);
        let (rgba, width, height) = result.unwrap();
        assert_eq!(width, 2);
        assert_eq!(height, 2);
        assert_eq!(rgba.len(), 2 * 2 * 4); // Converted to 4 bytes per pixel
    }

    #[test]
    #[cfg(feature = "png-images")]
    fn decode_png_grayscale_alpha() {
        // Create a 2x2 grayscale+alpha PNG - should be converted to RGBA
        let png_data = create_test_png(2, 2, png::ColorType::GrayscaleAlpha);
        let result = decode_png(&png_data);
        assert!(result.is_ok(), "PNG decode failed: {:?}", result);
        let (rgba, width, height) = result.unwrap();
        assert_eq!(width, 2);
        assert_eq!(height, 2);
        assert_eq!(rgba.len(), 2 * 2 * 4); // Converted to 4 bytes per pixel
    }

    #[test]
    #[cfg(feature = "png-images")]
    fn decode_png_invalid_data() {
        // Invalid PNG data should return an error
        let result = decode_png(b"not a png");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, KittyGraphicsError::InvalidImageData(_)));
    }

    #[test]
    #[cfg(feature = "png-images")]
    fn decode_png_empty() {
        // Empty data should return an error
        let result = decode_png(b"");
        assert!(result.is_err());
    }

    /// Helper function to create test PNG images
    #[cfg(feature = "png-images")]
    fn create_test_png(width: u32, height: u32, color_type: png::ColorType) -> Vec<u8> {
        use std::io::Cursor;

        let mut buf = Vec::new();
        {
            let mut encoder = png::Encoder::new(Cursor::new(&mut buf), width, height);
            encoder.set_color(color_type);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();

            // Calculate row size based on color type
            let bytes_per_pixel = match color_type {
                png::ColorType::Rgba => 4,
                png::ColorType::Rgb => 3,
                png::ColorType::GrayscaleAlpha => 2,
                png::ColorType::Grayscale => 1,
                png::ColorType::Indexed => 1,
            };
            let row_size = (width as usize) * bytes_per_pixel;
            #[allow(clippy::cast_possible_truncation)]
            let data: Vec<u8> = (0..(height as usize * row_size))
                .map(|i| i as u8) // Truncation is intentional for test pattern
                .collect();

            writer.write_image_data(&data).unwrap();
        }
        buf
    }

    #[test]
    #[cfg(not(feature = "png-images"))]
    fn decode_png_disabled() {
        // When PNG support is disabled, decode_png should return an error
        let result = decode_png(b"anything");
        assert!(result.is_err());
    }

    #[test]
    #[cfg(feature = "zlib")]
    fn decompress_zlib_valid() {
        use flate2::write::ZlibEncoder;
        use flate2::Compression;
        use std::io::Write;

        // Create some test data
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let original_data: Vec<u8> = (0..256).map(|i| i as u8).collect();

        // Compress it using zlib
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&original_data).unwrap();
        let compressed = encoder.finish().unwrap();

        // Decompress using our function
        let result = decompress_zlib(&compressed);
        assert!(result.is_ok(), "Zlib decompress failed: {:?}", result);
        let decompressed = result.unwrap();
        assert_eq!(decompressed, original_data);
    }

    #[test]
    #[cfg(feature = "zlib")]
    fn decompress_zlib_empty() {
        use flate2::write::ZlibEncoder;
        use flate2::Compression;
        use std::io::Write;

        // Compress empty data
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&[]).unwrap();
        let compressed = encoder.finish().unwrap();

        let result = decompress_zlib(&compressed);
        assert!(result.is_ok());
        let decompressed = result.unwrap();
        assert!(decompressed.is_empty());
    }

    #[test]
    #[cfg(feature = "zlib")]
    fn decompress_zlib_large() {
        use flate2::write::ZlibEncoder;
        use flate2::Compression;
        use std::io::Write;

        // Create large test data (100KB of repeated pattern)
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let original_data: Vec<u8> = (0..100_000).map(|i| i as u8).collect();

        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::best());
        encoder.write_all(&original_data).unwrap();
        let compressed = encoder.finish().unwrap();

        // Verify compression actually occurred
        assert!(
            compressed.len() < original_data.len(),
            "Expected compression, got {} -> {}",
            original_data.len(),
            compressed.len()
        );

        let result = decompress_zlib(&compressed);
        assert!(result.is_ok());
        let decompressed = result.unwrap();
        assert_eq!(decompressed, original_data);
    }

    #[test]
    #[cfg(feature = "zlib")]
    fn decompress_zlib_invalid() {
        // Try to decompress invalid data
        let result = decompress_zlib(b"not valid zlib data");
        assert!(result.is_err());
        match result {
            Err(KittyGraphicsError::CompressionError(_)) => {}
            other => panic!("Expected CompressionError, got {:?}", other),
        }
    }

    #[test]
    #[cfg(not(feature = "zlib"))]
    fn decompress_zlib_disabled() {
        // When zlib support is disabled, decompress_zlib should return an error
        let result = decompress_zlib(b"anything");
        assert!(result.is_err());
    }
}

#[cfg(kani)]
mod proofs {
    use super::*;

    /// Frame count never exceeds MAX_FRAMES_PER_IMAGE.
    ///
    /// INV-ANIM-1 from TLA+ Animation spec: frame_count <= MaxFrames
    #[kani::proof]
    fn frame_count_bounded() {
        let data = vec![0u8; 100];
        let mut image = KittyImage::new(1, 10, 10, data);

        let add_count: usize = kani::any();
        kani::assume(add_count <= MAX_FRAMES_PER_IMAGE + 5);

        for i in 0..add_count {
            let frame_data = vec![0u8; 100];
            let frame = AnimationFrame::new((i + 1) as u32, frame_data, 10, 10);
            let _ = image.add_frame(frame); // May fail when at limit
        }

        kani::assert(
            image.frame_count() <= MAX_FRAMES_PER_IMAGE,
            "frame count exceeds maximum",
        );
    }

    /// Current frame index is always valid.
    ///
    /// INV-ANIM-2 from TLA+ Animation spec: current_frame <= frame_count
    #[kani::proof]
    fn current_frame_valid() {
        let data = vec![0u8; 100];
        let mut image = KittyImage::new(1, 10, 10, data);

        let add_count: usize = kani::any();
        kani::assume(add_count <= 5);

        for i in 0..add_count {
            let frame_data = vec![0u8; 100];
            let frame = AnimationFrame::new((i + 1) as u32, frame_data, 10, 10);
            let _ = image.add_frame(frame);
        }

        // Advance frame multiple times
        let advance_count: u8 = kani::any();
        kani::assume(advance_count <= 10);

        image.animation_state = AnimationState::Running;
        for _ in 0..advance_count {
            image.advance_frame();
        }

        kani::assert(
            image.current_frame_index <= image.frame_count(),
            "current frame index exceeds frame count",
        );
    }

    /// Animation state transitions are valid.
    #[kani::proof]
    fn animation_state_valid() {
        let data = vec![0u8; 100];
        let mut image = KittyImage::new(1, 10, 10, data);

        // Add some frames
        let frame_data = vec![0u8; 100];
        let frame = AnimationFrame::new(1, frame_data, 10, 10);
        let _ = image.add_frame(frame);

        // Test state transitions
        let state_val: u8 = kani::any();
        kani::assume(state_val <= 3);

        let new_state = match state_val {
            0 => AnimationState::Stopped,
            1 => AnimationState::Running,
            _ => AnimationState::Loading,
        };

        image.animation_state = new_state;

        // Verify state is one of valid values
        kani::assert(
            matches!(
                image.animation_state,
                AnimationState::Stopped | AnimationState::Running | AnimationState::Loading
            ),
            "invalid animation state",
        );
    }

    /// Image data size calculation is accurate.
    #[kani::proof]
    fn data_size_accurate() {
        let root_size: usize = kani::any();
        kani::assume(root_size > 0 && root_size <= 1000);

        let data = vec![0u8; root_size];
        let mut image = KittyImage::new(1, 10, 10, data);

        let frame_count: u8 = kani::any();
        kani::assume(frame_count <= 3);

        let frame_size: usize = kani::any();
        kani::assume(frame_size > 0 && frame_size <= 1000);

        for i in 0..frame_count {
            let frame_data = vec![0u8; frame_size];
            let frame = AnimationFrame::new((i + 1) as u32, frame_data, 10, 10);
            let _ = image.add_frame(frame);
        }

        let total_size = image.data_size();
        let expected = root_size + (frame_count as usize * frame_size);

        kani::assert(total_size == expected, "data size doesn't match expected");
    }

    /// Placement sizing fields are clamped to u16 bounds.
    #[kani::proof]
    fn placement_size_fields_bounded() {
        let value: u32 = kani::any();
        let clamped = KittyPlacement::clamp_u16(value);
        kani::assert(
            clamped as u32 <= u16::MAX as u32,
            "clamped value exceeds u16 max",
        );
    }
}
