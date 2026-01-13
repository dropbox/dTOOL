//! Types for the GPU renderer.

use std::fmt;

/// Renderer configuration.
#[derive(Debug, Clone)]
pub struct RendererConfig {
    /// Background color (R, G, B)
    pub background_color: (u8, u8, u8),
    /// Whether to enable vsync
    pub vsync: bool,
    /// Target frames per second (used when vsync is disabled)
    pub target_fps: u32,
    /// Maximum time to wait for a drawable (milliseconds)
    pub drawable_timeout_ms: u64,
    /// Whether to enable damage-based rendering
    pub damage_rendering: bool,
}

/// Blend mode for background images.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DtermBlendMode {
    /// Standard alpha blend.
    Normal = 0,
    /// Multiply source and destination colors.
    Multiply = 1,
    /// Screen blend (inverse multiply).
    Screen = 2,
    /// Overlay blend (multiply or screen depending on base).
    Overlay = 3,
}

impl Default for DtermBlendMode {
    fn default() -> Self {
        Self::Normal
    }
}

impl Default for RendererConfig {
    fn default() -> Self {
        Self {
            background_color: (0, 0, 0), // Black
            vsync: true,
            target_fps: 60,
            drawable_timeout_ms: 17, // ~1 frame at 60fps
            damage_rendering: true,
        }
    }
}

/// Errors that can occur during rendering.
#[derive(Debug)]
pub enum RenderError {
    /// Failed to create a resource (buffer, texture, etc.)
    ResourceCreation(String),
    /// Frame timeout - drawable not provided in time
    FrameTimeout,
    /// No surface configured
    NoSurface,
    /// Surface lost (window closed, etc.)
    SurfaceLost,
    /// Invalid terminal state
    InvalidState(String),
    /// Shader compilation failed
    ShaderCompilation(String),
    /// Device lost
    DeviceLost,
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RenderError::ResourceCreation(msg) => write!(f, "Resource creation failed: {}", msg),
            RenderError::FrameTimeout => write!(f, "Frame timeout - drawable not provided"),
            RenderError::NoSurface => write!(f, "No rendering surface configured"),
            RenderError::SurfaceLost => write!(f, "Rendering surface was lost"),
            RenderError::InvalidState(msg) => write!(f, "Invalid terminal state: {}", msg),
            RenderError::ShaderCompilation(msg) => write!(f, "Shader compilation failed: {}", msg),
            RenderError::DeviceLost => write!(f, "GPU device was lost"),
        }
    }
}

impl std::error::Error for RenderError {}

/// Result type for rendering operations.
pub type RenderResult<T> = Result<T, RenderError>;

/// Represents a cell position in the terminal grid.
///
/// Part of the FFI API for damage-based rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(dead_code)]
pub struct CellPosition {
    /// Row (0-indexed from top)
    pub row: u32,
    /// Column (0-indexed from left)
    pub col: u32,
}

impl CellPosition {
    /// Create a new cell position.
    #[allow(dead_code)]
    pub const fn new(row: u32, col: u32) -> Self {
        Self { row, col }
    }
}

/// A rectangle of cells that needs to be redrawn.
///
/// Part of the FFI API for damage-based rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub struct DamageRect {
    /// Top-left cell (inclusive)
    pub min: CellPosition,
    /// Bottom-right cell (inclusive)
    pub max: CellPosition,
}

#[allow(dead_code)]
impl DamageRect {
    /// Create a damage rect for a single cell.
    pub const fn single(row: u32, col: u32) -> Self {
        Self {
            min: CellPosition::new(row, col),
            max: CellPosition::new(row, col),
        }
    }

    /// Create a damage rect for a range of cells.
    pub const fn range(min_row: u32, min_col: u32, max_row: u32, max_col: u32) -> Self {
        Self {
            min: CellPosition::new(min_row, min_col),
            max: CellPosition::new(max_row, max_col),
        }
    }

    /// Check if this damage rect is empty (covers no cells).
    pub const fn is_empty(&self) -> bool {
        self.max.row < self.min.row || self.max.col < self.min.col
    }

    /// Get the width in cells.
    pub const fn width(&self) -> u32 {
        if self.is_empty() {
            0
        } else {
            self.max.col - self.min.col + 1
        }
    }

    /// Get the height in cells.
    pub const fn height(&self) -> u32 {
        if self.is_empty() {
            0
        } else {
            self.max.row - self.min.row + 1
        }
    }

    /// Merge two damage rects into one that covers both.
    pub fn union(&self, other: &Self) -> Self {
        Self {
            min: CellPosition::new(
                self.min.row.min(other.min.row),
                self.min.col.min(other.min.col),
            ),
            max: CellPosition::new(
                self.max.row.max(other.max.row),
                self.max.col.max(other.max.col),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_renderer_config_default() {
        let config = RendererConfig::default();
        assert_eq!(config.background_color, (0, 0, 0));
        assert!(config.vsync);
        assert_eq!(config.target_fps, 60);
    }

    #[test]
    fn test_blend_mode_values() {
        assert_eq!(DtermBlendMode::Normal as u32, 0);
        assert_eq!(DtermBlendMode::Multiply as u32, 1);
        assert_eq!(DtermBlendMode::Screen as u32, 2);
        assert_eq!(DtermBlendMode::Overlay as u32, 3);
    }

    #[test]
    fn test_damage_rect_single() {
        let rect = DamageRect::single(5, 10);
        assert_eq!(rect.width(), 1);
        assert_eq!(rect.height(), 1);
        assert!(!rect.is_empty());
    }

    #[test]
    fn test_damage_rect_range() {
        let rect = DamageRect::range(0, 0, 24, 80);
        assert_eq!(rect.width(), 81);
        assert_eq!(rect.height(), 25);
    }

    #[test]
    fn test_damage_rect_union() {
        let a = DamageRect::single(0, 0);
        let b = DamageRect::single(10, 20);
        let u = a.union(&b);
        assert_eq!(u.min.row, 0);
        assert_eq!(u.min.col, 0);
        assert_eq!(u.max.row, 10);
        assert_eq!(u.max.col, 20);
    }
}
