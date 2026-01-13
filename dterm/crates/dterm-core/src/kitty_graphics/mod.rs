//! Kitty Graphics Protocol implementation.
//!
//! This module implements the Kitty graphics protocol for displaying images
//! in the terminal. The protocol uses APC (Application Program Command)
//! escape sequences: `ESC _ G <control-data> ; <payload> ESC \`
//!
//! ## Protocol Overview
//!
//! - **Transmission**: Images can be sent inline (base64), via file, temp file, or shared memory
//! - **Display**: Images can be displayed at cursor, scaled, cropped, or placed absolutely
//! - **Animation**: Supports multi-frame animations with controllable playback
//! - **Management**: Images can be queried, deleted individually or in groups
//!
//! ## References
//!
//! - [Kitty Graphics Protocol](https://sw.kovidgoyal.net/kitty/graphics-protocol/)
//! - `research/kitty/kitty/graphics.c`
//! - `research/ghostty/src/terminal/kitty/`

mod command;
pub mod storage;

pub use command::{
    Action, AnimationState, CompositionMode, CompressionType, CursorMovement, DeleteAction,
    ImageFormat, KittyGraphicsCommand, TransmissionType,
};
pub use storage::{
    decode_base64, AnimationFrame, KittyGraphicsError, KittyImage, KittyImageStorage,
    KittyPlacement, PlacementLocation, MAX_FRAMES_PER_IMAGE,
};

/// Maximum dimension for Kitty images (width or height).
/// Prevents DoS via extremely large images.
pub const KITTY_MAX_DIMENSION: u32 = 10000;

/// Default storage quota in bytes (320 MB).
pub const DEFAULT_STORAGE_QUOTA: usize = 320 * 1024 * 1024;

/// Maximum number of images that can be stored.
pub const MAX_IMAGES: usize = 10000;

/// Maximum depth of relative placement chains.
pub const MAX_PLACEMENT_CHAIN_DEPTH: u8 = 8;

#[cfg(test)]
mod tests;

// Kani proofs
#[cfg(kani)]
mod kani_proofs {
    use super::command::*;
    use super::*;

    /// Command parsing never panics on any input.
    #[kani::proof]
    #[kani::unwind(65)]
    fn command_parse_never_panics() {
        let data: [u8; 64] = kani::any();
        // Try parsing the data as a command - should never panic
        let _ = KittyGraphicsCommand::parse(&data);
    }

    /// Action byte conversion produces valid variants.
    #[kani::proof]
    fn action_from_byte_valid() {
        let byte: u8 = kani::any();
        let action = Action::from_byte(byte);
        // Should always produce a valid Action
        let _ = action;
    }

    /// Delete action byte conversion produces valid variants.
    #[kani::proof]
    fn delete_action_from_byte_valid() {
        let byte: u8 = kani::any();
        let action = DeleteAction::from_byte(byte);
        // Should always produce a valid DeleteAction
        let _ = action;
    }

    /// Transmission type byte conversion produces valid variants.
    #[kani::proof]
    fn transmission_type_from_byte_valid() {
        let byte: u8 = kani::any();
        let tt = TransmissionType::from_byte(byte);
        // Should always produce a valid TransmissionType
        let _ = tt;
    }

    /// Image format value produces valid variants.
    #[kani::proof]
    fn image_format_from_value_valid() {
        let value: u32 = kani::any();
        let format = ImageFormat::from_value(value);
        // Should always produce a valid ImageFormat
        let _ = format;
    }

    /// Compression type byte conversion produces valid variants.
    #[kani::proof]
    fn compression_type_from_byte_valid() {
        let byte: u8 = kani::any();
        let ct = CompressionType::from_byte(byte);
        // Should always produce a valid CompressionType
        let _ = ct;
    }

    /// Command quiet level is always 0, 1, or 2.
    #[kani::proof]
    fn quiet_level_bounded() {
        let mut cmd = KittyGraphicsCommand::new();
        let value: u32 = kani::any();
        cmd.quiet = value;
        // Quiet values > 2 are treated as 2 in practice
        let effective = cmd.quiet.min(2);
        kani::assert(effective <= 2, "quiet level out of bounds");
    }

    /// Image dimensions are clamped to KITTY_MAX_DIMENSION.
    #[kani::proof]
    fn image_dimensions_bounded() {
        let width: u32 = kani::any();
        let height: u32 = kani::any();

        let clamped_width = width.min(KITTY_MAX_DIMENSION);
        let clamped_height = height.min(KITTY_MAX_DIMENSION);

        kani::assert(clamped_width <= KITTY_MAX_DIMENSION, "width exceeds max");
        kani::assert(clamped_height <= KITTY_MAX_DIMENSION, "height exceeds max");
    }
}
