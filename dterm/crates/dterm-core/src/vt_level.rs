//! VT conformance level tracking.
//!
//! This module tracks which VT terminal level (VT100, VT220, VT320, VT420, VT520)
//! each escape sequence belongs to. This enables:
//!
//! - Proper DA1/DA2 (Device Attributes) response generation
//! - DECSCL (Set Conformance Level) handling
//! - Knowing which features require which terminal level
//!
//! ## VT Terminal Evolution
//!
//! | Level | Year | Key Features |
//! |-------|------|--------------|
//! | VT100 | 1978 | Basic ANSI escape sequences, 80/132 columns |
//! | VT220 | 1983 | User-defined keys, 8-bit controls, DRCS |
//! | VT320 | 1987 | 25th status line, locator (mouse) events |
//! | VT420 | 1990 | Rectangular area operations, macro recording |
//! | VT520 | 1995 | Session management, printer features |
//!
//! ## Usage
//!
//! ```
//! use dterm_core::vt_level::{VtLevel, VtExtension, DeviceAttributes};
//!
//! // Check minimum level for a feature
//! assert!(VtLevel::VT320.supports_mouse());
//! assert!(!VtLevel::VT100.supports_mouse());
//!
//! // Get DA2 response parameter
//! assert_eq!(VtLevel::VT520.da2_param(), 64);
//! ```

/// VT terminal conformance level.
///
/// Each level implies support for all features from previous levels.
/// The integer values match the DA2 (Secondary Device Attributes) response.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[repr(u8)]
pub enum VtLevel {
    /// VT100 (1978): Basic ANSI sequences, 80/132 columns, smooth scroll
    VT100 = 0,
    /// VT220 (1983): User-defined keys, 8-bit controls, DRCS soft fonts
    VT220 = 1,
    /// VT240 (1983): VT220 + ReGIS and Sixel graphics
    VT240 = 2,
    /// VT320 (1987): 25th status line, locator (mouse) input
    #[default]
    VT320 = 24,
    /// VT330 (1987): VT320 + monochrome Sixel graphics
    VT330 = 18,
    /// VT340 (1987): VT320 + color Sixel graphics
    VT340 = 19,
    /// VT420 (1990): Rectangular operations, macro recording, pages
    VT420 = 41,
    /// VT510 (1993): Enhanced character sets
    VT510 = 61,
    /// VT520 (1995): Session management, enhanced printing
    VT520 = 64,
    /// VT525 (1995): VT520 + color
    VT525 = 65,
}

impl VtLevel {
    /// Get the DA2 (Secondary Device Attributes) parameter for this level.
    ///
    /// This is the first parameter in the response to `CSI > c`.
    #[must_use]
    pub const fn da2_param(self) -> u8 {
        self as u8
    }

    /// Create from DA2 parameter value.
    #[must_use]
    pub const fn from_da2_param(param: u8) -> Option<Self> {
        match param {
            0 => Some(Self::VT100),
            1 => Some(Self::VT220),
            2 => Some(Self::VT240),
            18 => Some(Self::VT330),
            19 => Some(Self::VT340),
            24 => Some(Self::VT320),
            41 => Some(Self::VT420),
            61 => Some(Self::VT510),
            64 => Some(Self::VT520),
            65 => Some(Self::VT525),
            _ => None,
        }
    }

    /// Get the DECSCL (Set Conformance Level) parameter for this level.
    ///
    /// Used in `CSI Ps ; Ps " p` sequence.
    #[must_use]
    pub const fn decscl_param(self) -> u8 {
        match self {
            Self::VT100 => 61, // VT100 mode
            Self::VT220 | Self::VT240 => 62,
            Self::VT320 | Self::VT330 | Self::VT340 => 63,
            Self::VT420 => 64,
            Self::VT510 | Self::VT520 | Self::VT525 => 65,
        }
    }

    /// Create from DECSCL parameter value.
    #[must_use]
    pub const fn from_decscl_param(param: u8) -> Option<Self> {
        match param {
            61 => Some(Self::VT100),
            62 => Some(Self::VT220),
            63 => Some(Self::VT320),
            64 => Some(Self::VT420),
            65 => Some(Self::VT520),
            _ => None,
        }
    }

    /// Human-readable name of this terminal level.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::VT100 => "VT100",
            Self::VT220 => "VT220",
            Self::VT240 => "VT240",
            Self::VT320 => "VT320",
            Self::VT330 => "VT330",
            Self::VT340 => "VT340",
            Self::VT420 => "VT420",
            Self::VT510 => "VT510",
            Self::VT520 => "VT520",
            Self::VT525 => "VT525",
        }
    }

    /// Check if this level supports 8-bit C1 control codes.
    #[must_use]
    pub const fn supports_c1_controls(self) -> bool {
        matches!(
            self,
            Self::VT220
                | Self::VT240
                | Self::VT320
                | Self::VT330
                | Self::VT340
                | Self::VT420
                | Self::VT510
                | Self::VT520
                | Self::VT525
        )
    }

    /// Check if this level supports user-defined keys (DECUDK).
    #[must_use]
    pub const fn supports_user_defined_keys(self) -> bool {
        self.supports_c1_controls() // VT220+
    }

    /// Check if this level supports DRCS (downloadable soft fonts).
    #[must_use]
    pub const fn supports_drcs(self) -> bool {
        self.supports_c1_controls() // VT220+
    }

    /// Check if this level supports Sixel graphics.
    #[must_use]
    pub const fn supports_sixel(self) -> bool {
        matches!(self, Self::VT240 | Self::VT330 | Self::VT340 | Self::VT525)
    }

    /// Check if this level supports locator (mouse) input.
    #[must_use]
    pub const fn supports_mouse(self) -> bool {
        matches!(
            self,
            Self::VT320
                | Self::VT330
                | Self::VT340
                | Self::VT420
                | Self::VT510
                | Self::VT520
                | Self::VT525
        )
    }

    /// Check if this level supports rectangular area operations.
    #[must_use]
    pub const fn supports_rectangular_ops(self) -> bool {
        matches!(self, Self::VT420 | Self::VT510 | Self::VT520 | Self::VT525)
    }

    /// Check if this level supports multiple pages.
    #[must_use]
    pub const fn supports_pages(self) -> bool {
        matches!(self, Self::VT420 | Self::VT510 | Self::VT520 | Self::VT525)
    }

    /// Check if this level supports session management.
    #[must_use]
    pub const fn supports_sessions(self) -> bool {
        matches!(self, Self::VT520 | Self::VT525)
    }
}

impl std::fmt::Display for VtLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// VT extension beyond the DEC standard.
///
/// Modern terminals support extensions to the original VT protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum VtExtension {
    /// No extension, strict DEC compliance
    #[default]
    None,
    /// Unknown extension
    Unknown,
    /// xterm extensions (common baseline)
    XTerm,
    /// iTerm2 extensions
    ITerm2,
    /// Kitty extensions
    Kitty,
}

impl VtExtension {
    /// Human-readable name of this extension.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Unknown => "unknown",
            Self::XTerm => "xterm",
            Self::ITerm2 => "iTerm2",
            Self::Kitty => "Kitty",
        }
    }
}

impl std::fmt::Display for VtExtension {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// Device attributes flags for DA1 response.
///
/// These are the feature flags reported in response to `CSI c` (DA1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DeviceAttributes(u32);

impl DeviceAttributes {
    /// No special attributes
    pub const NONE: Self = Self(0);
    /// 132 columns support
    pub const COLUMNS_132: Self = Self(1 << 0);
    /// Printer port
    pub const PRINTER: Self = Self(1 << 1);
    /// Selective erase (DECSED/DECSERA)
    pub const SELECTIVE_ERASE: Self = Self(1 << 2);
    /// User-defined keys (DECUDK)
    pub const USER_DEFINED_KEYS: Self = Self(1 << 3);
    /// National replacement character sets
    pub const NRCS: Self = Self(1 << 4);
    /// Technical character set
    pub const TECHNICAL_CHARS: Self = Self(1 << 5);
    /// ANSI color (SGR 30-37, 40-47)
    pub const ANSI_COLOR: Self = Self(1 << 6);
    /// ANSI text locator (mouse)
    pub const ANSI_TEXT_LOCATOR: Self = Self(1 << 7);
    /// Sixel graphics
    pub const SIXEL_GRAPHICS: Self = Self(1 << 8);
    /// Rectangular editing
    pub const RECTANGULAR_EDITING: Self = Self(1 << 9);
    /// Windowing (XTWINOPS)
    pub const WINDOWING: Self = Self(1 << 10);
    /// Capture screen buffer
    pub const CAPTURE_SCREEN: Self = Self(1 << 11);
    /// Horizontal scrolling
    pub const HORIZONTAL_SCROLLING: Self = Self(1 << 12);
    /// Color text (256 colors)
    pub const COLOR_256: Self = Self(1 << 13);
    /// True color (24-bit RGB)
    pub const TRUE_COLOR: Self = Self(1 << 14);

    /// Create from raw bits.
    #[must_use]
    pub const fn from_bits(bits: u32) -> Self {
        Self(bits)
    }

    /// Get the raw bits.
    #[must_use]
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// Check if the given attribute is set.
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Set the given attribute.
    #[must_use]
    pub const fn with(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Clear the given attribute.
    #[must_use]
    pub const fn without(self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }

    /// Default attributes for dterm (modern terminal emulator).
    #[must_use]
    pub const fn dterm_default() -> Self {
        Self(0)
            .with(Self::COLUMNS_132)
            .with(Self::SELECTIVE_ERASE)
            .with(Self::USER_DEFINED_KEYS)
            .with(Self::NRCS)
            .with(Self::ANSI_COLOR)
            .with(Self::ANSI_TEXT_LOCATOR)
            .with(Self::SIXEL_GRAPHICS)
            .with(Self::RECTANGULAR_EDITING)
            .with(Self::WINDOWING)
            .with(Self::COLOR_256)
            .with(Self::TRUE_COLOR)
    }

    /// Generate DA1 response parameters.
    ///
    /// Returns a vector of parameter values for the CSI response.
    #[must_use]
    pub fn to_params(&self) -> Vec<u16> {
        let mut params = vec![62]; // VT220+ baseline

        if self.contains(Self::COLUMNS_132) {
            params.push(1);
        }
        if self.contains(Self::PRINTER) {
            params.push(2);
        }
        if self.contains(Self::SELECTIVE_ERASE) {
            params.push(6);
        }
        if self.contains(Self::USER_DEFINED_KEYS) {
            params.push(8);
        }
        if self.contains(Self::NRCS) {
            params.push(9);
        }
        if self.contains(Self::SIXEL_GRAPHICS) {
            params.push(4);
        }
        if self.contains(Self::RECTANGULAR_EDITING) {
            params.push(15);
        }
        if self.contains(Self::ANSI_TEXT_LOCATOR) {
            params.push(29);
        }
        if self.contains(Self::HORIZONTAL_SCROLLING) {
            params.push(21);
        }
        if self.contains(Self::ANSI_COLOR) {
            params.push(22);
        }

        params
    }

    /// Generate DA1 response string.
    #[must_use]
    pub fn to_response(&self) -> String {
        let params = self.to_params();
        let param_str: Vec<String> = params.iter().map(|p| p.to_string()).collect();
        format!("\x1b[?{}c", param_str.join(";"))
    }
}

impl std::ops::BitOr for DeviceAttributes {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitAnd for DeviceAttributes {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0)
    }
}

/// Minimum VT level required for common sequences.
///
/// This function returns the minimum VT level needed to support
/// a given CSI final byte.
#[must_use]
pub const fn min_vt_level_for_csi(final_byte: u8, private: bool) -> VtLevel {
    if private {
        // Private mode CSI sequences (CSI ? ...)
        match final_byte {
            b'h' | b'l' => VtLevel::VT100, // DECSET/DECRST
            b'r' => VtLevel::VT420,        // DECSTBM with margins
            b'J' | b'K' => VtLevel::VT220, // DECSED/DECSEL
            b's' | b'u' => VtLevel::VT420, // DECSLRM/DECSLRM
            b'$' => VtLevel::VT420,        // Rectangular ops
            _ => VtLevel::VT100,
        }
    } else {
        // Standard CSI sequences
        match final_byte {
            // Cursor movement (VT100)
            b'A' | b'B' | b'C' | b'D' | b'H' | b'f' => VtLevel::VT100,
            // Line/character operations (VT100)
            b'J' | b'K' | b'L' | b'M' | b'P' | b'@' => VtLevel::VT100,
            // SGR (VT100, extended in VT220+)
            b'm' => VtLevel::VT100,
            // Scroll (VT100)
            b'S' | b'T' => VtLevel::VT100,
            // Cursor save/restore (VT100)
            b's' | b'u' => VtLevel::VT100,
            // Device attributes (VT100)
            b'c' => VtLevel::VT100,
            // Tabs (VT100)
            b'g' | b'I' | b'Z' => VtLevel::VT100,
            // Status request (VT100)
            b'n' => VtLevel::VT100,
            // Set scroll region (VT100)
            b'r' => VtLevel::VT100,
            // Character attributes (VT220)
            b'q' => VtLevel::VT220,
            // Insert/replace mode (VT220)
            b'h' | b'l' => VtLevel::VT220,
            // Character protection (VT220)
            b'"' => VtLevel::VT220,
            // Cursor style (VT520)
            b' ' => VtLevel::VT520,
            // Rectangular area ops (VT420)
            b'$' | b'\'' | b'*' | b'+' => VtLevel::VT420,
            // Window operations (xterm extension)
            b't' => VtLevel::VT100, // Widely supported
            // Soft reset (VT220)
            b'!' => VtLevel::VT220,
            _ => VtLevel::VT100,
        }
    }
}

/// Minimum VT level required for common ESC sequences.
#[must_use]
pub const fn min_vt_level_for_esc(intermediate: Option<u8>, final_byte: u8) -> VtLevel {
    match (intermediate, final_byte) {
        // Cursor save/restore (VT100)
        (None, b'7' | b'8') => VtLevel::VT100,
        // Index/reverse index (VT100)
        (None, b'D' | b'M' | b'E') => VtLevel::VT100,
        // Tab set (VT100)
        (None, b'H') => VtLevel::VT100,
        // Reset (VT100)
        (None, b'c') => VtLevel::VT100,
        // Designate character sets (VT100, extended in VT220)
        (Some(b'(' | b')' | b'*' | b'+'), _) => VtLevel::VT100,
        // DEC private modes
        (Some(b'#'), _) => VtLevel::VT100,
        // Application keypad
        (None, b'=' | b'>') => VtLevel::VT100,
        // Single shifts (VT220)
        (None, b'N' | b'O') => VtLevel::VT220,
        // Lock shifts (VT220)
        (None, b'n' | b'o' | b'|' | b'}' | b'~') => VtLevel::VT220,
        _ => VtLevel::VT100,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vt_level_ordering() {
        assert!(VtLevel::VT100 < VtLevel::VT220);
        assert!(VtLevel::VT220 < VtLevel::VT320);
        assert!(VtLevel::VT320 < VtLevel::VT420);
        assert!(VtLevel::VT420 < VtLevel::VT520);
    }

    #[test]
    fn da2_param_roundtrip() {
        for level in [
            VtLevel::VT100,
            VtLevel::VT220,
            VtLevel::VT240,
            VtLevel::VT320,
            VtLevel::VT330,
            VtLevel::VT340,
            VtLevel::VT420,
            VtLevel::VT510,
            VtLevel::VT520,
            VtLevel::VT525,
        ] {
            let param = level.da2_param();
            let recovered = VtLevel::from_da2_param(param);
            assert_eq!(recovered, Some(level), "Failed for {level}");
        }
    }

    #[test]
    fn decscl_param_roundtrip() {
        for level in [
            VtLevel::VT100,
            VtLevel::VT220,
            VtLevel::VT320,
            VtLevel::VT420,
            VtLevel::VT520,
        ] {
            let param = level.decscl_param();
            let recovered = VtLevel::from_decscl_param(param);
            assert!(recovered.is_some(), "Failed for {level}");
        }
    }

    #[test]
    fn c1_controls_support() {
        assert!(!VtLevel::VT100.supports_c1_controls());
        assert!(VtLevel::VT220.supports_c1_controls());
        assert!(VtLevel::VT520.supports_c1_controls());
    }

    #[test]
    fn sixel_support() {
        assert!(!VtLevel::VT100.supports_sixel());
        assert!(!VtLevel::VT220.supports_sixel());
        assert!(VtLevel::VT240.supports_sixel());
        assert!(VtLevel::VT340.supports_sixel());
        assert!(!VtLevel::VT420.supports_sixel()); // VT420 doesn't have graphics
        assert!(VtLevel::VT525.supports_sixel());
    }

    #[test]
    fn mouse_support() {
        assert!(!VtLevel::VT100.supports_mouse());
        assert!(!VtLevel::VT220.supports_mouse());
        assert!(VtLevel::VT320.supports_mouse());
        assert!(VtLevel::VT520.supports_mouse());
    }

    #[test]
    fn rectangular_ops_support() {
        assert!(!VtLevel::VT100.supports_rectangular_ops());
        assert!(!VtLevel::VT320.supports_rectangular_ops());
        assert!(VtLevel::VT420.supports_rectangular_ops());
        assert!(VtLevel::VT520.supports_rectangular_ops());
    }

    #[test]
    fn device_attributes_default() {
        let attrs = DeviceAttributes::dterm_default();
        assert!(attrs.contains(DeviceAttributes::COLUMNS_132));
        assert!(attrs.contains(DeviceAttributes::ANSI_COLOR));
        assert!(attrs.contains(DeviceAttributes::TRUE_COLOR));
        assert!(attrs.contains(DeviceAttributes::SIXEL_GRAPHICS));
    }

    #[test]
    fn device_attributes_to_response() {
        let attrs = DeviceAttributes::COLUMNS_132 | DeviceAttributes::ANSI_COLOR;
        let response = attrs.to_response();
        assert!(response.starts_with("\x1b[?"));
        assert!(response.ends_with('c'));
    }

    #[test]
    fn min_vt_level_csi() {
        assert_eq!(min_vt_level_for_csi(b'A', false), VtLevel::VT100);
        assert_eq!(min_vt_level_for_csi(b'm', false), VtLevel::VT100);
        assert_eq!(min_vt_level_for_csi(b'q', false), VtLevel::VT220);
        assert_eq!(min_vt_level_for_csi(b'$', false), VtLevel::VT420);
    }

    #[test]
    fn min_vt_level_esc() {
        assert_eq!(min_vt_level_for_esc(None, b'7'), VtLevel::VT100);
        assert_eq!(min_vt_level_for_esc(None, b'D'), VtLevel::VT100);
        assert_eq!(min_vt_level_for_esc(None, b'N'), VtLevel::VT220);
    }

    #[test]
    fn vt_extension_display() {
        assert_eq!(VtExtension::XTerm.to_string(), "xterm");
        assert_eq!(VtExtension::Kitty.to_string(), "Kitty");
    }
}

#[cfg(kani)]
mod proofs {
    use super::*;

    #[kani::proof]
    fn da2_param_valid_range() {
        let param: u8 = kani::any();
        if let Some(level) = VtLevel::from_da2_param(param) {
            kani::assert(level.da2_param() == param, "DA2 param should roundtrip");
        }
    }

    #[kani::proof]
    fn decscl_param_produces_valid_level() {
        let param: u8 = kani::any();
        kani::assume(param >= 61 && param <= 65);
        let level = VtLevel::from_decscl_param(param);
        kani::assert(level.is_some(), "Valid DECSCL params should parse");
    }

    #[kani::proof]
    fn device_attributes_bitops_idempotent() {
        let bits: u32 = kani::any();
        let attrs = DeviceAttributes::from_bits(bits);
        kani::assert(attrs.bits() == bits, "Bits should roundtrip");
    }

    #[kani::proof]
    fn device_attributes_with_idempotent() {
        let a = DeviceAttributes::COLUMNS_132;
        let b = a.with(DeviceAttributes::COLUMNS_132);
        kani::assert(a.bits() == b.bits(), "with same flag is idempotent");
    }

    #[kani::proof]
    fn vt_level_ordering_transitive() {
        // Just verify the ordering is consistent
        let vt100 = VtLevel::VT100;
        let vt220 = VtLevel::VT220;
        let vt520 = VtLevel::VT520;
        kani::assert(vt100 < vt220, "VT100 < VT220");
        kani::assert(vt220 < vt520, "VT220 < VT520");
        kani::assert(vt100 < vt520, "VT100 < VT520 (transitive)");
    }
}
