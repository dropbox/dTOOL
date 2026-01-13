//! Checkpoint file format definitions.

/// Magic bytes for checkpoint files.
pub const CHECKPOINT_MAGIC: [u8; 4] = *b"DTCK";

/// Checkpoint file version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum CheckpointVersion {
    /// Version 1: Initial format.
    V1 = 1,
}

impl CheckpointVersion {
    /// Get version number.
    #[must_use]
    pub const fn as_u32(self) -> u32 {
        self as u32
    }

    /// Create from version number.
    #[must_use]
    pub const fn from_u32(v: u32) -> Option<Self> {
        match v {
            1 => Some(Self::V1),
            _ => None,
        }
    }
}

/// Header flags.
#[derive(Debug, Clone, Copy, Default)]
pub struct HeaderFlags(u32);

impl HeaderFlags {
    /// Data is zstd compressed.
    pub const COMPRESSED: u32 = 1 << 0;
    /// Has scrollback data.
    pub const HAS_SCROLLBACK: u32 = 1 << 1;

    /// Create empty flags.
    #[must_use]
    pub const fn empty() -> Self {
        Self(0)
    }

    /// Check if flag is set.
    #[must_use]
    pub const fn contains(self, flag: u32) -> bool {
        (self.0 & flag) != 0
    }

    /// Set a flag.
    pub fn set(&mut self, flag: u32) {
        self.0 |= flag;
    }

    /// Get raw bits.
    #[must_use]
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// Create from raw bits.
    #[must_use]
    pub const fn from_bits(bits: u32) -> Self {
        Self(bits)
    }
}

/// Checkpoint file header (32 bytes).
///
/// ```text
/// Offset  Size  Field
/// 0       4     magic ("DTCK")
/// 4       4     version
/// 8       4     flags
/// 12      4     checksum
/// 16      8     grid_offset
/// 24      8     scrollback_offset
/// ```
#[derive(Debug, Clone, Copy)]
pub struct CheckpointHeader {
    /// File format version.
    version: CheckpointVersion,
    /// Header flags.
    flags: HeaderFlags,
    /// CRC32 checksum of data sections.
    checksum: u32,
    /// Offset to grid section.
    grid_offset: u64,
    /// Offset to scrollback section.
    scrollback_offset: u64,
}

impl CheckpointHeader {
    /// Create a new header with default values.
    #[must_use]
    pub fn new() -> Self {
        Self {
            version: CheckpointVersion::V1,
            flags: HeaderFlags::empty(),
            checksum: 0,
            grid_offset: 0,
            scrollback_offset: 0,
        }
    }

    /// Get the version.
    #[must_use]
    pub const fn version(&self) -> CheckpointVersion {
        self.version
    }

    /// Get flags.
    #[must_use]
    pub const fn flags(&self) -> HeaderFlags {
        self.flags
    }

    /// Get checksum.
    #[must_use]
    pub const fn checksum(&self) -> u32 {
        self.checksum
    }

    /// Set checksum.
    pub fn set_checksum(&mut self, checksum: u32) {
        self.checksum = checksum;
    }

    /// Get grid section offset.
    #[must_use]
    pub const fn grid_offset(&self) -> u64 {
        self.grid_offset
    }

    /// Set grid section offset.
    pub fn set_grid_offset(&mut self, offset: u64) {
        self.grid_offset = offset;
    }

    /// Get scrollback section offset.
    #[must_use]
    pub const fn scrollback_offset(&self) -> u64 {
        self.scrollback_offset
    }

    /// Set scrollback section offset.
    pub fn set_scrollback_offset(&mut self, offset: u64) {
        self.scrollback_offset = offset;
    }

    /// Check if data is compressed.
    #[must_use]
    pub const fn is_compressed(&self) -> bool {
        self.flags.contains(HeaderFlags::COMPRESSED)
    }

    /// Set compressed flag.
    pub fn set_compressed(&mut self, compressed: bool) {
        if compressed {
            self.flags.set(HeaderFlags::COMPRESSED);
        }
    }

    /// Serialize header to bytes.
    #[must_use]
    pub fn to_bytes(&self) -> [u8; 32] {
        let mut bytes = [0u8; 32];

        // Magic
        bytes[0..4].copy_from_slice(&CHECKPOINT_MAGIC);

        // Version
        bytes[4..8].copy_from_slice(&self.version.as_u32().to_le_bytes());

        // Flags
        bytes[8..12].copy_from_slice(&self.flags.bits().to_le_bytes());

        // Checksum
        bytes[12..16].copy_from_slice(&self.checksum.to_le_bytes());

        // Grid offset
        bytes[16..24].copy_from_slice(&self.grid_offset.to_le_bytes());

        // Scrollback offset
        bytes[24..32].copy_from_slice(&self.scrollback_offset.to_le_bytes());

        bytes
    }

    /// Deserialize header from bytes.
    #[must_use]
    pub fn from_bytes(bytes: &[u8; 32]) -> Option<Self> {
        // Check magic
        if bytes[0..4] != CHECKPOINT_MAGIC {
            return None;
        }

        // Parse version
        let version_num = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        let version = CheckpointVersion::from_u32(version_num)?;

        // Parse flags
        let flags_bits = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
        let flags = HeaderFlags::from_bits(flags_bits);

        // Parse checksum
        let checksum = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);

        // Parse offsets
        let grid_offset = u64::from_le_bytes([
            bytes[16], bytes[17], bytes[18], bytes[19], bytes[20], bytes[21], bytes[22], bytes[23],
        ]);

        let scrollback_offset = u64::from_le_bytes([
            bytes[24], bytes[25], bytes[26], bytes[27], bytes[28], bytes[29], bytes[30], bytes[31],
        ]);

        Some(Self {
            version,
            flags,
            checksum,
            grid_offset,
            scrollback_offset,
        })
    }
}

impl Default for CheckpointHeader {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_roundtrip() {
        let mut header = CheckpointHeader::new();
        header.set_compressed(true);
        header.set_checksum(0x12345678);
        header.set_grid_offset(100);
        header.set_scrollback_offset(500);

        let bytes = header.to_bytes();
        let restored = CheckpointHeader::from_bytes(&bytes).unwrap();

        assert_eq!(restored.version(), CheckpointVersion::V1);
        assert!(restored.is_compressed());
        assert_eq!(restored.checksum(), 0x12345678);
        assert_eq!(restored.grid_offset(), 100);
        assert_eq!(restored.scrollback_offset(), 500);
    }

    #[test]
    fn header_invalid_magic() {
        let bytes = [0u8; 32];
        assert!(CheckpointHeader::from_bytes(&bytes).is_none());
    }

    #[test]
    fn header_size() {
        assert_eq!(std::mem::size_of::<[u8; 32]>(), 32);
    }
}

#[cfg(kani)]
mod proofs {
    use super::*;

    /// Proof: Header serialization roundtrip preserves all fields.
    ///
    /// For any valid header state, to_bytes() followed by from_bytes()
    /// produces an identical header.
    #[kani::proof]
    fn header_roundtrip_preserves_fields() {
        // Create header with arbitrary values
        let checksum: u32 = kani::any();
        let grid_offset: u64 = kani::any();
        let scrollback_offset: u64 = kani::any();
        let compressed: bool = kani::any();
        let has_scrollback: bool = kani::any();

        let mut header = CheckpointHeader::new();
        header.set_checksum(checksum);
        header.set_grid_offset(grid_offset);
        header.set_scrollback_offset(scrollback_offset);
        if compressed {
            header.flags.set(HeaderFlags::COMPRESSED);
        }
        if has_scrollback {
            header.flags.set(HeaderFlags::HAS_SCROLLBACK);
        }

        // Serialize and deserialize
        let bytes = header.to_bytes();
        let restored = CheckpointHeader::from_bytes(&bytes);

        // Roundtrip must succeed for valid header
        kani::assert(restored.is_some(), "roundtrip must succeed");
        let restored = restored.unwrap();

        // All fields must be preserved
        kani::assert(
            restored.version() == CheckpointVersion::V1,
            "version preserved",
        );
        kani::assert(restored.checksum() == checksum, "checksum preserved");
        kani::assert(
            restored.grid_offset() == grid_offset,
            "grid_offset preserved",
        );
        kani::assert(
            restored.scrollback_offset() == scrollback_offset,
            "scrollback_offset preserved",
        );
        kani::assert(
            restored.flags().contains(HeaderFlags::COMPRESSED) == compressed,
            "compressed flag preserved",
        );
        kani::assert(
            restored.flags().contains(HeaderFlags::HAS_SCROLLBACK) == has_scrollback,
            "has_scrollback flag preserved",
        );
    }

    /// Proof: Invalid magic bytes are always rejected.
    ///
    /// from_bytes() returns None for any input that doesn't start with "DTCK".
    #[kani::proof]
    fn invalid_magic_rejected() {
        let mut bytes: [u8; 32] = kani::any();

        // Assume magic is NOT "DTCK"
        kani::assume(bytes[0] != b'D' || bytes[1] != b'T' || bytes[2] != b'C' || bytes[3] != b'K');

        let result = CheckpointHeader::from_bytes(&bytes);
        kani::assert(result.is_none(), "invalid magic must be rejected");
    }

    /// Proof: Valid magic with valid version is accepted.
    ///
    /// from_bytes() returns Some for valid magic + valid version.
    #[kani::proof]
    fn valid_magic_valid_version_accepted() {
        let mut bytes: [u8; 32] = [0u8; 32];

        // Set valid magic
        bytes[0] = b'D';
        bytes[1] = b'T';
        bytes[2] = b'C';
        bytes[3] = b'K';

        // Set version 1 (little-endian)
        bytes[4] = 1;
        bytes[5] = 0;
        bytes[6] = 0;
        bytes[7] = 0;

        // Rest of bytes are arbitrary
        let flags: u32 = kani::any();
        let checksum: u32 = kani::any();
        let grid_offset: u64 = kani::any();
        let scrollback_offset: u64 = kani::any();

        bytes[8..12].copy_from_slice(&flags.to_le_bytes());
        bytes[12..16].copy_from_slice(&checksum.to_le_bytes());
        bytes[16..24].copy_from_slice(&grid_offset.to_le_bytes());
        bytes[24..32].copy_from_slice(&scrollback_offset.to_le_bytes());

        let result = CheckpointHeader::from_bytes(&bytes);
        kani::assert(result.is_some(), "valid magic + v1 must be accepted");
    }

    /// Proof: Unknown version numbers are rejected.
    ///
    /// from_bytes() returns None for valid magic but unknown version.
    #[kani::proof]
    fn unknown_version_rejected() {
        let mut bytes: [u8; 32] = [0u8; 32];

        // Set valid magic
        bytes[0] = b'D';
        bytes[1] = b'T';
        bytes[2] = b'C';
        bytes[3] = b'K';

        // Set version (arbitrary non-1 value)
        let version: u32 = kani::any();
        kani::assume(version != 1); // Not V1

        bytes[4..8].copy_from_slice(&version.to_le_bytes());

        let result = CheckpointHeader::from_bytes(&bytes);
        kani::assert(result.is_none(), "unknown version must be rejected");
    }

    /// Proof: Version conversion is bijective for known versions.
    ///
    /// as_u32() and from_u32() are inverses for known versions.
    #[kani::proof]
    fn version_conversion_bijective() {
        // V1 roundtrips correctly
        let v1 = CheckpointVersion::V1;
        let v1_num = v1.as_u32();
        let restored = CheckpointVersion::from_u32(v1_num);
        kani::assert(restored == Some(CheckpointVersion::V1), "V1 roundtrips");

        // Unknown versions return None
        let unknown: u32 = kani::any();
        kani::assume(unknown != 1);
        let result = CheckpointVersion::from_u32(unknown);
        kani::assert(result.is_none(), "unknown versions return None");
    }

    /// Proof: HeaderFlags set/contains is consistent.
    ///
    /// After setting a flag, contains() returns true for that flag.
    #[kani::proof]
    fn header_flags_set_contains_consistent() {
        let mut flags = HeaderFlags::empty();

        // Initially empty
        kani::assert(
            !flags.contains(HeaderFlags::COMPRESSED),
            "initially no compressed",
        );
        kani::assert(
            !flags.contains(HeaderFlags::HAS_SCROLLBACK),
            "initially no scrollback",
        );

        // Set compressed
        flags.set(HeaderFlags::COMPRESSED);
        kani::assert(flags.contains(HeaderFlags::COMPRESSED), "compressed set");

        // Set scrollback
        flags.set(HeaderFlags::HAS_SCROLLBACK);
        kani::assert(
            flags.contains(HeaderFlags::HAS_SCROLLBACK),
            "scrollback set",
        );

        // Both should be set
        kani::assert(
            flags.contains(HeaderFlags::COMPRESSED),
            "compressed still set",
        );
    }

    /// Proof: HeaderFlags from_bits/bits roundtrip.
    #[kani::proof]
    fn header_flags_bits_roundtrip() {
        let bits: u32 = kani::any();
        let flags = HeaderFlags::from_bits(bits);
        kani::assert(flags.bits() == bits, "bits roundtrip");
    }
}
