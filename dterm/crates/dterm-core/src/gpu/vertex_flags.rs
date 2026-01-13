//! Type-safe GPU vertex flags with clean separation of concerns.
//!
//! ## Bit Layout (u32)
//!
//! ```text
//! Bit:  31                    7  6  5  4  3  2  1  0
//!       [      Reserved      ][OV][  EFF  ][ TYPE ]
//!
//! TYPE (bits 0-1): VertexType enum
//!   0 = Glyph       - Sample from atlas texture
//!   1 = Background  - Solid background color
//!   2 = Decoration  - Solid foreground color (underlines, box drawing)
//!   3 = Reserved
//!
//! EFF (bits 2-4): EffectFlags
//!   bit 2 = DIM     - Reduce brightness by 50%
//!   bit 3 = BLINK   - Animate alpha
//!   bit 4 = INVERSE - Swap fg/bg colors
//!
//! OV (bits 5-6): OverlayFlags
//!   bit 5 = CURSOR    - Cursor highlight
//!   bit 6 = SELECTION - Selection highlight
//! ```
//!
//! Total: 7 bits used, 25 bits reserved for future expansion.

/// Vertex type determines the shader rendering path.
///
/// Packed into bits 0-1 of the flags u32.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VertexType {
    /// Sample from atlas texture, apply foreground color with alpha.
    #[default]
    Glyph = 0,
    /// Solid background color quad.
    Background = 1,
    /// Solid foreground color quad (underlines, strikethrough, box drawing).
    Decoration = 2,
}

impl VertexType {
    const MASK: u32 = 0b11;
    const SHIFT: u32 = 0;

    /// Pack into u32 flags.
    #[inline]
    pub const fn pack(self) -> u32 {
        (self as u32) << Self::SHIFT
    }

    /// Unpack from u32 flags.
    #[inline]
    pub const fn unpack(flags: u32) -> Self {
        match (flags >> Self::SHIFT) & Self::MASK {
            0 => Self::Glyph,
            1 => Self::Background,
            2 => Self::Decoration,
            _ => Self::Glyph, // Reserved maps to Glyph
        }
    }
}

bitflags::bitflags! {
    /// Effect modifiers that change how the vertex is rendered.
    ///
    /// Packed into bits 2-4 of the flags u32.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct EffectFlags: u8 {
        /// Dim the color by 50%.
        const DIM = 0b001;
        /// Animate alpha (blink effect).
        const BLINK = 0b010;
        /// Swap foreground and background colors.
        const INVERSE = 0b100;
    }
}

impl EffectFlags {
    const MASK: u32 = 0b111;
    const SHIFT: u32 = 2;

    /// Pack into u32 flags.
    #[inline]
    pub const fn pack(self) -> u32 {
        (self.bits() as u32) << Self::SHIFT
    }

    /// Unpack from u32 flags.
    #[inline]
    pub fn unpack(flags: u32) -> Self {
        Self::from_bits_truncate(((flags >> Self::SHIFT) & Self::MASK) as u8)
    }
}

bitflags::bitflags! {
    /// Overlay indicators for cursor and selection.
    ///
    /// Packed into bits 5-6 of the flags u32.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct OverlayFlags: u8 {
        /// This vertex is under the cursor.
        const CURSOR = 0b01;
        /// This vertex is selected.
        const SELECTION = 0b10;
    }
}

impl OverlayFlags {
    const MASK: u32 = 0b11;
    const SHIFT: u32 = 5;

    /// Pack into u32 flags.
    #[inline]
    pub const fn pack(self) -> u32 {
        (self.bits() as u32) << Self::SHIFT
    }

    /// Unpack from u32 flags.
    #[inline]
    pub fn unpack(flags: u32) -> Self {
        Self::from_bits_truncate(((flags >> Self::SHIFT) & Self::MASK) as u8)
    }
}

// =============================================================================
// GPU-compatible constants (for shader and FFI)
// =============================================================================

/// Vertex type: Glyph (sample atlas texture).
pub const VERTEX_TYPE_GLYPH: u32 = 0;
/// Vertex type: Background (solid bg color).
pub const VERTEX_TYPE_BACKGROUND: u32 = 1;
/// Vertex type: Decoration (solid fg color).
pub const VERTEX_TYPE_DECORATION: u32 = 2;
/// Mask for extracting vertex type from flags.
pub const VERTEX_TYPE_MASK: u32 = 0b11;

/// Effect: Dim (reduce brightness).
pub const EFFECT_DIM: u32 = 1 << 2;
/// Effect: Blink (animate alpha).
pub const EFFECT_BLINK: u32 = 1 << 3;
/// Effect: Inverse (swap fg/bg).
pub const EFFECT_INVERSE: u32 = 1 << 4;

/// Overlay: Cursor highlight.
pub const OVERLAY_CURSOR: u32 = 1 << 5;
/// Overlay: Selection highlight.
pub const OVERLAY_SELECTION: u32 = 1 << 6;

// =============================================================================
// Combined VertexFlags struct
// =============================================================================

/// Combined vertex flags for GPU transmission.
///
/// This struct provides type-safe construction and manipulation while
/// packing into a single u32 for GPU compatibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[must_use]
pub struct VertexFlags {
    /// The type of vertex (determines shader path).
    pub vertex_type: VertexType,
    /// Effect modifiers.
    pub effects: EffectFlags,
    /// Overlay indicators.
    pub overlays: OverlayFlags,
}

impl VertexFlags {
    /// Create new flags with specified vertex type.
    #[inline]
    pub const fn new(vertex_type: VertexType) -> Self {
        Self {
            vertex_type,
            effects: EffectFlags::empty(),
            overlays: OverlayFlags::empty(),
        }
    }

    /// Create glyph vertex flags.
    #[inline]
    pub const fn glyph() -> Self {
        Self::new(VertexType::Glyph)
    }

    /// Create background vertex flags.
    #[inline]
    pub const fn background() -> Self {
        Self::new(VertexType::Background)
    }

    /// Create decoration vertex flags.
    #[inline]
    pub const fn decoration() -> Self {
        Self::new(VertexType::Decoration)
    }

    /// Set effects (replaces existing).
    #[inline]
    pub const fn with_effects(mut self, effects: EffectFlags) -> Self {
        self.effects = effects;
        self
    }

    /// Add dim effect.
    #[inline]
    pub fn with_dim(mut self) -> Self {
        self.effects = self.effects.union(EffectFlags::DIM);
        self
    }

    /// Add blink effect.
    #[inline]
    pub fn with_blink(mut self) -> Self {
        self.effects = self.effects.union(EffectFlags::BLINK);
        self
    }

    /// Add inverse effect.
    #[inline]
    pub fn with_inverse(mut self) -> Self {
        self.effects = self.effects.union(EffectFlags::INVERSE);
        self
    }

    /// Set overlays (replaces existing).
    #[inline]
    pub const fn with_overlays(mut self, overlays: OverlayFlags) -> Self {
        self.overlays = overlays;
        self
    }

    /// Add cursor overlay.
    #[inline]
    pub fn with_cursor(mut self) -> Self {
        self.overlays = self.overlays.union(OverlayFlags::CURSOR);
        self
    }

    /// Add selection overlay.
    #[inline]
    pub fn with_selection(mut self) -> Self {
        self.overlays = self.overlays.union(OverlayFlags::SELECTION);
        self
    }

    /// Check if this has cursor overlay.
    #[inline]
    pub const fn has_cursor(&self) -> bool {
        self.overlays.contains(OverlayFlags::CURSOR)
    }

    /// Check if this has selection overlay.
    #[inline]
    pub const fn has_selection(&self) -> bool {
        self.overlays.contains(OverlayFlags::SELECTION)
    }

    /// Set vertex type to Background (keeps effects and overlays).
    #[inline]
    pub fn vertex_type_background(mut self) -> Self {
        self.vertex_type = VertexType::Background;
        self
    }

    /// Set vertex type to Decoration (keeps effects and overlays).
    #[inline]
    pub fn vertex_type_decoration(mut self) -> Self {
        self.vertex_type = VertexType::Decoration;
        self
    }

    /// Set vertex type to Glyph (keeps effects and overlays).
    #[inline]
    pub fn vertex_type_glyph(mut self) -> Self {
        self.vertex_type = VertexType::Glyph;
        self
    }

    /// Pack into u32 for GPU.
    #[inline]
    pub const fn pack(self) -> u32 {
        self.vertex_type.pack() | self.effects.pack() | self.overlays.pack()
    }

    /// Unpack from u32.
    #[inline]
    pub fn unpack(flags: u32) -> Self {
        Self {
            vertex_type: VertexType::unpack(flags),
            effects: EffectFlags::unpack(flags),
            overlays: OverlayFlags::unpack(flags),
        }
    }
}

impl From<VertexFlags> for u32 {
    #[inline]
    fn from(flags: VertexFlags) -> u32 {
        flags.pack()
    }
}

impl From<u32> for VertexFlags {
    #[inline]
    fn from(flags: u32) -> Self {
        Self::unpack(flags)
    }
}

// =============================================================================
// Legacy conversion (for gradual migration from old flags)
// =============================================================================

/// Old flag values (for migration only).
pub(crate) mod legacy {
    pub(crate) const FLAG_DIM: u32 = 2;
    pub(crate) const FLAG_BLINK: u32 = 8;
    pub(crate) const FLAG_INVERSE: u32 = 16;
    pub(crate) const FLAG_IS_CURSOR: u32 = 64;
    pub(crate) const FLAG_IS_SELECTION: u32 = 128;
    pub(crate) const FLAG_IS_BACKGROUND: u32 = 256;
    pub(crate) const FLAG_IS_DECORATION: u32 = 2048;
}

impl VertexFlags {
    /// Convert from old-style flags to new VertexFlags.
    ///
    /// This enables gradual migration without breaking existing code.
    #[inline]
    pub fn from_legacy(old_flags: u32) -> Self {
        let vertex_type = if old_flags & legacy::FLAG_IS_DECORATION != 0 {
            VertexType::Decoration
        } else if old_flags & legacy::FLAG_IS_BACKGROUND != 0 {
            VertexType::Background
        } else {
            VertexType::Glyph
        };

        let mut effects = EffectFlags::empty();
        if old_flags & legacy::FLAG_DIM != 0 {
            effects |= EffectFlags::DIM;
        }
        if old_flags & legacy::FLAG_BLINK != 0 {
            effects |= EffectFlags::BLINK;
        }
        if old_flags & legacy::FLAG_INVERSE != 0 {
            effects |= EffectFlags::INVERSE;
        }

        let mut overlays = OverlayFlags::empty();
        if old_flags & legacy::FLAG_IS_CURSOR != 0 {
            overlays |= OverlayFlags::CURSOR;
        }
        if old_flags & legacy::FLAG_IS_SELECTION != 0 {
            overlays |= OverlayFlags::SELECTION;
        }

        Self {
            vertex_type,
            effects,
            overlays,
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vertex_type_roundtrip() {
        for vt in [VertexType::Glyph, VertexType::Background, VertexType::Decoration] {
            let packed = vt.pack();
            let unpacked = VertexType::unpack(packed);
            assert_eq!(vt, unpacked);
        }
    }

    #[test]
    fn effect_flags_roundtrip() {
        let effects = EffectFlags::DIM | EffectFlags::INVERSE;
        let packed = effects.pack();
        let unpacked = EffectFlags::unpack(packed);
        assert_eq!(effects, unpacked);
    }

    #[test]
    fn overlay_flags_roundtrip() {
        let overlays = OverlayFlags::CURSOR | OverlayFlags::SELECTION;
        let packed = overlays.pack();
        let unpacked = OverlayFlags::unpack(packed);
        assert_eq!(overlays, unpacked);
    }

    #[test]
    fn combined_flags_roundtrip() {
        let flags = VertexFlags::decoration().with_dim().with_cursor();

        let packed = flags.pack();
        let unpacked = VertexFlags::unpack(packed);

        assert_eq!(flags.vertex_type, unpacked.vertex_type);
        assert_eq!(flags.effects, unpacked.effects);
        assert_eq!(flags.overlays, unpacked.overlays);
    }

    #[test]
    fn bits_dont_overlap() {
        // Verify no bit position is used by multiple flag groups
        let type_bits = VertexType::Decoration.pack();
        let effect_bits = (EffectFlags::DIM | EffectFlags::BLINK | EffectFlags::INVERSE).pack();
        let overlay_bits = (OverlayFlags::CURSOR | OverlayFlags::SELECTION).pack();

        assert_eq!(type_bits & effect_bits, 0, "type and effect overlap");
        assert_eq!(type_bits & overlay_bits, 0, "type and overlay overlap");
        assert_eq!(effect_bits & overlay_bits, 0, "effect and overlay overlap");
    }

    #[test]
    fn constants_match_packed() {
        assert_eq!(VERTEX_TYPE_GLYPH, VertexType::Glyph.pack());
        assert_eq!(VERTEX_TYPE_BACKGROUND, VertexType::Background.pack());
        assert_eq!(VERTEX_TYPE_DECORATION, VertexType::Decoration.pack());
        assert_eq!(EFFECT_DIM, EffectFlags::DIM.pack());
        assert_eq!(EFFECT_BLINK, EffectFlags::BLINK.pack());
        assert_eq!(EFFECT_INVERSE, EffectFlags::INVERSE.pack());
        assert_eq!(OVERLAY_CURSOR, OverlayFlags::CURSOR.pack());
        assert_eq!(OVERLAY_SELECTION, OverlayFlags::SELECTION.pack());
    }

    #[test]
    fn legacy_conversion() {
        // Old FLAG_IS_DECORATION | FLAG_DIM | FLAG_IS_CURSOR
        let old_flags: u32 = 2048 | 2 | 64;
        let converted = VertexFlags::from_legacy(old_flags);

        assert_eq!(converted.vertex_type, VertexType::Decoration);
        assert!(converted.effects.contains(EffectFlags::DIM));
        assert!(converted.overlays.contains(OverlayFlags::CURSOR));
    }

    #[test]
    fn u32_conversion() {
        let flags = VertexFlags::background().with_inverse().with_selection();
        let packed: u32 = flags.into();
        let unpacked: VertexFlags = packed.into();
        assert_eq!(flags, unpacked);
    }

    // =========================================================================
    // CRITICAL: Shader/FFI compatibility regression tests
    // =========================================================================
    // These tests verify the EXACT bit values that shaders expect.
    // DO NOT CHANGE these values without updating ALL shaders (WGSL + Metal).

    #[test]
    fn shader_compatibility_exact_bit_values() {
        // CRITICAL: These exact values are hardcoded in shader.wgsl and
        // docs/METAL_SHADER_MIGRATION.md. Any change breaks rendering.
        assert_eq!(VERTEX_TYPE_GLYPH, 0, "WGSL/Metal expects GLYPH=0");
        assert_eq!(VERTEX_TYPE_BACKGROUND, 1, "WGSL/Metal expects BACKGROUND=1");
        assert_eq!(VERTEX_TYPE_DECORATION, 2, "WGSL/Metal expects DECORATION=2");
        assert_eq!(VERTEX_TYPE_MASK, 3, "WGSL/Metal expects TYPE_MASK=3");

        assert_eq!(EFFECT_DIM, 4, "WGSL/Metal expects DIM=4 (bit 2)");
        assert_eq!(EFFECT_BLINK, 8, "WGSL/Metal expects BLINK=8 (bit 3)");
        assert_eq!(EFFECT_INVERSE, 16, "WGSL/Metal expects INVERSE=16 (bit 4)");

        assert_eq!(OVERLAY_CURSOR, 32, "WGSL/Metal expects CURSOR=32 (bit 5)");
        assert_eq!(OVERLAY_SELECTION, 64, "WGSL/Metal expects SELECTION=64 (bit 6)");
    }

    #[test]
    fn legacy_module_matches_pipeline_constants() {
        // CRITICAL: The legacy module must match pipeline.rs constants
        // for from_legacy() to work correctly.
        use super::super::pipeline;

        assert_eq!(legacy::FLAG_DIM, pipeline::FLAG_DIM);
        assert_eq!(legacy::FLAG_BLINK, pipeline::FLAG_BLINK);
        assert_eq!(legacy::FLAG_INVERSE, pipeline::FLAG_INVERSE);
        assert_eq!(legacy::FLAG_IS_CURSOR, pipeline::FLAG_IS_CURSOR);
        assert_eq!(legacy::FLAG_IS_SELECTION, pipeline::FLAG_IS_SELECTION);
        assert_eq!(legacy::FLAG_IS_BACKGROUND, pipeline::FLAG_IS_BACKGROUND);
        assert_eq!(legacy::FLAG_IS_DECORATION, pipeline::FLAG_IS_DECORATION);
    }

    #[test]
    fn box_drawing_uses_new_format() {
        // CRITICAL: box_drawing.rs must use VERTEX_TYPE_DECORATION (new format),
        // not FLAG_IS_DECORATION (old format = 2048).
        // This ensures box drawing characters render correctly.
        assert_eq!(
            VERTEX_TYPE_DECORATION, 2,
            "Box drawing expects DECORATION=2, not old value 2048"
        );
        assert_ne!(
            VERTEX_TYPE_DECORATION,
            legacy::FLAG_IS_DECORATION,
            "New format must differ from legacy"
        );
    }

    #[test]
    fn seven_bit_layout_fits_in_byte() {
        // The new format uses only 7 bits (0-6), leaving bit 7+ reserved.
        let max_flags = VertexFlags::decoration()
            .with_dim()
            .with_blink()
            .with_inverse()
            .with_cursor()
            .with_selection();
        let packed = max_flags.pack();
        assert!(packed < 128, "All flags should fit in 7 bits (< 128)");
        assert_eq!(packed, 2 | 4 | 8 | 16 | 32 | 64); // = 126
    }
}
