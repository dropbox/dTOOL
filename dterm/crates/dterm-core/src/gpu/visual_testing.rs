//! Visual regression testing infrastructure for GPU rendering.
//!
// Visual testing module - these casts are safe for terminal dimensions:
// - f32 -> u32: Cell/viewport sizes are bounded and non-negative
// - async functions: Required for wgpu buffer mapping, even if pollster blocks
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::unused_async)]
#![allow(clippy::redundant_else)]
#![allow(clippy::default_trait_access)]
#![allow(clippy::return_self_not_must_use)]
//!
//! This module provides utilities for rendering terminal content to offscreen
//! buffers and comparing the results against reference images. It is designed
//! to catch visual bugs like invisible characters that pass unit tests but
//! produce no visible output.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                    VisualTestHarness                             │
//! │  - Creates headless wgpu device                                  │
//! │  - Manages offscreen render targets                              │
//! │  - Provides image comparison utilities                          │
//! └───────────────┬─────────────────────────────────────────────────┘
//!                 │
//!                 ▼
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                    Renderer                                       │
//! │  - render_to_buffer() for offscreen rendering                   │
//! │  - Produces RGBA pixel data                                      │
//! └───────────────┬─────────────────────────────────────────────────┘
//!                 │
//!                 ▼
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                    Comparison                                     │
//! │  - Pixel-exact comparison                                        │
//! │  - Perceptual diff (optional)                                    │
//! │  - Golden image generation                                       │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Example
//!
//! ```ignore
//! use dterm_core::gpu::visual_testing::{VisualTestHarness, CompareResult};
//!
//! let harness = VisualTestHarness::new().await?;
//! let terminal = create_terminal_with("┌─┐\n│X│\n└─┘");
//!
//! let result = harness.render_and_compare(
//!     &terminal,
//!     "box_drawing_3x3",
//! ).await?;
//!
//! assert!(result.is_match(), "Visual regression: {}", result.description());
//! ```

use super::{Renderer, RendererConfig};
use crate::terminal::Terminal;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Error type for visual testing operations.
#[derive(Debug)]
pub enum VisualTestError {
    /// Failed to create wgpu device
    DeviceCreation(String),
    /// Failed to create offscreen texture
    TextureCreation(String),
    /// Failed to read back pixels from GPU
    BufferReadback(String),
    /// Failed to load or save image file
    ImageIo(String),
    /// Golden image not found
    GoldenNotFound(PathBuf),
    /// Image dimensions don't match
    DimensionMismatch {
        /// Expected dimensions (width, height)
        expected: (u32, u32),
        /// Actual dimensions (width, height)
        actual: (u32, u32),
    },
    /// Rendering failed
    RenderFailed(String),
}

impl std::fmt::Display for VisualTestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DeviceCreation(msg) => write!(f, "Failed to create GPU device: {}", msg),
            Self::TextureCreation(msg) => write!(f, "Failed to create texture: {}", msg),
            Self::BufferReadback(msg) => write!(f, "Failed to read pixels: {}", msg),
            Self::ImageIo(msg) => write!(f, "Image I/O error: {}", msg),
            Self::GoldenNotFound(path) => write!(f, "Golden image not found: {}", path.display()),
            Self::DimensionMismatch { expected, actual } => {
                write!(
                    f,
                    "Dimension mismatch: expected {}x{}, got {}x{}",
                    expected.0, expected.1, actual.0, actual.1
                )
            }
            Self::RenderFailed(msg) => write!(f, "Rendering failed: {}", msg),
        }
    }
}

impl std::error::Error for VisualTestError {}

/// Result of comparing a rendered image against a golden reference.
#[derive(Debug)]
pub struct CompareResult {
    /// Whether the images match (within tolerance)
    pub matches: bool,
    /// Number of pixels that differ
    pub diff_pixels: u32,
    /// Total number of pixels
    pub total_pixels: u32,
    /// Path to the diff image (if generated)
    pub diff_image_path: Option<PathBuf>,
    /// Human-readable description of the result
    pub description: String,
}

impl CompareResult {
    /// Returns true if the images match within tolerance.
    pub fn is_match(&self) -> bool {
        self.matches
    }

    /// Returns the percentage of pixels that differ.
    pub fn diff_percentage(&self) -> f64 {
        if self.total_pixels == 0 {
            0.0
        } else {
            (self.diff_pixels as f64 / self.total_pixels as f64) * 100.0
        }
    }
}

/// Configuration for visual test comparison.
#[derive(Debug, Clone)]
pub struct CompareConfig {
    /// Maximum percentage of pixels that can differ (0.0 - 100.0)
    pub max_diff_percentage: f64,
    /// Whether to generate diff images for failures
    pub generate_diff_image: bool,
    /// Directory to save diff images
    pub diff_output_dir: Option<PathBuf>,
    /// Whether to update golden images if they don't exist
    pub update_golden: bool,
}

impl Default for CompareConfig {
    fn default() -> Self {
        Self {
            max_diff_percentage: 0.0, // Exact match required by default
            generate_diff_image: true,
            diff_output_dir: None,
            update_golden: false,
        }
    }
}

/// Test harness for visual regression testing.
///
/// Creates a headless GPU context and provides utilities for rendering
/// terminal content to offscreen buffers and comparing against reference images.
pub struct VisualTestHarness {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    renderer: Renderer,
    golden_dir: PathBuf,
    config: CompareConfig,
}

impl VisualTestHarness {
    /// Create a new visual test harness with default settings.
    ///
    /// Uses the current crate's `tests/golden` directory for reference images.
    pub async fn new() -> Result<Self, VisualTestError> {
        Self::with_golden_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden")).await
    }

    /// Create a new visual test harness with a custom golden image directory.
    pub async fn with_golden_dir(golden_dir: PathBuf) -> Result<Self, VisualTestError> {
        // Create headless wgpu instance
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        // Request adapter (prefer low-power for testing)
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .ok_or_else(|| VisualTestError::DeviceCreation("No suitable GPU adapter found".into()))?;

        // Create device and queue
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Visual Test Device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::downlevel_defaults(),
                    memory_hints: Default::default(),
                },
                None,
            )
            .await
            .map_err(|e| VisualTestError::DeviceCreation(e.to_string()))?;

        let device = Arc::new(device);
        let queue = Arc::new(queue);

        // Create renderer with standard test configuration
        let renderer_config = RendererConfig {
            background_color: (0, 0, 0), // Black background
            ..Default::default()
        };

        let renderer = Renderer::new(Arc::clone(&device), Arc::clone(&queue), renderer_config);

        Ok(Self {
            device,
            queue,
            renderer,
            golden_dir,
            config: CompareConfig::default(),
        })
    }

    /// Set the comparison configuration.
    pub fn with_config(mut self, config: CompareConfig) -> Self {
        self.config = config;
        self
    }

    /// Get a reference to the renderer for additional configuration.
    pub fn renderer(&self) -> &Renderer {
        &self.renderer
    }

    /// Get a mutable reference to the renderer for additional configuration.
    pub fn renderer_mut(&mut self) -> &mut Renderer {
        &mut self.renderer
    }

    /// Render a terminal to an RGBA pixel buffer.
    ///
    /// # Arguments
    /// * `terminal` - The terminal to render
    /// * `width` - Output width in pixels
    /// * `height` - Output height in pixels
    ///
    /// # Returns
    /// RGBA pixel data as a Vec<u8> (width * height * 4 bytes)
    pub async fn render_to_pixels(
        &self,
        terminal: &Terminal,
        width: u32,
        height: u32,
    ) -> Result<Vec<u8>, VisualTestError> {
        // Create offscreen texture with COPY_SRC for readback
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Visual Test Render Target"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Bgra8UnormSrgb,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Render to texture
        self.renderer
            .render(terminal, &texture_view)
            .map_err(|e| VisualTestError::RenderFailed(e.to_string()))?;

        // Create staging buffer for readback
        let bytes_per_row = align_to_256(width * 4);
        let buffer_size = (bytes_per_row * height) as u64;

        let staging_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Visual Test Staging Buffer"),
            size: buffer_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        // Copy texture to staging buffer
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Visual Test Copy Encoder"),
            });

        encoder.copy_texture_to_buffer(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::ImageCopyBuffer {
                buffer: &staging_buffer,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        self.queue.submit(std::iter::once(encoder.finish()));

        // Map buffer and read data
        let buffer_slice = staging_buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();

        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            tx.send(result).ok();
        });

        // Wait for GPU to finish
        self.device.poll(wgpu::Maintain::Wait);

        rx.recv()
            .map_err(|_| VisualTestError::BufferReadback("Channel closed".into()))?
            .map_err(|e| VisualTestError::BufferReadback(e.to_string()))?;

        // Read pixels and convert BGRA to RGBA
        let data = buffer_slice.get_mapped_range();
        let mut pixels = Vec::with_capacity((width * height * 4) as usize);

        for row in 0..height {
            let row_start = (row * bytes_per_row) as usize;
            for col in 0..width {
                let pixel_start = row_start + (col * 4) as usize;
                // BGRA -> RGBA
                pixels.push(data[pixel_start + 2]); // R
                pixels.push(data[pixel_start + 1]); // G
                pixels.push(data[pixel_start]);     // B
                pixels.push(data[pixel_start + 3]); // A
            }
        }

        drop(data);
        staging_buffer.unmap();

        Ok(pixels)
    }

    /// Render a terminal and compare against a golden reference image.
    ///
    /// # Arguments
    /// * `terminal` - The terminal to render
    /// * `test_name` - Name for the golden image (without extension)
    ///
    /// # Returns
    /// Comparison result indicating whether the images match.
    pub async fn render_and_compare(
        &self,
        terminal: &Terminal,
        test_name: &str,
    ) -> Result<CompareResult, VisualTestError> {
        // Calculate render dimensions from terminal size
        let (cell_width, cell_height) = self.renderer.cell_dimensions();
        let cols = terminal.grid().cols();
        let rows = terminal.grid().rows();
        let width = (f32::from(cols) * cell_width) as u32;
        let height = (f32::from(rows) * cell_height) as u32;

        self.render_and_compare_at_size(terminal, test_name, width, height)
            .await
    }

    /// Render a terminal at a specific size and compare against a golden reference.
    pub async fn render_and_compare_at_size(
        &self,
        terminal: &Terminal,
        test_name: &str,
        width: u32,
        height: u32,
    ) -> Result<CompareResult, VisualTestError> {
        let pixels = self.render_to_pixels(terminal, width, height).await?;

        let golden_path = self.golden_dir.join(format!("{}.png", test_name));

        // Check if golden exists
        if !golden_path.exists() {
            if self.config.update_golden {
                // Save as new golden
                self.save_image(&golden_path, &pixels, width, height)?;
                return Ok(CompareResult {
                    matches: true,
                    diff_pixels: 0,
                    total_pixels: width * height,
                    diff_image_path: None,
                    description: format!("Created new golden image: {}", golden_path.display()),
                });
            } else {
                return Err(VisualTestError::GoldenNotFound(golden_path));
            }
        }

        // Load golden image
        let golden_img = image::open(&golden_path)
            .map_err(|e| VisualTestError::ImageIo(e.to_string()))?
            .to_rgba8();

        // Check dimensions
        if golden_img.width() != width || golden_img.height() != height {
            return Err(VisualTestError::DimensionMismatch {
                expected: (golden_img.width(), golden_img.height()),
                actual: (width, height),
            });
        }

        // Compare pixels
        let golden_pixels = golden_img.as_raw();
        let mut diff_pixels = 0u32;
        let mut diff_image_data: Option<Vec<u8>> = if self.config.generate_diff_image {
            Some(vec![0u8; pixels.len()])
        } else {
            None
        };

        for (i, (&actual, &expected)) in pixels.iter().zip(golden_pixels.iter()).enumerate() {
            if actual != expected {
                if i % 4 == 3 {
                    // Alpha channel
                    continue; // Don't count alpha differences
                }
                if i % 4 == 0 {
                    // Only count once per pixel (R channel)
                    diff_pixels += 1;
                }

                // Mark differing pixels in diff image (red for actual, green for expected)
                if let Some(ref mut diff_data) = diff_image_data {
                    let pixel_idx = (i / 4) * 4;
                    diff_data[pixel_idx] = 255;     // R - actual
                    diff_data[pixel_idx + 1] = 0;   // G - expected
                    diff_data[pixel_idx + 2] = 0;   // B
                    diff_data[pixel_idx + 3] = 255; // A
                }
            } else if let Some(ref mut diff_data) = diff_image_data {
                // Matching pixel - show dimmed version
                let pixel_idx = (i / 4) * 4;
                if i % 4 == 0 {
                    diff_data[pixel_idx] = actual / 3;
                } else if i % 4 == 1 {
                    diff_data[pixel_idx + 1] = actual / 3;
                } else if i % 4 == 2 {
                    diff_data[pixel_idx + 2] = actual / 3;
                } else {
                    diff_data[pixel_idx + 3] = 255;
                }
            }
        }

        let total_pixels = width * height;
        let diff_percentage = if total_pixels > 0 {
            (diff_pixels as f64 / total_pixels as f64) * 100.0
        } else {
            0.0
        };

        let matches = diff_percentage <= self.config.max_diff_percentage;

        // Save diff image if configured and there are differences
        let diff_image_path = if !matches && self.config.generate_diff_image {
            if let Some(diff_data) = diff_image_data {
                let diff_dir = self.config.diff_output_dir.as_ref().unwrap_or(&self.golden_dir);
                std::fs::create_dir_all(diff_dir)
                    .map_err(|e| VisualTestError::ImageIo(e.to_string()))?;

                let diff_path = diff_dir.join(format!("{}_diff.png", test_name));
                self.save_image(&diff_path, &diff_data, width, height)?;

                // Also save actual image for comparison
                let actual_path = diff_dir.join(format!("{}_actual.png", test_name));
                self.save_image(&actual_path, &pixels, width, height)?;

                Some(diff_path)
            } else {
                None
            }
        } else {
            None
        };

        Ok(CompareResult {
            matches,
            diff_pixels,
            total_pixels,
            diff_image_path,
            description: if matches {
                format!(
                    "Images match (diff: {:.2}%, threshold: {:.2}%)",
                    diff_percentage, self.config.max_diff_percentage
                )
            } else {
                format!(
                    "Images differ: {} pixels ({:.2}%) exceed threshold ({:.2}%)",
                    diff_pixels, diff_percentage, self.config.max_diff_percentage
                )
            },
        })
    }

    /// Save RGBA pixel data to a PNG file.
    pub fn save_image(
        &self,
        path: &Path,
        pixels: &[u8],
        width: u32,
        height: u32,
    ) -> Result<(), VisualTestError> {
        let img = image::RgbaImage::from_raw(width, height, pixels.to_vec())
            .ok_or_else(|| VisualTestError::ImageIo("Invalid pixel data".into()))?;

        img.save(path)
            .map_err(|e| VisualTestError::ImageIo(e.to_string()))?;

        Ok(())
    }

    /// Verify that rendering a terminal produces non-empty output.
    ///
    /// This is a simpler check than full visual comparison - it just verifies
    /// that *something* was rendered (not all black/transparent).
    pub async fn verify_non_empty(
        &self,
        terminal: &Terminal,
        width: u32,
        height: u32,
    ) -> Result<bool, VisualTestError> {
        let pixels = self.render_to_pixels(terminal, width, height).await?;

        // Check if any non-background pixels exist
        // Background is typically (0, 0, 0, 255) or similar
        let mut has_content = false;
        for chunk in pixels.chunks(4) {
            let r = chunk[0];
            let g = chunk[1];
            let b = chunk[2];
            // Check if pixel is not black (allowing for some tolerance)
            if r > 5 || g > 5 || b > 5 {
                has_content = true;
                break;
            }
        }

        Ok(has_content)
    }

    /// Get the golden image directory.
    pub fn golden_dir(&self) -> &Path {
        &self.golden_dir
    }
}

/// Align a value to 256-byte boundary (wgpu buffer copy requirement).
fn align_to_256(value: u32) -> u32 {
    (value + 255) & !255
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_align_to_256() {
        assert_eq!(align_to_256(0), 0);
        assert_eq!(align_to_256(1), 256);
        assert_eq!(align_to_256(255), 256);
        assert_eq!(align_to_256(256), 256);
        assert_eq!(align_to_256(257), 512);
        assert_eq!(align_to_256(320), 512);
        assert_eq!(align_to_256(512), 512);
    }

    #[test]
    fn test_compare_result_diff_percentage() {
        let result = CompareResult {
            matches: true,
            diff_pixels: 25,
            total_pixels: 100,
            diff_image_path: None,
            description: String::new(),
        };
        assert!((result.diff_percentage() - 25.0).abs() < 0.001);

        let result_zero = CompareResult {
            matches: true,
            diff_pixels: 0,
            total_pixels: 0,
            diff_image_path: None,
            description: String::new(),
        };
        assert!((result_zero.diff_percentage() - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_compare_config_default() {
        let config = CompareConfig::default();
        assert!((config.max_diff_percentage - 0.0).abs() < 0.001);
        assert!(config.generate_diff_image);
        assert!(!config.update_golden);
    }
}
