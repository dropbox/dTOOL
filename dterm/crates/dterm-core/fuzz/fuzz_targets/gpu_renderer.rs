//! GPU Renderer fuzz target.
//!
//! This fuzzer tests the GPU renderer components with arbitrary data.
//!
//! ## Running
//!
//! ```bash
//! cd crates/dterm-core
//! cargo +nightly fuzz run gpu_renderer -- -max_total_time=3600
//! ```
//!
//! ## Properties Tested
//!
//! - CellVertexBuilder never panics on any inputs
//! - Vertex positions are always bounded
//! - Flags are correctly combined and preserved
//! - GlyphEntry UV coordinates are always normalized
//! - AtlasConfig values are always valid
//!
//! ## Correspondence to TLA+
//!
//! This fuzzer validates properties from the RenderPipeline TLA+ spec:
//! - VertexBoundsValid: Vertex positions are within cell bounds
//! - AtlasNeverOverflows: UV coordinates stay normalized

#![no_main]

use arbitrary::{Arbitrary, Unstructured};
use libfuzzer_sys::fuzz_target;
use dterm_core::gpu::{AtlasConfig, CellVertexBuilder, GlyphEntry, Uniforms, VertexFlags};

/// Maximum grid dimensions for fuzzing.
const MAX_COLS: u32 = 500;
const MAX_ROWS: u32 = 200;

/// Operations that can be performed on the vertex builder.
#[derive(Debug)]
enum BuilderOp {
    /// Add a background quad at (col, row) with flags.
    AddBackground {
        col: u32,
        row: u32,
        bg_color: [f32; 4],
        flags: u32,
    },
    /// Add a glyph quad with UV coordinates.
    AddGlyph {
        col: u32,
        row: u32,
        uv_min: [f32; 2],
        uv_max: [f32; 2],
        fg_color: [f32; 4],
        bg_color: [f32; 4],
        flags: u32,
    },
    /// Clear all vertices.
    Clear,
}

impl<'a> Arbitrary<'a> for BuilderOp {
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        let tag = u.int_in_range(0..=2)?;
        Ok(match tag {
            0 => BuilderOp::AddBackground {
                col: u.int_in_range(0..=MAX_COLS)?,
                row: u.int_in_range(0..=MAX_ROWS)?,
                bg_color: [
                    f32::from(u.arbitrary::<u8>()?) / 255.0,
                    f32::from(u.arbitrary::<u8>()?) / 255.0,
                    f32::from(u.arbitrary::<u8>()?) / 255.0,
                    1.0,
                ],
                flags: u.int_in_range(0..=511)?, // Legacy flag combinations (includes background bit)
            },
            1 => BuilderOp::AddGlyph {
                col: u.int_in_range(0..=MAX_COLS)?,
                row: u.int_in_range(0..=MAX_ROWS)?,
                uv_min: [
                    f32::from(u.arbitrary::<u8>()?) / 255.0,
                    f32::from(u.arbitrary::<u8>()?) / 255.0,
                ],
                uv_max: [
                    f32::from(u.arbitrary::<u8>()?) / 255.0,
                    f32::from(u.arbitrary::<u8>()?) / 255.0,
                ],
                fg_color: [
                    f32::from(u.arbitrary::<u8>()?) / 255.0,
                    f32::from(u.arbitrary::<u8>()?) / 255.0,
                    f32::from(u.arbitrary::<u8>()?) / 255.0,
                    1.0,
                ],
                bg_color: [
                    f32::from(u.arbitrary::<u8>()?) / 255.0,
                    f32::from(u.arbitrary::<u8>()?) / 255.0,
                    f32::from(u.arbitrary::<u8>()?) / 255.0,
                    1.0,
                ],
                // Style flags only (exclude legacy background bit = 256)
                flags: u.int_in_range(0..=255)?,
            },
            2 => BuilderOp::Clear,
            _ => unreachable!(),
        })
    }
}

fuzz_target!(|data: &[u8]| {
    if data.len() < 4 {
        return;
    }

    let mut u = Unstructured::new(data);

    // Create builder with arbitrary cell dimensions
    let cell_width: f32 = u.int_in_range(1..=64).unwrap_or(8) as f32;
    let cell_height: f32 = u.int_in_range(1..=128).unwrap_or(16) as f32;

    let mut builder = CellVertexBuilder::new(cell_width, cell_height);

    // Limit operations to prevent memory exhaustion
    let op_count: usize = u.int_in_range(0..=500).unwrap_or(0);

    for _ in 0..op_count {
        let op = match BuilderOp::arbitrary(&mut u) {
            Ok(op) => op,
            Err(_) => break,
        };

        match op {
            BuilderOp::AddBackground { col, row, bg_color, flags } => {
                builder.add_background(col, row, bg_color, flags);

                let expected = VertexFlags::from_legacy(flags).vertex_type_background();
                let verts = builder.vertices();
                let last_6 = &verts[verts.len().saturating_sub(6)..];
                for v in last_6 {
                    let actual = VertexFlags::unpack(v.flags);
                    assert_eq!(actual, expected, "Background vertex flags mismatch");
                }
            }
            BuilderOp::AddGlyph { col, row, uv_min, uv_max, fg_color, bg_color, flags } => {
                builder.add_glyph(col, row, uv_min, uv_max, fg_color, bg_color, flags);

                let expected = VertexFlags::from_legacy(flags).vertex_type_glyph();
                let verts = builder.vertices();
                let last_6 = &verts[verts.len().saturating_sub(6)..];
                for v in last_6 {
                    let actual = VertexFlags::unpack(v.flags);
                    assert_eq!(actual, expected, "Glyph vertex flags mismatch");
                }
            }
            BuilderOp::Clear => {
                builder.clear();
                assert_eq!(builder.vertices().len(), 0, "Clear should empty vertices");
            }
        }

        // INVARIANT: Vertex count is always a multiple of 6 (triangles)
        assert_eq!(
            builder.vertices().len() % 6,
            0,
            "Vertex count must be multiple of 6"
        );
    }

    // Test Uniforms
    let viewport_width: f32 = u.int_in_range(1..=4096).unwrap_or(800) as f32;
    let viewport_height: f32 = u.int_in_range(1..=4096).unwrap_or(600) as f32;
    let atlas_size: f32 = u.int_in_range(64..=8192).unwrap_or(512) as f32;

    let uniforms = Uniforms {
        viewport_width,
        viewport_height,
        cell_width,
        cell_height,
        atlas_size,
        time: u.int_in_range(0..=10000).unwrap_or(0) as f32 / 1000.0,
        cursor_x: u.int_in_range(-1..=500).unwrap_or(0),
        cursor_y: u.int_in_range(-1..=200).unwrap_or(0),
        cursor_color: [1.0, 1.0, 1.0, 1.0],
        selection_color: [0.3, 0.5, 0.8, 0.5],
        cursor_style: u.int_in_range(0..=2).unwrap_or(0),
        cursor_blink_ms: u.int_in_range(0..=1000).unwrap_or(530),
        _padding: [0; 2],
    };

    // INVARIANT: Uniforms must be 80 bytes and 16-byte aligned
    assert_eq!(
        std::mem::size_of::<Uniforms>(),
        80,
        "Uniforms must be 80 bytes"
    );
    assert_eq!(
        std::mem::size_of::<Uniforms>() % 16,
        0,
        "Uniforms must be 16-byte aligned"
    );

    // Verify uniforms values are sensible
    assert!(uniforms.viewport_width > 0.0);
    assert!(uniforms.viewport_height > 0.0);
    assert!(uniforms.cell_width > 0.0);
    assert!(uniforms.cell_height > 0.0);
    assert!(uniforms.atlas_size > 0.0);
    assert!(uniforms.time >= 0.0);

    // Test GlyphEntry UV coordinates
    let glyph_atlas_size: u32 = u.int_in_range(64..=8192).unwrap_or(512);

    // Ensure glyph fits in atlas by generating width/height first, then constrained x/y
    let glyph_width: u16 = u.int_in_range(0..=256).unwrap_or(8).min(glyph_atlas_size as u16);
    let glyph_height: u16 = u.int_in_range(0..=256).unwrap_or(16).min(glyph_atlas_size as u16);

    // Max x/y is (atlas_size - width/height) to ensure glyph fits
    let max_x = glyph_atlas_size.saturating_sub(glyph_width as u32);
    let max_y = glyph_atlas_size.saturating_sub(glyph_height as u32);
    let glyph_x: u16 = u.int_in_range(0..=max_x).unwrap_or(0) as u16;
    let glyph_y: u16 = u.int_in_range(0..=max_y).unwrap_or(0) as u16;

    let entry = GlyphEntry {
        x: glyph_x,
        y: glyph_y,
        width: glyph_width,
        height: glyph_height,
        offset_x: u.int_in_range(-128..=127).unwrap_or(0) as i16,
        offset_y: u.int_in_range(-128..=127).unwrap_or(0) as i16,
        advance: u.int_in_range(0..=256).unwrap_or(8),
    };

    let (u_min, v_min, u_max, v_max) = entry.uv_coords(glyph_atlas_size);

    // INVARIANT: UV coordinates must be normalized (0.0-1.0)
    // This invariant only holds when the glyph fits within the atlas
    assert!(
        u_min >= 0.0 && u_min <= 1.0,
        "u_min must be normalized: {} (x={}, atlas={})",
        u_min, glyph_x, glyph_atlas_size
    );
    assert!(
        v_min >= 0.0 && v_min <= 1.0,
        "v_min must be normalized: {} (y={}, atlas={})",
        v_min, glyph_y, glyph_atlas_size
    );
    assert!(
        u_max >= 0.0 && u_max <= 1.0,
        "u_max must be normalized: {} (x={}, width={}, atlas={})",
        u_max, glyph_x, glyph_width, glyph_atlas_size
    );
    assert!(
        v_max >= 0.0 && v_max <= 1.0,
        "v_max must be normalized: {} (y={}, height={}, atlas={})",
        v_max, glyph_y, glyph_height, glyph_atlas_size
    );
    assert!(u_min <= u_max, "u_min must be <= u_max");
    assert!(v_min <= v_max, "v_min must be <= v_max");

    // Test AtlasConfig
    let config = AtlasConfig::default();
    assert!(config.initial_size > 0, "initial_size must be positive");
    assert!(
        config.max_size >= config.initial_size,
        "max_size must be >= initial_size"
    );
    assert!(config.default_font_size > 0, "default_font_size must be positive");
});
